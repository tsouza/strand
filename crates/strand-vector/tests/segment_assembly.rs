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

//! The first end-to-end test for the vector blob family (RFC 0010): all
//! four blob types, assembled into a real STRAND segment via
//! `strand-core`'s actual `SegmentBuilder`, opened cold (a real footer and
//! hotcache decode, per invariant 3), and every blob re-read from the
//! resulting registry entries — proving the pieces actually compose, the
//! same proof `field_end_to_end.rs` established for the lexical family.

use rand::SeedableRng;
use rand::rngs::StdRng;
use strand_core::container::{ChunkCodec, Footer, Hotcache, StorageClass, Tier};
use strand_core::segment::{BlobSpec, SegmentBuilder};
use strand_vector::descriptor::{self, DescriptorReader, DistanceMetric};
use strand_vector::flat::{FlatVectorsReader, build_flat_vectors};
use strand_vector::navigation::{ClusterDirEntry, NavigationTierReader, build_navigation_tier};
use strand_vector::posting_list::{ClusterInput, PostingListReader, build_posting_lists};

const FAMILY_ID: u16 = 3;
const BLOB_FLAT_VECTORS: u16 = 0;
const BLOB_DESCRIPTOR: u16 = 1;
const BLOB_NAVIGATION_TIER: u16 = 2;
const BLOB_POSTING_LISTS: u16 = 3;

fn open_segment_bytes(bytes: &[u8]) -> Hotcache {
    let footer_bytes: [u8; 40] = bytes[bytes.len() - 40..].try_into().unwrap();
    let footer = Footer::decode(&footer_bytes).expect("valid footer");
    let start = footer.hotcache_offset as usize;
    let end = start + footer.hotcache_length as usize;
    Hotcache::decode(&bytes[start..end]).expect("valid hotcache")
}

/// `(offset, length)` of the `family_id=3` blob with the given
/// `blob_type_id`, per the hotcache's own registry.
fn find_blob(hotcache: &Hotcache, blob_type_id: u16) -> (usize, usize) {
    let entry = hotcache
        .blobs
        .iter()
        .find(|b| b.family_id == FAMILY_ID && b.blob_type_id == blob_type_id)
        .unwrap_or_else(|| panic!("no family_id=3 blob_type_id={blob_type_id} in the registry"));
    (entry.offset as usize, entry.length as usize)
}

#[test]
fn assembles_and_reopens_all_four_vector_blob_types() {
    let dims = 64u32;
    let padded_dims = dims as usize; // already a multiple of 64

    // 1. Quantization descriptor.
    let mut rng = StdRng::seed_from_u64(7);
    let descriptor_bytes = descriptor::build_fht_kac(dims, DistanceMetric::L2, 1, &mut rng);

    // 2. Cluster navigation tier: 2 clusters.
    let centroid0 = vec![1.0f32; padded_dims];
    let centroid1 = vec![-1.0f32; padded_dims];
    let mut centroids = centroid0.clone();
    centroids.extend(&centroid1);
    let dirs = vec![
        ClusterDirEntry {
            region_offset: 0,
            code_bytes_length: 640,
            vector_count: 3,
        },
        ClusterDirEntry {
            region_offset: 664,
            code_bytes_length: 640,
            vector_count: 2,
        },
    ];
    let navigation_bytes = build_navigation_tier(&centroids, padded_dims, &dirs);

    // 3. Cluster posting lists: synthetic (non-quantized) codes — real
    // RaBitQ quantization is out of this crate's scope (RFC 0010 Design
    // §4); this test proves the wire format, not the quantization math.
    let cols = padded_dims / 8;
    let c0_codes: Vec<u8> = (0..(3 * cols)).map(|i| (i * 3 + 1) as u8).collect();
    let c0_factors = vec![1.0f32, 2.0, 3.0];
    let c1_codes: Vec<u8> = (0..(2 * cols)).map(|i| (i * 5 + 2) as u8).collect();
    let c1_factors = vec![4.0f32, 5.0];
    let c0_ids = vec![100u64, 101, 102];
    let c1_ids = vec![200u64, 201];
    let clusters = [
        ClusterInput {
            compact_codes: &c0_codes,
            f_add: &c0_factors,
            f_rescale: &c0_factors,
            f_error: &c0_factors,
            row_ids: &c0_ids,
            ex_region: None,
        },
        ClusterInput {
            compact_codes: &c1_codes,
            f_add: &c1_factors,
            f_rescale: &c1_factors,
            f_error: &c1_factors,
            row_ids: &c1_ids,
            ex_region: None,
        },
    ];
    let (posting_list_bytes, built_dirs) = build_posting_lists(&clusters, padded_dims);
    assert_eq!(
        built_dirs, dirs,
        "navigation tier's directory must match what build_posting_lists actually produced"
    );

    // 4. Flat vectors: 5 rows (row-ids 100-102, 200-201, but stored densely
    // by local ordinal — this test just needs 5 arbitrary rows).
    let flat_data: Vec<f32> = (0..5 * dims as usize).map(|i| i as f32 * 0.1).collect();
    let flat_bytes = build_flat_vectors(&flat_data, 5, dims as usize);

    // Assemble a real segment, per RFC 0010 Design §1's tier/storage-class
    // table.
    let mut builder = SegmentBuilder::new(5);
    builder.add_blob(BlobSpec {
        family_id: FAMILY_ID,
        field_id: 0,
        blob_type_id: BLOB_FLAT_VECTORS,
        storage_class: StorageClass::RawMappable,
        tier: Tier::NotApplicable,
        alignment: 8,
        chunk_codec: ChunkCodec::None,
        chunk_codec_level: 0,
        data: flat_bytes.clone(),
    });
    builder.add_blob(BlobSpec {
        family_id: FAMILY_ID,
        field_id: 0,
        blob_type_id: BLOB_DESCRIPTOR,
        storage_class: StorageClass::RawMappable,
        tier: Tier::ColdFetchable,
        alignment: 8,
        chunk_codec: ChunkCodec::None,
        chunk_codec_level: 0,
        data: descriptor_bytes.clone(),
    });
    builder.add_blob(BlobSpec {
        family_id: FAMILY_ID,
        field_id: 0,
        blob_type_id: BLOB_NAVIGATION_TIER,
        storage_class: StorageClass::RawMappable,
        tier: Tier::ColdFetchable,
        alignment: 8,
        chunk_codec: ChunkCodec::None,
        chunk_codec_level: 0,
        data: navigation_bytes.clone(),
    });
    builder.add_blob(BlobSpec {
        family_id: FAMILY_ID,
        field_id: 0,
        blob_type_id: BLOB_POSTING_LISTS,
        storage_class: StorageClass::RawMappable,
        tier: Tier::ColdFetchable,
        alignment: 8,
        chunk_codec: ChunkCodec::None,
        chunk_codec_level: 0,
        data: posting_list_bytes.clone(),
    });

    let segment_bytes = builder.build(0);

    // Real reader path: footer -> hotcache -> blob registry (invariant 3's
    // one-wave rule: everything past this point is already-resident bytes
    // once the four blobs' own ranges are fetched).
    let hotcache = open_segment_bytes(&segment_bytes);
    assert_eq!(hotcache.row_id_count, 5);
    assert_eq!(
        hotcache
            .blobs
            .iter()
            .filter(|b| b.family_id == FAMILY_ID)
            .count(),
        4
    );

    let (offset, length) = find_blob(&hotcache, BLOB_DESCRIPTOR);
    let extracted = &segment_bytes[offset..offset + length];
    assert_eq!(extracted, &descriptor_bytes[..]);
    let descriptor_reader = DescriptorReader::new(extracted).expect("valid descriptor");
    assert_eq!(descriptor_reader.dims(), 64);
    assert_eq!(descriptor_reader.padded_dims(), 64);

    let (offset, length) = find_blob(&hotcache, BLOB_NAVIGATION_TIER);
    let extracted = &segment_bytes[offset..offset + length];
    assert_eq!(extracted, &navigation_bytes[..]);
    let navigation_reader =
        NavigationTierReader::new(extracted, padded_dims).expect("valid navigation tier");
    assert_eq!(navigation_reader.num_clusters(), 2);
    assert_eq!(navigation_reader.centroid(0), centroid0);
    assert_eq!(navigation_reader.centroid(1), centroid1);

    let (offset, length) = find_blob(&hotcache, BLOB_POSTING_LISTS);
    let extracted = &segment_bytes[offset..offset + length];
    assert_eq!(extracted, &posting_list_bytes[..]);
    let posting_list_reader = PostingListReader::new(extracted);
    // Real one-wave query resolution: select both clusters from the
    // navigation tier (already in hand), then read each cluster's region
    // directly from the already-fetched posting-list blob bytes — no
    // further round trip, per invariant 3.
    for cluster_idx in 0..navigation_reader.num_clusters() {
        let dir = navigation_reader.cluster_dir(cluster_idx);
        let region = posting_list_reader
            .read_cluster(&dir, padded_dims, 0)
            .expect("valid cluster region");
        if cluster_idx == 0 {
            assert_eq!(region.compact_codes, c0_codes);
            assert_eq!(region.row_ids, c0_ids);
        } else {
            assert_eq!(region.compact_codes, c1_codes);
            assert_eq!(region.row_ids, c1_ids);
        }
    }

    let (offset, length) = find_blob(&hotcache, BLOB_FLAT_VECTORS);
    let extracted = &segment_bytes[offset..offset + length];
    assert_eq!(extracted, &flat_bytes[..]);
    let flat_reader = FlatVectorsReader::new(extracted, dims as usize).expect("valid flat vectors");
    assert_eq!(flat_reader.row_id_count(), 5);
    assert_eq!(flat_reader.vector(0), flat_data[0..64]);
}
