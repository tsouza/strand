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

//! The value-dictionary FST and filter-bitmap store blobs for the filter
//! family. Layout is normative per `spec/filter-bitmaps.md`, approved by RFC
//! 0006 (`rfcs/0006-filter-bitmaps.md`).

use crate::term_dictionary::{TermDictionary, TermDictionaryError, build_ordinal_fst};
use roaring::RoaringBitmap;

/// Fixed byte length of one filter-bitmap directory record
/// (`spec/filter-bitmaps.md` §3): `bitmap_offset` (u64) + `bitmap_length`
/// (u32) = 8 + 4.
pub const DIRECTORY_RECORD_LEN: usize = 12;

/// The exact cardinality the standard 32-bit Roaring form can index — the
/// normative cap `spec/filter-bitmaps.md` §3 places on `row_id_count` for any
/// segment declaring this blob family.
pub const MAX_ROW_ID_COUNT: u64 = 1 << 32;

/// One value ordinal's bitmap location within the filter-bitmap store blob
/// (`spec/filter-bitmaps.md` §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectoryEntry {
    /// Byte offset within this blob (not the segment file).
    pub bitmap_offset: u64,
    pub bitmap_length: u32,
}

impl DirectoryEntry {
    pub fn encode(&self) -> [u8; DIRECTORY_RECORD_LEN] {
        let mut out = [0u8; DIRECTORY_RECORD_LEN];
        out[0..8].copy_from_slice(&self.bitmap_offset.to_le_bytes());
        out[8..12].copy_from_slice(&self.bitmap_length.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8; DIRECTORY_RECORD_LEN]) -> DirectoryEntry {
        DirectoryEntry {
            bitmap_offset: u64::from_le_bytes(bytes[0..8].try_into().unwrap()),
            bitmap_length: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterBitmapError {
    /// A local ordinal (row_id_count) exceeded `MAX_ROW_ID_COUNT`
    /// (`spec/filter-bitmaps.md` §3).
    RowIdCountExceedsRoaringCapacity,
    Fst,
}

impl From<TermDictionaryError> for FilterBitmapError {
    fn from(_: TermDictionaryError) -> Self {
        FilterBitmapError::Fst
    }
}

/// Builds the value-dictionary FST blob from distinct values already in
/// unsigned UTF-8 byte order (`spec/filter-bitmaps.md` §2) — identical in
/// shape to the term dictionary's own FST (`spec/term-dictionary.md` §2).
pub fn build_value_dictionary(values: &[&[u8]]) -> Result<Vec<u8>, FilterBitmapError> {
    Ok(build_ordinal_fst(values.iter().copied())?)
}

pub type ValueDictionary<D> = TermDictionary<D>;

/// Builds the filter-bitmap store blob: a fixed-size directory (`ordinal *
/// DIRECTORY_RECORD_LEN`, `spec/filter-bitmaps.md` §3) followed by the
/// concatenated Roaring bitmaps, one per value ordinal in `bitmaps`.
///
/// Every bitmap in `bitmaps` MUST index only local ordinals below
/// `row_id_count`; `row_id_count` itself MUST NOT exceed `MAX_ROW_ID_COUNT`
/// (`spec/filter-bitmaps.md` §3) — checked here, not merely assumed. Each
/// bitmap is normalized with `remove_run_compression` before serialization,
/// enforcing this chapter's MUST to never emit run containers: two
/// logically-identical bitmaps built through different insertion APIs (e.g.
/// `insert` one at a time vs. a contiguous-range insert) would otherwise
/// serialize to different bytes on the same crate version and platform
/// (`references/roaring-format-spec-and-rust-crate.md`, RFC 0006 Design §3).
pub fn build_filter_bitmap_store(
    bitmaps: &[RoaringBitmap],
    row_id_count: u64,
) -> Result<Vec<u8>, FilterBitmapError> {
    if row_id_count > MAX_ROW_ID_COUNT {
        return Err(FilterBitmapError::RowIdCountExceedsRoaringCapacity);
    }

    let directory_len = bitmaps.len() * DIRECTORY_RECORD_LEN;
    let mut bitmap_bytes = Vec::new();
    let mut entries = Vec::with_capacity(bitmaps.len());
    for bitmap in bitmaps {
        let mut normalized = bitmap.clone();
        normalized.remove_run_compression();

        let start = bitmap_bytes.len();
        normalized
            .serialize_into(&mut bitmap_bytes)
            .expect("serializing into a Vec<u8> is infallible");
        let length = bitmap_bytes.len() - start;
        entries.push(DirectoryEntry {
            bitmap_offset: (directory_len + start) as u64,
            bitmap_length: length as u32,
        });
    }

    let mut out = Vec::with_capacity(directory_len + bitmap_bytes.len());
    for entry in &entries {
        out.extend_from_slice(&entry.encode());
    }
    out.extend_from_slice(&bitmap_bytes);
    Ok(out)
}

/// A resident filter-bitmap store blob (`spec/filter-bitmaps.md` §3, §4).
#[derive(Debug, Clone, Copy)]
pub struct FilterBitmapStore<'a> {
    bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterBitmapStoreError {
    OutOfRange,
}

impl<'a> FilterBitmapStore<'a> {
    pub fn new(bytes: &'a [u8]) -> FilterBitmapStore<'a> {
        FilterBitmapStore { bytes }
    }

    /// Direct-indexed read at `ordinal * DIRECTORY_RECORD_LEN`
    /// (`spec/filter-bitmaps.md` §4) — no scan.
    pub fn directory_entry(&self, ordinal: u64) -> Result<DirectoryEntry, FilterBitmapStoreError> {
        let start = usize::try_from(ordinal)
            .ok()
            .and_then(|o| o.checked_mul(DIRECTORY_RECORD_LEN))
            .ok_or(FilterBitmapStoreError::OutOfRange)?;
        let end = start
            .checked_add(DIRECTORY_RECORD_LEN)
            .ok_or(FilterBitmapStoreError::OutOfRange)?;
        let record: &[u8; DIRECTORY_RECORD_LEN] = self
            .bytes
            .get(start..end)
            .ok_or(FilterBitmapStoreError::OutOfRange)?
            .try_into()
            .unwrap();
        Ok(DirectoryEntry::decode(record))
    }

    /// The value's raw, standard 32-bit Roaring bitmap bytes, already
    /// resident — no further fetch (`spec/filter-bitmaps.md` §4).
    pub fn bitmap_bytes(&self, ordinal: u64) -> Result<&'a [u8], FilterBitmapStoreError> {
        let entry = self.directory_entry(ordinal)?;
        let start =
            usize::try_from(entry.bitmap_offset).map_err(|_| FilterBitmapStoreError::OutOfRange)?;
        let end = start
            .checked_add(entry.bitmap_length as usize)
            .ok_or(FilterBitmapStoreError::OutOfRange)?;
        self.bytes
            .get(start..end)
            .ok_or(FilterBitmapStoreError::OutOfRange)
    }

    /// Deserializes the value's bitmap for membership/set-operation queries
    /// (`spec/filter-bitmaps.md` §4).
    pub fn bitmap(&self, ordinal: u64) -> Result<RoaringBitmap, FilterBitmapStoreError> {
        let bytes = self.bitmap_bytes(ordinal)?;
        RoaringBitmap::deserialize_from(bytes).map_err(|_| FilterBitmapStoreError::OutOfRange)
    }
}
