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

//! SPANN-style closure replication (M2-1, `docs/roadmap.md`; RFC 0010
//! Discussion — post-approval amendment), exercised end to end: a real
//! set of vectors, real cluster assignment (`crate::kmeans` in the second
//! test, hand-placed centroids in the first for full byte-level
//! hand-checkability), `crate::closure::closure_replicate` deciding
//! closure assignments, `crate::closure::group_by_cluster` inverting them,
//! and the resulting posting-list/navigation-tier bytes checked exactly —
//! plus a real query proving the replicated row-id is found from either
//! scanned cluster and deduplicated to one candidate, per `spec/
//! vectors.md` §6 step 3.

use rand::SeedableRng;
use rand::rngs::StdRng;
use strand_vector::closure::{ClosureConfig, closure_replicate, group_by_cluster};
use strand_vector::descriptor::{self, DescriptorReader, DistanceMetric};
use strand_vector::estimate::QueryFactors;
use strand_vector::kmeans::kmeans;
use strand_vector::navigation::{
    ClusterDirEntry, NavigationTierReader, ReplicationDescriptor, ReplicationPolicy,
    build_navigation_tier_with_replication,
};
use strand_vector::posting_list::{ClusterInput, PostingListReader, build_posting_lists};
use strand_vector::quantize::{MetricType, quantize_one_bit};
use strand_vector::query::{scan_selected_clusters, select_nprobe_clusters};
use strand_vector::rotate::rotate_fht_kac;

/// A tiny, real, fully hand-checkable case: 3 vectors (dims=64, matching
/// RFC 0010's own worked-example dimensionality so `padded_dims = dims`
/// with no rotation-padding arithmetic to carry along), 2 raw centroids
/// separated along a single coordinate so distances reduce to simple
/// arithmetic a reader can verify by hand. Codes are synthetic (as RFC
/// 0010's own worked example and `posting_list.rs`'s tests already do —
/// real RaBitQ quantization is a separate concern from this wire-layout
/// check), but every offset, length, and row-id byte below is real and
/// exact.
#[test]
fn closure_replication_hand_checked_byte_layout() {
    let dims = 64usize;
    let padded_dims = dims;

    // Raw centroids: only the first coordinate differs, the rest zero —
    // squared L2 distance from any vector below reduces to
    // `(x0 - c0)^2`.
    let centroid0 = vec![0.0f32; dims];
    let mut centroid1 = vec![0.0f32; dims];
    centroid1[0] = 10.0;

    // Three vectors, only the first coordinate populated:
    //   idx0 (row-id 100): x0=1  -> d(c0)=1,  d(c1)=81 -> primary c0 only.
    //   idx1 (row-id 150): x0=9  -> d(c0)=81, d(c1)=1  -> primary c1 only.
    //   idx2 (row-id 200): x0=5  -> d(c0)=25, d(c1)=25 -> exact midpoint,
    //     primary c0 (by the hand-supplied primary_assignments below),
    //     replicates into c1 since the ratio is exactly 1.0 <= (1+0)*1.0.
    let mut v0 = vec![0.0f32; dims];
    v0[0] = 1.0;
    let mut v1 = vec![0.0f32; dims];
    v1[0] = 9.0;
    let mut v2 = vec![0.0f32; dims];
    v2[0] = 5.0;
    let vectors: Vec<f32> = [v0, v1, v2].concat();
    let centroids: Vec<f32> = [centroid0.clone(), centroid1.clone()].concat();
    let primary_assignments = vec![0usize, 1, 0];

    let config = ClosureConfig {
        epsilon: 0.0,
        max_replicas: 8,
        apply_rng_rule: false,
    };
    let assignments = closure_replicate(
        &vectors,
        3,
        dims,
        &centroids,
        2,
        &primary_assignments,
        &config,
    );
    assert_eq!(assignments[0], vec![0], "idx0: no replication");
    assert_eq!(assignments[1], vec![1], "idx1: no replication");
    assert_eq!(
        assignments[2],
        vec![0, 1],
        "idx2: exact midpoint replicates into cluster 1"
    );

    let grouped = group_by_cluster(&assignments, 2);
    assert_eq!(
        grouped[0],
        vec![0, 2],
        "cluster 0: idx0 primary, idx2 replica"
    );
    assert_eq!(
        grouped[1],
        vec![1, 2],
        "cluster 1: idx1 primary, idx2 replica"
    );

    // Row-ids monotonic in vector index (100, 150, 200), so grouping by
    // ascending vector index already yields ascending row-id order within
    // each cluster, matching `build_posting_lists`'s own precondition.
    let row_id_of = |idx: usize| -> u64 { 100 + idx as u64 * 50 };

    let synth_code = |idx: usize| -> Vec<u8> { vec![(idx * 10 + 1) as u8; padded_dims / 8] };

    type ClusterData = (Vec<u8>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<u64>);
    let build_cluster_data = |members: &[usize]| -> ClusterData {
        let mut codes = Vec::new();
        let mut f_add = Vec::new();
        let mut f_rescale = Vec::new();
        let mut f_error = Vec::new();
        let mut row_ids = Vec::new();
        for &idx in members {
            codes.extend(synth_code(idx));
            f_add.push(idx as f32);
            f_rescale.push(1.0 + idx as f32);
            f_error.push(-(idx as f32));
            row_ids.push(row_id_of(idx));
        }
        (codes, f_add, f_rescale, f_error, row_ids)
    };

    let c0_data = build_cluster_data(&grouped[0]);
    let c1_data = build_cluster_data(&grouped[1]);
    let clusters = [
        ClusterInput {
            compact_codes: &c0_data.0,
            f_add: &c0_data.1,
            f_rescale: &c0_data.2,
            f_error: &c0_data.3,
            row_ids: &c0_data.4,
            ex_region: None,
        },
        ClusterInput {
            compact_codes: &c1_data.0,
            f_add: &c1_data.1,
            f_rescale: &c1_data.2,
            f_error: &c1_data.3,
            row_ids: &c1_data.4,
            ex_region: None,
        },
    ];
    let (blob, dirs) = build_posting_lists(&clusters, padded_dims);

    // Exact byte layout, hand-checked: both clusters hold exactly 2
    // vectors, well under the 32-vector batch size, so each is one
    // (partially filled) batch at the same fixed 640-byte cost RFC 0010's
    // own worked example already established (`64*4 + 384`).
    assert_eq!(
        dirs[0],
        ClusterDirEntry {
            region_offset: 0,
            code_bytes_length: 640,
            vector_count: 2,
        }
    );
    assert_eq!(
        dirs[1],
        ClusterDirEntry {
            region_offset: 656, // 640 + 2*8 row-id bytes
            code_bytes_length: 640,
            vector_count: 2,
        }
    );
    assert_eq!(blob.len(), (640 + 2 * 8) * 2);

    // Row-id bytes, exact: cluster 0's row-ids (100, 200) at blob offset
    // 640; cluster 1's row-ids (150, 200) at blob offset 656+640=1296.
    // Row-id 200 appears in both — the replicated vector.
    let expected_c0_ids: [u8; 16] = [
        100, 0, 0, 0, 0, 0, 0, 0, // 100
        200, 0, 0, 0, 0, 0, 0, 0, // 200
    ];
    assert_eq!(&blob[640..656], &expected_c0_ids[..]);
    let expected_c1_ids: [u8; 16] = [
        150, 0, 0, 0, 0, 0, 0, 0, // 150
        200, 0, 0, 0, 0, 0, 0, 0, // 200
    ];
    assert_eq!(&blob[1296..1312], &expected_c1_ids[..]);

    // The navigation tier's replication trailer: real knobs, not a
    // provisional constant.
    let replication = ReplicationDescriptor::spann_closure(8, 0.0);
    let navigation_bytes =
        build_navigation_tier_with_replication(&centroids, padded_dims, &dirs, replication);
    // 8 (header) + 2*64*4 (centroid_table) + 2*24 (cluster_dir) + 8 (trailer)
    assert_eq!(navigation_bytes.len(), 8 + 512 + 48 + 8);

    let navigation = NavigationTierReader::new(&navigation_bytes, padded_dims).unwrap();
    let decoded = navigation.replication();
    assert_eq!(decoded.policy, Some(ReplicationPolicy::SpannClosure));
    assert_eq!(decoded.max_replicas, 8);
    assert_eq!(decoded.epsilon, 0.0);

    // Realized replication factor: 4 total assignments (2 per cluster)
    // over 3 distinct vectors.
    assert_eq!(navigation.realized_replication_factor(3), 4.0 / 3.0);

    // A reader can decode both clusters straight from the already-fetched
    // posting-list blob bytes, no further round trip.
    let reader = PostingListReader::new(&blob);
    let region0 = reader.read_cluster(&dirs[0], padded_dims, 0).unwrap();
    assert_eq!(region0.row_ids, vec![100, 200]);
    let region1 = reader.read_cluster(&dirs[1], padded_dims, 0).unwrap();
    assert_eq!(region1.row_ids, vec![150, 200]);
}

/// End to end with real clustering: real k-means assigns primary clusters,
/// `closure_replicate` decides real closure assignments against real
/// (non-hand-placed) centroids, a real index is built (real `FhtKacRotator`
/// rotation, real 1-bit RaBitQ quantization), and a real query proves the
/// deliberately-boundary-placed vector is found and deduplicated to a
/// single candidate when both its clusters are scanned.
#[test]
fn a_boundary_vector_is_found_via_either_cluster_and_deduplicated_by_query_resolution() {
    let dims = 32u32;
    let padded_dims = descriptor::padded_dims_for(dims) as usize;

    // Two well-separated blobs plus one deliberate boundary vector placed
    // exactly at their midpoint.
    let mut state = 0xC105_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        ((state % 2001) as f32 - 1000.0) / 1000.0 // small noise, [-1, 1]
    };
    let center_a: Vec<f32> = (0..dims).map(|d| d as f32).collect();
    let center_b: Vec<f32> = (0..dims).map(|d| d as f32 + 100.0).collect();
    let midpoint: Vec<f32> = center_a
        .iter()
        .zip(&center_b)
        .map(|(&a, &b)| (a + b) / 2.0)
        .collect();

    let mut raw_vectors: Vec<Vec<f32>> = Vec::new();
    for _ in 0..20 {
        raw_vectors.push(center_a.iter().map(|&c| c + next() * 0.5).collect());
    }
    for _ in 0..20 {
        raw_vectors.push(center_b.iter().map(|&c| c + next() * 0.5).collect());
    }
    let boundary_index = raw_vectors.len();
    raw_vectors.push(midpoint.clone());

    let n = raw_vectors.len();
    let flat_raw: Vec<f32> = raw_vectors.iter().flatten().copied().collect();

    let mut rng = StdRng::seed_from_u64(1234);
    let clustering = kmeans(&flat_raw, n, dims as usize, 2, 50, &mut rng);
    assert_ne!(
        clustering.assignments[0], clustering.assignments[20],
        "the two blobs must land in different clusters for this test to be meaningful"
    );

    // A loose-enough epsilon (SPANN's own default, `ClosureConfig::
    // spann_default`) that the boundary vector's near-equal distances to
    // both centroids pass the ratio test.
    let config = ClosureConfig::spann_default();
    let assignments = closure_replicate(
        &flat_raw,
        n,
        dims as usize,
        &clustering.centroids,
        2,
        &clustering.assignments,
        &config,
    );
    assert_eq!(
        assignments[boundary_index].len(),
        2,
        "the boundary vector must replicate into both clusters"
    );
    let boundary_primary = clustering.assignments[boundary_index];
    let boundary_replica_cluster = 1 - boundary_primary;
    assert!(assignments[boundary_index].contains(&boundary_replica_cluster));

    let grouped = group_by_cluster(&assignments, 2);

    let mut rng2 = StdRng::seed_from_u64(1235);
    let descriptor_bytes = descriptor::build_fht_kac(dims, DistanceMetric::L2, 1, &mut rng2);
    let flip = DescriptorReader::new(&descriptor_bytes)
        .unwrap()
        .rotation_payload()
        .to_vec();
    let rotated_centroids: Vec<Vec<f32>> = (0..2)
        .map(|c| {
            rotate_fht_kac(
                &clustering.centroids[c * dims as usize..(c + 1) * dims as usize],
                padded_dims,
                &flip,
            )
        })
        .collect();

    type ClusterData = (Vec<u8>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<u64>);
    let mut cluster_data: Vec<ClusterData> = Vec::with_capacity(2);
    for (c, members) in grouped.iter().enumerate() {
        let mut codes = Vec::new();
        let mut f_add = Vec::new();
        let mut f_rescale = Vec::new();
        let mut f_error = Vec::new();
        let mut row_ids = Vec::new();
        for &idx in members {
            let rotated = rotate_fht_kac(&raw_vectors[idx], padded_dims, &flip);
            let q = quantize_one_bit(&rotated, &rotated_centroids[c], MetricType::L2);
            codes.extend(q.compact_code);
            f_add.push(q.f_add);
            f_rescale.push(q.f_rescale);
            f_error.push(q.f_error);
            row_ids.push(idx as u64);
        }
        cluster_data.push((codes, f_add, f_rescale, f_error, row_ids));
    }
    let cluster_inputs: Vec<ClusterInput> = cluster_data
        .iter()
        .map(|(codes, f_add, f_rescale, f_error, row_ids)| ClusterInput {
            compact_codes: codes,
            f_add,
            f_rescale,
            f_error,
            row_ids,
            ex_region: None,
        })
        .collect();
    let (posting_list_bytes, dirs) = build_posting_lists(&cluster_inputs, padded_dims);
    let centroid_table: Vec<f32> = rotated_centroids.iter().flatten().copied().collect();
    let replication = ReplicationDescriptor::spann_closure(config.max_replicas, config.epsilon);
    let navigation_bytes =
        build_navigation_tier_with_replication(&centroid_table, padded_dims, &dirs, replication);

    let navigation = NavigationTierReader::new(&navigation_bytes, padded_dims).unwrap();
    let posting_reader = PostingListReader::new(&posting_list_bytes);

    // Query at the exact midpoint: scanning BOTH clusters (nprobe=2) must
    // find row-id `boundary_index` exactly once (deduplicated), per
    // `spec/vectors.md` §6 step 3.
    let rotated_query = rotate_fht_kac(&midpoint, padded_dims, &flip);
    let query_factors = QueryFactors::new(&rotated_query, 1);
    let selected = select_nprobe_clusters(&navigation, &rotated_query, 2, MetricType::L2);
    assert_eq!(selected.len(), 2);

    let candidates = scan_selected_clusters(
        &navigation,
        &posting_reader,
        &selected,
        &rotated_query,
        &query_factors,
        MetricType::L2,
        padded_dims,
        0,
    )
    .unwrap();
    let boundary_hits = candidates
        .iter()
        .filter(|c| c.row_id == boundary_index as u64)
        .count();
    assert_eq!(
        boundary_hits, 1,
        "the boundary row-id must appear exactly once after deduplication, not twice"
    );

    // The realized replication factor for this real index must be
    // strictly greater than 1.0 — real replication actually happened.
    let factor = navigation.realized_replication_factor(n as u64);
    assert!(
        factor > 1.0,
        "realized replication factor must exceed 1.0 when a real vector was replicated, got {factor}"
    );
}
