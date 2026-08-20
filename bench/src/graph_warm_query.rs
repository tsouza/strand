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

//! A real, measured baseline for the graph-blob family (RFC 0014,
//! `rfcs/0014-graph-blob-family.md`) — the gap that RFC's own Status line
//! and Napkin math section name explicitly: "every latency and fetch-count
//! figure here is literature-translated arithmetic, not a STRAND-measured
//! result." This is that measurement, following `bench/src/
//! vector_cold_open.rs`'s (RFC 0010) precedent of replacing a formula
//! estimate with a real, reproducible number, adapted to this family's own
//! different governing regime.
//!
//! **Why this benchmark does not just count S3 GETs the way
//! `vector_cold_open.rs`/`cold_open.rs` do.** The graph family is
//! registered `tier: warm` (RFC 0014 Design §7, `spec/vectors.md` §3),
//! explicitly exempted from invariant 3's one-wave, ~100ms-object-storage-
//! round-trip cold-path accounting by invariant 7's own text: "`tier: warm`
//! (assumes NVMe-class latency; graph families live here)." RFC 0014's own
//! Napkin math built its entire cost argument around DiskANN's own cited
//! "~100-300μs" retail-SSD random-read figure, not the ~100ms S3 figure.
//! Counting S3 GETs for this family's *query* path would measure the wrong
//! regime — so this benchmark's primary numbers are (1) a real fetch-count/
//! hop-count distribution from a real Vamana graph, queried through the
//! real cold-open wire format via `strand_vector::graph_query::
//! greedy_search_cold`, and (2) a real local random-read latency baseline
//! measured directly against this machine's own storage device, replacing
//! DiskANN's literature figure with a measured one where that is honestly
//! obtainable (see `measure_local_random_read_latency` below for exactly
//! what was checked and why it is a fair proxy here: this machine has a
//! real NVMe device, confirmed via `lsblk`/`rotational=0`/`dmidecode`, not
//! a virtualized or network-backed disk). A real, secondary S3 GET/byte
//! measurement is included too (Design §7 still lives in object storage —
//! a real reader might warm-cache this blob from S3 once, then serve many
//! local queries against it), following `vector_cold_open.rs`'s own
//! whole-segment-GET pattern.
//!
//! **Construction scale, chosen empirically, not assumed.** RFC 0014's own
//! task 4 unit test (`crates/strand-vector/src/graph_query.rs`) checked
//! `n=1,000, dims=32, R=16` as an ad-hoc sanity probe inside `cargo test`.
//! Before committing to a larger scale here, `build_vamana`'s own real
//! wall-clock cost was measured directly (a throwaway release-mode probe,
//! not included in this file) at several `(n, dims, R)` combinations:
//! `n=2,000, dims=128, R=64, L=100` took 80.6s; `n=4,000` at the same
//! `dims`/`R`/`L` took 181.3s (release mode, a real 2.25x-per-doubling
//! growth rate — super-linear in `n`, though not the quadratic-in-`n`
//! shape an earlier version of this comment incorrectly attributed to a
//! `robust_prune` complexity claim that does not actually appear anywhere
//! in `crate::vamana`'s own documentation; corrected here rather than left
//! standing, per `CLAUDE.md` §3. `robust_prune`'s own real loop
//! (`crates/strand-vector/src/vamana.rs`) runs at most `r` iterations,
//! each doing an `O(|remaining|·dims)` scan, so its own per-call cost is
//! roughly linear in its candidate-set size, not quadratic in it; the
//! observed 2.25x-per-doubling growth is closer to `O(n^1.15–1.3)`
//! empirically, consistent with construction's overall
//! `n`-many-GreedySearch-plus-RobustPrune-calls shape rather than any
//! single call's own asymptotics). `n=4,000` was chosen as this
//! benchmark's real construction scale: `dims=128` and `R=64` are both real
//! DiskANN-cited values (`references/diskann-neurips2019.md`: SIFT1M/
//! GIST1M uses `R=70`; DEEP1M and the SIFT1B merge-shard configuration both
//! use `R=64`; RFC 0014's own Napkin math cites "R=64-128" as the real
//! construction range) — a genuine, if modest, step up from the toy check's
//! `R=16`, `dims=32`, at 4x the node count, chosen specifically because it
//! keeps this benchmark's total real build time near three minutes rather
//! than the tens of minutes to hours `n=50,000+` measured as costing at
//! this same `R`/`dims` (the task's own warning, confirmed rather than
//! assumed). This benchmark therefore runs in release mode
//! (`cargo run -p strand-bench --release --bin graph-warm-query`) —
//! debug-mode `build_vamana` at this scale was not measured and is not
//! expected to finish in a reasonable time, the same discipline
//! `docs/roadmap.md`'s M2-6 entry already applies ("cargo test --release")
//! to another CPU-heavy measurement in this codebase.

use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::Serialize;
use std::alloc::{Layout, alloc, dealloc};
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::{FileExt, OpenOptionsExt};
use strand_bench::{CountingStore, percentile, store_for, timed, with_minio, write_report};
use strand_core::container::{Footer, Hotcache, field_id_from_name};
use strand_core::manifest::{commit, read_snapshot};
use strand_core::segment::{SegmentBuilder, write_segment};
use strand_core::store::ConditionalStore;
use strand_vector::graph_blob::{
    GraphBlobInput, NodeRecordReader, ShuffleAlgorithm, build_graph_blob_specs,
};
use strand_vector::graph_query::greedy_search_cold;
use strand_vector::reorder::{BnfConfig, bnf, overlap_ratio};
use strand_vector::vamana::{VamanaConfig, build_vamana};

// ---- Construction scale (see module doc for why these values) ----------
const CONSTRUCTION_N: usize = 4_000;
const DIMS: usize = 128;
const R: usize = 64;
const CONSTRUCTION_L: usize = 100;
const ALPHA: f32 = 1.2;
const CONSTRUCTION_SEED: u64 = 20_260_820;

const BNF_BLOCK_SIZE: usize = 32;
const BNF_BETA: usize = 10;
const BNF_TAU: f64 = 0.0;

// ---- Query workload --------------------------------------------------
// Two query-time `L` values, not one, because the first full run at
// `L = CONSTRUCTION_L = 100` (this file's own git history) found every
// query visiting essentially the *entire* search list before terminating
// (measured hops 100-104, i.e. `L`'s own ceiling) — `L = 100` against an
// `n = 4,000`-point corpus is 2.5% of the whole dataset, an unusually
// large query-time search list relative to corpus size, and this
// benchmark's synthetic uniform-random points (no cluster structure) give
// `GreedySearch` little gradient to converge early on, unlike a real
// embedding dataset. `32` is included as the smaller, more typical
// query-time search list DiskANN's own SIFT1M-scale usage suggests for
// `k = 10` (`references/diskann-neurips2019.md`'s own build-time `L`
// values run 70-125; query-time `L` is conventionally smaller, closer to
// `3-5x k`); `100` is kept too so the saturation behavior itself is a
// real, reported number rather than silently avoided. Both are real
// measurements — this module's own report and docs state plainly which
// regime each falls into rather than picking only the flattering one.
const QUERY_L_VALUES: [usize; 2] = [32, 100];
const NUM_QUERIES: usize = 300;
const QUERY_K: usize = 10;
const QUERY_SEED_BASE: u64 = 900_000;

// ---- Local random-read latency probe -------------------------------------
const LOCAL_READ_FILE_BYTES: u64 = 256 * 1024 * 1024; // 256 MiB
const LOCAL_READ_BLOCK_BYTES: usize = 4096; // one physical block/page
const LOCAL_READ_SAMPLES: usize = 2_000;
const LOCAL_READ_SEED: u64 = 555_001;

/// DiskANN's own cited retail-grade-SSD random-read range
/// (`references/diskann-neurips2019.md` §1: "a few hundred microseconds...
/// ∼300K random reads per second"), used as a literature cross-check and,
/// if a real local measurement is not obtainable in this environment, as
/// the honest fallback (`CLAUDE.md` §7's own discipline for the ~100ms S3
/// figure it could not independently re-derive).
const DISKANN_CITED_LOW_US: f64 = 100.0;
const DISKANN_CITED_HIGH_US: f64 = 300.0;

const S3_ITERATIONS: usize = 10;

/// A `libc::O_DIRECT`-aligned buffer: allocated with the alignment
/// `O_DIRECT` requires (the read length, `LOCAL_READ_BLOCK_BYTES`, doubles
/// as the alignment here since both are 4096, the common NVMe logical block
/// size confirmed against this machine's own device by the `dd
/// iflag=direct` probe this module's doc references).
///
/// # Safety discipline
///
/// `alloc`/`dealloc` are used directly (not a `Vec<u8>`) because `Vec`
/// gives no alignment guarantee beyond `align_of::<u8>() == 1`, and
/// `O_DIRECT` reads into a misaligned buffer fail with `EINVAL`. The
/// `unsafe` blocks below are the entire surface: `alloc` with a
/// `Layout` whose size and alignment are both `LOCAL_READ_BLOCK_BYTES`
/// (a valid power of two), and `slice::from_raw_parts_mut` over exactly
/// that allocation, borrowed through `&mut self` so no aliasing is
/// possible. `Drop` frees with the same `Layout` `new` allocated with.
struct AlignedBuffer {
    ptr: *mut u8,
    layout: Layout,
}

impl AlignedBuffer {
    fn new(len: usize, align: usize) -> Self {
        let layout = Layout::from_size_align(len, align).expect("valid alignment");
        // SAFETY: layout has nonzero size and a power-of-two alignment.
        let ptr = unsafe { alloc(layout) };
        assert!(!ptr.is_null(), "aligned allocation failed");
        AlignedBuffer { ptr, layout }
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: `ptr` was allocated above for exactly `layout.size()`
        // bytes and is exclusively borrowed via `&mut self`.
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.layout.size()) }
    }
}

impl Drop for AlignedBuffer {
    fn drop(&mut self) {
        // SAFETY: `ptr`/`layout` are exactly what `new` allocated with.
        unsafe { dealloc(self.ptr, self.layout) }
    }
}

#[derive(Serialize)]
struct LocalReadLatency {
    available: bool,
    /// Populated only when `available` is false — why a real measurement
    /// could not be taken here.
    unavailable_reason: Option<String>,
    device_description: String,
    samples: usize,
    block_bytes: usize,
    p50_us: f64,
    p90_us: f64,
    p99_us: f64,
    mean_us: f64,
    min_us: f64,
    max_us: f64,
}

/// Measures real random-read latency against this machine's own storage
/// device, bypassing the page cache via `O_DIRECT` — a real, local
/// substitute for DiskANN's own cited "~100-300μs" literature figure,
/// taken because this environment's own disk was confirmed to be a real,
/// physical NVMe device before relying on it: `lsblk` shows `nvme0n1` as
/// the backing device for `/` (via LVM), `/sys/block/nvme0n1/queue/
/// rotational` reads `0`, and `/sys/class/dmi/id/{sys_vendor,product_name}`
/// report "ASUSTeK COMPUTER INC." / "MINIPC PN62" — a real mini-PC
/// motherboard, not a cloud hypervisor's virtual-disk identity string, so
/// this is a real local NVMe device, not virtualized or network-backed
/// storage, and is a fair proxy for invariant 7's "NVMe-class latency"
/// assumption. `O_DIRECT` support on this filesystem (ext4-on-LVM) was
/// separately confirmed with `dd ... oflag=direct`/`iflag=direct` before
/// this code was written, so failure here is treated as a real, reportable
/// environment fact (recorded in `unavailable_reason`), not silently
/// swallowed.
///
/// The temp file is created inside the bench crate's own directory
/// (`CARGO_MANIFEST_DIR`), not `/tmp` — `/tmp` is `tmpfs` (RAM-backed) on
/// many Linux systems, which does not support `O_DIRECT` and would silently
/// measure RAM speed instead of disk speed; this repository's own checkout
/// is confirmed to live on the real ext4-on-NVMe filesystem (`df -T`).
fn measure_local_random_read_latency() -> LocalReadLatency {
    let device_description = "nvme0n1 (real NVMe, rotational=0), ASUSTeK MINIPC PN62, ext4-on-LVM \
         (confirmed via lsblk/dmidecode/df -T before this benchmark was written)"
        .to_string();

    let dir = env!("CARGO_MANIFEST_DIR");
    let temp = match tempfile::Builder::new()
        .prefix("graph-warm-query-odirect-")
        .tempfile_in(dir)
    {
        Ok(t) => t,
        Err(e) => {
            return LocalReadLatency {
                available: false,
                unavailable_reason: Some(format!("could not create temp file in {dir}: {e}")),
                device_description,
                samples: 0,
                block_bytes: LOCAL_READ_BLOCK_BYTES,
                p50_us: f64::NAN,
                p90_us: f64::NAN,
                p99_us: f64::NAN,
                mean_us: f64::NAN,
                min_us: f64::NAN,
                max_us: f64::NAN,
            };
        }
    };
    let path = temp.path().to_path_buf();

    // Fill the file with real, non-degenerate content via a plain buffered
    // writer (no O_DIRECT needed for the write side — only the timed random
    // reads below need to bypass the page cache).
    {
        let mut rng = StdRng::seed_from_u64(LOCAL_READ_SEED);
        let mut writer = std::io::BufWriter::new(temp.reopen().expect("reopen for write"));
        let chunk_len = 1024 * 1024;
        let mut chunk = vec![0u8; chunk_len];
        let mut written = 0u64;
        while written < LOCAL_READ_FILE_BYTES {
            rng.fill(chunk.as_mut_slice());
            writer.write_all(&chunk).expect("write temp file chunk");
            written += chunk_len as u64;
        }
        writer.flush().expect("flush temp file");
    }

    let reader = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECT)
        .open(&path);
    let file = match reader {
        Ok(f) => f,
        Err(e) => {
            return LocalReadLatency {
                available: false,
                unavailable_reason: Some(format!(
                    "O_DIRECT open failed on this filesystem ({e}); falling back to DiskANN's \
                     own cited literature figure"
                )),
                device_description,
                samples: 0,
                block_bytes: LOCAL_READ_BLOCK_BYTES,
                p50_us: f64::NAN,
                p90_us: f64::NAN,
                p99_us: f64::NAN,
                mean_us: f64::NAN,
                min_us: f64::NAN,
                max_us: f64::NAN,
            };
        }
    };

    let num_blocks = LOCAL_READ_FILE_BYTES / LOCAL_READ_BLOCK_BYTES as u64;
    let mut rng = StdRng::seed_from_u64(LOCAL_READ_SEED + 1);
    let mut buffer = AlignedBuffer::new(LOCAL_READ_BLOCK_BYTES, LOCAL_READ_BLOCK_BYTES);
    let mut latencies_us = Vec::with_capacity(LOCAL_READ_SAMPLES);

    for _ in 0..LOCAL_READ_SAMPLES {
        let block_index = rng.random_range(0..num_blocks);
        let offset = block_index * LOCAL_READ_BLOCK_BYTES as u64;
        let buf = buffer.as_mut_slice();
        let start = std::time::Instant::now();
        file.read_exact_at(buf, offset)
            .expect("O_DIRECT random read");
        let elapsed_us = start.elapsed().as_secs_f64() * 1_000_000.0;
        latencies_us.push(elapsed_us);
    }

    latencies_us.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean_us = latencies_us.iter().sum::<f64>() / latencies_us.len() as f64;

    LocalReadLatency {
        available: true,
        unavailable_reason: None,
        device_description,
        samples: LOCAL_READ_SAMPLES,
        block_bytes: LOCAL_READ_BLOCK_BYTES,
        p50_us: percentile(&latencies_us, 0.50),
        p90_us: percentile(&latencies_us, 0.90),
        p99_us: percentile(&latencies_us, 0.99),
        mean_us,
        min_us: latencies_us[0],
        max_us: latencies_us[latencies_us.len() - 1],
    }
}

/// One query-time `L`'s worth of the primary fetch-count/hop-count
/// measurement, over `NUM_QUERIES` real cold-open queries against the same
/// real graph blob. `main` runs one of these per entry in
/// `QUERY_L_VALUES`.
#[derive(Serialize)]
struct QuerySweepResult {
    l_param: usize,
    k: usize,
    num_queries: usize,
    mean_fetches_per_query: f64,
    p50_fetches_per_query: f64,
    p90_fetches_per_query: f64,
    p99_fetches_per_query: f64,
    max_fetches_per_query: usize,
    min_fetches_per_query: usize,
    mean_hops_per_query: f64,
    p50_hops_per_query: f64,
    p90_hops_per_query: f64,
    p99_hops_per_query: f64,
    max_hops_per_query: usize,
    min_hops_per_query: usize,
    /// `mean_hops_per_query * max_out_degree` — RFC 0014's own Napkin math
    /// pessimistic `hops × R` bound, computed against *this run's own*
    /// measured hop count rather than a literature figure.
    pessimistic_hops_times_r_bound: f64,
    /// `mean_fetches_per_query / pessimistic_hops_times_r_bound` — how much
    /// of the pessimistic bound this run's real inter-hop overlap actually
    /// spends.
    fetches_as_fraction_of_pessimistic_bound: f64,
    /// `mean_fetches_per_query * per_fetch_latency`, computed with the real
    /// local p50 (when available) and, for cross-check, with DiskANN's own
    /// cited low/high figures.
    estimated_query_latency_ms_using_local_p50: Option<f64>,
    estimated_query_latency_ms_using_diskann_low: f64,
    estimated_query_latency_ms_using_diskann_high: f64,
}

fn sorted_f64(values: &[usize]) -> Vec<f64> {
    let mut v: Vec<f64> = values.iter().map(|&x| x as f64).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v
}

#[allow(clippy::too_many_arguments)]
fn run_query_sweep(
    reader: &NodeRecordReader,
    dims: usize,
    r: usize,
    l_param: usize,
    k: usize,
    num_queries: usize,
    seed_base: u64,
    local_p50_us: Option<f64>,
) -> QuerySweepResult {
    let mut fetches: Vec<usize> = Vec::with_capacity(num_queries);
    let mut hops: Vec<usize> = Vec::with_capacity(num_queries);

    for query_idx in 0..num_queries {
        let mut query_rng = StdRng::seed_from_u64(seed_base + query_idx as u64);
        let query: Vec<f32> = (0..dims)
            .map(|_| query_rng.random_range(-50.0f32..50.0))
            .collect();
        let result = greedy_search_cold(reader, reader.entry_point_slot(), &query, k, l_param);
        fetches.push(result.fetches());
        hops.push(result.hops());
    }

    let fetches_f64 = sorted_f64(&fetches);
    let hops_f64 = sorted_f64(&hops);
    let mean_fetches_per_query = fetches.iter().sum::<usize>() as f64 / num_queries as f64;
    let mean_hops_per_query = hops.iter().sum::<usize>() as f64 / num_queries as f64;
    let pessimistic_hops_times_r_bound = mean_hops_per_query * r as f64;
    let fetches_as_fraction_of_pessimistic_bound =
        mean_fetches_per_query / pessimistic_hops_times_r_bound;
    let estimated_query_latency_ms_using_local_p50 =
        local_p50_us.map(|us| mean_fetches_per_query * us / 1000.0);
    let estimated_query_latency_ms_using_diskann_low =
        mean_fetches_per_query * DISKANN_CITED_LOW_US / 1000.0;
    let estimated_query_latency_ms_using_diskann_high =
        mean_fetches_per_query * DISKANN_CITED_HIGH_US / 1000.0;

    eprintln!(
        "  L={l_param}: fetches/query mean={mean_fetches_per_query:.1} p50={:.0} p90={:.0} \
         p99={:.0} max={} | hops/query mean={mean_hops_per_query:.1} min={} max={} | \
         pessimistic hops*R bound={:.0} | real/pessimistic={:.1}%",
        percentile(&fetches_f64, 0.50),
        percentile(&fetches_f64, 0.90),
        percentile(&fetches_f64, 0.99),
        fetches.iter().max().unwrap(),
        hops.iter().min().unwrap(),
        hops.iter().max().unwrap(),
        pessimistic_hops_times_r_bound,
        fetches_as_fraction_of_pessimistic_bound * 100.0,
    );

    QuerySweepResult {
        l_param,
        k,
        num_queries,
        mean_fetches_per_query,
        p50_fetches_per_query: percentile(&fetches_f64, 0.50),
        p90_fetches_per_query: percentile(&fetches_f64, 0.90),
        p99_fetches_per_query: percentile(&fetches_f64, 0.99),
        max_fetches_per_query: *fetches.iter().max().unwrap(),
        min_fetches_per_query: *fetches.iter().min().unwrap(),
        mean_hops_per_query,
        p50_hops_per_query: percentile(&hops_f64, 0.50),
        p90_hops_per_query: percentile(&hops_f64, 0.90),
        p99_hops_per_query: percentile(&hops_f64, 0.99),
        max_hops_per_query: *hops.iter().max().unwrap(),
        min_hops_per_query: *hops.iter().min().unwrap(),
        pessimistic_hops_times_r_bound,
        fetches_as_fraction_of_pessimistic_bound,
        estimated_query_latency_ms_using_local_p50,
        estimated_query_latency_ms_using_diskann_low,
        estimated_query_latency_ms_using_diskann_high,
    }
}

#[derive(Serialize)]
struct GraphWarmQueryResult {
    construction_n: usize,
    dims: usize,
    max_out_degree: usize,
    construction_l_param: usize,
    alpha: f32,
    construction_seed: u64,
    build_vamana_ms: f64,
    bnf_permute_ms: f64,
    /// Starling's own `OR(G)` locality metric (`references/starling-
    /// sigmod2024.md` §4.1), measured on this run's own real graph and
    /// real BNF permutation, at `block_size = BNF_BLOCK_SIZE`.
    overlap_ratio_bnf: f64,
    /// The same metric for the identity (unshuffled, ID-contiguous)
    /// permutation on the identical graph — the paper's own cited "naive
    /// baseline ≈ 0" comparison point, measured here rather than assumed.
    overlap_ratio_unshuffled: f64,

    // ---- Primary measurement 1: fetch-count/hop-count distribution,
    // ---- one entry per QUERY_L_VALUES ---------------------------------
    query_sweeps: Vec<QuerySweepResult>,

    // ---- Primary measurement 2: local random-read latency -------------
    local_read_latency: LocalReadLatency,
    diskann_cited_low_us: f64,
    diskann_cited_high_us: f64,

    // ---- Secondary measurement: real S3/MinIO whole-blob fetch cost ---
    node_records_bytes: u64,
    permutation_directory_bytes: u64,
    graph_blob_bytes_total: u64,
    s3_iterations: usize,
    s3_get_count_per_open: u64,
    s3_open_latency_ms_p50: f64,
    s3_open_latency_ms_p90: f64,
    s3_open_latency_ms_p99: f64,
}

fn open_segment_hotcache(bytes: &[u8]) -> Hotcache {
    let footer_bytes: [u8; 40] = bytes[bytes.len() - 40..].try_into().unwrap();
    let footer = Footer::decode(&footer_bytes).expect("valid footer");
    let start = footer.hotcache_offset as usize;
    let end = start + footer.hotcache_length as usize;
    Hotcache::decode(&bytes[start..end]).expect("valid hotcache")
}

fn main() {
    eprintln!(
        "Building a real Vamana graph: n={CONSTRUCTION_N}, dims={DIMS}, R={R}, \
         L={CONSTRUCTION_L}, alpha={ALPHA} (release mode; measured ~3 minutes at this scale)..."
    );
    let mut rng = StdRng::seed_from_u64(CONSTRUCTION_SEED);
    let points: Vec<f32> = (0..CONSTRUCTION_N * DIMS)
        .map(|_| rng.random_range(-50.0f32..50.0))
        .collect();
    let config = VamanaConfig {
        r: R,
        l_param: CONSTRUCTION_L,
        alpha: ALPHA,
    };
    let (vamana, build_vamana_ms) =
        timed(|| build_vamana(&points, CONSTRUCTION_N, DIMS, &config, &mut rng));
    eprintln!("build_vamana done in {build_vamana_ms:.0}ms");

    let bnf_config = BnfConfig {
        block_size: BNF_BLOCK_SIZE,
        beta: BNF_BETA,
        tau: BNF_TAU,
    };
    let (permutation, bnf_permute_ms) = timed(|| bnf(&vamana.graph, &bnf_config));
    eprintln!("BNF permutation done in {bnf_permute_ms:.0}ms");

    let overlap_ratio_bnf = overlap_ratio(&vamana.graph, &permutation, BNF_BLOCK_SIZE);
    let identity_permutation: Vec<usize> = (0..CONSTRUCTION_N).collect();
    let overlap_ratio_unshuffled =
        overlap_ratio(&vamana.graph, &identity_permutation, BNF_BLOCK_SIZE);
    eprintln!(
        "OR(G): bnf={overlap_ratio_bnf:.4}, unshuffled={overlap_ratio_unshuffled:.4} \
         (Starling's own cited naive baseline is \u{2248}0, optimal is 1)"
    );

    let row_ids: Vec<u64> = (0..CONSTRUCTION_N as u64).collect();
    let field_id = field_id_from_name("vec");
    let input = GraphBlobInput {
        vamana: &vamana,
        permutation: &permutation,
        points: &points,
        dims: DIMS,
        row_ids: &row_ids,
        max_out_degree: R,
        shuffle_algorithm: ShuffleAlgorithm::Bnf,
    };
    let (node_records_spec, permutation_directory_spec) = build_graph_blob_specs(field_id, &input);
    let node_records_bytes = node_records_spec.data.len() as u64;
    let permutation_directory_bytes = permutation_directory_spec.data.len() as u64;
    eprintln!(
        "Real graph blob: node_records={node_records_bytes} bytes, \
         permutation_directory={permutation_directory_bytes} bytes"
    );

    // ---- Primary measurement 2 (measured first so its p50 feeds the
    // ---- per-sweep latency estimate below): local random-read latency -
    eprintln!("Measuring real local random-read latency (O_DIRECT against this machine's NVMe)...");
    let local_read_latency = measure_local_random_read_latency();
    if local_read_latency.available {
        eprintln!(
            "local random-read latency: p50={:.1}us p90={:.1}us p99={:.1}us (n={})",
            local_read_latency.p50_us,
            local_read_latency.p90_us,
            local_read_latency.p99_us,
            local_read_latency.samples
        );
    } else {
        eprintln!(
            "local random-read latency NOT available: {} — falling back to DiskANN's cited figure",
            local_read_latency
                .unavailable_reason
                .as_deref()
                .unwrap_or("unknown")
        );
    }
    let local_p50_us = local_read_latency
        .available
        .then_some(local_read_latency.p50_us);

    // ---- Primary measurement 1: fetch-count/hop-count distribution, one
    // ---- sweep per QUERY_L_VALUES entry --------------------------------
    let reader = NodeRecordReader::new(&node_records_spec.data).expect("valid node records");
    eprintln!(
        "Running {NUM_QUERIES} real cold-open queries per L against the real graph blob, \
         L in {QUERY_L_VALUES:?}..."
    );
    let query_sweeps: Vec<QuerySweepResult> = QUERY_L_VALUES
        .iter()
        .map(|&l_param| {
            run_query_sweep(
                &reader,
                DIMS,
                R,
                l_param,
                QUERY_K,
                NUM_QUERIES,
                QUERY_SEED_BASE + l_param as u64 * 1_000_000,
                local_p50_us,
            )
        })
        .collect();

    // ---- Secondary measurement: real S3/MinIO whole-blob fetch cost ---
    // Everything from here on runs inside `with_minio`'s closure and moves
    // every needed value into it (rather than assigning through captured
    // `&mut` outer variables), because `with_minio` requires its closure to
    // be `UnwindSafe` and a captured `&mut` is not — the same reason
    // `vector_cold_open.rs` assembles and writes its own report from
    // entirely inside its own `with_minio` closure.
    eprintln!("Committing the real graph blob to real MinIO for the secondary S3 measurement...");

    with_minio(move |endpoint, bucket| {
        let store = store_for(endpoint, bucket);

        let mut builder = SegmentBuilder::new(CONSTRUCTION_N as u64);
        builder.add_blob(node_records_spec);
        builder.add_blob(permutation_directory_spec);

        let snapshot = commit(&store, |row_id_base| {
            vec![write_segment(
                &store,
                "segments/graph-warm-query.bin",
                &builder,
                row_id_base,
            )]
        })
        .expect("commit succeeds against an empty table");
        let segment_total_bytes = snapshot.segments[0].byte_length;
        eprintln!("Committed real graph segment: {segment_total_bytes} bytes on real MinIO");

        let counting = CountingStore::new(&store);
        let mut latencies = Vec::with_capacity(S3_ITERATIONS);
        let mut get_counts = Vec::with_capacity(S3_ITERATIONS);

        for _ in 0..S3_ITERATIONS {
            counting.reset();
            let (_hotcache, elapsed_ms) = timed(|| {
                let snapshot = read_snapshot(&counting).unwrap().unwrap();
                let segment_ref = &snapshot.segments[0];
                let (segment_bytes, _) = counting.get(&segment_ref.path).unwrap().unwrap();
                open_segment_hotcache(&segment_bytes)
            });
            latencies.push(elapsed_ms);
            get_counts.push(counting.get_count());
        }

        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let s3_get_count_per_open = get_counts[0];
        assert!(
            get_counts.iter().all(|&c| c == s3_get_count_per_open),
            "GET count per open must be constant across iterations: {get_counts:?}"
        );

        let s3_open_latency_ms_p50 = percentile(&latencies, 0.50);
        let s3_open_latency_ms_p90 = percentile(&latencies, 0.90);
        let s3_open_latency_ms_p99 = percentile(&latencies, 0.99);

        let result = GraphWarmQueryResult {
            construction_n: CONSTRUCTION_N,
            dims: DIMS,
            max_out_degree: R,
            construction_l_param: CONSTRUCTION_L,
            alpha: ALPHA,
            construction_seed: CONSTRUCTION_SEED,
            build_vamana_ms,
            bnf_permute_ms,
            overlap_ratio_bnf,
            overlap_ratio_unshuffled,

            query_sweeps,

            local_read_latency,
            diskann_cited_low_us: DISKANN_CITED_LOW_US,
            diskann_cited_high_us: DISKANN_CITED_HIGH_US,

            node_records_bytes,
            permutation_directory_bytes,
            graph_blob_bytes_total: node_records_bytes + permutation_directory_bytes,
            s3_iterations: S3_ITERATIONS,
            s3_get_count_per_open,
            s3_open_latency_ms_p50,
            s3_open_latency_ms_p90,
            s3_open_latency_ms_p99,
        };

        for sweep in &result.query_sweeps {
            println!(
                "graph warm-tier query: n={}, dims={}, R={}, L={}, {} queries: mean \
                 fetches/query={:.1} (p50={:.0} p90={:.0} p99={:.0}), mean hops/query={:.1} \
                 (min={} max={}), {:.1}% of pessimistic hops*R bound; local read p50={}",
                result.construction_n,
                result.dims,
                result.max_out_degree,
                sweep.l_param,
                sweep.num_queries,
                sweep.mean_fetches_per_query,
                sweep.p50_fetches_per_query,
                sweep.p90_fetches_per_query,
                sweep.p99_fetches_per_query,
                sweep.mean_hops_per_query,
                sweep.min_hops_per_query,
                sweep.max_hops_per_query,
                sweep.fetches_as_fraction_of_pessimistic_bound * 100.0,
                if result.local_read_latency.available {
                    format!("{:.1}us", result.local_read_latency.p50_us)
                } else {
                    "unavailable (see report)".to_string()
                },
            );
        }
        println!(
            "graph warm-tier query: S3 whole-blob open: {} GETs, p50={:.1}ms, {} bytes",
            result.s3_get_count_per_open,
            result.s3_open_latency_ms_p50,
            result.graph_blob_bytes_total,
        );

        write_report("graph-warm-query", result);
    });
}
