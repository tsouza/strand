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

//! Real end to end: real document text, through the analyzer, into real
//! term-dictionary/term-info/postings blobs, assembled into a real segment,
//! written and committed through `strand-core`'s real store and manifest
//! layers (the same pattern `crates/strand-lexical/tests/
//! field_end_to_end.rs` already established) — then opened as a
//! `StrandLexicalTable` and queried with Apache DataFusion's own SQL
//! engine, a real `SessionContext`, not a mocked one. Every assertion here
//! is checked against a row set computed by hand from `DOCS` below, not
//! against whatever the code happens to produce.

use std::sync::Arc;

use datafusion::arrow::array::{StringArray, UInt32Array, UInt64Array};
use datafusion::prelude::SessionContext;
use strand_core::container::{Footer, Hotcache};
use strand_core::manifest::{commit, read_snapshot};
use strand_core::segment::{SegmentBuilder, write_segment};
use strand_core::store::{ConditionalStore, InMemoryStore};
use strand_datafusion::StrandLexicalTable;
use strand_lexical::field::build_field;

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

/// Builds one real segment (field "body", the same three documents
/// `field_end_to_end.rs` uses) and commits it through the real manifest,
/// returning its bytes and the row-ID base the commit assigned.
fn build_and_commit_segment() -> (Vec<u8>, u64) {
    let field = build_field("body", &DOCS);
    let mut builder = SegmentBuilder::new(DOCS.len() as u64);
    for blob in field.to_blob_specs() {
        builder.add_blob(blob);
    }

    let store = InMemoryStore::new();
    commit(&store, |row_id_base| {
        vec![write_segment(
            &store,
            "segments/body-0.bin",
            &builder,
            row_id_base,
        )]
    })
    .expect("commit succeeds against an empty table");

    let snapshot = read_snapshot(&store)
        .expect("read succeeds")
        .expect("a snapshot exists");
    let segment_ref = &snapshot.segments[0];
    let (segment_bytes, _) = ConditionalStore::get(&store, &segment_ref.path)
        .expect("get succeeds")
        .expect("segment exists");

    let row_id_base = open_segment_bytes(&segment_bytes).row_id_base;
    assert_eq!(row_id_base, segment_ref.row_id_base);
    (segment_bytes, row_id_base)
}

#[tokio::test]
async fn a_real_sql_query_runs_through_datafusion_against_a_real_strand_segment() {
    let (segment_bytes, row_id_base) = build_and_commit_segment();

    let table =
        StrandLexicalTable::try_new(&segment_bytes, "body").expect("body field is present");

    let ctx = SessionContext::new();
    ctx.register_table("body", Arc::new(table))
        .expect("registers a real TableProvider");

    // "park" occurs once in doc 0 ("the dog runs in the park") and once in
    // doc 2 ("the dog and the cat play in the park"), never in doc 1 --
    // computed by hand from DOCS, not from the code under test.
    let df = ctx
        .sql("SELECT row_id, term_freq FROM body WHERE term = 'park' ORDER BY row_id")
        .await
        .expect("a real SQL query plans against a real TableProvider schema");
    let batches = df.collect().await.expect("a real scan executes");

    let mut rows: Vec<(u64, u32)> = Vec::new();
    for batch in &batches {
        let row_ids = batch
            .column(0)
            .as_any()
            .downcast_ref::<UInt64Array>()
            .unwrap();
        let freqs = batch
            .column(1)
            .as_any()
            .downcast_ref::<UInt32Array>()
            .unwrap();
        for i in 0..batch.num_rows() {
            rows.push((row_ids.value(i), freqs.value(i)));
        }
    }

    assert_eq!(
        rows,
        vec![(row_id_base, 1), (row_id_base + 2, 1)],
        "\"park\" occurs once in doc 0 and once in doc 2, and row_id must be \
         row_id_base + doc_ordinal (spec/row-ids.md \u{a7}1), not the bare doc_ordinal"
    );

    // A genuine miss: "giraffe" never occurs anywhere in DOCS, so the query
    // must return zero rows through the real engine, not an error.
    let df = ctx
        .sql("SELECT * FROM body WHERE term = 'giraffe'")
        .await
        .unwrap();
    let batches = df.collect().await.unwrap();
    let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
    assert_eq!(total_rows, 0);
}

#[tokio::test]
async fn a_real_sql_aggregate_reproduces_hand_computed_document_frequencies() {
    let (segment_bytes, _row_id_base) = build_and_commit_segment();
    let table =
        StrandLexicalTable::try_new(&segment_bytes, "body").expect("body field is present");

    let ctx = SessionContext::new();
    ctx.register_table("body", Arc::new(table)).unwrap();

    // Real GROUP BY / COUNT / ORDER BY through DataFusion's own optimizer
    // and aggregate operators -- not a raw scan. "the" is a stopword the
    // analyzer drops (matching field_end_to_end.rs's own tokenization,
    // strand_lexical::analyzer::analyze_lucene_en_word_only), so it must
    // not appear at all; "dog" and "cat" each occur in exactly two of the
    // three documents.
    let df = ctx
        .sql(
            "SELECT term, COUNT(*) AS doc_freq FROM body \
             WHERE term IN ('dog', 'cat', 'mat') \
             GROUP BY term ORDER BY term",
        )
        .await
        .unwrap();
    let batches = df.collect().await.unwrap();

    let mut freqs: Vec<(String, i64)> = Vec::new();
    for batch in &batches {
        let terms = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let counts = batch
            .column(1)
            .as_any()
            .downcast_ref::<datafusion::arrow::array::Int64Array>()
            .unwrap();
        for i in 0..batch.num_rows() {
            freqs.push((terms.value(i).to_string(), counts.value(i)));
        }
    }

    assert_eq!(
        freqs,
        vec![
            ("cat".to_string(), 2),
            ("dog".to_string(), 2),
            ("mat".to_string(), 1),
        ],
        "real DataFusion aggregation must match hand-computed document \
         frequencies over DOCS"
    );
}

#[tokio::test]
async fn opening_a_field_name_never_built_into_the_segment_is_a_clean_error() {
    let (segment_bytes, _row_id_base) = build_and_commit_segment();
    let err = StrandLexicalTable::try_new(&segment_bytes, "nonexistent_field")
        .expect_err("a field that was never built must not silently resolve to something else");
    assert!(matches!(
        err,
        strand_datafusion::LexicalTableError::Field(_)
    ));
}
