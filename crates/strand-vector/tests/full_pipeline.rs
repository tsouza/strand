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

//! The first test connecting every piece this crate has grounded so far
//! into one real pipeline: raw, unrotated vectors -> a real quantization
//! descriptor (real random rotation payload) -> rotation application ->
//! RaBitQ quantization -> the cluster posting-list wire format -> read
//! back. Every earlier test exercised one or two adjacent pieces; this is
//! the first proof the full chain composes end to end, matching the
//! precedent `strand-lexical`'s own `field_end_to_end.rs` and this crate's
//! own `segment_assembly.rs` already set for their respective slices.

use rand::SeedableRng;
use rand::rngs::StdRng;
use strand_vector::descriptor::{self, DescriptorReader, DistanceMetric};
use strand_vector::navigation::ClusterDirEntry;
use strand_vector::posting_list::{ClusterInput, PostingListReader, build_posting_lists};
use strand_vector::quantize::{MetricType, quantize_one_bit};
use strand_vector::rotate::rotate_fht_kac;

fn next_f32(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    ((*state % 2001) as f32 - 1000.0) / 100.0
}

#[test]
fn raw_vectors_flow_through_rotation_quantization_and_the_wire_format() {
    let dims = 100u32; // deliberately not a multiple of 64, exercising real padding
    let padded_dims = descriptor::padded_dims_for(dims) as usize;
    assert_eq!(padded_dims, 128);

    // A real quantization descriptor with a real random rotation.
    let mut rng = StdRng::seed_from_u64(20_260_819);
    let descriptor_bytes = descriptor::build_fht_kac(dims, DistanceMetric::L2, &mut rng);
    let reader = DescriptorReader::new(&descriptor_bytes).expect("valid descriptor");
    assert_eq!(reader.dims(), dims);
    assert_eq!(reader.padded_dims() as usize, padded_dims);

    // A real (raw, unrotated) centroid and a cluster of raw vectors near it.
    let mut state = 0xC0FFEE_u64;
    let raw_centroid: Vec<f32> = (0..dims).map(|_| next_f32(&mut state)).collect();
    let rotated_centroid = rotate_fht_kac(&raw_centroid, padded_dims, reader.rotation_payload());
    assert_eq!(rotated_centroid.len(), padded_dims);

    let vector_count = 40;
    let mut all_compact_codes = Vec::new();
    let mut f_add = Vec::with_capacity(vector_count);
    let mut f_rescale = Vec::with_capacity(vector_count);
    let mut f_error = Vec::with_capacity(vector_count);
    let mut row_ids = Vec::with_capacity(vector_count);

    for i in 0..vector_count {
        let raw_vector: Vec<f32> = raw_centroid
            .iter()
            .map(|&c| c + next_f32(&mut state))
            .collect();
        let rotated_vector = rotate_fht_kac(&raw_vector, padded_dims, reader.rotation_payload());

        let q = quantize_one_bit(&rotated_vector, &rotated_centroid, MetricType::L2);
        assert_eq!(q.compact_code.len(), padded_dims / 8);
        all_compact_codes.extend(q.compact_code);
        f_add.push(q.f_add);
        f_rescale.push(q.f_rescale);
        f_error.push(q.f_error);
        row_ids.push(5000 + i as u64);
    }

    let input = ClusterInput {
        compact_codes: &all_compact_codes,
        f_add: &f_add,
        f_rescale: &f_rescale,
        f_error: &f_error,
        row_ids: &row_ids,
    };
    let (blob, dirs) = build_posting_lists(&[input], padded_dims);
    assert_eq!(dirs.len(), 1);
    assert_eq!(
        dirs[0],
        ClusterDirEntry {
            region_offset: 0,
            code_bytes_length: dirs[0].code_bytes_length,
            vector_count: vector_count as u32
        }
    );

    let posting_reader = PostingListReader::new(&blob);
    let region = posting_reader
        .read_cluster(&dirs[0], padded_dims)
        .expect("valid cluster region");

    assert_eq!(region.compact_codes, all_compact_codes);
    assert_eq!(region.row_ids, row_ids);
    for i in 0..vector_count {
        assert_eq!(region.f_add[i].to_bits(), f_add[i].to_bits());
        assert_eq!(region.f_rescale[i].to_bits(), f_rescale[i].to_bits());
        assert_eq!(region.f_error[i].to_bits(), f_error[i].to_bits());
    }

    // Real end-to-end sanity: rotation preserves distance up to f32
    // rounding, so a rotated vector's residual against the rotated
    // centroid should have roughly the same L2 norm as the raw vector's
    // residual against the raw centroid — checked for the first vector,
    // reconstructed from the raw data this test itself generated.
    let mut state2 = 0xC0FFEE_u64;
    let _raw_centroid_check: Vec<f32> = (0..dims).map(|_| next_f32(&mut state2)).collect();
    let raw_vector0: Vec<f32> = raw_centroid
        .iter()
        .map(|&c| c + next_f32(&mut state2))
        .collect();
    let raw_residual_norm: f32 = raw_vector0
        .iter()
        .zip(&raw_centroid)
        .map(|(&v, &c)| (v - c).powi(2))
        .sum();
    let rotated_vector0 = rotate_fht_kac(&raw_vector0, padded_dims, reader.rotation_payload());
    let rotated_residual_norm: f32 = rotated_vector0
        .iter()
        .zip(&rotated_centroid)
        .map(|(&v, &c)| (v - c).powi(2))
        .sum();
    assert!(
        (raw_residual_norm - rotated_residual_norm).abs() < raw_residual_norm.max(1.0) * 0.05,
        "rotation must approximately preserve residual distance: raw={raw_residual_norm}, rotated={rotated_residual_norm}"
    );
}
