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

//! Tests for `PostingsReader::batches` — the postings blob's `next_batch`
//! consumer of invariant 9's frozen batch-reader shape
//! (`crates/strand-core/src/batch.rs`).

use proptest::prelude::*;
use strand_core::batch::BatchReader;
use strand_lexical::postings::{BLOCK_LEN, PostingsReader, build_postings};

fn arb_postings_list(max_len: usize, max_gap: u32) -> impl Strategy<Value = (Vec<u32>, Vec<u32>)> {
    prop::collection::vec((1u32..=max_gap, 0u32..=1_000_000u32), 1..max_len).prop_map(|pairs| {
        let mut ordinals = Vec::with_capacity(pairs.len());
        let mut term_freqs = Vec::with_capacity(pairs.len());
        let mut prev = 0u32;
        for (gap, tf) in pairs {
            prev += gap;
            ordinals.push(prev);
            term_freqs.push(tf);
        }
        (ordinals, term_freqs)
    })
}

#[test]
fn single_block_yields_one_batch_then_exhausts() {
    let ordinals = vec![5u32, 12, 47];
    let term_freqs = vec![2u32, 1, 3];
    let bytes = build_postings(&ordinals, &term_freqs);
    let reader = PostingsReader::new(&bytes, ordinals.len()).unwrap();
    let mut batches = reader.batches();

    let mut out = Vec::new();
    assert_eq!(batches.next_batch(&mut out), 3);
    assert_eq!(out, vec![(5, 2), (12, 1), (47, 3)]);

    let before_len = out.len();
    assert_eq!(
        batches.next_batch(&mut out),
        0,
        "exhausted reader must return 0"
    );
    assert_eq!(
        out.len(),
        before_len,
        "exhausted call must not append anything"
    );
}

#[test]
fn multi_block_batches_align_with_block_boundaries() {
    // 300 postings spans 2 blocks at BLOCK_LEN=256: batch sizes 256, 44.
    let n = BLOCK_LEN + 44;
    let mut ordinals = Vec::with_capacity(n);
    let mut prev = 0u32;
    for _ in 0..n {
        prev += 3;
        ordinals.push(prev);
    }
    let term_freqs: Vec<u32> = (0..n as u32).map(|i| i % 13 + 1).collect();

    let bytes = build_postings(&ordinals, &term_freqs);
    let reader = PostingsReader::new(&bytes, n).unwrap();
    let mut batches = reader.batches();

    let mut first = Vec::new();
    assert_eq!(batches.next_batch(&mut first), BLOCK_LEN);

    let mut second = Vec::new();
    assert_eq!(batches.next_batch(&mut second), 44);

    let mut third = Vec::new();
    assert_eq!(batches.next_batch(&mut third), 0);

    let mut all: Vec<(u32, u32)> = Vec::new();
    all.extend(first);
    all.extend(second);
    let expected: Vec<(u32, u32)> = ordinals.into_iter().zip(term_freqs).collect();
    assert_eq!(all, expected);
}

proptest! {
    #[test]
    fn batches_concatenate_to_decode_all((ordinals, term_freqs) in arb_postings_list(700, 500)) {
        let bytes = build_postings(&ordinals, &term_freqs);
        let reader = PostingsReader::new(&bytes, ordinals.len()).unwrap();

        let mut batches = reader.batches();
        let mut collected: Vec<(u32, u32)> = Vec::new();
        loop {
            let mut batch = Vec::new();
            let n = batches.next_batch(&mut batch);
            if n == 0 {
                prop_assert!(batch.is_empty());
                break;
            }
            prop_assert_eq!(batch.len(), n);
            collected.extend(batch);
        }

        let (decoded_ordinals, decoded_tfs) = reader.decode_all();
        let expected: Vec<(u32, u32)> = decoded_ordinals.into_iter().zip(decoded_tfs).collect();
        prop_assert_eq!(collected, expected);
    }
}
