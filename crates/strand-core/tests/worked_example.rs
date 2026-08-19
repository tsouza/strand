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

//! Assembles the toy segment from rfcs/0001-container-rowid-manifest.md's
//! "Worked example" and checks it against the pinned conformance golden file.

use strand_core::container::{ChunkCodec, Footer, Hotcache, StorageClass, Tier};
use strand_core::segment::{BlobSpec, SegmentBuilder};

fn assemble_toy_segment() -> Vec<u8> {
    let mut builder = SegmentBuilder::new(2);
    builder.add_blob(BlobSpec {
        family_id: 0,
        field_id: 0,
        blob_type_id: 0,
        storage_class: StorageClass::RawMappable,
        tier: Tier::NotApplicable,
        alignment: 8,
        chunk_codec: ChunkCodec::None,
        chunk_codec_level: 0,
        data: vec![0x2A, 0x00, 0x00, 0x00, 0x2B, 0x00, 0x00, 0x00],
    });
    builder.build(1000)
}

#[test]
fn worked_example_has_the_rfc_pinned_shape() {
    let segment = assemble_toy_segment();

    // 110 bytes total, up from the RFC 0001 worked example's original 102:
    // roadmap item X-1 (`field_id`, this RFC's own Discussion section)
    // grew `blob_entry` from 34 to 42 bytes, so the hotcache grows from 54
    // to 62 bytes and the footer now starts at 70, not 62.
    assert_eq!(segment.len(), 110, "total file size after adding field_id");
    assert_eq!(&segment[70..74], b"STRD", "footer starts at offset 70");

    let footer = Footer::decode(&segment[70..110].try_into().unwrap()).unwrap();
    assert_eq!(footer.hotcache_offset, 8);
    assert_eq!(footer.hotcache_length, 62);

    let hotcache_start = footer.hotcache_offset as usize;
    let hotcache_end = hotcache_start + footer.hotcache_length as usize;
    let hotcache = Hotcache::decode(&segment[hotcache_start..hotcache_end]).unwrap();
    assert_eq!(hotcache.row_id_base, 1000);
    assert_eq!(hotcache.row_id_count, 2);
    assert_eq!(hotcache.blobs.len(), 1);
    assert_eq!(hotcache.blobs[0].storage_class, StorageClass::RawMappable);
}

#[test]
fn worked_example_matches_conformance_golden_file() {
    let segment = assemble_toy_segment();
    let golden = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/container/toy-segment.bin"
    ))
    .expect("golden file conformance/container/toy-segment.bin must exist");

    assert_eq!(segment, golden);
}
