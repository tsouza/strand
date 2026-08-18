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
/// location. Fixed 34 bytes, little-endian, per RFC 0001 §1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlobEntry {
    pub family_id: u16,
    pub blob_type_id: u16,
    pub storage_class: StorageClass,
    pub tier: Tier,
    pub alignment: u16,
    pub chunk_codec: ChunkCodec,
    pub chunk_codec_level: u8,
    pub offset: u64,
    pub length: u64,
    pub checksum: u64,
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
        let mut out = Vec::with_capacity(20 + self.blobs.len() * 34);
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

        let expected_len = 20 + blob_count * 34;
        if bytes.len() != expected_len {
            return Err(DecodeError::Truncated);
        }

        let mut blobs = Vec::with_capacity(blob_count);
        for i in 0..blob_count {
            let start = 20 + i * 34;
            let entry_bytes: [u8; 34] = bytes[start..start + 34].try_into().unwrap();
            blobs.push(BlobEntry::decode(&entry_bytes)?);
        }

        Ok(Hotcache {
            row_id_base,
            row_id_count,
            blobs,
        })
    }
}

impl BlobEntry {
    pub fn encode(&self) -> [u8; 34] {
        let mut out = [0u8; 34];
        out[0..2].copy_from_slice(&self.family_id.to_le_bytes());
        out[2..4].copy_from_slice(&self.blob_type_id.to_le_bytes());
        out[4] = self.storage_class as u8;
        out[5] = self.tier as u8;
        out[6..8].copy_from_slice(&self.alignment.to_le_bytes());
        out[8] = self.chunk_codec as u8;
        out[9] = self.chunk_codec_level;
        out[10..18].copy_from_slice(&self.offset.to_le_bytes());
        out[18..26].copy_from_slice(&self.length.to_le_bytes());
        out[26..34].copy_from_slice(&self.checksum.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8; 34]) -> Result<BlobEntry, DecodeError> {
        let storage_class = match bytes[4] {
            0 => StorageClass::ChunkCompressed,
            1 => StorageClass::RawMappable,
            other => return Err(DecodeError::InvalidStorageClass(other)),
        };
        let tier = match bytes[5] {
            0 => Tier::NotApplicable,
            1 => Tier::ColdFetchable,
            2 => Tier::Warm,
            other => return Err(DecodeError::InvalidTier(other)),
        };
        let chunk_codec = match bytes[8] {
            0 => ChunkCodec::None,
            1 => ChunkCodec::Zstd,
            other => return Err(DecodeError::InvalidChunkCodec(other)),
        };
        Ok(BlobEntry {
            family_id: u16::from_le_bytes(bytes[0..2].try_into().unwrap()),
            blob_type_id: u16::from_le_bytes(bytes[2..4].try_into().unwrap()),
            storage_class,
            tier,
            alignment: u16::from_le_bytes(bytes[6..8].try_into().unwrap()),
            chunk_codec,
            chunk_codec_level: bytes[9],
            offset: u64::from_le_bytes(bytes[10..18].try_into().unwrap()),
            length: u64::from_le_bytes(bytes[18..26].try_into().unwrap()),
            checksum: u64::from_le_bytes(bytes[26..34].try_into().unwrap()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // rfcs/0001-container-rowid-manifest.md, "Worked example" -> `blob_entry[0]`.
        let entry = BlobEntry {
            family_id: 0,
            blob_type_id: 0,
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
        assert_eq!(bytes[4], 0x01, "storage_class = raw-mappable");
        assert_eq!(bytes[5], 0x00, "tier = n/a");
        assert_eq!(bytes[6..8], [0x08, 0x00], "alignment");
        assert_eq!(bytes[8], 0x00, "chunk_codec = none");
        assert_eq!(bytes[9], 0x00, "chunk_codec_level");
        assert_eq!(
            bytes[10..18],
            [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            "offset"
        );
        assert_eq!(
            bytes[18..26],
            [0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            "length"
        );
        assert_eq!(bytes.len(), 34, "blob_entry is 34 bytes fixed");
    }

    #[test]
    fn hotcache_round_trips_through_encode_decode() {
        let hotcache = Hotcache {
            row_id_base: 1000,
            row_id_count: 2,
            blobs: vec![BlobEntry {
                family_id: 0,
                blob_type_id: 0,
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
        assert_eq!(bytes.len(), 20 + 34, "20-byte header + one 34-byte entry");
    }
}
