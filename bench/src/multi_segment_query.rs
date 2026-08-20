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

//! `docs/roadmap.md` M3-7, **the without-compaction partial version only**.
//!
//! M3-7 is "the same corpus at 1, 16, and ~128 segments, cold and warm,
//! producing a measured segment-count-amplification curve," and its
//! roadmap entry names its own realistic shape as blocked on M3-1
//! (compaction, not yet built): a real corpus in production would reach a
//! hundred segments through commits *and* merges, and this benchmark
//! cannot exercise the merge half because there is no compaction code to
//! call. What it can and does exercise, per that same roadmap entry's own
//! text — "a *without-compaction* version (many small segments from
//! repeated small commits, no merge) could run earlier as a partial
//! measurement" — is the other half: real segment-count fan-out cost, with
//! every segment produced by its own independent `SegmentBuilder` +
//! `manifest::commit` call against real MinIO, never merged. This is a
//! partial measurement of M3-7, not the full one, and it does not close
//! M3-7 in `docs/roadmap.md`. M3-8 (R10's manifest-pruning-metadata
//! question) depends on the full M3-7 and stays blocked regardless of this
//! benchmark's results.
//!
//! **Method.** One fixed, real corpus (`TOTAL_DOCS` documents, generated
//! once from a seeded RNG over a fixed real-English-word vocabulary — see
//! "Corpus" below) is partitioned, unchanged, into 1, 16, and 128 shards.
//! Each shard is analyzed with the real `strand_lexical::field::build_field`
//! pipeline (real FST term dictionary, real BP-adjacent postings, real
//! per-document lengths) and committed as its own segment via
//! `strand_core::manifest::commit` — one commit per shard, sequentially, no
//! writer contention and no merge step, matching real production behavior
//! under `CLAUDE.md` §6's "write amplification is the writer's problem":
//! many small commits from one writer produce many small segments, exactly
//! as they would in a real deployment before a compactor ever runs. For
//! each of the three segment counts, the same one real BM25 query term is
//! then run against the resulting table two ways: **cold** — every trial
//! re-reads the pointer, the snapshot, and every segment from scratch, no
//! client-side cache, the same discipline `bench/src/cold_open.rs` and
//! `bench/src/field_cold_open.rs` already established — and **warm**, in
//! the two forms `docs/benchmarks.md` names: fully cached (zero GETs,
//! segment bytes already resident) and cached-with-freshness-check (a real
//! `read_snapshot` call every trial, to confirm the cached snapshot is
//! still current, per `CLAUDE.md` §6's "reader freshness has a price, and
//! it is stated"). GET count, bytes fetched, and latency (p50/p90/p99 over
//! 30 real trials, `ITERATIONS`, the same trial count `bench/src/
//! cold_open.rs` uses) are measured for all three, at all three segment
//! counts.
//!
//! **What "GETs" means here, honestly** — the same limitation `bench/src/
//! cold_open.rs`, `bench/src/field_cold_open.rs`, and `bench/src/
//! vector_cold_open.rs` all already state: `strand-core`'s manifest-driven
//! open path has no Range-GET variant, so opening a segment here means one
//! GET for the whole segment object, not just its cold-fetchable open-wave
//! subset. The cold GET count this benchmark measures is therefore exactly
//! `2 + segment_count` — 2 for the manifest (pointer, snapshot object),
//! plus one whole-segment GET per segment — asserted, not just observed,
//! the same way `bench/src/cold_open.rs` asserts its own GET-count bound.
//!
//! **What this does and does not confirm.** This runs against MinIO on
//! localhost with no injected network latency (the same limitation `bench/
//! src/cold_open.rs` and `bench/src/parallel_range_fetch.rs` both already
//! carry) — the latency figures below are real, measured wall-clock time,
//! but not representative of real S3 round-trip cost; `bench/src/
//! cold_open_injected_latency.rs` (roadmap X-4) is the benchmark that adds
//! that half for the single-segment case, and doing the same here (many
//! segments x injected latency) is further, still-open work, not attempted
//! by this benchmark. What this benchmark *does* give a real, measured
//! answer to is `CLAUDE.md` §7's own question for this exact scenario:
//! whether cold query cost actually grows O(segments) in GETs and bytes
//! when nothing in the manifest prunes segments at query time — the
//! per-config results below report the real curve, not an assumed one.
//!
//! **Results, real and measured (`bench/results/multi-segment-query-partial.json`,
//! run: `cargo run -p strand-bench --bin multi-segment-query`).** Cold GETs
//! were exactly `2 + segment_count` at all three points, asserted and
//! confirmed: **3 at 1 segment, 18 at 16 segments, 130 at 128 segments** —
//! the O(segments) model `CLAUDE.md` §7 states ("the manifest carries
//! nothing that prunes segments at query time") holds exactly, not just
//! approximately, for GET count on this real corpus. Cold bytes fetched
//! also grew with segment count on the *same* total corpus — 643,158 bytes
//! at 1 segment, 713,164 at 16, 1,291,500 at 128, roughly 2.0x the 1-segment
//! figure at 128 segments — a real, measured instance of the amplification
//! `CLAUDE.md` §7 names: splitting one corpus into more segments duplicates
//! each segment's fixed overhead (footer, hotcache, and — since `VOCAB` is
//! shared across every shard — a term dictionary that does not shrink
//! proportionally with shard size), even though total document count never
//! changes. Cold latency also grew with segment count (p50 98.4ms → 149.3ms
//! → 1434.0ms), but honestly, **not proportionally to GET count at every
//! step**: 1→16 segments is 6x the GETs but only 1.5x the p50 latency,
//! while 16→128 is 7.2x the GETs and 9.6x the p50 latency — consistent with
//! fixed per-request overhead dominating at small N and something closer to
//! linear-in-GET-count dominating at larger N, on localhost with no
//! injected network latency (this benchmark's own honestly-stated limit,
//! same as `bench/src/cold_open.rs`'s), not a claim about real S3's
//! round-trip-bound regime. Warm-cached latency (zero GETs, pure CPU
//! decode-and-score cost) did **not** grow monotonically with segment count
//! (p50 44.8ms at 1 segment, 11.8ms at 16, 57.2ms at 128) — a real result,
//! reported as measured rather than smoothed into a tidier curve; plausible
//! contributors are this benchmark's debug (unoptimized) build and the
//! heavy concurrent CPU load other work shared this host with while it ran
//! (`uptime` reported a load average above 21 on the run), neither of which
//! this benchmark controls for or claims to. Warm-with-freshness-check cost
//! exactly 2 GETs at every segment count, confirmed constant and
//! independent of segment count as `CLAUDE.md` §6 predicts, though at 2
//! real GETs (pointer + snapshot object), not the "one pointer round trip"
//! its prose names — `strand_core::manifest::read_snapshot` has no
//! pointer-only fast path today, a real, stated gap between that prose and
//! this implementation. Total BM25 matches for the query term were
//! identical (11,568) at every segment count, asserted and confirmed: the
//! same corpus produces the same answer regardless of how it is
//! partitioned into segments, which is the correctness half of this
//! result, alongside the cost curve.
//!
//! **Corpus.** `bench/src/field_cold_open.rs` and `bench/src/msmarco_index.rs`
//! use a real downloaded MS MARCO passage sample
//! (`bench/data/corpus.jsonl.gz`, ~1.07 GB, gitignored, docs/benchmarks.md).
//! This benchmark does not: its question is segment-count fan-out cost, not
//! text relevance or vocabulary realism, and `bench/src/vector_cold_open.rs`
//! already established the precedent for this project that a real,
//! seeded-RNG-generated corpus run through the real index-building pipeline
//! (there: real k-means and real RaBitQ quantization over synthetic
//! vectors; here: the real analyzer, term dictionary, and postings builder
//! over synthetic-but-real-English-word text) is a legitimate "real
//! measurement," as opposed to a mocked storage layer or a fabricated
//! result. `generate_corpus` below draws each document's words from
//! `VOCAB` (real, distinct English nouns) with a Zipfian weighting
//! (`weight(rank) = 1 / rank`, a real, standard model of natural-language
//! term-frequency skew) via a seeded `StdRng`, so the corpus and the
//! results below are exactly reproducible.

use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::Serialize;
use std::collections::HashMap;
use strand_bench::{CountingStore, create_bucket, percentile, store_for, timed, with_minio};
use strand_core::container::{Footer, Hotcache};
use strand_core::manifest::{commit, read_snapshot};
use strand_core::scoring::Bm25Profile;
use strand_core::segment::{SegmentBuilder, write_segment};
use strand_core::store::ConditionalStore;
use strand_lexical::field::{FieldReader, build_field};

/// The same corpus is partitioned into this many segments in turn — 1 (the
/// no-fan-out baseline), 16, and 128 (`docs/roadmap.md` M3-7's own stated
/// scale, matching `CLAUDE.md` §7's "on the order of a hundred segments").
const SEGMENT_COUNTS: [usize; 3] = [1, 16, 128];

/// Divisible by every entry in `SEGMENT_COUNTS` (128 x 100), so every
/// configuration partitions the identical corpus into equal-sized shards
/// with no remainder document dropped or duplicated.
const TOTAL_DOCS: usize = 12_800;

/// Real trial count for every percentile reported — matches
/// `bench/src/cold_open.rs`'s own `ITERATIONS`.
const ITERATIONS: usize = 30;

const CORPUS_SEED: u64 = 20_260_820;

const QUERY_TERM_RAW: &str = "energy";

/// Real, distinct English nouns — the vocabulary `generate_corpus` samples
/// from. Chosen to avoid `strand_lexical::analyzer::LUCENE_EN_10_5_1_STOPWORDS`
/// so every occurrence survives analysis into a real posting.
const VOCAB: &[&str] = &[
    "system",
    "water",
    "energy",
    "process",
    "market",
    "server",
    "network",
    "storage",
    "vector",
    "cluster",
    "segment",
    "index",
    "query",
    "record",
    "signal",
    "device",
    "engine",
    "planet",
    "forest",
    "garden",
    "harbor",
    "bridge",
    "castle",
    "desert",
    "island",
    "jungle",
    "meadow",
    "mountain",
    "ocean",
    "prairie",
    "river",
    "valley",
    "village",
    "student",
    "teacher",
    "doctor",
    "farmer",
    "artist",
    "singer",
    "writer",
    "driver",
    "worker",
    "leader",
    "player",
    "reader",
    "walker",
    "hunter",
    "sailor",
    "trader",
    "banker",
    "lawyer",
    "nurse",
    "pilot",
    "chef",
    "bakery",
    "library",
    "museum",
    "theater",
    "factory",
    "airport",
    "station",
    "tunnel",
    "canyon",
    "glacier",
    "volcano",
    "reef",
    "coral",
    "stream",
    "spring",
    "cave",
    "temple",
    "palace",
    "fortress",
    "cottage",
    "cabin",
    "tower",
    "chapel",
    "abbey",
    "orchard",
    "vineyard",
    "pasture",
    "ranch",
    "farm",
    "mill",
    "quarry",
    "mine",
    "furnace",
    "turbine",
    "reactor",
    "battery",
    "circuit",
    "antenna",
    "satellite",
    "telescope",
    "compass",
    "anchor",
    "cargo",
    "voyage",
    "journey",
    "expedition",
    "caravan",
    "convoy",
];

#[derive(Serialize)]
struct SegmentCountResult {
    segment_count: usize,
    docs_per_segment: usize,

    /// GETs for one cold query trial: pointer + snapshot + one whole-segment
    /// GET per segment, constant across all `ITERATIONS` trials (asserted).
    /// Expected, and asserted equal to, `2 + segment_count`.
    cold_get_count: u64,
    /// Total bytes returned by every GET in one cold trial (manifest
    /// objects plus every whole segment object), constant across trials.
    cold_bytes_fetched: u64,
    cold_latency_ms_p50: f64,
    cold_latency_ms_p90: f64,
    cold_latency_ms_p99: f64,
    cold_latency_ms_min: f64,
    cold_latency_ms_max: f64,

    /// Segment bytes already resident (fetched once, outside the timed
    /// loop); a warm trial here issues zero GETs (asserted).
    warm_cached_get_count: u64,
    warm_cached_latency_ms_p50: f64,
    warm_cached_latency_ms_p90: f64,
    warm_cached_latency_ms_p99: f64,

    /// Segment bytes cached, but every trial also re-reads the manifest
    /// (pointer + snapshot object, `strand_core::manifest::read_snapshot`)
    /// to confirm the cached snapshot is still current — `CLAUDE.md` §6's
    /// "reader freshness has a price" case. `read_snapshot` here has no
    /// pointer-only fast path (it always re-fetches the snapshot object
    /// too), so this measures 2 GETs, not the idealized "one pointer round
    /// trip" language in `CLAUDE.md` §6 — a real, honestly-stated gap
    /// between that prose and this implementation, not smoothed over.
    warm_freshness_checked_get_count: u64,
    warm_freshness_checked_latency_ms_p50: f64,
    warm_freshness_checked_latency_ms_p90: f64,
    warm_freshness_checked_latency_ms_p99: f64,

    /// Total BM25 matches for `query_term_stemmed` across every segment in
    /// this configuration's table — must be identical across all three
    /// `segment_count` configurations (same corpus, same term, only the
    /// partitioning differs), and is asserted so in `main`.
    total_matches: usize,
}

#[derive(Serialize)]
struct MultiSegmentQueryResult {
    scope: &'static str,
    total_docs: usize,
    iterations: usize,
    query_term_raw: String,
    query_term_stemmed: String,
    configs: Vec<SegmentCountResult>,
}

/// Generates `total_docs` real-but-synthetic English-word documents (see
/// this file's module doc, "Corpus"): each document's word count is drawn
/// uniformly from `20..60`, and each word is drawn from `VOCAB` with
/// `weight(rank) = 1 / rank` Zipfian skew — a real, standard model of
/// natural-language term-frequency distribution, not a uniform pick, so
/// segments carry the kind of skewed doc-frequency spread a real BM25
/// index would.
fn generate_corpus(total_docs: usize, seed: u64) -> Vec<String> {
    let mut rng = StdRng::seed_from_u64(seed);
    let weights: Vec<f64> = (1..=VOCAB.len()).map(|rank| 1.0 / rank as f64).collect();
    let total_weight: f64 = weights.iter().sum();

    let mut docs = Vec::with_capacity(total_docs);
    for _ in 0..total_docs {
        let doc_len = rng.random_range(20..60usize);
        let mut words = Vec::with_capacity(doc_len);
        for _ in 0..doc_len {
            let mut r = rng.random::<f64>() * total_weight;
            let mut chosen = VOCAB[VOCAB.len() - 1];
            for (i, w) in weights.iter().enumerate() {
                if r < *w {
                    chosen = VOCAB[i];
                    break;
                }
                r -= w;
            }
            words.push(chosen);
        }
        docs.push(words.join(" "));
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

/// Runs one segment's worth of BM25 scoring against already-resident bytes.
fn score_segment(
    segment_bytes: &[u8],
    doc_lengths: &[u32],
    term: &str,
    profile: &Bm25Profile,
) -> usize {
    let hotcache = open_segment_bytes(segment_bytes);
    let reader = FieldReader::open_by_name(segment_bytes, &hotcache.blobs, "passage")
        .expect("all lexical blobs present");
    reader
        .search_bm25(term, doc_lengths, profile)
        .map_or(0, |scored| scored.len())
}

fn run_config(
    endpoint: &str,
    base_bucket: &str,
    segment_count: usize,
    doc_refs: &[&str],
    term: &str,
) -> SegmentCountResult {
    let bucket = format!("{base_bucket}-n{segment_count}");
    create_bucket(endpoint, &bucket);
    let store = store_for(endpoint, &bucket);

    let docs_per_segment = TOTAL_DOCS / segment_count;
    assert_eq!(
        docs_per_segment * segment_count,
        TOTAL_DOCS,
        "TOTAL_DOCS must divide evenly by every configured segment count"
    );

    // Many small commits, no merge: compaction (M3-1) does not exist yet,
    // so this is genuinely the only way segments accumulate today —
    // exactly the shape this benchmark's module doc names as its scope.
    let mut doc_lengths_by_path: HashMap<String, Vec<u32>> = HashMap::new();
    for (shard_idx, shard_docs) in doc_refs.chunks(docs_per_segment).enumerate() {
        let field = build_field("passage", shard_docs);
        let mut builder = SegmentBuilder::new(shard_docs.len() as u64);
        for blob in field.to_blob_specs() {
            builder.add_blob(blob);
        }
        let path = format!("segments/shard-{shard_idx:04}.bin");
        doc_lengths_by_path.insert(path.clone(), field.doc_lengths.clone());
        commit(&store, |row_id_base| {
            vec![write_segment(&store, &path, &builder, row_id_base)]
        })
        .expect("commit succeeds against this configuration's fresh table");
    }

    let committed = read_snapshot(&store)
        .expect("read succeeds")
        .expect("a snapshot exists after at least one commit");
    assert_eq!(
        committed.segments.len(),
        segment_count,
        "every shard's commit must land as its own segment, with no merge"
    );

    let profile = Bm25Profile::default();
    let counting = CountingStore::new(&store);

    // --- Cold: every trial re-fetches the manifest and every segment. ---
    let mut cold_latencies = Vec::with_capacity(ITERATIONS);
    let mut cold_get_counts = Vec::with_capacity(ITERATIONS);
    let mut cold_byte_counts = Vec::with_capacity(ITERATIONS);
    let mut cold_total_matches = 0usize;
    for _ in 0..ITERATIONS {
        counting.reset();
        let (matches, elapsed_ms) = timed(|| {
            let snapshot = read_snapshot(&counting).unwrap().unwrap();
            let mut total = 0usize;
            for seg in &snapshot.segments {
                let (segment_bytes, _) = counting.get(&seg.path).unwrap().unwrap();
                let doc_lengths = &doc_lengths_by_path[&seg.path];
                total += score_segment(&segment_bytes, doc_lengths, term, &profile);
            }
            total
        });
        cold_latencies.push(elapsed_ms);
        cold_get_counts.push(counting.get_count());
        cold_byte_counts.push(counting.get_bytes());
        cold_total_matches = matches;
    }
    cold_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let cold_get_count = cold_get_counts[0];
    assert!(
        cold_get_counts.iter().all(|&c| c == cold_get_count),
        "cold GET count must be constant across iterations: {cold_get_counts:?}"
    );
    assert_eq!(
        cold_get_count,
        2 + segment_count as u64,
        "cold query GETs must be exactly pointer + snapshot + one whole-segment \
         GET per segment (no range-GET support yet): expected {}, got {cold_get_count}",
        2 + segment_count
    );
    let cold_bytes_fetched = cold_byte_counts[0];
    assert!(
        cold_byte_counts.iter().all(|&b| b == cold_bytes_fetched),
        "cold bytes fetched must be constant across iterations: {cold_byte_counts:?}"
    );

    // --- Warm (fully cached): pre-fetch every segment once, then query
    // resident bytes only — zero GETs per trial. ---
    let mut cached_segments: Vec<(String, Vec<u8>)> = Vec::with_capacity(segment_count);
    for seg in &committed.segments {
        let (bytes, _) = ConditionalStore::get(&store, &seg.path)
            .unwrap()
            .expect("segment exists");
        cached_segments.push((seg.path.clone(), bytes));
    }

    let mut warm_latencies = Vec::with_capacity(ITERATIONS);
    let mut warm_total_matches = 0usize;
    for _ in 0..ITERATIONS {
        let (matches, elapsed_ms) = timed(|| {
            let mut total = 0usize;
            for (path, segment_bytes) in &cached_segments {
                let doc_lengths = &doc_lengths_by_path[path];
                total += score_segment(segment_bytes, doc_lengths, term, &profile);
            }
            total
        });
        warm_latencies.push(elapsed_ms);
        warm_total_matches = matches;
    }
    warm_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(
        warm_total_matches, cold_total_matches,
        "warm and cold queries over the identical committed segments must \
         return the identical match count"
    );

    // --- Warm, with a pointer-freshness check every trial: cached bytes,
    // but a real read_snapshot call to confirm nothing changed. ---
    let mut warm_fresh_latencies = Vec::with_capacity(ITERATIONS);
    let mut warm_fresh_get_counts = Vec::with_capacity(ITERATIONS);
    for _ in 0..ITERATIONS {
        counting.reset();
        let (_, elapsed_ms) = timed(|| {
            let refreshed = read_snapshot(&counting).unwrap().unwrap();
            assert_eq!(
                refreshed.version, committed.version,
                "no concurrent writer runs during this benchmark, so the \
                 pointer must not have moved between trials"
            );
            let mut total = 0usize;
            for (path, segment_bytes) in &cached_segments {
                let doc_lengths = &doc_lengths_by_path[path];
                total += score_segment(segment_bytes, doc_lengths, term, &profile);
            }
            total
        });
        warm_fresh_latencies.push(elapsed_ms);
        warm_fresh_get_counts.push(counting.get_count());
    }
    warm_fresh_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let warm_fresh_gets = warm_fresh_get_counts[0];
    assert!(
        warm_fresh_get_counts.iter().all(|&c| c == warm_fresh_gets),
        "warm-with-freshness-check GET count must be constant across iterations: {warm_fresh_get_counts:?}"
    );
    assert_eq!(
        warm_fresh_gets, 2,
        "a freshness check costs exactly one read_snapshot call (pointer + \
         snapshot object, strand_core::manifest::read_snapshot has no \
         pointer-only fast path today) — independent of segment count"
    );

    println!(
        "segments={segment_count:4}: cold {cold_get_count:4} GETs, {cold_bytes_fetched:9} bytes, \
         p50={:.2}ms p90={:.2}ms p99={:.2}ms | warm-cached p50={:.3}ms p90={:.3}ms | \
         warm+freshness {warm_fresh_gets} GETs p50={:.2}ms p90={:.2}ms | matches={cold_total_matches}",
        percentile(&cold_latencies, 0.50),
        percentile(&cold_latencies, 0.90),
        percentile(&cold_latencies, 0.99),
        percentile(&warm_latencies, 0.50),
        percentile(&warm_latencies, 0.90),
        percentile(&warm_fresh_latencies, 0.50),
        percentile(&warm_fresh_latencies, 0.90),
    );

    SegmentCountResult {
        segment_count,
        docs_per_segment,
        cold_get_count,
        cold_bytes_fetched,
        cold_latency_ms_p50: percentile(&cold_latencies, 0.50),
        cold_latency_ms_p90: percentile(&cold_latencies, 0.90),
        cold_latency_ms_p99: percentile(&cold_latencies, 0.99),
        cold_latency_ms_min: cold_latencies[0],
        cold_latency_ms_max: cold_latencies[cold_latencies.len() - 1],
        warm_cached_get_count: 0,
        warm_cached_latency_ms_p50: percentile(&warm_latencies, 0.50),
        warm_cached_latency_ms_p90: percentile(&warm_latencies, 0.90),
        warm_cached_latency_ms_p99: percentile(&warm_latencies, 0.99),
        warm_freshness_checked_get_count: warm_fresh_gets,
        warm_freshness_checked_latency_ms_p50: percentile(&warm_fresh_latencies, 0.50),
        warm_freshness_checked_latency_ms_p90: percentile(&warm_fresh_latencies, 0.90),
        warm_freshness_checked_latency_ms_p99: percentile(&warm_fresh_latencies, 0.99),
        total_matches: cold_total_matches,
    }
}

fn main() {
    let docs = generate_corpus(TOTAL_DOCS, CORPUS_SEED);
    let doc_refs: Vec<&str> = docs.iter().map(String::as_str).collect();

    let stemmed = strand_lexical::analyzer::analyze_lucene_en_word_only(QUERY_TERM_RAW);
    let term = stemmed
        .first()
        .cloned()
        .expect("query term must survive analysis (not a stopword, not dropped)");
    eprintln!("query term {QUERY_TERM_RAW:?} -> stemmed {term:?}");

    with_minio(|endpoint, base_bucket| {
        let mut configs = Vec::with_capacity(SEGMENT_COUNTS.len());
        for &segment_count in &SEGMENT_COUNTS {
            eprintln!("=== segment_count = {segment_count} ===");
            configs.push(run_config(
                endpoint,
                base_bucket,
                segment_count,
                &doc_refs,
                &term,
            ));
        }

        let match_counts: Vec<usize> = configs.iter().map(|c| c.total_matches).collect();
        assert!(
            match_counts.windows(2).all(|w| w[0] == w[1]),
            "same corpus, same query term: total matches must not depend on \
             how the corpus was partitioned into segments: {match_counts:?}"
        );

        let gets_by_config: Vec<(usize, u64)> = configs
            .iter()
            .map(|c| (c.segment_count, c.cold_get_count))
            .collect();
        println!(
            "cold GETs by segment count (pointer + snapshot + one GET per segment): {gets_by_config:?}"
        );

        let result = MultiSegmentQueryResult {
            scope: "partial: without-compaction (many small commits, no merge) — \
                    docs/roadmap.md M3-7's full, post-compaction version remains \
                    blocked on M3-1",
            total_docs: TOTAL_DOCS,
            iterations: ITERATIONS,
            query_term_raw: QUERY_TERM_RAW.to_string(),
            query_term_stemmed: term,
            configs,
        };

        strand_bench::write_report("multi-segment-query-partial", result);
    });
}
