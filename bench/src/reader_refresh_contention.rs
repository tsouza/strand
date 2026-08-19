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

//! The reader 404-refresh retry bound (RFC 0001 §3, `manifest::
//! READER_REFRESH_RETRY_LIMIT`) is required to exist but was never pinned
//! against real data — RFC 0001's own Open Questions section says the M0
//! crash tests "should produce a recommended default, not just prove the
//! path exists." `crates/strand-core/tests/s3_store.rs`'s
//! `reader_on_a_compacted_snapshot_recovers_against_real_object_storage`
//! proves the path exists (one hand-scheduled 404 race). This benchmark
//! produces the number: real, concurrent writers committing back-to-back
//! against real MinIO, a compactor deleting each snapshot the instant a
//! newer one becomes current (the tightest race window the deletion-safety
//! rule, `CLAUDE.md` §6, allows — a snapshot is deletable exactly as soon as
//! it stops being current), and concurrent readers hammering
//! `read_snapshot` throughout, recording how many internal refresh
//! iterations each real call actually needed.
//!
//! A `read_snapshot` call's internal attempt count is recovered from
//! `CountingStore`'s GET count rather than a dedicated instrumentation hook:
//! `try_read_current` issues exactly 2 GETs per attempt once a table has any
//! commits (pointer, then the snapshot it names) — the `NoCommitsYet` 1-GET
//! case never applies here, since a baseline commit lands before any reader
//! starts. So `attempts = get_count / 2` and `retries = attempts - 1`.

use serde::Serialize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use strand_bench::{CountingStore, percentile, store_for, timed, with_minio, write_report};
use strand_core::container::{ChunkCodec, StorageClass, Tier};
use strand_core::manifest::{READER_REFRESH_RETRY_LIMIT, ReadError, commit, read_snapshot};
use strand_core::segment::{BlobSpec, SegmentBuilder, write_segment};
use strand_core::store::ConditionalStore;

const WRITERS: usize = 4;
const COMMITS_PER_WRITER: usize = 15;
const READERS: usize = 4;
const CURRENT_POINTER_KEY: &str = "_strand/current";

#[derive(Serialize)]
struct ReaderRefreshContentionResult {
    writers: usize,
    commits_per_writer: usize,
    total_commits: usize,
    readers: usize,
    total_reads: usize,
    total_wall_ms: f64,
    reads_needing_at_least_one_retry: usize,
    max_retries_observed: u64,
    retries_p50: f64,
    retries_p90: f64,
    retries_p99: f64,
    retries_exhausted_count: u64,
    current_retry_limit: u32,
}

fn toy_builder(tag: u8) -> SegmentBuilder {
    let mut builder = SegmentBuilder::new(1);
    builder.add_blob(BlobSpec {
        family_id: 0,
        field_id: 0,
        blob_type_id: 0,
        storage_class: StorageClass::RawMappable,
        tier: Tier::NotApplicable,
        alignment: 8,
        chunk_codec: ChunkCodec::None,
        chunk_codec_level: 0,
        data: vec![tag, 0x00, 0x00, 0x00],
    });
    builder
}

/// Runs one writer: `COMMITS_PER_WRITER` real commits, back to back, with no
/// artificial delay — the tightest legal commit rate a single writer can
/// sustain against real MinIO, which is exactly what maximizes how often the
/// compactor observes a pointer transition to race readers against.
fn run_writer(endpoint: &str, bucket: &str, writer_id: usize) {
    let store = store_for(endpoint, bucket);
    for i in 0..COMMITS_PER_WRITER {
        let attempt = std::cell::Cell::new(0u32);
        commit(&store, |next_row_id| {
            let this_attempt = attempt.get();
            attempt.set(this_attempt + 1);
            vec![write_segment(
                &store,
                &format!("segments/w{writer_id}-{i}-{this_attempt}.bin"),
                &toy_builder(writer_id as u8),
                next_row_id,
            )]
        })
        .unwrap();
    }
}

/// The compactor: deletes a snapshot the instant it observes the pointer
/// has moved past it. This is the tightest legal race window the
/// deletion-safety rule (`CLAUDE.md` §6) allows — a snapshot becomes
/// deletable the moment a newer one is current, and this compactor acts on
/// that instant rather than waiting, to maximize how often a concurrent
/// reader's pointer-read-then-snapshot-fetch sequence straddles a deletion.
fn run_compactor(endpoint: &str, bucket: &str, done: &AtomicBool) {
    let store = store_for(endpoint, bucket);
    let mut last_seen: Option<String> = None;
    while !done.load(Ordering::Relaxed) {
        let Ok(Some((bytes, _))) = store.get(CURRENT_POINTER_KEY) else {
            continue;
        };
        let current = String::from_utf8(bytes).unwrap();
        match &last_seen {
            Some(prev) if *prev != current => {
                // Ignore delete errors: another compactor pass or the
                // benchmark's teardown may have already removed it, and a
                // missing object is not a correctness problem here (unlike
                // the real M3 orphan-sweep tool, this bench has only one
                // compactor thread, but the loop structure is written to
                // stay correct even if that ever changes).
                let _ = store.delete(prev);
                last_seen = Some(current);
            }
            None => last_seen = Some(current),
            _ => {}
        }
    }
}

/// One reader: calls `read_snapshot` in a tight loop until `done`, recording
/// the internal retry count of every real call. Returns the retry counts
/// observed and how many calls exhausted the retry bound.
fn run_reader(endpoint: &str, bucket: &str, done: &AtomicBool) -> (Vec<u64>, u64) {
    let store = store_for(endpoint, bucket);
    let counting = CountingStore::new(&store);
    let mut retries = Vec::new();
    let mut exhausted = 0u64;
    while !done.load(Ordering::Relaxed) {
        counting.reset();
        match read_snapshot(&counting) {
            Ok(Some(_)) => {
                let gets = counting.get_count();
                assert_eq!(
                    gets % 2,
                    0,
                    "every attempt after the baseline commit issues exactly 2 GETs \
                     (pointer, then the snapshot it names); got {gets}"
                );
                let attempts = gets / 2;
                retries.push(attempts - 1);
            }
            Ok(None) => panic!(
                "baseline commit landed before any reader started; \
                                 NoCommitsYet should be unreachable"
            ),
            Err(ReadError::RetriesExhausted) => {
                exhausted += 1;
                retries.push(u64::from(READER_REFRESH_RETRY_LIMIT) + 1);
            }
            Err(ReadError::Io(msg)) => panic!("unexpected I/O error against real MinIO: {msg}"),
        }
    }
    (retries, exhausted)
}

fn main() {
    with_minio(|endpoint, bucket| {
        let store = store_for(endpoint, bucket);
        commit(&store, |next_row_id| {
            vec![write_segment(
                &store,
                "segments/baseline.bin",
                &toy_builder(0xFF),
                next_row_id,
            )]
        })
        .unwrap();

        let done = AtomicBool::new(false);
        let all_retries: Mutex<Vec<u64>> = Mutex::new(Vec::new());
        let total_exhausted = AtomicU64::new(0);

        let (_, total_wall_ms) = timed(|| {
            std::thread::scope(|scope| {
                let compactor = scope.spawn(|| run_compactor(endpoint, bucket, &done));
                let reader_handles: Vec<_> = (0..READERS)
                    .map(|_| scope.spawn(|| run_reader(endpoint, bucket, &done)))
                    .collect();
                let writer_handles: Vec<_> = (0..WRITERS)
                    .map(|writer_id| scope.spawn(move || run_writer(endpoint, bucket, writer_id)))
                    .collect();

                for h in writer_handles {
                    h.join().unwrap();
                }
                done.store(true, Ordering::Relaxed);

                for h in reader_handles {
                    let (retries, exhausted) = h.join().unwrap();
                    all_retries.lock().unwrap().extend(retries);
                    total_exhausted.fetch_add(exhausted, Ordering::Relaxed);
                }
                compactor.join().unwrap();
            });
        });

        let total_commits = WRITERS * COMMITS_PER_WRITER;
        let verify_store = store_for(endpoint, bucket);
        let final_snapshot = read_snapshot(&verify_store).unwrap().unwrap();
        assert_eq!(
            final_snapshot.segments.len(),
            total_commits + 1,
            "every writer's every commit, plus the baseline, must survive \
             concurrent compaction and reading"
        );

        let mut retries = all_retries.into_inner().unwrap();
        assert!(!retries.is_empty(), "no reads were recorded");
        retries.sort_unstable();
        let retries_f64: Vec<f64> = retries.iter().map(|&r| r as f64).collect();

        let result = ReaderRefreshContentionResult {
            writers: WRITERS,
            commits_per_writer: COMMITS_PER_WRITER,
            total_commits,
            readers: READERS,
            total_reads: retries.len(),
            total_wall_ms,
            reads_needing_at_least_one_retry: retries.iter().filter(|&&r| r > 0).count(),
            max_retries_observed: *retries.last().unwrap(),
            retries_p50: percentile(&retries_f64, 0.50),
            retries_p90: percentile(&retries_f64, 0.90),
            retries_p99: percentile(&retries_f64, 0.99),
            retries_exhausted_count: total_exhausted.load(Ordering::Relaxed),
            current_retry_limit: READER_REFRESH_RETRY_LIMIT,
        };

        println!(
            "reader refresh contention: {} reads, max retries={}, p99={:.1}, \
             {} reads needed >=1 retry, {} exhausted the bound (limit={}), {:.1}ms wall",
            result.total_reads,
            result.max_retries_observed,
            result.retries_p99,
            result.reads_needing_at_least_one_retry,
            result.retries_exhausted_count,
            result.current_retry_limit,
            result.total_wall_ms,
        );

        write_report("reader-refresh-contention", result);
    });
}
