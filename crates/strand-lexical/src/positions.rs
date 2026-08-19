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

//! The positions blob for the lexical family: per-term, within-document
//! token-position delta-gaps, enabling phrase queries. Layout is normative
//! per `spec/positions.md`, approved by RFC 0008
//! (`rfcs/0008-positions.md`) and amended by RFC 0009
//! (`rfcs/0009-per-term-overhead-reduction.md` Design §1: the
//! `postings_block_pos_prefix` region omits its always-`0` index-`0` entry
//! — a breaking, in-place change to this blob's layout, not an additive
//! one). Reuses `postings.rs`'s block-codec building blocks
//! (`scalar_pack`/`scalar_unpack`/`block_count_for`/`block_real_len`)
//! directly, per RFC 0008's own stated plan, rather than duplicating them.

use bitpacking::{BitPacker, BitPacker8x};

use crate::postings::{block_count_for, block_real_len, scalar_bits_needed, scalar_pack, scalar_unpack, BLOCK_LEN};

/// Builds a positions blob (`spec/positions.md` §4) from a term's postings,
/// in postings order: `doc_positions[i]` is the `i`-th posting's
/// within-document token positions (0-based, strictly increasing — token
/// indices into that document's own token stream, `spec/positions.md` §2).
/// `doc_positions.len()` is `doc_freq` (`TermInfo.doc_freq`, external,
/// never stored here — identical convention to `postings::build_postings`).
///
/// # Panics
///
/// Panics if `doc_positions` is empty, if any entry is empty, or if any
/// entry is not strictly increasing.
pub fn build_positions(doc_positions: &[Vec<u32>]) -> Vec<u8> {
    assert!(!doc_positions.is_empty(), "a positions list must cover at least one posting");
    for positions in doc_positions {
        assert!(!positions.is_empty(), "every posting has at least one occurrence");
        assert!(
            positions.windows(2).all(|w| w[0] < w[1]),
            "within-document positions must be strictly increasing"
        );
    }

    let doc_freq = doc_positions.len();
    let postings_block_count = block_count_for(doc_freq);

    // RFC 0009 Design §1: postings_block_pos_prefix[0] is always 0 (nothing
    // precedes the first postings block) and is never stored — only
    // entries for blocks 1..postings_block_count are kept.
    let mut postings_block_pos_prefix = Vec::with_capacity(postings_block_count.saturating_sub(1));
    let mut running: u32 = 0;
    for b in 0..postings_block_count {
        if b > 0 {
            postings_block_pos_prefix.push(running);
        }
        let start = b * BLOCK_LEN;
        let end = start + block_real_len(doc_freq, b);
        for positions in &doc_positions[start..end] {
            running += positions.len() as u32;
        }
    }
    let total_term_freq = running;

    // §2: delta-from-zero within each document, concatenated with no
    // separator.
    let mut deltas = Vec::with_capacity(total_term_freq as usize);
    for positions in doc_positions {
        let mut prev = 0u32;
        for &p in positions {
            deltas.push(p - prev);
            prev = p;
        }
    }

    let position_block_count = block_count_for(total_term_freq as usize);
    let full_blocks = total_term_freq as usize / BLOCK_LEN;
    let bp = BitPacker8x::new();

    let mut pos_widths = Vec::with_capacity(position_block_count);
    let mut stream = Vec::new();
    for b in 0..position_block_count {
        let start = b * BLOCK_LEN;
        let real_len = block_real_len(total_term_freq as usize, b);
        let end = start + real_len;
        let block_deltas = &deltas[start..end];

        if b < full_blocks {
            let width = bp.num_bits(block_deltas);
            pos_widths.push(width);
            let mut buf = vec![0u8; BLOCK_LEN * 4];
            let len = bp.compress(block_deltas, &mut buf, width);
            stream.extend_from_slice(&buf[..len]);
        } else {
            let width = scalar_bits_needed(block_deltas);
            pos_widths.push(width);
            stream.extend_from_slice(&scalar_pack(block_deltas, width));
        }
    }

    let mut out = Vec::with_capacity(
        4 + 4 * postings_block_pos_prefix.len() + position_block_count + stream.len(),
    );
    out.extend_from_slice(&total_term_freq.to_le_bytes());
    for &prefix in &postings_block_pos_prefix {
        out.extend_from_slice(&prefix.to_le_bytes());
    }
    out.extend_from_slice(&pos_widths);
    out.extend_from_slice(&stream);
    out
}

/// A resident positions blob (`spec/positions.md` §4–§6). `doc_freq` is
/// supplied externally (from `TermInfo`, identical convention to
/// `postings::PostingsReader`) — `postings_block_count` is computed from
/// it, not read; `total_term_freq` (and so `position_block_count`) is read
/// from this blob's own leading field (`spec/positions.md` §4).
#[derive(Debug, Clone, Copy)]
pub struct PositionsReader<'a> {
    bytes: &'a [u8],
    total_term_freq: usize,
    postings_block_count: usize,
    position_block_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionsError {
    Truncated,
}

impl<'a> PositionsReader<'a> {
    pub fn new(bytes: &'a [u8], doc_freq: usize) -> Result<Self, PositionsError> {
        if bytes.len() < 4 {
            return Err(PositionsError::Truncated);
        }
        let total_term_freq = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let postings_block_count = block_count_for(doc_freq);
        let position_block_count = block_count_for(total_term_freq);
        // RFC 0009 Design §1: postings_block_pos_prefix stores only
        // postings_block_count - 1 entries (index 0 is never stored).
        let min_len = 4 + 4 * postings_block_count.saturating_sub(1) + position_block_count;
        if bytes.len() < min_len {
            return Err(PositionsError::Truncated);
        }
        Ok(PositionsReader { bytes, total_term_freq, postings_block_count, position_block_count })
    }

    fn postings_block_pos_prefix_region(&self) -> &'a [u8] {
        let len = 4 * self.postings_block_count.saturating_sub(1);
        &self.bytes[4..4 + len]
    }

    /// The total count of positions preceding postings block `i`'s first
    /// document (`spec/positions.md` §5) — an `O(1)` indexed read, no
    /// decode. Index `0` is always `0` by definition (RFC 0009 Design §1)
    /// and is never stored; every other index reads from the region.
    pub fn postings_block_pos_prefix(&self, block_idx: usize) -> u32 {
        if block_idx == 0 {
            return 0;
        }
        let region = self.postings_block_pos_prefix_region();
        let start = (block_idx - 1) * 4;
        u32::from_le_bytes(region[start..start + 4].try_into().unwrap())
    }

    fn pos_widths_region(&self) -> &'a [u8] {
        let start = 4 + 4 * self.postings_block_count.saturating_sub(1);
        &self.bytes[start..start + self.position_block_count]
    }

    fn stream(&self) -> &'a [u8] {
        &self.bytes[4 + 4 * self.postings_block_count.saturating_sub(1) + self.position_block_count..]
    }

    fn packed_len(&self, block_idx: usize, width: u8) -> usize {
        let real_len = block_real_len(self.total_term_freq, block_idx);
        (real_len * width as usize).div_ceil(8)
    }

    fn stream_offset(&self, widths: &[u8], block_idx: usize) -> usize {
        (0..block_idx).map(|b| self.packed_len(b, widths[b])).sum()
    }

    /// Decodes exactly the `i`-th position block's deltas — touches only
    /// that block's compressed bytes.
    fn decode_position_block(&self, block_idx: usize) -> Vec<u32> {
        let full_blocks = self.total_term_freq / BLOCK_LEN;
        let widths = self.pos_widths_region();
        let width = widths[block_idx];
        let real_len = block_real_len(self.total_term_freq, block_idx);

        let off = self.stream_offset(widths, block_idx);
        let len = self.packed_len(block_idx, width);
        let bytes = &self.stream()[off..off + len];

        if block_idx < full_blocks {
            let bp = BitPacker8x::new();
            let mut deltas = vec![0u32; BLOCK_LEN];
            bp.decompress(bytes, &mut deltas, width);
            deltas
        } else {
            scalar_unpack(bytes, real_len, width)
        }
    }

    /// Targeted lookup (`spec/positions.md` §6): given the postings block
    /// index `lo` a `spec/postings.md` §6 skip query already resolved, the
    /// sum of `tf` for documents in block `lo` strictly before the target
    /// (already known from that same skip's block decode), and the target
    /// document's own `tf`, returns its absolute within-document positions.
    pub fn positions_for_doc(&self, postings_block_idx: usize, local_prefix_tf: u32, tf: u32) -> Vec<u32> {
        let start_index = self.postings_block_pos_prefix(postings_block_idx) as usize + local_prefix_tf as usize;
        let end_index = start_index + tf as usize;
        let start_block = start_index / BLOCK_LEN;
        let end_block = (end_index - 1) / BLOCK_LEN;

        let mut window = Vec::with_capacity((end_block - start_block + 1) * BLOCK_LEN);
        for b in start_block..=end_block {
            window.extend(self.decode_position_block(b));
        }

        let slice_start = start_index - start_block * BLOCK_LEN;
        let deltas = &window[slice_start..slice_start + tf as usize];

        let mut positions = Vec::with_capacity(tf as usize);
        let mut prev = 0u32;
        for &d in deltas {
            prev += d;
            positions.push(prev);
        }
        positions
    }

    /// Full decode (`spec/positions.md` §6): every document's absolute
    /// within-document positions, in postings order. `term_freqs` is the
    /// same term's postings term-frequency array (`PostingsReader::
    /// decode_all`'s second element) — this blob never stores document
    /// boundaries itself, per §2.
    pub fn decode_all(&self, term_freqs: &[u32]) -> Vec<Vec<u32>> {
        let mut all_deltas = Vec::with_capacity(self.total_term_freq);
        for b in 0..self.position_block_count {
            all_deltas.extend(self.decode_position_block(b));
        }

        let mut result = Vec::with_capacity(term_freqs.len());
        let mut idx = 0usize;
        for &tf in term_freqs {
            let mut prev = 0u32;
            let mut positions = Vec::with_capacity(tf as usize);
            for &d in &all_deltas[idx..idx + tf as usize] {
                prev += d;
                positions.push(prev);
            }
            result.push(positions);
            idx += tf as usize;
        }
        result
    }
}
