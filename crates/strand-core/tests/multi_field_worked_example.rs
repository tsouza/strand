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

//! The `field_id` worked example (roadmap item X-1,
//! `rfcs/0001-container-rowid-manifest.md` Discussion, `spec/container.md`
//! §5a): two fields, "title" and "body", each contributing a
//! `family_id = 1, blob_type_id = 0` blob (the real shape a term-dictionary
//! FST blob, RFC 0005, would use) to one segment — the exact
//! `(family_id, blob_type_id)` collision that was impossible to
//! disambiguate before `field_id` existed. Pinned as a real conformance
//! golden file (`conformance/container/multi-field-segment.bin`) so a
//! second implementation can verify its own `field_id_from_name` and
//! registry-entry encoding against real bytes, not just this repository's
//! own round-trip.

use strand_core::container::{
    ChunkCodec, Footer, Hotcache, StorageClass, Tier, field_id_from_name,
};
use strand_core::segment::{BlobSpec, SegmentBuilder};

/// Assembles the two-field toy segment: "title"'s blob (4 bytes, `11 11 11
/// 11`) then "body"'s blob (4 bytes, `22 22 22 22`), both
/// `family_id = 1, blob_type_id = 0`, 4-byte aligned, one row.
fn assemble_multi_field_segment() -> Vec<u8> {
    let mut builder = SegmentBuilder::new(1);
    builder.add_blob(BlobSpec {
        family_id: 1,
        blob_type_id: 0,
        field_id: field_id_from_name("title"),
        storage_class: StorageClass::RawMappable,
        tier: Tier::ColdFetchable,
        alignment: 4,
        chunk_codec: ChunkCodec::None,
        chunk_codec_level: 0,
        data: vec![0x11, 0x11, 0x11, 0x11],
    });
    builder.add_blob(BlobSpec {
        family_id: 1,
        blob_type_id: 0,
        field_id: field_id_from_name("body"),
        storage_class: StorageClass::RawMappable,
        tier: Tier::ColdFetchable,
        alignment: 4,
        chunk_codec: ChunkCodec::None,
        chunk_codec_level: 0,
        data: vec![0x22, 0x22, 0x22, 0x22],
    });
    builder.build(0)
}

#[test]
fn field_id_from_name_matches_the_rfc_pinned_worked_example_values() {
    // rfcs/0001-container-rowid-manifest.md, Discussion, "Task X-1" worked
    // example — computed by the reference implementation and pinned here so
    // a second implementation's own xxHash3-64-over-UTF-8-bytes computation
    // can be checked byte-for-byte, not just "produces some u64."
    assert_eq!(field_id_from_name("title"), 0x9605_0a97_f611_a0d5);
    assert_eq!(field_id_from_name("body"), 0x84f2_c8ee_187f_e1fa);
    assert_ne!(field_id_from_name("title"), field_id_from_name("body"));
}

#[test]
fn multi_field_worked_example_matches_conformance_golden_file() {
    let segment = assemble_multi_field_segment();
    let golden = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/container/multi-field-segment.bin"
    ))
    .expect("golden file conformance/container/multi-field-segment.bin must exist");

    assert_eq!(segment, golden);
}

#[test]
fn multi_field_worked_example_is_correctly_disambiguated_on_read() {
    let segment = assemble_multi_field_segment();

    let footer_bytes: [u8; 40] = segment[segment.len() - 40..].try_into().unwrap();
    let footer = Footer::decode(&footer_bytes).unwrap();
    let start = footer.hotcache_offset as usize;
    let end = start + footer.hotcache_length as usize;
    let hotcache = Hotcache::decode(&segment[start..end]).unwrap();

    assert_eq!(hotcache.blobs.len(), 2);
    assert!(
        hotcache
            .blobs
            .iter()
            .all(|b| b.family_id == 1 && b.blob_type_id == 0),
        "both entries share family_id/blob_type_id — field_id is the only \
         thing that tells them apart"
    );

    let title_entry = hotcache
        .blobs
        .iter()
        .find(|b| b.field_id == field_id_from_name("title"))
        .expect("title's entry must be findable by its own field_id");
    let body_entry = hotcache
        .blobs
        .iter()
        .find(|b| b.field_id == field_id_from_name("body"))
        .expect("body's entry must be findable by its own field_id");

    let slice_of = |entry: &strand_core::container::BlobEntry| {
        &segment[entry.offset as usize..(entry.offset + entry.length) as usize]
    };
    assert_eq!(slice_of(title_entry), &[0x11, 0x11, 0x11, 0x11]);
    assert_eq!(slice_of(body_entry), &[0x22, 0x22, 0x22, 0x22]);
}
