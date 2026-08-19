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

//! Reproduces RFC 0010's worked example (dims=64, `FhtKacRotator`, 2
//! clusters, 5 vectors total) and checks it against the pinned conformance
//! golden files.

use strand_vector::descriptor::{
    DescriptorReader, DistanceMetric, RotatorType, build_fht_kac_with_payload,
};
use strand_vector::navigation::{ClusterDirEntry, NavigationTierReader, build_navigation_tier};
use strand_vector::posting_list::{ClusterInput, batch_data_len, build_posting_lists};

fn golden(name: &str) -> Vec<u8> {
    std::fs::read(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../conformance/vectors/").to_owned() + name,
    )
    .unwrap_or_else(|e| panic!("golden file conformance/vectors/{name} must exist: {e}"))
}

#[test]
fn descriptor_matches_conformance_golden_file() {
    // RFC 0010's own illustrative, exactly-reproducible rotation payload:
    // the ascending byte sequence 0x00..0x1F, not real randomness.
    let payload: Vec<u8> = (0u8..32).collect();
    let bytes = build_fht_kac_with_payload(64, DistanceMetric::L2, 1, &payload);

    assert_eq!(
        bytes,
        golden("toy-descriptor.bin"),
        "must match RFC 0010's pinned 48 bytes exactly"
    );

    let reader = DescriptorReader::new(&bytes).expect("valid descriptor");
    assert_eq!(reader.dims(), 64);
    assert_eq!(reader.padded_dims(), 64);
    assert_eq!(reader.distance_metric(), DistanceMetric::L2);
    assert_eq!(reader.bit_width(), 1);
    assert_eq!(reader.rotator_type(), RotatorType::FhtKac);
    assert_eq!(reader.rotation_payload(), &payload[..]);
}

#[test]
fn navigation_tier_matches_conformance_golden_file() {
    let padded_dims = 64;
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
    let bytes = build_navigation_tier(&centroids, padded_dims, &dirs);

    assert_eq!(
        bytes,
        golden("toy-navigation-tier.bin"),
        "must match RFC 0010's pinned 568 bytes exactly"
    );

    let reader = NavigationTierReader::new(&bytes, padded_dims).expect("valid navigation tier");
    assert_eq!(reader.num_clusters(), 2);
    assert_eq!(reader.centroid(0), centroid0);
    assert_eq!(reader.centroid(1), centroid1);
    assert_eq!(reader.cluster_dir(0), dirs[0]);
    assert_eq!(reader.cluster_dir(1), dirs[1]);
}

#[test]
fn posting_list_directory_and_row_ids_match_the_worked_example() {
    // The code payload itself is opaque in RFC 0010's own worked example
    // (real RaBitQ quantization is out of this crate's scope, RFC 0010
    // Design §4) — this test confirms every byte the RFC *does* pin: the
    // directory offsets, the code region's byte length, and the row-id
    // arrays, using synthetic (not real) code content.
    let padded_dims = 64;
    assert_eq!(
        batch_data_len(padded_dims),
        640,
        "RFC 0010's own worked-example figure"
    );

    let cols = padded_dims / 8;
    let c0_codes = vec![0u8; 3 * cols];
    let c0_factors = vec![0f32; 3];
    let c1_codes = vec![0u8; 2 * cols];
    let c1_factors = vec![0f32; 2];
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
    let (blob, dirs) = build_posting_lists(&clusters, padded_dims);

    assert_eq!(
        blob.len(),
        1320,
        "RFC 0010's own total worked-example blob size"
    );
    assert_eq!(
        dirs[0],
        ClusterDirEntry {
            region_offset: 0,
            code_bytes_length: 640,
            vector_count: 3
        }
    );
    assert_eq!(
        dirs[1],
        ClusterDirEntry {
            region_offset: 664,
            code_bytes_length: 640,
            vector_count: 2
        }
    );

    // Row-id bytes, exactly as RFC 0010's worked example states them.
    assert_eq!(
        &blob[640..664],
        &[
            0x64, 0, 0, 0, 0, 0, 0, 0, 0x65, 0, 0, 0, 0, 0, 0, 0, 0x66, 0, 0, 0, 0, 0, 0, 0
        ][..]
    );
    assert_eq!(
        &blob[1304..1320],
        &[0xC8, 0, 0, 0, 0, 0, 0, 0, 0xC9, 0, 0, 0, 0, 0, 0, 0][..]
    );
}
