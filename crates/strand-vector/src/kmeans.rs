// Copyright the STRAND authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! K-means clustering: partitions raw (unrotated) vectors into clusters,
//! producing the centroids `crate::navigation`'s navigation tier stores
//! and the assignments `crate::posting_list`'s per-cluster row-id arrays
//! need. This is real, separate, construction-side work RFC 0010's own
//! Design §3 named as out of scope for a read-side wire-format RFC —
//! "the actual clustering algorithm (k-means or otherwise)... is
//! construction-side" — and Non-goals confirmed no algorithm choice is
//! wire-format-relevant: the format stores whatever centroids and cluster
//! assignments a writer produces, however it produced them.
//!
//! **This is a genuinely different grounding situation from every other
//! module in this crate.** `descriptor.rs`, `rotate.rs`, `quantize.rs`,
//! and `estimate.rs` all had to match a specific reference implementation
//! byte-for-byte or bit-for-bit, because their output is wire-visible and
//! interop-relevant. Clustering has no such constraint: the reference
//! library itself ships no C++ k-means at all — its own construction-side
//! tooling (`python/ivf.py`, per `references/rabitq-library-ivf-and-
//! batch-layout-source.md`) delegates to Faiss, an entirely separate
//! library this project has no reason to vendor or byte-match. What this
//! module owes is mathematical correctness against the well-known,
//! decades-old standard algorithm — Lloyd's algorithm (Lloyd, "Least
//! Squares Quantization in PCM," 1982, originally circulated 1957) with
//! k-means++ seeding (Arthur & Vassilvitskii, "k-means++: The Advantages
//! of Careful Seeding," SODA 2007) — not byte-identical reproduction of
//! any one implementation. Verified here by testing the properties a
//! correct implementation must have (inertia never increases across an
//! iteration, no empty clusters survive to the result, deterministic
//! given a seed), not by cross-checking against a compiled reference.
//!
//! Clustering runs on **raw, unrotated** vectors — the real construction
//! order the reference library's own IVF documentation states plainly
//! (`references/rabitq-library-ivf-and-batch-layout-source.md`: "The
//! first step is to run a clustering algorithm to partition raw data
//! vectors... [then] we first rotate the centroid and vectors in this
//! cluster, then compute the 1-bit codes"). The resulting centroids are
//! rotated afterward, with `crate::rotate::rotate_fht_kac`, before being
//! used anywhere quantization-related — this module never rotates
//! anything itself.

use rand::Rng;

/// The reference IVF documentation's own recommended cluster count —
/// `4·√N`, "following Faiss" (`references/rabitq-library-ivf-and-batch-
/// layout-source.md`) — already cited in RFC 0010's own napkin math
/// (Design §3). A starting point, not a requirement this module enforces:
/// callers may pass any `k` to `kmeans`.
pub fn recommended_cluster_count(num_vectors: usize) -> usize {
    ((4.0 * (num_vectors as f64).sqrt()).round() as usize).max(1)
}

/// The result of clustering: `num_clusters * dims` centroids (row-major,
/// raw/unrotated) and one cluster index per input vector.
#[derive(Debug, Clone, PartialEq)]
pub struct KMeansResult {
    pub centroids: Vec<f32>,
    pub assignments: Vec<usize>,
}

fn squared_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(&x, &y)| (x - y) * (x - y)).sum()
}

/// K-means++ seeding (Arthur & Vassilvitskii, SODA 2007): the first
/// centroid is chosen uniformly at random from `vectors`; each subsequent
/// centroid is chosen from the remaining points with probability
/// proportional to its squared distance to the nearest already-chosen
/// centroid (`D(x)^2`, in the paper's own notation — `D(x)` itself is a
/// distance, not a squared distance, so the sampling weight is exactly
/// the squared distance this function already computes, no extra
/// squaring needed).
///
/// # Panics
///
/// Panics if `n == 0`, `dims == 0`, or `k == 0` or `k > n`.
fn kmeans_plus_plus_init(
    vectors: &[f32],
    n: usize,
    dims: usize,
    k: usize,
    rng: &mut impl Rng,
) -> Vec<f32> {
    assert!(n > 0 && dims > 0, "n and dims must be non-zero");
    assert!(k > 0 && k <= n, "k must be in 1..=n");

    let mut centroids = Vec::with_capacity(k * dims);
    let first = rng.random_range(0..n);
    centroids.extend_from_slice(&vectors[first * dims..(first + 1) * dims]);

    // nearest_sqdist[i] = squared distance from vectors[i] to the nearest
    // centroid chosen so far.
    let mut nearest_sqdist: Vec<f32> = (0..n)
        .map(|i| squared_distance(&vectors[i * dims..(i + 1) * dims], &centroids[0..dims]))
        .collect();

    while centroids.len() / dims < k {
        let total: f64 = nearest_sqdist.iter().map(|&d| f64::from(d)).sum();
        let chosen = if total <= 0.0 {
            // All remaining points coincide with an already-chosen
            // centroid (degenerate input, e.g. many duplicate vectors) —
            // fall back to uniform choice rather than dividing by zero.
            rng.random_range(0..n)
        } else {
            let mut target = rng.random_range(0.0..total);
            let mut chosen = n - 1;
            for (i, &d) in nearest_sqdist.iter().enumerate() {
                target -= f64::from(d);
                if target <= 0.0 {
                    chosen = i;
                    break;
                }
            }
            chosen
        };

        let new_centroid = vectors[chosen * dims..(chosen + 1) * dims].to_vec();
        for (i, d) in nearest_sqdist.iter_mut().enumerate() {
            let dist = squared_distance(&vectors[i * dims..(i + 1) * dims], &new_centroid);
            if dist < *d {
                *d = dist;
            }
        }
        centroids.extend_from_slice(&new_centroid);
    }

    centroids
}

/// Runs Lloyd's algorithm to convergence (or `max_iters`, whichever comes
/// first), seeded with k-means++, on `n` raw `dims`-dimensional vectors
/// (`vectors` is `n * dims` f32 values, row-major).
///
/// A cluster that loses all its members during an iteration is
/// re-seeded to the point currently contributing the most squared error
/// (farthest from its own assigned centroid) — a standard, widely-used
/// empty-cluster recovery strategy (e.g. scikit-learn's own `KMeans`
/// applies the same idea) that keeps every returned cluster non-empty,
/// which `crate::posting_list::build_posting_lists` requires (an empty
/// `ClusterInput` is a real, if degenerate, case that RFC 0010's own wire
/// format tolerates — a cluster with `vector_count = 0` is a valid
/// directory entry — but is a poor clustering outcome this module avoids
/// producing when it can).
///
/// # Panics
///
/// Panics if `vectors.len() != n * dims`, if `n == 0` or `dims == 0`, or
/// if `k == 0` or `k > n`.
pub fn kmeans(
    vectors: &[f32],
    n: usize,
    dims: usize,
    k: usize,
    max_iters: usize,
    rng: &mut impl Rng,
) -> KMeansResult {
    assert_eq!(
        vectors.len(),
        n * dims,
        "vectors must be exactly n*dims f32 values"
    );
    assert!(n > 0 && dims > 0, "n and dims must be non-zero");
    assert!(k > 0 && k <= n, "k must be in 1..=n");

    let mut centroids = kmeans_plus_plus_init(vectors, n, dims, k, rng);
    let mut assignments = vec![0usize; n];

    for _ in 0..max_iters.max(1) {
        // Assignment step: each point to its nearest centroid.
        let mut changed = false;
        for i in 0..n {
            let point = &vectors[i * dims..(i + 1) * dims];
            let mut best = 0usize;
            let mut best_dist = f32::INFINITY;
            for c in 0..k {
                let dist = squared_distance(point, &centroids[c * dims..(c + 1) * dims]);
                if dist < best_dist {
                    best_dist = dist;
                    best = c;
                }
            }
            if assignments[i] != best {
                changed = true;
            }
            assignments[i] = best;
        }

        // Update step: each centroid becomes the mean of its members.
        let mut sums = vec![0f32; k * dims];
        let mut counts = vec![0usize; k];
        for i in 0..n {
            let c = assignments[i];
            counts[c] += 1;
            for d in 0..dims {
                sums[c * dims + d] += vectors[i * dims + d];
            }
        }

        // Empty-cluster recovery: reseed from the point with the largest
        // squared distance to its own assigned centroid (the point
        // contributing the most to total inertia), then treat it as its
        // own singleton cluster for this round's mean computation.
        for c in 0..k {
            if counts[c] == 0 {
                let mut worst = 0usize;
                let mut worst_dist = -1f32;
                for i in 0..n {
                    let own = assignments[i];
                    let dist = squared_distance(
                        &vectors[i * dims..(i + 1) * dims],
                        &centroids[own * dims..(own + 1) * dims],
                    );
                    if dist > worst_dist {
                        worst_dist = dist;
                        worst = i;
                    }
                }
                assignments[worst] = c;
                counts[c] = 1;
                for d in 0..dims {
                    sums[c * dims + d] = vectors[worst * dims + d];
                }
                changed = true;
            }
        }

        for c in 0..k {
            for d in 0..dims {
                centroids[c * dims + d] = sums[c * dims + d] / counts[c] as f32;
            }
        }

        if !changed {
            break;
        }
    }

    KMeansResult {
        centroids,
        assignments,
    }
}

/// Total within-cluster sum of squared distances (inertia) — the
/// objective Lloyd's algorithm minimizes. Exposed for testing (inertia
/// must never increase across an iteration of a correct implementation)
/// and for callers that want to compare multiple runs.
pub fn inertia(vectors: &[f32], n: usize, dims: usize, result: &KMeansResult) -> f64 {
    (0..n)
        .map(|i| {
            let c = result.assignments[i];
            f64::from(squared_distance(
                &vectors[i * dims..(i + 1) * dims],
                &result.centroids[c * dims..(c + 1) * dims],
            ))
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn three_well_separated_blobs() -> (Vec<f32>, usize, usize) {
        // 30 points total, three tight clusters far apart in 2D.
        let dims = 2;
        let mut vectors = Vec::new();
        let mut state = 0x5EED_u64;
        let mut jitter = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            ((state % 21) as f32 - 10.0) / 100.0 // small noise, [-0.1, 0.1]
        };
        for &(cx, cy) in &[(0.0f32, 0.0f32), (100.0, 0.0), (50.0, 100.0)] {
            for _ in 0..10 {
                vectors.push(cx + jitter());
                vectors.push(cy + jitter());
            }
        }
        let n = vectors.len() / dims;
        (vectors, n, dims)
    }

    #[test]
    fn recovers_well_separated_clusters() {
        let (vectors, n, dims) = three_well_separated_blobs();
        let mut rng = StdRng::seed_from_u64(1);
        let result = kmeans(&vectors, n, dims, 3, 100, &mut rng);

        // Every point within a true blob (10 consecutive points) must
        // share the same assigned cluster.
        for blob in 0..3 {
            let start = blob * 10;
            let first_assignment = result.assignments[start];
            for i in start..start + 10 {
                assert_eq!(
                    result.assignments[i], first_assignment,
                    "point {i} in blob {blob} was not clustered with the rest of its blob"
                );
            }
        }
        // The three blobs must land in three genuinely different clusters.
        let mut blob_clusters: Vec<usize> = (0..3).map(|b| result.assignments[b * 10]).collect();
        blob_clusters.sort_unstable();
        blob_clusters.dedup();
        assert_eq!(
            blob_clusters.len(),
            3,
            "the three well-separated blobs must map to three distinct clusters"
        );
    }

    #[test]
    fn every_returned_cluster_is_non_empty() {
        let (vectors, n, dims) = three_well_separated_blobs();
        let mut rng = StdRng::seed_from_u64(2);
        // k=5 with only 3 real blobs: at least two clusters have no
        // "natural" points and must be recovered by empty-cluster handling.
        let result = kmeans(&vectors, n, dims, 5, 100, &mut rng);
        let mut counts = vec![0usize; 5];
        for &a in &result.assignments {
            counts[a] += 1;
        }
        assert!(
            counts.iter().all(|&c| c > 0),
            "every cluster must have at least one member: {counts:?}"
        );
    }

    #[test]
    fn is_deterministic_given_the_same_seed() {
        let (vectors, n, dims) = three_well_separated_blobs();
        let mut rng1 = StdRng::seed_from_u64(42);
        let mut rng2 = StdRng::seed_from_u64(42);
        let r1 = kmeans(&vectors, n, dims, 3, 100, &mut rng1);
        let r2 = kmeans(&vectors, n, dims, 3, 100, &mut rng2);
        assert_eq!(r1, r2);
    }

    #[test]
    fn inertia_never_increases_across_iterations() {
        let (vectors, n, dims) = three_well_separated_blobs();
        let mut previous = f64::INFINITY;
        // Run with an increasing iteration cap and confirm the objective
        // is monotonically non-increasing as more iterations are allowed
        // — the defining correctness property of Lloyd's algorithm.
        for max_iters in 1..10 {
            let mut rng_copy = StdRng::seed_from_u64(7);
            let result = kmeans(&vectors, n, dims, 3, max_iters, &mut rng_copy);
            let current = inertia(&vectors, n, dims, &result);
            assert!(
                current <= previous + 1e-6,
                "inertia increased from {previous} to {current} at max_iters={max_iters}"
            );
            previous = current;
        }
    }

    #[test]
    fn recommended_cluster_count_matches_4_sqrt_n() {
        assert_eq!(recommended_cluster_count(1), 4);
        assert_eq!(recommended_cluster_count(1_000_000), 4000);
        assert_eq!(recommended_cluster_count(0), 1); // floor of 1, never zero clusters
    }

    #[test]
    #[should_panic(expected = "k must be in 1..=n")]
    fn rejects_more_clusters_than_points() {
        let vectors = vec![0.0f32; 8];
        let mut rng = StdRng::seed_from_u64(1);
        kmeans(&vectors, 4, 2, 5, 10, &mut rng);
    }
}
