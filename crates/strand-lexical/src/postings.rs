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

//! The postings blob for the lexical family: per-term doc-ID delta-gaps and
//! term frequencies. Layout is normative per `spec/postings.md`, approved by
//! RFC 0007 (`rfcs/0007-postings-codec.md`).

use bitpacking::{BitPacker, BitPacker8x};
use strand_core::batch::BatchReader;

/// `BitPacker8x`'s native block size (`spec/postings.md` §3).
pub const BLOCK_LEN: usize = BitPacker8x::BLOCK_LEN;

/// Minimal scalar bit-packer (shift/mask, no SIMD, no forced block size) for
/// the variable-length final block (`spec/postings.md` §3) — full blocks use
/// `BitPacker8x`'s SIMD kernel; only a trailing partial block uses this.
pub(crate) fn scalar_bits_needed(values: &[u32]) -> u8 {
    let max = values.iter().copied().max().unwrap_or(0);
    if max == 0 { 0 } else { (32 - max.leading_zeros()) as u8 }
}

pub(crate) fn scalar_pack(values: &[u32], width: u8) -> Vec<u8> {
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

pub(crate) fn scalar_unpack(bytes: &[u8], count: usize, width: u8) -> Vec<u32> {
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

fn gaps_of(ordinals: &[u32]) -> Vec<u32> {
    let mut gaps = Vec::with_capacity(ordinals.len());
    let mut prev = 0u32;
    for &v in ordinals {
        gaps.push(v - prev);
        prev = v;
    }
    gaps
}

pub(crate) fn block_count_for(doc_freq: usize) -> usize {
    doc_freq.div_ceil(BLOCK_LEN)
}

pub(crate) fn block_real_len(doc_freq: usize, block_idx: usize) -> usize {
    let start = block_idx * BLOCK_LEN;
    (doc_freq - start).min(BLOCK_LEN)
}

/// Builds a postings blob (`spec/postings.md` §4) from a term's local
/// ordinals (strictly increasing) and their term frequencies. `doc_freq`
/// (the caller already has it — it becomes `TermInfo.doc_freq`,
/// `spec/term-dictionary.md` §3) is `ordinals.len()`; it is intentionally
/// never stored in the returned bytes (§3).
///
/// # Panics
///
/// Panics if `ordinals` and `term_freqs` have different lengths, if either is
/// empty, or if `ordinals` is not strictly increasing.
pub fn build_postings(ordinals: &[u32], term_freqs: &[u32]) -> Vec<u8> {
    assert_eq!(ordinals.len(), term_freqs.len(), "ordinals and term_freqs must have equal length");
    assert!(!ordinals.is_empty(), "a postings list must be non-empty");
    assert!(
        ordinals.windows(2).all(|w| w[0] < w[1]),
        "ordinals must be strictly increasing"
    );

    let doc_freq = ordinals.len();
    let block_count = block_count_for(doc_freq);
    let full_blocks = doc_freq / BLOCK_LEN;
    let gaps = gaps_of(ordinals);
    let bp = BitPacker8x::new();

    let mut block_max = Vec::with_capacity(block_count);
    let mut gap_widths = Vec::with_capacity(block_count);
    let mut tf_widths = Vec::with_capacity(block_count);
    let mut gap_stream = Vec::new();
    let mut tf_stream = Vec::new();

    for b in 0..block_count {
        let start = b * BLOCK_LEN;
        let real_len = block_real_len(doc_freq, b);
        let end = start + real_len;
        block_max.push(ordinals[end - 1]);

        let block_gaps = &gaps[start..end];
        let block_tf = &term_freqs[start..end];

        if b < full_blocks {
            let width = bp.num_bits(block_gaps);
            gap_widths.push(width);
            let mut buf = vec![0u8; BLOCK_LEN * 4];
            let len = bp.compress(block_gaps, &mut buf, width);
            gap_stream.extend_from_slice(&buf[..len]);

            let tf_width = bp.num_bits(block_tf);
            tf_widths.push(tf_width);
            let mut tf_buf = vec![0u8; BLOCK_LEN * 4];
            let tf_len = bp.compress(block_tf, &mut tf_buf, tf_width);
            tf_stream.extend_from_slice(&tf_buf[..tf_len]);
        } else {
            let width = scalar_bits_needed(block_gaps);
            gap_widths.push(width);
            gap_stream.extend_from_slice(&scalar_pack(block_gaps, width));

            let tf_width = scalar_bits_needed(block_tf);
            tf_widths.push(tf_width);
            tf_stream.extend_from_slice(&scalar_pack(block_tf, tf_width));
        }
    }

    let mut out =
        Vec::with_capacity(4 * block_count + 2 * block_count + gap_stream.len() + tf_stream.len());
    for &m in &block_max {
        out.extend_from_slice(&m.to_le_bytes());
    }
    out.extend_from_slice(&gap_widths);
    out.extend_from_slice(&tf_widths);
    out.extend_from_slice(&gap_stream);
    out.extend_from_slice(&tf_stream);
    out
}

/// A resident postings blob (`spec/postings.md` §4–§6). `doc_freq` is
/// supplied externally (from `TermInfo`, `spec/term-dictionary.md` §3) —
/// this blob never stores it, so `block_count` is computed, not read.
#[derive(Debug, Clone, Copy)]
pub struct PostingsReader<'a> {
    bytes: &'a [u8],
    doc_freq: usize,
    block_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostingsError {
    Truncated,
}

impl<'a> PostingsReader<'a> {
    pub fn new(bytes: &'a [u8], doc_freq: usize) -> Result<Self, PostingsError> {
        let block_count = block_count_for(doc_freq);
        let min_len = 4 * block_count + 2 * block_count;
        if bytes.len() < min_len {
            return Err(PostingsError::Truncated);
        }
        Ok(PostingsReader { bytes, doc_freq, block_count })
    }

    fn block_max_region(&self) -> &'a [u8] {
        &self.bytes[0..4 * self.block_count]
    }

    fn gap_widths_region(&self) -> &'a [u8] {
        let start = 4 * self.block_count;
        &self.bytes[start..start + self.block_count]
    }

    fn tf_widths_region(&self) -> &'a [u8] {
        let start = 4 * self.block_count + self.block_count;
        &self.bytes[start..start + self.block_count]
    }

    fn streams(&self) -> &'a [u8] {
        &self.bytes[4 * self.block_count + 2 * self.block_count..]
    }

    /// The real (post-delta) local ordinal of the last posting the `i`-th
    /// block covers (`spec/postings.md` §5) — monotonically increasing,
    /// binary-searchable.
    pub fn block_max(&self, block_idx: usize) -> u32 {
        let region = self.block_max_region();
        u32::from_le_bytes(region[block_idx * 4..block_idx * 4 + 4].try_into().unwrap())
    }

    /// Byte length of the `i`-th block's packed region in one stream, given
    /// that block's own bit width.
    fn packed_len(&self, block_idx: usize, width: u8) -> usize {
        let real_len = block_real_len(self.doc_freq, block_idx);
        (real_len * width as usize).div_ceil(8)
    }

    fn stream_offset(&self, widths: &[u8], block_idx: usize) -> usize {
        (0..block_idx).map(|b| self.packed_len(b, widths[b])).sum()
    }

    /// Decodes exactly the `i`-th block's gaps and term frequencies —
    /// touches only that block's compressed bytes, per §6's skip contract.
    fn decode_block(&self, block_idx: usize) -> (Vec<u32>, Vec<u32>) {
        let full_blocks = self.doc_freq / BLOCK_LEN;
        let gap_widths = self.gap_widths_region();
        let tf_widths = self.tf_widths_region();
        let gap_width = gap_widths[block_idx];
        let tf_width = tf_widths[block_idx];
        let real_len = block_real_len(self.doc_freq, block_idx);

        let gap_off = self.stream_offset(gap_widths, block_idx);
        let gap_len = self.packed_len(block_idx, gap_width);
        let streams = self.streams();
        let gap_bytes = &streams[gap_off..gap_off + gap_len];

        // Term-frequency stream starts right after the whole gap stream.
        let gap_stream_total: usize = (0..self.block_count).map(|b| self.packed_len(b, gap_widths[b])).sum();
        let tf_off = gap_stream_total + self.stream_offset(tf_widths, block_idx);
        let tf_len = self.packed_len(block_idx, tf_width);
        let tf_bytes = &streams[tf_off..tf_off + tf_len];

        if block_idx < full_blocks {
            let bp = BitPacker8x::new();
            let mut gaps = vec![0u32; BLOCK_LEN];
            bp.decompress(gap_bytes, &mut gaps, gap_width);
            let mut tfs = vec![0u32; BLOCK_LEN];
            bp.decompress(tf_bytes, &mut tfs, tf_width);
            (gaps, tfs)
        } else {
            (scalar_unpack(gap_bytes, real_len, gap_width), scalar_unpack(tf_bytes, real_len, tf_width))
        }
    }

    /// Full decode (`spec/postings.md` §6): every posting's real local
    /// ordinal and term frequency, in order.
    pub fn decode_all(&self) -> (Vec<u32>, Vec<u32>) {
        let mut ordinals = Vec::with_capacity(self.doc_freq);
        let mut term_freqs = Vec::with_capacity(self.doc_freq);
        let mut prev = 0u32;
        for b in 0..self.block_count {
            let (gaps, tfs) = self.decode_block(b);
            for (g, tf) in gaps.into_iter().zip(tfs) {
                prev += g;
                ordinals.push(prev);
                term_freqs.push(tf);
            }
        }
        (ordinals, term_freqs)
    }

    /// Skip query (`spec/postings.md` §6): the first posting with a local
    /// ordinal `>= target`, or `None` if no such posting exists. Binary
    /// searches `block_max` (untouched compressed bytes), then decodes only
    /// the one candidate block.
    pub fn skip(&self, target: u32) -> Option<(u32, u32)> {
        let mut lo = 0usize;
        let mut hi = self.block_count;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.block_max(mid) < target {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo >= self.block_count {
            return None;
        }
        let (gaps, tfs) = self.decode_block(lo);
        let mut prev = if lo == 0 { 0 } else { self.block_max(lo - 1) };
        for (g, tf) in gaps.into_iter().zip(tfs) {
            prev += g;
            if prev >= target {
                return Some((prev, tf));
            }
        }
        None
    }

    /// A batch-shaped cursor over this reader's postings (invariant 9,
    /// `CLAUDE.md` §5), yielding one block's `(ordinal, term_freq)` pairs
    /// per `next_batch` call — the natural batch unit here, since a block
    /// is already the reader's unit of independent decode.
    pub fn batches(&self) -> PostingsBatchReader<'a> {
        PostingsBatchReader { reader: *self, next_block: 0, prev_ordinal: 0 }
    }
}

/// Batch-shaped cursor produced by [`PostingsReader::batches`]. Each
/// [`BatchReader::next_batch`] call decodes exactly one block.
#[derive(Debug, Clone, Copy)]
pub struct PostingsBatchReader<'a> {
    reader: PostingsReader<'a>,
    next_block: usize,
    prev_ordinal: u32,
}

impl<'a> BatchReader for PostingsBatchReader<'a> {
    type Item = (u32, u32);

    fn next_batch(&mut self, out: &mut Vec<Self::Item>) -> usize {
        if self.next_block >= self.reader.block_count {
            return 0;
        }
        let (gaps, tfs) = self.reader.decode_block(self.next_block);
        let mut count = 0;
        for (g, tf) in gaps.into_iter().zip(tfs) {
            self.prev_ordinal += g;
            out.push((self.prev_ordinal, tf));
            count += 1;
        }
        self.next_block += 1;
        count
    }
}
