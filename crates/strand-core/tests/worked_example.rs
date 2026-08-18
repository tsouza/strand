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

use strand_core::container::{BlobEntry, ChunkCodec, Footer, Hotcache, StorageClass, Tier};

fn assemble_toy_segment() -> Vec<u8> {
    let data_region: [u8; 8] = [0x2A, 0x00, 0x00, 0x00, 0x2B, 0x00, 0x00, 0x00];

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
            checksum: twox_hash::XxHash3_64::oneshot(&data_region),
        }],
    };
    let hotcache_bytes = hotcache.encode();

    let footer = Footer {
        format_major: 0,
        format_minor: 1,
        hotcache_offset: data_region.len() as u64,
        hotcache_length: hotcache_bytes.len() as u64,
    };

    let mut segment = Vec::new();
    segment.extend_from_slice(&data_region);
    segment.extend_from_slice(&hotcache_bytes);
    segment.extend_from_slice(&footer.encode());
    segment
}

#[test]
fn worked_example_has_the_rfc_pinned_shape() {
    let segment = assemble_toy_segment();

    assert_eq!(segment.len(), 102, "total file size per the RFC");
    assert_eq!(&segment[62..66], b"STRD", "footer starts at offset 62");

    let footer = Footer::decode(&segment[62..102].try_into().unwrap()).unwrap();
    assert_eq!(footer.hotcache_offset, 8);
    assert_eq!(footer.hotcache_length, 54);

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
