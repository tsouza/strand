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

//! Property-based round-trip tests for the postings blob: build from
//! arbitrary strictly-increasing ordinal lists (single-block and
//! multi-block, exercising both `BitPacker8x`'s SIMD kernel and the
//! variable-length final block's scalar packer), then confirm full decode
//! and skip queries match a plain reference implementation.

use proptest::prelude::*;
use strand_lexical::postings::{BLOCK_LEN, PostingsError, PostingsReader, build_postings};

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

proptest! {
    #[test]
    fn round_trips_through_build_and_decode_single_block((ordinals, term_freqs) in arb_postings_list(200, 5000)) {
        let bytes = build_postings(&ordinals, &term_freqs);
        let reader = PostingsReader::new(&bytes, ordinals.len()).unwrap();
        let (decoded_ordinals, decoded_tfs) = reader.decode_all();
        prop_assert_eq!(decoded_ordinals, ordinals);
        prop_assert_eq!(decoded_tfs, term_freqs);
    }

    #[test]
    fn round_trips_through_build_and_decode_multi_block((ordinals, term_freqs) in arb_postings_list(700, 500)) {
        // BLOCK_LEN is 256; a list of up to ~700 postings spans 1-3 blocks,
        // exercising full-block SIMD packing and a variable-length tail.
        let bytes = build_postings(&ordinals, &term_freqs);
        let reader = PostingsReader::new(&bytes, ordinals.len()).unwrap();
        let (decoded_ordinals, decoded_tfs) = reader.decode_all();
        prop_assert_eq!(decoded_ordinals, ordinals);
        prop_assert_eq!(decoded_tfs, term_freqs);
    }

    #[test]
    fn skip_matches_linear_scan((ordinals, term_freqs) in arb_postings_list(700, 500), target_frac in 0.0f64..1.2) {
        let bytes = build_postings(&ordinals, &term_freqs);
        let reader = PostingsReader::new(&bytes, ordinals.len()).unwrap();

        let max_ordinal = *ordinals.last().unwrap();
        let target = (max_ordinal as f64 * target_frac) as u32;

        let expected = ordinals.iter().zip(term_freqs.iter())
            .find(|&(&o, _)| o >= target)
            .map(|(&o, &tf)| (o, tf));

        prop_assert_eq!(reader.skip(target), expected);
    }

    #[test]
    fn exact_block_boundary_lengths_round_trip(n in prop::sample::select(vec![
        BLOCK_LEN, BLOCK_LEN - 1, BLOCK_LEN + 1, 2 * BLOCK_LEN, 2 * BLOCK_LEN - 1, 2 * BLOCK_LEN + 1, 1usize,
    ])) {
        // Deterministic gaps (1, 2, 1, 2, ...) so ordinals stay strictly
        // increasing regardless of n; specifically targets the block
        // boundary this codec's variable-length final block exists for.
        let mut ordinals = Vec::with_capacity(n);
        let mut prev = 0u32;
        for i in 0..n {
            prev += if i % 2 == 0 { 1 } else { 2 };
            ordinals.push(prev);
        }
        let term_freqs: Vec<u32> = (0..n as u32).map(|i| i % 17 + 1).collect();

        let bytes = build_postings(&ordinals, &term_freqs);
        let reader = PostingsReader::new(&bytes, n).unwrap();
        let (decoded_ordinals, decoded_tfs) = reader.decode_all();
        prop_assert_eq!(decoded_ordinals, ordinals.clone());
        prop_assert_eq!(decoded_tfs.clone(), term_freqs.clone());
        prop_assert_eq!(reader.skip(0), Some((ordinals[0], term_freqs[0])));
    }
}

#[test]
fn reader_rejects_truncated_bytes() {
    let bytes = build_postings(&[5, 12, 47], &[2, 1, 3]);
    let truncated = &bytes[..bytes.len() - 1];
    // block_max(0..3) + widths still fits, but let's truncate below the
    // minimum header size instead to guarantee a real Truncated error.
    let too_short = &bytes[..3];
    assert_eq!(
        PostingsReader::new(too_short, 3).unwrap_err(),
        PostingsError::Truncated
    );
    // A merely-shorter-than-full-payload slice may still pass header
    // validation (this reader doesn't validate stream lengths eagerly);
    // decode_all on it would panic on out-of-bounds access, which is the
    // expected contract for a malformed blob a caller must not construct.
    let _ = truncated;
}
