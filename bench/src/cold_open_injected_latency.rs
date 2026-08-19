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

//! Roadmap X-4: the real-network cold-open tail-latency measurement
//! `CLAUDE.md` §7 and `docs/ledger.md` flag as still open.
//! `bench/src/cold_open.rs` measures the identical open protocol (pointer
//! GET, snapshot GET, one-RTT segment open) against MinIO on localhost with
//! no injected latency; that confirms the **GET-count** half of invariant 3
//! but, by its own module doc, is "not yet a real-network tail figure."
//! This binary runs the same protocol against the same kind of MinIO
//! container with a real `netem` delay injected onto its network interface
//! (`strand_bench::inject_netem_delay`, `bench/src/lib.rs`) — real S3
//! credentials are not available in this environment, so injected-latency
//! MinIO is the closest real substitute, per `docs/benchmarks.md`'s own
//! "tested against local MinIO with injected latency" line.
//!
//! `DELAY_MS` is chosen to land the *measured* round-trip time — not the
//! injected one-way parameter — at `CLAUDE.md` §7's pinned ~100ms planning
//! figure. `inject_netem_delay`'s doc comment explains why: the injection
//! is egress-only (asymmetric), so on the keep-alive HTTP connection the
//! AWS SDK reuses across this benchmark's repeated GETs, each request pays
//! the delay once (the response leg), not twice — confirmed directly
//! against a real MinIO container before this benchmark was written. A
//! `netem` value of 100ms therefore targets a ~100ms measured round trip
//! per warm request, with the first (connection-establishing) request in
//! each cold-reopen paying roughly double.
const DELAY_MS: u64 = 100;

const ITERATIONS: usize = 30;

use serde::Serialize;
use strand_bench::{CountingStore, percentile, store_for, timed, with_minio_latency, write_report};
use strand_core::container::{ChunkCodec, Footer, Hotcache, StorageClass, Tier};
use strand_core::manifest::{commit, read_snapshot};
use strand_core::segment::{BlobSpec, SegmentBuilder, write_segment};
use strand_core::store::ConditionalStore;

#[derive(Serialize)]
struct ColdOpenInjectedLatencyResult {
    injected_netem_delay_ms: u64,
    iterations: usize,
    get_count_per_open: u64,
    latency_ms_p50: f64,
    latency_ms_p90: f64,
    latency_ms_p99: f64,
    latency_ms_min: f64,
    latency_ms_max: f64,
}

fn main() {
    with_minio_latency(DELAY_MS, |endpoint, bucket| {
        let store = store_for(endpoint, bucket);

        commit(&store, |next_row_id| {
            let mut builder = SegmentBuilder::new(2);
            builder.add_blob(BlobSpec {
                family_id: 0,
                blob_type_id: 0,
                storage_class: StorageClass::RawMappable,
                tier: Tier::NotApplicable,
                alignment: 8,
                chunk_codec: ChunkCodec::None,
                chunk_codec_level: 0,
                data: vec![0x2A, 0x00, 0x00, 0x00, 0x2B, 0x00, 0x00, 0x00],
            });
            vec![write_segment(
                &store,
                "segments/cold-open-injected-latency.bin",
                &builder,
                next_row_id,
            )]
        })
        .unwrap();

        let counting = CountingStore::new(&store);
        let mut latencies = Vec::with_capacity(ITERATIONS);
        let mut get_counts = Vec::with_capacity(ITERATIONS);

        for _ in 0..ITERATIONS {
            counting.reset();
            let (_, elapsed_ms) = timed(|| {
                let snapshot = read_snapshot(&counting).unwrap().unwrap();
                let segment_ref = &snapshot.segments[0];
                let (segment_bytes, _) = counting.get(&segment_ref.path).unwrap().unwrap();

                let footer_start = segment_bytes.len() - 40;
                let footer_bytes: [u8; 40] = segment_bytes[footer_start..].try_into().unwrap();
                let footer = Footer::decode(&footer_bytes).unwrap();
                let hotcache_start = footer.hotcache_offset as usize;
                let hotcache_end = hotcache_start + footer.hotcache_length as usize;
                Hotcache::decode(&segment_bytes[hotcache_start..hotcache_end]).unwrap()
            });
            latencies.push(elapsed_ms);
            get_counts.push(counting.get_count());
        }

        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let get_count_per_open = get_counts[0];
        assert!(
            get_counts.iter().all(|&c| c == get_count_per_open),
            "GET count per cold open must be constant across iterations: {get_counts:?}"
        );
        assert!(
            get_count_per_open <= 4,
            "pointer + snapshot + segment open (\u{2264}2 RTT per invariant 3) should be \u{2264}4 GETs, got {get_count_per_open}"
        );

        let result = ColdOpenInjectedLatencyResult {
            injected_netem_delay_ms: DELAY_MS,
            iterations: ITERATIONS,
            get_count_per_open,
            latency_ms_p50: percentile(&latencies, 0.50),
            latency_ms_p90: percentile(&latencies, 0.90),
            latency_ms_p99: percentile(&latencies, 0.99),
            latency_ms_min: latencies[0],
            latency_ms_max: latencies[latencies.len() - 1],
        };

        println!(
            "cold open (injected {DELAY_MS}ms netem delay): {get_count_per_open} GETs/open, p50={:.2}ms p90={:.2}ms p99={:.2}ms (n={ITERATIONS})",
            result.latency_ms_p50, result.latency_ms_p90, result.latency_ms_p99
        );

        write_report("cold-open-injected-latency", result);
    });
}
