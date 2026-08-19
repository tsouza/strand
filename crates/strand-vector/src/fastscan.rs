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

//! The FastScan intra-batch bit/lane shuffle for 1-bit RaBitQ codes
//! (`spec/vectors.md` §4), adopted verbatim from the reference
//! implementation's own `fastscan::pack_codes`
//! (`references/rabitq-library-fastscan-pack-codes-source.md`) — not
//! re-derived. `unpack_batch` is this crate's own inverse of that
//! algorithm (the reference source vendored so far is write-side only);
//! `round_trips` (below) proves it's a true inverse rather than trusting
//! the derivation by hand.

/// Vectors per FastScan batch (`fastscan::kBatchSize`, wire-format constant
/// per RFC 0010's "How this could be wrong").
pub const BATCH_SIZE: usize = 32;

/// The reference implementation's own fixed nibble-interleave permutation.
const K_PERM_0: [u8; 16] = [0, 8, 1, 9, 2, 10, 3, 11, 4, 12, 5, 13, 6, 14, 7, 15];

fn inverse_perm0() -> [u8; 16] {
    let mut inv = [0u8; 16];
    for (j, &p) in K_PERM_0.iter().enumerate() {
        inv[p as usize] = j as u8;
    }
    inv
}

/// Packs exactly one batch (`BATCH_SIZE` vector slots) of plain, sequential
/// one-bit-per-dimension compact codes into the FastScan-batched layout.
///
/// `compact_codes` MUST be exactly `BATCH_SIZE * cols` bytes, row-major
/// (`cols` bytes per slot); the caller zero-fills any slot beyond the
/// batch's real vector count (`spec/vectors.md` §4's zero-fill rule) before
/// calling this. Returns exactly `BATCH_SIZE * cols` bytes.
///
/// # Panics
///
/// Panics if `compact_codes.len() != BATCH_SIZE * cols`.
pub fn pack_batch(compact_codes: &[u8], cols: usize) -> Vec<u8> {
    assert_eq!(
        compact_codes.len(),
        BATCH_SIZE * cols,
        "compact_codes must be exactly BATCH_SIZE*cols bytes (one zero-padded batch)"
    );
    let mut out = vec![0u8; BATCH_SIZE * cols];
    for i in 0..cols {
        let mut col = [0u8; BATCH_SIZE];
        for (v, slot) in col.iter_mut().enumerate() {
            *slot = compact_codes[v * cols + i];
        }
        let hi: [u8; BATCH_SIZE] = std::array::from_fn(|v| col[v] >> 4);
        let lo: [u8; BATCH_SIZE] = std::array::from_fn(|v| col[v] & 0xF);
        for j in 0..16 {
            let p = K_PERM_0[j] as usize;
            out[i * BATCH_SIZE + j] = hi[p] | (hi[p + 16] << 4);
            out[i * BATCH_SIZE + 16 + j] = lo[p] | (lo[p + 16] << 4);
        }
    }
    out
}

/// The inverse of [`pack_batch`]: recovers one batch's plain, sequential
/// compact codes from their FastScan-packed bytes.
///
/// # Panics
///
/// Panics if `packed.len() != BATCH_SIZE * cols`.
pub fn unpack_batch(packed: &[u8], cols: usize) -> Vec<u8> {
    assert_eq!(
        packed.len(),
        BATCH_SIZE * cols,
        "packed must be exactly BATCH_SIZE*cols bytes"
    );
    let inv = inverse_perm0();
    let mut compact = vec![0u8; BATCH_SIZE * cols];
    for i in 0..cols {
        let block = &packed[i * BATCH_SIZE..i * BATCH_SIZE + BATCH_SIZE];
        let mut hi = [0u8; BATCH_SIZE];
        let mut lo = [0u8; BATCH_SIZE];
        for v in 0..16 {
            let j = inv[v] as usize;
            hi[v] = block[j] & 0xF;
            hi[v + 16] = block[j] >> 4;
            lo[v] = block[16 + j] & 0xF;
            lo[v + 16] = block[16 + j] >> 4;
        }
        for v in 0..BATCH_SIZE {
            compact[v * cols + i] = (hi[v] << 4) | lo[v];
        }
    }
    compact
}

/// Packs `num` real vectors' compact codes (`cols = padded_dims / 8` bytes
/// each, row-major, `num * cols` bytes total) into the full multi-batch
/// FastScan layout `spec/vectors.md` §4 registers: `ceil(num / BATCH_SIZE)`
/// batches, each `BATCH_SIZE * cols` bytes, the last batch zero-filled for
/// any slot beyond `num`.
///
/// # Panics
///
/// Panics if `compact_codes.len() != num * cols`.
pub fn pack_codes(compact_codes: &[u8], num: usize, cols: usize) -> Vec<u8> {
    assert_eq!(
        compact_codes.len(),
        num * cols,
        "compact_codes must be exactly num*cols bytes"
    );
    let num_batches = num.div_ceil(BATCH_SIZE);
    let mut out = Vec::with_capacity(num_batches * BATCH_SIZE * cols);
    for b in 0..num_batches {
        let start = b * BATCH_SIZE;
        let real = (num - start).min(BATCH_SIZE);
        let mut batch = vec![0u8; BATCH_SIZE * cols];
        batch[..real * cols]
            .copy_from_slice(&compact_codes[start * cols..start * cols + real * cols]);
        out.extend(pack_batch(&batch, cols));
    }
    out
}

/// The inverse of [`pack_codes`]: recovers `num` real vectors' compact
/// codes from the packed multi-batch bytes, dropping zero-fill padding.
///
/// # Panics
///
/// Panics if `packed.len()` isn't exactly `ceil(num / BATCH_SIZE) * BATCH_SIZE * cols`.
pub fn unpack_codes(packed: &[u8], num: usize, cols: usize) -> Vec<u8> {
    let num_batches = num.div_ceil(BATCH_SIZE);
    assert_eq!(
        packed.len(),
        num_batches * BATCH_SIZE * cols,
        "packed must be exactly num_batches*BATCH_SIZE*cols bytes"
    );
    let mut out = Vec::with_capacity(num * cols);
    for b in 0..num_batches {
        let batch = unpack_batch(
            &packed[b * BATCH_SIZE * cols..(b + 1) * BATCH_SIZE * cols],
            cols,
        );
        let start = b * BATCH_SIZE;
        let real = (num - start).min(BATCH_SIZE);
        out.extend_from_slice(&batch[..real * cols]);
    }
    out
}

/// Byte length of `pack_codes`'s output for `num` vectors at `cols` bytes
/// each — `spec/vectors.md` §4's code-region size, independent of content.
pub fn packed_code_len(num: usize, cols: usize) -> usize {
    num.div_ceil(BATCH_SIZE) * BATCH_SIZE * cols
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact micro-example from RFC 0010's Discussion — post-approval
    /// amendment (independently re-executed there via Python; reproduced
    /// here in Rust so the crate's own test suite pins it, not just the
    /// RFC's prose).
    #[test]
    fn matches_rfc_0010_discussion_micro_example() {
        let v0 = [0x12u8, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
        let v1 = [0x11u8, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        let mut compact = Vec::new();
        compact.extend_from_slice(&v0);
        compact.extend_from_slice(&v1);

        let packed = pack_codes(&compact, 2, 8);
        assert_eq!(packed.len(), 256, "= padded_dim * 4 at padded_dim = 64");

        let mut expected_first_half = [0u8; 16];
        expected_first_half[0] = 0x1;
        expected_first_half[1] = 0x0;
        expected_first_half[2] = 0x1;
        let mut expected_second_half = [0u8; 16];
        expected_second_half[0] = 0x2;
        expected_second_half[1] = 0x0;
        expected_second_half[2] = 0x1;

        assert_eq!(&packed[0..16], &expected_first_half[..]);
        assert_eq!(&packed[16..32], &expected_second_half[..]);
    }

    #[test]
    fn unpack_batch_inverts_pack_batch() {
        let cols = 8;
        let compact: Vec<u8> = (0..(BATCH_SIZE * cols))
            .map(|i| (i * 37 + 11) as u8)
            .collect();
        let packed = pack_batch(&compact, cols);
        let recovered = unpack_batch(&packed, cols);
        assert_eq!(recovered, compact);
    }

    #[test]
    fn unpack_codes_inverts_pack_codes_for_a_partial_final_batch() {
        let cols = 4;
        let num = 37; // not a multiple of BATCH_SIZE
        let compact: Vec<u8> = (0..(num * cols)).map(|i| (i * 13 + 5) as u8).collect();
        let packed = pack_codes(&compact, num, cols);
        assert_eq!(packed.len(), packed_code_len(num, cols));
        let recovered = unpack_codes(&packed, num, cols);
        assert_eq!(recovered, compact);
    }

    #[test]
    fn partial_batch_padding_is_zero_filled() {
        let cols = 2;
        let num = 3;
        let compact: Vec<u8> = vec![1, 2, 3, 4, 5, 6];
        let packed = pack_codes(&compact, num, cols);
        // Re-derive what a fully-populated batch (32 real vectors, the
        // last 29 all-zero) would pack to, and confirm it matches exactly
        // — proving the padding lanes really are zero, not undefined.
        let mut full_batch = vec![0u8; BATCH_SIZE * cols];
        full_batch[..num * cols].copy_from_slice(&compact);
        let expected = pack_batch(&full_batch, cols);
        assert_eq!(packed, expected);
    }
}
