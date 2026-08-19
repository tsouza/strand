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

//! The capstone test for this crate so far: builds a complete, real
//! cluster-family index from nothing but raw, unclustered vectors — no
//! pre-given centroid, matching a genuinely realistic index-build
//! scenario for the first time. Clusters the raw vectors with
//! `crate::kmeans`, rotates every centroid and vector with
//! `crate::rotate`, quantizes with `crate::quantize`, assembles a real
//! `strand-core` segment (all four blob types, matching
//! `segment_assembly.rs`'s own pattern), opens it cold, and queries it
//! with `crate::estimate` — confirming the answer matches brute-force
//! nearest-neighbor search over the original raw vectors.

use rand::SeedableRng;
use rand::rngs::StdRng;
use strand_core::container::{ChunkCodec, Footer, Hotcache, StorageClass, Tier};
use strand_core::segment::{BlobSpec, SegmentBuilder};
use strand_vector::descriptor::{self, DescriptorReader, DistanceMetric};
use strand_vector::estimate::{QueryFactors, estimate_distance};
use strand_vector::flat::build_flat_vectors;
use strand_vector::kmeans::{kmeans, recommended_cluster_count};
use strand_vector::navigation::{NavigationTierReader, build_navigation_tier};
use strand_vector::posting_list::{ClusterInput, PostingListReader, build_posting_lists};
use strand_vector::quantize::{MetricType, QuantizedVector, quantize_one_bit};
use strand_vector::rotate::rotate_fht_kac;

const FAMILY_ID: u16 = 3;
const BLOB_FLAT_VECTORS: u16 = 0;
const BLOB_DESCRIPTOR: u16 = 1;
const BLOB_NAVIGATION_TIER: u16 = 2;
const BLOB_POSTING_LISTS: u16 = 3;

fn next_f32(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    ((*state % 2001) as f32 - 1000.0) / 100.0
}

fn open_segment_bytes(bytes: &[u8]) -> Hotcache {
    let footer_bytes: [u8; 40] = bytes[bytes.len() - 40..].try_into().unwrap();
    let footer = Footer::decode(&footer_bytes).expect("valid footer");
    let start = footer.hotcache_offset as usize;
    let end = start + footer.hotcache_length as usize;
    Hotcache::decode(&bytes[start..end]).expect("valid hotcache")
}

fn find_blob(hotcache: &Hotcache, blob_type_id: u16) -> (usize, usize) {
    let entry = hotcache
        .blobs
        .iter()
        .find(|b| b.family_id == FAMILY_ID && b.blob_type_id == blob_type_id)
        .unwrap_or_else(|| panic!("no family_id=3 blob_type_id={blob_type_id} in the registry"));
    (entry.offset as usize, entry.length as usize)
}

#[test]
fn builds_a_real_index_from_raw_vectors_and_answers_a_query_correctly() {
    let dims = 32u32;
    let padded_dims = descriptor::padded_dims_for(dims) as usize;

    // 200 raw vectors: four well-separated blobs in a 32-dim space, plus
    // one vector deliberately closest to the query we'll issue.
    let vector_count = 200;
    let mut state = 0xB16B00B5_u64;
    let blob_centers: Vec<Vec<f32>> = (0..4)
        .map(|b| {
            (0..dims)
                .map(|d| ((b * 7 + d) as f32).sin() * 40.0)
                .collect()
        })
        .collect();
    let mut raw_vectors = Vec::with_capacity(vector_count);
    for i in 0..vector_count {
        let center = &blob_centers[i % 4];
        let v: Vec<f32> = center
            .iter()
            .map(|&c| c + next_f32(&mut state) * 0.5)
            .collect();
        raw_vectors.push(v);
    }
    let target_index = 42usize;
    let raw_query: Vec<f32> = raw_vectors[target_index]
        .iter()
        .map(|&v| v + next_f32(&mut state) * 0.05)
        .collect();

    // 1. Cluster the raw vectors (no pre-given centroids).
    let num_clusters = recommended_cluster_count(vector_count)
        .min(vector_count / 2)
        .max(2);
    let flat_raw: Vec<f32> = raw_vectors.iter().flatten().copied().collect();
    let mut rng = StdRng::seed_from_u64(99);
    let clustering = kmeans(
        &flat_raw,
        vector_count,
        dims as usize,
        num_clusters,
        50,
        &mut rng,
    );

    // Group row-ids by cluster (build_posting_lists needs ascending row-ids
    // per cluster).
    let mut clusters: Vec<Vec<usize>> = vec![Vec::new(); num_clusters];
    for (i, &c) in clustering.assignments.iter().enumerate() {
        clusters[c].push(i);
    }

    // 2. A real quantization descriptor + rotation.
    let mut rng2 = StdRng::seed_from_u64(100);
    let descriptor_bytes = descriptor::build_fht_kac(dims, DistanceMetric::L2, &mut rng2);
    let reader = DescriptorReader::new(&descriptor_bytes).unwrap();
    let flip = reader.rotation_payload();

    let rotated_centroids: Vec<Vec<f32>> = (0..num_clusters)
        .map(|c| {
            rotate_fht_kac(
                &clustering.centroids[c * dims as usize..(c + 1) * dims as usize],
                padded_dims,
                flip,
            )
        })
        .collect();

    // 3. Quantize every vector against its own cluster's rotated centroid.
    type ClusterData = (Vec<u8>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<u64>);
    let mut cluster_inputs_data: Vec<ClusterData> = Vec::with_capacity(num_clusters);
    for (c, members) in clusters.iter().enumerate() {
        let mut sorted_members = members.clone();
        sorted_members.sort_unstable();
        let mut codes = Vec::new();
        let mut f_add = Vec::with_capacity(sorted_members.len());
        let mut f_rescale = Vec::with_capacity(sorted_members.len());
        let mut f_error = Vec::with_capacity(sorted_members.len());
        let mut row_ids = Vec::with_capacity(sorted_members.len());
        for &i in &sorted_members {
            let rotated_vector = rotate_fht_kac(&raw_vectors[i], padded_dims, flip);
            let q = quantize_one_bit(&rotated_vector, &rotated_centroids[c], MetricType::L2);
            codes.extend(q.compact_code);
            f_add.push(q.f_add);
            f_rescale.push(q.f_rescale);
            f_error.push(q.f_error);
            row_ids.push(i as u64);
        }
        cluster_inputs_data.push((codes, f_add, f_rescale, f_error, row_ids));
    }
    let cluster_inputs: Vec<ClusterInput> = cluster_inputs_data
        .iter()
        .map(|(codes, f_add, f_rescale, f_error, row_ids)| ClusterInput {
            compact_codes: codes,
            f_add,
            f_rescale,
            f_error,
            row_ids,
        })
        .collect();
    let (posting_list_bytes, dirs) = build_posting_lists(&cluster_inputs, padded_dims);

    // 4. Navigation tier: rotated centroids + directory.
    let centroid_table: Vec<f32> = rotated_centroids.iter().flatten().copied().collect();
    let navigation_bytes = build_navigation_tier(&centroid_table, padded_dims, &dirs);

    // 5. Flat vectors (dense, local-ordinal order, for reranking).
    let flat_bytes = build_flat_vectors(&flat_raw, vector_count, dims as usize);

    // 6. Assemble a real segment.
    let mut builder = SegmentBuilder::new(vector_count as u64);
    builder.add_blob(BlobSpec {
        family_id: FAMILY_ID,
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
        blob_type_id: BLOB_NAVIGATION_TIER,
        storage_class: StorageClass::RawMappable,
        tier: Tier::ColdFetchable,
        alignment: 8,
        chunk_codec: ChunkCodec::None,
        chunk_codec_level: 0,
        data: navigation_bytes,
    });
    builder.add_blob(BlobSpec {
        family_id: FAMILY_ID,
        blob_type_id: BLOB_POSTING_LISTS,
        storage_class: StorageClass::RawMappable,
        tier: Tier::ColdFetchable,
        alignment: 8,
        chunk_codec: ChunkCodec::None,
        chunk_codec_level: 0,
        data: posting_list_bytes,
    });
    builder.add_blob(BlobSpec {
        family_id: FAMILY_ID,
        blob_type_id: BLOB_FLAT_VECTORS,
        storage_class: StorageClass::RawMappable,
        tier: Tier::NotApplicable,
        alignment: 8,
        chunk_codec: ChunkCodec::None,
        chunk_codec_level: 0,
        data: flat_bytes,
    });
    let segment_bytes = builder.build(0);

    // 7. Open cold, then run a real multi-cluster query (RFC 0010 Design
    // §6: rotate the query once, scan every cluster's already-fetched
    // bytes, no further round trip).
    let hotcache = open_segment_bytes(&segment_bytes);

    let (offset, length) = find_blob(&hotcache, BLOB_DESCRIPTOR);
    let descriptor_reader = DescriptorReader::new(&segment_bytes[offset..offset + length]).unwrap();
    assert_eq!(descriptor_reader.padded_dims() as usize, padded_dims);

    let (offset, length) = find_blob(&hotcache, BLOB_NAVIGATION_TIER);
    let navigation_reader =
        NavigationTierReader::new(&segment_bytes[offset..offset + length], padded_dims).unwrap();
    assert_eq!(navigation_reader.num_clusters(), num_clusters);

    let (offset, length) = find_blob(&hotcache, BLOB_POSTING_LISTS);
    let posting_reader = PostingListReader::new(&segment_bytes[offset..offset + length]);

    let rotated_query = rotate_fht_kac(&raw_query, padded_dims, flip);
    let query_factors = QueryFactors::new(&rotated_query);

    // nprobe = all clusters, for this test's own exhaustive correctness check.
    let mut ranked: Vec<(u64, f32)> = Vec::new();
    for c in 0..navigation_reader.num_clusters() {
        let dir = navigation_reader.cluster_dir(c);
        if dir.vector_count == 0 {
            continue;
        }
        let region = posting_reader.read_cluster(&dir, padded_dims).unwrap();
        let cols = padded_dims / 8;
        let centroid = navigation_reader.centroid(c);
        for i in 0..dir.vector_count as usize {
            let code = region.compact_codes[i * cols..(i + 1) * cols].to_vec();
            let quantized = QuantizedVector {
                compact_code: code,
                f_add: region.f_add[i],
                f_rescale: region.f_rescale[i],
                f_error: region.f_error[i],
            };
            let est = estimate_distance(
                &quantized,
                &rotated_query,
                &centroid,
                &query_factors,
                MetricType::L2,
            );
            ranked.push((region.row_ids[i], est.estimate));
        }
    }
    assert_eq!(
        ranked.len(),
        vector_count,
        "every real vector must appear exactly once across all clusters"
    );
    ranked.sort_by(|a, b| a.1.total_cmp(&b.1));

    assert_eq!(
        ranked[0].0,
        target_index as u64,
        "the deliberately-nearest vector must rank first across the whole real index; top 5: {:?}",
        &ranked[..5.min(ranked.len())]
    );

    let mut exact: Vec<(usize, f32)> = raw_vectors
        .iter()
        .enumerate()
        .map(|(i, v)| {
            (
                i,
                v.iter()
                    .zip(&raw_query)
                    .map(|(&a, &b)| (a - b) * (a - b))
                    .sum(),
            )
        })
        .collect();
    exact.sort_by(|a, b| a.1.total_cmp(&b.1));
    assert_eq!(
        exact[0].0, target_index,
        "sanity: brute-force must also rank the target first"
    );
}
