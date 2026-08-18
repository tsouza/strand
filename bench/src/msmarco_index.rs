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

//! Builds a real inverted index over a stride-sampled subset of the MS MARCO
//! passage corpus (`bench/data/corpus.jsonl.gz`, 8,841,823 passages,
//! `Tevatron/msmarco-passage-corpus` on Hugging Face — the same corpus as
//! Microsoft's official `collection.tar.gz`, whose public Azure blob access
//! was revoked as of this run; re-verify before reusing this source) and
//! emits real per-document doc-ID delta-gap and term-frequency value arrays,
//! stratified by document-frequency decile — the "real, not synthetic"
//! postings distribution `docs/ledger.md`'s R9 entry names as still missing.
//!
//! Tokenization reuses `strand_lexical::analyzer::analyze_lucene_en_word_only`
//! directly: the exact declared chain (UAX #29 word-only tokenization, lower
//! case folding, `lucene-en-10.5.1` stopwords, `snowball-porter2-en`
//! stemming) rather than a bench-only approximation, so the measured
//! distribution reflects what STRAND's own analyzer chain would actually
//! index, not a generic tokenizer's output.

use flate2::read::MultiGzDecoder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};

#[derive(Deserialize)]
struct CorpusLine {
    text: String,
}

#[derive(Serialize)]
struct IndexStats {
    corpus_total_passages: u64,
    sample_stride: u64,
    sampled_passages: u64,
    vocabulary_size: u64,
    total_postings: u64,
}

#[derive(Serialize)]
struct DecileSample {
    decile: u32,
    doc_freq_min: u32,
    doc_freq_max: u32,
    terms_sampled: u32,
    /// Consecutive doc-ordinal deltas across all sampled terms' postings
    /// lists in this decile, concatenated.
    d_gaps: Vec<u32>,
    /// Per-posting term frequency (occurrences of the term within that
    /// document, after the full analyzer chain) across the same postings.
    term_frequencies: Vec<u32>,
}

#[derive(Serialize)]
struct RealPostingsSample {
    source: String,
    stats: IndexStats,
    deciles: Vec<DecileSample>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let sample_target: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(500_000);
    let terms_per_decile: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(40);

    let data_path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/corpus.jsonl.gz");
    eprintln!("Reading {data_path}");

    let file = std::fs::File::open(data_path).unwrap_or_else(|e| {
        panic!("open {data_path}: {e} — run the download step first (see docs/ledger.md R9)")
    });
    let decoder = MultiGzDecoder::new(file);
    let reader = BufReader::with_capacity(1 << 20, decoder);

    // Known exact line count (bench/data/corpus.jsonl.gz, verified via `zcat
    // | wc -l` before this run) — used to compute a stride that spans the
    // whole corpus, not just its first (topically clustered) passages.
    const CORPUS_TOTAL_PASSAGES: u64 = 8_841_823;
    let stride = (CORPUS_TOTAL_PASSAGES / sample_target).max(1);

    eprintln!(
        "Sampling every {stride}-th passage (target {sample_target}, corpus {CORPUS_TOTAL_PASSAGES})"
    );

    // term -> sorted (by construction) list of (doc_ordinal, tf) postings.
    // doc_ordinal is the index among *sampled* passages, in corpus order.
    let mut index: HashMap<String, Vec<(u32, u32)>> = HashMap::new();
    let mut sampled_passages: u64 = 0;
    let mut doc_ordinal: u32 = 0;

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

        let tokens = strand_lexical::analyzer::analyze_lucene_en_word_only(&parsed.text);
        let mut per_doc_tf: HashMap<String, u32> = HashMap::new();
        for token in tokens {
            *per_doc_tf.entry(token).or_insert(0) += 1;
        }
        for (term, tf) in per_doc_tf {
            index.entry(term).or_default().push((doc_ordinal, tf));
        }

        doc_ordinal += 1;
        sampled_passages += 1;
        if sampled_passages.is_multiple_of(50_000) {
            eprintln!("  {sampled_passages} passages indexed, {} terms so far", index.len());
        }
    }

    let vocabulary_size = index.len() as u64;
    let total_postings: u64 = index.values().map(|v| v.len() as u64).sum();
    eprintln!(
        "Indexed {sampled_passages} passages, {vocabulary_size} distinct terms, {total_postings} postings"
    );

    // Stratify terms by document frequency into deciles, per
    // CLAUDE.md §7's own "bytes-fetched vs bytes-used across term frequency
    // deciles" framing — sample a bounded number of terms from each decile
    // rather than every term (postings-list count alone can run into the
    // millions of terms; this keeps the emitted JSON small while still
    // spanning the whole frequency spectrum, not just head or tail terms).
    let mut doc_freqs: Vec<(String, u32)> =
        index.iter().map(|(term, postings)| (term.clone(), postings.len() as u32)).collect();
    doc_freqs.sort_by_key(|(_, df)| *df);

    let n = doc_freqs.len();
    let mut deciles = Vec::with_capacity(10);
    for decile in 0..10u32 {
        let start = (n * decile as usize) / 10;
        let end = ((n * (decile as usize + 1)) / 10).max(start + 1).min(n);
        if start >= end {
            continue;
        }
        let slice = &doc_freqs[start..end];
        let doc_freq_min = slice.iter().map(|(_, df)| *df).min().unwrap();
        let doc_freq_max = slice.iter().map(|(_, df)| *df).max().unwrap();

        // Deterministic, evenly-spaced sample of terms within this decile
        // (not the full decile — keeps output size bounded and spans the
        // decile's own range rather than clustering at one end).
        let take = terms_per_decile.min(slice.len());
        let step = (slice.len() / take).max(1);
        let mut d_gaps = Vec::new();
        let mut term_frequencies = Vec::new();
        let mut terms_sampled = 0u32;
        for i in (0..slice.len()).step_by(step).take(take) {
            let (term, _) = &slice[i];
            let postings = &index[term];
            let mut prev: Option<u32> = None;
            for &(doc, tf) in postings {
                if let Some(p) = prev {
                    d_gaps.push(doc - p);
                }
                term_frequencies.push(tf);
                prev = Some(doc);
            }
            terms_sampled += 1;
        }

        deciles.push(DecileSample {
            decile,
            doc_freq_min,
            doc_freq_max,
            terms_sampled,
            d_gaps,
            term_frequencies,
        });
    }

    let output = RealPostingsSample {
        source: "Tevatron/msmarco-passage-corpus (huggingface.co), same 8,841,823-passage \
                  corpus as Microsoft's official MS MARCO passage collection.tar.gz \
                  (msmarco.blob.core.windows.net access revoked as of this run's fetch date); \
                  tokenized with strand_lexical::analyzer::analyze_lucene_en_word_only \
                  (UAX29-word/word-only, lower, lucene-en-10.5.1 stopwords, \
                  snowball-porter2-en stemming)."
            .to_string(),
        stats: IndexStats {
            corpus_total_passages: CORPUS_TOTAL_PASSAGES,
            sample_stride: stride,
            sampled_passages,
            vocabulary_size,
            total_postings,
        },
        deciles,
    };

    let out_path = concat!(env!("CARGO_MANIFEST_DIR"), "/results/msmarco-real-postings-sample.json");
    let json = serde_json::to_string_pretty(&output).unwrap();
    std::fs::write(out_path, &json).unwrap_or_else(|e| panic!("write {out_path}: {e}"));
    eprintln!("Wrote {out_path} ({} bytes)", json.len());
}
