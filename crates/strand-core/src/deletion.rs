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

//! The deletion-vector object: invariant 2's general "no in-place
//! mutation, deletes are deletion-vector blobs" machinery (`CLAUDE.md`
//! §5). Layout is normative per `spec/deletion.md`, approved by RFC 0012
//! (`rfcs/0012-deletion-vectors.md`).
//!
//! Not a segment, not a container — no footer, no hotcache, no blob
//! registry. The object's entire byte content is a standard 32-bit
//! Roaring bitmap (`references/roaring-format-spec-and-rust-crate.md`),
//! under `SERIAL_COOKIE_NO_RUNCONTAINER`, indexing the owning segment's
//! **local ordinals** (never the global row-ID space directly) — the
//! identical convention `crates/strand-lexical/src/filter_bitmaps.rs`
//! already established for filter bitmaps (`spec/filter-bitmaps.md` §3),
//! reused here rather than reinvented (`spec/deletion.md` §2).

use crate::store::{ConditionalStore, StoreError};
pub use roaring::RoaringBitmap;
use serde::{Deserialize, Serialize};

/// The exact cardinality the standard 32-bit Roaring form can index — the
/// same normative cap `spec/filter-bitmaps.md` §3 places on its own blob
/// family, restated here for the deletion-vector object (`spec/
/// deletion.md` §2).
pub const MAX_ROW_ID_COUNT: u64 = 1 << 32;

/// A segment's reference to its deletion-vector object (`spec/manifest.md`
/// `SegmentRef.deletion_vector`, `spec/deletion.md` §3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeletionVectorRef {
    pub path: String,
    pub byte_length: u64,
    pub checksum: u64,
}

/// xxHash3-64 over `bytes` (invariant 11's default) — the checksum
/// [`read`] verifies a fetched object against. Exposed so a caller
/// building a [`DeletionVectorRef`] (after writing the object bytes
/// [`build_deletion_vector`] produced) doesn't need its own `twox-hash`
/// dependency just to compute this one field.
pub fn checksum(bytes: &[u8]) -> u64 {
    twox_hash::XxHash3_64::oneshot(bytes)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeletionError {
    /// `row_id_count` exceeded [`MAX_ROW_ID_COUNT`] (`spec/deletion.md` §2).
    RowIdCountExceedsRoaringCapacity,
    /// The object's fetched bytes did not decode as a valid Roaring bitmap.
    Malformed,
    /// The fetched bytes' computed checksum did not match the
    /// `DeletionVectorRef.checksum` that named them — a real, distinct
    /// outcome from `Malformed`: the bytes decoded fine but are not the
    /// bytes the reference actually points to (`spec/deletion.md` §5).
    ChecksumMismatch { declared: u64, computed: u64 },
    /// The referenced object was not found in the store.
    NotFound,
    /// The backend failed for a reason other than a missing object.
    Io(String),
}

/// Builds a deletion-vector object's bytes from `bitmap` (indexing local
/// ordinals, `spec/row-ids.md` §1) and the owning segment's
/// `row_id_count`. Always serializes under `SERIAL_COOKIE_NO_RUNCONTAINER`
/// (`spec/deletion.md` §2's MUST rule) by normalizing with
/// `remove_run_compression` first, regardless of which container
/// representation `bitmap`'s own insertion history happens to have
/// produced — the identical discipline `filter_bitmaps::
/// build_filter_bitmap_store` already uses, for the identical reason
/// (`references/roaring-format-spec-and-rust-crate.md`).
///
/// # Errors
///
/// Returns [`DeletionError::RowIdCountExceedsRoaringCapacity`] if
/// `row_id_count > MAX_ROW_ID_COUNT`.
pub fn build_deletion_vector(
    bitmap: &RoaringBitmap,
    row_id_count: u64,
) -> Result<Vec<u8>, DeletionError> {
    if row_id_count > MAX_ROW_ID_COUNT {
        return Err(DeletionError::RowIdCountExceedsRoaringCapacity);
    }
    let mut normalized = bitmap.clone();
    normalized.remove_run_compression();
    let mut bytes = Vec::new();
    normalized
        .serialize_into(&mut bytes)
        .expect("serializing into a Vec<u8> is infallible");
    Ok(bytes)
}

/// A resident deletion vector: which of one segment's local ordinals are
/// tombstoned.
#[derive(Debug, Clone)]
pub struct DeletionVector {
    bitmap: RoaringBitmap,
}

impl DeletionVector {
    /// Decodes a deletion-vector object's raw bytes (already fetched and,
    /// for a real store-backed read, checksum-verified — [`read`] does
    /// both). Accepts either `SERIAL_COOKIE_NO_RUNCONTAINER` or
    /// `SERIAL_COOKIE` (run-container) input on read, per `spec/
    /// deletion.md` §2 — only the writer side is constrained to the
    /// no-run-container form.
    pub fn decode(bytes: &[u8]) -> Result<DeletionVector, DeletionError> {
        let bitmap =
            RoaringBitmap::deserialize_from(bytes).map_err(|_| DeletionError::Malformed)?;
        Ok(DeletionVector { bitmap })
    }

    /// The decoded local-ordinal bitmap itself — for a writer that needs
    /// to union in a new tombstone against the current set (`spec/
    /// deletion.md` §4, "Superseding, not accumulating").
    pub fn clone_bitmap(&self) -> RoaringBitmap {
        self.bitmap.clone()
    }

    /// Whether `row_id` (from a segment whose hotcache declares
    /// `row_id_base`) is tombstoned — translates to a local ordinal
    /// internally (`spec/deletion.md` §2).
    ///
    /// # Panics
    ///
    /// Panics if `row_id < row_id_base`, or if the resulting local
    /// ordinal does not fit in a `u32` — both mean `row_id` does not
    /// belong to this segment at all, a caller bug, not a normal outcome.
    pub fn is_deleted(&self, row_id: u64, row_id_base: u64) -> bool {
        let local_ordinal = row_id
            .checked_sub(row_id_base)
            .expect("row_id must be >= row_id_base");
        let local_ordinal =
            u32::try_from(local_ordinal).expect("local ordinal must fit in u32 (MAX_ROW_ID_COUNT)");
        self.bitmap.contains(local_ordinal)
    }
}

/// Fetches and decodes a segment's deletion vector, given its
/// [`DeletionVectorRef`] (`SegmentRef.deletion_vector`). Verifies the
/// fetched bytes' checksum against `deletion_vector_ref.checksum` before
/// decoding — the first checksum verification actually implemented in
/// this codebase (`SegmentRef.checksum` is computed at write time,
/// `crates/strand-core/src/segment.rs`, but nothing reads it back yet;
/// `spec/deletion.md` §5 states this precisely rather than implying an
/// existing precedent).
///
/// # Errors
///
/// [`DeletionError::NotFound`] if the object is missing;
/// [`DeletionError::ChecksumMismatch`] if the fetched bytes don't match
/// `deletion_vector_ref.checksum`; [`DeletionError::Malformed`] if they
/// pass the checksum but don't decode as a Roaring bitmap;
/// [`DeletionError::Io`] for any other backend failure.
pub fn read<S: ConditionalStore>(
    store: &S,
    deletion_vector_ref: &DeletionVectorRef,
) -> Result<DeletionVector, DeletionError> {
    let bytes = match store.get(&deletion_vector_ref.path) {
        Ok(Some((bytes, _etag))) => bytes,
        Ok(None) => return Err(DeletionError::NotFound),
        Err(StoreError::Io(msg) | StoreError::Ambiguous(msg)) => {
            return Err(DeletionError::Io(msg));
        }
        Err(StoreError::PreconditionFailed) => {
            unreachable!("ConditionalStore::get never returns PreconditionFailed")
        }
    };
    let computed = checksum(&bytes);
    if computed != deletion_vector_ref.checksum {
        return Err(DeletionError::ChecksumMismatch {
            declared: deletion_vector_ref.checksum,
            computed,
        });
    }
    DeletionVector::decode(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::InMemoryStore;

    #[test]
    fn round_trips_a_real_bitmap_through_build_and_decode() {
        let mut bitmap = RoaringBitmap::new();
        bitmap.insert(2);
        bitmap.insert(5);
        bitmap.insert(100);
        let bytes = build_deletion_vector(&bitmap, 200).unwrap();

        // Real, executed bytes — matches RFC 0012's own worked example.
        assert_eq!(bytes.len(), 22);
        assert_eq!(
            bytes,
            vec![
                0x3a, 0x30, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x10, 0x00,
                0x00, 0x00, 0x02, 0x00, 0x05, 0x00, 0x64, 0x00,
            ]
        );

        let decoded = DeletionVector::decode(&bytes).unwrap();
        assert!(decoded.is_deleted(1002, 1000)); // local ordinal 2
        assert!(decoded.is_deleted(1005, 1000)); // local ordinal 5
        assert!(decoded.is_deleted(1100, 1000)); // local ordinal 100
        assert!(!decoded.is_deleted(1000, 1000));
        assert!(!decoded.is_deleted(1003, 1000));
    }

    #[test]
    fn rejects_row_id_count_over_roaring_capacity() {
        let bitmap = RoaringBitmap::new();
        let err = build_deletion_vector(&bitmap, MAX_ROW_ID_COUNT + 1).unwrap_err();
        assert_eq!(err, DeletionError::RowIdCountExceedsRoaringCapacity);
    }

    #[test]
    fn decode_rejects_malformed_bytes() {
        let err = DeletionVector::decode(&[0xFF, 0x00, 0x01]).unwrap_err();
        assert_eq!(err, DeletionError::Malformed);
    }

    #[test]
    fn always_serializes_without_run_containers_regardless_of_insertion_api() {
        // The same logical set, built two different ways: one that stays
        // array-container, one that can select a run container
        // (contiguous-range insertion) — the identical property
        // filter_bitmaps.rs's own round-trip test checks (spec/deletion.md
        // §2's MUST rule).
        let mut one_at_a_time = RoaringBitmap::new();
        for i in 10..20 {
            one_at_a_time.insert(i);
        }
        let mut via_range = RoaringBitmap::new();
        via_range.insert_range(10..20);

        let a = build_deletion_vector(&one_at_a_time, 1000).unwrap();
        let b = build_deletion_vector(&via_range, 1000).unwrap();
        assert_eq!(a, b, "both insertion paths must serialize identically");
    }

    #[test]
    fn read_fetches_verifies_checksum_and_decodes() {
        let store = InMemoryStore::new();
        let mut bitmap = RoaringBitmap::new();
        bitmap.insert(7);
        let bytes = build_deletion_vector(&bitmap, 100).unwrap();
        let checksum = twox_hash::XxHash3_64::oneshot(&bytes);
        store.put_if_absent("dv/seg0.bin", &bytes).unwrap();

        let dv_ref = DeletionVectorRef {
            path: "dv/seg0.bin".to_string(),
            byte_length: bytes.len() as u64,
            checksum,
        };
        let decoded = read(&store, &dv_ref).unwrap();
        assert!(decoded.is_deleted(1007, 1000));
        assert!(!decoded.is_deleted(1008, 1000));
    }

    #[test]
    fn read_rejects_a_checksum_mismatch() {
        let store = InMemoryStore::new();
        let bitmap = RoaringBitmap::new();
        let bytes = build_deletion_vector(&bitmap, 100).unwrap();
        store.put_if_absent("dv/seg0.bin", &bytes).unwrap();

        let dv_ref = DeletionVectorRef {
            path: "dv/seg0.bin".to_string(),
            byte_length: bytes.len() as u64,
            checksum: 0xDEAD_BEEF,
        };
        let err = read(&store, &dv_ref).unwrap_err();
        assert!(matches!(err, DeletionError::ChecksumMismatch { .. }));
    }

    #[test]
    fn read_reports_not_found_for_a_missing_object() {
        let store = InMemoryStore::new();
        let dv_ref = DeletionVectorRef {
            path: "dv/nonexistent.bin".to_string(),
            byte_length: 0,
            checksum: 0,
        };
        assert_eq!(read(&store, &dv_ref).unwrap_err(), DeletionError::NotFound);
    }

    #[test]
    #[should_panic(expected = "row_id must be >= row_id_base")]
    fn is_deleted_rejects_a_row_id_below_the_segment_base() {
        let bitmap = RoaringBitmap::new();
        let dv = DeletionVector { bitmap };
        dv.is_deleted(5, 1000);
    }
}
