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

//! Runs `crates/strand-lexical/tests/field_end_to_end.rs`'s same real
//! pipeline (analyze -> build blobs -> segment -> commit -> read back cold
//! -> query) over a real, much larger MS MARCO sample instead of that
//! test's 3-document toy corpus — a stress test of the segment-lexical
//! integration layer at real scale, not just a bigger synthetic example.

use flate2::read::MultiGzDecoder;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::time::Instant;
use strand_core::container::{Footer, Hotcache};
use strand_core::manifest::{commit, read_snapshot};
use strand_core::scoring::Bm25Profile;
use strand_core::segment::{SegmentBuilder, write_segment};
use strand_core::store::{ConditionalStore, InMemoryStore};
use strand_lexical::field::{FieldReader, build_field, build_field_without_positions};

#[derive(Deserialize)]
struct CorpusLine {
    text: String,
}

#[derive(Serialize)]
struct FieldEndToEndResult {
    doc_count: usize,
    vocabulary_size: usize,
    term_dict_bytes: usize,
    term_info_bytes: usize,
    postings_bytes: usize,
    positions_bytes: usize,
    /// `TermInfo`'s short, positions-free record (RFC 0009 Design §2),
    /// same real vocabulary — no positions blob at all in that case.
    term_info_bytes_without_positions: usize,
    segment_bytes: u64,
    build_field_seconds: f64,
    commit_seconds: f64,
    cold_open_ms: f64,
}

fn open_segment_bytes(bytes: &[u8]) -> Hotcache {
    let footer_bytes: [u8; 40] = bytes[bytes.len() - 40..].try_into().unwrap();
    let footer = Footer::decode(&footer_bytes).expect("valid footer");
    let start = footer.hotcache_offset as usize;
    let end = start + footer.hotcache_length as usize;
    Hotcache::decode(&bytes[start..end]).expect("valid hotcache")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let sample_target: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(10_000);

    let data_path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/corpus.jsonl.gz");
    eprintln!("Reading real passages from {data_path}");

    let file = std::fs::File::open(data_path).unwrap_or_else(|e| {
        panic!("open {data_path}: {e} — run the download step first (see docs/ledger.md R9)")
    });
    let decoder = MultiGzDecoder::new(file);
    let reader = BufReader::with_capacity(1 << 20, decoder);

    // Identical stride formula to bench/src/msmarco_index.rs and
    // bench/src/tantivy_index.rs, so all three tools sample the exact same
    // document set at a given target count — a same-corpus comparison, not
    // a same-count-different-documents one.
    const CORPUS_TOTAL_PASSAGES: u64 = 8_841_823;
    let stride = (CORPUS_TOTAL_PASSAGES / sample_target).max(1);
    eprintln!(
        "Sampling every {stride}-th passage (target {sample_target}, corpus {CORPUS_TOTAL_PASSAGES})"
    );

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
        let parsed: CorpusLine = match serde_json::from_str(&line) {
            Ok(p) => p,
            Err(_) => continue,
        };
        docs.push(parsed.text);
    }
    eprintln!("Loaded {} real passages", docs.len());

    let doc_refs: Vec<&str> = docs.iter().map(String::as_str).collect();

    let build_start = Instant::now();
    let field = build_field(&doc_refs);
    let build_elapsed = build_start.elapsed();
    eprintln!(
        "build_field: {:.2}s — term_dict {} bytes, term_info {} bytes, postings {} bytes, positions {} bytes",
        build_elapsed.as_secs_f64(),
        field.term_dict.len(),
        field.term_info.len(),
        field.postings.len(),
        field.positions.len()
    );

    // RFC 0009 Design §2's short term-info record, on the same real
    // corpus — a real, committed number for the fix's payoff, previously
    // unexercised (docs/ledger.md's RFC 0009 entry).
    let field_no_positions = build_field_without_positions(&doc_refs);
    let term_info_savings =
        field.term_info.len() as i64 - field_no_positions.term_info.len() as i64;
    eprintln!(
        "build_field_without_positions: term_info {} bytes (vs. {} bytes with positions, {} bytes saved), \
         no positions blob at all ({} bytes not written)",
        field_no_positions.term_info.len(),
        field.term_info.len(),
        term_info_savings,
        field.positions.len()
    );

    let mut builder = SegmentBuilder::new(doc_refs.len() as u64);
    for blob in field.to_blob_specs() {
        builder.add_blob(blob);
    }

    let store = InMemoryStore::new();
    let commit_start = Instant::now();
    let snapshot = commit(&store, |row_id_base| {
        vec![write_segment(
            &store,
            "segments/field-large.bin",
            &builder,
            row_id_base,
        )]
    })
    .expect("commit succeeds against an empty table");
    let commit_elapsed = commit_start.elapsed();
    eprintln!(
        "commit: {:.2}s — segment {} bytes, {} rows",
        commit_elapsed.as_secs_f64(),
        snapshot.segments[0].byte_length,
        snapshot.next_row_id
    );

    let open_start = Instant::now();
    let read_back = read_snapshot(&store)
        .expect("read succeeds")
        .expect("a snapshot exists");
    let segment_ref = &read_back.segments[0];
    let (segment_bytes, _) = ConditionalStore::get(&store, &segment_ref.path)
        .expect("get succeeds")
        .expect("segment exists");
    let hotcache = open_segment_bytes(&segment_bytes);
    let reader =
        FieldReader::open(&segment_bytes, &hotcache.blobs).expect("all four blobs present");
    let open_elapsed = open_start.elapsed();
    eprintln!(
        "cold open (pointer -> snapshot -> segment -> hotcache -> FieldReader): {:.2}ms",
        open_elapsed.as_secs_f64() * 1000.0
    );

    let profile = Bm25Profile::default();
    // FieldReader::lookup does raw-string FST lookup — documents are
    // indexed *stemmed* (e.g. "energy" -> "energi"), so a realistic query
    // path must run the query term through the same analyzer before
    // looking it up, exactly like a document does at index time.
    let sample_terms = [
        "state", "system", "water", "time", "process", "law", "energy", "health",
    ];
    for raw_term in sample_terms {
        let stemmed = strand_lexical::analyzer::analyze_lucene_en_word_only(raw_term);
        let Some(term) = stemmed.first() else {
            eprintln!("  query {raw_term:>8?}: analyzer dropped it (stopword or empty)");
            continue;
        };
        let query_start = Instant::now();
        let ranked = reader.search_bm25(term, &field.doc_lengths, &profile);
        let query_elapsed = query_start.elapsed();
        match ranked {
            Some(results) => {
                let top = results.first().copied();
                eprintln!(
                    "  query {raw_term:>8?} (-> {term:?}): {} matches in {:.1}us, top = {top:?}",
                    results.len(),
                    query_elapsed.as_secs_f64() * 1e6
                );
            }
            None => eprintln!("  query {raw_term:>8?} (-> {term:?}): no matches"),
        }
    }

    // Real phrase queries: sample real adjacent stemmed-token pairs from
    // the loaded documents themselves (same approach
    // bench/src/tantivy_index.rs uses for its own phrase-query benchmark),
    // so every query is guaranteed to have at least one real match.
    let mut phrase_pairs: Vec<(String, String)> = Vec::new();
    for text in &docs {
        if phrase_pairs.len() >= 500 {
            break;
        }
        let tokens = strand_lexical::analyzer::analyze_lucene_en_word_only(text);
        if tokens.len() >= 2 {
            phrase_pairs.push((tokens[0].clone(), tokens[1].clone()));
        }
    }
    let mut phrase_latencies_us = Vec::with_capacity(phrase_pairs.len());
    let mut phrase_matches_total = 0usize;
    for (a, b) in &phrase_pairs {
        let query_start = Instant::now();
        let matches = reader.phrase_query(&[a.as_str(), b.as_str()]);
        phrase_latencies_us.push(query_start.elapsed().as_secs_f64() * 1e6);
        phrase_matches_total += matches.len();
    }
    if !phrase_latencies_us.is_empty() {
        let mean = phrase_latencies_us.iter().sum::<f64>() / phrase_latencies_us.len() as f64;
        eprintln!(
            "phrase queries: {} queries, {phrase_matches_total} total matches, mean {mean:.1}us",
            phrase_latencies_us.len()
        );
    }

    let vocabulary_size =
        field.term_info.len() / strand_lexical::term_dictionary::TERM_INFO_RECORD_LEN;
    eprintln!(
        "Done: {} real documents, {vocabulary_size} distinct terms, real end-to-end query path validated at scale.",
        doc_refs.len(),
    );

    let result = FieldEndToEndResult {
        doc_count: doc_refs.len(),
        vocabulary_size,
        term_dict_bytes: field.term_dict.len(),
        term_info_bytes: field.term_info.len(),
        postings_bytes: field.postings.len(),
        positions_bytes: field.positions.len(),
        term_info_bytes_without_positions: field_no_positions.term_info.len(),
        segment_bytes: snapshot.segments[0].byte_length,
        build_field_seconds: build_elapsed.as_secs_f64(),
        commit_seconds: commit_elapsed.as_secs_f64(),
        cold_open_ms: open_elapsed.as_secs_f64() * 1000.0,
    };
    // Named by real doc count, matching bench/src/tantivy_index.rs's own
    // fix for the same clobbering risk: a fixed filename would silently
    // overwrite a prior run's committed numbers on the next differently-
    // scaled run.
    let out_path = format!(
        "{}/results/field-end-to-end-{}.json",
        env!("CARGO_MANIFEST_DIR"),
        result.doc_count
    );
    std::fs::write(&out_path, serde_json::to_string_pretty(&result).unwrap())
        .unwrap_or_else(|e| panic!("write {out_path}: {e}"));
    eprintln!("Wrote {out_path}");
}
