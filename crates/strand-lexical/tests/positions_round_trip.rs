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

//! Property-based round-trip tests for the positions blob: build from
//! arbitrary per-document within-document position lists (single- and
//! multi-block, exercising both `BitPacker8x`'s SIMD kernel and the
//! variable-length final block's scalar packer, with postings-block and
//! position-block boundaries stressed independently since they're
//! different counts), then confirm full decode and targeted lookups match
//! the original input directly — not each other.

use proptest::prelude::*;
use strand_lexical::positions::{PositionsError, PositionsReader, build_positions};
use strand_lexical::postings::BLOCK_LEN;

fn arb_doc_positions(
    max_docs: usize,
    max_positions_per_doc: usize,
    max_gap: u32,
) -> impl Strategy<Value = Vec<Vec<u32>>> {
    prop::collection::vec(
        prop::collection::vec(1u32..=max_gap, 1..=max_positions_per_doc).prop_map(|gaps| {
            let mut positions = Vec::with_capacity(gaps.len());
            let mut prev = 0u32;
            for g in gaps {
                prev += g;
                positions.push(prev);
            }
            positions
        }),
        1..max_docs,
    )
}

fn term_freqs_of(doc_positions: &[Vec<u32>]) -> Vec<u32> {
    doc_positions.iter().map(|p| p.len() as u32).collect()
}

/// The postings-block index and the sum of `tf` for every document
/// strictly before `target_doc_idx` within that same block — computed
/// directly from the original input, exactly as a real caller (who has
/// already run `spec/postings.md` §6's skip query) would have it.
fn locate(doc_positions: &[Vec<u32>], target_doc_idx: usize) -> (usize, u32, u32) {
    let block_idx = target_doc_idx / BLOCK_LEN;
    let block_start = block_idx * BLOCK_LEN;
    let local_prefix_tf: u32 = doc_positions[block_start..target_doc_idx]
        .iter()
        .map(|p| p.len() as u32)
        .sum();
    let tf = doc_positions[target_doc_idx].len() as u32;
    (block_idx, local_prefix_tf, tf)
}

proptest! {
    #[test]
    fn round_trips_through_build_and_decode_single_block(
        doc_positions in arb_doc_positions(50, 4, 5000)
    ) {
        let bytes = build_positions(&doc_positions);
        let term_freqs = term_freqs_of(&doc_positions);
        let reader = PositionsReader::new(&bytes, doc_positions.len()).unwrap();

        prop_assert_eq!(reader.decode_all(&term_freqs), doc_positions);
    }

    #[test]
    fn round_trips_through_build_and_decode_multi_block(
        doc_positions in arb_doc_positions(700, 3, 500)
    ) {
        // Up to ~700 docs spans 1-3 postings blocks; up to 3 positions/doc
        // means total_term_freq can independently span 1-9 position
        // blocks — the two block counts are genuinely different axes.
        let bytes = build_positions(&doc_positions);
        let term_freqs = term_freqs_of(&doc_positions);
        let reader = PositionsReader::new(&bytes, doc_positions.len()).unwrap();

        prop_assert_eq!(reader.decode_all(&term_freqs), doc_positions);
    }

    #[test]
    fn targeted_lookup_matches_original_input_directly(
        doc_positions in arb_doc_positions(700, 3, 500),
        target_frac in 0.0f64..1.0
    ) {
        let bytes = build_positions(&doc_positions);
        let reader = PositionsReader::new(&bytes, doc_positions.len()).unwrap();

        let target_doc_idx = ((doc_positions.len() as f64 - 1.0) * target_frac).round() as usize;
        let (block_idx, local_prefix_tf, tf) = locate(&doc_positions, target_doc_idx);

        prop_assert_eq!(
            reader.positions_for_doc(block_idx, local_prefix_tf, tf),
            doc_positions[target_doc_idx].clone()
        );
    }

    #[test]
    fn exact_block_boundary_lengths_round_trip(doc_freq in prop::sample::select(vec![
        BLOCK_LEN, BLOCK_LEN - 1, BLOCK_LEN + 1, 2 * BLOCK_LEN, 2 * BLOCK_LEN - 1, 2 * BLOCK_LEN + 1, 1usize,
    ]), positions_per_doc in prop::sample::select(vec![1usize, 2, 3])) {
        // Deterministic positions (1, 3, 5, ... per doc) so every document
        // has exactly `positions_per_doc` strictly increasing positions,
        // stressing postings-block and position-block boundaries at the
        // same time from different, independently chosen counts.
        let mut doc_positions = Vec::with_capacity(doc_freq);
        for _ in 0..doc_freq {
            let positions: Vec<u32> = (0..positions_per_doc as u32).map(|i| i * 2 + 1).collect();
            doc_positions.push(positions);
        }
        let term_freqs = term_freqs_of(&doc_positions);

        let bytes = build_positions(&doc_positions);
        let reader = PositionsReader::new(&bytes, doc_freq).unwrap();

        prop_assert_eq!(reader.decode_all(&term_freqs), doc_positions.clone());
        let (block_idx, local_prefix_tf, tf) = locate(&doc_positions, 0);
        prop_assert_eq!(reader.positions_for_doc(block_idx, local_prefix_tf, tf), doc_positions[0].clone());
    }
}

#[test]
fn reader_rejects_truncated_bytes() {
    let doc_positions = vec![vec![3u32, 9], vec![0], vec![1, 4, 10]];
    let bytes = build_positions(&doc_positions);
    let too_short = &bytes[..3];
    assert_eq!(
        PositionsReader::new(too_short, 3).unwrap_err(),
        PositionsError::Truncated
    );
}
