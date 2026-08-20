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

//! `strand-tools export-puffin`: a one-way, on-demand export of an
//! already-built STRAND segment (and, optionally, its current deletion
//! vector) into a standalone Puffin v1 file
//! (`references/puffin-spec-and-iceberg-rust-implementation.md`) a
//! Puffin-aware tool with no STRAND-specific code can open. Layout is
//! normative per `spec/puffin-export.md`, approved by RFC 0013
//! (`rfcs/0013-puffin-export-sidecar.md`, roadmap item M4-5).
//!
//! Two translations, and only two (`spec/puffin-export.md` §1): STRAND's
//! deletion-vector object (`spec/deletion.md` §2) becomes Puffin's own
//! registered `deletion-vector-v1` blob type, byte-exact
//! ([`build_deletion_vector_v1_blob`]); every other blob in the source
//! segment's blob registry (`spec/container.md` §5) is carried through
//! unmodified as one catch-all STRAND-namespaced type,
//! `strand-segment-blob-v1` ([`opaque_blobs_from_segment`]). A sidecar this
//! module produces is never referenced by `spec/manifest.md`'s snapshot
//! metadata and is never read by any STRAND reader — it is a standalone
//! export artifact, assembled once and handed to the caller
//! ([`write_puffin_file`]).
//!
//! Out of scope, per RFC 0013's own Non-goals: a Puffin-to-STRAND importer,
//! chunked or per-block export of a large blob (a blob's on-disk bytes
//! always travel as one opaque payload), and any change to
//! `spec/manifest.md`'s own snapshot format.

use strand_core::container::{DecodeError, Footer, Hotcache};

/// Puffin's own magic sequence, `"PFA1"` — identical at the start of the
/// file and inside the footer
/// (`references/puffin-spec-and-iceberg-rust-implementation.md`).
const PUFFIN_MAGIC: [u8; 4] = [0x50, 0x46, 0x41, 0x31];

/// `deletion-vector-v1`'s own 4-byte magic sequence, quoted verbatim in
/// the Puffin spec (`references/puffin-spec-and-iceberg-rust-implementation.md`).
const DELETION_VECTOR_MAGIC: [u8; 4] = [0xD1, 0xD3, 0x39, 0x64];

const FOOTER_TRAILER_SIZE: usize = 40;

#[derive(Debug)]
pub enum PuffinExportError {
    /// The segment bytes did not decode as a valid STRAND container
    /// (`spec/container.md`).
    Segment(DecodeError),
}

impl std::fmt::Display for PuffinExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PuffinExportError::Segment(e) => write!(f, "invalid STRAND segment: {e:?}"),
        }
    }
}

impl std::error::Error for PuffinExportError {}

/// One STRAND blob, carried through opaquely as a `strand-segment-blob-v1`
/// Puffin blob (`spec/puffin-export.md` §4) — every field copied straight
/// from that blob's own `spec/container.md` §5 registry entry, plus its
/// on-disk bytes, unmodified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueBlob {
    pub family_id: u16,
    pub blob_type_id: u16,
    pub field_id: u64,
    /// xxHash3-64 over the blob's on-disk bytes (invariant 11's default),
    /// carried forward as a `strand-checksum` property since Puffin's own
    /// `BlobMetadata` has no checksum field of any kind.
    pub checksum: u64,
    pub data: Vec<u8>,
}

/// The one real translation target (`spec/puffin-export.md` §3): a STRAND
/// deletion-vector object's own bytes, verbatim, plus the two properties
/// `deletion-vector-v1` requires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletionVectorExport {
    /// The deletion-vector object's raw bytes (`spec/deletion.md` §2) —
    /// carried into the translated blob's `bitmap[0]` field unmodified.
    pub bitmap_bytes: Vec<u8>,
    /// The exporting caller's declared `SegmentRef.path` for the source
    /// segment, copied into `deletion-vector-v1`'s required
    /// `referenced-data-file` property.
    pub referenced_data_file: String,
    /// The number of tombstoned rows, copied into `deletion-vector-v1`'s
    /// required `cardinality` property.
    pub cardinality: u64,
}

/// Decodes `segment_bytes` (a bare, already-built STRAND segment file —
/// `spec/container.md` §1) and returns one [`OpaqueBlob`] per entry in its
/// blob registry, in the registry's own order
/// (`spec/puffin-export.md` §2's deterministic-ordering rule).
///
/// This function opens the segment directly, the same way
/// `strand-tools inspect` does (`crates/strand-tools/src/inspect.rs`), not
/// through the manifest's `byte_length`-in-hand path — it is not bound by
/// invariant 3's round-trip budget, which governs the query-serving path
/// only (`spec/container.md` §3).
pub fn opaque_blobs_from_segment(
    segment_bytes: &[u8],
) -> Result<Vec<OpaqueBlob>, PuffinExportError> {
    if segment_bytes.len() < FOOTER_TRAILER_SIZE {
        return Err(PuffinExportError::Segment(DecodeError::Truncated));
    }
    let footer_bytes: [u8; FOOTER_TRAILER_SIZE] = segment_bytes
        [segment_bytes.len() - FOOTER_TRAILER_SIZE..]
        .try_into()
        .unwrap();
    let footer = Footer::decode(&footer_bytes).map_err(PuffinExportError::Segment)?;

    let hotcache_start = footer.hotcache_offset as usize;
    let hotcache_end = hotcache_start + footer.hotcache_length as usize;
    let hotcache = Hotcache::decode(
        segment_bytes
            .get(hotcache_start..hotcache_end)
            .ok_or(PuffinExportError::Segment(DecodeError::Truncated))?,
    )
    .map_err(PuffinExportError::Segment)?;

    let mut blobs = Vec::with_capacity(hotcache.blobs.len());
    for entry in &hotcache.blobs {
        let start = entry.offset as usize;
        let end = start + entry.length as usize;
        let data = segment_bytes
            .get(start..end)
            .ok_or(PuffinExportError::Segment(DecodeError::Truncated))?
            .to_vec();
        blobs.push(OpaqueBlob {
            family_id: entry.family_id,
            blob_type_id: entry.blob_type_id,
            field_id: entry.field_id,
            checksum: entry.checksum,
            data,
        });
    }
    Ok(blobs)
}

/// Builds a `deletion-vector-v1` blob's payload bytes from a STRAND
/// deletion-vector object's own bytes, per `spec/puffin-export.md` §3's
/// table: `combined_length` (4 bytes, big-endian) ‖ `magic` (4 bytes) ‖
/// `bitmap_count` (8 bytes, little-endian, always 1 —
/// `spec/deletion.md` §2's `row_id_count <= 2^32` cap means every local
/// ordinal fits under one 32-bit key) ‖ `key[0]` (4 bytes, little-endian,
/// always 0) ‖ `bitmap[0]` (`bitmap_bytes`, verbatim) ‖ `crc32` (4 bytes,
/// big-endian, IEEE/ISO-HDLC — the same variant `zlib.crc32` computes,
/// confirmed against RFC 0013's own worked example).
pub fn build_deletion_vector_v1_blob(bitmap_bytes: &[u8]) -> Vec<u8> {
    let mut inner = Vec::with_capacity(4 + 8 + 4 + bitmap_bytes.len());
    inner.extend_from_slice(&DELETION_VECTOR_MAGIC);
    inner.extend_from_slice(&1u64.to_le_bytes()); // bitmap_count
    inner.extend_from_slice(&0u32.to_le_bytes()); // key[0]
    inner.extend_from_slice(bitmap_bytes);

    let combined_length = u32::try_from(inner.len())
        .expect("a deletion vector's translated inner region fits in a u32");
    let crc = crc32fast::hash(&inner);

    let mut blob = Vec::with_capacity(4 + inner.len() + 4);
    blob.extend_from_slice(&combined_length.to_be_bytes());
    blob.extend_from_slice(&inner);
    blob.extend_from_slice(&crc.to_be_bytes());
    blob
}

/// Escapes `s` as a JSON string literal, including the surrounding quotes.
/// Puffin's footer payload is UTF-8 JSON
/// (`references/puffin-spec-and-iceberg-rust-implementation.md`); this
/// module builds that JSON by hand (`spec/puffin-export.md` §5's pinned key
/// order) rather than through `serde_json::Map`, whose default,
/// `preserve_order`-feature-free backing store sorts keys alphabetically
/// and would not reproduce the canonical order the worked example pins.
fn escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// One `properties` object's entries, in the order they must appear
/// (`spec/puffin-export.md` §5).
fn properties_json(entries: &[(&str, String)]) -> String {
    let mut out = String::from("{");
    for (i, (key, value)) in entries.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&escape_json_string(key));
        out.push(':');
        out.push_str(&escape_json_string(value));
    }
    out.push('}');
    out
}

/// One `BlobMetadata` footer entry, in the canonical field order
/// `spec/puffin-export.md` §5 pins.
fn blob_metadata_json(blob_type: &str, offset: u64, length: u64, properties: &str) -> String {
    format!(
        "{{\"type\":{},\"fields\":[],\"snapshot-id\":-1,\"sequence-number\":-1,\"offset\":{offset},\"length\":{length},\"properties\":{properties}}}",
        escape_json_string(blob_type),
    )
}

/// Assembles a complete Puffin v1 sidecar file
/// (`spec/puffin-export.md` §2), given an optional deletion-vector
/// translation and the source segment's opaque blobs (in registry order —
/// [`opaque_blobs_from_segment`] already returns them that way).
///
/// Blob ordering: the deletion vector (if present) always comes first,
/// then the opaque blobs in the order given, matching
/// `spec/puffin-export.md` §2's deterministic-ordering rule.
pub fn write_puffin_file(
    deletion_vector: Option<&DeletionVectorExport>,
    opaque_blobs: &[OpaqueBlob],
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&PUFFIN_MAGIC);

    let mut footer_entries = Vec::with_capacity(opaque_blobs.len() + 1);

    if let Some(dv) = deletion_vector {
        let blob = build_deletion_vector_v1_blob(&dv.bitmap_bytes);
        let offset = out.len() as u64;
        let length = blob.len() as u64;
        out.extend_from_slice(&blob);
        let properties = properties_json(&[
            ("referenced-data-file", dv.referenced_data_file.clone()),
            ("cardinality", dv.cardinality.to_string()),
        ]);
        footer_entries.push(blob_metadata_json(
            "deletion-vector-v1",
            offset,
            length,
            &properties,
        ));
    }

    for blob in opaque_blobs {
        let offset = out.len() as u64;
        let length = blob.data.len() as u64;
        out.extend_from_slice(&blob.data);
        let properties = properties_json(&[
            ("strand-family-id", blob.family_id.to_string()),
            ("strand-blob-type-id", blob.blob_type_id.to_string()),
            ("strand-field-id", blob.field_id.to_string()),
            ("strand-checksum", format!("{:016x}", blob.checksum)),
        ]);
        footer_entries.push(blob_metadata_json(
            "strand-segment-blob-v1",
            offset,
            length,
            &properties,
        ));
    }

    let created_by = format!(
        "strand-tools {} (rfc-0013-puffin-export)",
        env!("CARGO_PKG_VERSION")
    );
    let footer_json = format!(
        "{{\"blobs\":[{}],\"properties\":{{\"created-by\":{}}}}}",
        footer_entries.join(","),
        escape_json_string(&created_by),
    );
    let footer_bytes = footer_json.into_bytes();

    out.extend_from_slice(&PUFFIN_MAGIC);
    out.extend_from_slice(&footer_bytes);
    let footer_payload_size = i32::try_from(footer_bytes.len())
        .expect("a footer payload fits comfortably in an i32-sized length field");
    out.extend_from_slice(&footer_payload_size.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // flags: footer uncompressed, all bits reserved/zero
    out.extend_from_slice(&PUFFIN_MAGIC);

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use strand_core::container::{
        ChunkCodec, FIELD_ID_NONE, StorageClass, Tier, field_id_from_name,
    };
    use strand_core::deletion::{DeletionVector, RoaringBitmap, build_deletion_vector};
    use strand_core::segment::{BlobSpec, SegmentBuilder};

    /// The exact 22-byte deletion vector `rfcs/0012-deletion-vectors.md`'s
    /// own worked example builds (local ordinals `{2, 5, 100}`, segment
    /// `row_id_count = 200`) — the identical input
    /// `crates/strand-core/src/deletion.rs`'s own
    /// `round_trips_a_real_bitmap_through_build_and_decode` test uses, and
    /// the input `rfcs/0013-puffin-export-sidecar.md`'s worked example
    /// translates.
    fn rfc_0012_worked_example_bitmap_bytes() -> Vec<u8> {
        let mut bitmap = RoaringBitmap::new();
        bitmap.insert(2);
        bitmap.insert(5);
        bitmap.insert(100);
        build_deletion_vector(&bitmap, 200).unwrap()
    }

    #[test]
    fn deletion_vector_bitmap_bytes_match_rfc_0012_worked_example() {
        // Sanity check that this test file's own fixture matches the exact
        // bytes RFC 0012 and RFC 0013 both cite, before trusting any byte
        // built from it below.
        assert_eq!(
            rfc_0012_worked_example_bitmap_bytes(),
            vec![
                0x3a, 0x30, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x10, 0x00,
                0x00, 0x00, 0x02, 0x00, 0x05, 0x00, 0x64, 0x00,
            ]
        );
    }

    #[test]
    fn build_deletion_vector_v1_blob_matches_rfc_0013_worked_example_table() {
        // rfcs/0013-puffin-export-sidecar.md, "Worked example" ->
        // "Deletion-vector-v1 blob payload" (46 bytes) /
        // spec/puffin-export.md §7.
        let blob = build_deletion_vector_v1_blob(&rfc_0012_worked_example_bitmap_bytes());

        assert_eq!(blob.len(), 46, "combined_length(4)+inner(38)+crc32(4)");
        assert_eq!(&blob[0..4], &[0x00, 0x00, 0x00, 0x26], "combined_length");
        assert_eq!(&blob[4..8], &[0xD1, 0xD3, 0x39, 0x64], "magic");
        assert_eq!(
            &blob[8..16],
            &[0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            "bitmap_count"
        );
        assert_eq!(&blob[16..20], &[0x00, 0x00, 0x00, 0x00], "key[0]");
        assert_eq!(
            &blob[20..42],
            &[
                0x3A, 0x30, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x10, 0x00,
                0x00, 0x00, 0x02, 0x00, 0x05, 0x00, 0x64, 0x00,
            ],
            "bitmap[0], STRAND's own bytes verbatim"
        );
        assert_eq!(&blob[42..46], &[0x85, 0x87, 0x2A, 0xAD], "crc32");
    }

    #[test]
    fn deletion_vector_v1_blob_decodes_via_the_real_roaring_crate() {
        // A second, real-crate-backed check on top of the byte-table
        // assertion above: the translated bitmap region, decoded through
        // the same `roaring` crate STRAND's own reader uses, recovers the
        // exact tombstoned set.
        let bitmap_bytes = rfc_0012_worked_example_bitmap_bytes();
        let blob = build_deletion_vector_v1_blob(&bitmap_bytes);
        // bitmap[0] starts at byte 20 and runs to len - 4 (before crc32).
        let recovered = DeletionVector::decode(&blob[20..blob.len() - 4]).unwrap();
        assert!(recovered.is_deleted(1002, 1000));
        assert!(recovered.is_deleted(1005, 1000));
        assert!(recovered.is_deleted(1100, 1000));
        assert!(!recovered.is_deleted(1000, 1000));
    }

    #[test]
    fn full_puffin_file_matches_rfc_0013_worked_example_byte_for_byte() {
        // rfcs/0013-puffin-export-sidecar.md, "Worked example" -> "Full
        // Puffin file" (345 bytes) / spec/puffin-export.md §7. This is the
        // task's own strongest proof: the whole assembled file, not just
        // one blob's bytes.
        let dv = DeletionVectorExport {
            bitmap_bytes: rfc_0012_worked_example_bitmap_bytes(),
            referenced_data_file: "segments/0000000000000001.strand".to_string(),
            cardinality: 3,
        };

        let file = write_puffin_file(Some(&dv), &[]);

        let golden = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/puffin/toy-deletion-vector.puffin"
        ))
        .unwrap();
        assert_eq!(file.len(), 345);
        assert_eq!(
            file, golden,
            "must match the independently Python-built golden file bit for bit"
        );

        // Field-by-field, against the RFC's own offset table, so a
        // mismatch localizes immediately rather than as an opaque diff.
        assert_eq!(&file[0..4], &[0x50, 0x46, 0x41, 0x31], "file magic");
        assert_eq!(file[4..50].len(), 46, "blob 0 region");
        assert_eq!(&file[50..54], &[0x50, 0x46, 0x41, 0x31], "footer magic");
        let footer_payload = &file[54..333];
        assert_eq!(footer_payload.len(), 279);
        let expected_json = "{\"blobs\":[{\"type\":\"deletion-vector-v1\",\"fields\":[],\"snapshot-id\":-1,\"sequence-number\":-1,\"offset\":4,\"length\":46,\"properties\":{\"referenced-data-file\":\"segments/0000000000000001.strand\",\"cardinality\":\"3\"}}],\"properties\":{\"created-by\":\"strand-tools 0.1.0 (rfc-0013-puffin-export)\"}}";
        assert_eq!(std::str::from_utf8(footer_payload).unwrap(), expected_json);
        assert_eq!(
            &file[333..337],
            &[0x17, 0x01, 0x00, 0x00],
            "footer_payload_size"
        );
        assert_eq!(&file[337..341], &[0x00, 0x00, 0x00, 0x00], "flags");
        assert_eq!(&file[341..345], &[0x50, 0x46, 0x41, 0x31], "trailing magic");
    }

    #[test]
    fn opaque_blobs_from_segment_extracts_the_container_worked_example_blob() {
        // conformance/container/toy-segment.bin: one raw-mappable,
        // anonymous blob, two little-endian u32s (spec/container.md §7).
        let segment_bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/container/toy-segment.bin"
        ))
        .unwrap();

        let blobs = opaque_blobs_from_segment(&segment_bytes).unwrap();

        assert_eq!(blobs.len(), 1);
        assert_eq!(blobs[0].family_id, 0);
        assert_eq!(blobs[0].blob_type_id, 0);
        assert_eq!(blobs[0].field_id, FIELD_ID_NONE);
        assert_eq!(
            blobs[0].data,
            vec![0x2A, 0x00, 0x00, 0x00, 0x2B, 0x00, 0x00, 0x00]
        );
    }

    #[test]
    fn opaque_blobs_from_segment_preserves_registry_order_across_two_fields() {
        let title_id = field_id_from_name("title");
        let body_id = field_id_from_name("body");

        let mut builder = SegmentBuilder::new(3);
        builder.add_blob(BlobSpec {
            family_id: 1,
            blob_type_id: 0,
            field_id: title_id,
            storage_class: StorageClass::RawMappable,
            tier: Tier::ColdFetchable,
            alignment: 8,
            chunk_codec: ChunkCodec::None,
            chunk_codec_level: 0,
            data: vec![1, 2, 3, 4, 5, 6, 7, 8],
        });
        builder.add_blob(BlobSpec {
            family_id: 1,
            blob_type_id: 0,
            field_id: body_id,
            storage_class: StorageClass::RawMappable,
            tier: Tier::ColdFetchable,
            alignment: 8,
            chunk_codec: ChunkCodec::None,
            chunk_codec_level: 0,
            data: vec![9, 10, 11, 12, 13, 14, 15, 16],
        });
        let segment_bytes = builder.build(0);

        let blobs = opaque_blobs_from_segment(&segment_bytes).unwrap();

        assert_eq!(blobs.len(), 2);
        assert_eq!(blobs[0].field_id, title_id);
        assert_eq!(blobs[0].data, vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(blobs[1].field_id, body_id);
        assert_eq!(blobs[1].data, vec![9, 10, 11, 12, 13, 14, 15, 16]);
    }

    #[test]
    fn opaque_blobs_from_segment_rejects_truncated_input() {
        let err = opaque_blobs_from_segment(&[0u8; 10]).unwrap_err();
        assert!(matches!(
            err,
            PuffinExportError::Segment(DecodeError::Truncated)
        ));
    }

    #[test]
    fn write_puffin_file_with_only_opaque_blobs_places_no_deletion_vector_entry() {
        let opaque = OpaqueBlob {
            family_id: 3,
            blob_type_id: 2,
            field_id: 42,
            checksum: 0x1234_5678_9abc_def0,
            data: vec![0xAA; 16],
        };

        let file = write_puffin_file(None, std::slice::from_ref(&opaque));

        // magic(4) + blob(16) + footer_magic(4) + footer JSON + size(4) + flags(4) + magic(4)
        assert_eq!(&file[0..4], &PUFFIN_MAGIC);
        assert_eq!(&file[4..20], opaque.data.as_slice());
        assert_eq!(&file[20..24], &PUFFIN_MAGIC);
        let footer_payload_size =
            i32::from_le_bytes(file[file.len() - 12..file.len() - 8].try_into().unwrap());
        let footer_start = 24;
        let footer_end = footer_start + footer_payload_size as usize;
        let footer_json = std::str::from_utf8(&file[footer_start..footer_end]).unwrap();
        assert!(footer_json.contains("\"type\":\"strand-segment-blob-v1\""));
        assert!(footer_json.contains("\"strand-family-id\":\"3\""));
        assert!(footer_json.contains("\"strand-blob-type-id\":\"2\""));
        assert!(footer_json.contains("\"strand-field-id\":\"42\""));
        assert!(footer_json.contains("\"strand-checksum\":\"123456789abcdef0\""));
        assert!(!footer_json.contains("deletion-vector-v1"));
        assert!(!footer_json.contains("compression-codec"));
        assert_eq!(&file[file.len() - 4..], &PUFFIN_MAGIC, "trailing magic");
        assert_eq!(
            &file[file.len() - 8..file.len() - 4],
            &[0x00, 0x00, 0x00, 0x00],
            "flags"
        );
    }

    #[test]
    fn escape_json_string_escapes_quotes_and_backslashes() {
        assert_eq!(escape_json_string("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(escape_json_string("plain"), "\"plain\"");
    }
}
