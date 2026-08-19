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

//! `docs/research/r2-hybrid-codec-methodology.md` Phase 1 — Cheap Separability
//! Pilot. Tests, on a sample of real postings lists, whether the BP128-vs-EF
//! winner (on size, and on skip/decode cost) is predictable from cheap,
//! precomputable per-list statistics, before committing to anything resembling
//! Phase 2B's whole-corpus dual encoding.
//!
//! Both codecs are existing, unmodified implementations (`bitpacking`'s
//! `BitPacker8x` for BP128, `sucds`'s `mii_sequences::EliasFano` for EF) — no
//! new codec engineering, per Phase 1 Step 2's own instruction. Postings lists
//! are real: a sample of MS MARCO passage-corpus per-term doc-ordinal lists,
//! built with STRAND's own analyzer chain (`strand_lexical::analyzer`), the
//! same corpus and tokenization `bin/msmarco-index` uses for R9's codec
//! measurement.

use bitpacking::{BitPacker, BitPacker8x};
use flate2::read::MultiGzDecoder;
use rand::SeedableRng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::time::Instant;
use strand_bench::write_report;
use sucds::Serializable;
use sucds::mii_sequences::EliasFanoBuilder;

#[derive(Deserialize)]
struct CorpusLine {
    text: String,
}

const REPEATS: usize = 20_000;

// ---------------------------------------------------------------------------
// Step 1: build a real inverted index over a stride-sampled corpus subset.
// Same corpus, sampling method, and tokenization as bin/msmarco-index.
// ---------------------------------------------------------------------------

fn build_index(sample_target: u64) -> (HashMap<String, Vec<u32>>, u32) {
    let data_path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/corpus.jsonl.gz");
    let file = std::fs::File::open(data_path).unwrap_or_else(|e| {
        panic!("open {data_path}: {e} — run `cargo run -p strand-bench --bin msmarco-index` first")
    });
    let reader = BufReader::with_capacity(1 << 20, MultiGzDecoder::new(file));

    const CORPUS_TOTAL_PASSAGES: u64 = 8_841_823;
    let stride = (CORPUS_TOTAL_PASSAGES / sample_target).max(1);

    let mut index: HashMap<String, Vec<u32>> = HashMap::new();
    let mut doc_ordinal: u32 = 0;

    for (line_no, line) in reader.lines().enumerate() {
        let line_no = line_no as u64;
        if !line_no.is_multiple_of(stride) {
            continue;
        }
        let line = line.unwrap_or_else(|e| panic!("read line {line_no}: {e}"));
        if line.is_empty() {
            continue;
        }
        let Ok(parsed) = serde_json::from_str::<CorpusLine>(&line) else {
            continue;
        };
        let tokens = strand_lexical::analyzer::analyze_lucene_en_word_only(&parsed.text);
        let mut seen_this_doc = std::collections::HashSet::new();
        for token in tokens {
            if seen_this_doc.insert(token.clone()) {
                index.entry(token).or_default().push(doc_ordinal);
            }
        }
        doc_ordinal += 1;
        if doc_ordinal.is_multiple_of(50_000) {
            eprintln!(
                "  {doc_ordinal} passages indexed, {} terms so far",
                index.len()
            );
        }
    }

    eprintln!(
        "Indexed {doc_ordinal} passages, {} distinct terms",
        index.len()
    );
    (index, doc_ordinal)
}

// ---------------------------------------------------------------------------
// Step 2: per-list cheap statistics and both codecs' measured size/cost.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
struct ListRecord {
    n: usize,
    universe: u32,
    gap_mean: f64,
    gap_variance: f64,
    density: f64,
    bp128_bytes: usize,
    ef_bytes: usize,
    blockmax_bytes: usize,
    bp128var_bytes: usize,
    bp128_decode_ns: f64,
    ef_decode_ns: f64,
    bp128var_decode_ns: f64,
    bp128_skip_ns: f64,
    ef_skip_ns: f64,
    blockmax_skip_ns: f64,
    bp128var_skip_ns: f64,
    winner_size_ef: bool,
    winner_skip_ef: bool,
    winner_decode_ef: bool,
    /// EF vs. the variable-final-block BP128 variant specifically, isolating
    /// whether the fixed-256-padding artifact explains the decode signal.
    winner_decode_ef_vs_var: bool,
    /// Among {plain BP128, block-max BP128, EF}, which has the fastest skip.
    skip_winner: &'static str,
}

fn gaps_of(list: &[u32]) -> Vec<u32> {
    let mut gaps = Vec::with_capacity(list.len());
    let mut prev = 0u32;
    for &v in list {
        gaps.push(v - prev);
        prev = v;
    }
    gaps
}

/// Minimal scalar bit-packer (shift/mask, no SIMD, no forced block size) for
/// the variable-length final block variant below. `bitpacking::BitPacker8x`
/// has no API for anything but a fixed 256-value block; this exists only to
/// test whether that fixed block size, not EF's own decode advantage, is
/// what the Phase 1 decode signal (`n <= 8`) was actually measuring.
fn scalar_bits_needed(values: &[u32]) -> u8 {
    let max = values.iter().copied().max().unwrap_or(0);
    if max == 0 {
        0
    } else {
        (32 - max.leading_zeros()) as u8
    }
}

fn scalar_pack(values: &[u32], width: u8) -> Vec<u8> {
    if width == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut acc: u64 = 0;
    let mut acc_bits: u32 = 0;
    for &v in values {
        acc |= (v as u64) << acc_bits;
        acc_bits += width as u32;
        while acc_bits >= 8 {
            out.push((acc & 0xFF) as u8);
            acc >>= 8;
            acc_bits -= 8;
        }
    }
    if acc_bits > 0 {
        out.push((acc & 0xFF) as u8);
    }
    out
}

fn scalar_unpack(bytes: &[u8], count: usize, width: u8) -> Vec<u32> {
    if width == 0 {
        return vec![0u32; count];
    }
    let mut out = Vec::with_capacity(count);
    let mut acc: u64 = 0;
    let mut acc_bits: u32 = 0;
    let mut byte_idx = 0;
    let mask: u64 = (1u64 << width) - 1;
    for _ in 0..count {
        while acc_bits < width as u32 {
            acc |= (bytes[byte_idx] as u64) << acc_bits;
            byte_idx += 1;
            acc_bits += 8;
        }
        out.push((acc & mask) as u32);
        acc >>= width;
        acc_bits -= width as u32;
    }
    out
}

/// BP128 with a variable-length final block: full 256-value blocks still use
/// `BitPacker8x`'s SIMD kernel exactly as `bp128_bench` does; a trailing
/// partial block (`n % 256` values, or the whole list for `n < 256`) is
/// packed with the scalar packer above at its own bit width, sized to the
/// real remaining count — no padding, no forced 256-wide decode. Tests
/// directly whether Phase 1's `n <= 8` decode signal was measuring
/// `BitPacker8x`'s fixed block-size padding overhead rather than a real
/// property of the two codecs.
fn bp128_variable_bench(list: &[u32], targets: &[u32; 3]) -> (usize, f64, f64) {
    let gaps = gaps_of(list);
    let bp = BitPacker8x::new();
    let block_len = BitPacker8x::BLOCK_LEN;
    let full_blocks = gaps.len() / block_len;
    let remainder = gaps.len() % block_len;

    let mut widths = Vec::with_capacity(full_blocks);
    let mut compressed = vec![0u8; gaps.len() * 4 + 16];
    let mut compressed_len = 0;
    for b in 0..full_blocks {
        let block = &gaps[b * block_len..(b + 1) * block_len];
        let width = bp.num_bits(block);
        widths.push(width);
        let out = &mut compressed[compressed_len..];
        compressed_len += bp.compress(block, out, width);
    }
    let tail_width = if remainder > 0 {
        scalar_bits_needed(&gaps[full_blocks * block_len..])
    } else {
        0
    };
    let tail_start = compressed_len;
    if remainder > 0 {
        let tail_bytes = scalar_pack(&gaps[full_blocks * block_len..], tail_width);
        compressed[compressed_len..compressed_len + tail_bytes.len()].copy_from_slice(&tail_bytes);
        compressed_len += tail_bytes.len();
    }
    // A real wire format needs the per-block bit width to decode; not
    // recording it (as an earlier version of this function did) understated
    // this codec's true size, especially for the single-tail-block lists
    // that dominate this sample. One byte per full block, one more for the
    // tail if present — matching bp128_bench and bp128_blockmax_bench.
    let width_metadata_bytes = full_blocks + usize::from(remainder > 0);
    let total_bytes = compressed_len + width_metadata_bytes;

    let decode_all = || -> Vec<u32> {
        let mut decompressed = vec![0u32; full_blocks * block_len];
        let mut offset = 0;
        for (b, &width) in widths.iter().enumerate() {
            let block_bytes = width as usize * block_len / 8;
            let src = &compressed[offset..offset + block_bytes];
            let dst = &mut decompressed[b * block_len..(b + 1) * block_len];
            bp.decompress(src, dst, width);
            offset += block_bytes;
        }
        if remainder > 0 {
            let tail_bytes_len = (remainder * tail_width as usize).div_ceil(8);
            let tail = scalar_unpack(
                &compressed[tail_start..tail_start + tail_bytes_len],
                remainder,
                tail_width,
            );
            decompressed.extend(tail);
        }
        let mut out = Vec::with_capacity(decompressed.len());
        let mut prev = 0u32;
        for g in decompressed {
            prev += g;
            out.push(prev);
        }
        out
    };

    assert_eq!(
        decode_all(),
        list,
        "BP128 variable-final-block round-trip mismatch"
    );

    let decode_start = Instant::now();
    for _ in 0..REPEATS {
        std::hint::black_box(decode_all());
    }
    let decode_ns = decode_start.elapsed().as_nanos() as f64 / REPEATS as f64;

    let skip_start = Instant::now();
    for _ in 0..REPEATS {
        for &t in targets {
            let decoded = decode_all();
            let pos = decoded.partition_point(|&v| v < t);
            std::hint::black_box(pos);
        }
    }
    let skip_ns = skip_start.elapsed().as_nanos() as f64 / (REPEATS * targets.len()) as f64;

    (total_bytes, decode_ns, skip_ns)
}

fn skip_targets(list: &[u32]) -> [u32; 3] {
    let n = list.len();
    [list[n / 4], list[n / 2], list[(3 * n) / 4]]
}

/// BP128 (`BitPacker8x`, 256-int blocks): delta-gap encode, one bit-width per
/// block, padded with trailing zero-gaps to a block boundary (zero never
/// increases the required width). Skip = decode-then-linear-scan, since BP128
/// has no native compressed-domain skip (`references/lemire-boytsov-simd-bp128.md`).
fn bp128_bench(list: &[u32], targets: &[u32; 3]) -> (usize, f64, f64) {
    let gaps = gaps_of(list);
    let bp = BitPacker8x::new();
    let block_len = BitPacker8x::BLOCK_LEN;
    let padded_len = gaps.len().div_ceil(block_len) * block_len;
    let mut padded = gaps.clone();
    padded.resize(padded_len, 0);

    let blocks = padded_len / block_len;
    let mut widths = Vec::with_capacity(blocks);
    let mut compressed = vec![0u8; padded_len * 4];
    let mut compressed_len = 0;
    for b in 0..blocks {
        let block = &padded[b * block_len..(b + 1) * block_len];
        let width = bp.num_bits(block);
        widths.push(width);
        let out = &mut compressed[compressed_len..];
        compressed_len += bp.compress(block, out, width);
    }
    // Real wire size includes the per-block width byte needed to decode —
    // omitting it understated this codec's true size.
    let total_bytes = compressed_len + blocks;

    let decode_all = || -> Vec<u32> {
        let mut decompressed = vec![0u32; padded_len];
        let mut offset = 0;
        for (b, &width) in widths.iter().enumerate() {
            let block_bytes = width as usize * block_len / 8;
            let src = &compressed[offset..offset + block_bytes];
            let dst = &mut decompressed[b * block_len..(b + 1) * block_len];
            bp.decompress(src, dst, width);
            offset += block_bytes;
        }
        decompressed.truncate(gaps.len());
        // Undo the delta encoding to get real doc ordinals back.
        let mut out = Vec::with_capacity(decompressed.len());
        let mut prev = 0u32;
        for g in decompressed {
            prev += g;
            out.push(prev);
        }
        out
    };

    // Correctness check once, outside the timing loop.
    assert_eq!(decode_all(), list, "BP128 round-trip mismatch");

    let decode_start = Instant::now();
    for _ in 0..REPEATS {
        std::hint::black_box(decode_all());
    }
    let decode_ns = decode_start.elapsed().as_nanos() as f64 / REPEATS as f64;

    // Decode fresh per target, not once shared across all three — matching
    // ef_bench's and bp128_blockmax_bench's methodology. Sharing one decode
    // across targets would amortize decode cost across queries that, in a
    // real workload, arrive independently; that amortization was a real bug
    // caught after block-max's skip looked implausibly slower than plain
    // decode-then-scan, which made no sense until this asymmetry was found.
    let skip_start = Instant::now();
    for _ in 0..REPEATS {
        for &t in targets {
            let decoded = decode_all();
            let pos = decoded.partition_point(|&v| v < t);
            std::hint::black_box(pos);
        }
    }
    let skip_ns = skip_start.elapsed().as_nanos() as f64 / (REPEATS * targets.len()) as f64;

    (total_bytes, decode_ns, skip_ns)
}

/// BP128 + block-max (invariant 4): identical encoding to `bp128_bench`, plus
/// a sibling array of one `u32` per block holding that block's maximum real
/// doc-ordinal value (monotonically increasing across blocks, since blocks
/// are consecutive slices of a sorted list — so it's binary-searchable). Skip
/// = binary search the tiny block-max array (untouched compressed bytes) to
/// find the one block that can contain the target, decode *only* that block,
/// then linear-scan within it. This is STRAND's own already-settled
/// invariant-4 mechanism, not a new design — implemented here to measure it
/// on the same real lists, not asserted from the spec.
fn bp128_blockmax_bench(list: &[u32], targets: &[u32; 3]) -> (usize, f64) {
    let gaps = gaps_of(list);
    let bp = BitPacker8x::new();
    let block_len = BitPacker8x::BLOCK_LEN;
    let padded_len = gaps.len().div_ceil(block_len) * block_len;
    let mut padded = gaps.clone();
    padded.resize(padded_len, 0);
    let blocks = padded_len / block_len;

    let mut widths = Vec::with_capacity(blocks);
    let mut block_offsets = Vec::with_capacity(blocks);
    let mut compressed = vec![0u8; padded_len * 4];
    let mut compressed_len = 0;
    for b in 0..blocks {
        let block = &padded[b * block_len..(b + 1) * block_len];
        let width = bp.num_bits(block);
        widths.push(width);
        block_offsets.push(compressed_len);
        let out = &mut compressed[compressed_len..];
        compressed_len += bp.compress(block, out, width);
    }

    // Block-max array: the real (post-delta) doc-ordinal value of the last
    // element each block covers — clamped to the list's real length, since
    // the final block may be padded with trailing zero-gaps.
    let mut block_max = Vec::with_capacity(blocks);
    for b in 0..blocks {
        let last_real_idx = ((b + 1) * block_len).min(list.len());
        block_max.push(list[last_real_idx - 1]);
    }
    // The real doc-ordinal value immediately preceding each block's start —
    // precomputed once (not timed), so a single-block decode+skip touches
    // exactly one block's compressed bytes, never its predecessors'. This is
    // the whole point of block-max skipping; recomputing it per skip call
    // would silently decode every preceding block on every query.
    let block_start_value: Vec<u32> = (0..blocks)
        .map(|b| {
            if b * block_len == 0 {
                0
            } else {
                list[b * block_len - 1]
            }
        })
        .collect();

    // Sibling metadata bytes: one u32 per block for the block-max array
    // (invariant 4's "raw statistics, sibling blob" shape), plus the
    // per-block width byte needed to decode at all — omitting the latter
    // understated this codec's true size, the same gap fixed in
    // bp128_bench and bp128_variable_bench.
    let total_bytes = compressed_len + blocks * 4 + blocks;

    let decode_block = |b: usize| -> Vec<u32> {
        let width = widths[b];
        let block_bytes = width as usize * block_len / 8;
        let src = &compressed[block_offsets[b]..block_offsets[b] + block_bytes];
        let mut decompressed = vec![0u32; block_len];
        bp.decompress(src, &mut decompressed, width);
        let mut out = Vec::with_capacity(block_len);
        let mut prev = block_start_value[b];
        for g in decompressed {
            prev += g;
            out.push(prev);
        }
        out
    };

    // Correctness check once, outside the timing loop: skip must agree with
    // a full decode-then-search for every target.
    let full = {
        let mut all = Vec::new();
        for b in 0..blocks {
            all.extend(decode_block(b));
        }
        all.truncate(list.len());
        all
    };
    assert_eq!(full, list, "block-max BP128 round-trip mismatch");
    for &t in targets {
        let block_idx = block_max.partition_point(|&m| m < t).min(blocks - 1);
        let decoded_block = decode_block(block_idx);
        let expected = full.partition_point(|&v| v < t);
        let found = block_idx * block_len
            + decoded_block
                .partition_point(|&v| v < t)
                .min(decoded_block.len());
        assert_eq!(
            found.min(list.len()),
            expected,
            "block-max skip disagreed with full scan"
        );
    }

    let skip_start = Instant::now();
    for _ in 0..REPEATS {
        for &t in targets {
            let block_idx = block_max.partition_point(|&m| m < t).min(blocks - 1);
            let decoded_block = decode_block(block_idx);
            std::hint::black_box(decoded_block.partition_point(|&v| v < t));
        }
    }
    let skip_ns = skip_start.elapsed().as_nanos() as f64 / (REPEATS * targets.len()) as f64;

    (total_bytes, skip_ns)
}

/// EF (`sucds::mii_sequences::EliasFano`): built directly over the raw
/// (non-delta) values, per the standard construction — EF's own bound already
/// accounts for value distribution via the shared universe.
fn ef_bench(list: &[u32], universe: u32, targets: &[u32; 3]) -> (usize, f64, f64) {
    let mut builder = EliasFanoBuilder::new(universe as usize, list.len()).unwrap();
    builder.extend(list.iter().map(|&v| v as usize)).unwrap();
    let ef = builder.build().enable_rank();
    let total_bytes = ef.size_in_bytes();

    let decode_all = || -> Vec<u32> { ef.iter(0).map(|v| v as u32).collect() };
    assert_eq!(decode_all(), list, "EF round-trip mismatch");

    let decode_start = Instant::now();
    for _ in 0..REPEATS {
        std::hint::black_box(decode_all());
    }
    let decode_ns = decode_start.elapsed().as_nanos() as f64 / REPEATS as f64;

    let skip_start = Instant::now();
    for _ in 0..REPEATS {
        for &t in targets {
            let pos = ef.successor(t as usize);
            std::hint::black_box(pos);
        }
    }
    let skip_ns = skip_start.elapsed().as_nanos() as f64 / (REPEATS * targets.len()) as f64;

    (total_bytes, decode_ns, skip_ns)
}

fn measure_list(list: &[u32], universe: u32) -> ListRecord {
    let gaps = gaps_of(list);
    let n = list.len();
    let gap_mean = gaps.iter().map(|&g| g as f64).sum::<f64>() / n as f64;
    let gap_variance = gaps
        .iter()
        .map(|&g| (g as f64 - gap_mean).powi(2))
        .sum::<f64>()
        / n as f64;
    let density = n as f64 / universe as f64;

    let targets = skip_targets(list);
    let (bp128_bytes, bp128_decode_ns, bp128_skip_ns) = bp128_bench(list, &targets);
    let (ef_bytes, ef_decode_ns, ef_skip_ns) = ef_bench(list, universe, &targets);
    let (blockmax_bytes, blockmax_skip_ns) = bp128_blockmax_bench(list, &targets);
    let (bp128var_bytes, bp128var_decode_ns, bp128var_skip_ns) =
        bp128_variable_bench(list, &targets);

    let skip_winner = [
        ("bp128", bp128_skip_ns),
        ("blockmax", blockmax_skip_ns),
        ("ef", ef_skip_ns),
    ]
    .into_iter()
    .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
    .unwrap()
    .0;

    ListRecord {
        n,
        universe,
        gap_mean,
        gap_variance,
        density,
        bp128_bytes,
        ef_bytes,
        blockmax_bytes,
        bp128var_bytes,
        bp128_decode_ns,
        ef_decode_ns,
        bp128var_decode_ns,
        bp128_skip_ns,
        ef_skip_ns,
        blockmax_skip_ns,
        bp128var_skip_ns,
        winner_size_ef: ef_bytes < bp128_bytes,
        winner_skip_ef: ef_skip_ns < bp128_skip_ns,
        winner_decode_ef: ef_decode_ns < bp128_decode_ns,
        winner_decode_ef_vs_var: ef_decode_ns < bp128var_decode_ns,
        skip_winner,
    }
}

// ---------------------------------------------------------------------------
// Step 3/4: fit the simplest candidate signal on a train split, evaluate
// out-of-sample on a held-out split.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct ThresholdSignal {
    feature: String,
    threshold: f64,
    /// true => predict EF wins when feature > threshold
    ef_wins_above: bool,
    train_accuracy: f64,
    held_out_accuracy: f64,
    held_out_majority_baseline: f64,
}

/// Sweeps candidate thresholds (every observed train-split value of the given
/// feature) and keeps the one maximizing train accuracy for predicting
/// `label` — the single simplest per-feature rule, per Phase 1 Step 3.
fn fit_threshold(
    train: &[(f64, bool)],
    held_out: &[(f64, bool)],
    feature_name: &str,
) -> ThresholdSignal {
    let mut candidates: Vec<f64> = train.iter().map(|(f, _)| *f).collect();
    candidates.sort_by(|a, b| a.partial_cmp(b).unwrap());
    candidates.dedup();

    let eval = |threshold: f64, ef_wins_above: bool, data: &[(f64, bool)]| -> f64 {
        let correct = data
            .iter()
            .filter(|(f, label)| {
                let predicted_ef_wins = if ef_wins_above {
                    *f > threshold
                } else {
                    *f <= threshold
                };
                predicted_ef_wins == *label
            })
            .count();
        correct as f64 / data.len() as f64
    };

    let mut best = (f64::MIN, 0.0, true);
    for &t in &candidates {
        for &ef_wins_above in &[true, false] {
            let acc = eval(t, ef_wins_above, train);
            if acc > best.0 {
                best = (acc, t, ef_wins_above);
            }
        }
    }
    let (train_accuracy, threshold, ef_wins_above) = best;

    let held_out_accuracy = eval(threshold, ef_wins_above, held_out);
    let ef_wins_count = held_out.iter().filter(|(_, l)| *l).count();
    let majority = ef_wins_count.max(held_out.len() - ef_wins_count) as f64 / held_out.len() as f64;

    ThresholdSignal {
        feature: feature_name.to_string(),
        threshold,
        ef_wins_above,
        train_accuracy,
        held_out_accuracy,
        held_out_majority_baseline: majority,
    }
}

#[derive(Serialize)]
struct PilotResults {
    sampled_passages: u32,
    lists_total: usize,
    lists_train: usize,
    lists_held_out: usize,
    ef_wins_size_fraction: f64,
    ef_wins_size_vs_var_fraction: f64,
    ef_wins_skip_fraction: f64,
    ef_wins_decode_fraction: f64,
    ef_wins_decode_vs_var_fraction: f64,
    mean_bp128_decode_ns: f64,
    mean_ef_decode_ns: f64,
    mean_bp128var_decode_ns: f64,
    mean_bp128_skip_ns: f64,
    mean_ef_skip_ns: f64,
    mean_blockmax_skip_ns: f64,
    mean_bp128_bytes: f64,
    mean_ef_bytes: f64,
    mean_blockmax_bytes: f64,
    skip_winner_bp128_fraction: f64,
    skip_winner_blockmax_fraction: f64,
    skip_winner_ef_fraction: f64,
    size_signals: Vec<ThresholdSignal>,
    skip_signals: Vec<ThresholdSignal>,
    decode_signals: Vec<ThresholdSignal>,
    decode_var_signals: Vec<ThresholdSignal>,
    size_interaction_signal: ThresholdSignal,
    skip_interaction_signal: ThresholdSignal,
    decode_interaction_signal: ThresholdSignal,
    decode_var_interaction_signal: ThresholdSignal,
    go_no_go: String,
    phase2b_skip_ceiling_gain: f64,
    phase2b_decode_ceiling_gain: f64,
    phase2b_size_ceiling_gain: f64,
}

fn main() {
    eprintln!("Phase 1: building real inverted index...");
    let (index, sampled_passages) = build_index(500_000);

    // Sample lists with n >= 2 (a skip/gap comparison is meaningless for a
    // single-posting list), spanning the full document-frequency spectrum via
    // systematic sampling over doc-frequency-sorted order, target ~4000 lists.
    let mut terms: Vec<(&String, &Vec<u32>)> = index.iter().filter(|(_, v)| v.len() >= 2).collect();
    // Sort by (length, term) — length alone leaves ties broken by HashMap
    // iteration order, which is randomized per-process (SipHash with a
    // random seed), silently sampling a different set of lists on every run.
    terms.sort_by(|(term_a, list_a), (term_b, list_b)| {
        list_a
            .len()
            .cmp(&list_b.len())
            .then_with(|| term_a.cmp(term_b))
    });

    const TARGET_LISTS: usize = 4000;
    let stride = (terms.len() / TARGET_LISTS).max(1);
    let sampled: Vec<&Vec<u32>> = terms.iter().step_by(stride).map(|(_, v)| *v).collect();
    eprintln!(
        "Sampling {} of {} eligible lists (stride {stride}) for measurement",
        sampled.len(),
        terms.len()
    );

    let mut records = Vec::with_capacity(sampled.len());
    for (i, list) in sampled.iter().enumerate() {
        records.push(measure_list(list, sampled_passages));
        if (i + 1).is_multiple_of(500) {
            eprintln!("  measured {}/{}", i + 1, sampled.len());
        }
    }

    // 70/30 train/held-out split via a fixed-seed shuffle — `records` is in
    // length-sorted order (an artifact of the sampling stride above), and a
    // positional split on sorted data risks systematic bias if the outcome
    // correlates with length, which it plausibly does here. The fixed seed
    // keeps the split reproducible without a positional artifact.
    let mut shuffled_indices: Vec<usize> = (0..records.len()).collect();
    let mut rng = rand::rngs::StdRng::seed_from_u64(0x0050_4841_5331); // "PHAS1"
    shuffled_indices.shuffle(&mut rng);
    let split_at = (records.len() * 7) / 10;
    let train: Vec<&ListRecord> = shuffled_indices[..split_at]
        .iter()
        .map(|&i| &records[i])
        .collect();
    let held_out: Vec<&ListRecord> = shuffled_indices[split_at..]
        .iter()
        .map(|&i| &records[i])
        .collect();
    eprintln!("Split: {} train, {} held-out", train.len(), held_out.len());

    let size_train: Vec<(f64, bool)> = train.iter().map(|r| (0.0, r.winner_size_ef)).collect();
    let skip_train: Vec<(f64, bool)> = train.iter().map(|r| (0.0, r.winner_skip_ef)).collect();

    type FeatureExtractor = fn(&ListRecord) -> f64;
    let feature_extractors: Vec<(&str, FeatureExtractor)> = vec![
        ("gap_variance", |r: &ListRecord| r.gap_variance),
        ("log_gap_variance", |r: &ListRecord| {
            (r.gap_variance + 1.0).ln()
        }),
        ("n", |r: &ListRecord| r.n as f64),
        ("density", |r: &ListRecord| r.density),
        ("gap_mean", |r: &ListRecord| r.gap_mean),
    ];

    let mut size_signals = Vec::new();
    let mut skip_signals = Vec::new();
    let mut decode_signals = Vec::new();
    let mut decode_var_signals = Vec::new();
    for (name, extractor) in &feature_extractors {
        let tr: Vec<(f64, bool)> = train
            .iter()
            .map(|r| (extractor(r), r.winner_size_ef))
            .collect();
        let ho: Vec<(f64, bool)> = held_out
            .iter()
            .map(|r| (extractor(r), r.winner_size_ef))
            .collect();
        size_signals.push(fit_threshold(&tr, &ho, name));

        let tr: Vec<(f64, bool)> = train
            .iter()
            .map(|r| (extractor(r), r.winner_skip_ef))
            .collect();
        let ho: Vec<(f64, bool)> = held_out
            .iter()
            .map(|r| (extractor(r), r.winner_skip_ef))
            .collect();
        skip_signals.push(fit_threshold(&tr, &ho, name));

        let tr: Vec<(f64, bool)> = train
            .iter()
            .map(|r| (extractor(r), r.winner_decode_ef))
            .collect();
        let ho: Vec<(f64, bool)> = held_out
            .iter()
            .map(|r| (extractor(r), r.winner_decode_ef))
            .collect();
        decode_signals.push(fit_threshold(&tr, &ho, name));

        let tr: Vec<(f64, bool)> = train
            .iter()
            .map(|r| (extractor(r), r.winner_decode_ef_vs_var))
            .collect();
        let ho: Vec<(f64, bool)> = held_out
            .iter()
            .map(|r| (extractor(r), r.winner_decode_ef_vs_var))
            .collect();
        decode_var_signals.push(fit_threshold(&tr, &ho, name));
    }
    let _ = (size_train, skip_train);

    // Step 4: one pairwise-interaction term, gap_variance * density.
    let interaction = |r: &ListRecord| r.gap_variance * r.density;
    let tr: Vec<(f64, bool)> = train
        .iter()
        .map(|r| (interaction(r), r.winner_size_ef))
        .collect();
    let ho: Vec<(f64, bool)> = held_out
        .iter()
        .map(|r| (interaction(r), r.winner_size_ef))
        .collect();
    let size_interaction_signal = fit_threshold(&tr, &ho, "gap_variance*density");

    let tr: Vec<(f64, bool)> = train
        .iter()
        .map(|r| (interaction(r), r.winner_skip_ef))
        .collect();
    let ho: Vec<(f64, bool)> = held_out
        .iter()
        .map(|r| (interaction(r), r.winner_skip_ef))
        .collect();
    let skip_interaction_signal = fit_threshold(&tr, &ho, "gap_variance*density");

    let tr: Vec<(f64, bool)> = train
        .iter()
        .map(|r| (interaction(r), r.winner_decode_ef))
        .collect();
    let ho: Vec<(f64, bool)> = held_out
        .iter()
        .map(|r| (interaction(r), r.winner_decode_ef))
        .collect();
    let decode_interaction_signal = fit_threshold(&tr, &ho, "gap_variance*density");

    let tr: Vec<(f64, bool)> = train
        .iter()
        .map(|r| (interaction(r), r.winner_decode_ef_vs_var))
        .collect();
    let ho: Vec<(f64, bool)> = held_out
        .iter()
        .map(|r| (interaction(r), r.winner_decode_ef_vs_var))
        .collect();
    let decode_var_interaction_signal = fit_threshold(&tr, &ho, "gap_variance*density");

    // GO/NO-GO: a signal counts only if it beats its own held-out majority
    // baseline by a real margin (not just "different from chance") — 10
    // percentage points, a threshold fixed here rather than post-hoc.
    const REAL_SIGNAL_MARGIN: f64 = 0.10;
    let best_size = size_signals
        .iter()
        .chain(std::iter::once(&size_interaction_signal))
        .max_by(|a, b| {
            a.held_out_accuracy
                .partial_cmp(&b.held_out_accuracy)
                .unwrap()
        })
        .unwrap();
    let best_skip = skip_signals
        .iter()
        .chain(std::iter::once(&skip_interaction_signal))
        .max_by(|a, b| {
            a.held_out_accuracy
                .partial_cmp(&b.held_out_accuracy)
                .unwrap()
        })
        .unwrap();
    let best_decode = decode_signals
        .iter()
        .chain(std::iter::once(&decode_interaction_signal))
        .max_by(|a, b| {
            a.held_out_accuracy
                .partial_cmp(&b.held_out_accuracy)
                .unwrap()
        })
        .unwrap();
    let size_signal_found =
        best_size.held_out_accuracy - best_size.held_out_majority_baseline >= REAL_SIGNAL_MARGIN;
    let skip_signal_found =
        best_skip.held_out_accuracy - best_skip.held_out_majority_baseline >= REAL_SIGNAL_MARGIN;
    let decode_signal_found = best_decode.held_out_accuracy
        - best_decode.held_out_majority_baseline
        >= REAL_SIGNAL_MARGIN;

    let go_no_go = if size_signal_found || skip_signal_found || decode_signal_found {
        format!(
            "GO: out-of-sample signal found (size={size_signal_found} via {}, skip={skip_signal_found} via {}, decode={decode_signal_found} via {}) -> proceed to Phase 2B",
            best_size.feature, best_skip.feature, best_decode.feature
        )
    } else {
        "NO-GO: no out-of-sample signal found on any univariate feature or the pairwise interaction, even after Step 4's extension -> stop; necessary-but-not-sufficient screen only, does not rule out a signal only visible to a full learned chooser".to_string()
    };
    eprintln!("{go_no_go}");

    let ef_wins_size_fraction =
        records.iter().filter(|r| r.winner_size_ef).count() as f64 / records.len() as f64;
    // Fair comparison: winner_size_ef used the padded bp128_bytes. Checking
    // per-list against bp128var_bytes too, not just the aggregate mean —
    // the padding bug already inflated one aggregate mean (decode) in a way
    // a per-list check caught; verify size the same way before trusting it.
    let ef_wins_size_vs_var_fraction = records
        .iter()
        .filter(|r| r.ef_bytes < r.bp128var_bytes)
        .count() as f64
        / records.len() as f64;
    let ef_wins_skip_fraction =
        records.iter().filter(|r| r.winner_skip_ef).count() as f64 / records.len() as f64;
    let ef_wins_decode_fraction = records
        .iter()
        .filter(|r| r.ef_decode_ns < r.bp128_decode_ns)
        .count() as f64
        / records.len() as f64;
    let n = records.len() as f64;
    let mean_bp128_decode_ns = records.iter().map(|r| r.bp128_decode_ns).sum::<f64>() / n;
    let mean_ef_decode_ns = records.iter().map(|r| r.ef_decode_ns).sum::<f64>() / n;
    let mean_bp128_skip_ns = records.iter().map(|r| r.bp128_skip_ns).sum::<f64>() / n;
    let mean_ef_skip_ns = records.iter().map(|r| r.ef_skip_ns).sum::<f64>() / n;
    let mean_blockmax_skip_ns = records.iter().map(|r| r.blockmax_skip_ns).sum::<f64>() / n;
    let mean_bp128_bytes = records.iter().map(|r| r.bp128_bytes as f64).sum::<f64>() / n;
    let mean_ef_bytes = records.iter().map(|r| r.ef_bytes as f64).sum::<f64>() / n;
    let mean_blockmax_bytes = records.iter().map(|r| r.blockmax_bytes as f64).sum::<f64>() / n;
    let skip_winner_bp128_fraction =
        records.iter().filter(|r| r.skip_winner == "bp128").count() as f64 / n;
    let skip_winner_blockmax_fraction = records
        .iter()
        .filter(|r| r.skip_winner == "blockmax")
        .count() as f64
        / n;
    let skip_winner_ef_fraction =
        records.iter().filter(|r| r.skip_winner == "ef").count() as f64 / n;
    eprintln!(
        "EF wins on size (vs padded BP128): {:.1}% of lists; EF wins on size (vs variable-block BP128): {:.1}%; EF wins on skip (vs plain BP128): {:.1}%; EF wins on full decode: {:.1}%",
        ef_wins_size_fraction * 100.0,
        ef_wins_size_vs_var_fraction * 100.0,
        ef_wins_skip_fraction * 100.0,
        ef_wins_decode_fraction * 100.0
    );
    eprintln!(
        "mean decode ns/list: BP128={mean_bp128_decode_ns:.1} EF={mean_ef_decode_ns:.1} | mean skip ns/query: BP128={mean_bp128_skip_ns:.1} blockmax={mean_blockmax_skip_ns:.1} EF={mean_ef_skip_ns:.1}"
    );
    eprintln!(
        "three-way skip winner: bp128={:.1}% blockmax={:.1}% ef={:.1}%",
        skip_winner_bp128_fraction * 100.0,
        skip_winner_blockmax_fraction * 100.0,
        skip_winner_ef_fraction * 100.0
    );
    eprintln!(
        "mean bytes/list: BP128={mean_bp128_bytes:.1} blockmax={mean_blockmax_bytes:.1} EF={mean_ef_bytes:.1}"
    );

    // Variable-length-final-block BP128 vs EF, isolating whether the n<=8
    // decode signal from the first Phase 1 pass was really about
    // BitPacker8x's fixed 256-value block padding rather than a fundamental
    // property. Reported both overall and specifically for n<=8, the exact
    // regime the original signal was about.
    let mean_bp128var_decode_ns = records.iter().map(|r| r.bp128var_decode_ns).sum::<f64>() / n;
    let ef_wins_decode_vs_var_fraction =
        records.iter().filter(|r| r.winner_decode_ef_vs_var).count() as f64 / n;
    eprintln!(
        "variable-final-block BP128: mean decode ns/list={mean_bp128var_decode_ns:.1} (fixed-block was {mean_bp128_decode_ns:.1}) | EF still wins decode on {:.1}% of lists (was {:.1}% vs fixed-block)",
        ef_wins_decode_vs_var_fraction * 100.0,
        ef_wins_decode_fraction * 100.0
    );
    let short: Vec<&ListRecord> = records.iter().filter(|r| r.n <= 8).collect();
    if !short.is_empty() {
        let s = short.len() as f64;
        let mean_bp128_short = short.iter().map(|r| r.bp128_decode_ns).sum::<f64>() / s;
        let mean_bp128var_short = short.iter().map(|r| r.bp128var_decode_ns).sum::<f64>() / s;
        let mean_ef_short = short.iter().map(|r| r.ef_decode_ns).sum::<f64>() / s;
        let ef_wins_fixed = short.iter().filter(|r| r.winner_decode_ef).count() as f64 / s;
        let ef_wins_var = short.iter().filter(|r| r.winner_decode_ef_vs_var).count() as f64 / s;
        eprintln!(
            "n<=8 lists only ({} of {} lists): mean decode ns: BP128-fixed={mean_bp128_short:.1} BP128-variable={mean_bp128var_short:.1} EF={mean_ef_short:.1} | EF wins vs fixed-block on {:.1}%, EF wins vs variable-block on {:.1}%",
            short.len(),
            records.len(),
            ef_wins_fixed * 100.0,
            ef_wins_var * 100.0
        );
    }

    // Block-max only has an opportunity to skip whole blocks on lists
    // spanning more than one BitPacker8x block (256 values) — report it
    // separately from the short-list-dominated aggregate above, which would
    // otherwise hide whatever real advantage it has on long lists.
    let multi_block: Vec<&ListRecord> = records
        .iter()
        .filter(|r| r.n > BitPacker8x::BLOCK_LEN)
        .collect();
    if !multi_block.is_empty() {
        let m = multi_block.len() as f64;
        let mean_bp128 = multi_block.iter().map(|r| r.bp128_skip_ns).sum::<f64>() / m;
        let mean_blockmax = multi_block.iter().map(|r| r.blockmax_skip_ns).sum::<f64>() / m;
        let mean_ef = multi_block.iter().map(|r| r.ef_skip_ns).sum::<f64>() / m;
        let blockmax_beats_bp128 = multi_block
            .iter()
            .filter(|r| r.blockmax_skip_ns < r.bp128_skip_ns)
            .count() as f64
            / m;
        eprintln!(
            "multi-block lists only (n>{}, {} of {} lists): mean skip ns: BP128={mean_bp128:.1} blockmax={mean_blockmax:.1} EF={mean_ef:.1} | blockmax beats plain BP128 on {:.1}% of these",
            BitPacker8x::BLOCK_LEN,
            multi_block.len(),
            records.len(),
            blockmax_beats_bp128 * 100.0
        );
    } else {
        eprintln!(
            "no lists longer than one BitPacker8x block ({}) in this sample",
            BitPacker8x::BLOCK_LEN
        );
    }

    // -----------------------------------------------------------------
    // Phase 2B intermediate checkpoint (per its own stated off-ramp: "on a
    // growing corpus subset, if the achievable ceiling is already collapsing
    // toward one codec's baseline, stop before finishing the full corpus").
    // Uses only data already collected above — free to compute, and the
    // plan-sanctioned reason to decide whether the full harness (alternation
    // cost, chooser error cost, engineering surface) is worth building at
    // all before building it.
    // -----------------------------------------------------------------
    let m = records.len() as f64;
    // The realistic pure-BP128-family baseline is BP128+block-max together
    // (invariant 4 is already-designed, not optional) — using block-max only
    // where it actually helps (a static n>256 check, not an oracle peek).
    let bp128_family_skip = |r: &ListRecord| {
        if r.n > BitPacker8x::BLOCK_LEN {
            r.blockmax_skip_ns
        } else {
            r.bp128_skip_ns
        }
    };
    let oracle_skip_mean = records
        .iter()
        .map(|r| bp128_family_skip(r).min(r.ef_skip_ns))
        .sum::<f64>()
        / m;
    let bp128_family_skip_mean = records.iter().map(bp128_family_skip).sum::<f64>() / m;
    let ef_only_skip_mean = records.iter().map(|r| r.ef_skip_ns).sum::<f64>() / m;
    let better_pure_skip = bp128_family_skip_mean.min(ef_only_skip_mean);
    let skip_ceiling_gain = (better_pure_skip - oracle_skip_mean) / better_pure_skip;

    let oracle_decode_mean = records
        .iter()
        .map(|r| r.bp128var_decode_ns.min(r.ef_decode_ns))
        .sum::<f64>()
        / m;
    let bp128var_decode_mean = records.iter().map(|r| r.bp128var_decode_ns).sum::<f64>() / m;
    let ef_only_decode_mean = records.iter().map(|r| r.ef_decode_ns).sum::<f64>() / m;
    let better_pure_decode = bp128var_decode_mean.min(ef_only_decode_mean);
    let decode_ceiling_gain = (better_pure_decode - oracle_decode_mean) / better_pure_decode;

    // Fair baseline uses bp128var_bytes (the already-established better BP128
    // encoding, no padding waste), not the padded bp128_bytes — using the
    // padded figure here would let the "oracle" silently rediscover the
    // encoder fix already made rather than measure real chooser value.
    let oracle_bytes_mean = records
        .iter()
        .map(|r| (r.bp128var_bytes as f64).min(r.ef_bytes as f64))
        .sum::<f64>()
        / m;
    let bp128var_bytes_mean = records.iter().map(|r| r.bp128var_bytes as f64).sum::<f64>() / m;
    let ef_bytes_mean = records.iter().map(|r| r.ef_bytes as f64).sum::<f64>() / m;
    let better_pure_bytes = bp128var_bytes_mean.min(ef_bytes_mean);
    let size_ceiling_gain = (better_pure_bytes - oracle_bytes_mean) / better_pure_bytes;

    eprintln!("=== Phase 2B checkpoint: oracle ceiling over the better pure baseline ===");
    eprintln!(
        "skip:   oracle={oracle_skip_mean:.1}ns  bp128-family={bp128_family_skip_mean:.1}ns  ef={ef_only_skip_mean:.1}ns  ceiling_gain={:.1}%",
        skip_ceiling_gain * 100.0
    );
    eprintln!(
        "decode: oracle={oracle_decode_mean:.1}ns  bp128-variable={bp128var_decode_mean:.1}ns  ef={ef_only_decode_mean:.1}ns  ceiling_gain={:.1}%",
        decode_ceiling_gain * 100.0
    );
    eprintln!(
        "size:   oracle={oracle_bytes_mean:.1}B  bp128-variable={bp128var_bytes_mean:.1}B  ef={ef_bytes_mean:.1}B  ceiling_gain={:.1}%",
        size_ceiling_gain * 100.0
    );

    write_report(
        "hybrid-codec-pilot",
        PilotResults {
            sampled_passages,
            lists_total: records.len(),
            lists_train: train.len(),
            lists_held_out: held_out.len(),
            ef_wins_size_fraction,
            ef_wins_size_vs_var_fraction,
            ef_wins_skip_fraction,
            ef_wins_decode_fraction,
            ef_wins_decode_vs_var_fraction,
            mean_bp128_decode_ns,
            mean_ef_decode_ns,
            mean_bp128var_decode_ns,
            mean_bp128_skip_ns,
            mean_ef_skip_ns,
            mean_blockmax_skip_ns,
            mean_bp128_bytes,
            mean_ef_bytes,
            mean_blockmax_bytes,
            skip_winner_bp128_fraction,
            skip_winner_blockmax_fraction,
            skip_winner_ef_fraction,
            size_signals,
            skip_signals,
            decode_signals,
            decode_var_signals,
            size_interaction_signal,
            skip_interaction_signal,
            decode_interaction_signal,
            decode_var_interaction_signal,
            go_no_go,
            phase2b_skip_ceiling_gain: skip_ceiling_gain,
            phase2b_decode_ceiling_gain: decode_ceiling_gain,
            phase2b_size_ceiling_gain: size_ceiling_gain,
        },
    );
}
