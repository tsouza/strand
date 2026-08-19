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

//! Measures the real, compiled `fst`-crate term-dictionary blob size (RFC
//! 0005 §2) at real MS MARCO vocabulary scale — the specific number RFC
//! 0005's own Open questions section names as missing ("needed before the
//! cold-open byte budget question... can be answered with a real number
//! instead of a structural argument", `docs/roadmap.md` M1-2).
//!
//! Deliberately narrower than `bench/src/field_end_to_end.rs`: this binary
//! builds only the term-dictionary FST (via
//! `strand_lexical::term_dictionary::build_term_dictionary`, the exact same
//! production function `field.rs` calls — so the measured bytes are
//! byte-for-byte what a real field build would emit, not a bench-only
//! approximation), skipping postings/positions construction entirely. That
//! makes it cheap enough to run over a much larger real vocabulary sample
//! than `field_end_to_end`'s existing 100,476-document run, which is what
//! "realistic vocabulary scale" (RFC 0005's own phrase) calls for.
//!
//! Real term data throughout, per `CLAUDE.md` §2: real MS MARCO passages,
//! the real declared analyzer chain
//! (`strand_lexical::analyzer::analyze_lucene_en_word_only`), the real `fst`
//! crate (version pinned in `crates/strand-lexical/Cargo.toml`, registered
//! in RFC 0005 §2 as `0.4.7`) — no synthetic term filler.

use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::BTreeSet;
use std::io::{BufRead, BufReader};
use std::time::Instant;
use strand_lexical::term_dictionary::{TermInfo, build_term_dictionary};

#[derive(serde::Deserialize)]
struct CorpusLine {
    text: String,
}

#[derive(Serialize)]
struct ScaleResult {
    sample_target: u64,
    sample_stride: u64,
    sampled_passages: u64,
    /// Distinct terms after the full declared analyzer chain (word-only
    /// UAX #29 tokenization, lower case folding, `lucene-en-10.5.1`
    /// stopwords, `snowball-porter2-en` stemming) — the same vocabulary a
    /// real field build over this sample would index.
    vocabulary_size: u64,
    /// Real bytes of the compiled `fst` crate `Map` blob (RFC 0005 §2),
    /// built by the same `build_term_dictionary` function
    /// `strand_lexical::field::build_field` calls in production — not a
    /// reimplementation.
    fst_bytes: u64,
    /// Bytes-per-term for the FST blob alone: `fst_bytes / vocabulary_size`.
    fst_bytes_per_term: f64,
    /// The paired term-info store this run also builds (28-byte fixed
    /// records, RFC 0005 §3) — reported for completeness since
    /// `build_term_dictionary` returns both blobs together, though this
    /// run's `TermInfo` values are placeholder zeros (doc_freq/postings/
    /// positions locations are not tracked by this binary; see module
    /// doc comment). Confirms the RFC's own stated arithmetic
    /// (`vocabulary_size * 28`), nothing more.
    term_info_bytes: u64,
    tokenize_and_dedupe_seconds: f64,
    fst_build_seconds: f64,
}

#[derive(Serialize)]
struct TermDictSizeResult {
    source: String,
    corpus_total_passages: u64,
    fst_crate_version: String,
    scales: Vec<ScaleResult>,
}

const CORPUS_TOTAL_PASSAGES: u64 = 8_841_823;

/// Reads the corpus with the given stride, tokenizes every sampled passage
/// with the real declared analyzer chain, and returns the distinct
/// vocabulary in unsigned UTF-8 byte order (`String`'s `Ord` compares the
/// underlying bytes directly, matching invariant 11's sort-order pin) —
/// exactly the order `fst::MapBuilder::insert` requires
/// (`crates/strand-lexical/src/term_dictionary.rs`).
fn sample_vocabulary(data_path: &str, sample_target: u64) -> (u64, u64, Vec<String>) {
    let stride = (CORPUS_TOTAL_PASSAGES / sample_target).max(1);
    eprintln!(
        "  sampling every {stride}-th passage (target {sample_target}, corpus {CORPUS_TOTAL_PASSAGES})"
    );

    let file = std::fs::File::open(data_path).unwrap_or_else(|e| {
        panic!("open {data_path}: {e} — run the download step first (see docs/ledger.md R9)")
    });
    let decoder = MultiGzDecoder::new(file);
    let reader = BufReader::with_capacity(1 << 20, decoder);

    let mut vocab: BTreeSet<String> = BTreeSet::new();
    let mut sampled_passages: u64 = 0;

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
                eprintln!("  skipping unparseable line {line_no}: {e}");
                continue;
            }
        };
        for token in strand_lexical::analyzer::analyze_lucene_en_word_only(&parsed.text) {
            vocab.insert(token);
        }
        sampled_passages += 1;
        if sampled_passages.is_multiple_of(500_000) {
            eprintln!(
                "    {sampled_passages} passages tokenized, {} distinct terms so far",
                vocab.len()
            );
        }
    }

    (stride, sampled_passages, vocab.into_iter().collect())
}

fn measure_scale(data_path: &str, sample_target: u64) -> ScaleResult {
    eprintln!("Scale: target {sample_target} passages");
    let tokenize_start = Instant::now();
    let (stride, sampled_passages, vocab) = sample_vocabulary(data_path, sample_target);
    let tokenize_elapsed = tokenize_start.elapsed();
    let vocabulary_size = vocab.len() as u64;
    eprintln!(
        "  {sampled_passages} passages -> {vocabulary_size} distinct terms in {:.2}s",
        tokenize_elapsed.as_secs_f64()
    );

    // Placeholder TermInfo: this binary measures FST-blob size only (the
    // FST encodes term bytes -> dense ordinal, never TermInfo's own
    // fields), so doc_freq/postings/positions values are irrelevant to the
    // measured quantity and are not tracked here (module doc comment).
    let terms: Vec<(&[u8], TermInfo)> = vocab
        .iter()
        .map(|term| (term.as_bytes(), TermInfo::default()))
        .collect();

    let fst_start = Instant::now();
    let (fst_bytes, term_info_bytes) = build_term_dictionary(&terms)
        .expect("terms are in sorted, deduplicated order by construction (BTreeSet)");
    let fst_elapsed = fst_start.elapsed();
    eprintln!(
        "  real fst crate Map: {} bytes ({:.3} bytes/term) built in {:.2}s",
        fst_bytes.len(),
        fst_bytes.len() as f64 / vocabulary_size as f64,
        fst_elapsed.as_secs_f64()
    );

    ScaleResult {
        sample_target,
        sample_stride: stride,
        sampled_passages,
        vocabulary_size,
        fst_bytes: fst_bytes.len() as u64,
        fst_bytes_per_term: fst_bytes.len() as f64 / vocabulary_size as f64,
        term_info_bytes: term_info_bytes.len() as u64,
        tokenize_and_dedupe_seconds: tokenize_elapsed.as_secs_f64(),
        fst_build_seconds: fst_elapsed.as_secs_f64(),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // Each arg is one sample_target scale to measure, so a single run can
    // report the FST's size-vs-vocabulary growth curve (RFC 0005 Napkin
    // math's "compiled size grows sublinearly... but 'sublinear' is not a
    // number" claim) instead of one isolated point. Defaults to the same
    // 100,476-document scale `bench/src/field_end_to_end.rs` already
    // measured (a cross-check: this binary's independent code path should
    // land close to that run's real 956,446-byte/135,874-term figure, not
    // exactly — stride sampling at a fixed passage-count *target* lands on
    // a slightly different real passage count than a contiguous prefix
    // would) plus the full real MS MARCO corpus.
    let scale_args: Vec<u64> = args[1..].iter().filter_map(|s| s.parse().ok()).collect();
    let scales_to_run: Vec<u64> = if scale_args.is_empty() {
        vec![100_476, CORPUS_TOTAL_PASSAGES]
    } else {
        scale_args
    };

    let data_path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/corpus.jsonl.gz");
    eprintln!("Reading real MS MARCO passages from {data_path}");

    let scales: Vec<ScaleResult> = scales_to_run
        .into_iter()
        .map(|target| measure_scale(data_path, target))
        .collect();

    let output = TermDictSizeResult {
        source: "Tevatron/msmarco-passage-corpus (huggingface.co), same 8,841,823-passage \
                  corpus as bench/src/msmarco_index.rs and bench/src/field_end_to_end.rs; \
                  tokenized with strand_lexical::analyzer::analyze_lucene_en_word_only \
                  (UAX29-word/word-only, lower, lucene-en-10.5.1 stopwords, \
                  snowball-porter2-en stemming); FST built by \
                  strand_lexical::term_dictionary::build_term_dictionary, the real production \
                  function, not a bench-only approximation."
            .to_string(),
        corpus_total_passages: CORPUS_TOTAL_PASSAGES,
        fst_crate_version: "0.4.7".to_string(),
        scales,
    };

    let out_path = concat!(env!("CARGO_MANIFEST_DIR"), "/results/term-dict-size.json");
    let json = serde_json::to_string_pretty(&output).unwrap();
    std::fs::write(out_path, &json).unwrap_or_else(|e| panic!("write {out_path}: {e}"));
    eprintln!("Wrote {out_path} ({} bytes)", json.len());
}
