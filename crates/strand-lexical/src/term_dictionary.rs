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

//! The term-dictionary FST and term-info store blobs for the lexical family.
//! Layout is normative per `spec/term-dictionary.md`, approved by RFC 0005
//! (`rfcs/0005-term-dictionary.md`).

/// Fixed byte length of one term-info record (`spec/term-dictionary.md` §3):
/// `doc_freq` (u32) + `postings_offset` (u64) + `postings_length` (u32) +
/// `positions_offset` (u64) + `positions_length` (u32) = 4 + 8 + 4 + 8 + 4.
pub const TERM_INFO_RECORD_LEN: usize = 28;

/// One term's scoring input and postings/positions location, per
/// `spec/term-dictionary.md` §3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TermInfo {
    pub doc_freq: u32,
    /// Byte offset within the (separate) postings blob, not the segment file.
    pub postings_offset: u64,
    pub postings_length: u32,
    /// Byte offset within the (separate) positions blob, not the segment file.
    pub positions_offset: u64,
    pub positions_length: u32,
}

impl TermInfo {
    pub fn encode(&self) -> [u8; TERM_INFO_RECORD_LEN] {
        let mut out = [0u8; TERM_INFO_RECORD_LEN];
        out[0..4].copy_from_slice(&self.doc_freq.to_le_bytes());
        out[4..12].copy_from_slice(&self.postings_offset.to_le_bytes());
        out[12..16].copy_from_slice(&self.postings_length.to_le_bytes());
        out[16..24].copy_from_slice(&self.positions_offset.to_le_bytes());
        out[24..28].copy_from_slice(&self.positions_length.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8; TERM_INFO_RECORD_LEN]) -> TermInfo {
        TermInfo {
            doc_freq: u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            postings_offset: u64::from_le_bytes(bytes[4..12].try_into().unwrap()),
            postings_length: u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            positions_offset: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            positions_length: u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
        }
    }
}

/// A resident term-info store blob: a flat array of fixed 28-byte records,
/// directly indexed by ordinal (`spec/term-dictionary.md` §3, §4).
#[derive(Debug, Clone, Copy)]
pub struct TermInfoStore<'a> {
    bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermInfoStoreError {
    /// The blob's length is not a multiple of `TERM_INFO_RECORD_LEN`.
    Truncated,
}

impl<'a> TermInfoStore<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, TermInfoStoreError> {
        if !bytes.len().is_multiple_of(TERM_INFO_RECORD_LEN) {
            return Err(TermInfoStoreError::Truncated);
        }
        Ok(TermInfoStore { bytes })
    }

    pub fn len(&self) -> usize {
        self.bytes.len() / TERM_INFO_RECORD_LEN
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Direct-indexed read at `ordinal * TERM_INFO_RECORD_LEN`
    /// (`spec/term-dictionary.md` §4) — no scan.
    pub fn get(&self, ordinal: u64) -> Option<TermInfo> {
        let start = usize::try_from(ordinal).ok()?.checked_mul(TERM_INFO_RECORD_LEN)?;
        let end = start.checked_add(TERM_INFO_RECORD_LEN)?;
        let record: &[u8; TERM_INFO_RECORD_LEN] = self.bytes.get(start..end)?.try_into().ok()?;
        Some(TermInfo::decode(record))
    }
}

#[derive(Debug)]
pub enum TermDictionaryError {
    Fst(fst::Error),
}

impl From<fst::Error> for TermDictionaryError {
    fn from(e: fst::Error) -> Self {
        TermDictionaryError::Fst(e)
    }
}

impl std::fmt::Display for TermDictionaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TermDictionaryError::Fst(e) => write!(f, "fst error: {e}"),
        }
    }
}

impl std::error::Error for TermDictionaryError {}

/// Builds the term-dictionary FST blob and the term-info store blob from
/// terms already in unsigned UTF-8 byte order (invariant 11), each paired
/// with its `TermInfo`. Ordinal `i` is `terms[i]`'s position — the same
/// ordinal indexes both returned blobs (`spec/term-dictionary.md` §2, §3).
///
/// Returns `TermDictionaryError::Fst` if `terms` is not strictly increasing
/// in byte order (`fst::MapBuilder::insert` rejects out-of-order or
/// duplicate keys).
pub fn build_term_dictionary(
    terms: &[(&[u8], TermInfo)],
) -> Result<(Vec<u8>, Vec<u8>), TermDictionaryError> {
    let mut builder = fst::MapBuilder::memory();
    let mut term_info_bytes = Vec::with_capacity(terms.len() * TERM_INFO_RECORD_LEN);
    for (ordinal, (term, info)) in terms.iter().enumerate() {
        builder.insert(term, ordinal as u64)?;
        term_info_bytes.extend_from_slice(&info.encode());
    }
    let fst_bytes = builder.into_inner()?;
    Ok((fst_bytes, term_info_bytes))
}

/// A resident term-dictionary FST blob (`spec/term-dictionary.md` §2).
pub struct TermDictionary<D: AsRef<[u8]>> {
    map: fst::Map<D>,
}

impl TermDictionary<Vec<u8>> {
    pub fn open(bytes: Vec<u8>) -> Result<Self, TermDictionaryError> {
        Ok(TermDictionary {
            map: fst::Map::new(bytes)?,
        })
    }
}

impl<D: AsRef<[u8]>> TermDictionary<D> {
    /// Looks up a term, returning its ordinal. A miss is a normal outcome
    /// (`spec/term-dictionary.md` §2), not an error.
    pub fn get(&self, term: &[u8]) -> Option<u64> {
        self.map.get(term)
    }
}
