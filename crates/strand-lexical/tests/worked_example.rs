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

//! Reproduces RFC 0005's ("cat"/"dog"/"fish") worked example and checks it
//! against the pinned conformance golden files.

use strand_lexical::term_dictionary::{
    TermDictionary, TermInfo, TermInfoStore, build_term_dictionary,
};

fn toy_terms() -> Vec<(&'static [u8], TermInfo)> {
    vec![
        (
            b"cat".as_slice(),
            TermInfo {
                doc_freq: 1,
                postings_offset: 0,
                postings_length: 4,
                positions_offset: 0,
                positions_length: 0,
            },
        ),
        (
            b"dog".as_slice(),
            TermInfo {
                doc_freq: 2,
                postings_offset: 4,
                postings_length: 8,
                positions_offset: 0,
                positions_length: 0,
            },
        ),
        (
            b"fish".as_slice(),
            TermInfo {
                doc_freq: 1,
                postings_offset: 12,
                postings_length: 4,
                positions_offset: 0,
                positions_length: 0,
            },
        ),
    ]
}

#[test]
fn worked_example_matches_conformance_golden_files() {
    let (fst_bytes, term_info_bytes) = build_term_dictionary(&toy_terms()).unwrap();

    let golden_fst = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/term-dictionary/toy-terms.fst"
    ))
    .expect("golden file conformance/term-dictionary/toy-terms.fst must exist");
    let golden_term_info = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/term-dictionary/toy-terms.terminfo"
    ))
    .expect("golden file conformance/term-dictionary/toy-terms.terminfo must exist");

    assert_eq!(
        fst_bytes, golden_fst,
        "FST bytes must match RFC 0005's pinned 60 bytes"
    );
    assert_eq!(
        term_info_bytes, golden_term_info,
        "term-info bytes must match RFC 0005's pinned 84 bytes"
    );
}

#[test]
fn worked_example_resolves_query_term_per_the_rfc() {
    let (fst_bytes, term_info_bytes) = build_term_dictionary(&toy_terms()).unwrap();

    let dict = TermDictionary::open(fst_bytes).unwrap();
    let store = TermInfoStore::new(&term_info_bytes).unwrap();

    assert_eq!(dict.get(b"cat"), Some(0));
    assert_eq!(dict.get(b"dog"), Some(1));
    assert_eq!(dict.get(b"fish"), Some(2));
    assert_eq!(dict.get(b"bird"), None, "a lookup miss is a normal outcome");

    let ordinal = dict.get(b"dog").unwrap();
    let info = store.get(ordinal).unwrap();
    assert_eq!(info.doc_freq, 2);
    assert_eq!(info.postings_offset, 4);
    assert_eq!(info.postings_length, 8);
}
