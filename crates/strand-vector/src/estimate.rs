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

//! The query-side distance estimator: given an already-rotated query
//! vector, an already-rotated cluster centroid, and one database vector's
//! `quantize::QuantizedVector` (compact code plus `f_add`/`f_rescale`/
//! `f_error`), estimates the true distance between the query and that
//! database vector, with a two-sided error bound. This is the piece RFC
//! 0010 Design §4 named as "the query-side distance *estimator* that
//! consumes these factors... real, separate, ungrounded-by-this-module
//! work" — now grounded.
//!
//! Adopted from the reference implementation's own formal derivation and
//! its real `G_add`/`G_error`/`G_k1xSumq` computation
//! (`references/rabitq-library-estimator-source.md`).
//!
//! **A real ambiguity in the reference documentation's own math notation
//! was resolved by reading the actual code, not by picking an
//! interpretation.** `estimator.md`'s derivation writes the query term as
//! `q_r' = P^{-1} q_r` (a *reverse*-rotated query, in the same coordinate
//! frame the stored binary code `x_u` is defined in) — read in isolation,
//! this could be misread as requiring a second, inverse rotation pipeline
//! distinct from `crate::rotate`'s forward transform. `query.hpp`'s real
//! code settles it: `BatchQuery`/`SplitBatchQuery`/`SplitSingleQuery` all
//! take a parameter literally named `rotated_query` and use it directly,
//! with no inverse-rotation step anywhere — the same forward `rotate()`
//! this crate already implements (`crate::rotate::rotate_fht_kac`) is
//! applied uniformly to database vectors, centroids, *and* queries. The
//! math notation's `q_r'` is not a second transform; because the forward
//! rotation is orthogonal, `<x_u, P^{-1}q_r> = <P x_u, q_r>`, so building
//! the lookup table from the *forward*-rotated query and dotting it
//! against the *unrotated* code bits (exactly what `code_query_ip` below
//! does) computes the identical quantity by a cheaper route — one
//! rotation per query, not one inverse rotation per candidate.
//!
//! **Verified beyond transcription**: a standalone, dependency-free C++
//! reimplementation of the full encode-then-estimate pipeline (reusing the
//! already-verified `quantize.rs` formula) was compiled and run against a
//! real dim=64 case for both metrics. Both estimates landed close to the
//! true distance, and — the real, load-bearing check, not just value
//! matching — **the true distance fell inside `[lb, ub]` in both cases**,
//! the actual theoretical guarantee this estimator exists to provide, not
//! merely "produces a similar-looking number."

use crate::quantize::{MetricType, QuantizedVector};

/// `c_1 = -((1 << 1) - 1) / 2`, the same bit-width-1 constant
/// `crate::quantize`'s `cb` specializes — shared between the encode and
/// query sides by construction, not coincidence.
const C_1: f32 = -0.5;

/// Per-query factors, computed once and reused across every database
/// vector a query is compared against within one cluster scan.
#[derive(Debug, Clone, Copy)]
pub struct QueryFactors {
    /// `c_1 * sum(rotated_query)` — independent of any particular
    /// database vector or centroid.
    g_k1x_sumq: f32,
}

impl QueryFactors {
    /// Precomputes the query-side factors that don't depend on which
    /// centroid or database vector is being compared against.
    pub fn new(rotated_query: &[f32]) -> Self {
        let sumq: f32 = rotated_query.iter().sum();
        QueryFactors {
            g_k1x_sumq: sumq * C_1,
        }
    }
}

/// An estimated distance with its two-sided error bound
/// (`docs/docs/rabitq/estimator.md`'s own `[lb_dist, ub_dist]`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DistanceEstimate {
    pub estimate: f32,
    pub lower_bound: f32,
    pub upper_bound: f32,
}

/// The dot product between a compact code's 0/1 bits and the rotated
/// query — `estimator.md`'s pseudocode `ip`, the quantity FastScan's own
/// `accumulate()` LUT computes by table lookup for speed; this is its
/// normative scalar equivalent (invariant 9), computed directly rather
/// than via the batched LUT.
///
/// # Panics
///
/// Panics if `compact_code.len() * 8 < rotated_query.len()`.
fn code_query_ip(compact_code: &[u8], rotated_query: &[f32]) -> f32 {
    assert!(
        compact_code.len() * 8 >= rotated_query.len(),
        "compact_code must have at least one bit per rotated_query component"
    );
    let mut acc = 0f32;
    for (d, &q) in rotated_query.iter().enumerate() {
        let byte = compact_code[d / 8];
        let bit = (byte >> (7 - (d % 8))) & 1;
        if bit == 1 {
            acc += q;
        }
    }
    acc
}

/// Estimates the distance between an already-rotated `query` and one
/// database vector, given that vector's `quantized` code and factors
/// (`crate::quantize::quantize_one_bit`), the already-rotated `centroid`
/// both `query` and the database vector were quantized against, and the
/// precomputed `QueryFactors`.
///
/// For `MetricType::L2`, `estimate` approximates the squared Euclidean
/// distance. For `MetricType::InnerProduct`, it approximates the
/// *negative* inner product (`estimator.md`'s own convention, matching
/// Faiss/hnswlib: minimizing the estimate finds the maximum inner
/// product).
///
/// # Panics
///
/// Panics if `query.len() != centroid.len()`, or if
/// `quantized.compact_code.len() * 8 < query.len()`.
pub fn estimate_distance(
    quantized: &QuantizedVector,
    query: &[f32],
    centroid: &[f32],
    query_factors: &QueryFactors,
    metric: MetricType,
) -> DistanceEstimate {
    assert_eq!(
        query.len(),
        centroid.len(),
        "query and centroid must have equal length"
    );

    let qc_residual_norm_sqr: f32 = query
        .iter()
        .zip(centroid)
        .map(|(&q, &c)| (q - c) * (q - c))
        .sum();
    let qc_residual_norm = qc_residual_norm_sqr.sqrt();

    let (g_add, g_error) = match metric {
        MetricType::L2 => (qc_residual_norm_sqr, qc_residual_norm),
        MetricType::InnerProduct => {
            let dot_qc: f32 = query.iter().zip(centroid).map(|(&q, &c)| q * c).sum();
            (-dot_qc, qc_residual_norm)
        }
    };

    let ip = code_query_ip(&quantized.compact_code, query);
    let estimate = quantized.f_add + g_add + quantized.f_rescale * (ip + query_factors.g_k1x_sumq);
    let bound = quantized.f_error * g_error;

    DistanceEstimate {
        estimate,
        lower_bound: estimate - bound,
        upper_bound: estimate + bound,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quantize::quantize_one_bit;

    fn test_vectors(dim: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let data: Vec<f32> = (0..dim)
            .map(|i| (i as f32 * 0.21).sin() * 3.0 + 0.1 * (i % 5) as f32)
            .collect();
        let centroid: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.19).sin() * 2.5).collect();
        let query: Vec<f32> = (0..dim)
            .map(|i| (i as f32 * 0.21).sin() * 3.0 + 0.05 * (i % 7) as f32 - 0.3)
            .collect();
        (data, centroid, query)
    }

    /// Real reference values from a standalone, compiled-and-run C++
    /// reimplementation of the exact fetched formula (module doc).
    #[test]
    fn matches_the_reference_implementation_l2() {
        let dim = 64;
        let (data, centroid, query) = test_vectors(dim);

        let quantized = quantize_one_bit(&data, &centroid, MetricType::L2);
        assert!(
            (quantized.f_add - 144.231_02).abs() < 1e-2,
            "f_add={}",
            quantized.f_add
        );
        assert!(
            (quantized.f_rescale - (-7.155_358)).abs() < 1e-3,
            "f_rescale={}",
            quantized.f_rescale
        );
        assert!(
            (quantized.f_error - 4.086_786).abs() < 1e-3,
            "f_error={}",
            quantized.f_error
        );

        let qf = QueryFactors::new(&query);
        let est = estimate_distance(&quantized, &query, &centroid, &qf, MetricType::L2);
        assert!(
            (est.estimate - 8.331_818).abs() < 1e-2,
            "estimate={}",
            est.estimate
        );
        assert!(
            (est.lower_bound - (-38.083_485)).abs() < 1e-1,
            "lower_bound={}",
            est.lower_bound
        );
        assert!(
            (est.upper_bound - 54.747_12).abs() < 1e-1,
            "upper_bound={}",
            est.upper_bound
        );

        let true_dist: f32 = data
            .iter()
            .zip(&query)
            .map(|(&d, &q)| (d - q) * (d - q))
            .sum();
        assert!(
            (true_dist - 9.777_499).abs() < 1e-3,
            "true_dist={true_dist}"
        );
        assert!(
            est.lower_bound <= true_dist && true_dist <= est.upper_bound,
            "the true distance MUST fall within [lb, ub]: true={true_dist}, lb={}, ub={}",
            est.lower_bound,
            est.upper_bound
        );
    }

    #[test]
    fn matches_the_reference_implementation_inner_product() {
        let dim = 64;
        let (data, centroid, query) = test_vectors(dim);

        let quantized = quantize_one_bit(&data, &centroid, MetricType::InnerProduct);
        assert!(
            (quantized.f_add - 35.568_21).abs() < 1e-2,
            "f_add={}",
            quantized.f_add
        );
        assert!(
            (quantized.f_rescale - (-3.577_679)).abs() < 1e-3,
            "f_rescale={}",
            quantized.f_rescale
        );
        assert!(
            (quantized.f_error - 2.043_393).abs() < 1e-3,
            "f_error={}",
            quantized.f_error
        );

        let qf = QueryFactors::new(&query);
        let est = estimate_distance(&quantized, &query, &centroid, &qf, MetricType::InnerProduct);
        assert!(
            (est.estimate - (-273.164_7)).abs() < 1e-1,
            "estimate={}",
            est.estimate
        );
        assert!(
            (est.lower_bound - (-296.372_35)).abs() < 1e-1,
            "lower_bound={}",
            est.lower_bound
        );
        assert!(
            (est.upper_bound - (-249.957_05)).abs() < 1e-1,
            "upper_bound={}",
            est.upper_bound
        );

        let true_neg_ip: f32 = -data.iter().zip(&query).map(|(&d, &q)| d * q).sum::<f32>();
        assert!(
            (true_neg_ip - (-273.441_93)).abs() < 1e-1,
            "true_neg_ip={true_neg_ip}"
        );
        assert!(
            est.lower_bound <= true_neg_ip && true_neg_ip <= est.upper_bound,
            "the true negative inner product MUST fall within [lb, ub]: true={true_neg_ip}, lb={}, ub={}",
            est.lower_bound,
            est.upper_bound
        );
    }

    #[test]
    fn code_query_ip_is_the_sum_of_query_components_where_the_code_bit_is_set() {
        // dims=8, code = 0b1010_0101 (MSB-first, matching pack_binary):
        // bits set at dimensions 0, 2, 5, 7.
        let compact_code = vec![0b1010_0101u8];
        let query = [10.0f32, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0];
        let ip = code_query_ip(&compact_code, &query);
        assert_eq!(ip, 10.0 + 30.0 + 60.0 + 80.0);
    }
}
