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

//! The `nprobe`-bounded query pipeline (`crate::query`), exercised
//! against a real, from-scratch clustered index — completing the story
//! `build_a_real_index.rs` started by exhaustively scanning every
//! cluster. Two things this file proves that no earlier test could: a
//! genuinely *bounded* query (small `nprobe`, not every cluster) still
//! finds the right answer when the query is reasonably close to its own
//! cluster's centroid, and recall is monotonically non-decreasing as
//! `nprobe` grows — the property the whole `nprobe` knob exists to trade
//! off (more clusters scanned, more I/O, never worse recall).

use rand::SeedableRng;
use rand::rngs::StdRng;
use strand_vector::descriptor::{self, DescriptorReader, DistanceMetric};
use strand_vector::estimate::QueryFactors;
use strand_vector::kmeans::{kmeans, recommended_cluster_count};
use strand_vector::navigation::{NavigationTierReader, build_navigation_tier};
use strand_vector::posting_list::{ClusterInput, PostingListReader, build_posting_lists};
use strand_vector::quantize::{MetricType, quantize_one_bit};
use strand_vector::query::{scan_selected_clusters, select_nprobe_clusters};
use strand_vector::rotate::rotate_fht_kac;

fn next_f32(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    ((*state % 2001) as f32 - 1000.0) / 100.0
}

struct BuiltIndex {
    navigation_bytes: Vec<u8>,
    posting_list_bytes: Vec<u8>,
    padded_dims: usize,
    flip: Vec<u8>,
    num_clusters: usize,
    raw_vectors: Vec<Vec<f32>>,
}

fn build_real_index(
    dims: u32,
    vector_count: usize,
    raw_vectors: Vec<Vec<f32>>,
    seed: u64,
) -> BuiltIndex {
    let padded_dims = descriptor::padded_dims_for(dims) as usize;
    let num_clusters = recommended_cluster_count(vector_count)
        .min(vector_count / 2)
        .max(2);
    let flat_raw: Vec<f32> = raw_vectors.iter().flatten().copied().collect();

    let mut rng = StdRng::seed_from_u64(seed);
    let clustering = kmeans(
        &flat_raw,
        vector_count,
        dims as usize,
        num_clusters,
        50,
        &mut rng,
    );

    let mut clusters: Vec<Vec<usize>> = vec![Vec::new(); num_clusters];
    for (i, &c) in clustering.assignments.iter().enumerate() {
        clusters[c].push(i);
    }

    let mut rng2 = StdRng::seed_from_u64(seed + 1);
    let descriptor_bytes = descriptor::build_fht_kac(dims, DistanceMetric::L2, 1, &mut rng2);
    let flip = DescriptorReader::new(&descriptor_bytes)
        .unwrap()
        .rotation_payload()
        .to_vec();

    let rotated_centroids: Vec<Vec<f32>> = (0..num_clusters)
        .map(|c| {
            rotate_fht_kac(
                &clustering.centroids[c * dims as usize..(c + 1) * dims as usize],
                padded_dims,
                &flip,
            )
        })
        .collect();

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
            let rotated_vector = rotate_fht_kac(&raw_vectors[i], padded_dims, &flip);
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
            ex_region: None,
        })
        .collect();
    let (posting_list_bytes, dirs) = build_posting_lists(&cluster_inputs, padded_dims);

    let centroid_table: Vec<f32> = rotated_centroids.iter().flatten().copied().collect();
    let navigation_bytes = build_navigation_tier(&centroid_table, padded_dims, &dirs);

    BuiltIndex {
        navigation_bytes,
        posting_list_bytes,
        padded_dims,
        flip,
        num_clusters,
        raw_vectors,
    }
}

#[test]
fn a_small_bounded_nprobe_finds_the_nearest_neighbor_when_the_query_is_near_its_own_cluster() {
    let dims = 32u32;
    let vector_count = 300;
    let mut state = 0xC0DE_u64;
    let blob_centers: Vec<Vec<f32>> = (0..6)
        .map(|b| {
            (0..dims)
                .map(|d| ((b * 11 + d) as f32).sin() * 50.0)
                .collect()
        })
        .collect();
    let mut raw_vectors: Vec<Vec<f32>> = Vec::with_capacity(vector_count);
    for i in 0..vector_count {
        let center = &blob_centers[i % 6];
        raw_vectors.push(
            center
                .iter()
                .map(|&c| c + next_f32(&mut state) * 0.3)
                .collect(),
        );
    }
    let target_index = 77usize;
    let raw_query: Vec<f32> = raw_vectors[target_index]
        .iter()
        .map(|&v| v + next_f32(&mut state) * 0.03)
        .collect();

    let index = build_real_index(dims, vector_count, raw_vectors, 500);

    let navigation = NavigationTierReader::new(&index.navigation_bytes, index.padded_dims).unwrap();
    let posting_reader = PostingListReader::new(&index.posting_list_bytes);
    let rotated_query = rotate_fht_kac(&raw_query, index.padded_dims, &index.flip);
    let query_factors = QueryFactors::new(&rotated_query, 1);

    // nprobe well below num_clusters: a genuinely bounded query.
    let nprobe = 3.min(index.num_clusters);
    let selected = select_nprobe_clusters(&navigation, &rotated_query, nprobe, MetricType::L2);
    assert!(selected.len() <= nprobe);

    let candidates = scan_selected_clusters(
        &navigation,
        &posting_reader,
        &selected,
        &rotated_query,
        &query_factors,
        MetricType::L2,
        index.padded_dims,
        0,
    )
    .unwrap();
    assert!(!candidates.is_empty());
    assert_eq!(
        candidates[0].row_id, target_index as u64,
        "bounded nprobe must still find the deliberately-nearest vector"
    );
}

#[test]
fn nprobe_covering_every_cluster_matches_an_exhaustive_scan() {
    let dims = 16u32;
    let vector_count = 120;
    let mut state = 0xFACE_u64;
    let raw_vectors: Vec<Vec<f32>> = (0..vector_count)
        .map(|_| (0..dims).map(|_| next_f32(&mut state)).collect())
        .collect();
    let index = build_real_index(dims, vector_count, raw_vectors, 700);

    let navigation = NavigationTierReader::new(&index.navigation_bytes, index.padded_dims).unwrap();
    let posting_reader = PostingListReader::new(&index.posting_list_bytes);
    let rotated_query = rotate_fht_kac(&index.raw_vectors[0], index.padded_dims, &index.flip);
    let query_factors = QueryFactors::new(&rotated_query, 1);

    let all_clusters: Vec<usize> = (0..index.num_clusters).collect();
    let exhaustive = scan_selected_clusters(
        &navigation,
        &posting_reader,
        &all_clusters,
        &rotated_query,
        &query_factors,
        MetricType::L2,
        index.padded_dims,
        0,
    )
    .unwrap();

    let selected_all = select_nprobe_clusters(
        &navigation,
        &rotated_query,
        index.num_clusters,
        MetricType::L2,
    );
    let via_nprobe = scan_selected_clusters(
        &navigation,
        &posting_reader,
        &selected_all,
        &rotated_query,
        &query_factors,
        MetricType::L2,
        index.padded_dims,
        0,
    )
    .unwrap();

    assert_eq!(exhaustive.len(), via_nprobe.len());
    let mut exhaustive_ids: Vec<u64> = exhaustive.iter().map(|c| c.row_id).collect();
    let mut nprobe_ids: Vec<u64> = via_nprobe.iter().map(|c| c.row_id).collect();
    exhaustive_ids.sort_unstable();
    nprobe_ids.sort_unstable();
    assert_eq!(
        exhaustive_ids, nprobe_ids,
        "nprobe = num_clusters must scan exactly the same candidate set as manual exhaustive scan"
    );
}

#[test]
fn recall_at_the_true_nearest_neighbor_is_monotonically_non_decreasing_in_nprobe() {
    let dims = 24u32;
    let vector_count = 400;
    let mut state = 0xBEEF_u64;
    let blob_centers: Vec<Vec<f32>> = (0..10)
        .map(|b| {
            (0..dims)
                .map(|d| ((b * 13 + d) as f32).cos() * 60.0)
                .collect()
        })
        .collect();
    let mut raw_vectors = Vec::with_capacity(vector_count);
    for i in 0..vector_count {
        let center = &blob_centers[i % 10];
        raw_vectors.push(
            center
                .iter()
                .map(|&c| c + next_f32(&mut state) * 0.4)
                .collect(),
        );
    }

    let index = build_real_index(dims, vector_count, raw_vectors.clone(), 900);
    let navigation = NavigationTierReader::new(&index.navigation_bytes, index.padded_dims).unwrap();
    let posting_reader = PostingListReader::new(&index.posting_list_bytes);

    // For each of several query points, find the true brute-force nearest
    // neighbor, then confirm: once nprobe is large enough to find it, no
    // larger nprobe ever loses it again (recall for this one query is a
    // step function 0 -> 1, never 1 -> 0).
    for &qi in &[10usize, 123, 250, 399] {
        let raw_query: Vec<f32> = raw_vectors[qi]
            .iter()
            .map(|&v| v + next_f32(&mut state) * 0.02)
            .collect();
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
        let true_nearest = exact[0].0 as u64;

        let rotated_query = rotate_fht_kac(&raw_query, index.padded_dims, &index.flip);
        let query_factors = QueryFactors::new(&rotated_query, 1);

        let mut found_once = false;
        for nprobe in 1..=index.num_clusters {
            let selected =
                select_nprobe_clusters(&navigation, &rotated_query, nprobe, MetricType::L2);
            let candidates = scan_selected_clusters(
                &navigation,
                &posting_reader,
                &selected,
                &rotated_query,
                &query_factors,
                MetricType::L2,
                index.padded_dims,
                0,
            )
            .unwrap();
            let found = candidates.iter().any(|c| c.row_id == true_nearest);
            if found_once {
                assert!(
                    found,
                    "query {qi}: true nearest neighbor was found at a smaller nprobe but lost at nprobe={nprobe}"
                );
            }
            found_once |= found;
        }
        assert!(
            found_once,
            "query {qi}: true nearest neighbor was never found even at nprobe=num_clusters"
        );
    }
}
