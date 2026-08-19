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

//! `MatrixRotator`'s matrix *generation*: samples a random Gaussian
//! matrix and orthogonalizes it via Householder QR decomposition,
//! producing the realized rotation payload `crate::descriptor::
//! build_matrix` serializes and `crate::rotate::rotate_matrix` applies.
//! Both of those already existed and were tested against a
//! caller-supplied matrix; this module is what actually produces one.
//!
//! Adopted from the reference implementation's own algorithm
//! (`references/rabitq-library-rotator-source.md`: sample a random
//! Gaussian `padded_dims × padded_dims` matrix, compute its Householder
//! QR decomposition, take `Q`'s transpose, keep only the first `dims`
//! rows — "the random matrix only need the first dim rows, since we just
//! pad zeros for the vector to be rotated to padded dimension"). That
//! reference source already grounds *why* the realized matrix is always
//! serialized rather than a seed: Eigen's own `HouseholderQR` is not
//! guaranteed bit-exact across BLAS/LAPACK implementations, and this
//! module's own Householder QR is a *third*, independent implementation
//! of the same well-known algorithm — it would be a fourth source of
//! divergence even if it tried to match Eigen bit-for-bit, which
//! `descriptor.rs`'s own design already accounts for by never requiring
//! that: any implementation of `MatrixRotator` need only produce *a*
//! valid orthogonal matrix, serialized verbatim once realized, not any
//! *particular* one.
//!
//! **This is unlike every other numerically-precise module in this
//! crate.** `quantize.rs`, `rotate.rs`, and `estimate.rs` all had a
//! specific reference algorithm to match, verified by cross-checking
//! against a compiled reimplementation of that exact algorithm. QR
//! decomposition is not unique — two correct implementations can (and
//! typically do) disagree on the sign of individual columns — so
//! matching a specific reference's output isn't even the right
//! correctness criterion here. What must hold is the property that
//! *defines* a valid QR decomposition, independent of which specific one
//! an implementation produces: `Q` is orthogonal (`Q^T Q = I`), `R` is
//! upper triangular, and `Q * R` reconstructs the original matrix. This
//! module is verified against exactly those three properties, directly,
//! for matrices up to real embedding scale — a stronger and more directly
//! relevant check here than bit-matching any single other implementation
//! would have been.
//!
//! Computed internally in f64 for numerical stability (standard practice
//! for Householder QR — the reference implementation's own `Eigen::
//! HouseholderQR<RowMajorMatrix<T>>` is instantiated at `T = float`, but
//! nothing in the algorithm's correctness depends on matching that
//! choice, and f64 intermediate arithmetic strictly reduces accumulated
//! rounding error for the larger matrix sizes real embeddings need,
//! e.g. 768×768), truncated to f32 only in the final returned payload
//! (matching the wire format's own f32 requirement, `spec/vectors.md`
//! §2.1).

use rand::Rng;

/// Samples one value from the standard normal distribution via the
/// Box-Muller transform (Box & Muller, "A Note on the Generation of
/// Random Normal Deviates," 1958) — the standard, textbook method,
/// avoiding an extra dependency (`rand_distr`) for a single distribution
/// this crate uses in more than one place (`quantize_ex::calibrate_rescale_
/// factor`, RFC 0011 Non-goals' `faster_quantize_ex` construction-time
/// speedup, reuses this exact sampler rather than a second Box-Muller
/// transcription — same reasoning as this module's own single-distribution
/// note, generalized to a second caller).
pub(crate) fn sample_standard_normal(rng: &mut impl Rng) -> f64 {
    // u1 in (0, 1], never exactly 0, to keep ln(u1) finite.
    let u1: f64 = 1.0 - rng.random::<f64>();
    let u2: f64 = rng.random();
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

/// Householder QR decomposition of a square `n × n` row-major matrix `a`,
/// returning `(q, r)`, both `n × n` row-major, such that `a = q * r`, `q`
/// is orthogonal, and `r` is upper triangular (Golub & Van Loan, "Matrix
/// Computations" — the standard reference for this textbook algorithm).
///
/// # Panics
///
/// Panics if `a.len() != n * n` or `n == 0`.
fn householder_qr(a: &[f64], n: usize) -> (Vec<f64>, Vec<f64>) {
    assert_eq!(a.len(), n * n, "a must be exactly n*n values");
    assert!(n > 0, "n must be non-zero");

    let mut r = a.to_vec();
    let mut q = vec![0.0; n * n];
    for i in 0..n {
        q[i * n + i] = 1.0;
    }

    for k in 0..n.saturating_sub(1) {
        let m = n - k;
        let mut x: Vec<f64> = (0..m).map(|i| r[(k + i) * n + k]).collect();
        let x_norm = x.iter().map(|v| v * v).sum::<f64>().sqrt();
        if x_norm == 0.0 {
            continue; // column already zero below the diagonal: H_k = I.
        }

        // Golub & Van Loan's numerically stable sign choice: alpha takes
        // the opposite sign of x[0], so v[0] = x[0] - alpha never cancels.
        let alpha = -x[0].signum() * x_norm;
        x[0] -= alpha;
        let v_norm = x.iter().map(|v| v * v).sum::<f64>().sqrt();
        if v_norm == 0.0 {
            continue; // x was already a multiple of e1: H_k = I.
        }
        let v: Vec<f64> = x.iter().map(|&xi| xi / v_norm).collect();

        // Apply H_k = I - 2vv^T to r[k.., k..] from the left.
        for j in 0..n - k {
            let mut dot = 0.0;
            for i in 0..m {
                dot += v[i] * r[(k + i) * n + (k + j)];
            }
            for i in 0..m {
                r[(k + i) * n + (k + j)] -= 2.0 * v[i] * dot;
            }
        }

        // Accumulate q = q * H_k (apply H_k to q from the right).
        for i in 0..n {
            let mut dot = 0.0;
            for j in 0..m {
                dot += q[i * n + (k + j)] * v[j];
            }
            for j in 0..m {
                q[i * n + (k + j)] -= 2.0 * dot * v[j];
            }
        }
    }

    (q, r)
}

/// Generates a fresh `MatrixRotator` rotation payload: `dims × padded_dims`
/// row-major f32 values, the first `dims` rows of the transpose of a
/// random `padded_dims × padded_dims` orthogonal matrix — exactly the
/// shape `crate::descriptor::build_matrix` and `crate::rotate::
/// rotate_matrix` both already expect.
///
/// # Panics
///
/// Panics if `dims == 0`, `padded_dims == 0`, or `dims > padded_dims`.
pub fn generate_matrix_rotation(dims: usize, padded_dims: usize, rng: &mut impl Rng) -> Vec<f32> {
    assert!(
        dims > 0 && padded_dims > 0,
        "dims and padded_dims must be non-zero"
    );
    assert!(dims <= padded_dims, "dims must be <= padded_dims");

    let gaussian: Vec<f64> = (0..padded_dims * padded_dims)
        .map(|_| sample_standard_normal(rng))
        .collect();
    let (q, _r) = householder_qr(&gaussian, padded_dims);

    // q^T's first `dims` rows == q's first `dims` columns, transposed.
    let mut payload = vec![0f32; dims * padded_dims];
    for row in 0..dims {
        for col in 0..padded_dims {
            // q^T[row][col] = q[col][row]
            payload[row * padded_dims + col] = q[col * padded_dims + row] as f32;
        }
    }
    payload
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn matmul(a: &[f64], b: &[f64], n: usize) -> Vec<f64> {
        let mut out = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                let mut acc = 0.0;
                for k in 0..n {
                    acc += a[i * n + k] * b[k * n + j];
                }
                out[i * n + j] = acc;
            }
        }
        out
    }

    fn transpose(a: &[f64], n: usize) -> Vec<f64> {
        let mut out = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                out[j * n + i] = a[i * n + j];
            }
        }
        out
    }

    fn max_abs_diff(a: &[f64], b: &[f64]) -> f64 {
        a.iter()
            .zip(b)
            .map(|(&x, &y)| (x - y).abs())
            .fold(0.0, f64::max)
    }

    /// The three properties that *define* a valid QR decomposition
    /// (module doc), checked directly for one size.
    fn assert_valid_qr_decomposition(n: usize) {
        let mut rng = StdRng::seed_from_u64(n as u64);
        let a: Vec<f64> = (0..n * n)
            .map(|_| sample_standard_normal(&mut rng))
            .collect();
        let (q, r) = householder_qr(&a, n);

        // Q orthogonal: Q^T Q ≈ I.
        let qt = transpose(&q, n);
        let qtq = matmul(&qt, &q, n);
        let mut identity = vec![0.0; n * n];
        for i in 0..n {
            identity[i * n + i] = 1.0;
        }
        let orth_err = max_abs_diff(&qtq, &identity);
        assert!(
            orth_err < 1e-8,
            "n={n}: Q^T Q deviates from I by {orth_err}"
        );

        // R upper triangular: every entry strictly below the diagonal ~ 0.
        for i in 1..n {
            for j in 0..i {
                assert!(
                    r[i * n + j].abs() < 1e-8,
                    "n={n}: R[{i}][{j}]={} is not ~0",
                    r[i * n + j]
                );
            }
        }

        // Reconstruction: Q * R ≈ A.
        let qr = matmul(&q, &r, n);
        let recon_err = max_abs_diff(&qr, &a);
        assert!(
            recon_err < 1e-6,
            "n={n}: Q*R deviates from A by {recon_err}"
        );
    }

    /// Small-to-moderate sizes, fast enough to run in every default
    /// `cargo test` invocation (debug-mode Householder QR is O(n^3) with a
    /// real constant factor — this stays well under a second even
    /// unoptimized).
    #[test]
    fn householder_qr_produces_a_genuinely_valid_decomposition() {
        for &n in &[1usize, 2, 3, 5, 8, 32, 64] {
            assert_valid_qr_decomposition(n);
        }
    }

    /// The realistic embedding-scale case (768, this project's own
    /// repeatedly-cited reference dimension, `CLAUDE.md` §5 invariant 7's
    /// scaling table) — same check, but `#[ignore]`d by default: an
    /// unoptimized O(n^3) Householder QR at n=768 takes ~40s in debug
    /// mode (~3s in `--release`), which is a real cost this crate's
    /// otherwise-fast test suite shouldn't pay on every routine run. Run
    /// explicitly with `cargo test -p strand-vector --release -- --ignored`
    /// (or without `--release`, just slower) when touching this module.
    #[test]
    #[ignore = "O(n^3) at n=768 is slow in debug mode (~40s); run explicitly, ideally with --release"]
    fn householder_qr_is_valid_at_realistic_embedding_scale() {
        assert_valid_qr_decomposition(768);
    }

    #[test]
    fn generate_matrix_rotation_produces_a_real_orthonormal_row_set() {
        // dims < padded_dims (the padded, non-square case every real
        // descriptor uses): the resulting `dims` rows must still be
        // mutually orthonormal, since they're rows of an orthogonal
        // matrix's transpose.
        let dims = 5;
        let padded_dims = 8;
        let mut rng = StdRng::seed_from_u64(7);
        let payload = generate_matrix_rotation(dims, padded_dims, &mut rng);
        assert_eq!(payload.len(), dims * padded_dims);

        for i in 0..dims {
            let row_i = &payload[i * padded_dims..(i + 1) * padded_dims];
            let norm_sqr: f32 = row_i.iter().map(|v| v * v).sum();
            assert!(
                (norm_sqr - 1.0).abs() < 1e-4,
                "row {i} is not unit norm: {norm_sqr}"
            );
            for j in (i + 1)..dims {
                let row_j = &payload[j * padded_dims..(j + 1) * padded_dims];
                let dot: f32 = row_i.iter().zip(row_j).map(|(&a, &b)| a * b).sum();
                assert!(
                    dot.abs() < 1e-4,
                    "rows {i} and {j} are not orthogonal: dot={dot}"
                );
            }
        }
    }

    #[test]
    fn is_deterministic_given_the_same_seed() {
        let mut rng1 = StdRng::seed_from_u64(42);
        let mut rng2 = StdRng::seed_from_u64(42);
        let p1 = generate_matrix_rotation(16, 16, &mut rng1);
        let p2 = generate_matrix_rotation(16, 16, &mut rng2);
        assert_eq!(p1, p2);
    }

    #[test]
    #[should_panic(expected = "dims must be <= padded_dims")]
    fn rejects_dims_greater_than_padded_dims() {
        let mut rng = StdRng::seed_from_u64(1);
        generate_matrix_rotation(10, 5, &mut rng);
    }

    /// `sample_standard_normal` should have roughly zero mean and unit
    /// variance over a large sample — a real, if loose, statistical
    /// sanity check on the Box-Muller transform itself.
    #[test]
    fn sample_standard_normal_has_roughly_zero_mean_and_unit_variance() {
        let mut rng = StdRng::seed_from_u64(123);
        let n = 200_000;
        let samples: Vec<f64> = (0..n).map(|_| sample_standard_normal(&mut rng)).collect();
        let mean = samples.iter().sum::<f64>() / n as f64;
        let variance = samples.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n as f64;
        assert!(mean.abs() < 0.02, "mean={mean}");
        assert!((variance - 1.0).abs() < 0.05, "variance={variance}");
    }
}
