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

//! The segment container's footer trailer, hotcache region, and blob registry.
//! Layout is normative per RFC 0001 (`rfcs/0001-container-rowid-manifest.md`).

/// The fixed 40-byte trailer always found at the end of a segment file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Footer {
    pub format_major: u16,
    pub format_minor: u16,
    pub hotcache_offset: u64,
    pub hotcache_length: u64,
}

impl Footer {
    pub fn encode(&self) -> [u8; 40] {
        let mut out = [0u8; 40];
        out[0..4].copy_from_slice(b"STRD");
        out[4..6].copy_from_slice(&self.format_major.to_le_bytes());
        out[6..8].copy_from_slice(&self.format_minor.to_le_bytes());
        out[8..16].copy_from_slice(&self.hotcache_offset.to_le_bytes());
        out[16..24].copy_from_slice(&self.hotcache_length.to_le_bytes());
        out[24] = 1; // checksum_algo: xxHash3-64, invariant 11 default
        let checksum = twox_hash::XxHash3_64::oneshot(&out[0..32]);
        out[32..40].copy_from_slice(&checksum.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8; 40]) -> Result<Footer, DecodeError> {
        if &bytes[0..4] != b"STRD" {
            return Err(DecodeError::BadMagic);
        }
        let expected = twox_hash::XxHash3_64::oneshot(&bytes[0..32]);
        let actual = u64::from_le_bytes(bytes[32..40].try_into().unwrap());
        if expected != actual {
            return Err(DecodeError::ChecksumMismatch);
        }
        Ok(Footer {
            format_major: u16::from_le_bytes(bytes[4..6].try_into().unwrap()),
            format_minor: u16::from_le_bytes(bytes[6..8].try_into().unwrap()),
            hotcache_offset: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            hotcache_length: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    BadMagic,
    ChecksumMismatch,
    InvalidStorageClass(u8),
    InvalidTier(u8),
    InvalidChunkCodec(u8),
    Truncated,
}

/// A single registry entry in the hotcache, describing one blob's storage and
/// location. Fixed 42 bytes, little-endian, per `spec/container.md` §5
/// (originally 34 bytes under RFC 0001; grown by `field_id`, roadmap item
/// X-1, `rfcs/0008-positions.md` Discussion).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlobEntry {
    pub family_id: u16,
    pub blob_type_id: u16,
    /// Which field this blob belongs to, or [`FIELD_ID_NONE`] for a blob
    /// with no field association (a segment-scoped blob, or a legacy
    /// single-field/single-blob segment predating this field). Nonzero
    /// values are [`field_id`] applied to the field's declared name — see
    /// `spec/container.md` §5a.
    pub field_id: u64,
    pub storage_class: StorageClass,
    pub tier: Tier,
    pub alignment: u16,
    pub chunk_codec: ChunkCodec,
    pub chunk_codec_level: u8,
    pub offset: u64,
    pub length: u64,
    pub checksum: u64,
}

/// Reserved `field_id` value meaning "this blob has no field association" —
/// a segment-scoped blob (for example, a deletion vector) or a blob built
/// before this field existed. `spec/container.md` §5a: no real field may
/// hash to this value in a conforming index, and `field_id` treats `0` as
/// reserved rather than a possible (if astronomically unlikely) real hash
/// output, exactly as `family_id = 0` is already reserved (§9).
pub const FIELD_ID_NONE: u64 = 0;

/// Computes a blob registry entry's `field_id` from a field's declared name
/// (`spec/container.md` §5a): `xxHash3-64` over `name`'s raw UTF-8 bytes,
/// the same checksum algorithm the footer already names (`checksum_algo =
/// 1`), reused rather than registering a second hash function (invariant 8).
/// Any two conforming writers computing this function over the same field
/// name produce the same `field_id`, with no shared catalog blob or
/// coordination required (invariant 11) — this is what lets a reader that
/// only knows the field name it wants locate that field's blobs among many
/// without an extra round trip (invariant 3's one-wave rule). The name is
/// hashed exactly as given: no Unicode normalization, case-folding, or
/// trimming is applied, so two writers MUST agree on the exact declared
/// name bytes for their `field_id`s to match — schema agreement on field
/// names is an external, deployment-level concern the same way agreement on
/// analyzer descriptors is (invariant 6).
///
/// # Panics
///
/// Panics if `name` is empty. An empty field name is never a real field
/// identity, and `field_id_from_name("")` would otherwise silently collide
/// with [`FIELD_ID_NONE`] only if `xxHash3-64` of the empty string were `0`
/// (it is not, in fact, but a caller relying on that non-collision by luck
/// is a real, avoidable bug) — a real requirement, not a coincidence to
/// depend on.
pub fn field_id_from_name(name: &str) -> u64 {
    assert!(!name.is_empty(), "field name must not be empty");
    let hash = twox_hash::XxHash3_64::oneshot(name.as_bytes());
    if hash == FIELD_ID_NONE {
        // xxHash3-64 over a nonempty input landing exactly on 0 is a real,
        // if vanishingly unlikely (2^-64), possible output. Folding it away
        // from the reserved sentinel keeps `field_id_from_name`'s codomain
        // disjoint from `FIELD_ID_NONE` for every input, so a reader can
        // always tell "no field" apart from "a field whose hash happened to
        // be zero" without tracking which case it is separately.
        hash.wrapping_add(1)
    } else {
        hash
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageClass {
    ChunkCompressed = 0,
    RawMappable = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    NotApplicable = 0,
    ColdFetchable = 1,
    Warm = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkCodec {
    None = 0,
    Zstd = 1,
}

/// The navigation tier fetched wholesale at open: the segment's row-ID range
/// and its blob registry. Per RFC 0001 §1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hotcache {
    pub row_id_base: u64,
    pub row_id_count: u64,
    pub blobs: Vec<BlobEntry>,
}

impl Hotcache {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(20 + self.blobs.len() * BLOB_ENTRY_SIZE);
        out.extend_from_slice(&self.row_id_base.to_le_bytes());
        out.extend_from_slice(&self.row_id_count.to_le_bytes());
        out.extend_from_slice(&(self.blobs.len() as u32).to_le_bytes());
        for blob in &self.blobs {
            out.extend_from_slice(&blob.encode());
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Hotcache, DecodeError> {
        if bytes.len() < 20 {
            return Err(DecodeError::Truncated);
        }
        let row_id_base = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let row_id_count = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        let blob_count = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;

        let expected_len = 20 + blob_count * BLOB_ENTRY_SIZE;
        if bytes.len() != expected_len {
            return Err(DecodeError::Truncated);
        }

        let mut blobs = Vec::with_capacity(blob_count);
        for i in 0..blob_count {
            let start = 20 + i * BLOB_ENTRY_SIZE;
            let entry_bytes: [u8; BLOB_ENTRY_SIZE] =
                bytes[start..start + BLOB_ENTRY_SIZE].try_into().unwrap();
            blobs.push(BlobEntry::decode(&entry_bytes)?);
        }

        Ok(Hotcache {
            row_id_base,
            row_id_count,
            blobs,
        })
    }
}

/// Fixed on-disk size of one `BlobEntry`, bytes. `spec/container.md` §5.
pub const BLOB_ENTRY_SIZE: usize = 42;

impl BlobEntry {
    pub fn encode(&self) -> [u8; BLOB_ENTRY_SIZE] {
        let mut out = [0u8; BLOB_ENTRY_SIZE];
        out[0..2].copy_from_slice(&self.family_id.to_le_bytes());
        out[2..4].copy_from_slice(&self.blob_type_id.to_le_bytes());
        out[4..12].copy_from_slice(&self.field_id.to_le_bytes());
        out[12] = self.storage_class as u8;
        out[13] = self.tier as u8;
        out[14..16].copy_from_slice(&self.alignment.to_le_bytes());
        out[16] = self.chunk_codec as u8;
        out[17] = self.chunk_codec_level;
        out[18..26].copy_from_slice(&self.offset.to_le_bytes());
        out[26..34].copy_from_slice(&self.length.to_le_bytes());
        out[34..42].copy_from_slice(&self.checksum.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8; BLOB_ENTRY_SIZE]) -> Result<BlobEntry, DecodeError> {
        let storage_class = match bytes[12] {
            0 => StorageClass::ChunkCompressed,
            1 => StorageClass::RawMappable,
            other => return Err(DecodeError::InvalidStorageClass(other)),
        };
        let tier = match bytes[13] {
            0 => Tier::NotApplicable,
            1 => Tier::ColdFetchable,
            2 => Tier::Warm,
            other => return Err(DecodeError::InvalidTier(other)),
        };
        let chunk_codec = match bytes[16] {
            0 => ChunkCodec::None,
            1 => ChunkCodec::Zstd,
            other => return Err(DecodeError::InvalidChunkCodec(other)),
        };
        Ok(BlobEntry {
            family_id: u16::from_le_bytes(bytes[0..2].try_into().unwrap()),
            blob_type_id: u16::from_le_bytes(bytes[2..4].try_into().unwrap()),
            field_id: u64::from_le_bytes(bytes[4..12].try_into().unwrap()),
            storage_class,
            tier,
            alignment: u16::from_le_bytes(bytes[14..16].try_into().unwrap()),
            chunk_codec,
            chunk_codec_level: bytes[17],
            offset: u64::from_le_bytes(bytes[18..26].try_into().unwrap()),
            length: u64::from_le_bytes(bytes[26..34].try_into().unwrap()),
            checksum: u64::from_le_bytes(bytes[34..42].try_into().unwrap()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn footer_round_trips_through_encode_decode() {
        let footer = Footer {
            format_major: 0,
            format_minor: 1,
            hotcache_offset: 8,
            hotcache_length: 54,
        };

        let bytes = footer.encode();
        let decoded = Footer::decode(&bytes).unwrap();

        assert_eq!(decoded, footer);
    }

    #[test]
    fn footer_decode_rejects_wrong_magic() {
        let mut bytes = [0u8; 40];
        bytes[0..4].copy_from_slice(b"NOPE");

        assert_eq!(Footer::decode(&bytes), Err(DecodeError::BadMagic));
    }

    #[test]
    fn footer_encode_sets_checksum_algo_and_footer_checksum() {
        let footer = Footer {
            format_major: 0,
            format_minor: 1,
            hotcache_offset: 8,
            hotcache_length: 54,
        };

        let bytes = footer.encode();

        assert_eq!(bytes[24], 1, "checksum_algo must be 1 (xxHash3-64)");
        assert_eq!(bytes[25..32], [0u8; 7], "reserved bytes must be zero");
        let expected = twox_hash::XxHash3_64::oneshot(&bytes[0..32]);
        assert_eq!(&bytes[32..40], &expected.to_le_bytes());
    }

    #[test]
    fn footer_decode_rejects_checksum_mismatch() {
        let footer = Footer {
            format_major: 0,
            format_minor: 1,
            hotcache_offset: 8,
            hotcache_length: 54,
        };
        let mut bytes = footer.encode();
        bytes[8] ^= 0xFF;

        assert_eq!(Footer::decode(&bytes), Err(DecodeError::ChecksumMismatch));
    }

    #[test]
    fn blob_entry_round_trips_through_encode_decode() {
        let entry = BlobEntry {
            family_id: 0,
            blob_type_id: 0,
            field_id: 0x0102_0304_0506_0708,
            storage_class: StorageClass::RawMappable,
            tier: Tier::NotApplicable,
            alignment: 8,
            chunk_codec: ChunkCodec::None,
            chunk_codec_level: 0,
            offset: 0,
            length: 8,
            checksum: 0x1234_5678_9abc_def0,
        };

        let bytes = entry.encode();
        let decoded = BlobEntry::decode(&bytes).unwrap();

        assert_eq!(decoded, entry);
    }

    #[test]
    fn blob_entry_encode_matches_rfc_worked_example() {
        // rfcs/0001-container-rowid-manifest.md, "Worked example" -> `blob_entry[0]`,
        // as amended by roadmap item X-1 (`field_id`, this RFC's own
        // Discussion section) — this anonymous placeholder blob carries no
        // field association, so `field_id = FIELD_ID_NONE`.
        let entry = BlobEntry {
            family_id: 0,
            blob_type_id: 0,
            field_id: FIELD_ID_NONE,
            storage_class: StorageClass::RawMappable,
            tier: Tier::NotApplicable,
            alignment: 8,
            chunk_codec: ChunkCodec::None,
            chunk_codec_level: 0,
            offset: 0,
            length: 8,
            checksum: 0,
        };

        let bytes = entry.encode();

        assert_eq!(bytes[0..2], [0x00, 0x00], "family_id");
        assert_eq!(bytes[2..4], [0x00, 0x00], "blob_type_id");
        assert_eq!(
            bytes[4..12],
            [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            "field_id = FIELD_ID_NONE"
        );
        assert_eq!(bytes[12], 0x01, "storage_class = raw-mappable");
        assert_eq!(bytes[13], 0x00, "tier = n/a");
        assert_eq!(bytes[14..16], [0x08, 0x00], "alignment");
        assert_eq!(bytes[16], 0x00, "chunk_codec = none");
        assert_eq!(bytes[17], 0x00, "chunk_codec_level");
        assert_eq!(
            bytes[18..26],
            [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            "offset"
        );
        assert_eq!(
            bytes[26..34],
            [0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            "length"
        );
        assert_eq!(bytes.len(), 42, "blob_entry is 42 bytes fixed");
    }

    #[test]
    fn field_id_from_name_is_deterministic_and_avoids_the_reserved_sentinel() {
        assert_eq!(field_id_from_name("title"), field_id_from_name("title"));
        assert_ne!(field_id_from_name("title"), field_id_from_name("body"));
        assert_ne!(field_id_from_name("title"), FIELD_ID_NONE);
        assert_ne!(field_id_from_name("body"), FIELD_ID_NONE);
    }

    #[test]
    #[should_panic(expected = "field name must not be empty")]
    fn field_id_from_name_rejects_empty_name() {
        field_id_from_name("");
    }

    #[test]
    fn hotcache_round_trips_through_encode_decode() {
        let hotcache = Hotcache {
            row_id_base: 1000,
            row_id_count: 2,
            blobs: vec![BlobEntry {
                family_id: 0,
                blob_type_id: 0,
                field_id: FIELD_ID_NONE,
                storage_class: StorageClass::RawMappable,
                tier: Tier::NotApplicable,
                alignment: 8,
                chunk_codec: ChunkCodec::None,
                chunk_codec_level: 0,
                offset: 0,
                length: 8,
                checksum: 0xdead_beef,
            }],
        };

        let bytes = hotcache.encode();
        let decoded = Hotcache::decode(&bytes).unwrap();

        assert_eq!(decoded, hotcache);
    }

    #[test]
    fn hotcache_encode_matches_rfc_worked_example_header() {
        // rfcs/0001-container-rowid-manifest.md, "Worked example" -> hotcache header.
        let hotcache = Hotcache {
            row_id_base: 1000,
            row_id_count: 2,
            blobs: vec![BlobEntry {
                family_id: 0,
                blob_type_id: 0,
                field_id: FIELD_ID_NONE,
                storage_class: StorageClass::RawMappable,
                tier: Tier::NotApplicable,
                alignment: 8,
                chunk_codec: ChunkCodec::None,
                chunk_codec_level: 0,
                offset: 0,
                length: 8,
                checksum: 0,
            }],
        };

        let bytes = hotcache.encode();

        assert_eq!(
            bytes[0..8],
            [0xE8, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            "row_id_base = 1000"
        );
        assert_eq!(
            bytes[8..16],
            [0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            "row_id_count = 2"
        );
        assert_eq!(bytes[16..20], [0x01, 0x00, 0x00, 0x00], "blob_count = 1");
        assert_eq!(bytes.len(), 20 + 42, "20-byte header + one 42-byte entry");
    }

    #[test]
    fn two_fields_own_blob_type_id_are_disambiguated_by_field_id() {
        // The scenario roadmap item X-1 exists to fix: two fields ("title"
        // and "body") each contribute a `family_id = 1, blob_type_id = 0`
        // blob (the shape of a real term-dictionary FST, RFC 0005) to the
        // same segment. Before `field_id`, a reader's `find(blob_type_id)`
        // (`crates/strand-lexical/src/field.rs`) could not tell these two
        // registry entries apart at all. With `field_id`, a reader that
        // knows the field name it wants finds exactly one entry.
        let title_id = field_id_from_name("title");
        let body_id = field_id_from_name("body");
        assert_ne!(title_id, body_id, "distinct field names must not collide");

        let hotcache = Hotcache {
            row_id_base: 0,
            row_id_count: 3,
            blobs: vec![
                BlobEntry {
                    family_id: 1,
                    blob_type_id: 0,
                    field_id: title_id,
                    storage_class: StorageClass::RawMappable,
                    tier: Tier::ColdFetchable,
                    alignment: 8,
                    chunk_codec: ChunkCodec::None,
                    chunk_codec_level: 0,
                    offset: 0,
                    length: 16,
                    checksum: 0x1111,
                },
                BlobEntry {
                    family_id: 1,
                    blob_type_id: 0,
                    field_id: body_id,
                    storage_class: StorageClass::RawMappable,
                    tier: Tier::ColdFetchable,
                    alignment: 8,
                    chunk_codec: ChunkCodec::None,
                    chunk_codec_level: 0,
                    offset: 16,
                    length: 24,
                    checksum: 0x2222,
                },
            ],
        };

        let bytes = hotcache.encode();
        let decoded = Hotcache::decode(&bytes).unwrap();
        assert_eq!(decoded, hotcache);

        let find = |field_id: u64| {
            decoded
                .blobs
                .iter()
                .find(|b| b.family_id == 1 && b.blob_type_id == 0 && b.field_id == field_id)
                .expect("entry must round-trip and remain findable by field_id")
        };
        let title_entry = find(title_id);
        let body_entry = find(body_id);
        assert_eq!(title_entry.length, 16, "title's own blob, not body's");
        assert_eq!(body_entry.length, 24, "body's own blob, not title's");
        assert_ne!(
            title_entry.offset, body_entry.offset,
            "the two same-blob_type_id entries stay independently addressable"
        );
    }

    proptest! {
        #[test]
        fn blob_entry_round_trips_for_any_field_id(
            family_id: u16,
            blob_type_id: u16,
            field_id: u64,
            storage_class_raw in 0u8..=1,
            tier_raw in 0u8..=2,
            alignment: u16,
            chunk_codec_raw in 0u8..=1,
            chunk_codec_level: u8,
            offset: u64,
            length: u64,
            checksum: u64,
        ) {
            let storage_class = if storage_class_raw == 0 {
                StorageClass::ChunkCompressed
            } else {
                StorageClass::RawMappable
            };
            let tier = match tier_raw {
                0 => Tier::NotApplicable,
                1 => Tier::ColdFetchable,
                _ => Tier::Warm,
            };
            let chunk_codec = if chunk_codec_raw == 0 {
                ChunkCodec::None
            } else {
                ChunkCodec::Zstd
            };

            let entry = BlobEntry {
                family_id,
                blob_type_id,
                field_id,
                storage_class,
                tier,
                alignment,
                chunk_codec,
                chunk_codec_level,
                offset,
                length,
                checksum,
            };

            let bytes = entry.encode();
            prop_assert_eq!(bytes.len(), BLOB_ENTRY_SIZE);
            let decoded = BlobEntry::decode(&bytes).unwrap();
            prop_assert_eq!(decoded, entry);
        }

        #[test]
        fn hotcache_round_trips_for_arbitrary_multi_field_blob_sets(
            row_id_base: u64,
            row_id_count: u64,
            field_ids in prop::collection::vec(any::<u64>(), 0..8),
        ) {
            let blobs: Vec<BlobEntry> = field_ids
                .into_iter()
                .enumerate()
                .map(|(i, field_id)| BlobEntry {
                    family_id: 1,
                    blob_type_id: (i % 5) as u16,
                    field_id,
                    storage_class: if i % 2 == 0 {
                        StorageClass::RawMappable
                    } else {
                        StorageClass::ChunkCompressed
                    },
                    tier: Tier::ColdFetchable,
                    alignment: 8,
                    chunk_codec: ChunkCodec::None,
                    chunk_codec_level: 0,
                    offset: i as u64 * 16,
                    length: 16,
                    checksum: i as u64,
                })
                .collect();

            let hotcache = Hotcache {
                row_id_base,
                row_id_count,
                blobs,
            };

            let bytes = hotcache.encode();
            let decoded = Hotcache::decode(&bytes).unwrap();
            prop_assert_eq!(decoded, hotcache);
        }

        #[test]
        fn field_id_from_name_never_collides_with_the_reserved_sentinel(
            name in "\\PC+",
        ) {
            prop_assert_ne!(field_id_from_name(&name), FIELD_ID_NONE);
        }
    }
}
