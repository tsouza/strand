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

//! The first real end-to-end test in this project: real document text in,
//! through the analyzer, into real term-dictionary/term-info/postings
//! blobs, assembled into a real segment, written and committed through
//! `strand-core`'s actual store and manifest layers, read back cold (a
//! footer and hotcache decode, per invariant 3), and resolved into real
//! query results — including a BM25-ranked search. Every prior test in
//! this repository checked one blob type in isolation; this is the first
//! proof the pieces actually compose.

use strand_core::container::{Footer, Hotcache};
use strand_core::manifest::{commit, read_snapshot};
use strand_core::scoring::Bm25Profile;
use strand_core::segment::{write_segment, SegmentBuilder};
use strand_core::store::{ConditionalStore, InMemoryStore};
use strand_lexical::field::{build_field, FieldReader};

const DOCS: [&str; 3] = [
    "the dog runs in the park",
    "a cat sleeps on the mat",
    "the dog and the cat play in the park",
];

fn open_segment_bytes(bytes: &[u8]) -> Hotcache {
    let footer_bytes: [u8; 40] = bytes[bytes.len() - 40..].try_into().unwrap();
    let footer = Footer::decode(&footer_bytes).expect("valid footer");
    let start = footer.hotcache_offset as usize;
    let end = start + footer.hotcache_length as usize;
    Hotcache::decode(&bytes[start..end]).expect("valid hotcache")
}

#[test]
fn builds_writes_commits_and_queries_a_real_field_end_to_end() {
    let field = build_field(&DOCS);
    assert_eq!(field.doc_lengths.len(), 3);

    let mut builder = SegmentBuilder::new(DOCS.len() as u64);
    for blob in field.to_blob_specs() {
        builder.add_blob(blob);
    }

    let store = InMemoryStore::new();
    let snapshot = commit(&store, |row_id_base| {
        vec![write_segment(&store, "segments/field-0.bin", &builder, row_id_base)]
    })
    .expect("commit succeeds against an empty table");

    assert_eq!(snapshot.version, 0);
    assert_eq!(snapshot.next_row_id, 3);
    assert_eq!(snapshot.segments.len(), 1);

    // Real reader path: pointer -> snapshot -> segment bytes -> footer ->
    // hotcache -> blob registry -> FieldReader (invariant 3's one-wave
    // rule: everything past this point is already-resident bytes).
    let read_back = read_snapshot(&store).expect("read succeeds").expect("a snapshot exists");
    assert_eq!(read_back, snapshot);

    let segment_ref = &read_back.segments[0];
    let (segment_bytes, _) =
        ConditionalStore::get(&store, &segment_ref.path).expect("get succeeds").expect("segment exists");
    let hotcache = open_segment_bytes(&segment_bytes);
    assert_eq!(hotcache.row_id_count, 3);

    let reader = FieldReader::open(&segment_bytes, &hotcache.blobs).expect("all three blobs present");

    // "dog" appears in docs 0 and 2; "cat" in docs 1 and 2; "park" in 0 and 2.
    let dog_matches = reader.lookup("dog").expect("dog is a real term");
    let mut dog_docs: Vec<u32> = dog_matches.iter().map(|&(doc, _)| doc).collect();
    dog_docs.sort_unstable();
    assert_eq!(dog_docs, vec![0, 2]);

    let cat_matches = reader.lookup("cat").expect("cat is a real term");
    let mut cat_docs: Vec<u32> = cat_matches.iter().map(|&(doc, _)| doc).collect();
    cat_docs.sort_unstable();
    assert_eq!(cat_docs, vec![1, 2]);

    assert!(reader.lookup("giraffe").is_none(), "a real miss must be None, not an error");

    // BM25-ranked search: "park" appears in docs 0 and 2, doc 0 is shorter
    // so should rank at least as high after length normalization.
    let profile = Bm25Profile::default();
    let ranked =
        reader.search_bm25("park", &field.doc_lengths, &profile).expect("park is a real term");
    let ranked_docs: Vec<u32> = ranked.iter().map(|&(doc, _)| doc).collect();
    assert_eq!(ranked_docs.len(), 2);
    assert!(ranked_docs.contains(&0));
    assert!(ranked_docs.contains(&2));
    assert!(
        ranked.windows(2).all(|w| w[0].1 >= w[1].1),
        "results must be sorted by descending score: {ranked:?}"
    );

    assert!(reader.search_bm25("giraffe", &field.doc_lengths, &profile).is_none());
}
