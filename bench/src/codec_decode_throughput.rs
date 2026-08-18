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
//! SSE3; `BitPacker8x`, 256-int blocks, AVX2) against `fastlanes`
//! (`spiraldb/fastlanes`, 1024-int blocks) at matched bit-widths, matched
//! total value counts (1024 values per comparison unit), and matched input
//! data per width.
//!
//! **What this is not.** Not a benchmark against real MS MARCO postings
//! distributions (`docs/data-structures.md`'s own bake-off target) — this
//! generates synthetic, uniform-random values in `[0, 2^W)` per declared
//! bit-width `W`, the same data-generation convention both crates' own
//! upstream benchmarks use. A uniform distribution is the worst case for any
//! scheme that could exploit skew (it can't), so this measures raw decode
//! throughput at a fixed bit-width honestly, not full corpus-level
//! compression behavior. The R2 bake-off (`docs/milestones.md` M1) still
//! needs the real corpus; this is a narrower, still-useful first measurement
//! of the specific margin R9 asks about.

use bitpacking::{BitPacker, BitPacker4x, BitPacker8x};
use fastlanes::BitPacking as FastLanesBitPacking;
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
struct AllResults {
    values_per_unit: usize,
    iterations: usize,
    cpu: String,
    bitpacker4x_128int_sse3: Vec<CodecResult>,
    bitpacker8x_256int_avx2: Vec<CodecResult>,
    fastlanes_1024int: Vec<CodecResult>,
}

fn random_values(rng: &mut StdRng, width: u8) -> Vec<u32> {
    let max = if width == 32 {
        u32::MAX
    } else {
        (1u32 << width) - 1
    };
    (0..VALUES_PER_UNIT).map(|_| rng.random_range(0..=max)).collect()
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
    assert_eq!(values, &decompressed[..], "BitPacker4x round-trip mismatch at width {width}");

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
    assert_eq!(values, &decompressed[..], "BitPacker8x round-trip mismatch at width {width}");

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
    assert_eq!(values, &unpacked[..], "FastLanes round-trip mismatch at width {width}");

    let encode_start = Instant::now();
    for _ in 0..ITERATIONS {
        unsafe {
            FastLanesBitPacking::unchecked_pack(
                width,
                std::hint::black_box(values),
                &mut packed,
            );
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

    for &width in &WIDTHS {
        let values = random_values(&mut rng, width);
        bitpacker4x.push(bench_bitpacker4x(&values, width));
        bitpacker8x.push(bench_bitpacker8x(&values, width));
        fastlanes.push(bench_fastlanes(&values, width));
        println!("width {width} done");
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
        },
    );
}
