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

//! Reproduces RFC 0009's worked example (the same 3-posting term RFC 0007/
//! 0008 used — local ordinals 5, 12, 47; term frequencies 2, 1, 3 —
//! extended with within-document positions), the current, RFC-0009-amended
//! 8-byte layout (RFC 0008's original 12-byte layout is retired, not kept
//! alongside this one, per RFC 0009 Design §1), and checks it against the
//! pinned conformance golden file.

use strand_lexical::positions::{PositionsReader, build_positions};

fn toy_doc_positions() -> Vec<Vec<u32>> {
    vec![
        vec![3, 9],     // document at ordinal 5, tf = 2
        vec![0],        // document at ordinal 12, tf = 1
        vec![1, 4, 10], // document at ordinal 47, tf = 3
    ]
}

fn toy_term_freqs() -> Vec<u32> {
    vec![2, 1, 3]
}

#[test]
fn worked_example_matches_conformance_golden_file() {
    let bytes = build_positions(&toy_doc_positions());

    let golden = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/positions/toy-positions.bin"
    ))
    .expect("golden file conformance/positions/toy-positions.bin must exist");

    assert_eq!(
        bytes, golden,
        "must match RFC 0009's pinned 8 bytes exactly"
    );
    assert_eq!(
        bytes,
        vec![0x06, 0x00, 0x00, 0x00, 0x03, 0x33, 0x32, 0x03],
        "must match RFC 0009's worked example bytes exactly \
         (total_term_freq, no postings_block_pos_prefix entries since \
         index 0 is never stored, pos_widths, stream)"
    );
}

#[test]
fn worked_example_decodes_and_resolves_targeted_lookups_correctly() {
    let bytes = build_positions(&toy_doc_positions());
    // doc_freq = 3 (single postings block, block index 0 for every doc here).
    let reader = PositionsReader::new(&bytes, 3).unwrap();

    let decoded = reader.decode_all(&toy_term_freqs());
    assert_eq!(decoded, toy_doc_positions());

    // Postings block 0 is the only block; postings_block_pos_prefix[0] is
    // always 0 and is never stored (RFC 0009 Design §1) — the accessor
    // still returns 0 for it.
    assert_eq!(reader.postings_block_pos_prefix(0), 0);

    // Document at ordinal 5 (first in the block, local_prefix_tf = 0, tf = 2).
    assert_eq!(reader.positions_for_doc(0, 0, 2), vec![3, 9]);
    // Document at ordinal 12 (second in the block, preceded by tf=2, tf = 1).
    assert_eq!(reader.positions_for_doc(0, 2, 1), vec![0]);
    // Document at ordinal 47 (third in the block, preceded by tf=2+1=3, tf = 3).
    assert_eq!(reader.positions_for_doc(0, 3, 3), vec![1, 4, 10]);
}
