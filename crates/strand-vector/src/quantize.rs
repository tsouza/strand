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
//! `spec/vectors.md` §4).
//!
//! Adopted verbatim from the reference implementation's own
//! `one_bit_code_with_factor`/`one_bit_compact_code`/`pack_binary`
//! (`references/rabitq-library-one-bit-quantization-source.md`).
//! Cross-checked against a standalone, dependency-free C++
//! reimplementation of the same fetched source, compiled and executed
//! independently of this crate — not merely transcribed and trusted.
//!
//! Rotation is a separate, prior step (`crate::descriptor`'s rotation
//! payload) — this module's `data`/`centroid` inputs are assumed already
//! rotated, exactly as the reference implementation's own construction
//! path applies rotation before calling into this function
//! (RaBitQ-Library's `IVF::construct`: "we first rotate the centroid and
//! vectors in this cluster... then compute the 1-bit codes").
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

/// One quantized vector: its compact 1-bit code (`ceil(dim/8)` bytes,
/// MSB-first per byte — bit `j` of `data[i]`'s residual sign lands at bit
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
    debug_assert!(binary_code.len().is_multiple_of(8));
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
/// `one_bit_code_with_factor` + `one_bit_compact_code` + `pack_binary`.
///
/// # Panics
///
/// Panics if `data.len() != centroid.len()`, if that length is zero or not
/// a multiple of 8, or if `data == centroid` exactly (the residual vector
/// is the zero vector — the reference implementation's own corner-case
/// guard maps this to an infinite `ip_resi_xucb`, which this
/// implementation surfaces as a panic rather than silently propagating a
/// NaN/inf factor).
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

    let residual: Vec<f32> = data.iter().zip(centroid).map(|(&d, &c)| d - c).collect();
    let binary_code: Vec<u8> = residual.iter().map(|&r| u8::from(r > 0.0)).collect();

    // xu_cb[i] = binary_code[i] - 0.5, i.e. the ±0.5-centered reconstruction.
    let xu_cb: Vec<f32> = binary_code.iter().map(|&b| f32::from(b) - 0.5).collect();

    let l2_sqr: f32 = residual.iter().map(|r| r * r).sum();
    let l2_norm = l2_sqr.sqrt();

    let ip_resi_xucb: f32 = residual.iter().zip(&xu_cb).map(|(r, x)| r * x).sum();
    let ip_cent_xucb: f32 = centroid.iter().zip(&xu_cb).map(|(c, x)| c * x).sum();

    assert!(
        ip_resi_xucb != 0.0,
        "data must differ from centroid (residual is the zero vector)"
    );

    let xu_cb_norm_sqr: f32 = xu_cb.iter().map(|x| x * x).sum();
    let tmp_error = l2_norm
        * K_CONST_EPSILON
        * (((l2_sqr * xu_cb_norm_sqr) / (ip_resi_xucb * ip_resi_xucb) - 1.0) / (dim as f32 - 1.0))
            .sqrt();

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
    /// `l2norm_sqr`, `dot_product` — plain loops, not Eigen) against the
    /// same inputs, independent of this Rust transcription.
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
    #[should_panic(expected = "residual is the zero vector")]
    fn rejects_a_data_vector_identical_to_its_centroid() {
        let v = [1.0f32; 8];
        quantize_one_bit(&v, &v, MetricType::L2);
    }
}
