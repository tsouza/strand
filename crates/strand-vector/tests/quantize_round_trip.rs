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

//! Property-based tests for `crate::quantize` across many random vectors —
//! `CLAUDE.md` §9's "prove" step. Complements the fixed-input reference
//! tests in `quantize.rs` itself (which pin exact values against a
//! compiled-and-run C++ reimplementation) with broad coverage no small set
//! of hand-picked cases can give.

use proptest::prelude::*;
use strand_vector::navigation::ClusterDirEntry;
use strand_vector::posting_list::{ClusterInput, PostingListReader, build_posting_lists};
use strand_vector::quantize::{MetricType, quantize_one_bit};

fn finite_component() -> impl Strategy<Value = f32> {
    prop::num::f32::NORMAL.prop_filter("bounded magnitude", |v| v.abs() < 1e6)
}

proptest! {
    /// `quantize_one_bit` never produces a non-finite factor for finite
    /// input, its code bit for each dimension exactly matches the
    /// residual's sign, `f_error` is never negative, and `f_rescale` is
    /// never positive — the property that would have caught the
    /// pre-clamp bracket-goes-negative bug (`quantize.rs`'s own module
    /// doc) immediately, rather than requiring a hand-picked case.
    #[test]
    fn quantize_one_bit_never_produces_non_finite_or_out_of_range_factors(
        data in prop::collection::vec(finite_component(), 64),
        centroid in prop::collection::vec(finite_component(), 64),
        use_ip in any::<bool>(),
    ) {
        let metric = if use_ip { MetricType::InnerProduct } else { MetricType::L2 };
        // A zero residual is a real, separately-tested case
        // (quantize.rs's own zero_residual_is_finite_and_matches_the_
        // reference_implementation) — skip it here so this property
        // focuses on the ordinary, non-degenerate path.
        prop_assume!(data != centroid);

        let q = quantize_one_bit(&data, &centroid, metric);

        prop_assert!(q.f_add.is_finite(), "f_add={} not finite", q.f_add);
        prop_assert!(q.f_rescale.is_finite(), "f_rescale={} not finite", q.f_rescale);
        prop_assert!(q.f_error.is_finite(), "f_error={} not finite", q.f_error);
        prop_assert!(q.f_error >= 0.0, "f_error={} must be non-negative", q.f_error);
        prop_assert!(q.f_rescale <= 0.0, "f_rescale={} must be non-positive", q.f_rescale);

        for j in 0..64 {
            let expected_bit = data[j] - centroid[j] > 0.0;
            let actual_bit = (q.compact_code[j / 8] >> (7 - j % 8)) & 1 == 1;
            prop_assert_eq!(actual_bit, expected_bit, "bit mismatch at dimension {}", j);
        }
    }

    /// quantize -> build_posting_lists -> read_cluster recovers every
    /// code, factor (bit-exact), and row-id for a random cluster.
    #[test]
    fn quantize_then_posting_list_round_trips_bit_exactly(
        vector_count in 1usize..70,
        centroid_seed in prop::collection::vec(finite_component(), 64),
    ) {
        let padded_dims = 64;
        let mut compact_codes = Vec::new();
        let mut f_add = Vec::with_capacity(vector_count);
        let mut f_rescale = Vec::with_capacity(vector_count);
        let mut f_error = Vec::with_capacity(vector_count);
        let mut row_ids = Vec::with_capacity(vector_count);

        for i in 0..vector_count {
            // Deterministic per-vector offset from the centroid, always
            // non-zero in at least one dimension, so every vector has a
            // real, non-degenerate residual.
            let vector: Vec<f32> = centroid_seed
                .iter()
                .enumerate()
                .map(|(j, &c)| c + if j == i % 64 { 1.0 } else { 0.0 })
                .collect();
            let q = quantize_one_bit(&vector, &centroid_seed, MetricType::L2);
            compact_codes.extend(q.compact_code);
            f_add.push(q.f_add);
            f_rescale.push(q.f_rescale);
            f_error.push(q.f_error);
            row_ids.push(i as u64);
        }

        let input = ClusterInput {
            compact_codes: &compact_codes,
            f_add: &f_add,
            f_rescale: &f_rescale,
            f_error: &f_error,
            row_ids: &row_ids,
            ex_region: None,
        };
        let (blob, dirs) = build_posting_lists(&[input], padded_dims);
        prop_assert_eq!(dirs.len(), 1);
        prop_assert_eq!(dirs[0], ClusterDirEntry {
            region_offset: 0,
            code_bytes_length: dirs[0].code_bytes_length,
            vector_count: vector_count as u32,
        });

        let reader = PostingListReader::new(&blob);
        let region = reader.read_cluster(&dirs[0], padded_dims, 0).unwrap();

        prop_assert_eq!(region.compact_codes, compact_codes);
        prop_assert_eq!(
            region.f_add.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            f_add.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
        prop_assert_eq!(
            region.f_rescale.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            f_rescale.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
        prop_assert_eq!(
            region.f_error.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            f_error.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
        prop_assert_eq!(region.row_ids, row_ids);
    }
}
