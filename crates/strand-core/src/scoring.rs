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

//! The `bm25` and `lucene-parity` scoring profiles, normative per
//! `spec/scoring-profiles.md`, approved by RFC 0003
//! (`rfcs/0003-scoring-profiles.md`).
//!
//! `bm25` is the canonical Robertson & Zaragoza form (no `+1` inside the idf
//! logarithm, no `(k1+1)` numerator factor —
//! `references/robertson-zaragoza-bm25-and-beyond.md` eq. 3.15).
//! `lucene-parity` reproduces Apache Lucene 10.5.1's `BM25Similarity`
//! exactly, including its idf `+1` and its one-byte `SmallFloat.intToByte4`/
//! `byte4ToInt` document-length quantization
//! (`references/lucene-bm25similarity-and-smallfloat.md`) — ported directly
//! from that vendored source, not from memory (`CLAUDE.md` §3).

/// Default `k1`/`b`, shared by both profiles (Lucene's `BM25Similarity()`
/// no-arg constructor, `references/lucene-bm25similarity-and-smallfloat.md`).
pub const DEFAULT_K1: f64 = 1.2;
pub const DEFAULT_B: f64 = 0.75;

/// The canonical BM25 profile (`spec/scoring-profiles.md`'s `bm25`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bm25Profile {
    pub k1: f64,
    pub b: f64,
}

impl Default for Bm25Profile {
    fn default() -> Self {
        Bm25Profile { k1: DEFAULT_K1, b: DEFAULT_B }
    }
}

impl Bm25Profile {
    /// Robertson & Zaragoza's canonical idf: `ln((N - n + 0.5) / (n + 0.5))`,
    /// no `+1`. Invariant 5: a writer MUST NOT clip a negative result (a term
    /// occurring in more than half the collection) — returned as computed.
    pub fn idf(&self, doc_freq: u64, doc_count: u64) -> f64 {
        ((doc_count as f64 - doc_freq as f64 + 0.5) / (doc_freq as f64 + 0.5)).ln()
    }

    /// `idf * tf / (tf + k1 * ((1 - b) + b * dl / avdl))`, using `dl` exactly
    /// as stored (no quantization).
    pub fn score(&self, doc_freq: u64, doc_count: u64, tf: f64, dl: f64, avdl: f64) -> f64 {
        let idf = self.idf(doc_freq, doc_count);
        let norm = self.k1 * ((1.0 - self.b) + self.b * dl / avdl);
        idf * tf / (tf + norm)
    }
}

/// The Lucene-parity profile (`spec/scoring-profiles.md`'s `lucene-parity`),
/// byte-exact against `BM25Similarity` 10.5.1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LuceneParityProfile {
    pub k1: f64,
    pub b: f64,
}

impl Default for LuceneParityProfile {
    fn default() -> Self {
        LuceneParityProfile { k1: DEFAULT_K1, b: DEFAULT_B }
    }
}

impl LuceneParityProfile {
    /// Lucene's idf: `ln(1 + (N - n + 0.5) / (n + 0.5))` — the `+1` invariant
    /// 5 requires this profile to reproduce, not average away.
    pub fn idf(&self, doc_freq: u64, doc_count: u64) -> f64 {
        (1.0 + (doc_count as f64 - doc_freq as f64 + 0.5) / (doc_freq as f64 + 0.5)).ln()
    }

    /// Same shape as `Bm25Profile::score`, but `dl` is first round-tripped
    /// through `intToByte4`/`byte4ToInt` (Lucene's one-byte norm), per
    /// invariant 5's "parity within Lucene's one-byte norm quantization."
    /// `dl_tokens` MUST already be Lucene's `discountOverlaps`-adjusted
    /// length (`computeNorm`,
    /// `references/lucene-bm25similarity-and-smallfloat.md`), not a raw
    /// token count, whenever the field carries overlaps.
    pub fn score(&self, doc_freq: u64, doc_count: u64, tf: f64, dl_tokens: u32, avdl: f64) -> f64 {
        let quantized_dl = byte4_to_int(int_to_byte4(dl_tokens)) as f64;
        let idf = self.idf(doc_freq, doc_count);
        let norm = self.k1 * ((1.0 - self.b) + self.b * quantized_dl / avdl);
        idf * tf / (tf + norm)
    }
}

/// Float-like encoding for positive `u32`s that preserves ordering and 4
/// significant bits. Ported directly from `SmallFloat.longToInt4`
/// (`references/lucene-bm25similarity-and-smallfloat.md`).
const fn long_to_int4(i: i64) -> i32 {
    let num_bits = 64 - i.leading_zeros() as i32;
    if num_bits < 4 {
        // subnormal value
        i as i32
    } else {
        // normal value
        let shift = num_bits - 4;
        // only keep the 4 most significant bits (after clearing the implicit leading one)
        let mut encoded = (i >> shift) as i32;
        encoded &= 0x07;
        // encode the shift, adding 1 because 0 is reserved for subnormal values
        encoded |= (shift + 1) << 3;
        encoded
    }
}

/// Decodes values encoded with `long_to_int4`. Ported from
/// `SmallFloat.int4ToLong`.
const fn int4_to_long(i: i32) -> i64 {
    let bits = (i & 0x07) as i64;
    let shift = (i >> 3) - 1;
    if shift == -1 {
        // subnormal value
        bits
    } else {
        // normal value
        (bits | 0x08) << shift
    }
}

const MAX_INT4: i32 = long_to_int4(i32::MAX as i64);
/// `255 - MAX_INT4` — computed, not hand-derived, matching
/// `references/lucene-bm25similarity-and-smallfloat.md`'s pinned value.
const NUM_FREE_VALUES: i32 = 255 - MAX_INT4;

/// Encodes a document length to Lucene's one-byte norm. Ported from
/// `SmallFloat.intToByte4`: document lengths `0..NUM_FREE_VALUES` (`0..24`)
/// encode exactly; longer documents fall into the lossy 4-significant-bit
/// encoding.
pub const fn int_to_byte4(i: u32) -> u8 {
    let i = i as i32;
    if i < NUM_FREE_VALUES {
        i as u8
    } else {
        (NUM_FREE_VALUES + long_to_int4((i - NUM_FREE_VALUES) as i64)) as u8
    }
}

/// Decodes a byte produced by `int_to_byte4`. Ported from
/// `SmallFloat.byte4ToInt`.
pub const fn byte4_to_int(b: u8) -> u32 {
    let i = b as i32;
    if i < NUM_FREE_VALUES {
        i as u32
    } else {
        (NUM_FREE_VALUES as i64 + int4_to_long(i - NUM_FREE_VALUES)) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn num_free_values_matches_the_vendored_reference() {
        // references/lucene-bm25similarity-and-smallfloat.md: "longToInt4(2147483647) = 231,
        // so NUM_FREE_VALUES = 24".
        assert_eq!(MAX_INT4, 231);
        assert_eq!(NUM_FREE_VALUES, 24);
    }

    #[test]
    fn lengths_zero_through_23_encode_exactly() {
        for dl in 0..24u32 {
            assert_eq!(byte4_to_int(int_to_byte4(dl)), dl, "dl={dl}");
        }
    }

    #[test]
    fn the_rfc_0003_worked_examples_first_lossy_length_is_41() {
        // rfcs/0003-scoring-profiles.md's worked example: intToByte4(41) = 40, byte4ToInt(40) = 40.
        assert_eq!(int_to_byte4(41), 40);
        assert_eq!(byte4_to_int(40), 40);
        // And it is in fact the first lossy value (dl=24..40 all round-trip exactly).
        for dl in 24..41u32 {
            assert_eq!(byte4_to_int(int_to_byte4(dl)), dl, "dl={dl}");
        }
    }

    fn assert_close(actual: f64, expected: f64, label: &str) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "{label}: expected {expected}, got {actual}"
        );
    }

    /// Reproduces rfcs/0003-scoring-profiles.md's worked example exactly:
    /// N=4, n=1, dl=41, tf=3, avdl=40, k1=1.2, b=0.75.
    #[test]
    fn worked_example_bm25_profile() {
        let profile = Bm25Profile::default();
        assert_close(profile.idf(1, 4), 0.847298, "idf");
        let score = profile.score(1, 4, 3.0, 41.0, 40.0);
        assert_close(score, 0.601988, "score");
    }

    #[test]
    fn worked_example_lucene_parity_profile() {
        let profile = LuceneParityProfile::default();
        assert_close(profile.idf(1, 4), 1.203973, "idf_lucene");
        let score = profile.score(1, 4, 3.0, 41, 40.0);
        assert_close(score, 0.859981, "score_lucene");
    }

    #[test]
    fn the_two_profiles_diverge_by_the_rfcs_stated_margin() {
        let bm25 = Bm25Profile::default().score(1, 4, 3.0, 41.0, 40.0);
        let lucene = LuceneParityProfile::default().score(1, 4, 3.0, 41, 40.0);
        // RFC 0003 states "≈ 1.4286" (4 significant digits); the underlying
        // scores are checked to 1e-6 by the two tests above, so this only
        // confirms the ratio at the RFC's own stated precision.
        assert!(
            (lucene / bm25 - 1.4286).abs() < 1e-4,
            "ratio: expected ~1.4286, got {}",
            lucene / bm25
        );
    }
}
