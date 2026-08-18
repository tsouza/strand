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

//! Cold end-to-end open: pointer GET, snapshot GET, then a segment open
//! (footer + hotcache, ≤2 RTT per invariant 3) — the `docs/milestones.md`
//! M0 benchmark, and the measurement that confirms or replaces the §7
//! placeholder tail-latency figure. GET count is asserted, not just
//! measured, since CLAUDE.md §7 requires cold-path claims justified in
//! GETs and bytes, not only wall-clock time.

use serde::Serialize;
use strand_bench::{CountingStore, percentile, store_for, timed, with_minio, write_report};
use strand_core::container::{ChunkCodec, Footer, Hotcache, StorageClass, Tier};
use strand_core::manifest::{commit, read_snapshot};
use strand_core::segment::{BlobSpec, SegmentBuilder, write_segment};
use strand_core::store::ConditionalStore;

const ITERATIONS: usize = 30;

#[derive(Serialize)]
struct ColdOpenResult {
    iterations: usize,
    get_count_per_open: u64,
    latency_ms_p50: f64,
    latency_ms_p90: f64,
    latency_ms_p99: f64,
    latency_ms_min: f64,
    latency_ms_max: f64,
}

fn main() {
    with_minio(|endpoint, bucket| {
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
                "segments/cold-open.bin",
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

        let result = ColdOpenResult {
            iterations: ITERATIONS,
            get_count_per_open,
            latency_ms_p50: percentile(&latencies, 0.50),
            latency_ms_p90: percentile(&latencies, 0.90),
            latency_ms_p99: percentile(&latencies, 0.99),
            latency_ms_min: latencies[0],
            latency_ms_max: latencies[latencies.len() - 1],
        };

        println!(
            "cold open: {get_count_per_open} GETs/open, p50={:.2}ms p90={:.2}ms p99={:.2}ms (n={ITERATIONS})",
            result.latency_ms_p50, result.latency_ms_p90, result.latency_ms_p99
        );

        write_report("cold-open", result);
    });
}
