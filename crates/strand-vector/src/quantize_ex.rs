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

//! Multi-bit Extended-RaBitQ ("RaBitQ+") encode-side quantization: the
//! `ex_bits` extra magnitude bits per dimension registered by RFC 0011
//! (`rfcs/0011-multibit-extended-rabitq.md`), on top of the 1-bit sign
//! code `quantize::quantize_one_bit` already produces. Transcribes the
//! reference implementation's `ex_bits_code_with_factor` verbatim
//! (`references/rabitq-library-multibit-quantization-source.md`), with one
//! documented, deliberate divergence: an explicit zero-residual guard the
//! reference itself does not have (below).
//!
//! **`best_rescale_factor` is a genuine numerical search, not a
//! closed-form formula** — an event-driven greedy walk (a priority-queue-
//! ordered incremental search) for the scalar rescale factor that
//! maximizes the cosine similarity between the quantized magnitude vector
//! and the true residual. Unlike `quantize_one_bit`'s pure sign function,
//! two independent, conforming implementations may legitimately converge
//! on different, equally valid quantizations for the same input, because
//! floating-point tie-breaking order is not pinned by the reference or by
//! this module (RFC 0011 Design §3). This is sound because the format's
//! error-bound guarantee is self-consistent for whichever valid
//! quantization a writer's search actually produces — the same reasoning
//! that already lets `kmeans.rs`'s clustering go unstandardized.
//!
//! **Zero-residual guard (a real, documented divergence from the
//! reference, found during this RFC's own adversarial review).** The
//! reference's `ex_bits_code` normalizes the residual before quantizing
//! (`abs_res = abs(residual / ||residual||)`); a zero residual (`data ==
//! centroid` exactly — real and expected for any singleton k-means
//! cluster, the identical scenario `quantize.rs`'s own ACPR found Critical
//! for the 1-bit path) makes this a `0.0 / 0.0` division, producing `NaN`
//! in every dimension *before* the reference's own `ip_resi_xucb == 0 →
//! infinity` guard ever runs — that guard cannot catch it, since by the
//! time `ip_resi_xucb` is computed it is already `NaN`, and `NaN == 0.0`
//! is `false`. This module checks the residual's L2 norm for exactly zero
//! before normalizing and, if zero, skips the search entirely, returning
//! the degenerate output `ex_code = [0; dim]`, `f_add_ex = 0.0`,
//! `f_rescale_ex = -0.0` — bit-for-bit the same degenerate values the
//! 1-bit path's own existing guard already produces, and provably correct
//! here too: substituting them into the query-side boosted formula
//! (`estimate::estimate_distance_boosted`) gives `ex_dist = G_add`
//! exactly, which is exactly right — a zero residual means the database
//! vector *is* the centroid, so the true distance is exactly the
//! query-to-centroid distance, with zero estimation error.

//! ## `quantize_ex_fast` — RFC 0011 Non-goals' `faster_quantize_ex`, closed (M2-6)
//!
//! `quantize_ex`, above, calls `best_rescale_factor` once per vector: an
//! event-driven greedy search over a priority queue, `O(dim)` pushes plus
//! up to `O(dim * 2^ex_bits)` more in the worst case, that finds the
//! per-vector-optimal rescale factor `t`. The reference implementation
//! also ships a second, unregistered construction-side path
//! (`rabitq_impl.hpp`'s `faster_quantize_ex`/`get_const_scaling_factors`,
//! `references/rabitq-library-multibit-quantization-source.md` and, since
//! that vendored excerpt predates this closure, re-fetched live from
//! `include/rabitqlib/quantization/rabitq_impl.hpp` on the `RaBitQ-Library`
//! repository's `main` branch, 2026-08-19, for this module): calibrate one
//! shared rescale factor `t_const` once, from 100 random unit-norm
//! Gaussian direction vectors (a stand-in for "what a typical rotated
//! residual looks like," sound precisely because RaBitQ's own random
//! rotation is designed to make real residuals approximately isotropic —
//! `references/rabitq-library-rotator-source.md`), then reuse that single
//! `t_const` for every vector's magnitude code with a plain `O(dim)` pass,
//! no search at all.
//!
//! **This is genuinely writer-side and does not touch the wire format.**
//! RFC 0011 Design §3's byte-determinism carve-out already states a reader
//! MUST NOT assume two conforming writers produce identical ex-code region
//! bytes for the same logical vectors, precisely because different valid
//! `t` choices are already an accepted degree of freedom — `quantize_ex_
//! fast` exercises exactly that freedom, choosing a shared `t_const`
//! instead of a per-vector optimum. The stored `f_add_ex`/`f_rescale_ex`
//! factors are still computed from whichever code was actually chosen (the
//! same downstream formula `quantize_ex` uses, shared via `finish_
//! quantization` below), so the query-side error-bound guarantee holds
//! exactly as documented in RFC 0011 Design §3's "Alternatives considered"
//! — self-consistency, not idealized-quantization comparison, is what the
//! guarantee actually needs. **This is why `quantize_ex_fast`'s output is
//! not, and is not expected to be, bit-for-bit identical to `quantize_ex`'s
//! for the same input** — it is a different valid quantization of the
//! same vector, exactly as RFC 0011's own Non-goals section already stated
//! ("construction-side speedups that change *which* valid `t` a writer's
//! search converges to, not the format"). What the tests below prove
//! instead, bit-for-bit: given the *same* `t`, `quantize_ex_fast`'s
//! magnitude-code arithmetic and `quantize_ex`'s magnitude-code arithmetic
//! produce identical codes and identical `ipnorm_inv` — i.e. the two
//! entry points share one correct downstream implementation and differ
//! only in how `t` is obtained, the same claim RFC 0011's Non-goals makes,
//! made mechanically checkable rather than merely asserted.

use crate::orthogonal::sample_standard_normal;
use crate::quantize::MetricType;
use rand::Rng;

const K_CONST_EPSILON: f32 = 1.9;
const K_EPS: f64 = 1e-5;
const K_N_ENUM: i64 = 10;

/// `rabitq_impl.hpp`'s own `kTightStart`, a `float`-typed table indexed by
/// `ex_bits` (1..=8): the starting fraction of the search range
/// `best_rescale_factor` begins its greedy walk from.
const K_TIGHT_START: [f64; 9] = [0.0, 0.15, 0.20, 0.52, 0.59, 0.71, 0.75, 0.77, 0.81];

/// One database vector's multi-bit ex-code and factors, as produced by
/// [`quantize_ex`] — the RaBitQ+ counterpart to `quantize::QuantizedVector`.
#[derive(Debug, Clone, PartialEq)]
pub struct ExQuantizedVector {
    /// `dim` per-dimension `ex_bits`-wide unsigned integer codes (range
    /// `0..2^ex_bits`), one `u8` per dimension, **unpacked** — packing into
    /// the wire format's bit-contiguous form is `posting_list.rs`'s concern
    /// (`spec/vectors.md` §4.1).
    pub ex_code: Vec<u8>,
    pub f_add_ex: f32,
    pub f_rescale_ex: f32,
}

/// Event-driven greedy search for the rescale factor `t` maximizing cosine
/// similarity between the `ex_bits`-quantized magnitude vector and the true
/// `o_abs` (`rabitq_impl.hpp`'s `best_rescale_factor`, transcribed
/// verbatim; `references/rabitq-library-multibit-quantization-source.md`).
///
/// `o_abs` MUST be non-empty and every element MUST be strictly positive
/// (the caller normalizes and takes the absolute value first, and the
/// zero-residual case is intercepted before this function is ever called —
/// module doc above).
fn best_rescale_factor(o_abs: &[f64], ex_bits: u8) -> f64 {
    use std::cmp::Ordering;
    use std::collections::BinaryHeap;

    let dim = o_abs.len();
    let max_o = o_abs.iter().cloned().fold(f64::MIN, f64::max);
    let ex_bits = ex_bits as u32;
    let t_end = (((1i64 << ex_bits) - 1 + K_N_ENUM) as f64) / max_o;
    let t_start = t_end * K_TIGHT_START[ex_bits as usize];

    let mut cur_o_bar = vec![0i64; dim];
    let mut sqr_denominator = dim as f64 * 0.25;
    let mut numerator = 0.0f64;
    for i in 0..dim {
        let cur = ((t_start * o_abs[i]) + K_EPS) as i64;
        cur_o_bar[i] = cur;
        sqr_denominator += (cur * cur) as f64 + cur as f64;
        numerator += (cur as f64 + 0.5) * o_abs[i];
    }

    // Min-heap on `next_t` (Rust's BinaryHeap is a max-heap, so we invert
    // via `Reverse`-style manual Ord, matching `std::greater<>` in the
    // reference's own `std::priority_queue`).
    #[derive(PartialEq)]
    struct Entry(f64, usize);
    impl Eq for Entry {}
    impl Ord for Entry {
        fn cmp(&self, other: &Self) -> Ordering {
            other.0.partial_cmp(&self.0).unwrap_or(Ordering::Equal)
        }
    }
    impl PartialOrd for Entry {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    let mut next_t: BinaryHeap<Entry> = BinaryHeap::with_capacity(dim);
    for i in 0..dim {
        next_t.push(Entry((cur_o_bar[i] + 1) as f64 / o_abs[i], i));
    }

    let mut max_ip = 0.0f64;
    let mut t = 0.0f64;

    while let Some(Entry(cur_t, update_id)) = next_t.pop() {
        cur_o_bar[update_id] += 1;
        let update_o_bar = cur_o_bar[update_id];
        sqr_denominator += 2.0 * update_o_bar as f64;
        numerator += o_abs[update_id];

        let cur_ip = numerator / sqr_denominator.sqrt();
        if cur_ip > max_ip {
            max_ip = cur_ip;
            t = cur_t;
        }

        if update_o_bar < (1i64 << ex_bits) - 1 {
            let t_next = (update_o_bar + 1) as f64 / o_abs[update_id];
            if t_next < t_end {
                next_t.push(Entry(t_next, update_id));
            }
        }
    }

    t
}

/// Quantizes `o_abs` (already normalized, non-negative) into `ex_bits`-wide
/// codes, returning `ipnorm_inv = 1 / sum((code[i]+0.5)*o_abs[i])`
/// (`rabitq_impl.hpp`'s `quantize_ex`, transcribed verbatim).
fn quantize_ex_magnitude(o_abs: &[f64], ex_bits: u8, code: &mut [u8]) -> f64 {
    let t = best_rescale_factor(o_abs, ex_bits);
    let max_code = (1i64 << ex_bits) - 1;
    let mut ipnorm = 0.0f64;
    for i in 0..o_abs.len() {
        let mut c = ((t * o_abs[i]) + K_EPS) as i64;
        if c >= 1i64 << ex_bits {
            c = max_code;
        }
        code[i] = c as u8;
        ipnorm += (c as f64 + 0.5) * o_abs[i];
    }
    let ipnorm_inv = 1.0 / ipnorm;
    if ipnorm_inv.is_normal() {
        ipnorm_inv
    } else {
        1.0
    }
}

/// Quantizes one already-rotated `data` vector against an already-rotated
/// `centroid` into `ex_bits` extra magnitude bits per dimension, computing
/// the `f_add_ex`/`f_rescale_ex` distance-correction factors
/// (`rabitq_impl.hpp`'s `ex_bits_code_with_factor`, module doc above for
/// the zero-residual divergence).
///
/// # Panics
///
/// Panics if `data.len() != centroid.len()`, if `ex_bits` is not in
/// `1..=7`, or if any input is non-finite.
pub fn quantize_ex(
    data: &[f32],
    centroid: &[f32],
    ex_bits: u8,
    metric: MetricType,
) -> ExQuantizedVector {
    assert_eq!(
        data.len(),
        centroid.len(),
        "data and centroid must have equal length"
    );
    assert!((1..=7).contains(&ex_bits), "ex_bits must be in 1..=7");
    assert!(
        data.iter().chain(centroid).all(|v| v.is_finite()),
        "data and centroid must be finite"
    );

    let dim = data.len();
    let residual: Vec<f64> = data
        .iter()
        .zip(centroid)
        .map(|(&d, &c)| (d - c) as f64)
        .collect();
    let l2_sqr: f64 = residual.iter().map(|v| v * v).sum();

    if l2_sqr == 0.0 {
        // Zero-residual guard (module doc above): skip the NaN-producing
        // normalization/search entirely.
        return ExQuantizedVector {
            ex_code: vec![0u8; dim],
            f_add_ex: 0.0,
            f_rescale_ex: -0.0,
        };
    }

    let l2_norm = l2_sqr.sqrt();
    let abs_res: Vec<f64> = residual.iter().map(|&r| (r / l2_norm).abs()).collect();

    let mut ex_code = vec![0u8; dim];
    let ipnorm_inv = quantize_ex_magnitude(&abs_res, ex_bits, &mut ex_code);

    finish_quantization(
        &residual, centroid, ex_code, ipnorm_inv, ex_bits, l2_sqr, l2_norm, metric,
    )
}

/// Shared downstream computation for both [`quantize_ex`] and
/// [`quantize_ex_fast`]: sign-complements the magnitude code, reconstructs
/// `total_code`/`xu_cb`, and computes `f_add_ex`/`f_rescale_ex`
/// (`rabitq_impl.hpp`'s `ex_bits_code_with_factor`, the part of the
/// algorithm that does not depend on which search strategy produced `code`/
/// `ipnorm_inv` — module doc above). Factoring this out is what makes the
/// module-doc equivalence claim mechanically checkable: both entry points
/// call this exact function, so the only place their outputs can possibly
/// differ is in the `code`/`ipnorm_inv` each passes in.
#[allow(clippy::too_many_arguments)]
fn finish_quantization(
    residual: &[f64],
    centroid: &[f32],
    mut ex_code: Vec<u8>,
    ipnorm_inv: f64,
    ex_bits: u8,
    l2_sqr: f64,
    l2_norm: f64,
    metric: MetricType,
) -> ExQuantizedVector {
    let dim = residual.len();
    let mask = ((1i64 << ex_bits) - 1) as u8;
    for j in 0..dim {
        if residual[j] < 0.0 {
            ex_code[j] = (!ex_code[j]) & mask;
        }
    }

    let total_code: Vec<f64> = (0..dim)
        .map(|i| {
            ex_code[i] as f64
                + (if residual[i] >= 0.0 {
                    1i64 << ex_bits
                } else {
                    0
                }) as f64
        })
        .collect();
    let cb = -((1i64 << ex_bits) as f64 - 0.5);
    let xu_cb: Vec<f64> = total_code.iter().map(|&c| c + cb).collect();

    let mut ip_resi_xucb: f64 = residual.iter().zip(&xu_cb).map(|(&r, &x)| r * x).sum();
    let ip_cent_xucb: f64 = centroid
        .iter()
        .zip(&xu_cb)
        .map(|(&c, &x)| c as f64 * x)
        .sum();
    let xucb_sqr: f64 = xu_cb.iter().map(|&x| x * x).sum();

    if ip_resi_xucb == 0.0 {
        ip_resi_xucb = f64::INFINITY;
    }

    let bracket = ((l2_sqr * xucb_sqr) / (ip_resi_xucb * ip_resi_xucb) - 1.0) / (dim as f64 - 1.0);
    let tmp_error = l2_norm * K_CONST_EPSILON as f64 * bracket.max(0.0).sqrt();

    let (f_add_ex, f_rescale_ex) = match metric {
        MetricType::L2 => (
            l2_sqr + 2.0 * l2_sqr * ip_cent_xucb / ip_resi_xucb,
            ipnorm_inv * -2.0 * l2_norm,
        ),
        MetricType::InnerProduct => {
            let dot_resi_cent: f64 = residual
                .iter()
                .zip(centroid)
                .map(|(&r, &c)| r * c as f64)
                .sum();
            (
                1.0 - dot_resi_cent + l2_sqr * ip_cent_xucb / ip_resi_xucb,
                ipnorm_inv * -l2_norm,
            )
        }
    };
    let _f_error_ex = if metric == MetricType::L2 {
        2.0 * tmp_error
    } else {
        tmp_error
    }; // computed for grounding fidelity; never stored (spec/vectors.md §4.1)

    ExQuantizedVector {
        ex_code,
        f_add_ex: f_add_ex as f32,
        f_rescale_ex: f_rescale_ex as f32,
    }
}

/// The `faster_quantize_ex` construction-time speedup (RFC 0011 Non-goals,
/// closed by M2-6; module doc above): quantizes one already-rotated `data`
/// vector against an already-rotated `centroid` using a **precomputed,
/// shared** rescale factor `t_const` (from [`calibrate_rescale_factor`])
/// instead of running [`best_rescale_factor`]'s per-vector search — an
/// `O(dim)` pass in place of the full search's `O(dim)` to `O(dim *
/// 2^ex_bits)` worst case.
///
/// **Not expected to produce output identical to [`quantize_ex`] for the
/// same input** — a different, but equally valid, `t` in general (module
/// doc above). Given the *same* `t`, though, the two functions' magnitude-
/// code arithmetic is bit-identical (`tests::fast_path_matches_the_full_
/// search_path_given_the_same_t` proves this directly).
///
/// # Panics
///
/// Panics if `data.len() != centroid.len()`, if `ex_bits` is not in
/// `1..=7`, or if any input is non-finite — same preconditions as
/// [`quantize_ex`].
pub fn quantize_ex_fast(
    data: &[f32],
    centroid: &[f32],
    ex_bits: u8,
    metric: MetricType,
    t_const: f64,
) -> ExQuantizedVector {
    assert_eq!(
        data.len(),
        centroid.len(),
        "data and centroid must have equal length"
    );
    assert!((1..=7).contains(&ex_bits), "ex_bits must be in 1..=7");
    assert!(
        data.iter().chain(centroid).all(|v| v.is_finite()),
        "data and centroid must be finite"
    );

    let dim = data.len();
    let residual: Vec<f64> = data
        .iter()
        .zip(centroid)
        .map(|(&d, &c)| (d - c) as f64)
        .collect();
    let l2_sqr: f64 = residual.iter().map(|v| v * v).sum();

    if l2_sqr == 0.0 {
        // Same zero-residual guard as `quantize_ex` (module doc above):
        // this precondition does not depend on which search strategy the
        // caller would otherwise use.
        return ExQuantizedVector {
            ex_code: vec![0u8; dim],
            f_add_ex: 0.0,
            f_rescale_ex: -0.0,
        };
    }

    let l2_norm = l2_sqr.sqrt();
    let abs_res: Vec<f64> = residual.iter().map(|&r| (r / l2_norm).abs()).collect();

    let mut ex_code = vec![0u8; dim];
    let ipnorm_inv = faster_quantize_ex_magnitude(&abs_res, ex_bits, t_const, &mut ex_code);

    finish_quantization(
        &residual, centroid, ex_code, ipnorm_inv, ex_bits, l2_sqr, l2_norm, metric,
    )
}

/// Quantizes `o_abs` into `ex_bits`-wide codes using a precomputed,
/// shared `t_const` in place of [`best_rescale_factor`]'s per-vector
/// search (`rabitq_impl.hpp`'s `faster_quantize_ex`, transcribed verbatim
/// — bit-identical arithmetic to [`quantize_ex_magnitude`], the only
/// difference being where `t` comes from).
fn faster_quantize_ex_magnitude(o_abs: &[f64], ex_bits: u8, t_const: f64, code: &mut [u8]) -> f64 {
    let max_code = (1i64 << ex_bits) - 1;
    let mut ipnorm = 0.0f64;
    for i in 0..o_abs.len() {
        let mut c = ((t_const * o_abs[i]) + K_EPS) as i64;
        if c >= 1i64 << ex_bits {
            c = max_code;
        }
        code[i] = c as u8;
        ipnorm += (c as f64 + 0.5) * o_abs[i];
    }
    let ipnorm_inv = 1.0 / ipnorm;
    if ipnorm_inv.is_normal() {
        ipnorm_inv
    } else {
        1.0
    }
}

/// Calibrates a single, shared rescale factor `t_const` for a given
/// `(dim, ex_bits)` pair, for use with [`quantize_ex_fast`]
/// (`rabitq_impl.hpp`'s `get_const_scaling_factors`, transcribed
/// verbatim except for RNG plumbing — see below).
///
/// Samples 100 random `dim`-dimensional standard-Gaussian vectors, each
/// L2-normalized and absolute-valued into a random point in the positive
/// orthant of the unit sphere (a stand-in for "what a typical rotated
/// residual looks like," module doc above), runs [`best_rescale_factor`]
/// on each, and averages the 100 results.
///
/// **RNG divergence from the reference, deliberate and inconsequential.**
/// The reference hardcodes its own internal RNG seeded with the literal
/// `42`; this function instead takes a caller-supplied `rng`, matching
/// this crate's own established convention (`kmeans::kmeans`,
/// `orthogonal::generate_matrix_rotation`) rather than hiding a seed
/// inside a library function. This cannot be a byte-determinism concern:
/// `t_const` is never itself part of the wire format (RFC 0011 Design §4.1
/// registers only `ex_code`/`f_add_ex`/`f_rescale_ex`), and the ex-code
/// region's own byte-determinism carve-out already makes which valid `t` a
/// writer picks an explicitly unpinned writer choice (module doc above).
///
/// # Panics
///
/// Panics if `dim == 0` or `ex_bits` is not in `1..=7`.
pub fn calibrate_rescale_factor(dim: usize, ex_bits: u8, rng: &mut impl Rng) -> f64 {
    assert!(dim > 0, "dim must be non-zero");
    assert!((1..=7).contains(&ex_bits), "ex_bits must be in 1..=7");

    const K_CONST_NUM: usize = 100;
    let mut sum = 0.0f64;
    for _ in 0..K_CONST_NUM {
        let sample: Vec<f64> = (0..dim).map(|_| sample_standard_normal(rng)).collect();
        let norm: f64 = sample.iter().map(|v| v * v).sum::<f64>().sqrt();
        let abs_unit: Vec<f64> = sample.iter().map(|&v| (v / norm).abs()).collect();
        sum += best_rescale_factor(&abs_unit, ex_bits);
    }
    sum / K_CONST_NUM as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_reference_implementation_dim8_ex_bits2_l2() {
        let data = [1.0f32, -2.0, 3.5, 0.5, -1.5, 2.0, -0.25, 4.0];
        let centroid = [0.5f32, -1.0, 2.0, 1.0, -1.0, 1.5, 0.0, 3.0];
        let q = quantize_ex(&data, &centroid, 2, MetricType::L2);
        assert_eq!(q.ex_code, vec![1, 1, 3, 2, 2, 1, 3, 2]);
        assert!(
            (q.f_add_ex - 21.200_35).abs() < 1e-3,
            "f_add_ex={}",
            q.f_add_ex
        );
        assert!(
            (q.f_rescale_ex - (-0.794_392_5)).abs() < 1e-5,
            "f_rescale_ex={}",
            q.f_rescale_ex
        );
    }

    #[test]
    fn matches_the_reference_implementation_dim8_ex_bits2_ip() {
        let data = [1.0f32, -2.0, 3.5, 0.5, -1.5, 2.0, -0.25, 4.0];
        let centroid = [0.5f32, -1.0, 2.0, 1.0, -1.0, 1.5, 0.0, 3.0];
        let q = quantize_ex(&data, &centroid, 2, MetricType::InnerProduct);
        assert_eq!(q.ex_code, vec![1, 1, 3, 2, 2, 1, 3, 2]);
        assert!(
            (q.f_add_ex - 0.943_925_2).abs() < 1e-5,
            "f_add_ex={}",
            q.f_add_ex
        );
        assert!(
            (q.f_rescale_ex - (-0.397_196_3)).abs() < 1e-5,
            "f_rescale_ex={}",
            q.f_rescale_ex
        );
    }

    #[test]
    fn matches_the_reference_implementation_dim16_ex_bits3_l2() {
        let data = [
            -1.34f32, 7.05, 6.69, -8.75, 5.74, 4.43, 4.29, 8.02, 5.24, -0.59, -3.51, -0.87, 1.69,
            0.37, -4.26, -1.28,
        ];
        let centroid = [
            -2.24f32, 5.68, -5.83, 5.93, 4.56, -6.65, -8.29, 4.97, -4.64, 1.47, 3.67, -0.37, 2.90,
            1.59, -9.18, -5.97,
        ];
        let q = quantize_ex(&data, &centroid, 3, MetricType::L2);
        assert_eq!(
            q.ex_code,
            vec![0, 0, 7, 0, 0, 7, 7, 2, 6, 6, 3, 7, 7, 7, 3, 3]
        );
        assert!(
            (q.f_add_ex - (-82.824_27)).abs() < 1e-1,
            "f_add_ex={}",
            q.f_add_ex
        );
        assert!(
            (q.f_rescale_ex - (-3.309_0)).abs() < 1e-3,
            "f_rescale_ex={}",
            q.f_rescale_ex
        );
    }

    #[test]
    fn zero_residual_is_finite_and_matches_the_documented_degenerate_output() {
        let v = [0.5f32, -1.0, 2.0, 1.0, -1.0, 1.5, 0.0, 3.0];
        let q = quantize_ex(&v, &v, 3, MetricType::L2);
        assert_eq!(q.ex_code, vec![0u8; 8]);
        assert_eq!(q.f_add_ex, 0.0);
        assert_eq!(q.f_rescale_ex.to_bits(), (-0.0f32).to_bits());
        assert!(q.f_add_ex.is_finite());
        assert!(q.f_rescale_ex.is_finite());
    }

    #[test]
    fn ex_code_values_are_always_within_range() {
        let mut state = 0x1234_5678u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            ((state % 2001) as f32 - 1000.0) / 100.0
        };
        for ex_bits in 1u8..=7 {
            let data: Vec<f32> = (0..32).map(|_| next()).collect();
            let centroid: Vec<f32> = (0..32).map(|_| next()).collect();
            let q = quantize_ex(&data, &centroid, ex_bits, MetricType::L2);
            let max = (1u16 << ex_bits) - 1;
            for &c in &q.ex_code {
                assert!(
                    (c as u16) <= max,
                    "ex_bits={ex_bits}: code {c} exceeds max {max}"
                );
            }
            assert!(q.f_add_ex.is_finite());
            assert!(q.f_rescale_ex.is_finite());
        }
    }

    #[test]
    #[should_panic(expected = "data and centroid must be finite")]
    fn rejects_non_finite_input() {
        let data = [f32::NAN; 8];
        let centroid = [0.0f32; 8];
        quantize_ex(&data, &centroid, 2, MetricType::L2);
    }

    // M2-6: `faster_quantize_ex` construction-time speedup.

    /// The load-bearing equivalence proof (module doc above): given the
    /// *exact same* `t`, `quantize_ex_fast`'s magnitude-code arithmetic and
    /// `quantize_ex`'s own internal search-based arithmetic must produce
    /// bit-identical output, because both paths call the same shared
    /// `finish_quantization` and the same per-dimension formula
    /// (`((t * o_abs[i]) + K_EPS) as i64`, clamped). This does not assert
    /// the two *entry points* agree for arbitrary input (module doc: they
    /// legitimately don't, in general) — it isolates and proves the one
    /// claim that must hold for `quantize_ex_fast` to be a faithful
    /// transcription of `faster_quantize_ex` rather than a different
    /// computation wearing the same name.
    #[test]
    fn fast_path_matches_the_full_search_path_given_the_same_t() {
        let cases: &[(&[f32], &[f32], u8, MetricType)] = &[
            (
                &[1.0, -2.0, 3.5, 0.5, -1.5, 2.0, -0.25, 4.0],
                &[0.5, -1.0, 2.0, 1.0, -1.0, 1.5, 0.0, 3.0],
                2,
                MetricType::L2,
            ),
            (
                &[1.0, -2.0, 3.5, 0.5, -1.5, 2.0, -0.25, 4.0],
                &[0.5, -1.0, 2.0, 1.0, -1.0, 1.5, 0.0, 3.0],
                2,
                MetricType::InnerProduct,
            ),
            (
                &[
                    -1.34, 7.05, 6.69, -8.75, 5.74, 4.43, 4.29, 8.02, 5.24, -0.59, -3.51, -0.87,
                    1.69, 0.37, -4.26, -1.28,
                ],
                &[
                    -2.24, 5.68, -5.83, 5.93, 4.56, -6.65, -8.29, 4.97, -4.64, 1.47, 3.67, -0.37,
                    2.90, 1.59, -9.18, -5.97,
                ],
                3,
                MetricType::L2,
            ),
        ];

        for &(data, centroid, ex_bits, metric) in cases {
            let l2_norm = data
                .iter()
                .zip(centroid)
                .map(|(&d, &c)| ((d - c) as f64).powi(2))
                .sum::<f64>()
                .sqrt();
            let abs_res: Vec<f64> = data
                .iter()
                .zip(centroid)
                .map(|(&d, &c)| (((d - c) as f64) / l2_norm).abs())
                .collect();
            // The exact same `t` the full-search path's internal
            // `quantize_ex_magnitude` would have found for this input.
            let t = best_rescale_factor(&abs_res, ex_bits);

            let full = quantize_ex(data, centroid, ex_bits, metric);
            let fast = quantize_ex_fast(data, centroid, ex_bits, metric, t);
            assert_eq!(
                full, fast,
                "ex_bits={ex_bits}, metric={metric:?}: fast path with the exact \
                 search-derived t must match the full-search path bit-for-bit"
            );
        }
    }

    /// The zero-residual guard (module doc above) fires identically on
    /// both entry points, independent of `t_const`.
    #[test]
    fn fast_path_zero_residual_matches_the_full_search_path() {
        let v = [0.5f32, -1.0, 2.0, 1.0, -1.0, 1.5, 0.0, 3.0];
        let full = quantize_ex(&v, &v, 3, MetricType::L2);
        // An arbitrary, deliberately "wrong" t_const: the guard must fire
        // before t_const is ever used, so its value cannot matter.
        let fast = quantize_ex_fast(&v, &v, 3, MetricType::L2, 12345.0);
        assert_eq!(full, fast);
    }

    #[test]
    fn calibrate_rescale_factor_is_deterministic_given_the_same_seed() {
        use rand::SeedableRng;
        use rand::rngs::StdRng;

        let mut rng1 = StdRng::seed_from_u64(42);
        let mut rng2 = StdRng::seed_from_u64(42);
        let t1 = calibrate_rescale_factor(768, 3, &mut rng1);
        let t2 = calibrate_rescale_factor(768, 3, &mut rng2);
        assert_eq!(t1.to_bits(), t2.to_bits());
        assert!(t1.is_finite() && t1 > 0.0, "t_const={t1}");
    }

    #[test]
    fn calibrate_rescale_factor_differs_across_seeds() {
        use rand::SeedableRng;
        use rand::rngs::StdRng;

        let mut rng1 = StdRng::seed_from_u64(1);
        let mut rng2 = StdRng::seed_from_u64(2);
        let t1 = calibrate_rescale_factor(128, 3, &mut rng1);
        let t2 = calibrate_rescale_factor(128, 3, &mut rng2);
        assert_ne!(t1.to_bits(), t2.to_bits());
    }

    #[test]
    fn fast_path_ex_code_values_are_always_within_range() {
        use rand::SeedableRng;
        use rand::rngs::StdRng;

        let mut state = 0x1234_5678u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            ((state % 2001) as f32 - 1000.0) / 100.0
        };
        let mut cal_rng = StdRng::seed_from_u64(99);
        for ex_bits in 1u8..=7 {
            let t_const = calibrate_rescale_factor(32, ex_bits, &mut cal_rng);
            let data: Vec<f32> = (0..32).map(|_| next()).collect();
            let centroid: Vec<f32> = (0..32).map(|_| next()).collect();
            let q = quantize_ex_fast(&data, &centroid, ex_bits, MetricType::L2, t_const);
            let max = (1u16 << ex_bits) - 1;
            for &c in &q.ex_code {
                assert!(
                    (c as u16) <= max,
                    "ex_bits={ex_bits}: code {c} exceeds max {max}"
                );
            }
            assert!(q.f_add_ex.is_finite());
            assert!(q.f_rescale_ex.is_finite());
        }
    }

    /// Quality check, not just "doesn't crash": a calibrated `t_const`
    /// applied across a batch of vectors from one distribution produces
    /// distance-estimate error comparable to the full per-vector search —
    /// the real reason `faster_quantize_ex` is a legitimate speedup and
    /// not just a cheaper-but-broken shortcut. Mirrors the MSE-comparison
    /// pattern `tests/multibit_query.rs` already uses for the 1-bit-vs-
    /// boosted comparison, applied here to full-search-vs-fast-path.
    #[test]
    fn fast_path_reconstruction_error_is_comparable_to_full_search() {
        use rand::SeedableRng;
        use rand::rngs::StdRng;

        let dim = 64;
        let ex_bits = 4;
        let mut rng = StdRng::seed_from_u64(7);

        // A synthetic centroid and a batch of vectors scattered around it —
        // roughly the shape a real rotated cluster + residual has.
        let centroid: Vec<f32> = (0..dim)
            .map(|_| (sample_standard_normal(&mut rng) * 2.0) as f32)
            .collect();
        let vectors: Vec<Vec<f32>> = (0..40)
            .map(|_| {
                (0..dim)
                    .map(|i| centroid[i] + sample_standard_normal(&mut rng) as f32)
                    .collect()
            })
            .collect();

        let t_const = calibrate_rescale_factor(dim, ex_bits, &mut rng);

        let mut full_sq_err = 0.0f64;
        let mut fast_sq_err = 0.0f64;
        for v in &vectors {
            let full = quantize_ex(v, &centroid, ex_bits, MetricType::L2);
            let fast = quantize_ex_fast(v, &centroid, ex_bits, MetricType::L2, t_const);

            // Reconstruct each dimension's implied magnitude from the
            // stored code and compare to the true absolute residual —
            // a direct proxy for how well each path's `t` fits this data,
            // independent of the query-side error-bound formula.
            let residual: Vec<f64> = v
                .iter()
                .zip(&centroid)
                .map(|(&d, &c)| (d - c) as f64)
                .collect();
            let l2_norm = residual.iter().map(|r| r * r).sum::<f64>().sqrt();
            let abs_res: Vec<f64> = residual.iter().map(|&r| (r / l2_norm).abs()).collect();

            for ((&full_code, &fast_code), &true_mag) in
                full.ex_code.iter().zip(&fast.ex_code).zip(&abs_res)
            {
                let full_mag = (full_code & ((1 << ex_bits) - 1)) as f64;
                let fast_mag = (fast_code & ((1 << ex_bits) - 1)) as f64;
                full_sq_err += (full_mag / (1i64 << ex_bits) as f64 - true_mag).powi(2);
                fast_sq_err += (fast_mag / (1i64 << ex_bits) as f64 - true_mag).powi(2);
            }
        }
        let n = (vectors.len() * dim) as f64;
        let full_mse = full_sq_err / n;
        let fast_mse = fast_sq_err / n;
        // Not "equal" (module doc: a different, valid t in general) but
        // the same order of magnitude — the calibrated constant must be a
        // genuinely useful approximation of the per-vector optimum, not
        // an arbitrary value that happens not to crash.
        assert!(
            fast_mse < full_mse * 5.0,
            "fast_mse={fast_mse} is more than 5x full_mse={full_mse}: \
             the calibrated t_const is not a useful approximation"
        );
    }

    /// Proves the actual performance claim, not just plausibility:
    /// `quantize_ex_fast` is meaningfully faster than `quantize_ex` at a
    /// realistic embedding scale. `#[ignore]`d by default, matching
    /// `orthogonal.rs`'s own convention for scale-dependent timing checks
    /// that shouldn't run on every routine `cargo test` — run explicitly
    /// with `cargo test -p strand-vector --release -- --ignored`.
    #[test]
    #[ignore = "timing-based; run explicitly, ideally with --release \
                (see orthogonal.rs's householder_qr_is_valid_at_realistic_embedding_scale)"]
    fn fast_path_is_measurably_faster_at_realistic_scale() {
        use rand::SeedableRng;
        use rand::rngs::StdRng;
        use std::time::Instant;

        let dim = 768;
        let ex_bits = 7; // the widest registered ex_bits: the worst case for
        // the full search's `O(dim * 2^ex_bits)` heap-refill bound.
        let n = 2000;
        let mut rng = StdRng::seed_from_u64(11);

        let centroid: Vec<f32> = (0..dim)
            .map(|_| sample_standard_normal(&mut rng) as f32)
            .collect();
        let vectors: Vec<Vec<f32>> = (0..n)
            .map(|_| {
                (0..dim)
                    .map(|i| centroid[i] + sample_standard_normal(&mut rng) as f32)
                    .collect()
            })
            .collect();
        let t_const = calibrate_rescale_factor(dim, ex_bits, &mut rng);

        let full_start = Instant::now();
        for v in &vectors {
            std::hint::black_box(quantize_ex(v, &centroid, ex_bits, MetricType::L2));
        }
        let full_elapsed = full_start.elapsed();

        let fast_start = Instant::now();
        for v in &vectors {
            std::hint::black_box(quantize_ex_fast(
                v,
                &centroid,
                ex_bits,
                MetricType::L2,
                t_const,
            ));
        }
        let fast_elapsed = fast_start.elapsed();

        eprintln!(
            "quantize_ex (full search): {full_elapsed:?} for {n} vectors \
             ({:.2} us/vector)",
            full_elapsed.as_secs_f64() * 1e6 / n as f64
        );
        eprintln!(
            "quantize_ex_fast (t_const): {fast_elapsed:?} for {n} vectors \
             ({:.2} us/vector)",
            fast_elapsed.as_secs_f64() * 1e6 / n as f64
        );
        eprintln!(
            "speedup: {:.2}x",
            full_elapsed.as_secs_f64() / fast_elapsed.as_secs_f64()
        );

        assert!(
            fast_elapsed < full_elapsed,
            "fast path ({fast_elapsed:?}) was not faster than the full \
             search ({full_elapsed:?}) at dim={dim}, ex_bits={ex_bits}, n={n}"
        );
    }
}
