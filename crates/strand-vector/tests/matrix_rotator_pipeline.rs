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

//! The `MatrixRotator` counterpart to `full_pipeline.rs`: a freshly
//! *generated* rotation matrix (`crate::orthogonal`), not a caller-
//! supplied one, carried all the way through descriptor serialization,
//! rotation application, quantization, and the posting-list wire format.
//! `full_pipeline.rs` already proved this chain for the default
//! `FhtKacRotator`; this is the first test proving the registered,
//! non-default `MatrixRotator` path composes too, starting from nothing
//! but raw vectors and a random seed.

use rand::SeedableRng;
use rand::rngs::StdRng;
use strand_vector::descriptor::{self, DescriptorReader, DistanceMetric};
use strand_vector::navigation::ClusterDirEntry;
use strand_vector::posting_list::{ClusterInput, PostingListReader, build_posting_lists};
use strand_vector::quantize::{MetricType, quantize_one_bit};
use strand_vector::rotate::rotate_matrix;

fn next_f32(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    ((*state % 2001) as f32 - 1000.0) / 100.0
}

fn parse_f32_le(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

#[test]
fn generates_serializes_and_applies_a_real_matrix_rotation_end_to_end() {
    let dims = 20u32;
    let padded_dims = descriptor::padded_dims_for(dims) as usize;

    // 1. Generate a real MatrixRotator descriptor (fresh QR-orthogonalized
    // matrix, not a caller-supplied one).
    let mut rng = StdRng::seed_from_u64(20_260_819);
    let descriptor_bytes =
        descriptor::build_matrix_generated(dims, DistanceMetric::L2, 1, &mut rng);
    let reader = DescriptorReader::new(&descriptor_bytes).expect("valid descriptor");
    assert_eq!(reader.dims(), dims);
    assert_eq!(reader.padded_dims() as usize, padded_dims);
    let matrix = parse_f32_le(reader.rotation_payload());
    assert_eq!(matrix.len(), dims as usize * padded_dims);

    // 2. Real raw vectors and a centroid.
    let mut state = 0xA55A_u64;
    let raw_centroid: Vec<f32> = (0..dims).map(|_| next_f32(&mut state)).collect();
    let rotated_centroid = rotate_matrix(&raw_centroid, padded_dims, &matrix);
    assert_eq!(rotated_centroid.len(), padded_dims);

    // A genuine orthogonal transform preserves L2 norm — the same
    // property `rotate.rs`'s own FhtKacRotator tests check, now for
    // MatrixRotator's freshly generated matrix specifically.
    let identity_probe = vec![1.0f32; dims as usize];
    let rotated_probe = rotate_matrix(&identity_probe, padded_dims, &matrix);
    let norm_in: f32 = identity_probe.iter().map(|v| v * v).sum();
    let norm_out: f32 = rotated_probe.iter().map(|v| v * v).sum();
    assert!(
        (norm_in - norm_out).abs() < 1e-2,
        "rotation must preserve L2 norm: in={norm_in}, out={norm_out}"
    );

    // 3. Quantize a real cluster of vectors and build a real posting-list blob.
    let vector_count = 30;
    let mut compact_codes = Vec::new();
    let mut f_add = Vec::with_capacity(vector_count);
    let mut f_rescale = Vec::with_capacity(vector_count);
    let mut f_error = Vec::with_capacity(vector_count);
    let mut row_ids = Vec::with_capacity(vector_count);
    for i in 0..vector_count {
        let raw_vector: Vec<f32> = raw_centroid
            .iter()
            .map(|&c| c + next_f32(&mut state))
            .collect();
        let rotated_vector = rotate_matrix(&raw_vector, padded_dims, &matrix);
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
        ex_region: None,
    };
    let (blob, dirs) = build_posting_lists(&[input], padded_dims);
    assert_eq!(
        dirs[0],
        ClusterDirEntry {
            region_offset: 0,
            code_bytes_length: dirs[0].code_bytes_length,
            vector_count: vector_count as u32
        }
    );

    // 4. Real read-back round trip.
    let reader = PostingListReader::new(&blob);
    let region = reader
        .read_cluster(&dirs[0], padded_dims, 0)
        .expect("valid cluster region");
    assert_eq!(region.compact_codes, compact_codes);
    assert_eq!(region.row_ids, row_ids);
    for i in 0..vector_count {
        assert_eq!(region.f_add[i].to_bits(), f_add[i].to_bits());
        assert_eq!(region.f_rescale[i].to_bits(), f_rescale[i].to_bits());
        assert_eq!(region.f_error[i].to_bits(), f_error[i].to_bits());
    }
}
