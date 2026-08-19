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

//! Builds a real tantivy index over the identical stride-sampled MS MARCO
//! subset `bench/src/msmarco_index.rs` uses (`bench/data/corpus.jsonl.gz`,
//! same stride, same passage count), so its real on-disk size and real
//! query latency can be compared directly against RFC 0007's (postings) and
//! RFC 0008's (positions) own measured/estimated numbers on the same
//! corpus — the "battle-tested software" comparison `docs/milestones.md`'s
//! M1 entry names as a required benchmark, not yet run before this file.
//!
//! Fairness note: every document is fed to tantivy as a `PreTokenizedString`
//! built directly from `strand_lexical::analyzer::analyze_lucene_en_word_only`'s
//! own output — the identical token stream (same vocabulary, same doc
//! frequencies, same term frequencies, same position counts) STRAND's own
//! postings/positions blobs would index for the same corpus. This isolates
//! the comparison to on-disk *format* efficiency and query latency, not
//! analyzer differences: tantivy's own tokenizer/stopword/stemmer chain is
//! never invoked here.

use flate2::read::MultiGzDecoder;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::time::Instant;
use tantivy::collector::TopDocs;
use tantivy::query::{PhraseQuery, TermQuery};
use tantivy::schema::{IndexRecordOption, Schema, TextFieldIndexing, TextOptions};
use tantivy::tokenizer::{PreTokenizedString, Token};
use tantivy::{Index, IndexWriter, Term, doc};

#[derive(Deserialize)]
struct CorpusLine {
    text: String,
}

#[derive(Serialize)]
struct LatencyStats {
    count: usize,
    mean_ns: f64,
    p50_ns: u64,
    p90_ns: u64,
    p99_ns: u64,
    max_ns: u64,
}

fn latency_stats(mut samples: Vec<u64>) -> LatencyStats {
    samples.sort_unstable();
    let count = samples.len();
    let percentile = |p: f64| -> u64 {
        if count == 0 {
            return 0;
        }
        let idx = ((count as f64 - 1.0) * p).round() as usize;
        samples[idx.min(count - 1)]
    };
    let mean_ns = if count == 0 {
        0.0
    } else {
        samples.iter().sum::<u64>() as f64 / count as f64
    };
    LatencyStats {
        count,
        mean_ns,
        p50_ns: percentile(0.50),
        p90_ns: percentile(0.90),
        p99_ns: percentile(0.99),
        max_ns: samples.last().copied().unwrap_or(0),
    }
}

#[derive(Serialize)]
struct TantivyBenchResult {
    source: String,
    corpus_total_passages: u64,
    sample_stride: u64,
    sampled_passages: u64,
    build_wall_seconds: f64,
    index_total_bytes: u64,
    bytes_by_extension: BTreeMap<String, u64>,
    term_query_latency: LatencyStats,
    phrase_query_latency: LatencyStats,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let sample_target: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(500_000);

    let data_path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/corpus.jsonl.gz");
    eprintln!("Reading {data_path}");

    let file = std::fs::File::open(data_path).unwrap_or_else(|e| {
        panic!("open {data_path}: {e} — run the download step first (see docs/ledger.md R9)")
    });
    let decoder = MultiGzDecoder::new(file);
    let reader = BufReader::with_capacity(1 << 20, decoder);

    // Identical constant and stride formula to bench/src/msmarco_index.rs,
    // so this run samples the exact same passages.
    const CORPUS_TOTAL_PASSAGES: u64 = 8_841_823;
    let stride = (CORPUS_TOTAL_PASSAGES / sample_target).max(1);
    eprintln!(
        "Sampling every {stride}-th passage (target {sample_target}, corpus {CORPUS_TOTAL_PASSAGES})"
    );

    let index_dir = tempfile::tempdir().expect("create temp index dir");

    let mut schema_builder = Schema::builder();
    let body_indexing =
        TextFieldIndexing::default().set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let body = schema_builder.add_text_field(
        "body",
        TextOptions::default().set_indexing_options(body_indexing),
    );
    let schema = schema_builder.build();

    let index = Index::create_in_dir(index_dir.path(), schema).expect("create tantivy index");
    let mut index_writer: IndexWriter = index.writer(200_000_000).expect("open index writer");

    let mut sampled_passages: u64 = 0;
    let mut term_query_terms: Vec<String> = Vec::new();
    let mut phrase_query_pairs: Vec<(String, String)> = Vec::new();
    const QUERY_SAMPLE_CAP: usize = 500;

    let build_start = Instant::now();

    for (line_no, line) in reader.lines().enumerate() {
        let line_no = line_no as u64;
        if !line_no.is_multiple_of(stride) {
            continue;
        }
        let line = line.unwrap_or_else(|e| panic!("read line {line_no}: {e}"));
        if line.is_empty() {
            continue;
        }
        let parsed: CorpusLine = match serde_json::from_str(&line) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("skipping unparseable line {line_no}: {e}");
                continue;
            }
        };

        let words = strand_lexical::analyzer::analyze_lucene_en_word_only(&parsed.text);
        if words.is_empty() {
            sampled_passages += 1;
            continue;
        }

        if term_query_terms.len() < QUERY_SAMPLE_CAP {
            term_query_terms.push(words[0].clone());
        }
        if phrase_query_pairs.len() < QUERY_SAMPLE_CAP && words.len() >= 2 {
            phrase_query_pairs.push((words[0].clone(), words[1].clone()));
        }

        let mut tokens = Vec::with_capacity(words.len());
        let mut offset = 0usize;
        for (position, word) in words.iter().enumerate() {
            let offset_from = offset;
            let offset_to = offset_from + word.len();
            tokens.push(Token {
                offset_from,
                offset_to,
                position,
                text: word.clone(),
                position_length: 1,
            });
            offset = offset_to + 1; // one-byte separator, matching the joined text below
        }
        let text = words.join(" ");
        let pretok = PreTokenizedString { text, tokens };

        index_writer
            .add_document(doc!(body => pretok))
            .expect("add_document");

        sampled_passages += 1;
        if sampled_passages.is_multiple_of(50_000) {
            eprintln!("  {sampled_passages} passages indexed");
        }
    }

    index_writer.commit().expect("commit");
    let build_wall_seconds = build_start.elapsed().as_secs_f64();
    eprintln!("Indexed {sampled_passages} passages in {build_wall_seconds:.1}s, committed");

    let mut index_total_bytes: u64 = 0;
    let mut bytes_by_extension: BTreeMap<String, u64> = BTreeMap::new();
    for entry in std::fs::read_dir(index_dir.path()).expect("read index dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let size = entry.metadata().expect("metadata").len();
        index_total_bytes += size;
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("(none)")
            .to_string();
        *bytes_by_extension.entry(ext).or_insert(0) += size;
    }
    eprintln!(
        "Index on disk: {index_total_bytes} bytes total, by extension: {bytes_by_extension:?}"
    );

    let reader = index.reader().expect("open reader");
    let searcher = reader.searcher();

    let mut term_latencies_ns: Vec<u64> = Vec::with_capacity(term_query_terms.len());
    for term_text in &term_query_terms {
        let term = Term::from_field_text(body, term_text);
        let query = TermQuery::new(term, IndexRecordOption::WithFreqsAndPositions);
        let start = Instant::now();
        let _ = searcher
            .search(&query, &TopDocs::with_limit(10).order_by_score())
            .expect("term search");
        term_latencies_ns.push(start.elapsed().as_nanos() as u64);
    }

    let mut phrase_latencies_ns: Vec<u64> = Vec::with_capacity(phrase_query_pairs.len());
    for (a, b) in &phrase_query_pairs {
        let terms = vec![
            Term::from_field_text(body, a),
            Term::from_field_text(body, b),
        ];
        let query = PhraseQuery::new(terms);
        let start = Instant::now();
        let _ = searcher
            .search(&query, &TopDocs::with_limit(10).order_by_score())
            .expect("phrase search");
        phrase_latencies_ns.push(start.elapsed().as_nanos() as u64);
    }

    let output = TantivyBenchResult {
        source: "Tevatron/msmarco-passage-corpus (huggingface.co), same corpus and stride \
                  sampling as bench/src/msmarco_index.rs; every document indexed as a \
                  PreTokenizedString built directly from \
                  strand_lexical::analyzer::analyze_lucene_en_word_only's own output, so \
                  tantivy's own tokenizer/stopword/stemmer chain is never invoked — the \
                  vocabulary, doc frequencies, term frequencies, and position counts are \
                  identical to what STRAND's own postings/positions blobs would index for \
                  the same corpus subset."
            .to_string(),
        corpus_total_passages: CORPUS_TOTAL_PASSAGES,
        sample_stride: stride,
        sampled_passages,
        build_wall_seconds,
        index_total_bytes,
        bytes_by_extension,
        term_query_latency: latency_stats(term_latencies_ns),
        phrase_query_latency: latency_stats(phrase_latencies_ns),
    };

    // Named by sample size, not a fixed filename: RFC 0007/0008's Discussion
    // sections and docs/ledger.md cite the specific ~520K-passage
    // (sample_target=500_000) run's numbers by name
    // (tantivy-index-benchmark.json) — a fixed name would silently
    // overwrite that citation source on the next differently-scaled run.
    let out_path = if sample_target == 500_000 {
        format!(
            "{}/results/tantivy-index-benchmark.json",
            env!("CARGO_MANIFEST_DIR")
        )
    } else {
        format!(
            "{}/results/tantivy-index-benchmark-{sampled_passages}.json",
            env!("CARGO_MANIFEST_DIR")
        )
    };
    let json = serde_json::to_string_pretty(&output).unwrap();
    std::fs::write(&out_path, &json).unwrap_or_else(|e| panic!("write {out_path}: {e}"));
    eprintln!("Wrote {out_path} ({} bytes)", json.len());
}
