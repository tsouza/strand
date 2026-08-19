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

//! Reproduces RFC 0007's worked example (3 postings, single block) and
//! checks it against the pinned conformance golden file.

use strand_lexical::postings::{build_postings, PostingsReader};

fn toy_ordinals() -> Vec<u32> {
    vec![5, 12, 47]
}

fn toy_term_freqs() -> Vec<u32> {
    vec![2, 1, 3]
}

#[test]
fn worked_example_matches_conformance_golden_file() {
    let bytes = build_postings(&toy_ordinals(), &toy_term_freqs());

    let golden = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/postings/toy-postings.bin"
    ))
    .expect("golden file conformance/postings/toy-postings.bin must exist");

    assert_eq!(bytes, golden, "must match RFC 0007's pinned 10 bytes exactly");
    assert_eq!(
        bytes,
        vec![0x2F, 0x00, 0x00, 0x00, 0x06, 0x02, 0xC5, 0x31, 0x02, 0x36],
        "must match RFC 0007's worked example bytes exactly"
    );
}

#[test]
fn worked_example_decodes_and_skips_correctly() {
    let bytes = build_postings(&toy_ordinals(), &toy_term_freqs());
    let reader = PostingsReader::new(&bytes, 3).unwrap();

    let (ordinals, term_freqs) = reader.decode_all();
    assert_eq!(ordinals, toy_ordinals());
    assert_eq!(term_freqs, toy_term_freqs());

    assert_eq!(reader.block_max(0), 47);

    // Resolving the query from RFC 0007's own "Query resolution" (§6/§7):
    // skip to the first posting >= 12.
    assert_eq!(reader.skip(12), Some((12, 1)));
    assert_eq!(reader.skip(6), Some((12, 1)));
    assert_eq!(reader.skip(48), None, "no posting >= 48 exists");
    assert_eq!(reader.skip(5), Some((5, 2)), "exact match on the first posting");
}
