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

//! The real RaBitQ 1-bit quantization math: given an already-rotated data
//! vector and an already-rotated centroid, computes the binary code and
//! the `f_add`/`f_rescale`/`f_error` distance-correction factors this
//! family's posting-list blob stores (`crate::posting_list`,
//! `spec/vectors.md` §4). This module is the de-facto normative scalar
//! reference for those three wire-visible f32 fields (invariant 9) —
//! `spec/vectors.md` §4 does not independently pin the formula, on the
//! grounds that RFC 0010 Design §4 scopes the algorithm itself out of the
//! container layer; this module's own literal computation, including
//! summation order and the corner-case handling below, is what a second
//! implementation must match to produce bit-identical factors.
//!
//! Adopted from the reference implementation's own
//! `one_bit_code_with_factor`/`one_bit_compact_code`/`pack_binary`
//! (`references/rabitq-library-one-bit-quantization-source.md`), with one
//! deliberate, documented divergence (the `.max(0.0)` clamp below) fixing
//! a real bug the reference's own naive transcription has. Cross-checked
//! against a standalone, dependency-free C++ reimplementation of the same
//! fetched formula (plain loops, no Eigen), compiled and executed — this
//! validates the *formula and its corner-case handling*, not RaBitQ-
//! Library's own compiled numeric output: `l2norm_sqr`/`dot_product` are
//! Eigen `.dot()` reductions in the real library (which may use multiple
//! accumulators or FMA) versus this module's strictly sequential f32
//! summation, so the two will differ in the last few ulps even where both
//! are correct. A real conformance test against RaBitQ-Library's actual
//! binary output remains open (Open questions in the commit that added
//! this module).
//!
//! Rotation is a separate, prior step (`crate::descriptor`'s rotation
//! payload) — this module's `data`/`centroid` inputs are assumed already
//! rotated, exactly as the reference implementation's own construction
//! path applies rotation before calling into this function
//! (RaBitQ-Library's `IVF::construct`: "we first rotate the centroid and
//! vectors in this cluster... then compute the 1-bit codes"). This
//! precondition cannot be checked at runtime — a caller that forgets to
//! rotate gets silently degraded recall, not an error.
//!
//! Out of scope here, matching RFC 0010 Design §4's own boundary: the
//! query-side distance *estimator* that consumes these factors (FastScan's
//! `accumulate()` and the higher-level formula that turns an accumulated
//! dot product plus `f_add`/`f_rescale`/`f_error` into a distance
//! estimate) is real, separate, ungrounded-by-this-module work.

/// The reference implementation's own empirical error-bound multiplier
/// (`kConstEpsilon`, `rabitq_impl.hpp`).
const K_CONST_EPSILON: f32 = 1.9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricType {
    L2,
    InnerProduct,
}

/// One quantized vector: its compact 1-bit code (exactly `dim/8` bytes,
/// MSB-first per byte — the sign of `data[i] - centroid[i]` lands at bit
/// `7 - (i % 8)` of byte `i / 8`) and its three distance-correction
/// factors.
#[derive(Debug, Clone, PartialEq)]
pub struct QuantizedVector {
    pub compact_code: Vec<u8>,
    pub f_add: f32,
    pub f_rescale: f32,
    pub f_error: f32,
}

fn pack_binary(binary_code: &[u8]) -> Vec<u8> {
    assert!(
        binary_code.len().is_multiple_of(8),
        "binary_code length must be a multiple of 8"
    );
    binary_code
        .chunks(8)
        .map(|chunk| {
            let mut byte = 0u8;
            for (j, &b) in chunk.iter().enumerate() {
                byte |= b << (7 - j);
            }
            byte
        })
        .collect()
}

/// Quantizes one already-rotated `data` vector against an already-rotated
/// `centroid`, per the reference implementation's own
/// `one_bit_code_with_factor` + `one_bit_compact_code` + `pack_binary`,
/// with the corner-case fix documented in the module doc above.
///
/// A `data` vector identical to its `centroid` (the residual is the zero
/// vector) is a real, expected case — a singleton k-means cluster
/// guarantees it — not an error: this function returns finite, exact
/// degenerate factors for it (`f_add = 0.0`/`1.0`, `f_rescale = -0.0`,
/// `f_error = 0.0`, per metric), matching the reference implementation's
/// own `+inf` substitution for `ip_resi_xucb` traced all the way through.
///
/// # Panics
///
/// Panics if `data.len() != centroid.len()`, if that length is zero or not
/// a multiple of 8, or if any input component is non-finite (`NaN` or
/// `±inf`) — those are malformed input, not a mathematically meaningful
/// vector, and this function's own arithmetic does not define a sane
/// result for them (unlike the zero-residual case above, which does).
pub fn quantize_one_bit(data: &[f32], centroid: &[f32], metric: MetricType) -> QuantizedVector {
    let dim = data.len();
    assert_eq!(
        centroid.len(),
        dim,
        "data and centroid must have equal length"
    );
    assert!(
        dim > 0 && dim.is_multiple_of(8),
        "dim must be a positive multiple of 8"
    );
    assert!(
        data.iter().chain(centroid).all(|v| v.is_finite()),
        "data and centroid components must all be finite"
    );

    let residual: Vec<f32> = data.iter().zip(centroid).map(|(&d, &c)| d - c).collect();
    let binary_code: Vec<u8> = residual.iter().map(|&r| u8::from(r > 0.0)).collect();

    // xu_cb[i] = binary_code[i] - 0.5, i.e. the ±0.5-centered reconstruction
    // — the bit_width=1 specialization of the general cb = -((1<<bits)-1)/2
    // formula (rabitq_impl.hpp): at bits=1, cb = -0.5 exactly.
    let xu_cb: Vec<f32> = binary_code.iter().map(|&b| f32::from(b) - 0.5).collect();

    let l2_sqr: f32 = residual.iter().map(|r| r * r).sum();
    let l2_norm = l2_sqr.sqrt();

    let mut ip_resi_xucb: f32 = residual.iter().zip(&xu_cb).map(|(r, x)| r * x).sum();
    let ip_cent_xucb: f32 = centroid.iter().zip(&xu_cb).map(|(c, x)| c * x).sum();

    // The reference implementation's own corner-case guard: a data point
    // sitting exactly on its centroid (module doc, and the `# Panics`
    // section's own note that this is NOT panic-worthy). Substituting
    // +inf yields the exact limits of f_add/f_rescale (both finite; see
    // the module doc) rather than a 0/0 NaN.
    if ip_resi_xucb == 0.0 {
        ip_resi_xucb = f32::INFINITY;
    }

    let xu_cb_norm_sqr: f32 = xu_cb.iter().map(|x| x * x).sum();
    // Cauchy-Schwarz guarantees this bracket is >= 0 mathematically (with
    // equality exactly when every |residual_i| is equal); f32 rounding can
    // still push it a few ulps negative, which would otherwise poison
    // tmp_error — and therefore f_error, and the whole wire-format
    // record — with NaN. The reference implementation's own naive
    // transcription does not guard this; clamping at zero is a real,
    // deliberate divergence, not a no-op everywhere, but it is a no-op
    // everywhere the unclamped value would have been non-NaN, so it never
    // changes a value the reference would have produced correctly.
    let bracket = (((l2_sqr * xu_cb_norm_sqr) / (ip_resi_xucb * ip_resi_xucb) - 1.0)
        / (dim as f32 - 1.0))
        .max(0.0);
    let tmp_error = l2_norm * K_CONST_EPSILON * bracket.sqrt();

    let (f_add, f_rescale, f_error) = match metric {
        MetricType::L2 => {
            let f_add = l2_sqr + (2.0 * l2_sqr * ip_cent_xucb / ip_resi_xucb);
            let f_rescale = -2.0 * l2_sqr / ip_resi_xucb;
            (f_add, f_rescale, 2.0 * tmp_error)
        }
        MetricType::InnerProduct => {
            let dot_resid_cent: f32 = residual.iter().zip(centroid).map(|(r, c)| r * c).sum();
            let f_add = 1.0 - dot_resid_cent + (l2_sqr * ip_cent_xucb / ip_resi_xucb);
            let f_rescale = -l2_sqr / ip_resi_xucb;
            (f_add, f_rescale, tmp_error)
        }
    };

    QuantizedVector {
        compact_code: pack_binary(&binary_code),
        f_add,
        f_rescale,
        f_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real reference values, obtained by compiling and running a
    /// standalone, dependency-free C++ reimplementation of the exact
    /// fetched source (`one_bit_code_with_factor`, `pack_binary`,
    /// `l2norm_sqr`, `dot_product` — plain loops, not Eigen), with the
    /// module doc's two fixes applied identically, against the same
    /// inputs, independent of this Rust transcription.
    #[test]
    fn matches_the_reference_implementation_case1_l2() {
        let data = [1.0f32, -2.0, 3.5, 0.5, -1.5, 2.0, -0.25, 4.0];
        let centroid = [0.5f32, -1.0, 2.0, 1.0, -1.0, 1.5, 0.0, 3.0];
        let q = quantize_one_bit(&data, &centroid, MetricType::L2);
        assert_eq!(q.compact_code, vec![0xA5]);
        assert!((q.f_add - 20.095_108).abs() < 1e-4, "f_add={}", q.f_add);
        assert!(
            (q.f_rescale - (-3.695_652_3)).abs() < 1e-4,
            "f_rescale={}",
            q.f_rescale
        );
        assert!(
            (q.f_error - 1.768_661_3).abs() < 1e-4,
            "f_error={}",
            q.f_error
        );
    }

    #[test]
    fn matches_the_reference_implementation_case1_inner_product() {
        let data = [1.0f32, -2.0, 3.5, 0.5, -1.5, 2.0, -0.25, 4.0];
        let centroid = [0.5f32, -1.0, 2.0, 1.0, -1.0, 1.5, 0.0, 3.0];
        let q = quantize_one_bit(&data, &centroid, MetricType::InnerProduct);
        assert_eq!(q.compact_code, vec![0xA5]);
        assert!((q.f_add - 0.391_304_5).abs() < 1e-4, "f_add={}", q.f_add);
        assert!(
            (q.f_rescale - (-1.847_826_1)).abs() < 1e-4,
            "f_rescale={}",
            q.f_rescale
        );
        assert!(
            (q.f_error - 0.884_330_6).abs() < 1e-4,
            "f_error={}",
            q.f_error
        );
    }

    #[test]
    fn matches_the_reference_implementation_case2_l2_dim16() {
        let data: Vec<f32> = (0..16).map(|i| (i as f32 - 8.0) * 0.37).collect();
        let centroid: Vec<f32> = (0..16).map(|i| (i as f32 - 5.0) * 0.21).collect();
        let q = quantize_one_bit(&data, &centroid, MetricType::L2);
        assert_eq!(q.compact_code, vec![0x00, 0x0F]);
        assert!((q.f_add - 31.530_863).abs() < 1e-3, "f_add={}", q.f_add);
        assert!(
            (q.f_rescale - (-5.020_838)).abs() < 1e-3,
            "f_rescale={}",
            q.f_rescale
        );
        assert!((q.f_error - 2.850_29).abs() < 1e-3, "f_error={}", q.f_error);
    }

    /// dim=64, distinct arbitrary values, both metrics — real reference
    /// values from the fixed C++ reimplementation.
    #[test]
    fn matches_the_reference_implementation_case_a_dim64() {
        let data: Vec<f32> = (0..64).map(|i: i32| ((i % 13) - 6) as f32 * 0.53).collect();
        let centroid: Vec<f32> = (0..64).map(|i: i32| ((i % 7) - 3) as f32 * 0.31).collect();

        let q = quantize_one_bit(&data, &centroid, MetricType::L2);
        assert_eq!(q.compact_code, hex_bytes("01F807C03E03F07F"));
        assert!((q.f_add - 248.582_6).abs() < 1e-1, "f_add={}", q.f_add);
        assert!(
            (q.f_rescale - (-10.038_671)).abs() < 1e-3,
            "f_rescale={}",
            q.f_rescale
        );
        assert!(
            (q.f_error - 5.351_642).abs() < 1e-3,
            "f_error={}",
            q.f_error
        );

        let q = quantize_one_bit(&data, &centroid, MetricType::InnerProduct);
        assert_eq!(q.compact_code, hex_bytes("01F807C03E03F07F"));
        assert!((q.f_add - 15.079_058).abs() < 1e-3, "f_add={}", q.f_add);
        assert!(
            (q.f_rescale - (-5.019_336)).abs() < 1e-3,
            "f_rescale={}",
            q.f_rescale
        );
        assert!(
            (q.f_error - 2.675_821).abs() < 1e-3,
            "f_error={}",
            q.f_error
        );
    }

    /// dim=64, a large-magnitude, strongly-negative-dot-product case —
    /// exercises `1 - dot_resid_cent` at real scale for `InnerProduct`,
    /// which case1/caseA (small, similar-signed data/centroid) do not.
    #[test]
    fn matches_the_reference_implementation_case_b_dim64_large_negative_dot() {
        let data: Vec<f32> = (0..64).map(|i: i32| -((i + 1) as f32) * 0.7).collect();
        let centroid: Vec<f32> = (0..64).map(|i: i32| (i + 1) as f32 * 0.9).collect();

        let q = quantize_one_bit(&data, &centroid, MetricType::L2);
        assert_eq!(q.compact_code, vec![0u8; 8]);
        assert!((q.f_add - (-28_620.81)).abs() < 1.0, "f_add={}", q.f_add);
        assert!(
            (q.f_rescale - (-275.2)).abs() < 1e-1,
            "f_rescale={}",
            q.f_rescale
        );
        assert!(
            (q.f_error - 130.212_36).abs() < 1e-1,
            "f_error={}",
            q.f_error
        );

        let q = quantize_one_bit(&data, &centroid, MetricType::InnerProduct);
        assert_eq!(q.compact_code, vec![0u8; 8]);
        assert!((q.f_add - 0.984_375).abs() < 1e-3, "f_add={}", q.f_add);
        assert!(
            (q.f_rescale - (-137.6)).abs() < 1e-1,
            "f_rescale={}",
            q.f_rescale
        );
        assert!(
            (q.f_error - 65.106_18).abs() < 1e-1,
            "f_error={}",
            q.f_error
        );
    }

    /// A data vector identical to its centroid MUST NOT panic (a real,
    /// expected case, module doc) and MUST produce the exact finite
    /// degenerate values the reference implementation's own `+inf`
    /// substitution yields when traced through — real reference values
    /// from the fixed C++ reimplementation, dim=64.
    #[test]
    fn zero_residual_is_finite_and_matches_the_reference_implementation() {
        let v: Vec<f32> = (0..64).map(|i: i32| (i - 30) as f32 * 0.17).collect();

        let q = quantize_one_bit(&v, &v, MetricType::L2);
        assert_eq!(q.compact_code, vec![0u8; 8]);
        assert_eq!(q.f_add, 0.0);
        assert_eq!(
            q.f_rescale.to_bits(),
            0x8000_0000,
            "f_rescale must be -0.0, not +0.0 (invariant 11 byte determinism)"
        );
        assert_eq!(q.f_error, 0.0);

        let q = quantize_one_bit(&v, &v, MetricType::InnerProduct);
        assert_eq!(q.f_add, 1.0);
        assert_eq!(q.f_rescale.to_bits(), 0x8000_0000);
        assert_eq!(q.f_error, 0.0);
    }

    /// An equal-magnitude residual (`Cauchy-Schwarz` equality case) pushes
    /// the pre-sqrt bracket a few ulps negative under real f32 rounding —
    /// the bug the module doc's clamp fixes. Confirmed via the fixed C++
    /// reimplementation: finite, not NaN.
    #[test]
    fn equal_magnitude_residual_does_not_produce_nan() {
        let centroid = [0.0f32; 64];
        let data: Vec<f32> = (0..64)
            .map(|i| if i % 2 == 0 { 0.1 } else { -0.1 })
            .collect();
        let q = quantize_one_bit(&data, &centroid, MetricType::L2);
        assert!(q.f_add.is_finite() && q.f_rescale.is_finite() && q.f_error.is_finite());
        assert!(
            (q.f_error - 0.000_295_69).abs() < 1e-6,
            "f_error={}",
            q.f_error
        );
    }

    /// A residual whose magnitude underflows f32 (denormal-adjacent) must
    /// not produce NaN either — confirmed via the fixed C++
    /// reimplementation.
    #[test]
    fn underflowing_residual_does_not_produce_nan() {
        let centroid = [0.0f32; 64];
        let data = [1e-23f32; 64];
        let q = quantize_one_bit(&data, &centroid, MetricType::L2);
        assert!(q.f_add.is_finite() && q.f_rescale.is_finite() && q.f_error.is_finite());
        assert_eq!(q.f_add, 0.0);
        assert_eq!(q.f_error, 0.0);
    }

    #[test]
    fn all_ones_binary_code_is_representable() {
        let centroid = [0.0f32; 64];
        let data: Vec<f32> = (0..64).map(|i| 1.0 + i as f32 * 0.05).collect();
        let q = quantize_one_bit(&data, &centroid, MetricType::L2);
        assert_eq!(q.compact_code, vec![0xFFu8; 8]);
        assert!((q.f_add - 478.960_05).abs() < 1e-1, "f_add={}", q.f_add);
    }

    #[test]
    fn compact_code_bit_order_is_msb_first_per_byte() {
        // residual > 0 (bit=1) at index 0 only, dim=8: expect 0b1000_0000.
        let data = [1.0f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let centroid = [0.0f32; 8];
        let q = quantize_one_bit(&data, &centroid, MetricType::L2);
        assert_eq!(q.compact_code, vec![0b1000_0000]);

        // residual > 0 at index 7 only: expect 0b0000_0001.
        let data = [0.0f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0];
        let q = quantize_one_bit(&data, &centroid, MetricType::L2);
        assert_eq!(q.compact_code, vec![0b0000_0001]);
    }

    #[test]
    #[should_panic(expected = "must all be finite")]
    fn rejects_non_finite_input() {
        let mut data = [1.0f32; 8];
        data[3] = f32::NAN;
        let centroid = [0.0f32; 8];
        quantize_one_bit(&data, &centroid, MetricType::L2);
    }

    fn hex_bytes(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
}
