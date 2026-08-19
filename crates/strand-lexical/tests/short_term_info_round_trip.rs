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

//! Property-based round-trip tests for RFC 0009 Design §2's short,
//! positions-free term-info record: build from arbitrary (sorted,
//! deduplicated) terms, then confirm every term resolves to the right
//! ordinal and `TermInfo` back out — with `positions_offset`/
//! `positions_length` always decoding to `0`, regardless of what the input
//! `TermInfo` carried, since this record shape never writes them.

use proptest::prelude::*;
use std::collections::BTreeSet;
use strand_lexical::term_dictionary::{
    build_term_dictionary_short, ShortTermInfoStore, TermDictionary, TermInfo,
    SHORT_TERM_INFO_RECORD_LEN,
};

fn arb_term_info() -> impl Strategy<Value = TermInfo> {
    (any::<u32>(), any::<u64>(), any::<u32>(), any::<u64>(), any::<u32>()).prop_map(
        |(doc_freq, postings_offset, postings_length, positions_offset, positions_length)| {
            TermInfo {
                doc_freq,
                postings_offset,
                postings_length,
                positions_offset,
                positions_length,
            }
        },
    )
}

proptest! {
    #[test]
    fn short_record_round_trips_postings_fields_and_zeroes_positions(info in arb_term_info()) {
        let decoded = TermInfo::decode_short(&info.encode_short());
        prop_assert_eq!(decoded.doc_freq, info.doc_freq);
        prop_assert_eq!(decoded.postings_offset, info.postings_offset);
        prop_assert_eq!(decoded.postings_length, info.postings_length);
        prop_assert_eq!(decoded.positions_offset, 0);
        prop_assert_eq!(decoded.positions_length, 0);
    }

    #[test]
    fn every_built_term_resolves_through_the_short_store(
        raw_terms in prop::collection::hash_set("[a-z]{1,12}", 1..40),
        infos in prop::collection::vec(arb_term_info(), 40),
    ) {
        let sorted_terms: BTreeSet<Vec<u8>> = raw_terms.into_iter().map(String::into_bytes).collect();
        let pairs: Vec<(&[u8], TermInfo)> = sorted_terms
            .iter()
            .zip(infos.iter())
            .map(|(term, info)| (term.as_slice(), *info))
            .collect();

        let (fst_bytes, term_info_bytes) = build_term_dictionary_short(&pairs).unwrap();
        let dict = TermDictionary::open(fst_bytes).unwrap();
        let store = ShortTermInfoStore::new(&term_info_bytes).unwrap();

        prop_assert_eq!(store.len(), pairs.len());

        for (ordinal, (term, expected_info)) in pairs.iter().enumerate() {
            let looked_up_ordinal = dict.get(term);
            prop_assert_eq!(looked_up_ordinal, Some(ordinal as u64));
            let actual_info = store.get(ordinal as u64).unwrap();
            prop_assert_eq!(actual_info.doc_freq, expected_info.doc_freq);
            prop_assert_eq!(actual_info.postings_offset, expected_info.postings_offset);
            prop_assert_eq!(actual_info.postings_length, expected_info.postings_length);
            prop_assert_eq!(actual_info.positions_offset, 0);
            prop_assert_eq!(actual_info.positions_length, 0);
        }

        prop_assert_eq!(dict.get(b"this-term-was-never-inserted-xyz"), None);
    }

    #[test]
    fn short_term_info_store_rejects_truncated_blobs(len in 1usize..(SHORT_TERM_INFO_RECORD_LEN - 1)) {
        let bytes = vec![0u8; len];
        prop_assert!(ShortTermInfoStore::new(&bytes).is_err());
    }
}
