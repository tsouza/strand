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

//! Property-based round-trip tests for the term-dictionary FST and term-info
//! store: build from arbitrary (sorted, deduplicated) terms, then confirm
//! every term resolves to the right ordinal and `TermInfo` back out.

use proptest::prelude::*;
use std::collections::BTreeSet;
use strand_lexical::term_dictionary::{
    build_term_dictionary, TermDictionary, TermInfo, TermInfoStore,
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
    fn term_info_round_trips_through_encode_decode(info in arb_term_info()) {
        let decoded = TermInfo::decode(&info.encode());
        prop_assert_eq!(info, decoded);
    }

    #[test]
    fn every_built_term_resolves_to_its_ordinal_and_term_info(
        raw_terms in prop::collection::hash_set("[a-z]{1,12}", 1..40),
        infos in prop::collection::vec(arb_term_info(), 40),
    ) {
        // Sorted, deduplicated unsigned-UTF-8-byte-order terms, per invariant 11 —
        // `fst::MapBuilder::insert` requires strictly increasing keys.
        let sorted_terms: BTreeSet<Vec<u8>> = raw_terms.into_iter().map(String::into_bytes).collect();
        let pairs: Vec<(&[u8], TermInfo)> = sorted_terms
            .iter()
            .zip(infos.iter())
            .map(|(term, info)| (term.as_slice(), *info))
            .collect();

        let (fst_bytes, term_info_bytes) = build_term_dictionary(&pairs).unwrap();
        let dict = TermDictionary::open(fst_bytes).unwrap();
        let store = TermInfoStore::new(&term_info_bytes).unwrap();

        prop_assert_eq!(store.len(), pairs.len());

        for (ordinal, (term, expected_info)) in pairs.iter().enumerate() {
            let looked_up_ordinal = dict.get(term);
            prop_assert_eq!(looked_up_ordinal, Some(ordinal as u64));
            let actual_info = store.get(ordinal as u64).unwrap();
            prop_assert_eq!(&actual_info, expected_info);
        }

        // A term that was never inserted misses cleanly.
        prop_assert_eq!(dict.get(b"this-term-was-never-inserted-xyz"), None);
    }

    #[test]
    fn term_info_store_rejects_truncated_blobs(len in 1usize..27) {
        let bytes = vec![0u8; len];
        prop_assert!(TermInfoStore::new(&bytes).is_err());
    }
}
