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

//! SPANN-style closure replication: the construction-side algorithm that
//! decides, for each vector, which clusters *beyond* its primary (nearest)
//! one it should also be replicated into, so a query landing near a
//! cluster boundary can still find it. `docs/roadmap.md` M2-1; RFC 0010
//! Non-goals and Open questions named this as the one stated M2 milestone
//! deliverable that RFC's own approval did not complete, and its Discussion
//! — post-approval amendment registers the metadata slot
//! (`crate::navigation::ReplicationDescriptor`) this module's output feeds.
//!
//! This is not an invented algorithm (`CLAUDE.md` §3): it implements
//! SPANN's own Eq. 2 closure criterion and its RNG-rule redundancy pruning,
//! grounded against the paper's own text and cross-checked against its
//! reference implementation's shipped parameter defaults
//! (`references/spann-closure-assignment-algorithm.md`). Like
//! `crate::kmeans`, this module runs on **raw, unrotated** vectors and
//! centroids — the same construction order `crate::kmeans`'s own module
//! documentation already establishes (cluster first, rotate afterward),
//! since closure assignment only needs relative distances between a vector
//! and centroids, and rotation (an orthogonal transform) preserves those
//! exactly.

/// SPANN's own closure-assignment construction parameters (`references/
/// spann-closure-assignment-algorithm.md`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClosureConfig {
    /// SPANN's ε₁ (Eq. 2): a vector `x` is also assigned to a non-primary
    /// cluster `c_ij` iff `Dist(x, c_ij) ≤ (1 + epsilon) * Dist(x, c_i1)`,
    /// `c_i1` being `x`'s primary (nearest) cluster. `Dist` here is
    /// **squared** Euclidean distance, matching this crate's own
    /// established L2 convention (`crate::kmeans::squared_distance`,
    /// `crate::query::centroid_distance`) — `references/
    /// spann-closure-assignment-algorithm.md`'s own "STRAND-specific
    /// interpretation choice" section explains why the paper leaves this
    /// ambiguous and why this crate resolves it this way. SPANN's own
    /// paper states `10.0` for posting-list expansion.
    pub epsilon: f32,
    /// The per-vector replica cap, **total** posting lists a vector may
    /// land in (primary included) — SPANN's own `ReplicaCount`, confirmed
    /// by both the paper's ablation and its reference implementation's
    /// shipped default (`8`, `references/
    /// spann-closure-assignment-algorithm.md`).
    ///
    /// MUST be `>= 1` ([`closure_replicate`] panics otherwise) — every
    /// vector always keeps its primary assignment regardless of this cap.
    pub max_replicas: u8,
    /// Whether to additionally apply SPANN's RNG-rule redundancy pruning
    /// (`references/spann-closure-assignment-algorithm.md`) on top of the
    /// epsilon ratio test. SPANN's own paper applies it always (`true`);
    /// exposed as a knob because this rule's exact call-site fidelity to
    /// the reference implementation was not independently re-derived from
    /// its source (a named, real grounding gap, same reference file) —
    /// disabling it still yields a real, if less redundancy-pruned, SPANN
    /// closure assignment bounded by the same `epsilon`/`max_replicas`
    /// caps.
    pub apply_rng_rule: bool,
}

impl ClosureConfig {
    /// SPANN's own defaults, both independently confirmed against its
    /// reference implementation's shipped parameter values (`references/
    /// spann-closure-assignment-algorithm.md`): `epsilon = 10.0`
    /// (`ϵ₁` for posting list expansion, §4.2), `max_replicas = 8`
    /// (`ReplicaCount`, `ParameterDefinitionList.h`), RNG rule applied.
    pub fn spann_default() -> Self {
        ClosureConfig {
            epsilon: 10.0,
            max_replicas: 8,
            apply_rng_rule: true,
        }
    }
}

fn squared_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(&x, &y)| (x - y) * (x - y)).sum()
}

/// Computes, for every vector `0..n`, the full list of clusters it is
/// assigned to under SPANN's closure-assignment algorithm (`references/
/// spann-closure-assignment-algorithm.md`, Eq. 2 plus the RNG-rule
/// pruning): index `0` of each returned list is always the vector's
/// primary cluster (`primary_assignments[i]`, e.g. `crate::kmeans`'s own
/// output), unchanged; any further entries are closure replicas, in
/// ascending-distance-from-the-vector order.
///
/// `vectors` is `n * dims` f32 values, row-major (raw, unrotated);
/// `centroids` is `num_clusters * dims` f32 values, row-major (raw,
/// unrotated, e.g. `crate::kmeans::KMeansResult::centroids`).
///
/// # Panics
///
/// Panics if `vectors.len() != n * dims`, if `centroids.len() !=
/// num_clusters * dims`, if `primary_assignments.len() != n`, if any
/// `primary_assignments[i] >= num_clusters`, if `n == 0`, `dims == 0`, or
/// `num_clusters == 0`, or if `config.max_replicas == 0`.
pub fn closure_replicate(
    vectors: &[f32],
    n: usize,
    dims: usize,
    centroids: &[f32],
    num_clusters: usize,
    primary_assignments: &[usize],
    config: &ClosureConfig,
) -> Vec<Vec<usize>> {
    assert_eq!(
        vectors.len(),
        n * dims,
        "vectors must be exactly n*dims f32 values"
    );
    assert_eq!(
        centroids.len(),
        num_clusters * dims,
        "centroids must be exactly num_clusters*dims f32 values"
    );
    assert_eq!(
        primary_assignments.len(),
        n,
        "primary_assignments must have exactly n entries"
    );
    assert!(
        n > 0 && dims > 0 && num_clusters > 0,
        "n, dims, and num_clusters must be non-zero"
    );
    assert!(
        config.max_replicas > 0,
        "max_replicas must be nonzero — every vector always keeps its primary assignment"
    );
    assert!(
        primary_assignments.iter().all(|&c| c < num_clusters),
        "every primary_assignments entry must be a valid cluster index"
    );

    let centroid_at = |c: usize| &centroids[c * dims..(c + 1) * dims];

    (0..n)
        .map(|i| {
            let point = &vectors[i * dims..(i + 1) * dims];
            let primary = primary_assignments[i];
            let d1 = squared_distance(point, centroid_at(primary));

            // Every other cluster, sorted ascending by distance to this
            // vector — SPANN's own c_i2, c_i3, ..., c_ik ordering (Eq. 2).
            let mut others: Vec<(usize, f32)> = (0..num_clusters)
                .filter(|&c| c != primary)
                .map(|c| (c, squared_distance(point, centroid_at(c))))
                .collect();
            others.sort_by(|a, b| a.1.total_cmp(&b.1));

            let mut assigned = vec![primary];
            let mut prev_centroid_idx = primary; // c_i(j-1), advances every step regardless of accept/reject
            for &(c, d) in &others {
                if assigned.len() as u8 >= config.max_replicas {
                    break;
                }
                // Eq. 2's ratio test is monotonic in the distance-sorted
                // order (the right-hand side is fixed per vector), so the
                // first failure ends the search — every farther candidate
                // fails too.
                if d > (1.0 + config.epsilon) * d1 {
                    break;
                }
                let accept = if config.apply_rng_rule {
                    let d_centroids =
                        squared_distance(centroid_at(prev_centroid_idx), centroid_at(c));
                    d <= d_centroids
                } else {
                    true
                };
                if accept {
                    assigned.push(c);
                }
                prev_centroid_idx = c;
            }
            assigned
        })
        .collect()
}

/// Inverts [`closure_replicate`]'s per-vector assignment lists into
/// per-cluster vector-index lists, ascending — the shape
/// `crate::posting_list::ClusterInput` needs. `assignments[c]` is the
/// sorted list of vector indices assigned to cluster `c` (its primary
/// members plus any closure replicas), possibly containing an index that
/// also appears in another cluster's list under replication (this is the
/// whole point: `assignments.iter().flatten().count() >=
/// per_vector_assignments.len()` whenever any replication occurred).
///
/// A caller building posting lists MUST map each vector index in the
/// returned lists to its actual row-id itself (this function operates on
/// vector indices, not row-ids, so it makes no assumption about how a
/// caller's row-ids relate to vector-array position) and MUST re-sort by
/// row-id if row-ids are not already index-monotonic
/// (`crate::posting_list::build_posting_lists`'s own `row_ids` MUST be
/// strictly ascending precondition).
///
/// # Panics
///
/// Panics if any vector index in `per_vector_assignments` is
/// `>= num_clusters`.
pub fn group_by_cluster(
    per_vector_assignments: &[Vec<usize>],
    num_clusters: usize,
) -> Vec<Vec<usize>> {
    let mut per_cluster: Vec<Vec<usize>> = vec![Vec::new(); num_clusters];
    for (vector_idx, clusters) in per_vector_assignments.iter().enumerate() {
        for &c in clusters {
            assert!(c < num_clusters, "cluster index out of range");
            per_cluster[c].push(vector_idx);
        }
    }
    for v in &mut per_cluster {
        v.sort_unstable();
    }
    per_cluster
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_vector_always_keeps_its_primary_assignment() {
        // Four centroids far apart; epsilon=0 and no RNG rule means no
        // replication should ever be accepted, but the primary must
        // always be present regardless.
        let dims = 2;
        let centroids: Vec<f32> = vec![0.0, 0.0, 100.0, 0.0, 0.0, 100.0, 100.0, 100.0];
        let vectors: Vec<f32> = vec![0.1, 0.1, 99.9, 0.1, 0.1, 99.9, 99.9, 99.9];
        let primary = vec![0, 1, 2, 3];
        let config = ClosureConfig {
            epsilon: 0.0,
            max_replicas: 8,
            apply_rng_rule: false,
        };
        let result = closure_replicate(&vectors, 4, dims, &centroids, 4, &primary, &config);
        for (i, assigned) in result.iter().enumerate() {
            assert_eq!(assigned[0], primary[i]);
        }
    }

    #[test]
    fn a_boundary_vector_replicates_into_its_second_nearest_cluster_under_a_loose_epsilon() {
        // Two centroids on a line; a vector exactly at the midpoint has
        // equal distance to both, so even epsilon=0 must admit the second
        // cluster (ratio test: d2 <= (1+0)*d1 when d1==d2).
        let dims = 1;
        let centroids: Vec<f32> = vec![0.0, 10.0];
        let vectors: Vec<f32> = vec![5.0]; // midpoint
        let primary = vec![0];
        let config = ClosureConfig {
            epsilon: 0.0,
            max_replicas: 8,
            apply_rng_rule: false,
        };
        let result = closure_replicate(&vectors, 1, dims, &centroids, 2, &primary, &config);
        assert_eq!(
            result[0],
            vec![0, 1],
            "midpoint vector replicates into both clusters"
        );
    }

    #[test]
    fn a_vector_far_from_its_second_nearest_cluster_does_not_replicate_under_a_tight_epsilon() {
        let dims = 1;
        let centroids: Vec<f32> = vec![0.0, 10.0];
        let vectors: Vec<f32> = vec![1.0]; // close to cluster 0, far from cluster 1
        let primary = vec![0];
        let config = ClosureConfig {
            epsilon: 0.1, // (1.1)*1.0 = 1.1 << 81 (distance to cluster 1)
            max_replicas: 8,
            apply_rng_rule: false,
        };
        let result = closure_replicate(&vectors, 1, dims, &centroids, 2, &primary, &config);
        assert_eq!(result[0], vec![0]);
    }

    #[test]
    fn max_replicas_caps_the_total_assignment_count() {
        // A vector equidistant from four centroids under a very loose
        // epsilon would otherwise replicate into all four; max_replicas=2
        // must cap it at 2 total (primary + 1).
        let dims = 2;
        // Centroids on a circle of radius 10 around the origin.
        let centroids: Vec<f32> = vec![10.0, 0.0, -10.0, 0.0, 0.0, 10.0, 0.0, -10.0];
        let vectors: Vec<f32> = vec![0.0, 0.0]; // equidistant from all four
        let primary = vec![0];
        let config = ClosureConfig {
            epsilon: 10.0,
            max_replicas: 2,
            apply_rng_rule: false,
        };
        let result = closure_replicate(&vectors, 1, dims, &centroids, 4, &primary, &config);
        assert_eq!(
            result[0].len(),
            2,
            "max_replicas=2 must cap total assignments at 2"
        );
        assert_eq!(result[0][0], 0, "primary is always kept");
    }

    #[test]
    #[should_panic(expected = "max_replicas must be nonzero")]
    fn rejects_a_zero_max_replicas() {
        let dims = 1;
        let centroids = vec![0.0f32, 1.0];
        let vectors = vec![0.0f32];
        let primary = vec![0];
        let config = ClosureConfig {
            epsilon: 1.0,
            max_replicas: 0,
            apply_rng_rule: false,
        };
        closure_replicate(&vectors, 1, dims, &centroids, 2, &primary, &config);
    }

    #[test]
    fn group_by_cluster_inverts_the_mapping_and_preserves_replicated_duplicates() {
        // Vector 0 -> clusters [0, 1] (replicated); vector 1 -> [1] only;
        // vector 2 -> [2] only.
        let per_vector = vec![vec![0, 1], vec![1], vec![2]];
        let grouped = group_by_cluster(&per_vector, 3);
        assert_eq!(grouped[0], vec![0]);
        assert_eq!(
            grouped[1],
            vec![0, 1],
            "vector 0 must appear in cluster 1 too"
        );
        assert_eq!(grouped[2], vec![2]);
    }

    #[test]
    fn a_worked_example_hand_checked_end_to_end() {
        // A small, real, hand-checkable case: 5 two-dimensional vectors,
        // 3 centroids on a line at x = 0, 10, 20 (dims=2, y fixed at 0).
        // Vector positions chosen so exactly one vector sits close enough
        // to its boundary to replicate, and the rest do not.
        let dims = 2;
        let centroids: Vec<f32> = vec![
            0.0, 0.0, // cluster 0
            10.0, 0.0, // cluster 1
            20.0, 0.0, // cluster 2
        ];
        // v0=(1,0): d(c0)=1, d(c1)=81 -> primary c0 only.
        // v1=(9,0): d(c0)=81, d(c1)=1 -> primary c1 only.
        // v2=(9.9,0): d(c1)=0.01, d(c0)=98.01 -> primary c1 only (still far from c0).
        // v3=(5,0): d(c0)=25, d(c1)=25 -> exact midpoint, primary c0
        //   (kmeans-style tie-break: first-listed wins in this hand-built
        //   `primary_assignments`), replicates into c1 (ratio exactly 1.0
        //   <= 1+epsilon for any epsilon >= 0).
        // v4=(19,0): d(c2)=1, d(c1)=81 -> primary c2 only.
        let vectors: Vec<f32> = vec![
            1.0, 0.0, //
            9.0, 0.0, //
            9.9, 0.0, //
            5.0, 0.0, //
            19.0, 0.0,
        ];
        let primary = vec![0, 1, 1, 0, 2];
        let config = ClosureConfig {
            epsilon: 0.0, // tightest possible: only exact ties replicate
            max_replicas: 8,
            apply_rng_rule: false,
        };
        let result = closure_replicate(&vectors, 5, dims, &centroids, 3, &primary, &config);

        assert_eq!(result[0], vec![0], "v0: no replication, close to c0");
        assert_eq!(result[1], vec![1], "v1: no replication, close to c1");
        assert_eq!(result[2], vec![1], "v2: no replication, very close to c1");
        assert_eq!(
            result[3],
            vec![0, 1],
            "v3: exact midpoint, replicates into c1"
        );
        assert_eq!(result[4], vec![2], "v4: no replication, close to c2");

        let grouped = group_by_cluster(&result, 3);
        assert_eq!(grouped[0], vec![0, 3], "cluster 0: v0 primary, v3 replica");
        assert_eq!(
            grouped[1],
            vec![1, 2, 3],
            "cluster 1: v1, v2 primary, v3 replica"
        );
        assert_eq!(grouped[2], vec![4], "cluster 2: v4 primary only");

        // Realized replication factor for this tiny example: 6 total
        // assignments (1+1+2 across clusters... wait, count directly)
        // over 5 distinct vectors.
        let total_assignments: usize = grouped.iter().map(|c| c.len()).sum();
        assert_eq!(total_assignments, 6);
        assert_eq!(total_assignments as f64 / 5.0, 1.2);
    }
}
