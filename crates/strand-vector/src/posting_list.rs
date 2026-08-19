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

//! The cluster posting-list blob for the vector family: per-cluster
//! FastScan-batched 1-bit codes plus distance-correction factors,
//! immediately followed by that cluster's row-id array. Layout is
//! normative per `spec/vectors.md` §4, approved by RFC 0010
//! (`rfcs/0010-vector-blob-cluster-family.md`).
//!
//! Quantization itself (rotation application, sign-based bit selection,
//! and the `f_add`/`f_rescale`/`f_error` factor formulas) is RaBitQ's own
//! algorithm, out of this container-layer crate's scope (RFC 0010 Design
//! §4) — this module packs and unpacks already-quantized codes and
//! factors, however they were produced.

use crate::fastscan;
use crate::navigation::ClusterDirEntry;

const FACTORS_PER_BATCH_LEN: usize = 3 * fastscan::BATCH_SIZE * 4; // f_add, f_rescale, f_error

/// Byte length of one FastScan batch's code region plus its three factor
/// arrays (`BatchDataMap::data_bytes`, `spec/vectors.md` §4).
pub fn batch_data_len(padded_dims: usize) -> usize {
    padded_dims * fastscan::BATCH_SIZE / 8 + FACTORS_PER_BATCH_LEN
}

/// Byte length of a cluster's quantized-code region for `vector_count`
/// vectors at `padded_dims` (`spec/vectors.md` §4's `code_bytes_length`).
pub fn code_bytes_length_for(vector_count: usize, padded_dims: usize) -> usize {
    vector_count.div_ceil(fastscan::BATCH_SIZE) * batch_data_len(padded_dims)
}

/// One cluster's already-quantized input: `compact_codes` is
/// `row_ids.len() * (padded_dims/8)` bytes of plain, sequential
/// one-bit-per-dimension codes (row-major); `f_add`/`f_rescale`/`f_error`
/// and `row_ids` are each `row_ids.len()` long. `row_ids` MUST be strictly
/// ascending (`spec/vectors.md` §4).
pub struct ClusterInput<'a> {
    pub compact_codes: &'a [u8],
    pub f_add: &'a [f32],
    pub f_rescale: &'a [f32],
    pub f_error: &'a [f32],
    pub row_ids: &'a [u64],
}

fn build_code_region(input: &ClusterInput, padded_dims: usize) -> Vec<u8> {
    let cols = padded_dims / 8;
    let num = input.row_ids.len();
    let num_batches = num.div_ceil(fastscan::BATCH_SIZE);
    let mut out = Vec::with_capacity(num_batches * batch_data_len(padded_dims));
    for b in 0..num_batches {
        let start = b * fastscan::BATCH_SIZE;
        let real = (num - start).min(fastscan::BATCH_SIZE);

        let mut batch_compact = vec![0u8; fastscan::BATCH_SIZE * cols];
        batch_compact[..real * cols]
            .copy_from_slice(&input.compact_codes[start * cols..start * cols + real * cols]);
        out.extend(fastscan::pack_batch(&batch_compact, cols));

        for factors in [input.f_add, input.f_rescale, input.f_error] {
            let mut lane = vec![0f32; fastscan::BATCH_SIZE];
            lane[..real].copy_from_slice(&factors[start..start + real]);
            for v in &lane {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
    }
    out
}

/// Builds the cluster posting-list blob (`spec/vectors.md` §4) for all of a
/// field's clusters, in the given order — which MUST match the navigation
/// tier's own `cluster_dir` order. Returns the blob bytes and the
/// `ClusterDirEntry` list the navigation tier's `cluster_dir` region needs
/// (`crate::navigation::build_navigation_tier`).
///
/// # Panics
///
/// Panics if `padded_dims` is not a positive multiple of 64 (the
/// registered FastScan codec's own requirement — Design §4's
/// `one_bit_batch_code` — not merely a STRAND alignment convenience,
/// `references/rabitq-library-fastscan-pack-codes-source.md`), or if any
/// cluster's `compact_codes`/`f_add`/`f_rescale`/`f_error` lengths don't
/// match its `row_ids` length, or if `row_ids` is not strictly ascending.
pub fn build_posting_lists(
    clusters: &[ClusterInput],
    padded_dims: usize,
) -> (Vec<u8>, Vec<ClusterDirEntry>) {
    assert!(
        padded_dims > 0 && padded_dims.is_multiple_of(64),
        "padded_dims must be a positive multiple of 64"
    );
    let cols = padded_dims / 8;
    let mut out = Vec::new();
    let mut dirs = Vec::with_capacity(clusters.len());
    for input in clusters {
        let vector_count = input.row_ids.len();
        assert_eq!(
            input.compact_codes.len(),
            vector_count * cols,
            "compact_codes length must match row_ids.len()*cols"
        );
        assert_eq!(
            input.f_add.len(),
            vector_count,
            "f_add length must match row_ids.len()"
        );
        assert_eq!(
            input.f_rescale.len(),
            vector_count,
            "f_rescale length must match row_ids.len()"
        );
        assert_eq!(
            input.f_error.len(),
            vector_count,
            "f_error length must match row_ids.len()"
        );
        assert!(
            input
                .f_add
                .iter()
                .chain(input.f_rescale)
                .chain(input.f_error)
                .all(|v| v.is_finite()),
            "distance-correction factors must all be finite (NaN/inf would silently corrupt distance estimates for every reader of this cluster)"
        );
        assert!(
            input.row_ids.windows(2).all(|w| w[0] < w[1]),
            "row_ids must be strictly ascending"
        );

        let region_offset = out.len() as u64;
        let code_region = build_code_region(input, padded_dims);
        let code_bytes_length = code_region.len() as u64;
        out.extend(code_region);
        for &rid in input.row_ids {
            out.extend_from_slice(&rid.to_le_bytes());
        }
        dirs.push(ClusterDirEntry {
            region_offset,
            code_bytes_length,
            vector_count: vector_count as u32,
        });
    }
    (out, dirs)
}

/// One cluster's decoded content, recovered from its region of the
/// posting-list blob.
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterRegion {
    pub compact_codes: Vec<u8>,
    pub f_add: Vec<f32>,
    pub f_rescale: Vec<f32>,
    pub f_error: Vec<f32>,
    pub row_ids: Vec<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostingListError {
    Truncated,
    CodeBytesLengthMismatch { declared: u64, computed: u64 },
}

/// A resident cluster posting-list blob (`spec/vectors.md` §4).
#[derive(Debug, Clone, Copy)]
pub struct PostingListReader<'a> {
    bytes: &'a [u8],
}

impl<'a> PostingListReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        PostingListReader { bytes }
    }

    /// Reads and fully decodes one cluster's region, given its directory
    /// entry (from `NavigationTierReader::cluster_dir`) and the field's
    /// `padded_dims`.
    pub fn read_cluster(
        &self,
        dir: &ClusterDirEntry,
        padded_dims: usize,
    ) -> Result<ClusterRegion, PostingListError> {
        let cols = padded_dims / 8;
        let vector_count = dir.vector_count as usize;
        let expected_code_bytes = code_bytes_length_for(vector_count, padded_dims) as u64;
        if dir.code_bytes_length != expected_code_bytes {
            return Err(PostingListError::CodeBytesLengthMismatch {
                declared: dir.code_bytes_length,
                computed: expected_code_bytes,
            });
        }
        let region_start = dir.region_offset as usize;
        let code_len = dir.code_bytes_length as usize;
        let row_id_len = vector_count * 8;
        let region_end = region_start + code_len + row_id_len;
        if region_end > self.bytes.len() {
            return Err(PostingListError::Truncated);
        }
        let code_bytes = &self.bytes[region_start..region_start + code_len];
        let row_id_bytes = &self.bytes[region_start + code_len..region_end];

        let batch_len = batch_data_len(padded_dims);
        let num_batches = vector_count.div_ceil(fastscan::BATCH_SIZE);
        let mut packed_codes_only = Vec::with_capacity(num_batches * fastscan::BATCH_SIZE * cols);
        let mut f_add = Vec::with_capacity(vector_count);
        let mut f_rescale = Vec::with_capacity(vector_count);
        let mut f_error = Vec::with_capacity(vector_count);
        for b in 0..num_batches {
            let start = b * batch_len;
            let code_part = &code_bytes[start..start + fastscan::BATCH_SIZE * cols];
            packed_codes_only.extend_from_slice(code_part);

            let real = (vector_count - b * fastscan::BATCH_SIZE).min(fastscan::BATCH_SIZE);
            let factors_start = start + fastscan::BATCH_SIZE * cols;
            let read_lane = |offset: usize, out: &mut Vec<f32>| {
                let lane = &code_bytes
                    [factors_start + offset..factors_start + offset + fastscan::BATCH_SIZE * 4];
                for i in 0..real {
                    out.push(f32::from_le_bytes(
                        lane[i * 4..i * 4 + 4].try_into().unwrap(),
                    ));
                }
            };
            read_lane(0, &mut f_add);
            read_lane(fastscan::BATCH_SIZE * 4, &mut f_rescale);
            read_lane(2 * fastscan::BATCH_SIZE * 4, &mut f_error);
        }
        let compact_codes = fastscan::unpack_codes(&packed_codes_only, vector_count, cols);
        let row_ids = (0..vector_count)
            .map(|i| u64::from_le_bytes(row_id_bytes[i * 8..i * 8 + 8].try_into().unwrap()))
            .collect();

        Ok(ClusterRegion {
            compact_codes,
            f_add,
            f_rescale,
            f_error,
            row_ids,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_cluster(
        padded_dims: usize,
        row_ids: Vec<u64>,
    ) -> (Vec<u8>, Vec<f32>, Vec<f32>, Vec<f32>) {
        let cols = padded_dims / 8;
        let n = row_ids.len();
        let compact_codes: Vec<u8> = (0..(n * cols)).map(|i| (i * 17 + 3) as u8).collect();
        let f_add: Vec<f32> = (0..n).map(|i| i as f32 * 0.5).collect();
        let f_rescale: Vec<f32> = (0..n).map(|i| 1.0 + i as f32).collect();
        let f_error: Vec<f32> = (0..n).map(|i| -(i as f32)).collect();
        (compact_codes, f_add, f_rescale, f_error)
    }

    #[test]
    fn round_trips_a_single_cluster_within_one_batch() {
        let padded_dims = 64;
        let row_ids = vec![100u64, 101, 102];
        let (compact_codes, f_add, f_rescale, f_error) =
            synthetic_cluster(padded_dims, row_ids.clone());

        let input = ClusterInput {
            compact_codes: &compact_codes,
            f_add: &f_add,
            f_rescale: &f_rescale,
            f_error: &f_error,
            row_ids: &row_ids,
        };
        let (blob, dirs) = build_posting_lists(&[input], padded_dims);

        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].region_offset, 0);
        assert_eq!(dirs[0].code_bytes_length, 640); // 64*4 + 384, RFC 0010's own worked example figure
        assert_eq!(dirs[0].vector_count, 3);
        assert_eq!(blob.len(), 640 + 3 * 8);

        let reader = PostingListReader::new(&blob);
        let region = reader
            .read_cluster(&dirs[0], padded_dims)
            .expect("valid cluster region");
        assert_eq!(region.compact_codes, compact_codes);
        assert_eq!(region.f_add, f_add);
        assert_eq!(region.f_rescale, f_rescale);
        assert_eq!(region.f_error, f_error);
        assert_eq!(region.row_ids, row_ids);
    }

    #[test]
    fn matches_rfc_0010_worked_example_row_id_bytes() {
        // RFC 0010's own worked example: dims=padded_dims=64, cluster 0 has
        // row-ids 100,101,102 at blob offset 640 (24 bytes); cluster 1 has
        // row-ids 200,201 at blob offset 1304 (16 bytes); total blob 1320
        // bytes. This test reproduces that exact layout with synthetic
        // (non-opaque) code content, since the RFC's own worked example
        // deliberately leaves the code payload opaque.
        let padded_dims = 64;
        let (c0_codes, c0_add, c0_rescale, c0_error) =
            synthetic_cluster(padded_dims, vec![100, 101, 102]);
        let (c1_codes, c1_add, c1_rescale, c1_error) =
            synthetic_cluster(padded_dims, vec![200, 201]);
        let c0_ids = vec![100u64, 101, 102];
        let c1_ids = vec![200u64, 201];
        let clusters = [
            ClusterInput {
                compact_codes: &c0_codes,
                f_add: &c0_add,
                f_rescale: &c0_rescale,
                f_error: &c0_error,
                row_ids: &c0_ids,
            },
            ClusterInput {
                compact_codes: &c1_codes,
                f_add: &c1_add,
                f_rescale: &c1_rescale,
                f_error: &c1_error,
                row_ids: &c1_ids,
            },
        ];
        let (blob, dirs) = build_posting_lists(&clusters, padded_dims);

        assert_eq!(blob.len(), 1320);
        assert_eq!(dirs[0].region_offset, 0);
        assert_eq!(dirs[1].region_offset, 664);
        assert_eq!(dirs[0].code_bytes_length, 640);
        assert_eq!(dirs[1].code_bytes_length, 640);

        // Row-id bytes, byte-for-byte against the RFC's own worked example.
        let expected_c0_ids = [
            0x64, 0, 0, 0, 0, 0, 0, 0, 0x65, 0, 0, 0, 0, 0, 0, 0, 0x66, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(&blob[640..664], &expected_c0_ids[..]);
        let expected_c1_ids = [0xC8, 0, 0, 0, 0, 0, 0, 0, 0xC9, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(&blob[1304..1320], &expected_c1_ids[..]);
    }

    #[test]
    fn round_trips_a_cluster_spanning_multiple_batches() {
        let padded_dims = 64;
        let row_ids: Vec<u64> = (0..70).collect(); // spans 3 batches (32+32+6)
        let (compact_codes, f_add, f_rescale, f_error) =
            synthetic_cluster(padded_dims, row_ids.clone());
        let input = ClusterInput {
            compact_codes: &compact_codes,
            f_add: &f_add,
            f_rescale: &f_rescale,
            f_error: &f_error,
            row_ids: &row_ids,
        };
        let (blob, dirs) = build_posting_lists(&[input], padded_dims);

        let reader = PostingListReader::new(&blob);
        let region = reader
            .read_cluster(&dirs[0], padded_dims)
            .expect("valid cluster region");
        assert_eq!(region.compact_codes, compact_codes);
        assert_eq!(region.f_add, f_add);
        assert_eq!(region.f_rescale, f_rescale);
        assert_eq!(region.f_error, f_error);
        assert_eq!(region.row_ids, row_ids);
    }

    #[test]
    #[should_panic(expected = "row_ids must be strictly ascending")]
    fn rejects_non_ascending_row_ids() {
        let padded_dims = 64;
        let row_ids = vec![5u64, 3];
        let (compact_codes, f_add, f_rescale, f_error) =
            synthetic_cluster(padded_dims, row_ids.clone());
        let input = ClusterInput {
            compact_codes: &compact_codes,
            f_add: &f_add,
            f_rescale: &f_rescale,
            f_error: &f_error,
            row_ids: &row_ids,
        };
        build_posting_lists(&[input], padded_dims);
    }

    #[test]
    #[should_panic(expected = "distance-correction factors must all be finite")]
    fn rejects_non_finite_factors() {
        let padded_dims = 64;
        let row_ids = vec![1u64, 2, 3];
        let (compact_codes, _, f_rescale, f_error) =
            synthetic_cluster(padded_dims, row_ids.clone());
        let f_add = vec![1.0f32, f32::NAN, 3.0];
        let input = ClusterInput {
            compact_codes: &compact_codes,
            f_add: &f_add,
            f_rescale: &f_rescale,
            f_error: &f_error,
            row_ids: &row_ids,
        };
        build_posting_lists(&[input], padded_dims);
    }
}
