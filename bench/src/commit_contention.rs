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

//! Manifest commit contention: real concurrent writers (independent OS
//! threads, each its own `S3Store`/Tokio runtime, deliberately not sharing
//! one runtime across threads) racing the pointer against real MinIO — the
//! `docs/milestones.md` M0 benchmark. Correctness (every writer's every
//! commit lands, with non-overlapping row-ID ranges) is asserted, not just
//! latency measured: a benchmark that silently corrupted data would be a
//! worse result than a slow one.

use serde::Serialize;
use std::cell::Cell;
use strand_bench::{CountingStore, store_for, timed, with_minio, write_report};
use strand_core::container::{ChunkCodec, StorageClass, Tier};
use strand_core::manifest::{commit, read_snapshot};
use strand_core::segment::{BlobSpec, SegmentBuilder, write_segment};

const WRITERS: usize = 8;
const COMMITS_PER_WRITER: usize = 5;

#[derive(Serialize)]
struct CommitContentionResult {
    writers: usize,
    commits_per_writer: usize,
    total_commits: usize,
    total_wall_ms: f64,
    total_pointer_put_attempts: u64,
    extra_put_attempts_from_retries: u64,
}

fn run_writer(endpoint: &str, bucket: &str, writer_id: usize) -> u64 {
    let store = store_for(endpoint, bucket);
    let counting = CountingStore::new(&store);

    for i in 0..COMMITS_PER_WRITER {
        // `build_segments` can run more than once per commit (RFC 0001 §3:
        // every retry recomputes from fresh state), so the segment path
        // must be attempt-unique, not just (writer_id, i)-unique — a fixed
        // path collides with its own first attempt's already-written
        // object on retry. Found by hitting exactly that collision the
        // first time this benchmark ran, not by reasoning about it up
        // front; see the note added to manifest::commit's own docs.
        let attempt = Cell::new(0u32);
        commit(&counting, |next_row_id| {
            let this_attempt = attempt.get();
            attempt.set(this_attempt + 1);

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
                data: vec![writer_id as u8, i as u8, 0x00, 0x00],
            });
            vec![write_segment(
                &counting,
                &format!("segments/w{writer_id}-{i}-{this_attempt}.bin"),
                &builder,
                next_row_id,
            )]
        })
        .unwrap();
    }

    counting.put_count()
}

fn main() {
    with_minio(|endpoint, bucket| {
        let (put_counts, total_wall_ms) = timed(|| {
            std::thread::scope(|scope| {
                let handles: Vec<_> = (0..WRITERS)
                    .map(|writer_id| scope.spawn(move || run_writer(endpoint, bucket, writer_id)))
                    .collect();
                handles
                    .into_iter()
                    .map(|h| h.join().unwrap())
                    .collect::<Vec<u64>>()
            })
        });

        let total_commits = WRITERS * COMMITS_PER_WRITER;
        // Each successful commit makes exactly 2 PUTs when it wins on the
        // first try (the snapshot object, then the pointer): a total above
        // 2 * total_commits means some attempts lost the pointer CAS and
        // retried, per RFC 0001 §3.
        let total_pointer_put_attempts: u64 = put_counts.iter().sum();
        let extra_put_attempts_from_retries =
            total_pointer_put_attempts.saturating_sub(2 * total_commits as u64);

        let verify_store = store_for(endpoint, bucket);
        let snapshot = read_snapshot(&verify_store).unwrap().unwrap();
        assert_eq!(
            snapshot.segments.len(),
            total_commits,
            "every writer's every commit must be present in the final snapshot"
        );

        let mut ranges: Vec<(u64, u64)> = snapshot
            .segments
            .iter()
            .map(|s| (s.row_id_base, s.row_id_base + s.row_id_count))
            .collect();
        ranges.sort();
        for pair in ranges.windows(2) {
            assert!(
                pair[0].1 <= pair[1].0,
                "row-ID ranges must not overlap under real concurrency: {pair:?}"
            );
        }

        let result = CommitContentionResult {
            writers: WRITERS,
            commits_per_writer: COMMITS_PER_WRITER,
            total_commits,
            total_wall_ms,
            total_pointer_put_attempts,
            extra_put_attempts_from_retries,
        };

        println!(
            "commit contention: {WRITERS} writers x {COMMITS_PER_WRITER} commits = {total_commits} total, \
             {total_wall_ms:.1}ms wall, {extra_put_attempts_from_retries} retry-driven extra PUTs, \
             all row-ID ranges verified non-overlapping"
        );

        write_report("commit-contention", result);
    });
}
