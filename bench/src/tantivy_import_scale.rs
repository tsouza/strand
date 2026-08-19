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

//! Real-scale verification of `strand-tools convert`'s tantivy importer
//! (`crates/strand-tools/src/convert.rs`), prompted directly by "test at
//! real scale with the MS MARCO tantivy index" — the crate's own unit
//! tests only exercise a 3-document, hand-written corpus.
//!
//! Strategy: build the *same* real MS MARCO sample two independent ways —
//! (1) natively, `strand_lexical::field::build_field` straight from text,
//! the same path `bench/src/field_end_to_end.rs` already validated at
//! scale; (2) via a real tantivy index (the same `PreTokenizedString`
//! trick `bench/src/tantivy_index.rs` uses, so both paths tokenize
//! identically — single-threaded writer, so the output is the
//! single-segment, deletion-free index the importer requires), then
//! `strand_tools::convert::import_tantivy_field` on it — and compare the
//! two resulting `FieldBlobs` byte-for-byte. Both paths feed documents in
//! the same stride-sampled order and tantivy's own single-threaded `DocId`
//! assignment is append-order, so if the importer is correct the two
//! should produce identical bytes, not just similar-looking ones.

use flate2::read::MultiGzDecoder;
use serde::Deserialize;
use std::io::{BufRead, BufReader};
use std::time::Instant;
use strand_lexical::field::build_field;
use tantivy::schema::{IndexRecordOption, Schema, TextFieldIndexing, TextOptions};
use tantivy::tokenizer::{PreTokenizedString, Token};
use tantivy::{doc, Index, IndexWriter};

#[derive(Deserialize)]
struct CorpusLine {
    text: String,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let sample_target: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(10_000);

    let data_path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/corpus.jsonl.gz");
    eprintln!("Reading {data_path}");
    let file = std::fs::File::open(data_path).unwrap_or_else(|e| {
        panic!("open {data_path}: {e} — run the download step first (see docs/ledger.md R9)")
    });
    let decoder = MultiGzDecoder::new(file);
    let reader = BufReader::with_capacity(1 << 20, decoder);

    const CORPUS_TOTAL_PASSAGES: u64 = 8_841_823;
    let stride = (CORPUS_TOTAL_PASSAGES / sample_target).max(1);
    eprintln!("Sampling every {stride}-th passage (target {sample_target}, corpus {CORPUS_TOTAL_PASSAGES})");

    let mut docs: Vec<String> = Vec::new();
    for (line_no, line) in reader.lines().enumerate() {
        let line_no = line_no as u64;
        if !line_no.is_multiple_of(stride) {
            continue;
        }
        let line = line.expect("read line");
        if line.is_empty() {
            continue;
        }
        if let Ok(parsed) = serde_json::from_str::<CorpusLine>(&line) {
            // Skip documents the analyzer reduces to zero tokens — an
            // empty posting list is not representable (`build_postings`
            // panics on an empty list), and native `build_field` simply
            // never creates an entry for a term such a document never
            // contributes, so it is never a problem there; a
            // *tantivy-side* empty document is harmless too (it just adds
            // no postings), but excluding it from both paths keeps the
            // byte-for-byte comparison clean without touching either real
            // code path's own handling of the case.
            if !strand_lexical::analyzer::analyze_lucene_en_word_only(&parsed.text).is_empty() {
                docs.push(parsed.text);
            }
        }
    }
    eprintln!("Loaded {} real, non-empty-after-analysis passages", docs.len());
    let doc_refs: Vec<&str> = docs.iter().map(String::as_str).collect();

    // Path 1: native.
    let native_start = Instant::now();
    let native = build_field(&doc_refs);
    eprintln!(
        "native build_field: {:.2}s — term_dict {} bytes, term_info {} bytes, postings {} bytes, positions {} bytes",
        native_start.elapsed().as_secs_f64(),
        native.term_dict.len(),
        native.term_info.len(),
        native.postings.len(),
        native.positions.len(),
    );

    // Path 2: real tantivy index, then real import.
    let index_dir = tempfile::tempdir().expect("create temp index dir");
    let mut schema_builder = Schema::builder();
    let body_indexing =
        TextFieldIndexing::default().set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let body = schema_builder.add_text_field("body", TextOptions::default().set_indexing_options(body_indexing));
    let schema = schema_builder.build();
    let index = Index::create_in_dir(index_dir.path(), schema).expect("create tantivy index");
    // Single-threaded: the importer only accepts a single-segment index
    // (crates/strand-tools/src/convert.rs's own stated Non-goals), and
    // this also guarantees DocId assignment follows add_document order
    // exactly, matching native build_field's doc_ordinal = array-index
    // convention — required for the byte-for-byte comparison below to be
    // meaningful.
    let mut writer: IndexWriter = index.writer_with_num_threads(1, 200_000_000).expect("open index writer");

    let tantivy_build_start = Instant::now();
    for text in &docs {
        let words = strand_lexical::analyzer::analyze_lucene_en_word_only(text);
        let mut tokens = Vec::with_capacity(words.len());
        let mut offset = 0usize;
        for (position, word) in words.iter().enumerate() {
            let offset_from = offset;
            let offset_to = offset_from + word.len();
            tokens.push(Token { offset_from, offset_to, position, text: word.clone(), position_length: 1 });
            offset = offset_to + 1;
        }
        let pretok = PreTokenizedString { text: words.join(" "), tokens };
        writer.add_document(doc!(body => pretok)).expect("add_document");
    }
    writer.commit().expect("commit");
    let tantivy_build_seconds = tantivy_build_start.elapsed().as_secs_f64();
    eprintln!("real tantivy index built and committed in {tantivy_build_seconds:.2}s");

    let import_start = Instant::now();
    let (imported, row_count) = strand_tools::convert::import_tantivy_field(index_dir.path(), "body")
        .expect("import succeeds");
    eprintln!(
        "strand_tools::convert::import_tantivy_field: {:.2}s — term_dict {} bytes, term_info {} bytes, postings {} bytes, positions {} bytes",
        import_start.elapsed().as_secs_f64(),
        imported.term_dict.len(),
        imported.term_info.len(),
        imported.postings.len(),
        imported.positions.len(),
    );
    assert_eq!(row_count, doc_refs.len() as u64, "imported row count must match the real document count");

    // The real comparison: byte-for-byte, not "close enough."
    let mut mismatches: Vec<String> = Vec::new();
    if native.term_dict != imported.term_dict {
        mismatches.push(format!(
            "term_dict differs: native {} bytes vs imported {} bytes",
            native.term_dict.len(),
            imported.term_dict.len()
        ));
    }
    if native.term_info != imported.term_info {
        mismatches.push(format!(
            "term_info differs: native {} bytes vs imported {} bytes",
            native.term_info.len(),
            imported.term_info.len()
        ));
    }
    if native.postings != imported.postings {
        mismatches.push(format!(
            "postings differs: native {} bytes vs imported {} bytes",
            native.postings.len(),
            imported.postings.len()
        ));
    }
    if native.positions != imported.positions {
        mismatches.push(format!(
            "positions differs: native {} bytes vs imported {} bytes",
            native.positions.len(),
            imported.positions.len()
        ));
    }
    if native.doc_lengths != imported.doc_lengths {
        mismatches.push(format!(
            "doc_lengths differs: native {:?}.. vs imported {:?}..",
            &native.doc_lengths[..native.doc_lengths.len().min(5)],
            &imported.doc_lengths[..imported.doc_lengths.len().min(5)]
        ));
    }

    if mismatches.is_empty() {
        println!(
            "MATCH: native build_field and the real tantivy-import path produced byte-identical \
             FieldBlobs on {} real documents ({} bytes total: {} term_dict + {} term_info + {} postings + {} positions).",
            doc_refs.len(),
            native.term_dict.len() + native.term_info.len() + native.postings.len() + native.positions.len(),
            native.term_dict.len(),
            native.term_info.len(),
            native.postings.len(),
            native.positions.len(),
        );
    } else {
        println!("MISMATCH on {} real documents:", doc_refs.len());
        for m in &mismatches {
            println!("  {m}");
        }
        std::process::exit(1);
    }
}
