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

//! Roadmap X-5: the parallel-wave aggregate throughput measurement behind
//! `CLAUDE.md` §7's 100 MB provisional cold-open byte budget. §7's own text
//! states the empirical claim this benchmark exists to check: "a single
//! object-storage stream runs in the ~50–100 MB/s class... the budget
//! assumes a reader that fetches ranges in parallel... bringing the tier-1
//! wave into the low hundreds of milliseconds, the same order as one round
//! trip, so the open stays round-trip-bound rather than bandwidth-bound" —
//! and "the achieved aggregate throughput of a parallel wave is measured at
//! M0." That measurement did not exist until this benchmark: it was gated
//! on a real `tier: cold-fetchable` vector blob existing (RFC 0010, shipped
//! this session), per `docs/ledger.md`'s "Other pending figures" entry.
//!
//! This measures real N-way parallel byte-range GETs against real MinIO,
//! compared to one sequential stream, using `strand-core`'s new
//! `RangeGetStore` trait (`crates/strand-core/src/store.rs`) — a small,
//! scoped extension added here because no Range-GET capability existed on
//! any `strand-core` store before this session (confirmed by grep, and
//! noted as a limitation in `bench/src/vector_cold_open.rs`'s own module
//! doc, which this benchmark resolves for the throughput question, though
//! not by rewriting that benchmark's own whole-object-GET open path — a
//! separate, still-open piece of work).
//!
//! Scope, stated honestly: the real vector segment `vector_cold_open.rs`
//! builds has an open-wave of only ~1.2 MB at its benchmarked scale — far
//! below the 100 MB budget this rationale is actually about. To measure
//! aggregate throughput at the byte volume the budget names, this
//! benchmark fetches a single synthetic 100 MB object, split into N
//! equal byte ranges fetched concurrently (one real OS thread and one real
//! independent `S3Store` per range, `bench/src/commit_contention.rs`'s own
//! established concurrency pattern — a shared client cannot be driven from
//! multiple threads, see `bench/src/lib.rs`'s `store_for` docs). This
//! isolates the raw store-level range-GET throughput question from segment
//! parsing; it is real object-storage I/O, not a simulation.
//!
//! Second honest limitation, the same one `bench/src/cold_open.rs` and
//! `bench/src/vector_cold_open.rs` both already carry: this runs against
//! MinIO on localhost with no injected network latency. §7's "~50–100 MB/s"
//! and "low hundreds of milliseconds" figures describe real S3 over a real
//! network; loopback has no realistic per-request RTT or bandwidth ceiling
//! to reproduce those absolute figures against. What a real, unsimulated
//! backend *can* confirm or refute, even on loopback, is the mechanism
//! itself: whether splitting one fetch into many concurrent range GETs
//! measurably increases aggregate throughput over one sequential stream
//! against the same backend, and by how much. That comparison — not the
//! absolute MB/s number — is this benchmark's real contribution, and its
//! report is labeled accordingly.

use rand::RngCore;
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::Serialize;
use strand_bench::{percentile, store_for, timed, with_minio, write_report};
use strand_core::store::{ConditionalStore, RangeGetStore};

/// Matches `bench/src/vector_cold_open.rs`'s `COLD_OPEN_BYTE_BUDGET` and
/// `CLAUDE.md` §7's own 100 MB figure (decimal bytes, matching that
/// section's stated decimal-MB usage throughout).
const OBJECT_BYTES: u64 = 100_000_000;

const PARALLELISM_LEVELS: &[usize] = &[1, 2, 4, 8, 16, 32, 64];
const ITERATIONS: usize = 5;
const SEED: u64 = 20_260_819;
const KEY: &str = "objects/parallel-range-fetch-payload.bin";

/// `CLAUDE.md` §7's own cited single-stream figure, vendored here as a
/// literal comparison target rather than re-derived — "a single
/// object-storage stream runs in the ~50–100 MB/s class."
const SINGLE_STREAM_ASSUMPTION_MBPS_LOW: f64 = 50.0;
const SINGLE_STREAM_ASSUMPTION_MBPS_HIGH: f64 = 100.0;

#[derive(Serialize, Clone)]
struct LevelResult {
    parallelism: usize,
    latency_ms_p50: f64,
    latency_ms_p90: f64,
    latency_ms_min: f64,
    latency_ms_max: f64,
    /// Decimal MB/s (`OBJECT_BYTES / 1e6` divided by the p50 latency in
    /// seconds) — the aggregate throughput of fetching the *whole* 100 MB
    /// object at this parallelism, comparable directly across levels since
    /// every level fetches the same total bytes, just split differently.
    aggregate_throughput_mbps_p50: f64,
}

#[derive(Serialize)]
struct ParallelRangeFetchResult {
    iterations: usize,
    object_bytes: u64,
    cold_open_byte_budget: u64,
    upload_latency_ms: f64,
    parallelism_levels: Vec<LevelResult>,
    sequential_throughput_mbps_p50: f64,
    best_parallel_throughput_mbps_p50: f64,
    best_parallelism: usize,
    speedup_factor_best_parallel_over_sequential: f64,
    claude_md_single_stream_assumption_mbps_low: f64,
    claude_md_single_stream_assumption_mbps_high: f64,
    note: String,
}

/// Splits `[0, total)` into `n` contiguous, non-overlapping half-open
/// ranges as close to equal length as integer division allows (the first
/// `total % n` ranges get one extra byte). Covers `[0, total)` exactly
/// once, asserted by the caller summing fetched lengths.
fn split_ranges(total: u64, n: usize) -> Vec<(u64, u64)> {
    let n = n as u64;
    let base = total / n;
    let remainder = total % n;
    let mut ranges = Vec::with_capacity(n as usize);
    let mut start = 0u64;
    for i in 0..n {
        let len = base + u64::from(i < remainder);
        let end = start + len;
        ranges.push((start, end));
        start = end;
    }
    assert_eq!(start, total, "range split must cover [0, total) exactly");
    ranges
}

/// Fetches `key`'s full `object_bytes` as `parallelism` concurrent
/// range GETs (or one plain range GET covering the whole object, when
/// `parallelism == 1` — the sequential-stream baseline), each on its own
/// OS thread with its own independent `S3Store`, and returns the wall-clock
/// elapsed milliseconds from dispatch to every range landing.
fn fetch_at_parallelism(
    endpoint: &str,
    bucket: &str,
    key: &str,
    object_bytes: u64,
    parallelism: usize,
) -> f64 {
    let ranges = split_ranges(object_bytes, parallelism);
    let (total_fetched, elapsed_ms) = timed(|| {
        std::thread::scope(|scope| {
            let handles: Vec<_> = ranges
                .iter()
                .map(|&(start, end)| {
                    scope.spawn(move || {
                        let store = store_for(endpoint, bucket);
                        let bytes = store.get_range(key, start, end).unwrap();
                        bytes.len() as u64
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).sum::<u64>()
        })
    });
    assert_eq!(
        total_fetched, object_bytes,
        "range split at parallelism={parallelism} must fetch the whole object exactly once"
    );
    elapsed_ms
}

fn main() {
    with_minio(|endpoint, bucket| {
        let store = store_for(endpoint, bucket);

        eprintln!(
            "Generating and uploading a real {}-byte object ({:.0} MB, matching CLAUDE.md \
             §7's cold-open byte budget) to real MinIO...",
            OBJECT_BYTES,
            OBJECT_BYTES as f64 / 1_000_000.0
        );
        let mut rng = StdRng::seed_from_u64(SEED);
        let mut payload = vec![0u8; OBJECT_BYTES as usize];
        rng.fill_bytes(&mut payload);
        let (_etag, upload_latency_ms) = timed(|| store.put_if_absent(KEY, &payload).unwrap());
        eprintln!("Uploaded in {upload_latency_ms:.0}ms");
        drop(payload);

        let mut level_results = Vec::with_capacity(PARALLELISM_LEVELS.len());
        for &parallelism in PARALLELISM_LEVELS {
            eprintln!("Measuring parallelism={parallelism} ({ITERATIONS} iterations)...");
            let mut latencies = Vec::with_capacity(ITERATIONS);
            for _ in 0..ITERATIONS {
                latencies.push(fetch_at_parallelism(
                    endpoint,
                    bucket,
                    KEY,
                    OBJECT_BYTES,
                    parallelism,
                ));
            }
            latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let p50 = percentile(&latencies, 0.50);
            let p90 = percentile(&latencies, 0.90);
            let throughput_mbps = (OBJECT_BYTES as f64 / 1_000_000.0) / (p50 / 1000.0);
            eprintln!(
                "  parallelism={parallelism}: p50={p50:.1}ms p90={p90:.1}ms throughput={throughput_mbps:.1} MB/s"
            );
            level_results.push(LevelResult {
                parallelism,
                latency_ms_p50: p50,
                latency_ms_p90: p90,
                latency_ms_min: latencies[0],
                latency_ms_max: latencies[latencies.len() - 1],
                aggregate_throughput_mbps_p50: throughput_mbps,
            });
        }

        let sequential = level_results
            .iter()
            .find(|r| r.parallelism == 1)
            .expect("parallelism=1 (sequential baseline) must be in PARALLELISM_LEVELS")
            .clone();
        let best = level_results
            .iter()
            .max_by(|a, b| {
                a.aggregate_throughput_mbps_p50
                    .partial_cmp(&b.aggregate_throughput_mbps_p50)
                    .unwrap()
            })
            .unwrap()
            .clone();

        let result = ParallelRangeFetchResult {
            iterations: ITERATIONS,
            object_bytes: OBJECT_BYTES,
            cold_open_byte_budget: OBJECT_BYTES,
            upload_latency_ms,
            parallelism_levels: level_results,
            sequential_throughput_mbps_p50: sequential.aggregate_throughput_mbps_p50,
            best_parallel_throughput_mbps_p50: best.aggregate_throughput_mbps_p50,
            best_parallelism: best.parallelism,
            speedup_factor_best_parallel_over_sequential: best.aggregate_throughput_mbps_p50
                / sequential.aggregate_throughput_mbps_p50,
            claude_md_single_stream_assumption_mbps_low: SINGLE_STREAM_ASSUMPTION_MBPS_LOW,
            claude_md_single_stream_assumption_mbps_high: SINGLE_STREAM_ASSUMPTION_MBPS_HIGH,
            note: "Real MinIO on localhost, no injected network latency (same caveat as \
                   bench/src/cold_open.rs and bench/src/vector_cold_open.rs): the absolute \
                   MB/s and millisecond figures here do not reproduce real-S3 network \
                   conditions and are not claimed to. What is real: the sequential-vs-parallel \
                   comparison against one real, unsimulated backend."
                .to_string(),
        };

        println!(
            "parallel range fetch: {} MB object, sequential p50={:.1}ms ({:.1} MB/s), \
             best parallel = {}-way p50={:.1}ms ({:.1} MB/s), speedup={:.2}x (n={ITERATIONS} \
             per level, localhost MinIO)",
            result.object_bytes / 1_000_000,
            sequential.latency_ms_p50,
            result.sequential_throughput_mbps_p50,
            result.best_parallelism,
            best.latency_ms_p50,
            result.best_parallel_throughput_mbps_p50,
            result.speedup_factor_best_parallel_over_sequential,
        );

        write_report("parallel-range-fetch", result);
    });
}
