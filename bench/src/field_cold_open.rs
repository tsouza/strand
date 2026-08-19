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

//! `bench/src/cold_open.rs` proves invariant 3's GET-count bound (pointer +
//! snapshot + segment open, ≤2 RTT) against a segment containing literal
//! placeholder bytes — real for the container/manifest mechanics, but it
//! has never proven the bound holds for a segment with real, queryable
//! lexical content, and nothing has measured how long it takes to go from
//! "cold" to "first real query result" against real network-backed object
//! storage. This benchmark closes that gap: a real field (RFC 0005/0007/
//! 0008's term-dictionary/postings/positions blobs, built from real MS
//! MARCO passages via `strand_lexical::field::build_field`) is committed to
//! real MinIO, then repeatedly opened cold and queried — a real BM25 search
//! and a real phrase query — measuring both the GET count (must still be
//! bounded, invariant 3's one-wave rule: the query itself costs zero
//! additional round trips once the segment's bytes are resident) and the
//! full wall-clock cost from cold to answered query.

use flate2::read::MultiGzDecoder;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use strand_bench::{CountingStore, percentile, store_for, timed, with_minio};
use strand_core::container::{Footer, Hotcache};
use strand_core::manifest::{commit, read_snapshot};
use strand_core::scoring::Bm25Profile;
use strand_core::segment::{SegmentBuilder, write_segment};
use strand_core::store::ConditionalStore;
use strand_lexical::field::{FieldReader, build_field};

const ITERATIONS: usize = 30;

#[derive(Deserialize)]
struct CorpusLine {
    text: String,
}

#[derive(Serialize)]
struct FieldColdOpenResult {
    iterations: usize,
    doc_count: usize,
    vocabulary_size: usize,
    segment_bytes: u64,
    /// GETs for open alone (pointer + snapshot + segment), before any query
    /// runs — must match `docs/milestones.md` M0's bound.
    get_count_per_open: u64,
    open_latency_ms_p50: f64,
    open_latency_ms_p90: f64,
    open_latency_ms_p99: f64,
    /// GETs for open *and* running one BM25 query plus one phrase query —
    /// must equal `get_count_per_open`, proving invariant 3's one-wave
    /// rule holds for a real query, not just the open itself.
    get_count_open_and_query: u64,
    open_and_query_latency_ms_p50: f64,
    open_and_query_latency_ms_p90: f64,
    open_and_query_latency_ms_p99: f64,
}

fn load_docs(sample_target: u64) -> Vec<String> {
    let data_path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/corpus.jsonl.gz");
    let file = std::fs::File::open(data_path).unwrap_or_else(|e| {
        panic!("open {data_path}: {e} — run the download step first (see docs/ledger.md R9)")
    });
    let decoder = MultiGzDecoder::new(file);
    let reader = BufReader::with_capacity(1 << 20, decoder);

    const CORPUS_TOTAL_PASSAGES: u64 = 8_841_823;
    let stride = (CORPUS_TOTAL_PASSAGES / sample_target).max(1);

    let mut docs = Vec::new();
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
            docs.push(parsed.text);
        }
    }
    docs
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
    let sample_target: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(5_000);

    eprintln!("Loading real MS MARCO passages (target {sample_target})...");
    let docs = load_docs(sample_target);
    let doc_refs: Vec<&str> = docs.iter().map(String::as_str).collect();
    eprintln!("Loaded {} real passages; building field...", doc_refs.len());

    let field = build_field(&doc_refs);
    let vocabulary_size =
        field.term_info.len() / strand_lexical::term_dictionary::TERM_INFO_RECORD_LEN;
    eprintln!(
        "Field built: {} docs, {vocabulary_size} terms, {} bytes across 4 blobs",
        doc_refs.len(),
        field.term_dict.len()
            + field.term_info.len()
            + field.postings.len()
            + field.positions.len()
    );

    let doc_lengths = field.doc_lengths.clone();
    let profile = Bm25Profile::default();

    with_minio(|endpoint, bucket| {
        let store = store_for(endpoint, bucket);

        let mut builder = SegmentBuilder::new(doc_refs.len() as u64);
        for blob in field.to_blob_specs() {
            builder.add_blob(blob);
        }

        let snapshot = commit(&store, |row_id_base| {
            vec![write_segment(
                &store,
                "segments/field-cold-open.bin",
                &builder,
                row_id_base,
            )]
        })
        .expect("commit succeeds against an empty table");
        let segment_bytes_len = snapshot.segments[0].byte_length;
        eprintln!("Committed real segment: {segment_bytes_len} bytes on real MinIO");

        let counting = CountingStore::new(&store);

        // Phase 1: open alone (pointer -> snapshot -> segment -> footer ->
        // hotcache), no query — the same measurement bench/src/cold_open.rs
        // makes, now against a segment with real, queryable content.
        let mut open_latencies = Vec::with_capacity(ITERATIONS);
        let mut open_get_counts = Vec::with_capacity(ITERATIONS);
        for _ in 0..ITERATIONS {
            counting.reset();
            let (_, elapsed_ms) = timed(|| {
                let snapshot = read_snapshot(&counting).unwrap().unwrap();
                let segment_ref = &snapshot.segments[0];
                let (segment_bytes, _) = counting.get(&segment_ref.path).unwrap().unwrap();
                open_segment_bytes(&segment_bytes)
            });
            open_latencies.push(elapsed_ms);
            open_get_counts.push(counting.get_count());
        }
        open_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let get_count_per_open = open_get_counts[0];
        assert!(
            open_get_counts.iter().all(|&c| c == get_count_per_open),
            "GET count per cold open must be constant across iterations: {open_get_counts:?}"
        );
        assert!(
            get_count_per_open <= 4,
            "pointer + snapshot + segment open (\u{2264}2 RTT per invariant 3) should be \u{2264}4 GETs, got {get_count_per_open}"
        );

        // Phase 2: open, then run one real BM25 query and one real phrase
        // query — must cost the *same* GET count as open alone, proving
        // invariant 3's one-wave rule for a real query path, not just the
        // open.
        let mut full_latencies = Vec::with_capacity(ITERATIONS);
        let mut full_get_counts = Vec::with_capacity(ITERATIONS);
        for _ in 0..ITERATIONS {
            counting.reset();
            let (_, elapsed_ms) = timed(|| {
                let snapshot = read_snapshot(&counting).unwrap().unwrap();
                let segment_ref = &snapshot.segments[0];
                let (segment_bytes, _) = counting.get(&segment_ref.path).unwrap().unwrap();
                let hotcache = open_segment_bytes(&segment_bytes);
                let reader = FieldReader::open(&segment_bytes, &hotcache.blobs)
                    .expect("all four blobs present");

                let bm25 = reader.search_bm25("state", &doc_lengths, &profile);
                let phrase = reader.phrase_query(&["united", "states"]);
                (bm25, phrase)
            });
            full_latencies.push(elapsed_ms);
            full_get_counts.push(counting.get_count());
        }
        full_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let get_count_open_and_query = full_get_counts[0];
        assert!(
            full_get_counts
                .iter()
                .all(|&c| c == get_count_open_and_query),
            "GET count per open+query must be constant across iterations: {full_get_counts:?}"
        );
        assert_eq!(
            get_count_open_and_query, get_count_per_open,
            "running a real query after open must cost zero additional GETs (invariant 3's one-wave rule) — \
             open alone cost {get_count_per_open}, open+query cost {get_count_open_and_query}"
        );

        let result = FieldColdOpenResult {
            iterations: ITERATIONS,
            doc_count: doc_refs.len(),
            vocabulary_size,
            segment_bytes: segment_bytes_len,
            get_count_per_open,
            open_latency_ms_p50: percentile(&open_latencies, 0.50),
            open_latency_ms_p90: percentile(&open_latencies, 0.90),
            open_latency_ms_p99: percentile(&open_latencies, 0.99),
            get_count_open_and_query,
            open_and_query_latency_ms_p50: percentile(&full_latencies, 0.50),
            open_and_query_latency_ms_p90: percentile(&full_latencies, 0.90),
            open_and_query_latency_ms_p99: percentile(&full_latencies, 0.99),
        };

        println!(
            "field cold open: {} docs, {} bytes, {get_count_per_open} GETs/open (open: p50={:.2}ms p90={:.2}ms), \
             {get_count_open_and_query} GETs/open+query (p50={:.2}ms p90={:.2}ms) (n={ITERATIONS})",
            result.doc_count,
            result.segment_bytes,
            result.open_latency_ms_p50,
            result.open_latency_ms_p90,
            result.open_and_query_latency_ms_p50,
            result.open_and_query_latency_ms_p90,
        );

        // Named by real doc count — a fixed filename would silently
        // overwrite a prior run's numbers on the next differently-scaled
        // run (the exact bug bench/src/tantivy_index.rs hit and fixed
        // earlier this session).
        strand_bench::write_report(&format!("field-cold-open-{}", result.doc_count), result);
    });
}
