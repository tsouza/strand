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

//! Builds a segment's on-disk bytes from blob content (RFC 0001 §1), and
//! writes a built segment to a `ConditionalStore`, producing the
//! `manifest::SegmentRef` a commit references it by (RFC 0001 §3).

use crate::container::{BlobEntry, ChunkCodec, Footer, Hotcache, StorageClass, Tier};
use crate::manifest::SegmentRef;
use crate::store::ConditionalStore;

/// One blob to include in a built segment: its registry metadata plus its
/// on-disk bytes. For a `raw-mappable` blob, `data` is the uncompressed
/// content directly; chunk-compressed blobs (M1/M2 codecs) are out of scope
/// here — this layer only assembles whatever bytes it's given.
pub struct BlobSpec {
    pub family_id: u16,
    pub blob_type_id: u16,
    /// Which field this blob belongs to, or
    /// [`crate::container::FIELD_ID_NONE`] for a segment-scoped blob with no
    /// field association. `spec/container.md` §5a;
    /// [`crate::container::field_id_from_name`] computes this from a
    /// field's declared name.
    pub field_id: u64,
    pub storage_class: StorageClass,
    pub tier: Tier,
    pub alignment: u16,
    pub chunk_codec: ChunkCodec,
    pub chunk_codec_level: u8,
    pub data: Vec<u8>,
}

/// Assembles a segment's on-disk bytes (data region + hotcache + footer)
/// from a fixed row count and a set of blobs, per RFC 0001 §1.
pub struct SegmentBuilder {
    row_id_count: u64,
    blobs: Vec<BlobSpec>,
}

impl SegmentBuilder {
    pub fn new(row_id_count: u64) -> Self {
        SegmentBuilder {
            row_id_count,
            blobs: Vec::new(),
        }
    }

    pub fn add_blob(&mut self, blob: BlobSpec) -> &mut Self {
        self.blobs.push(blob);
        self
    }

    /// Builds the full segment, with row-IDs starting at `row_id_base`.
    pub fn build(&self, row_id_base: u64) -> Vec<u8> {
        let mut data_region = Vec::new();
        let mut entries = Vec::with_capacity(self.blobs.len());
        for blob in &self.blobs {
            // spec/container.md §5: a raw-mappable blob's `offset` MUST be a
            // multiple of its declared `alignment`. Chunk-compressed blobs
            // carry no such obligation — their `alignment` field is
            // meaningless (invariant 10) and must not introduce a gap.
            if blob.storage_class == StorageClass::RawMappable && blob.alignment > 1 {
                let alignment = u64::from(blob.alignment);
                let remainder = data_region.len() as u64 % alignment;
                if remainder != 0 {
                    let padding = (alignment - remainder) as usize;
                    data_region.resize(data_region.len() + padding, 0);
                }
            }
            let offset = data_region.len() as u64;
            let checksum = twox_hash::XxHash3_64::oneshot(&blob.data);
            data_region.extend_from_slice(&blob.data);
            entries.push(BlobEntry {
                family_id: blob.family_id,
                blob_type_id: blob.blob_type_id,
                field_id: blob.field_id,
                storage_class: blob.storage_class,
                tier: blob.tier,
                alignment: blob.alignment,
                chunk_codec: blob.chunk_codec,
                chunk_codec_level: blob.chunk_codec_level,
                offset,
                length: blob.data.len() as u64,
                checksum,
            });
        }

        let hotcache = Hotcache {
            row_id_base,
            row_id_count: self.row_id_count,
            blobs: entries,
        };
        let hotcache_bytes = hotcache.encode();

        let footer = Footer {
            format_major: 0,
            format_minor: 1,
            hotcache_offset: data_region.len() as u64,
            hotcache_length: hotcache_bytes.len() as u64,
        };

        let mut segment = data_region;
        segment.extend_from_slice(&hotcache_bytes);
        segment.extend_from_slice(&footer.encode());
        segment
    }
}

/// Builds `builder` against `row_id_base`, writes it to `store` at `path`,
/// and returns the `SegmentRef` a manifest commit references it by. `path`
/// must be unique — segments are immutable (invariant 2) and this call
/// panics if something already occupies it, since a colliding path is
/// always a caller bug, never a real contention case (unlike the manifest's
/// nonce-suffixed snapshot paths, which are unique by construction).
pub fn write_segment<S: ConditionalStore>(
    store: &S,
    path: &str,
    builder: &SegmentBuilder,
    row_id_base: u64,
) -> SegmentRef {
    let bytes = builder.build(row_id_base);
    let checksum = twox_hash::XxHash3_64::oneshot(&bytes);
    store
        .put_if_absent(path, &bytes)
        .expect("segment path must be unique");
    SegmentRef {
        path: path.to_string(),
        row_id_base,
        row_id_count: builder.row_id_count,
        byte_length: bytes.len() as u64,
        checksum,
        deletion_vector: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_builder_matches_the_rfc_worked_example() {
        // rfcs/0001-container-rowid-manifest.md, "Worked example".
        let mut builder = SegmentBuilder::new(2);
        builder.add_blob(BlobSpec {
            family_id: 0,
            field_id: crate::container::FIELD_ID_NONE,
            blob_type_id: 0,
            storage_class: crate::container::StorageClass::RawMappable,
            tier: crate::container::Tier::NotApplicable,
            alignment: 8,
            chunk_codec: crate::container::ChunkCodec::None,
            chunk_codec_level: 0,
            data: vec![0x2A, 0x00, 0x00, 0x00, 0x2B, 0x00, 0x00, 0x00],
        });

        let bytes = builder.build(1000);

        let golden = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/container/toy-segment.bin"
        ))
        .unwrap();
        assert_eq!(bytes, golden);
    }

    #[test]
    fn build_pads_a_raw_mappable_blob_up_to_its_declared_alignment() {
        // spec/container.md §5: "A writer MUST place a raw-mappable blob at an
        // `offset` that is a multiple of its declared `alignment`."
        let mut builder = SegmentBuilder::new(2);
        builder.add_blob(BlobSpec {
            family_id: 0,
            field_id: crate::container::FIELD_ID_NONE,
            blob_type_id: 0,
            storage_class: StorageClass::RawMappable,
            tier: Tier::NotApplicable,
            alignment: 1,
            chunk_codec: ChunkCodec::None,
            chunk_codec_level: 0,
            data: vec![0xAA, 0xBB, 0xCC],
        });
        builder.add_blob(BlobSpec {
            family_id: 1,
            field_id: crate::container::FIELD_ID_NONE,
            blob_type_id: 0,
            storage_class: StorageClass::RawMappable,
            tier: Tier::NotApplicable,
            alignment: 8,
            chunk_codec: ChunkCodec::None,
            chunk_codec_level: 0,
            data: vec![0x11, 0x22, 0x33, 0x44],
        });

        let bytes = builder.build(0);
        let hotcache = decode_hotcache(&bytes);

        assert_eq!(hotcache.blobs[0].offset, 0);
        assert_eq!(
            hotcache.blobs[1].offset, 8,
            "second blob must start at a multiple of its declared alignment (8), \
             not immediately after the first blob's 3 bytes"
        );
        assert_eq!(&bytes[0..3], &[0xAA, 0xBB, 0xCC]);
        assert_eq!(&bytes[3..8], &[0, 0, 0, 0, 0], "padding bytes must be zero");
        assert_eq!(&bytes[8..12], &[0x11, 0x22, 0x33, 0x44]);
    }

    #[test]
    fn build_does_not_pad_chunk_compressed_blobs() {
        // Padding is a raw-mappable-only obligation (spec/container.md §5); a
        // chunk-compressed blob's `alignment` field is meaningless and MUST NOT
        // introduce a gap.
        let mut builder = SegmentBuilder::new(2);
        builder.add_blob(BlobSpec {
            family_id: 0,
            field_id: crate::container::FIELD_ID_NONE,
            blob_type_id: 0,
            storage_class: StorageClass::RawMappable,
            tier: Tier::NotApplicable,
            alignment: 1,
            chunk_codec: ChunkCodec::None,
            chunk_codec_level: 0,
            data: vec![0xAA, 0xBB, 0xCC],
        });
        builder.add_blob(BlobSpec {
            family_id: 1,
            field_id: crate::container::FIELD_ID_NONE,
            blob_type_id: 0,
            storage_class: StorageClass::ChunkCompressed,
            tier: Tier::ColdFetchable,
            alignment: 8,
            chunk_codec: ChunkCodec::Zstd,
            chunk_codec_level: 3,
            data: vec![0x11, 0x22, 0x33, 0x44],
        });

        let bytes = builder.build(0);
        let hotcache = decode_hotcache(&bytes);

        assert_eq!(
            hotcache.blobs[1].offset, 3,
            "chunk-compressed blobs are never padded, even with a nonzero \
             alignment field"
        );
    }

    fn decode_hotcache(segment_bytes: &[u8]) -> crate::container::Hotcache {
        let footer_bytes: [u8; 40] = segment_bytes[segment_bytes.len() - 40..]
            .try_into()
            .unwrap();
        let footer = crate::container::Footer::decode(&footer_bytes).unwrap();
        let start = footer.hotcache_offset as usize;
        let end = start + footer.hotcache_length as usize;
        crate::container::Hotcache::decode(&segment_bytes[start..end]).unwrap()
    }

    #[test]
    fn write_segment_stores_bytes_and_returns_a_matching_segment_ref() {
        let store = crate::store::InMemoryStore::new();
        let mut builder = SegmentBuilder::new(2);
        builder.add_blob(BlobSpec {
            family_id: 0,
            field_id: crate::container::FIELD_ID_NONE,
            blob_type_id: 0,
            storage_class: StorageClass::RawMappable,
            tier: Tier::NotApplicable,
            alignment: 8,
            chunk_codec: ChunkCodec::None,
            chunk_codec_level: 0,
            data: vec![0x2A, 0x00, 0x00, 0x00, 0x2B, 0x00, 0x00, 0x00],
        });

        let segment_ref = write_segment(&store, "segments/a.bin", &builder, 1000);

        assert_eq!(segment_ref.path, "segments/a.bin");
        assert_eq!(segment_ref.row_id_base, 1000);
        assert_eq!(segment_ref.row_id_count, 2);
        assert_eq!(segment_ref.byte_length, 110);

        let (stored_bytes, _) = crate::store::ConditionalStore::get(&store, "segments/a.bin")
            .unwrap()
            .expect("segment must be written to the store");
        assert_eq!(stored_bytes.len(), 110);
        assert_eq!(
            segment_ref.checksum,
            twox_hash::XxHash3_64::oneshot(&stored_bytes)
        );
    }
}
