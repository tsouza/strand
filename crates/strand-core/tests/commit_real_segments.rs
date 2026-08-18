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

//! Wires container-format segment building (`segment::SegmentBuilder`,
//! `segment::write_segment`) into `manifest::commit`'s callback, then reads
//! the result back exactly as a real reader would: `read_snapshot`, fetch
//! the segment bytes the manifest names, decode them with the container
//! format. Proves the pieces compose, not just that each works alone.

use strand_core::container::{ChunkCodec, Footer, Hotcache, StorageClass, Tier};
use strand_core::manifest::{commit, read_snapshot};
use strand_core::segment::{BlobSpec, SegmentBuilder, write_segment};
use strand_core::store::{ConditionalStore, InMemoryStore};

fn toy_blob(byte_a: u8, byte_b: u8) -> BlobSpec {
    BlobSpec {
        family_id: 0,
        blob_type_id: 0,
        storage_class: StorageClass::RawMappable,
        tier: Tier::NotApplicable,
        alignment: 8,
        chunk_codec: ChunkCodec::None,
        chunk_codec_level: 0,
        data: vec![byte_a, 0x00, 0x00, 0x00, byte_b, 0x00, 0x00, 0x00],
    }
}

fn decode_hotcache(segment_bytes: &[u8]) -> (Footer, Hotcache) {
    let footer_start = segment_bytes.len() - 40;
    let footer_bytes: [u8; 40] = segment_bytes[footer_start..].try_into().unwrap();
    let footer = Footer::decode(&footer_bytes).unwrap();
    let hotcache_start = footer.hotcache_offset as usize;
    let hotcache_end = hotcache_start + footer.hotcache_length as usize;
    let hotcache = Hotcache::decode(&segment_bytes[hotcache_start..hotcache_end]).unwrap();
    (footer, hotcache)
}

#[test]
fn commit_writes_a_real_segment_that_round_trips_through_the_container_format() {
    let store = InMemoryStore::new();

    let committed = commit(&store, |next_row_id| {
        let mut builder = SegmentBuilder::new(2);
        builder.add_blob(toy_blob(0x2A, 0x2B));
        vec![write_segment(
            &store,
            "segments/one.bin",
            &builder,
            next_row_id,
        )]
    });

    assert_eq!(committed.segments.len(), 1);
    let segment_ref = &committed.segments[0];
    assert_eq!(segment_ref.row_id_base, 0);
    assert_eq!(segment_ref.row_id_count, 2);

    // A real reader's path: resolve the snapshot, then fetch and decode the
    // segment it names — not the writer's own in-memory values.
    let read_back = read_snapshot(&store).unwrap().unwrap();
    assert_eq!(read_back, committed);

    let (segment_bytes, _) = store.get(&segment_ref.path).unwrap();
    assert_eq!(segment_bytes.len() as u64, segment_ref.byte_length);
    assert_eq!(
        segment_ref.checksum,
        twox_hash::XxHash3_64::oneshot(&segment_bytes)
    );

    let (footer, hotcache) = decode_hotcache(&segment_bytes);
    assert_eq!(footer.hotcache_offset, 8);
    assert_eq!(hotcache.row_id_base, 0);
    assert_eq!(hotcache.row_id_count, 2);
    assert_eq!(hotcache.blobs.len(), 1);
    assert_eq!(hotcache.blobs[0].storage_class, StorageClass::RawMappable);
}

#[test]
fn two_commits_write_two_segments_with_continuous_non_overlapping_row_ids() {
    let store = InMemoryStore::new();

    commit(&store, |next_row_id| {
        let mut builder = SegmentBuilder::new(2);
        builder.add_blob(toy_blob(0x2A, 0x2B));
        vec![write_segment(
            &store,
            "segments/one.bin",
            &builder,
            next_row_id,
        )]
    });
    let second = commit(&store, |next_row_id| {
        let mut builder = SegmentBuilder::new(3);
        builder.add_blob(toy_blob(0x01, 0x02));
        vec![write_segment(
            &store,
            "segments/two.bin",
            &builder,
            next_row_id,
        )]
    });

    assert_eq!(second.segments.len(), 2);
    assert_eq!(second.segments[0].row_id_base, 0);
    assert_eq!(second.segments[1].row_id_base, 2);
    assert_eq!(second.next_row_id, 5);

    let (bytes_two, _) = store.get(&second.segments[1].path).unwrap();
    let (_, hotcache_two) = decode_hotcache(&bytes_two);
    assert_eq!(
        hotcache_two.row_id_base, 2,
        "the segment's own on-disk row_id_base matches the manifest's record of it"
    );
}
