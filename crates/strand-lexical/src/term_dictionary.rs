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
//! (`rfcs/0005-term-dictionary.md`) and extended by RFC 0009
//! (`rfcs/0009-per-term-overhead-reduction.md` Design §2): a second,
//! 16-byte term-info record shape (`blob_type_id = 4`) for fields that
//! never carry positions, alongside the original 28-byte record
//! (`blob_type_id = 1`), which is untouched.

/// Fixed byte length of one term-info record (`spec/term-dictionary.md` §3):
/// `doc_freq` (u32) + `postings_offset` (u64) + `postings_length` (u32) +
/// `positions_offset` (u64) + `positions_length` (u32) = 4 + 8 + 4 + 8 + 4.
pub const TERM_INFO_RECORD_LEN: usize = 28;

/// Fixed byte length of one short term-info record (RFC 0009 Design §2,
/// `blob_type_id = 4`): `doc_freq` (u32) + `postings_offset` (u64) +
/// `postings_length` (u32) = 4 + 8 + 4. No `positions_offset`/
/// `positions_length` — a field using this record shape MUST NOT also
/// register a positions blob.
pub const SHORT_TERM_INFO_RECORD_LEN: usize = 16;

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

    /// Encodes the short, positions-free record (RFC 0009 Design §2,
    /// `blob_type_id = 4`): `doc_freq` + `postings_offset` +
    /// `postings_length` only. `self.positions_offset`/`positions_length`
    /// are not written — a field adopting this shape never populates them.
    pub fn encode_short(&self) -> [u8; SHORT_TERM_INFO_RECORD_LEN] {
        let mut out = [0u8; SHORT_TERM_INFO_RECORD_LEN];
        out[0..4].copy_from_slice(&self.doc_freq.to_le_bytes());
        out[4..12].copy_from_slice(&self.postings_offset.to_le_bytes());
        out[12..16].copy_from_slice(&self.postings_length.to_le_bytes());
        out
    }

    /// Decodes the short record. `positions_offset`/`positions_length` on
    /// the returned `TermInfo` are always `0` — this record shape never
    /// carries them (`spec/positions.md` §1's "absent" convention applies
    /// uniformly to every term using this shape).
    pub fn decode_short(bytes: &[u8; SHORT_TERM_INFO_RECORD_LEN]) -> TermInfo {
        TermInfo {
            doc_freq: u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            postings_offset: u64::from_le_bytes(bytes[4..12].try_into().unwrap()),
            postings_length: u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            positions_offset: 0,
            positions_length: 0,
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
        let start = usize::try_from(ordinal)
            .ok()?
            .checked_mul(TERM_INFO_RECORD_LEN)?;
        let end = start.checked_add(TERM_INFO_RECORD_LEN)?;
        let record: &[u8; TERM_INFO_RECORD_LEN] = self.bytes.get(start..end)?.try_into().ok()?;
        Some(TermInfo::decode(record))
    }
}

/// A resident short term-info store blob (RFC 0009 Design §2): a flat array
/// of fixed 16-byte records, directly indexed by ordinal — identical
/// mechanics to `TermInfoStore`, over the short record shape.
#[derive(Debug, Clone, Copy)]
pub struct ShortTermInfoStore<'a> {
    bytes: &'a [u8],
}

impl<'a> ShortTermInfoStore<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, TermInfoStoreError> {
        if !bytes.len().is_multiple_of(SHORT_TERM_INFO_RECORD_LEN) {
            return Err(TermInfoStoreError::Truncated);
        }
        Ok(ShortTermInfoStore { bytes })
    }

    pub fn len(&self) -> usize {
        self.bytes.len() / SHORT_TERM_INFO_RECORD_LEN
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Direct-indexed read at `ordinal * SHORT_TERM_INFO_RECORD_LEN` — no
    /// scan. The returned `TermInfo`'s `positions_offset`/`positions_length`
    /// are always `0` (`TermInfo::decode_short`'s own contract).
    pub fn get(&self, ordinal: u64) -> Option<TermInfo> {
        let start = usize::try_from(ordinal)
            .ok()?
            .checked_mul(SHORT_TERM_INFO_RECORD_LEN)?;
        let end = start.checked_add(SHORT_TERM_INFO_RECORD_LEN)?;
        let record: &[u8; SHORT_TERM_INFO_RECORD_LEN] =
            self.bytes.get(start..end)?.try_into().ok()?;
        Some(TermInfo::decode_short(record))
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
    let fst_bytes = build_ordinal_fst(terms.iter().map(|(term, _)| *term))?;
    let mut term_info_bytes = Vec::with_capacity(terms.len() * TERM_INFO_RECORD_LEN);
    for (_, info) in terms {
        term_info_bytes.extend_from_slice(&info.encode());
    }
    Ok((fst_bytes, term_info_bytes))
}

/// Builds the term-dictionary FST blob and the *short* term-info store blob
/// (RFC 0009 Design §2, `blob_type_id = 4`) for a field that opts out of
/// positions entirely. `terms[i].1.positions_offset`/`positions_length` are
/// ignored (never written — `TermInfo::encode_short`'s own contract), so a
/// caller with a real positions blob for this field must use
/// `build_term_dictionary` (the 28-byte record) instead; mixing the two
/// shapes for one field is a caller bug this function does not detect
/// (`spec/container.md` §5's registry has no per-field identity yet to
/// check it against, RFC 0009's own Non-goals).
pub fn build_term_dictionary_short(
    terms: &[(&[u8], TermInfo)],
) -> Result<(Vec<u8>, Vec<u8>), TermDictionaryError> {
    let fst_bytes = build_ordinal_fst(terms.iter().map(|(term, _)| *term))?;
    let mut term_info_bytes = Vec::with_capacity(terms.len() * SHORT_TERM_INFO_RECORD_LEN);
    for (_, info) in terms {
        term_info_bytes.extend_from_slice(&info.encode_short());
    }
    Ok((fst_bytes, term_info_bytes))
}

/// Builds an `fst` crate `Map` blob from keys already in unsigned UTF-8 byte
/// order, each assigned its position as a dense `u64` ordinal. Shared by
/// `build_term_dictionary` (above) and `filter_bitmaps::build_value_dictionary`
/// — `spec/filter-bitmaps.md` §2 states the value dictionary is "identical in
/// shape" to the term dictionary's FST, so both build the same way.
pub(crate) fn build_ordinal_fst<'a>(
    keys: impl Iterator<Item = &'a [u8]>,
) -> Result<Vec<u8>, TermDictionaryError> {
    let mut builder = fst::MapBuilder::memory();
    for (ordinal, key) in keys.enumerate() {
        builder.insert(key, ordinal as u64)?;
    }
    Ok(builder.into_inner()?)
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

    /// Enumerates every `(term_bytes, ordinal)` pair in the dictionary, in
    /// the FST's own key order — unsigned byte order (invariant 11), the
    /// same order `build_term_dictionary` inserted them in. `get` alone only
    /// resolves one already-known term; a full-dictionary consumer (a
    /// `strand-tools inspect` listing, or `strand-datafusion`'s field-level
    /// table scan, `crates/strand-datafusion`) needs every term, not a point
    /// lookup. `fst::Map` exposes this via its own `Streamer` trait, one
    /// key-value pair at a time without materializing the whole dictionary;
    /// this wraps that as a plain `Iterator` so callers don't need `fst` as
    /// a direct dependency.
    pub fn iter(&self) -> impl Iterator<Item = (Vec<u8>, u64)> + '_ {
        use fst::Streamer;
        let mut stream = self.map.stream();
        std::iter::from_fn(move || stream.next().map(|(term, ordinal)| (term.to_vec(), ordinal)))
    }
}

#[cfg(test)]
mod term_dictionary_iter_tests {
    use super::*;

    #[test]
    fn iter_yields_every_term_in_byte_order_with_its_ordinal() {
        let infos = [
            TermInfo {
                doc_freq: 1,
                ..Default::default()
            },
            TermInfo {
                doc_freq: 2,
                ..Default::default()
            },
            TermInfo {
                doc_freq: 3,
                ..Default::default()
            },
        ];
        let terms: Vec<(&[u8], TermInfo)> =
            vec![(b"apple", infos[0]), (b"banana", infos[1]), (b"cherry", infos[2])];
        let (fst_bytes, _term_info) = build_term_dictionary(&terms).unwrap();
        let dict = TermDictionary::open(fst_bytes).unwrap();

        let collected: Vec<(Vec<u8>, u64)> = dict.iter().collect();
        assert_eq!(
            collected,
            vec![
                (b"apple".to_vec(), 0),
                (b"banana".to_vec(), 1),
                (b"cherry".to_vec(), 2),
            ]
        );
    }

    #[test]
    fn iter_on_an_empty_dictionary_yields_nothing() {
        let terms: Vec<(&[u8], TermInfo)> = vec![];
        let (fst_bytes, _term_info) = build_term_dictionary(&terms).unwrap();
        let dict = TermDictionary::open(fst_bytes).unwrap();
        assert_eq!(dict.iter().count(), 0);
    }
}
