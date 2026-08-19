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

//! R9's still-open measurement (`docs/ledger.md`): decode throughput of the
//! `bitpacking` crate's SIMD-BP128 candidates (`BitPacker4x`, 128-int blocks,
//! SSE3; `BitPacker8x`, 256-int blocks, AVX2), `fastlanes`
//! (`spiraldb/fastlanes`, 1024-int blocks), and `fastpfor`
//! (`fast-pack/FastPFOR-rs`, 256-int blocks, pure Rust, exception-based) at
//! matched bit-widths, matched total value counts (1024 values per
//! comparison unit), and matched input data per width.
//!
//! **What this is not.** Not a benchmark against real MS MARCO postings
//! distributions (`docs/data-structures.md`'s own bake-off target) — this
//! generates synthetic, uniform-random values in `[0, 2^W)` per declared
//! bit-width `W`, the same data-generation convention the `bitpacking` and
//! `fastlanes` crates' own upstream benchmarks use. A uniform distribution
//! is the worst case for any scheme that could exploit skew (it can't), so
//! this measures raw decode throughput at a fixed bit-width honestly, not
//! full corpus-level compression behavior. The R2 bake-off
//! (`docs/milestones.md` M1) still needs the real corpus; this is a
//! narrower, still-useful first measurement of the specific margin R9 asks
//! about.
//!
//! **FastPFOR specifically needs a second, skewed distribution to be a fair
//! test.** FastPFOR is an adaptive, exception-based codec (Lemire & Boytsov,
//! `references/lemire-boytsov-simd-bp128.md`): it bit-packs the common case
//! at a small width and stores a minority of larger "exception" values
//! separately, so it should look artificially weak on uniform-random data
//! (every value forces the same width, so the exception machinery buys
//! nothing but still costs something) and should look better on skewed data
//! resembling real delta-gap postings (mostly small values, occasional large
//! ones). This file measures both: FastPFOR on the same uniform sweep as the
//! other three codecs (apples-to-apples, explicitly not its intended case),
//! and a second, separate skewed-distribution comparison against
//! `BitPacker8x` (the fastest plain bit-packer measured) specifically to
//! test FastPFOR's actual design target honestly rather than only on the
//! distribution that disadvantages it.
//!
//! **Real MS MARCO postings distributions, no longer missing.** The
//! `real_msmarco_d_gaps`/`real_msmarco_term_frequencies` results below read
//! `bench/results/msmarco-real-postings-sample.json` (produced by
//! `bench/src/msmarco_index.rs` from a real, stride-sampled ~520K-passage
//! subset of the MS MARCO passage corpus, tokenized with STRAND's own
//! analyzer chain) and run the same FastPFOR-vs-`BitPacker8x` comparison
//! against every full 1024-value chunk of the actual doc-ID delta-gaps and
//! term frequencies observed, averaged across chunks — not a synthetic
//! stand-in. This still isn't the full corpus (8,841,823 passages; this
//! sample is ~520K) or a full R2 bake-off, but it replaces guessed skew with
//! measured skew for the specific margin R9 asks about.

use bitpacking::{BitPacker, BitPacker4x, BitPacker8x};
use fastlanes::BitPacking as FastLanesBitPacking;
use fastpfor::{BlockCodec, FastPForBlock256};
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::Serialize;
use std::time::Instant;
use strand_bench::write_report;

/// Values per comparison unit — the least common multiple of every block
/// size under test (128, 256, 1024), so every codec decodes a whole number
/// of its own blocks with no partial-block remainder.
const VALUES_PER_UNIT: usize = 1024;
const ITERATIONS: usize = 2000;

/// A representative sweep of bit-widths, matching the range real delta-gap
/// postings values fall into: 1-4 bits for short gaps in dense/common-term
/// lists, up to the high 20s for sparse/rare-term lists on a
/// tens-of-millions-of-documents collection (2^28 > 268M documents).
const WIDTHS: [u8; 10] = [1, 2, 4, 6, 8, 10, 12, 16, 20, 24];

#[derive(Serialize)]
struct CodecResult {
    bit_width: u8,
    decode_ns_per_1024_values: f64,
    decode_values_per_sec: f64,
    encode_ns_per_1024_values: f64,
}

#[derive(Serialize)]
struct SkewedResult {
    codec: String,
    decode_ns_per_1024_values: f64,
    decode_values_per_sec: f64,
    encode_ns_per_1024_values: f64,
    compressed_bytes_per_1024_values: usize,
}

#[derive(Serialize)]
struct RealCorpusResult {
    codec: String,
    /// How many full 1024-value chunks of the real array this result
    /// averages over — small for sparse fields (e.g. term frequencies from
    /// a modest sample), stated rather than hidden.
    chunks_measured: usize,
    decode_ns_per_1024_values: f64,
    decode_values_per_sec: f64,
    encode_ns_per_1024_values: f64,
    compressed_bytes_per_1024_values: f64,
}

#[derive(Serialize)]
struct AllResults {
    values_per_unit: usize,
    iterations: usize,
    cpu: String,
    bitpacker4x_128int_sse3: Vec<CodecResult>,
    bitpacker8x_256int_avx2: Vec<CodecResult>,
    fastlanes_1024int: Vec<CodecResult>,
    fastpfor_256int_uniform: Vec<CodecResult>,
    skewed_distribution_95pct_4bit_5pct_24bit: Vec<SkewedResult>,
    real_msmarco_d_gaps: Option<Vec<RealCorpusResult>>,
    real_msmarco_term_frequencies: Option<Vec<RealCorpusResult>>,
}

fn random_values(rng: &mut StdRng, width: u8) -> Vec<u32> {
    let max = if width == 32 {
        u32::MAX
    } else {
        (1u32 << width) - 1
    };
    (0..VALUES_PER_UNIT)
        .map(|_| rng.random_range(0..=max))
        .collect()
}

fn bench_bitpacker4x(values: &[u32], width: u8) -> CodecResult {
    let bp = BitPacker4x::new();
    let blocks = VALUES_PER_UNIT / BitPacker4x::BLOCK_LEN;
    let mut compressed = vec![0u8; VALUES_PER_UNIT * 4];
    let mut decompressed = vec![0u32; VALUES_PER_UNIT];

    // Encode once, verify round-trip, then time steady-state encode/decode
    // separately so a slow encode can't be hidden inside a decode number.
    let mut compressed_len = 0;
    for b in 0..blocks {
        let block = &values[b * BitPacker4x::BLOCK_LEN..(b + 1) * BitPacker4x::BLOCK_LEN];
        let out = &mut compressed[compressed_len..];
        compressed_len += bp.compress(block, out, width);
    }
    for b in 0..blocks {
        let block_bytes = width as usize * BitPacker4x::BLOCK_LEN / 8;
        let src = &compressed[b * block_bytes..(b + 1) * block_bytes];
        let dst = &mut decompressed[b * BitPacker4x::BLOCK_LEN..(b + 1) * BitPacker4x::BLOCK_LEN];
        bp.decompress(src, dst, width);
    }
    assert_eq!(
        values,
        &decompressed[..],
        "BitPacker4x round-trip mismatch at width {width}"
    );

    let encode_start = Instant::now();
    for _ in 0..ITERATIONS {
        let mut len = 0;
        for b in 0..blocks {
            let block = &values[b * BitPacker4x::BLOCK_LEN..(b + 1) * BitPacker4x::BLOCK_LEN];
            let out = &mut compressed[len..];
            len += bp.compress(std::hint::black_box(block), out, width);
        }
        std::hint::black_box(&compressed);
    }
    let encode_ns = encode_start.elapsed().as_nanos() as f64 / ITERATIONS as f64;

    let decode_start = Instant::now();
    for _ in 0..ITERATIONS {
        for b in 0..blocks {
            let block_bytes = width as usize * BitPacker4x::BLOCK_LEN / 8;
            let src = &compressed[b * block_bytes..(b + 1) * block_bytes];
            let dst =
                &mut decompressed[b * BitPacker4x::BLOCK_LEN..(b + 1) * BitPacker4x::BLOCK_LEN];
            bp.decompress(std::hint::black_box(src), dst, width);
        }
        std::hint::black_box(&decompressed);
    }
    let decode_ns = decode_start.elapsed().as_nanos() as f64 / ITERATIONS as f64;

    CodecResult {
        bit_width: width,
        decode_ns_per_1024_values: decode_ns,
        decode_values_per_sec: VALUES_PER_UNIT as f64 / (decode_ns / 1e9),
        encode_ns_per_1024_values: encode_ns,
    }
}

fn bench_bitpacker8x(values: &[u32], width: u8) -> CodecResult {
    let bp = BitPacker8x::new();
    let blocks = VALUES_PER_UNIT / BitPacker8x::BLOCK_LEN;
    let mut compressed = vec![0u8; VALUES_PER_UNIT * 4];
    let mut decompressed = vec![0u32; VALUES_PER_UNIT];

    let mut compressed_len = 0;
    for b in 0..blocks {
        let block = &values[b * BitPacker8x::BLOCK_LEN..(b + 1) * BitPacker8x::BLOCK_LEN];
        let out = &mut compressed[compressed_len..];
        compressed_len += bp.compress(block, out, width);
    }
    for b in 0..blocks {
        let block_bytes = width as usize * BitPacker8x::BLOCK_LEN / 8;
        let src = &compressed[b * block_bytes..(b + 1) * block_bytes];
        let dst = &mut decompressed[b * BitPacker8x::BLOCK_LEN..(b + 1) * BitPacker8x::BLOCK_LEN];
        bp.decompress(src, dst, width);
    }
    assert_eq!(
        values,
        &decompressed[..],
        "BitPacker8x round-trip mismatch at width {width}"
    );

    let encode_start = Instant::now();
    for _ in 0..ITERATIONS {
        let mut len = 0;
        for b in 0..blocks {
            let block = &values[b * BitPacker8x::BLOCK_LEN..(b + 1) * BitPacker8x::BLOCK_LEN];
            let out = &mut compressed[len..];
            len += bp.compress(std::hint::black_box(block), out, width);
        }
        std::hint::black_box(&compressed);
    }
    let encode_ns = encode_start.elapsed().as_nanos() as f64 / ITERATIONS as f64;

    let decode_start = Instant::now();
    for _ in 0..ITERATIONS {
        for b in 0..blocks {
            let block_bytes = width as usize * BitPacker8x::BLOCK_LEN / 8;
            let src = &compressed[b * block_bytes..(b + 1) * block_bytes];
            let dst =
                &mut decompressed[b * BitPacker8x::BLOCK_LEN..(b + 1) * BitPacker8x::BLOCK_LEN];
            bp.decompress(std::hint::black_box(src), dst, width);
        }
        std::hint::black_box(&decompressed);
    }
    let decode_ns = decode_start.elapsed().as_nanos() as f64 / ITERATIONS as f64;

    CodecResult {
        bit_width: width,
        decode_ns_per_1024_values: decode_ns,
        decode_values_per_sec: VALUES_PER_UNIT as f64 / (decode_ns / 1e9),
        encode_ns_per_1024_values: encode_ns,
    }
}

fn bench_fastlanes(values: &[u32], width: u8) -> CodecResult {
    let width = width as usize;
    let packed_len = VALUES_PER_UNIT * width / 32;
    let mut packed = vec![0u32; packed_len];
    let mut unpacked = vec![0u32; VALUES_PER_UNIT];

    // SAFETY: `values` and `unpacked` are exactly 1024 elements
    // (VALUES_PER_UNIT); `packed` is exactly `1024 * width / 32` elements,
    // per unchecked_pack/unchecked_unpack's own documented length
    // contract; `width <= 32` for every entry in WIDTHS.
    unsafe {
        FastLanesBitPacking::unchecked_pack(width, values, &mut packed);
        FastLanesBitPacking::unchecked_unpack(width, &packed, &mut unpacked);
    }
    assert_eq!(
        values,
        &unpacked[..],
        "FastLanes round-trip mismatch at width {width}"
    );

    let encode_start = Instant::now();
    for _ in 0..ITERATIONS {
        unsafe {
            FastLanesBitPacking::unchecked_pack(width, std::hint::black_box(values), &mut packed);
        }
        std::hint::black_box(&packed);
    }
    let encode_ns = encode_start.elapsed().as_nanos() as f64 / ITERATIONS as f64;

    let decode_start = Instant::now();
    for _ in 0..ITERATIONS {
        unsafe {
            FastLanesBitPacking::unchecked_unpack(
                width,
                std::hint::black_box(&packed),
                &mut unpacked,
            );
        }
        std::hint::black_box(&unpacked);
    }
    let decode_ns = decode_start.elapsed().as_nanos() as f64 / ITERATIONS as f64;

    CodecResult {
        bit_width: width as u8,
        decode_ns_per_1024_values: decode_ns,
        decode_values_per_sec: VALUES_PER_UNIT as f64 / (decode_ns / 1e9),
        encode_ns_per_1024_values: encode_ns,
    }
}

fn bench_fastpfor(values: &[u32]) -> CodecResult {
    use fastpfor::slice_to_blocks;

    let mut codec = FastPForBlock256::default();
    let (blocks, remainder) = slice_to_blocks::<FastPForBlock256>(values);
    assert!(
        remainder.is_empty(),
        "VALUES_PER_UNIT must be a multiple of 256"
    );

    let mut encoded = Vec::new();
    codec.encode_blocks(blocks, &mut encoded).unwrap();
    let mut decoded = Vec::new();
    codec
        .decode_blocks(&encoded, Some(values.len() as u32), &mut decoded)
        .unwrap();
    assert_eq!(values, &decoded[..], "FastPFOR round-trip mismatch");

    let encode_start = Instant::now();
    for _ in 0..ITERATIONS {
        encoded.clear();
        codec
            .encode_blocks(std::hint::black_box(blocks), &mut encoded)
            .unwrap();
        std::hint::black_box(&encoded);
    }
    let encode_ns = encode_start.elapsed().as_nanos() as f64 / ITERATIONS as f64;

    let decode_start = Instant::now();
    for _ in 0..ITERATIONS {
        decoded.clear();
        codec
            .decode_blocks(
                std::hint::black_box(&encoded),
                Some(values.len() as u32),
                &mut decoded,
            )
            .unwrap();
        std::hint::black_box(&decoded);
    }
    let decode_ns = decode_start.elapsed().as_nanos() as f64 / ITERATIONS as f64;

    CodecResult {
        bit_width: 0, // not width-parameterized; FastPFOR chooses its own encoding per block
        decode_ns_per_1024_values: decode_ns,
        decode_values_per_sec: VALUES_PER_UNIT as f64 / (decode_ns / 1e9),
        encode_ns_per_1024_values: encode_ns,
    }
}

/// A realistic delta-gap-shaped distribution: ~95% of values are small
/// (dense/common-term short gaps, `[0, 16)`), ~5% are large "exception"
/// values (`[0, 2^24)`, a rare-term-style long gap) — the shape FastPFOR's
/// exception mechanism is designed for, unlike the uniform sweep above.
fn skewed_values(rng: &mut StdRng) -> Vec<u32> {
    (0..VALUES_PER_UNIT)
        .map(|_| {
            if rng.random_ratio(95, 100) {
                rng.random_range(0..16u32)
            } else {
                rng.random_range(0..(1u32 << 24))
            }
        })
        .collect()
}

fn bench_fastpfor_skewed(values: &[u32]) -> SkewedResult {
    use fastpfor::slice_to_blocks;

    let mut codec = FastPForBlock256::default();
    let (blocks, remainder) = slice_to_blocks::<FastPForBlock256>(values);
    assert!(remainder.is_empty());

    let mut encoded = Vec::new();
    codec.encode_blocks(blocks, &mut encoded).unwrap();
    let mut decoded = Vec::new();
    codec
        .decode_blocks(&encoded, Some(values.len() as u32), &mut decoded)
        .unwrap();
    assert_eq!(values, &decoded[..], "FastPFOR skewed round-trip mismatch");
    let compressed_bytes = encoded.len();

    let encode_start = Instant::now();
    for _ in 0..ITERATIONS {
        encoded.clear();
        codec
            .encode_blocks(std::hint::black_box(blocks), &mut encoded)
            .unwrap();
        std::hint::black_box(&encoded);
    }
    let encode_ns = encode_start.elapsed().as_nanos() as f64 / ITERATIONS as f64;

    let decode_start = Instant::now();
    for _ in 0..ITERATIONS {
        decoded.clear();
        codec
            .decode_blocks(
                std::hint::black_box(&encoded),
                Some(values.len() as u32),
                &mut decoded,
            )
            .unwrap();
        std::hint::black_box(&decoded);
    }
    let decode_ns = decode_start.elapsed().as_nanos() as f64 / ITERATIONS as f64;

    SkewedResult {
        codec: "fastpfor_256int".to_string(),
        decode_ns_per_1024_values: decode_ns,
        decode_values_per_sec: VALUES_PER_UNIT as f64 / (decode_ns / 1e9),
        encode_ns_per_1024_values: encode_ns,
        compressed_bytes_per_1024_values: compressed_bytes,
    }
}

/// `BitPacker8x` on the same skewed distribution, for comparison: plain
/// bit-packing must size every value in a block to the block's own maximum,
/// so the rare large exceptions force the whole block to a wide bit-width —
/// exactly the cost FastPFOR's exception mechanism exists to avoid.
fn bench_bitpacker8x_skewed(values: &[u32]) -> SkewedResult {
    let bp = BitPacker8x::new();
    let blocks = VALUES_PER_UNIT / BitPacker8x::BLOCK_LEN;
    let mut compressed = vec![0u8; VALUES_PER_UNIT * 4];
    let mut decompressed = vec![0u32; VALUES_PER_UNIT];

    let mut compressed_len = 0;
    let mut widths = Vec::with_capacity(blocks);
    for b in 0..blocks {
        let block = &values[b * BitPacker8x::BLOCK_LEN..(b + 1) * BitPacker8x::BLOCK_LEN];
        let width = bp.num_bits(block);
        widths.push(width);
        let out = &mut compressed[compressed_len..];
        compressed_len += bp.compress(block, out, width);
    }
    for (b, &width) in widths.iter().enumerate() {
        let block_bytes = width as usize * BitPacker8x::BLOCK_LEN / 8;
        let start: usize = widths[..b]
            .iter()
            .map(|&w| w as usize * BitPacker8x::BLOCK_LEN / 8)
            .sum();
        let src = &compressed[start..start + block_bytes];
        let dst = &mut decompressed[b * BitPacker8x::BLOCK_LEN..(b + 1) * BitPacker8x::BLOCK_LEN];
        bp.decompress(src, dst, width);
    }
    assert_eq!(
        values,
        &decompressed[..],
        "BitPacker8x skewed round-trip mismatch"
    );
    let compressed_bytes = compressed_len;

    let decode_start = Instant::now();
    for _ in 0..ITERATIONS {
        let mut offset = 0;
        for (b, &width) in widths.iter().enumerate() {
            let block_bytes = width as usize * BitPacker8x::BLOCK_LEN / 8;
            let src = &compressed[offset..offset + block_bytes];
            let dst =
                &mut decompressed[b * BitPacker8x::BLOCK_LEN..(b + 1) * BitPacker8x::BLOCK_LEN];
            bp.decompress(std::hint::black_box(src), dst, width);
            offset += block_bytes;
        }
        std::hint::black_box(&decompressed);
    }
    let decode_ns = decode_start.elapsed().as_nanos() as f64 / ITERATIONS as f64;

    SkewedResult {
        codec: "bitpacker8x_256int".to_string(),
        decode_ns_per_1024_values: decode_ns,
        decode_values_per_sec: VALUES_PER_UNIT as f64 / (decode_ns / 1e9),
        encode_ns_per_1024_values: 0.0, // not remeasured; encode cost isn't this comparison's point
        compressed_bytes_per_1024_values: compressed_bytes,
    }
}

/// Loads `bench/results/msmarco-real-postings-sample.json` (produced by
/// `bin/msmarco-index`) and pools every decile's `field` array (`d_gaps` or
/// `term_frequencies`) into one `Vec<u32>`, in the file's own decile order.
/// Returns `None` if the file doesn't exist yet — this measurement is
/// optional, run only after `cargo run -p strand-bench --bin msmarco-index`.
fn load_real_postings_field(field: &str) -> Option<Vec<u32>> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/results/msmarco-real-postings-sample.json"
    );
    let raw = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let deciles = json.get("deciles")?.as_array()?;
    let mut pooled = Vec::new();
    for decile in deciles {
        let values = decile.get(field)?.as_array()?;
        for v in values {
            pooled.push(v.as_u64()? as u32);
        }
    }
    Some(pooled)
}

/// Runs `bench_fastpfor_skewed`'s exact encode/decode/round-trip logic
/// against every full 1024-value chunk of real pooled data, averaging
/// decode_ns and compressed size across chunks.
fn bench_fastpfor_real(pooled: &[u32]) -> Option<RealCorpusResult> {
    let chunks: Vec<&[u32]> = pooled.chunks_exact(VALUES_PER_UNIT).collect();
    if chunks.is_empty() {
        return None;
    }
    let results: Vec<SkewedResult> = chunks.iter().map(|c| bench_fastpfor_skewed(c)).collect();
    Some(average_skewed_results("fastpfor_256int", &results))
}

/// `BitPacker8x` counterpart to `bench_fastpfor_real`.
fn bench_bitpacker8x_real(pooled: &[u32]) -> Option<RealCorpusResult> {
    let chunks: Vec<&[u32]> = pooled.chunks_exact(VALUES_PER_UNIT).collect();
    if chunks.is_empty() {
        return None;
    }
    let results: Vec<SkewedResult> = chunks.iter().map(|c| bench_bitpacker8x_skewed(c)).collect();
    Some(average_skewed_results("bitpacker8x_256int", &results))
}

fn average_skewed_results(codec: &str, results: &[SkewedResult]) -> RealCorpusResult {
    let n = results.len() as f64;
    RealCorpusResult {
        codec: codec.to_string(),
        chunks_measured: results.len(),
        decode_ns_per_1024_values: results
            .iter()
            .map(|r| r.decode_ns_per_1024_values)
            .sum::<f64>()
            / n,
        decode_values_per_sec: results.iter().map(|r| r.decode_values_per_sec).sum::<f64>() / n,
        encode_ns_per_1024_values: results
            .iter()
            .map(|r| r.encode_ns_per_1024_values)
            .sum::<f64>()
            / n,
        compressed_bytes_per_1024_values: results
            .iter()
            .map(|r| r.compressed_bytes_per_1024_values as f64)
            .sum::<f64>()
            / n,
    }
}

fn cpu_model() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split_once(':'))
                .map(|(_, v)| v.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn main() {
    let mut rng = StdRng::seed_from_u64(0x5741524D); // "WARM" — fixed seed, reproducible input
    let mut bitpacker4x = Vec::new();
    let mut bitpacker8x = Vec::new();
    let mut fastlanes = Vec::new();
    let mut fastpfor = Vec::new();

    for &width in &WIDTHS {
        let values = random_values(&mut rng, width);
        bitpacker4x.push(bench_bitpacker4x(&values, width));
        bitpacker8x.push(bench_bitpacker8x(&values, width));
        fastlanes.push(bench_fastlanes(&values, width));
        let mut fastpfor_result = bench_fastpfor(&values);
        fastpfor_result.bit_width = width; // label with the source width, though FastPFOR ignores it
        fastpfor.push(fastpfor_result);
        println!("width {width} done");
    }

    let skewed = skewed_values(&mut rng);
    let skewed_results = vec![
        bench_fastpfor_skewed(&skewed),
        bench_bitpacker8x_skewed(&skewed),
    ];
    println!("skewed distribution done");

    let real_msmarco_d_gaps = load_real_postings_field("d_gaps").map(|pooled| {
        println!("real d_gaps: {} pooled values", pooled.len());
        vec![
            bench_fastpfor_real(&pooled),
            bench_bitpacker8x_real(&pooled),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
    });
    let real_msmarco_term_frequencies =
        load_real_postings_field("term_frequencies").map(|pooled| {
            println!("real term_frequencies: {} pooled values", pooled.len());
            vec![
                bench_fastpfor_real(&pooled),
                bench_bitpacker8x_real(&pooled),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
        });
    if real_msmarco_d_gaps.is_none() {
        println!(
            "no bench/results/msmarco-real-postings-sample.json found — \
             run `cargo run -p strand-bench --bin msmarco-index` first to include real-corpus results"
        );
    }

    write_report(
        "codec-decode-throughput",
        AllResults {
            values_per_unit: VALUES_PER_UNIT,
            iterations: ITERATIONS,
            cpu: cpu_model(),
            bitpacker4x_128int_sse3: bitpacker4x,
            bitpacker8x_256int_avx2: bitpacker8x,
            fastlanes_1024int: fastlanes,
            fastpfor_256int_uniform: fastpfor,
            skewed_distribution_95pct_4bit_5pct_24bit: skewed_results,
            real_msmarco_d_gaps,
            real_msmarco_term_frequencies,
        },
    );
}
