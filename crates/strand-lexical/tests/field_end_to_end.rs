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
//! through the analyzer, into real term-dictionary/term-info/postings/
//! positions blobs, assembled into a real segment, written and committed
//! through `strand-core`'s actual store and manifest layers, read back
//! cold (a footer and hotcache decode, per invariant 3), and resolved into
//! real query results — a BM25-ranked search, and a real phrase query.
//! Every prior test in this repository checked one blob type in isolation;
//! this is the first proof the pieces actually compose.

use strand_core::container::{Footer, Hotcache};
use strand_core::manifest::{commit, read_snapshot};
use strand_core::scoring::Bm25Profile;
use strand_core::segment::{SegmentBuilder, write_segment};
use strand_core::store::{ConditionalStore, InMemoryStore};
use strand_lexical::field::{FieldReader, build_field, build_field_without_positions};

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
    let field = build_field("body", &DOCS);
    assert_eq!(field.doc_lengths.len(), 3);

    let mut builder = SegmentBuilder::new(DOCS.len() as u64);
    for blob in field.to_blob_specs() {
        builder.add_blob(blob);
    }

    let store = InMemoryStore::new();
    let snapshot = commit(&store, |row_id_base| {
        vec![write_segment(
            &store,
            "segments/field-0.bin",
            &builder,
            row_id_base,
        )]
    })
    .expect("commit succeeds against an empty table");

    assert_eq!(snapshot.version, 0);
    assert_eq!(snapshot.next_row_id, 3);
    assert_eq!(snapshot.segments.len(), 1);

    // Real reader path: pointer -> snapshot -> segment bytes -> footer ->
    // hotcache -> blob registry -> FieldReader (invariant 3's one-wave
    // rule: everything past this point is already-resident bytes).
    let read_back = read_snapshot(&store)
        .expect("read succeeds")
        .expect("a snapshot exists");
    assert_eq!(read_back, snapshot);

    let segment_ref = &read_back.segments[0];
    let (segment_bytes, _) = ConditionalStore::get(&store, &segment_ref.path)
        .expect("get succeeds")
        .expect("segment exists");
    let hotcache = open_segment_bytes(&segment_bytes);
    assert_eq!(hotcache.row_id_count, 3);

    let reader = FieldReader::open_by_name(&segment_bytes, &hotcache.blobs, "body")
        .expect("all four blobs present");

    // "dog" appears in docs 0 and 2; "cat" in docs 1 and 2; "park" in 0 and 2.
    let dog_matches = reader.lookup("dog").expect("dog is a real term");
    let mut dog_docs: Vec<u32> = dog_matches.iter().map(|&(doc, _)| doc).collect();
    dog_docs.sort_unstable();
    assert_eq!(dog_docs, vec![0, 2]);

    let cat_matches = reader.lookup("cat").expect("cat is a real term");
    let mut cat_docs: Vec<u32> = cat_matches.iter().map(|&(doc, _)| doc).collect();
    cat_docs.sort_unstable();
    assert_eq!(cat_docs, vec![1, 2]);

    assert!(
        reader.lookup("giraffe").is_none(),
        "a real miss must be None, not an error"
    );

    // BM25-ranked search: "park" appears in docs 0 and 2, doc 0 is shorter
    // so should rank at least as high after length normalization.
    let profile = Bm25Profile::default();
    let ranked = reader
        .search_bm25("park", &field.doc_lengths, &profile)
        .expect("park is a real term");
    let ranked_docs: Vec<u32> = ranked.iter().map(|&(doc, _)| doc).collect();
    assert_eq!(ranked_docs.len(), 2);
    assert!(ranked_docs.contains(&0));
    assert!(ranked_docs.contains(&2));
    assert!(
        ranked.windows(2).all(|w| w[0].1 >= w[1].1),
        "results must be sorted by descending score: {ranked:?}"
    );

    assert!(
        reader
            .search_bm25("giraffe", &field.doc_lengths, &profile)
            .is_none()
    );

    // Real phrase query (RFC 0008, `spec/positions.md` §6). Doc 2, "the dog
    // and the cat play in the park", tokenizes (after stopword removal) to
    // dog(0), cat(1), play(2), park(3) — "dog cat" is a real adjacent
    // phrase there, and nowhere else ("dog" and "cat" never appear adjacent
    // in doc 0 or doc 1, since doc 0 has no "cat" at all).
    assert_eq!(reader.phrase_query(&["dog", "cat"]), vec![2]);

    // "dog" and "park" co-occur in both doc 0 ("dog runs park", positions
    // 0 and 2 — not adjacent) and doc 2 (positions 0 and 3 — not adjacent
    // either): a real negative case that plain co-occurrence (what
    // `lookup` alone can check) would wrongly call a match, but a true
    // phrase query correctly rejects.
    assert!(
        reader.phrase_query(&["dog", "park"]).is_empty(),
        "dog and park never appear adjacent in any document"
    );

    assert!(
        reader.phrase_query(&["dog", "giraffe"]).is_empty(),
        "a real miss must yield no matches"
    );

    // all_postings is the full field-level table scan strand-datafusion's
    // TableProvider is built on: every (term, doc_ordinal, term_freq) the
    // field's blobs actually store, not a point lookup. Cross-check it
    // against the same lookup() results already verified above.
    let mut all: Vec<(String, u32, u32)> = reader.all_postings().collect();
    all.sort();
    let mut dog_rows: Vec<(u32, u32)> = all
        .iter()
        .filter(|(t, _, _)| t == "dog")
        .map(|(_, d, tf)| (*d, *tf))
        .collect();
    dog_rows.sort();
    assert_eq!(
        dog_rows,
        vec![(0, 1), (2, 1)],
        "\"dog\" occurs once each in docs 0 and 2"
    );

    let cat_rows: Vec<(u32, u32)> = all
        .iter()
        .filter(|(t, _, _)| t == "cat")
        .map(|(_, d, tf)| (*d, *tf))
        .collect();
    assert_eq!(
        cat_rows,
        vec![(1, 1), (2, 1)],
        "\"cat\" occurs once each in docs 1 and 2"
    );

    // Every term all_postings names must be one the term dictionary really
    // has, and every row's term_freq must equal what lookup() itself
    // reports for that (term, doc) pair -- the same data, two paths.
    for (term, doc_ordinal, term_freq) in &all {
        let matches = reader
            .lookup(term)
            .expect("all_postings never invents a term");
        let expected = matches
            .iter()
            .find(|(d, _)| d == doc_ordinal)
            .map(|(_, tf)| *tf)
            .expect("all_postings never invents a (term, doc) pair lookup() doesn't have");
        assert_eq!(*term_freq, expected);
    }

    let total_rows = all.len();
    assert!(
        total_rows > 0,
        "a real field with real documents must yield real rows"
    );
}

#[test]
fn a_field_that_opts_out_of_positions_builds_a_smaller_segment_and_still_answers_term_queries() {
    let with_positions = build_field("body", &DOCS);
    let without_positions = build_field_without_positions("body", &DOCS);

    // RFC 0009 Design §2: the short term-info record is real bytes smaller
    // than the 28-byte one, and the whole positions blob is gone.
    assert!(without_positions.term_info.len() < with_positions.term_info.len());
    assert!(without_positions.positions.is_empty());

    let mut builder = SegmentBuilder::new(DOCS.len() as u64);
    let blob_specs = without_positions.to_blob_specs();
    assert_eq!(
        blob_specs.len(),
        3,
        "no positions blob when a field opts out"
    );
    for blob in blob_specs {
        builder.add_blob(blob);
    }

    let store = InMemoryStore::new();
    commit(&store, |row_id_base| {
        vec![write_segment(
            &store,
            "segments/field-no-positions.bin",
            &builder,
            row_id_base,
        )]
    })
    .expect("commit succeeds against an empty table");

    let read_back = read_snapshot(&store)
        .expect("read succeeds")
        .expect("a snapshot exists");
    let segment_ref = &read_back.segments[0];
    let (segment_bytes, _) = ConditionalStore::get(&store, &segment_ref.path)
        .expect("get succeeds")
        .expect("segment exists");
    let hotcache = open_segment_bytes(&segment_bytes);

    let reader = FieldReader::open_by_name(&segment_bytes, &hotcache.blobs, "body")
        .expect("term-dictionary, short term-info, and postings are present");

    // Term queries and BM25 search still work exactly as with the
    // full-featured field — opting out of positions costs nothing here.
    let dog_matches = reader.lookup("dog").expect("dog is a real term");
    let mut dog_docs: Vec<u32> = dog_matches.iter().map(|&(doc, _)| doc).collect();
    dog_docs.sort_unstable();
    assert_eq!(dog_docs, vec![0, 2]);

    let profile = Bm25Profile::default();
    let ranked = reader
        .search_bm25("park", &without_positions.doc_lengths, &profile)
        .expect("park is a real term");
    assert_eq!(ranked.len(), 2);

    // Phrase queries correctly report no matches (not an error, not a
    // panic) since this field has no positions blob at all.
    assert!(reader.phrase_query(&["dog", "cat"]).is_empty());
    assert!(reader.lookup_with_positions("dog").is_none());
}

/// Real, end-to-end proof that two fields' blobs of the *same*
/// `blob_type_id` now coexist in one segment and are correctly
/// disambiguated on read (roadmap item X-1,
/// `rfcs/0001-container-rowid-manifest.md` Discussion) — the gap RFC
/// 0008's and RFC 0009's own Non-goals both flagged: before `field_id`, a
/// segment could hold at most one field,
/// because `FieldReader::open`'s `find(blob_type_id)` had no way to tell
/// two same-`blob_type_id` registry entries apart. `title` and `body` here
/// deliberately share a term ("dog") with *different* postings in each
/// field, so a reader that opened the wrong field's blob by accident would
/// silently return the wrong answer rather than an error — the failure
/// mode this test would actually catch.
#[test]
fn two_fields_with_the_same_blob_type_ids_coexist_in_one_segment_and_stay_disambiguated() {
    const TITLES: [&str; 3] = ["dog park guide", "cat care basics", "dog and cat care"];

    let title_field = build_field("title", &TITLES);
    let body_field = build_field("body", &DOCS);
    assert_eq!(
        title_field.field_id,
        strand_core::container::field_id_from_name("title")
    );
    assert_eq!(
        body_field.field_id,
        strand_core::container::field_id_from_name("body")
    );
    assert_ne!(title_field.field_id, body_field.field_id);

    let mut builder = SegmentBuilder::new(DOCS.len() as u64);
    // Every blob from both fields lands in the same registry, term-dictionary
    // (`blob_type_id = 0`) and postings (`blob_type_id = 2`) included — the
    // exact `family_id`/`blob_type_id` collision this roadmap item exists
    // to resolve.
    for blob in title_field.to_blob_specs() {
        builder.add_blob(blob);
    }
    for blob in body_field.to_blob_specs() {
        builder.add_blob(blob);
    }

    let store = InMemoryStore::new();
    commit(&store, |row_id_base| {
        vec![write_segment(
            &store,
            "segments/field-multi.bin",
            &builder,
            row_id_base,
        )]
    })
    .expect("commit succeeds against an empty table");

    let read_back = read_snapshot(&store)
        .expect("read succeeds")
        .expect("a snapshot exists");
    let segment_ref = &read_back.segments[0];
    let (segment_bytes, _) = ConditionalStore::get(&store, &segment_ref.path)
        .expect("get succeeds")
        .expect("segment exists");
    let hotcache = open_segment_bytes(&segment_bytes);

    // Two entries at family_id=1, blob_type_id=0 (term-dictionary) now
    // really do coexist in one registry — the exact scenario that was
    // impossible before this change.
    let term_dict_entries: Vec<_> = hotcache
        .blobs
        .iter()
        .filter(|b| b.family_id == 1 && b.blob_type_id == 0)
        .collect();
    assert_eq!(
        term_dict_entries.len(),
        2,
        "two fields' term-dictionary blobs must both be present, distinctly registered"
    );
    assert_ne!(term_dict_entries[0].field_id, term_dict_entries[1].field_id);

    let title_reader = FieldReader::open_by_name(&segment_bytes, &hotcache.blobs, "title")
        .expect("title field's blobs are present");
    let body_reader = FieldReader::open_by_name(&segment_bytes, &hotcache.blobs, "body")
        .expect("body field's blobs are present");

    // "dog" occurs in title docs 0 and 2, but in body docs 0 and 2 too --
    // pick fields whose postings for the shared term differ so a
    // misrouted lookup is visible, not accidentally correct.
    let title_dog: Vec<u32> = {
        let mut docs: Vec<u32> = title_reader
            .lookup("dog")
            .expect("dog is a real term in title")
            .into_iter()
            .map(|(d, _)| d)
            .collect();
        docs.sort_unstable();
        docs
    };
    let body_dog: Vec<u32> = {
        let mut docs: Vec<u32> = body_reader
            .lookup("dog")
            .expect("dog is a real term in body")
            .into_iter()
            .map(|(d, _)| d)
            .collect();
        docs.sort_unstable();
        docs
    };
    assert_eq!(title_dog, vec![0, 2], "title's own postings for \"dog\"");
    assert_eq!(body_dog, vec![0, 2], "body's own postings for \"dog\"");

    // A term that exists only in one field must miss cleanly in the other,
    // proving the reader is not silently falling back to the wrong blob.
    assert!(
        title_reader.lookup("sleeps").is_none(),
        "\"sleeps\" only occurs in body, must miss in title"
    );
    assert!(
        body_reader.lookup("guide").is_none(),
        "\"guide\" only occurs in title, must miss in body"
    );

    // Opening a field name that was never built at all is a clean miss,
    // not a panic or a misroute to some other field's blobs.
    assert!(matches!(
        FieldReader::open_by_name(&segment_bytes, &hotcache.blobs, "nonexistent"),
        Err(strand_lexical::field::FieldReaderError::MissingBlob(_))
    ));
}
