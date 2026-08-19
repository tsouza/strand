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

//! The multi-bit Extended-RaBitQ counterpart to `query_a_real_cluster.rs`:
//! a real cluster is quantized with both the 1-bit sign code AND a
//! `bit_width > 1` ex-code region (RFC 0011), written into a real
//! posting-list blob, then queried through the crate's actual integration
//! point (`query::scan_selected_clusters`, not a hand-rolled loop) —
//! proving the whole boosted-estimate chain composes end to end, and that
//! it is a real improvement over the 1-bit-only estimate, not merely
//! present.

use rand::SeedableRng;
use rand::rngs::StdRng;
use strand_vector::descriptor::{self, DescriptorReader, DistanceMetric};
use strand_vector::estimate::{QueryFactors, estimate_distance};
use strand_vector::navigation::{NavigationTierReader, build_navigation_tier};
use strand_vector::posting_list::{
    ClusterInput, ExRegionInput, PostingListReader, build_posting_lists,
};
use strand_vector::quantize::{MetricType, QuantizedVector, quantize_one_bit};
use strand_vector::quantize_ex::quantize_ex;
use strand_vector::query::{scan_selected_clusters, select_nprobe_clusters};
use strand_vector::rotate::rotate_fht_kac;

fn next_f32(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    ((*state % 2001) as f32 - 1000.0) / 200.0
}

#[test]
fn boosted_estimate_ranks_correctly_and_beats_the_1bit_only_estimate() {
    let dims = 96u32;
    let padded_dims = descriptor::padded_dims_for(dims) as usize;
    let ex_bits = 3u8; // bit_width = 4
    let bit_width = ex_bits + 1;

    let mut rng = StdRng::seed_from_u64(4_242);
    let descriptor_bytes = descriptor::build_fht_kac(dims, DistanceMetric::L2, bit_width, &mut rng);
    let reader = DescriptorReader::new(&descriptor_bytes).unwrap();
    assert_eq!(reader.bit_width(), bit_width);
    let flip = reader.rotation_payload();

    let mut state = 0xF00D_u64;
    let raw_centroid: Vec<f32> = (0..dims).map(|_| next_f32(&mut state)).collect();
    let rotated_centroid = rotate_fht_kac(&raw_centroid, padded_dims, flip);

    let vector_count = 50;
    let target_index = 17;
    let mut raw_vectors = Vec::with_capacity(vector_count);
    for i in 0..vector_count {
        let scale = if i == target_index { 0.05 } else { 1.0 };
        let v: Vec<f32> = raw_centroid
            .iter()
            .map(|&c| c + next_f32(&mut state) * scale)
            .collect();
        raw_vectors.push(v);
    }
    let raw_query: Vec<f32> = raw_vectors[target_index]
        .iter()
        .map(|&v| v + next_f32(&mut state) * 0.02)
        .collect();

    // Write side: quantize every vector with BOTH the 1-bit code and the
    // ex-code region.
    let mut compact_codes = Vec::new();
    let mut f_add = Vec::with_capacity(vector_count);
    let mut f_rescale = Vec::with_capacity(vector_count);
    let mut f_error = Vec::with_capacity(vector_count);
    let mut row_ids = Vec::with_capacity(vector_count);
    let mut ex_code = Vec::new();
    let mut f_add_ex = Vec::with_capacity(vector_count);
    let mut f_rescale_ex = Vec::with_capacity(vector_count);
    for (i, raw_vector) in raw_vectors.iter().enumerate() {
        let rotated_vector = rotate_fht_kac(raw_vector, padded_dims, flip);
        let q = quantize_one_bit(&rotated_vector, &rotated_centroid, MetricType::L2);
        compact_codes.extend(q.compact_code);
        f_add.push(q.f_add);
        f_rescale.push(q.f_rescale);
        f_error.push(q.f_error);
        row_ids.push(i as u64);

        let ex = quantize_ex(&rotated_vector, &rotated_centroid, ex_bits, MetricType::L2);
        ex_code.extend(ex.ex_code);
        f_add_ex.push(ex.f_add_ex);
        f_rescale_ex.push(ex.f_rescale_ex);
    }
    let input = ClusterInput {
        compact_codes: &compact_codes,
        f_add: &f_add,
        f_rescale: &f_rescale,
        f_error: &f_error,
        row_ids: &row_ids,
        ex_region: Some(ExRegionInput {
            ex_bits,
            ex_code: &ex_code,
            f_add_ex: &f_add_ex,
            f_rescale_ex: &f_rescale_ex,
        }),
    };
    let (posting_bytes, dirs) = build_posting_lists(&[input], padded_dims);

    let centroids: Vec<f32> = rotated_centroid.clone();
    let nav_bytes = build_navigation_tier(&centroids, padded_dims, &dirs);
    let navigation = NavigationTierReader::new(&nav_bytes, padded_dims).unwrap();
    let posting_reader = PostingListReader::new(&posting_bytes);

    let rotated_query = rotate_fht_kac(&raw_query, padded_dims, flip);
    let query_factors = QueryFactors::new(&rotated_query, bit_width);

    let selected = select_nprobe_clusters(&navigation, &rotated_query, 1, MetricType::L2);
    let boosted = scan_selected_clusters(
        &navigation,
        &posting_reader,
        &selected,
        &rotated_query,
        &query_factors,
        MetricType::L2,
        padded_dims,
        ex_bits,
    )
    .unwrap();

    assert_eq!(
        boosted[0].row_id, target_index as u64,
        "the deliberately-nearest vector must rank first under the boosted estimate"
    );

    // The real property this feature exists for: the boosted estimate's
    // squared error against the true distance must, in aggregate, be
    // smaller than the 1-bit-only estimate's — not just "present and
    // plausible," a real, measured improvement.
    let exact_distance = |v: &[f32]| -> f32 {
        v.iter()
            .zip(&raw_query)
            .map(|(&a, &b)| (a - b) * (a - b))
            .sum()
    };
    let qf_1bit = QueryFactors::new(&rotated_query, 1);
    let mut boosted_sq_err = 0f64;
    let mut onebit_sq_err = 0f64;
    let cols = padded_dims / 8;
    for i in 0..vector_count {
        let true_dist = exact_distance(&raw_vectors[i]) as f64;
        let code = &compact_codes[i * cols..(i + 1) * cols];
        let quantized = QuantizedVector {
            compact_code: code.to_vec(),
            f_add: f_add[i],
            f_rescale: f_rescale[i],
            f_error: f_error[i],
        };
        let onebit_est = estimate_distance(
            &quantized,
            &rotated_query,
            &rotated_centroid,
            &qf_1bit,
            MetricType::L2,
        );
        let boosted_est = boosted
            .iter()
            .find(|c| c.row_id == i as u64)
            .expect("every row_id must appear in the boosted candidate set")
            .estimate;
        boosted_sq_err += (boosted_est.estimate as f64 - true_dist).powi(2);
        onebit_sq_err += (onebit_est.estimate as f64 - true_dist).powi(2);
    }
    assert!(
        boosted_sq_err < onebit_sq_err,
        "boosted (ex_bits={ex_bits}) mean-squared error ({boosted_sq_err}) must beat \
         1-bit-only mean-squared error ({onebit_sq_err}) — otherwise the extra bytes bought nothing"
    );
}

#[test]
fn read_cluster_rejects_a_wrong_ex_bits_as_a_length_mismatch() {
    let padded_dims = 64;
    let row_ids = vec![1u64, 2, 3];
    let n = row_ids.len();
    let compact_codes: Vec<u8> = vec![0u8; n * padded_dims / 8];
    let f_add = vec![0.0f32; n];
    let f_rescale = vec![-1.0f32; n];
    let f_error = vec![0.0f32; n];
    let ex_code = vec![0u8; n * padded_dims];
    let f_add_ex = vec![0.0f32; n];
    let f_rescale_ex = vec![0.0f32; n];
    let input = ClusterInput {
        compact_codes: &compact_codes,
        f_add: &f_add,
        f_rescale: &f_rescale,
        f_error: &f_error,
        row_ids: &row_ids,
        ex_region: Some(ExRegionInput {
            ex_bits: 3,
            ex_code: &ex_code,
            f_add_ex: &f_add_ex,
            f_rescale_ex: &f_rescale_ex,
        }),
    };
    let (blob, dirs) = build_posting_lists(&[input], padded_dims);
    let reader = PostingListReader::new(&blob);

    // Reading back with the wrong ex_bits (2, not 3) must be rejected as a
    // length mismatch, not silently misparsed.
    let err = reader.read_cluster(&dirs[0], padded_dims, 2).unwrap_err();
    assert!(matches!(
        err,
        strand_vector::posting_list::PostingListError::CodeBytesLengthMismatch { .. }
    ));

    // The correct ex_bits reads back cleanly.
    let region = reader.read_cluster(&dirs[0], padded_dims, 3).unwrap();
    assert_eq!(region.ex_code.len(), n * padded_dims);
}
