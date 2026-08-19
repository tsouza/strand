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

//! Reproduces RFC 0009 Design §2's short term-info worked example (a term
//! with `doc_freq = 3`, `postings_offset = 0`, `postings_length = 10` —
//! reusing RFC 0007's own 10-byte postings worked example's length for
//! continuity) and checks it against the pinned conformance golden file.

use strand_lexical::term_dictionary::{SHORT_TERM_INFO_RECORD_LEN, TermInfo};

fn toy_short_term_info() -> TermInfo {
    TermInfo {
        doc_freq: 3,
        postings_offset: 0,
        postings_length: 10,
        // Never written by encode_short; included here only to document
        // that a real caller building this shape ignores these fields.
        positions_offset: 0,
        positions_length: 0,
    }
}

#[test]
fn worked_example_matches_conformance_golden_file() {
    let bytes = toy_short_term_info().encode_short();

    let golden = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/term-dictionary/short-term-info-worked-example.bin"
    ))
    .expect(
        "golden file conformance/term-dictionary/short-term-info-worked-example.bin must exist",
    );

    assert_eq!(
        bytes.to_vec(),
        golden,
        "must match RFC 0009's pinned 16 bytes exactly"
    );
    assert_eq!(
        bytes,
        [
            0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0A, 0x00,
            0x00, 0x00
        ],
        "must match RFC 0009's worked example bytes exactly"
    );
}

#[test]
fn short_record_round_trips_and_zeroes_positions_fields() {
    let info = toy_short_term_info();
    let bytes = info.encode_short();
    let decoded = TermInfo::decode_short(&bytes);

    assert_eq!(decoded.doc_freq, 3);
    assert_eq!(decoded.postings_offset, 0);
    assert_eq!(decoded.postings_length, 10);
    // decode_short's own contract: positions fields are always 0, even if
    // the encoded TermInfo (never actually written) had non-zero values.
    assert_eq!(decoded.positions_offset, 0);
    assert_eq!(decoded.positions_length, 0);

    assert_eq!(bytes.len(), SHORT_TERM_INFO_RECORD_LEN);
}
