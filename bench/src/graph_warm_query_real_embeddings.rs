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

//! `docs/roadmap.md` D-3: the same real, measured graph-blob benchmark as
//! `bench/src/graph_warm_query.rs`, run against **real, clustered code
//! embeddings** instead of synthetic uniform-random points, to close the
//! gap that file's own module doc and `CLAUDE.md` §7's graph-family
//! paragraph already name honestly: the original run's fetch/hop counts sit
//! near the query-time `L` ceiling, a near-worst-case convergence regime
//! this project attributed to its synthetic distribution having no cluster
//! structure, not to the graph parameters (`R`, `L`) themselves. This file
//! is the direct test of that attribution.
//!
//! **Data provenance (real, not literature-translated).** Embeddings were
//! generated offline by
//! `bench/data/d3-jina-code-v2/../../../scratch/d3_extract_and_embed.py`
//! (kept outside this crate; see `docs/roadmap.md`'s D-3 entry for the full
//! pipeline write-up) and land here as two gitignored binary files under
//! `bench/data/d3-jina-code-v2/` (same `/bench/data` gitignore pattern
//! `bench/src/msmarco_index.rs` already uses):
//!
//! - `rust-stdlib-code-embeddings.f32le.bin` — the build set: 4,000 vectors
//!   (matching `graph_warm_query.rs`'s own `CONSTRUCTION_N = 4_000`),
//!   embedded from real per-function chunks extracted out of the vendored
//!   Rust standard library (`library/core`+`library/std`+`library/alloc`,
//!   tag `1.97.1`, dual MIT/Apache-2.0 per `docs/roadmap.md`'s D-2 entry),
//!   using `jinaai/jina-embeddings-v2-base-code` (Apache-2.0, confirmed
//!   live via Hugging Face's model API), run as the `onnx/model_quantized.onnx`
//!   export (161,895,621 bytes) through `onnxruntime` with a pure
//!   tokenizer (no PyTorch/`transformers`/`trust_remote_code` needed — the
//!   ONNX graph already bakes in the model's custom ALiBi attention). Real
//!   output dimensionality is **768**, not the original benchmark's
//!   `dims=128` — used as-is, not truncated or padded (truncating would
//!   throw away real signal from a model that was never trained to be
//!   truncated; padding would inject fake, meaningless dimensions). Each
//!   embedding is mean-pooled over `last_hidden_state` (attention-mask-
//!   weighted) and L2-normalized, matching this model's own intended
//!   similarity convention (cosine similarity via inner product on unit
//!   vectors) and pairing naturally with `crate::vamana`'s squared-Euclidean
//!   distance (`‖a−b‖² = 2 − 2·cos(a,b)` for unit vectors, so nearest-by-
//!   squared-distance and nearest-by-cosine agree exactly here).
//! - `rust-stdlib-code-embeddings-queries.f32le.bin` — 300 more real code
//!   chunks, embedded the same way, drawn from the same corpus but with a
//!   disjoint, separately-seeded sample so no query vector is also a build
//!   point. Real held-out code embeddings are used as queries rather than
//!   fresh random vectors — a random vector in ambient 768-d space would
//!   sit off this embedding model's real data manifold (real embedding
//!   models do not fill their output space uniformly), which would be a
//!   *less* representative query workload than reusing the corpus, not a
//!   neutral one, so held-out real chunks are the honest choice.
//!
//! **Chunking method, stated plainly (governs what "a code embedding" means
//! here).** Per-function, via a regex + brace-counting heuristic (not a
//! real parser — `syn`/tree-sitter were not used, since a fast, mostly-
//! correct heuristic was enough for a one-off data-generation pass): find
//! `fn` signatures (with optional `pub`/`async`/`unsafe`/`extern`/`const`
//! modifiers and attributes) up to their opening `{`, then walk forward
//! counting braces to the matching close. 17,262 raw chunks were extracted
//! from 1,062 real `.rs` files (excluding D-2's own two named per-file
//! license-exception files: `library/core/src/unicode/unicode_data.rs` and
//! `library/std/src/sys/sync/mutex/fuchsia.rs`); 14,773 remained after
//! exact-text dedup (macro-expanded impls across primitive integer types
//! produce some identical bodies); a 4,000-chunk build sample and a
//! disjoint 300-chunk query sample were drawn with fixed seeds
//! (`20260820`/`20260821`) from that deduplicated pool. Chunks were length-
//! filtered to 40–4,000 characters (mean 352.8) before sampling, dropping
//! one-line trivial getters and pathological multi-KB functions.
//!
//! **Same construction/query parameters as `graph_warm_query.rs` where
//! comparable, so the two runs are a fair A/B.** `R = 64`, construction
//! `L = 100`, `alpha = 1.2`, the same two query-time `L` values
//! (`{32, 100}`), the same `k = 10`. `dims` differs honestly (768 vs 128)
//! because that is the real model's real output width; local NVMe
//! read-latency and the S3/MinIO whole-blob open cost are not re-measured
//! here (per the D-3 task's own instruction) since those measure the
//! storage device and the wire format, not the data distribution — this
//! file re-runs `measure_local_random_read_latency` anyway for a
//! self-contained report, but the number is expected to match
//! `graph-warm-query.json`'s within noise, not to differ.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::Serialize;
use serde_json::Value;
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

const R: usize = 64;
const CONSTRUCTION_L: usize = 100;
const ALPHA: f32 = 1.2;

const BNF_BLOCK_SIZE: usize = 32;
const BNF_BETA: usize = 10;
const BNF_TAU: f64 = 0.0;

const QUERY_L_VALUES: [usize; 2] = [32, 100];
const QUERY_K: usize = 10;
const MAX_QUERIES: usize = 300;

const LOCAL_READ_FILE_BYTES: u64 = 256 * 1024 * 1024;
const LOCAL_READ_BLOCK_BYTES: usize = 4096;
const LOCAL_READ_SAMPLES: usize = 2_000;
const LOCAL_READ_SEED: u64 = 555_101;

const DISKANN_CITED_LOW_US: f64 = 100.0;
const DISKANN_CITED_HIGH_US: f64 = 300.0;

const S3_ITERATIONS: usize = 10;

const DATA_DIR_SUFFIX: &str = "data/d3-jina-code-v2";

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

/// Identical measurement to `graph_warm_query.rs::measure_local_random_read_latency`
/// (same device, same method) — re-run here only so this file's report is
/// self-contained, not because a different number is expected.
fn measure_local_random_read_latency() -> LocalReadLatency {
    let device_description = "nvme0n1 (real NVMe, rotational=0), ASUSTeK MINIPC PN62, ext4-on-LVM \
         (confirmed via lsblk/dmidecode/df -T in graph_warm_query.rs; not re-checked here)"
        .to_string();

    let dir = env!("CARGO_MANIFEST_DIR");
    let temp = match tempfile::Builder::new()
        .prefix("graph-warm-query-real-embeddings-odirect-")
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
                unavailable_reason: Some(format!("O_DIRECT open failed on this filesystem ({e})")),
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
    pessimistic_hops_times_r_bound: f64,
    fetches_as_fraction_of_pessimistic_bound: f64,
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
    queries: &[Vec<f32>],
    r: usize,
    l_param: usize,
    k: usize,
    local_p50_us: Option<f64>,
) -> QuerySweepResult {
    let num_queries = queries.len();
    let mut fetches: Vec<usize> = Vec::with_capacity(num_queries);
    let mut hops: Vec<usize> = Vec::with_capacity(num_queries);

    for query in queries {
        let result = greedy_search_cold(reader, reader.entry_point_slot(), query, k, l_param);
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
struct EmbeddingProvenance {
    model: String,
    model_license: String,
    model_onnx_file: String,
    model_onnx_file_bytes: u64,
    embedding_dims: usize,
    build_n: usize,
    query_n: usize,
    l2_normalized: bool,
    pooling: String,
    corpus_repo: String,
    corpus_tag: String,
    corpus_subdirs: Vec<String>,
    corpus_excluded_files: Vec<String>,
    raw_chunks_extracted: u64,
    unique_chunks_after_dedup: u64,
    sample_seed: u64,
    min_chunk_chars: u64,
    max_chunk_chars: u64,
    chunk_char_len_mean: f64,
}

#[derive(Serialize)]
struct GraphWarmQueryRealEmbeddingsResult {
    embedding_provenance: EmbeddingProvenance,

    construction_n: usize,
    dims: usize,
    max_out_degree: usize,
    construction_l_param: usize,
    alpha: f32,
    build_vamana_ms: f64,
    bnf_permute_ms: f64,
    overlap_ratio_bnf: f64,
    overlap_ratio_unshuffled: f64,

    query_sweeps: Vec<QuerySweepResult>,

    local_read_latency: LocalReadLatency,
    diskann_cited_low_us: f64,
    diskann_cited_high_us: f64,

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

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(DATA_DIR_SUFFIX)
}

fn load_meta(path: &std::path::Path) -> Value {
    let bytes = std::fs::read(path)
        .unwrap_or_else(|e| panic!("reading {path:?} failed: {e} (run the D-3 embedding pipeline first — see docs/roadmap.md's D-3 entry)"));
    serde_json::from_slice(&bytes).expect("valid JSON meta sidecar")
}

fn load_f32_vectors(path: &std::path::Path, n: usize, dims: usize) -> Vec<f32> {
    let bytes = std::fs::read(path)
        .unwrap_or_else(|e| panic!("reading {path:?} failed: {e} (run the D-3 embedding pipeline first)"));
    let expected_bytes = n * dims * 4;
    assert_eq!(
        bytes.len(),
        expected_bytes,
        "{path:?}: expected {expected_bytes} bytes (n={n} * dims={dims} * 4), got {}",
        bytes.len()
    );
    let mut out = Vec::with_capacity(n * dims);
    for chunk in bytes.chunks_exact(4) {
        out.push(f32::from_le_bytes(chunk.try_into().unwrap()));
    }
    out
}

fn main() {
    let dir = data_dir();
    let build_meta = load_meta(&dir.join("rust-stdlib-code-embeddings.meta.json"));
    let query_meta = load_meta(&dir.join("rust-stdlib-code-embeddings-queries.meta.json"));

    let dims = build_meta["embedding_dims"].as_u64().unwrap() as usize;
    let construction_n = build_meta["n"].as_u64().unwrap() as usize;
    let query_dims = query_meta["embedding_dims"].as_u64().unwrap() as usize;
    assert_eq!(dims, query_dims, "build/query embedding dims must match");
    let query_n_available = query_meta["n"].as_u64().unwrap() as usize;
    let num_queries = query_n_available.min(MAX_QUERIES);

    eprintln!(
        "Loading real embeddings: build n={construction_n}, dims={dims}, queries={num_queries} \
         (of {query_n_available} available), model={}",
        build_meta["model"].as_str().unwrap_or("?")
    );

    let points = load_f32_vectors(
        &dir.join("rust-stdlib-code-embeddings.f32le.bin"),
        construction_n,
        dims,
    );
    let query_points_flat = load_f32_vectors(
        &dir.join("rust-stdlib-code-embeddings-queries.f32le.bin"),
        query_n_available,
        dims,
    );
    let queries: Vec<Vec<f32>> = query_points_flat
        .chunks_exact(dims)
        .take(num_queries)
        .map(|c| c.to_vec())
        .collect();

    let embedding_provenance = EmbeddingProvenance {
        model: build_meta["model"].as_str().unwrap_or("?").to_string(),
        model_license: build_meta["model_license"]
            .as_str()
            .unwrap_or("?")
            .to_string(),
        model_onnx_file: build_meta["model_onnx_file"]
            .as_str()
            .unwrap_or("?")
            .to_string(),
        model_onnx_file_bytes: build_meta["model_onnx_file_bytes"].as_u64().unwrap_or(0),
        embedding_dims: dims,
        build_n: construction_n,
        query_n: num_queries,
        l2_normalized: build_meta["l2_normalized"].as_bool().unwrap_or(false),
        pooling: build_meta["pooling"].as_str().unwrap_or("?").to_string(),
        corpus_repo: build_meta["corpus_repo"].as_str().unwrap_or("?").to_string(),
        corpus_tag: build_meta["corpus_tag"].as_str().unwrap_or("?").to_string(),
        corpus_subdirs: build_meta["corpus_subdirs"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        corpus_excluded_files: build_meta["corpus_excluded_files"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        raw_chunks_extracted: build_meta["raw_chunks_extracted"].as_u64().unwrap_or(0),
        unique_chunks_after_dedup: build_meta["unique_chunks_after_dedup"]
            .as_u64()
            .unwrap_or(0),
        sample_seed: build_meta["sample_seed"].as_u64().unwrap_or(0),
        min_chunk_chars: build_meta["min_chunk_chars"].as_u64().unwrap_or(0),
        max_chunk_chars: build_meta["max_chunk_chars"].as_u64().unwrap_or(0),
        chunk_char_len_mean: build_meta["chunk_char_len_mean"].as_f64().unwrap_or(0.0),
    };

    eprintln!(
        "Building a real Vamana graph over real embeddings: n={construction_n}, dims={dims}, \
         R={R}, L={CONSTRUCTION_L}, alpha={ALPHA} (release mode)..."
    );
    let mut rng = StdRng::seed_from_u64(20_260_820);
    let config = VamanaConfig {
        r: R,
        l_param: CONSTRUCTION_L,
        alpha: ALPHA,
    };
    let (vamana, build_vamana_ms) =
        timed(|| build_vamana(&points, construction_n, dims, &config, &mut rng));
    eprintln!("build_vamana done in {build_vamana_ms:.0}ms");

    let bnf_config = BnfConfig {
        block_size: BNF_BLOCK_SIZE,
        beta: BNF_BETA,
        tau: BNF_TAU,
    };
    let (permutation, bnf_permute_ms) = timed(|| bnf(&vamana.graph, &bnf_config));
    eprintln!("BNF permutation done in {bnf_permute_ms:.0}ms");

    let overlap_ratio_bnf = overlap_ratio(&vamana.graph, &permutation, BNF_BLOCK_SIZE);
    let identity_permutation: Vec<usize> = (0..construction_n).collect();
    let overlap_ratio_unshuffled =
        overlap_ratio(&vamana.graph, &identity_permutation, BNF_BLOCK_SIZE);
    eprintln!(
        "OR(G): bnf={overlap_ratio_bnf:.4}, unshuffled={overlap_ratio_unshuffled:.4}"
    );

    let row_ids: Vec<u64> = (0..construction_n as u64).collect();
    let field_id = field_id_from_name("vec");
    let input = GraphBlobInput {
        vamana: &vamana,
        permutation: &permutation,
        points: &points,
        dims,
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
    }
    let local_p50_us = local_read_latency
        .available
        .then_some(local_read_latency.p50_us);

    let reader = NodeRecordReader::new(&node_records_spec.data).expect("valid node records");
    eprintln!(
        "Running {num_queries} real held-out-query-embedding cold-open queries per L against \
         the real graph blob, L in {QUERY_L_VALUES:?}..."
    );
    let query_sweeps: Vec<QuerySweepResult> = QUERY_L_VALUES
        .iter()
        .map(|&l_param| run_query_sweep(&reader, &queries, R, l_param, QUERY_K, local_p50_us))
        .collect();

    eprintln!("Committing the real graph blob to real MinIO for the secondary S3 measurement...");

    with_minio(move |endpoint, bucket| {
        let store = store_for(endpoint, bucket);

        let mut builder = SegmentBuilder::new(construction_n as u64);
        builder.add_blob(node_records_spec);
        builder.add_blob(permutation_directory_spec);

        let snapshot = commit(&store, |row_id_base| {
            vec![write_segment(
                &store,
                "segments/graph-warm-query-real-embeddings.bin",
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

        let result = GraphWarmQueryRealEmbeddingsResult {
            embedding_provenance,
            construction_n,
            dims,
            max_out_degree: R,
            construction_l_param: CONSTRUCTION_L,
            alpha: ALPHA,
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
                "graph warm-tier query (real embeddings): n={}, dims={}, R={}, L={}, {} queries: \
                 mean fetches/query={:.1} (p50={:.0} p90={:.0} p99={:.0}), mean hops/query={:.1} \
                 (min={} max={}), {:.1}% of pessimistic hops*R bound",
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
            );
        }
        println!(
            "graph warm-tier query (real embeddings): S3 whole-blob open: {} GETs, p50={:.1}ms, \
             {} bytes",
            result.s3_get_count_per_open,
            result.s3_open_latency_ms_p50,
            result.graph_blob_bytes_total,
        );

        write_report("graph-warm-query-real-embeddings", result);
    });
}
