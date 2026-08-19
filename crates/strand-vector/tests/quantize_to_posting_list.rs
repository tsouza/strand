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

//! The first end-to-end test connecting real RaBitQ quantization
//! (`crate::quantize`) to the cluster posting-list wire format
//! (`crate::posting_list`): real (synthetic but non-trivial) vectors are
//! quantized against a real centroid, the resulting codes and factors are
//! packed into a real blob, and read back — proving the two pieces
//! actually compose, not just that each is independently correct.

use strand_vector::navigation::ClusterDirEntry;
use strand_vector::posting_list::{ClusterInput, PostingListReader, build_posting_lists};
use strand_vector::quantize::{MetricType, quantize_one_bit};

/// A small deterministic PRNG (no extra dependency) for reproducible
/// synthetic vectors.
fn next_f32(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    ((*state % 2001) as f32 - 1000.0) / 100.0 // roughly [-10.0, 10.0]
}

#[test]
fn quantizes_real_vectors_and_round_trips_them_through_the_posting_list_blob() {
    let padded_dims = 64;
    let vector_count = 37; // spans more than one FastScan batch (32+5)

    let mut state = 0x5EED_u64;
    let centroid: Vec<f32> = (0..padded_dims).map(|_| next_f32(&mut state)).collect();

    let mut all_compact_codes = Vec::new();
    let mut f_add = Vec::with_capacity(vector_count);
    let mut f_rescale = Vec::with_capacity(vector_count);
    let mut f_error = Vec::with_capacity(vector_count);
    let mut row_ids = Vec::with_capacity(vector_count);

    for i in 0..vector_count {
        // Each vector is the centroid plus real per-dimension noise, so
        // every vector has a real, non-degenerate residual against the
        // centroid (quantize_one_bit panics on an exact-zero residual).
        let vector: Vec<f32> = centroid.iter().map(|&c| c + next_f32(&mut state)).collect();
        let q = quantize_one_bit(&vector, &centroid, MetricType::L2);
        assert_eq!(q.compact_code.len(), padded_dims / 8);
        all_compact_codes.extend(q.compact_code);
        f_add.push(q.f_add);
        f_rescale.push(q.f_rescale);
        f_error.push(q.f_error);
        row_ids.push(1000 + i as u64);
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
            code_bytes_length: (2 * (padded_dims * 4 + 384)) as u64, // 2 batches: 32 + 5
            vector_count: vector_count as u32,
        }
    );

    let reader = PostingListReader::new(&blob);
    let region = reader
        .read_cluster(&dirs[0], padded_dims)
        .expect("valid cluster region");

    // Every real quantized code and factor round-trips through the wire
    // format exactly.
    assert_eq!(region.compact_codes, all_compact_codes);
    assert_eq!(region.f_add, f_add);
    assert_eq!(region.f_rescale, f_rescale);
    assert_eq!(region.f_error, f_error);
    assert_eq!(region.row_ids, row_ids);

    // Sanity check on the quantization itself: f_rescale is always
    // negative (per the L2 formula, -2*l2_sqr/ip_resi_xucb with l2_sqr > 0
    // and ip_resi_xucb == 0.5*||residual||_1 > 0 for any non-degenerate
    // residual), and f_error is always non-negative (it's a magnitude).
    for i in 0..vector_count {
        assert!(
            f_rescale[i] < 0.0,
            "f_rescale must be negative, got {} at index {i}",
            f_rescale[i]
        );
        assert!(
            f_error[i] >= 0.0,
            "f_error must be non-negative, got {} at index {i}",
            f_error[i]
        );
    }
}
