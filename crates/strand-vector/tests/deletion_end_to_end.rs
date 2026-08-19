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

//! The first end-to-end test for deletion-vector integration (RFC 0012):
//! a real segment is committed through `strand-core`'s actual manifest CAS
//! protocol, a real deletion vector is committed against it through
//! `commit_deletion_vector`, and a real vector-family query
//! (`select_nprobe_clusters` → `scan_selected_clusters` →
//! `query::filter_deleted`) excludes the tombstoned row — proving the
//! whole chain from the manifest layer down to a single query composes,
//! not just that each piece works in isolation.

use rand::SeedableRng;
use rand::rngs::StdRng;
use strand_core::deletion::{self, DeletionVectorRef, RoaringBitmap};
use strand_core::manifest::{self, SegmentRef};
use strand_core::store::{ConditionalStore, InMemoryStore};
use strand_vector::descriptor::{self, DescriptorReader, DistanceMetric};
use strand_vector::estimate::QueryFactors;
use strand_vector::navigation::{NavigationTierReader, build_navigation_tier};
use strand_vector::posting_list::{ClusterInput, PostingListReader, build_posting_lists};
use strand_vector::quantize::{MetricType, quantize_one_bit};
use strand_vector::query::{filter_deleted, scan_selected_clusters, select_nprobe_clusters};
use strand_vector::rotate::rotate_fht_kac;

fn next_f32(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    ((*state % 2001) as f32 - 1000.0) / 200.0
}

#[test]
fn a_deleted_row_is_excluded_from_a_real_end_to_end_query() {
    let dims = 32u32;
    let padded_dims = descriptor::padded_dims_for(dims) as usize;

    let mut rng = StdRng::seed_from_u64(777);
    let descriptor_bytes = descriptor::build_fht_kac(dims, DistanceMetric::L2, 1, &mut rng);
    let reader = DescriptorReader::new(&descriptor_bytes).unwrap();
    let flip = reader.rotation_payload();

    let mut state = 0xC0FFEE_u64;
    let raw_centroid: Vec<f32> = (0..dims).map(|_| next_f32(&mut state)).collect();
    let rotated_centroid = rotate_fht_kac(&raw_centroid, padded_dims, flip);

    // Two vectors: one is the deliberate nearest neighbor to the query,
    // the other is a decoy. Both close to the centroid so the cluster
    // scan finds them.
    let vector_count = 5;
    let target_local = 2usize; // will be deleted
    let runner_up_local = 4usize; // must win once target is gone
    let mut raw_vectors = Vec::with_capacity(vector_count);
    for i in 0..vector_count {
        let scale = if i == target_local || i == runner_up_local {
            0.05
        } else {
            1.0
        };
        let v: Vec<f32> = raw_centroid
            .iter()
            .map(|&c| c + next_f32(&mut state) * scale)
            .collect();
        raw_vectors.push(v);
    }
    // Query sits between the target and the runner-up, but closer to the
    // target — target must win while present, runner-up once it's gone.
    let raw_query: Vec<f32> = raw_vectors[target_local]
        .iter()
        .zip(&raw_vectors[runner_up_local])
        .map(|(&t, &r)| t * 0.7 + r * 0.3)
        .collect();

    // 1. Real commit: write a (synthetic-content) segment object and
    // commit its SegmentRef through the real manifest CAS protocol.
    let store = InMemoryStore::new();
    let segment_committed = manifest::commit(&store, |next_row_id| {
        store
            .put_if_absent("segments/seg0.bin", b"placeholder")
            .unwrap();
        vec![SegmentRef {
            path: "segments/seg0.bin".to_string(),
            row_id_base: next_row_id,
            row_id_count: vector_count as u64,
            byte_length: 11,
            checksum: 0,
            deletion_vector: None,
        }]
    })
    .unwrap();
    let row_id_base = segment_committed.segments[0].row_id_base;

    // 2. Build the real cluster (quantize, posting list, navigation tier)
    // — the actual query-time data, independent of the manifest commit.
    let mut compact_codes = Vec::new();
    let mut f_add = Vec::with_capacity(vector_count);
    let mut f_rescale = Vec::with_capacity(vector_count);
    let mut f_error = Vec::with_capacity(vector_count);
    let mut row_ids = Vec::with_capacity(vector_count);
    for (i, raw_vector) in raw_vectors.iter().enumerate() {
        let rotated_vector = rotate_fht_kac(raw_vector, padded_dims, flip);
        let q = quantize_one_bit(&rotated_vector, &rotated_centroid, MetricType::L2);
        compact_codes.extend(q.compact_code);
        f_add.push(q.f_add);
        f_rescale.push(q.f_rescale);
        f_error.push(q.f_error);
        row_ids.push(row_id_base + i as u64);
    }
    let input = ClusterInput {
        compact_codes: &compact_codes,
        f_add: &f_add,
        f_rescale: &f_rescale,
        f_error: &f_error,
        row_ids: &row_ids,
        ex_region: None,
    };
    let (posting_bytes, dirs) = build_posting_lists(&[input], padded_dims);
    let nav_bytes = build_navigation_tier(&rotated_centroid, padded_dims, &dirs);
    let navigation = NavigationTierReader::new(&nav_bytes, padded_dims).unwrap();
    let posting_reader = PostingListReader::new(&posting_bytes);

    let rotated_query = rotate_fht_kac(&raw_query, padded_dims, flip);
    let query_factors = QueryFactors::new(&rotated_query, 1);
    let selected = select_nprobe_clusters(&navigation, &rotated_query, 1, MetricType::L2);

    // 3. Before any delete: the target must win.
    let before = scan_selected_clusters(
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
    assert_eq!(
        before[0].row_id,
        row_id_base + target_local as u64,
        "sanity: the target must win before any deletion"
    );

    // 4. Real delete: commit a real deletion-vector object through
    // `commit_deletion_vector`, tombstoning the target's local ordinal.
    let deletion_committed = manifest::commit_deletion_vector(&store, "segments/seg0.bin", |seg| {
        assert!(seg.deletion_vector.is_none());
        let mut bitmap = RoaringBitmap::new();
        bitmap.insert(target_local as u32);
        let bytes = deletion::build_deletion_vector(&bitmap, seg.row_id_count).unwrap();
        let checksum = deletion::checksum(&bytes);
        store.put_if_absent("deletions/seg0-0.bin", &bytes).unwrap();
        DeletionVectorRef {
            path: "deletions/seg0-0.bin".to_string(),
            byte_length: bytes.len() as u64,
            checksum,
        }
    })
    .unwrap();
    let dv_ref = deletion_committed.segments[0]
        .deletion_vector
        .as_ref()
        .unwrap();
    let dv = deletion::read(&store, dv_ref).unwrap();
    assert!(dv.is_deleted(row_id_base + target_local as u64, row_id_base));

    // 5. Re-read the manifest fresh (as a real reader would) and re-query
    // — the target must now be filtered out, and the runner-up must win.
    let fresh_snapshot = manifest::read_snapshot(&store).unwrap().unwrap();
    let fresh_seg = &fresh_snapshot.segments[0];
    let fresh_dv_ref = fresh_seg.deletion_vector.as_ref().unwrap();
    let fresh_dv = deletion::read(&store, fresh_dv_ref).unwrap();

    let after = scan_selected_clusters(
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
    let filtered = filter_deleted(after, fresh_seg.row_id_base, Some(&fresh_dv));

    assert!(
        filtered
            .iter()
            .all(|c| c.row_id != row_id_base + target_local as u64),
        "the deleted row must not appear in the final results"
    );
    assert_eq!(
        filtered[0].row_id,
        row_id_base + runner_up_local as u64,
        "the runner-up must now win"
    );
}
