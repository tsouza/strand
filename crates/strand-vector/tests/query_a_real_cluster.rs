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

//! The first test that closes the full loop this crate's own modules have
//! been building toward: raw vectors are rotated and quantized into a real
//! posting-list blob (write side, matching `full_pipeline.rs`), then a
//! real query vector is rotated and used to *scan* that blob with
//! `crate::estimate` (read side) — reproducing, in real code, RFC 0010
//! Design §6's own query-resolution steps 1–3 against one cluster: rotate
//! the query once, then estimate distance to every candidate from
//! already-fetched bytes, with no further round trip.

use rand::SeedableRng;
use rand::rngs::StdRng;
use strand_vector::descriptor::{self, DescriptorReader, DistanceMetric};
use strand_vector::estimate::{QueryFactors, estimate_distance};
use strand_vector::navigation::ClusterDirEntry;
use strand_vector::posting_list::{ClusterInput, PostingListReader, build_posting_lists};
use strand_vector::quantize::{MetricType, quantize_one_bit};
use strand_vector::rotate::rotate_fht_kac;

fn next_f32(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    ((*state % 2001) as f32 - 1000.0) / 200.0
}

#[test]
fn scans_a_real_cluster_and_ranks_the_nearest_neighbor_correctly() {
    let dims = 96u32;
    let padded_dims = descriptor::padded_dims_for(dims) as usize;

    let mut rng = StdRng::seed_from_u64(4_242);
    let descriptor_bytes = descriptor::build_fht_kac(dims, DistanceMetric::L2, &mut rng);
    let reader = DescriptorReader::new(&descriptor_bytes).unwrap();
    let flip = reader.rotation_payload();

    let mut state = 0xF00D_u64;
    let raw_centroid: Vec<f32> = (0..dims).map(|_| next_f32(&mut state)).collect();
    let rotated_centroid = rotate_fht_kac(&raw_centroid, padded_dims, flip);

    // Build a real cluster: 50 vectors near the centroid, plus one
    // deliberately much closer to a chosen query than all the rest —
    // this is the vector a correct scan must rank first.
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
    // The query is near the target vector specifically (small perturbation).
    let raw_query: Vec<f32> = raw_vectors[target_index]
        .iter()
        .map(|&v| v + next_f32(&mut state) * 0.02)
        .collect();

    // Write side: quantize every vector, build the posting-list blob.
    let mut compact_codes = Vec::new();
    let mut f_add = Vec::with_capacity(vector_count);
    let mut f_rescale = Vec::with_capacity(vector_count);
    let mut f_error = Vec::with_capacity(vector_count);
    let mut row_ids = Vec::with_capacity(vector_count);
    for (i, raw_vector) in raw_vectors.iter().enumerate() {
        let rotated_vector = rotate_fht_kac(raw_vector, padded_dims, flip);
        let q = quantize_one_bit(&rotated_vector, &rotated_centroid, MetricType::L2);
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
    };
    let (blob, dirs) = build_posting_lists(&[input], padded_dims);

    // Read side: rotate the query ONCE, then scan every candidate in the
    // already-fetched cluster region — no further rotation, no further
    // I/O, matching RFC 0010 Design §6.
    let rotated_query = rotate_fht_kac(&raw_query, padded_dims, flip);
    let query_factors = QueryFactors::new(&rotated_query);

    let posting_reader = PostingListReader::new(&blob);
    let cluster_dir = &dirs[0];
    assert_eq!(
        cluster_dir,
        &ClusterDirEntry {
            region_offset: 0,
            code_bytes_length: cluster_dir.code_bytes_length,
            vector_count: vector_count as u32
        }
    );
    let region = posting_reader
        .read_cluster(cluster_dir, padded_dims)
        .unwrap();

    let cols = padded_dims / 8;
    let mut ranked: Vec<(u64, f32)> = Vec::with_capacity(vector_count);
    for i in 0..vector_count {
        let code = &region.compact_codes[i * cols..(i + 1) * cols];
        let quantized = strand_vector::quantize::QuantizedVector {
            compact_code: code.to_vec(),
            f_add: region.f_add[i],
            f_rescale: region.f_rescale[i],
            f_error: region.f_error[i],
        };
        let est = estimate_distance(
            &quantized,
            &rotated_query,
            &rotated_centroid,
            &query_factors,
            MetricType::L2,
        );
        ranked.push((region.row_ids[i], est.estimate));
    }
    ranked.sort_by(|a, b| a.1.total_cmp(&b.1));

    assert_eq!(
        ranked[0].0, target_index as u64,
        "the deliberately-nearest vector must rank first; full ranking: {ranked:?}"
    );

    // Cross-check against exact (unquantized) L2 distances: the estimator
    // is approximate, but for this deliberately well-separated case it
    // must agree with brute-force on the actual nearest neighbor.
    let mut exact: Vec<(u64, f32)> = raw_vectors
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let d: f32 = v
                .iter()
                .zip(&raw_query)
                .map(|(&a, &b)| (a - b) * (a - b))
                .sum();
            (i as u64, d)
        })
        .collect();
    exact.sort_by(|a, b| a.1.total_cmp(&b.1));
    assert_eq!(
        exact[0].0, target_index as u64,
        "sanity: brute-force must also rank the target first"
    );
}
