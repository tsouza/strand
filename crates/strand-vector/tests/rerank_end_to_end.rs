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

//! Closes RFC 0010 Design §6 step 5, the last piece of the vector
//! family's query-resolution pipeline: a real cluster is quantized,
//! scanned (step 3), and reranked (step 5) against a real flat-vector
//! blob (`crate::flat`) built from the *same* raw vectors — proving that
//! whatever order the lossy quantized scan produces, reranking recovers
//! the exact brute-force ground-truth ordering, not merely "a plausible
//! ordering." `query_a_real_cluster.rs` already proved the quantized scan
//! alone gets the easy, well-separated case right; this test's cluster is
//! deliberately closer and more ambiguous, the regime where RaBitQ's 1-bit
//! lossiness has a real chance to misrank the top candidates — exactly
//! the case reranking exists for.

use rand::SeedableRng;
use rand::rngs::StdRng;
use strand_vector::descriptor::{self, DescriptorReader, DistanceMetric};
use strand_vector::estimate::QueryFactors;
use strand_vector::flat::{FlatVectorsReader, build_flat_vectors};
use strand_vector::navigation::{NavigationTierReader, build_navigation_tier};
use strand_vector::posting_list::{ClusterInput, PostingListReader, build_posting_lists};
use strand_vector::quantize::{MetricType, quantize_one_bit};
use strand_vector::query::{
    filter_deleted, rerank, scan_selected_clusters, select_nprobe_clusters,
};
use strand_vector::rotate::rotate_fht_kac;

fn next_f32(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    ((*state % 2001) as f32 - 1000.0) / 200.0
}

#[test]
fn reranking_recovers_exact_brute_force_ground_truth_from_a_real_cluster() {
    let dims = 64u32;
    let padded_dims = descriptor::padded_dims_for(dims) as usize;
    let row_id_base = 3_000u64;

    let mut rng = StdRng::seed_from_u64(31_337);
    let descriptor_bytes = descriptor::build_fht_kac(dims, DistanceMetric::L2, 1, &mut rng);
    let reader = DescriptorReader::new(&descriptor_bytes).unwrap();
    let flip = reader.rotation_payload();

    let mut state = 0xABCD_u64;
    let raw_centroid: Vec<f32> = (0..dims).map(|_| next_f32(&mut state)).collect();
    let rotated_centroid = rotate_fht_kac(&raw_centroid, padded_dims, flip);

    // A tight, ambiguous cluster: every vector is a small, similar
    // perturbation of the centroid — real conditions under which 1-bit
    // RaBitQ's own lossiness can plausibly misorder close candidates.
    let vector_count = 40;
    let mut raw_vectors = Vec::with_capacity(vector_count);
    for _ in 0..vector_count {
        let v: Vec<f32> = raw_centroid
            .iter()
            .map(|&c| c + next_f32(&mut state) * 0.3)
            .collect();
        raw_vectors.push(v);
    }
    let raw_query: Vec<f32> = raw_centroid
        .iter()
        .map(|&c| c + next_f32(&mut state) * 0.3)
        .collect();

    // Write side: real quantization, real posting list, real flat vectors,
    // all keyed by the same real row-ids.
    let mut compact_codes = Vec::new();
    let mut f_add = Vec::with_capacity(vector_count);
    let mut f_rescale = Vec::with_capacity(vector_count);
    let mut f_error = Vec::with_capacity(vector_count);
    let mut row_ids = Vec::with_capacity(vector_count);
    let mut flat = Vec::with_capacity(vector_count * dims as usize);
    for (i, raw_vector) in raw_vectors.iter().enumerate() {
        let rotated_vector = rotate_fht_kac(raw_vector, padded_dims, flip);
        let q = quantize_one_bit(&rotated_vector, &rotated_centroid, MetricType::L2);
        compact_codes.extend(q.compact_code);
        f_add.push(q.f_add);
        f_rescale.push(q.f_rescale);
        f_error.push(q.f_error);
        row_ids.push(row_id_base + i as u64);
        flat.extend_from_slice(raw_vector);
    }
    let input = ClusterInput {
        compact_codes: &compact_codes,
        f_add: &f_add,
        f_rescale: &f_rescale,
        f_error: &f_error,
        row_ids: &row_ids,
        ex_region: None,
    };
    let (posting_bytes, dirs) = build_posting_lists(&[input], padded_dims);
    let nav_bytes = build_navigation_tier(&rotated_centroid, padded_dims, &dirs);
    let navigation = NavigationTierReader::new(&nav_bytes, padded_dims).unwrap();
    let posting_reader = PostingListReader::new(&posting_bytes);
    let flat_bytes = build_flat_vectors(&flat, vector_count, dims as usize);
    let flat_reader = FlatVectorsReader::new(&flat_bytes, dims as usize).unwrap();

    let rotated_query = rotate_fht_kac(&raw_query, padded_dims, flip);
    let query_factors = QueryFactors::new(&rotated_query, 1);
    let selected = select_nprobe_clusters(&navigation, &rotated_query, 1, MetricType::L2);

    // Steps 3-4: the quantized scan, deletion filter (a no-op here).
    let scanned = scan_selected_clusters(
        &navigation,
        &posting_reader,
        &selected,
        &rotated_query,
        &query_factors,
        MetricType::L2,
        padded_dims,
        0,
    )
    .unwrap();
    let survivors = filter_deleted(scanned, row_id_base, None);
    assert_eq!(
        survivors.len(),
        vector_count,
        "no deletions: everyone survives"
    );

    // Step 5: rerank against the real flat-vector blob.
    let reranked = rerank(
        survivors,
        &flat_reader,
        row_id_base,
        &raw_query,
        MetricType::L2,
    );

    // Independent brute-force ground truth, computed directly from the
    // original raw vectors — not derived from anything the quantized
    // pipeline touched.
    let mut brute_force: Vec<(u64, f32)> = raw_vectors
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let d: f32 = v
                .iter()
                .zip(&raw_query)
                .map(|(&a, &b)| (a - b) * (a - b))
                .sum();
            (row_id_base + i as u64, d)
        })
        .collect();
    brute_force.sort_by(|a, b| a.1.total_cmp(&b.1));

    let reranked_order: Vec<u64> = reranked.iter().map(|c| c.row_id).collect();
    let brute_force_order: Vec<u64> = brute_force.iter().map(|(id, _)| *id).collect();
    assert_eq!(
        reranked_order, brute_force_order,
        "reranking must recover the exact brute-force ordering, regardless of \
         what order the lossy quantized scan produced"
    );

    // And the reranked distances themselves must match brute-force
    // exactly, not just the ordering.
    for (reranked_candidate, (_, exact_dist)) in reranked.iter().zip(&brute_force) {
        assert!(
            (reranked_candidate.estimate.estimate - exact_dist).abs() < 1e-4,
            "reranked distance must equal the true exact distance"
        );
        assert_eq!(
            reranked_candidate.estimate.lower_bound, reranked_candidate.estimate.estimate,
            "no estimation uncertainty remains after reranking"
        );
        assert_eq!(
            reranked_candidate.estimate.upper_bound,
            reranked_candidate.estimate.estimate
        );
    }
}
