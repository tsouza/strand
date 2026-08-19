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

//! RFC 0001 §1's open protocol names two unpinned numbers: the speculative
//! tail-read size `N` and, implicitly, a hotcache-size ceiling writers
//! should stay under to keep opens at one RTT. The RFC's own Open Questions
//! section says both "need M0 MinIO benchmark data before either is stated
//! as more than provisional." This benchmark produces that data: it builds
//! real segments across a sweep of blob counts (`spec/container.md` §9's
//! registry currently tops out at 12 blob entries for one field spanning
//! every registered family — lexical, filter, vector, deletion — so the
//! sweep runs well past that into synthetic territory, since the format
//! itself imposes no cap and `CLAUDE.md` §7's segment-count discussion
//! already anticipates multi-field growth via X-1), commits each to real
//! MinIO, and executes RFC 0001 §1's actual two-phase open protocol —
//! `S3Store::get_tail_range` for the speculative tail read, a real `Footer`/
//! `Hotcache` decode, and a conditional second range GET — against each,
//! for a sweep of candidate `N` values.
//!
//! The check for whether the whole hotcache landed in the speculative
//! window is RFC 0001 §1's own stated one: `hotcache_length + 40 <= N`.

use serde::Serialize;
use strand_bench::{store_for, timed, with_minio, write_report};
use strand_core::container::{ChunkCodec, Footer, Hotcache, StorageClass, Tier};
use strand_core::manifest::commit;
use strand_core::s3_store::S3Store;
use strand_core::segment::{BlobSpec, SegmentBuilder, write_segment};

/// Blob counts to sweep. 12 is today's real maximum for one field spanning
/// every currently-registered family (`spec/container.md` §9: 5 lexical +
/// 2 filter + 4 vector + 1 deletion). The rest are synthetic stress points
/// standing in for multi-field growth (X-1, `docs/roadmap.md`), since the
/// container format itself places no cap on blob count.
const BLOB_COUNTS: &[usize] = &[1, 12, 50, 100, 250, 500, 1000];

/// Candidate speculative tail-read sizes, bytes.
const CANDIDATE_N: &[u64] = &[512, 1024, 2048, 4096, 8192, 16384];

const ITERATIONS: usize = 10;

#[derive(Serialize)]
struct OpenTrial {
    blob_count: usize,
    hotcache_length: u64,
    /// `hotcache_length + 40` — the total trailing region (hotcache plus
    /// footer trailer) a tail read must cover to land the whole hotcache in
    /// one RTT, per RFC 0001 §1's own stated check.
    trailing_region_bytes: u64,
    n: u64,
    one_rtt: bool,
    rtts_used_p50: f64,
    latency_ms_p50: f64,
    latency_ms_p90: f64,
}

#[derive(Serialize)]
struct HotcacheTailReadResult {
    iterations_per_trial: usize,
    trials: Vec<OpenTrial>,
}

/// Builds and commits a segment with `blob_count` tiny raw-mappable blobs
/// (8 bytes each, 8-byte aligned — the same shape RFC 0001 §7's worked
/// example uses), returning its `SegmentRef`.
fn build_and_commit(
    store: &S3Store,
    path: &str,
    blob_count: usize,
) -> strand_core::manifest::SegmentRef {
    commit(store, |next_row_id| {
        let mut builder = SegmentBuilder::new(1);
        for i in 0..blob_count {
            builder.add_blob(BlobSpec {
                family_id: 1,
                blob_type_id: (i % 5) as u16,
                storage_class: StorageClass::RawMappable,
                tier: Tier::NotApplicable,
                alignment: 8,
                chunk_codec: ChunkCodec::None,
                chunk_codec_level: 0,
                data: vec![0x2A, 0x00, 0x00, 0x00, 0x2B, 0x00, 0x00, 0x00],
            });
        }
        vec![write_segment(store, path, &builder, next_row_id)]
    })
    .unwrap()
    .segments
    .into_iter()
    .find(|s| s.path == path)
    .unwrap()
}

/// Executes RFC 0001 §1's actual two-phase open protocol against a real,
/// already-committed segment: a speculative tail read of `n` bytes, a real
/// footer decode, and — only if the check fails — a second range GET for
/// the rest of the hotcache. Returns the number of RTTs used (1 or 2) and
/// decodes the hotcache fully either way, so a wrong answer here would be a
/// real correctness bug, not just a slow one.
fn open_with_tail_read(store: &S3Store, path: &str, byte_length: u64, n: u64) -> u32 {
    let tail_start = byte_length.saturating_sub(n);
    let window = store
        .get_tail_range(path, tail_start, byte_length - 1)
        .unwrap()
        .expect("segment must exist");

    let footer_start = window.len() - 40;
    let footer_bytes: [u8; 40] = window[footer_start..].try_into().unwrap();
    let footer = Footer::decode(&footer_bytes).unwrap();

    if footer.hotcache_length + 40 <= n {
        let hotcache_start_in_window = (footer.hotcache_offset - tail_start) as usize;
        let hotcache_end_in_window = hotcache_start_in_window + footer.hotcache_length as usize;
        Hotcache::decode(&window[hotcache_start_in_window..hotcache_end_in_window]).unwrap();
        1
    } else {
        let hotcache_bytes = store
            .get_tail_range(path, footer.hotcache_offset, byte_length - 40 - 1)
            .unwrap()
            .expect("segment must exist");
        Hotcache::decode(&hotcache_bytes).unwrap();
        2
    }
}

fn main() {
    with_minio(|endpoint, bucket| {
        let store = store_for(endpoint, bucket);
        let mut trials = Vec::new();

        for &blob_count in BLOB_COUNTS {
            let path = format!("segments/sweep-{blob_count}.bin");
            let segment_ref = build_and_commit(&store, &path, blob_count);
            let hotcache_length = 20 + 34 * blob_count as u64;
            let trailing_region_bytes = hotcache_length + 40;

            for &n in CANDIDATE_N {
                let mut rtts = Vec::with_capacity(ITERATIONS);
                let mut latencies = Vec::with_capacity(ITERATIONS);
                for _ in 0..ITERATIONS {
                    let (used, elapsed_ms) =
                        timed(|| open_with_tail_read(&store, &path, segment_ref.byte_length, n));
                    rtts.push(used);
                    latencies.push(elapsed_ms);
                }
                latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let one_rtt = trailing_region_bytes <= n;
                assert!(
                    rtts.iter().all(|&r| r == if one_rtt { 1 } else { 2 }),
                    "RTT count must be constant and match the predicted check \
                     for blob_count={blob_count} n={n}: {rtts:?}"
                );

                trials.push(OpenTrial {
                    blob_count,
                    hotcache_length,
                    trailing_region_bytes,
                    n,
                    one_rtt,
                    rtts_used_p50: f64::from(rtts[ITERATIONS / 2]),
                    latency_ms_p50: latencies[ITERATIONS / 2],
                    latency_ms_p90: latencies[(ITERATIONS * 9) / 10],
                });
            }
        }

        for t in &trials {
            println!(
                "blob_count={} hotcache={}B trailing={}B N={}B -> {} RTT, p50={:.2}ms p90={:.2}ms",
                t.blob_count,
                t.hotcache_length,
                t.trailing_region_bytes,
                t.n,
                if t.one_rtt { 1 } else { 2 },
                t.latency_ms_p50,
                t.latency_ms_p90,
            );
        }

        write_report(
            "hotcache-tail-read",
            HotcacheTailReadResult {
                iterations_per_trial: ITERATIONS,
                trials,
            },
        );
    });
}
