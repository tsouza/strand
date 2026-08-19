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

//! Rotation *application*: given a raw, unrotated vector and the rotation
//! payload `crate::descriptor` already knows how to serialize, produces
//! the rotated, `padded_dims`-length vector `crate::quantize::
//! quantize_one_bit` expects as input. This is the piece RFC 0010's own
//! Design §2/§4 and this crate's earlier modules named as real, separate,
//! unwritten work — rotation *payload storage* was implemented first
//! (`descriptor.rs`), rotation *application* is this module.
//!
//! `rotate_fht_kac` (the registered default, `RotatorType::FhtKac`) is
//! adopted from the reference implementation's own `FhtKacRotator::
//! rotate()`, its Fast Walsh-Hadamard Transform stages (a standard,
//! well-known algorithm; later `helper_float_N` hand-vectorize the
//! identical math with AVX intrinsics, which this module does not need to
//! replicate: per invariant 9, the scalar implementation is normative and
//! SIMD is an optimization), and its `flip_sign`/`kacs_walk` primitives
//! (read for their scalar *semantics*, not transcribed as SIMD) —
//! `references/rabitq-library-rotation-application-source.md`.
//!
//! **Verified beyond transcription**: a standalone, dependency-free C++
//! reimplementation of the full pipeline (both branches) was compiled and
//! run against three cases — a general-branch case (`dim=100,
//! padded_dim=128`), the degenerate simple-branch case where `padded_dim`
//! is itself a power of two (`dim=padded_dim=64`), and the realistic
//! embedding case (`dim=padded_dim=768`, the common case this branch
//! actually is for STRAND's own use, since 768 is not a power of two).
//! The `dim=768` case's own output additionally confirms a real,
//! independent mathematical property no transcription-matching alone
//! would catch: a true rotation preserves L2 norm, and the compiled
//! reference's own input/output sums of squares matched to 4 decimal
//! places (`1549.8966` vs `1549.8970`) — this crate's own tests assert
//! the same property, not just value equality against the C++ output.
//!
//! `rotate_matrix` (the registered non-default, `RotatorType::Matrix`) is
//! a plain row-major matrix-vector product — grounded already in
//! `descriptor.rs`'s own doc comment (`references/rabitq-library-rotator-
//! source.md`: `rv = v * this->rand_mat_`).
//!
//! Both functions' `data` input is `dims` (raw, unpadded) elements long —
//! matching the reference implementation's own `rotate()` contract, which
//! zero-extends internally — and both return `padded_dims` elements.

/// The generalized Fast Walsh-Hadamard Transform: the standard in-place,
/// recursive-doubling butterfly network, applied to the first `n`
/// elements of `buf` (`n` MUST be a power of two). Generalizes the
/// reference implementation's own `helper_float_1`/`helper_float_2`
/// (`fht_avx.hpp`) — those are this exact algorithm, specialized to `n=2`
/// and `n=4` respectively; later `helper_float_N` hand-vectorize the
/// identical transform with AVX, which this scalar, normative
/// implementation does not need to reproduce (invariant 9).
fn fht(buf: &mut [f32], n: usize) {
    debug_assert!(n.is_power_of_two());
    let mut len = 1;
    while len < n {
        let mut i = 0;
        while i < n {
            for j in i..i + len {
                let u = buf[j];
                let v = buf[j + len];
                buf[j] = u + v;
                buf[j + len] = u - v;
            }
            i += 2 * len;
        }
        len *= 2;
    }
}

fn vec_rescale(data: &mut [f32], len: usize, val: f32) {
    for v in &mut data[..len] {
        *v *= val;
    }
}

/// Flips the sign of `data[d]` wherever bit `d % 8` of `flip[d / 8]` is
/// set — LSB-first within each byte (bit 0 governs the first dimension of
/// each 8-dimension group), matching `rotator_avx2.cpp`'s `flip_sign_avx2`
/// exactly. This is the *opposite* bit-order convention from
/// `crate::quantize::pack_binary`'s MSB-first packing — a different
/// function from a different part of the reference implementation, no
/// convention carried over between them.
fn flip_sign(flip: &[u8], data: &mut [f32]) {
    for (d, v) in data.iter_mut().enumerate() {
        let byte = flip[d / 8];
        if (byte >> (d % 8)) & 1 == 1 {
            *v = -*v;
        }
    }
}

/// One Kac's-walk mixing stage: splits `data[..len]` into two halves and
/// replaces each pair `(data[i], data[i + len/2])` with `(sum,
/// difference)` — matching `rotator_avx2.cpp`'s `kacs_walk_avx2` exactly.
///
/// # Panics
///
/// Panics if `len` is not even or `data.len() < len`.
fn kacs_walk(data: &mut [f32], len: usize) {
    assert!(len.is_multiple_of(2), "len must be even");
    let half = len / 2;
    for i in 0..half {
        let x = data[i];
        let y = data[i + half];
        data[i] = x + y;
        data[i + half] = x - y;
    }
}

/// Applies the registered default rotation (`RotatorType::FhtKac`,
/// `spec/vectors.md` §2.1) to a raw `dims`-length vector, producing a
/// `padded_dims`-length rotated vector. `flip` MUST be the descriptor's
/// own `rotation_payload` bytes (`4 * padded_dims / 8` bytes, four
/// Rademacher sign sequences — `crate::descriptor::build_fht_kac`).
///
/// # Panics
///
/// Panics if `data.is_empty()`, if `padded_dims < data.len()` or
/// `padded_dims` is not a multiple of 64, or if `flip.len() != 4 *
/// padded_dims / 8`.
pub fn rotate_fht_kac(data: &[f32], padded_dims: usize, flip: &[u8]) -> Vec<f32> {
    let dims = data.len();
    assert!(dims > 0, "data must be non-empty");
    assert!(
        padded_dims >= dims && padded_dims.is_multiple_of(64),
        "padded_dims must be >= dims and a multiple of 64"
    );
    assert_eq!(
        flip.len(),
        4 * padded_dims / 8,
        "flip must be exactly 4*padded_dims/8 bytes"
    );

    let mut rotated = vec![0f32; padded_dims];
    rotated[..dims].copy_from_slice(data);

    let trunc_dim = 1usize << (dims as u32).ilog2();
    let fac = 1.0 / (trunc_dim as f32).sqrt();
    let seq_bytes = padded_dims / 8;

    if trunc_dim == padded_dims {
        for s in 0..4 {
            flip_sign(&flip[s * seq_bytes..(s + 1) * seq_bytes], &mut rotated);
            fht(&mut rotated, trunc_dim);
            vec_rescale(&mut rotated, trunc_dim, fac);
        }
        return rotated;
    }

    let start = padded_dims - trunc_dim;

    flip_sign(&flip[0..seq_bytes], &mut rotated);
    fht(&mut rotated[..trunc_dim], trunc_dim);
    vec_rescale(&mut rotated, trunc_dim, fac);
    kacs_walk(&mut rotated, padded_dims);

    flip_sign(&flip[seq_bytes..2 * seq_bytes], &mut rotated);
    fht(&mut rotated[start..], trunc_dim);
    vec_rescale(&mut rotated[start..], trunc_dim, fac);
    kacs_walk(&mut rotated, padded_dims);

    flip_sign(&flip[2 * seq_bytes..3 * seq_bytes], &mut rotated);
    fht(&mut rotated[..trunc_dim], trunc_dim);
    vec_rescale(&mut rotated, trunc_dim, fac);
    kacs_walk(&mut rotated, padded_dims);

    flip_sign(&flip[3 * seq_bytes..4 * seq_bytes], &mut rotated);
    fht(&mut rotated[start..], trunc_dim);
    vec_rescale(&mut rotated[start..], trunc_dim, fac);
    kacs_walk(&mut rotated, padded_dims);

    vec_rescale(&mut rotated, padded_dims, 0.25);
    rotated
}

/// Applies the registered, non-default `MatrixRotator` rotation
/// (`spec/vectors.md` §2.1) to a raw `dims`-length vector: a plain
/// row-major matrix-vector product, `rotated[j] = sum_i data[i] *
/// matrix[i][j]`. `matrix` MUST be the descriptor's own `rotation_payload`
/// bytes reinterpreted as `dims * padded_dims` little-endian f32 values,
/// row-major (`crate::descriptor::build_matrix`'s own payload).
///
/// # Panics
///
/// Panics if `data.is_empty()`, `padded_dims < data.len()`, or
/// `matrix.len() != data.len() * padded_dims`.
pub fn rotate_matrix(data: &[f32], padded_dims: usize, matrix: &[f32]) -> Vec<f32> {
    let dims = data.len();
    assert!(dims > 0, "data must be non-empty");
    assert!(padded_dims >= dims, "padded_dims must be >= dims");
    assert_eq!(
        matrix.len(),
        dims * padded_dims,
        "matrix must be exactly dims*padded_dims f32 values"
    );

    let mut rotated = vec![0f32; padded_dims];
    for (j, out) in rotated.iter_mut().enumerate() {
        let mut acc = 0f32;
        for (i, &d) in data.iter().enumerate() {
            acc += d * matrix[i * padded_dims + j];
        }
        *out = acc;
    }
    rotated
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Matches each test's own C++ reference generator exactly:
    /// `flip[i] = (i*mult + seed) & 0xFF`.
    fn flip_bytes(len: usize, mult: u32, seed: u32) -> Vec<u8> {
        (0..len)
            .map(|i| ((i as u32 * mult + seed) & 0xFF) as u8)
            .collect()
    }

    /// Real reference values from a standalone, compiled-and-run C++
    /// reimplementation of the exact fetched source (module doc).
    #[test]
    fn matches_the_reference_implementation_general_branch_dim100() {
        let dim = 100;
        let padded_dim = 128;
        let data: Vec<f32> = (0..dim)
            .map(|i| (i % 11) as f32 - 5.0 + 0.3 * i as f32)
            .collect();
        // Matches the C++ reference's own `flip[i] = (i*37+11) & 0xFF`.
        let flip = flip_bytes(4 * padded_dim / 8, 37, 11);

        let rotated = rotate_fht_kac(&data, padded_dim, &flip);
        assert_eq!(rotated.len(), padded_dim);

        let expected_first5 = [8.336_329f32, -8.212_109, -12.540_235, 13.923_831, 32.210_55];
        for (i, &e) in expected_first5.iter().enumerate() {
            assert!(
                (rotated[i] - e).abs() < 1e-2,
                "index {i}: got {}, expected {e}",
                rotated[i]
            );
        }
        let l2sqr: f32 = rotated.iter().map(|v| v * v).sum();
        assert!((l2sqr - 30_863.508).abs() < 1.0, "l2sqr={l2sqr}");
    }

    #[test]
    fn matches_the_reference_implementation_simple_branch_dim64() {
        let dim = 64;
        let padded_dim = 64;
        let data: Vec<f32> = (0..dim)
            .map(|i| (i % 7) as f32 - 3.0 + 0.1 * i as f32)
            .collect();
        // Matches the C++ reference's own `flip[i] = (i*53+5) & 0xFF`.
        let flip = flip_bytes(4 * padded_dim / 8, 53, 5);

        let rotated = rotate_fht_kac(&data, padded_dim, &flip);
        let expected_first5 = [0.303_125f32, 2.150_001, -4.075_0, -6.428_125, 1.487_5];
        for (i, &e) in expected_first5.iter().enumerate() {
            assert!(
                (rotated[i] - e).abs() < 1e-3,
                "index {i}: got {}, expected {e}",
                rotated[i]
            );
        }
        let l2sqr: f32 = rotated.iter().map(|v| v * v).sum();
        assert!((l2sqr - 1_127.039_9).abs() < 1e-1, "l2sqr={l2sqr}");
    }

    /// The realistic embedding case (768 dims — not a power of two, so
    /// this exercises the general branch, the common real-world path).
    /// Confirms both the C++-cross-checked first/last 8 values and the
    /// independent mathematical property (rotation preserves L2 norm).
    #[test]
    fn matches_the_reference_implementation_dim768() {
        let dim = 768;
        let padded_dim = 768;
        let data: Vec<f32> = (0..dim)
            .map(|i| (i as f32 * 0.13).sin() * 2.0 + 0.01 * (i % 17) as f32)
            .collect();
        // Matches the C++ reference's own `flip[i] = (i*91+3) & 0xFF` pattern exactly.
        let flip: Vec<u8> = (0..4 * padded_dim / 8)
            .map(|i| ((i as u32 * 91 + 3) & 0xFF) as u8)
            .collect();

        let rotated = rotate_fht_kac(&data, padded_dim, &flip);

        let expected_first8 = [
            1.137_42_f32,
            1.836_812,
            0.371_798,
            1.187_979,
            1.655_093,
            -2.021_67,
            0.097_482,
            1.928_663,
        ];
        for (i, &e) in expected_first8.iter().enumerate() {
            assert!(
                (rotated[i] - e).abs() < 1e-2,
                "first8[{i}]: got {}, expected {e}",
                rotated[i]
            );
        }
        let expected_last8 = [
            -1.766_746f32,
            0.996_227,
            1.541_398,
            -0.119_192,
            1.072_138,
            -3.963_441,
            1.345_107,
            -2.738_988,
        ];
        for (i, &e) in expected_last8.iter().enumerate() {
            let got = rotated[padded_dim - 8 + i];
            assert!(
                (got - e).abs() < 1e-2,
                "last8[{i}]: got {got}, expected {e}"
            );
        }

        let norm_in: f32 = data.iter().map(|v| v * v).sum();
        let norm_out: f32 = rotated.iter().map(|v| v * v).sum();
        assert!(
            (norm_in - norm_out).abs() < 1.0,
            "rotation must preserve L2 norm: in={norm_in}, out={norm_out}"
        );
    }

    #[test]
    fn rotate_matrix_is_a_plain_row_major_matrix_vector_product() {
        // dims=2, padded_dims=3, matrix = [[1,0,1],[0,1,1]] (row-major).
        let data = [2.0f32, 3.0];
        let matrix = [1.0f32, 0.0, 1.0, 0.0, 1.0, 1.0];
        let rotated = rotate_matrix(&data, 3, &matrix);
        // rotated[j] = data[0]*matrix[0][j] + data[1]*matrix[1][j]
        assert_eq!(rotated, vec![2.0, 3.0, 5.0]);
    }

    #[test]
    fn identity_matrix_leaves_padded_prefix_unchanged_and_zero_fills_the_rest() {
        let data = [1.0f32, 2.0, 3.0];
        // dims=3, padded_dims=4: identity in the first 3 columns, zero column 4.
        #[rustfmt::skip]
        let matrix = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
        ];
        let rotated = rotate_matrix(&data, 4, &matrix);
        assert_eq!(rotated, vec![1.0, 2.0, 3.0, 0.0]);
    }
}
