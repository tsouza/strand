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

//! `strand-tools convert`: imports one field of a real tantivy index into a
//! real STRAND segment file (`CLAUDE.md`'s repository shape names this
//! "strand-tools/ CLI: inspect, verify, convert (tantivy/CIFF importers)").
//!
//! Reads tantivy's own on-disk index via its real reader API (`InvertedIndexReader`,
//! `TermDictionary::stream`, `Postings`) — not a hand-reimplementation of
//! tantivy's binary format — and feeds the extracted `(term, doc_ordinal,
//! within_document_positions)` triples into
//! `strand_lexical::field::build_field_from_postings`, the same real,
//! already-tested blob-building code `build_field` uses for text input.
//! Tantivy's segment-local `DocId` becomes the STRAND local ordinal
//! directly — both are already dense `0..num_docs` spaces, so no remapping
//! is needed for the cases this importer accepts.
//!
//! Deliberately narrow, stated scope, not silently assumed general: only a
//! **single-segment**, **deletion-free** tantivy index is accepted.
//! Multi-segment import would need to merge segments into one STRAND
//! segment (a real "rebuild" per invariant 1, `spec/postings.md` §7) or a
//! multi-segment STRAND output — real, separate follow-on work, not
//! attempted here. A segment with deletions would need STRAND's own
//! deletion-vector mechanism (invariant 2) wired in, which nothing in this
//! project does yet.

use std::collections::BTreeMap;
use std::path::Path;

use strand_lexical::field::{build_field_from_postings, FieldBlobs};
use tantivy::postings::Postings;
use tantivy::schema::IndexRecordOption;
use tantivy::{DocSet, Index, TantivyError, TERMINATED};

#[derive(Debug)]
pub enum ImportError {
    Tantivy(TantivyError),
    /// A raw I/O error surfaced by tantivy's own reader (its `TermDictionary`/
    /// `Postings` APIs return `std::io::Result` directly rather than
    /// wrapping every error in `TantivyError`).
    Io(std::io::Error),
    /// The requested field does not exist in the index's schema.
    UnknownField(String),
    /// This importer only accepts a single-segment index (Non-goals, above).
    MultiSegment(usize),
    /// This importer only accepts a segment with no deleted documents.
    HasDeletions(u32),
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::Tantivy(e) => write!(f, "tantivy: {e}"),
            ImportError::Io(e) => write!(f, "tantivy reader I/O: {e}"),
            ImportError::UnknownField(name) => write!(f, "field {name:?} not found in index schema"),
            ImportError::MultiSegment(n) => {
                write!(f, "index has {n} segments; this importer only accepts a single-segment index")
            }
            ImportError::HasDeletions(n) => {
                write!(f, "segment has {n} deleted documents; this importer only accepts a deletion-free segment")
            }
        }
    }
}

impl std::error::Error for ImportError {}

impl From<TantivyError> for ImportError {
    fn from(e: TantivyError) -> Self {
        ImportError::Tantivy(e)
    }
}

impl From<std::io::Error> for ImportError {
    fn from(e: std::io::Error) -> Self {
        ImportError::Io(e)
    }
}

/// Imports `field_name` from the real tantivy index at `index_dir` into a
/// `FieldBlobs`, and returns it alongside the row count (`doc_lengths.len()`
/// as `u64`, the row-ID count a segment built from these blobs must use,
/// `spec/row-ids.md` §1). Positions are always imported
/// (`IndexRecordOption::WithFreqsAndPositions`) — a source field indexed
/// without them is a real, separate case this importer does not handle
/// (Non-goals in this module's own doc comment).
pub fn import_tantivy_field(index_dir: &Path, field_name: &str) -> Result<(FieldBlobs, u64), ImportError> {
    let index = Index::open_in_dir(index_dir)?;
    let schema = index.schema();
    let field = schema
        .get_field(field_name)
        .map_err(|_| ImportError::UnknownField(field_name.to_string()))?;

    let reader = index.reader()?;
    let searcher = reader.searcher();
    let segment_readers = searcher.segment_readers();
    if segment_readers.len() != 1 {
        return Err(ImportError::MultiSegment(segment_readers.len()));
    }
    let segment_reader = &segment_readers[0];
    if segment_reader.num_deleted_docs() > 0 {
        return Err(ImportError::HasDeletions(segment_reader.num_deleted_docs()));
    }

    let doc_count = segment_reader.max_doc();
    let mut doc_lengths = vec![0u32; doc_count as usize];
    let mut per_term: BTreeMap<Vec<u8>, Vec<(u32, Vec<u32>)>> = BTreeMap::new();

    let inverted_index = segment_reader.inverted_index(field)?;
    let term_dict = inverted_index.terms();
    let mut stream = term_dict.stream()?;
    let mut positions_buf: Vec<u32> = Vec::new();
    while let Some((term_bytes, term_info)) = stream.next() {
        let mut postings =
            inverted_index.read_postings_from_terminfo(term_info, IndexRecordOption::WithFreqsAndPositions)?;

        let mut doc_postings: Vec<(u32, Vec<u32>)> = Vec::new();
        let mut doc_id = postings.doc();
        while doc_id != TERMINATED {
            let term_freq = postings.term_freq();
            postings.positions(&mut positions_buf);
            doc_lengths[doc_id as usize] += term_freq;
            doc_postings.push((doc_id, positions_buf.clone()));
            doc_id = postings.advance();
        }
        // tantivy's own postings iterator already yields doc ids in
        // ascending order within a segment — build_postings's strictly-
        // increasing precondition holds by construction, not by a sort
        // this importer performs itself.
        per_term.insert(term_bytes.to_vec(), doc_postings);
    }

    let field_blobs = build_field_from_postings(per_term, doc_lengths, true);
    Ok((field_blobs, doc_count as u64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use strand_core::container::{Footer, Hotcache};
    use strand_core::segment::SegmentBuilder;
    use strand_lexical::field::FieldReader;
    use tantivy::schema::{Schema, TEXT};
    use tantivy::{doc, IndexWriter};

    fn open_segment_bytes(bytes: &[u8]) -> Hotcache {
        let footer_bytes: [u8; 40] = bytes[bytes.len() - 40..].try_into().unwrap();
        let footer = Footer::decode(&footer_bytes).expect("valid footer");
        let start = footer.hotcache_offset as usize;
        let end = start + footer.hotcache_length as usize;
        Hotcache::decode(&bytes[start..end]).expect("valid hotcache")
    }

    /// Builds a real, single-segment, deletion-free tantivy index on disk
    /// (tantivy's own `default` tokenizer: lowercase, whitespace/punctuation
    /// split, no stemming, no stopword removal — deliberately simple,
    /// already-lowercase, punctuation-free text avoids any ambiguity about
    /// what that tokenizer does to these specific documents) and returns
    /// its directory plus the field.
    fn build_real_tantivy_index() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().expect("create temp index dir");
        let mut schema_builder = Schema::builder();
        let title = schema_builder.add_text_field("title", TEXT);
        let schema = schema_builder.build();

        let index = tantivy::Index::create_in_dir(dir.path(), schema).expect("create tantivy index");
        // Single-threaded writer: this importer only accepts a
        // single-segment index (Non-goals, above), and tantivy's default
        // multi-threaded writer can split even a handful of documents
        // added before one commit across multiple segments.
        let mut writer: IndexWriter = index.writer_with_num_threads(1, 50_000_000).expect("open index writer");
        writer.add_document(doc!(title => "dog runs park")).unwrap();
        writer.add_document(doc!(title => "cat sleeps mat")).unwrap();
        writer.add_document(doc!(title => "dog cat park")).unwrap();
        writer.commit().expect("commit");

        (dir, "title".to_string())
    }

    #[test]
    fn imports_a_real_tantivy_index_and_answers_real_queries() {
        let (dir, field) = build_real_tantivy_index();

        let (field_blobs, row_count) =
            import_tantivy_field(dir.path(), &field).expect("import succeeds");
        assert_eq!(row_count, 3);
        // Each of the three documents is exactly 3 tokens ("dog runs
        // park", "cat sleeps mat", "dog cat park") — a direct check that
        // doc_lengths (accumulated as a side effect of the postings scan,
        // doc_lengths[doc_id] += term_freq per posting) is actually right,
        // not just plausible.
        assert_eq!(field_blobs.doc_lengths, vec![3, 3, 3]);

        let mut builder = SegmentBuilder::new(row_count);
        for blob in field_blobs.to_blob_specs() {
            builder.add_blob(blob);
        }
        let segment_bytes = builder.build(0);
        let hotcache = open_segment_bytes(&segment_bytes);

        let reader = FieldReader::open(&segment_bytes, &hotcache.blobs)
            .expect("all four blobs present (importer always keeps positions)");

        let mut dog_docs: Vec<u32> =
            reader.lookup("dog").expect("dog is a real term").into_iter().map(|(d, _)| d).collect();
        dog_docs.sort_unstable();
        assert_eq!(dog_docs, vec![0, 2]);

        let mut cat_docs: Vec<u32> =
            reader.lookup("cat").expect("cat is a real term").into_iter().map(|(d, _)| d).collect();
        cat_docs.sort_unstable();
        assert_eq!(cat_docs, vec![1, 2]);

        assert!(reader.lookup("giraffe").is_none());

        // Real phrase queries against imported positions: "dog cat" is
        // adjacent only in doc 2; "dog park" co-occurs in docs 0 and 2 but
        // is never adjacent in either — a true positional check, not
        // co-occurrence, exercised against real tantivy-sourced positions.
        assert_eq!(reader.phrase_query(&["dog", "cat"]), vec![2]);
        assert_eq!(reader.phrase_query(&["cat", "park"]), vec![2]);
        assert!(reader.phrase_query(&["dog", "park"]).is_empty());
    }

    #[test]
    fn rejects_an_unknown_field() {
        let (dir, _field) = build_real_tantivy_index();
        match import_tantivy_field(dir.path(), "no-such-field") {
            Err(ImportError::UnknownField(name)) => assert_eq!(name, "no-such-field"),
            Err(other) => panic!("expected ImportError::UnknownField, got {other}"),
            Ok(_) => panic!("expected an error, got Ok"),
        }
    }

    #[test]
    fn rejects_a_real_multi_segment_index() {
        let dir = tempfile::tempdir().expect("create temp index dir");
        let mut schema_builder = Schema::builder();
        let title = schema_builder.add_text_field("title", TEXT);
        let schema = schema_builder.build();
        let index = tantivy::Index::create_in_dir(dir.path(), schema).expect("create tantivy index");

        // NoMergePolicy: two separate commits below must land as two real,
        // distinct segments, deterministically — without this, tantivy's
        // default background merge policy could (nondeterministically,
        // depending on timing) merge them into one before this test reads
        // the index, silently making the test not exercise what it claims
        // to.
        let mut writer: IndexWriter = index.writer_with_num_threads(1, 50_000_000).expect("open index writer");
        writer.set_merge_policy(Box::new(tantivy::merge_policy::NoMergePolicy));
        writer.add_document(doc!(title => "dog runs park")).unwrap();
        writer.commit().expect("first commit");
        writer.add_document(doc!(title => "cat sleeps mat")).unwrap();
        writer.commit().expect("second commit");

        // Confirm the precondition this test actually depends on — two
        // real segments — rather than assuming NoMergePolicy did its job.
        let reader = index.reader().expect("open reader");
        assert_eq!(reader.searcher().segment_readers().len(), 2, "test setup must produce two real segments");

        match import_tantivy_field(dir.path(), "title") {
            Err(ImportError::MultiSegment(2)) => {}
            Err(other) => panic!("expected ImportError::MultiSegment(2), got {other}"),
            Ok(_) => panic!("expected an error, got Ok"),
        }
    }

    #[test]
    fn rejects_a_real_segment_with_deletions() {
        let dir = tempfile::tempdir().expect("create temp index dir");
        let mut schema_builder = Schema::builder();
        let title = schema_builder.add_text_field("title", TEXT);
        let schema = schema_builder.build();
        let index = tantivy::Index::create_in_dir(dir.path(), schema).expect("create tantivy index");

        let mut writer: IndexWriter = index.writer_with_num_threads(1, 50_000_000).expect("open index writer");
        writer.add_document(doc!(title => "dog runs park")).unwrap();
        writer.add_document(doc!(title => "cat sleeps mat")).unwrap();
        writer.commit().expect("commit");
        writer.delete_term(tantivy::Term::from_field_text(title, "cat"));
        writer.commit().expect("commit the deletion");

        // Confirm the precondition this test actually depends on — a real
        // deleted document in a real, still-single segment.
        let reader = index.reader().expect("open reader");
        let segment_readers = reader.searcher().segment_readers().to_vec();
        assert_eq!(segment_readers.len(), 1, "test setup must stay single-segment");
        assert_eq!(segment_readers[0].num_deleted_docs(), 1, "test setup must produce one real deletion");

        match import_tantivy_field(dir.path(), "title") {
            Err(ImportError::HasDeletions(1)) => {}
            Err(other) => panic!("expected ImportError::HasDeletions(1), got {other}"),
            Ok(_) => panic!("expected an error, got Ok"),
        }
    }
}
