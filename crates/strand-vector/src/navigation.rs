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

//! The cluster navigation tier blob for the vector family: full-precision
//! centroids plus a per-cluster directory into the posting-list blob.
//! Layout is normative per `spec/vectors.md` §3, approved by RFC 0010
//! (`rfcs/0010-vector-blob-cluster-family.md`).

/// Fixed size of one `cluster_dir` entry (`spec/vectors.md` §3).
pub const CLUSTER_DIR_ENTRY_LEN: usize = 24;

/// One cluster's directory entry: where its region lives in the
/// posting-list blob, and how many vectors it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClusterDirEntry {
    pub region_offset: u64,
    pub code_bytes_length: u64,
    pub vector_count: u32,
}

/// Builds a cluster navigation tier blob (`spec/vectors.md` §3).
/// `centroids` is `num_clusters * padded_dims` f32 values, row-major, in
/// the same cluster-index order as `cluster_dirs`.
///
/// # Panics
///
/// Panics if `centroids.len() != cluster_dirs.len() * padded_dims`.
pub fn build_navigation_tier(
    centroids: &[f32],
    padded_dims: usize,
    cluster_dirs: &[ClusterDirEntry],
) -> Vec<u8> {
    let num_clusters = cluster_dirs.len();
    assert_eq!(
        centroids.len(),
        num_clusters * padded_dims,
        "centroids must be exactly num_clusters*padded_dims f32 values"
    );
    let mut out =
        Vec::with_capacity(8 + centroids.len() * 4 + num_clusters * CLUSTER_DIR_ENTRY_LEN);
    out.extend_from_slice(&(num_clusters as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // reserved
    for &c in centroids {
        out.extend_from_slice(&c.to_le_bytes());
    }
    for entry in cluster_dirs {
        out.extend_from_slice(&entry.region_offset.to_le_bytes());
        out.extend_from_slice(&entry.code_bytes_length.to_le_bytes());
        out.extend_from_slice(&entry.vector_count.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // reserved
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationTierError {
    Truncated,
}

/// A resident cluster navigation tier blob (`spec/vectors.md` §3).
#[derive(Debug, Clone, Copy)]
pub struct NavigationTierReader<'a> {
    bytes: &'a [u8],
    padded_dims: usize,
    num_clusters: usize,
}

impl<'a> NavigationTierReader<'a> {
    pub fn new(bytes: &'a [u8], padded_dims: usize) -> Result<Self, NavigationTierError> {
        assert!(padded_dims > 0, "padded_dims must be non-zero");
        if bytes.len() < 8 {
            return Err(NavigationTierError::Truncated);
        }
        let num_clusters = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let expected_len =
            8 + num_clusters * padded_dims * 4 + num_clusters * CLUSTER_DIR_ENTRY_LEN;
        if bytes.len() != expected_len {
            return Err(NavigationTierError::Truncated);
        }
        Ok(NavigationTierReader {
            bytes,
            padded_dims,
            num_clusters,
        })
    }

    pub fn num_clusters(&self) -> usize {
        self.num_clusters
    }

    fn centroid_table_region(&self) -> &'a [u8] {
        let start = 8;
        let len = self.num_clusters * self.padded_dims * 4;
        &self.bytes[start..start + len]
    }

    fn cluster_dir_region(&self) -> &'a [u8] {
        let start = 8 + self.num_clusters * self.padded_dims * 4;
        &self.bytes[start..]
    }

    /// The `cluster_idx`-th centroid's `padded_dims` f32 components.
    ///
    /// # Panics
    ///
    /// Panics if `cluster_idx >= self.num_clusters()`.
    pub fn centroid(&self, cluster_idx: usize) -> Vec<f32> {
        assert!(cluster_idx < self.num_clusters, "cluster_idx out of range");
        let region = self.centroid_table_region();
        let start = cluster_idx * self.padded_dims * 4;
        (0..self.padded_dims)
            .map(|i| {
                let off = start + i * 4;
                f32::from_le_bytes(region[off..off + 4].try_into().unwrap())
            })
            .collect()
    }

    /// The `cluster_idx`-th cluster's directory entry.
    ///
    /// # Panics
    ///
    /// Panics if `cluster_idx >= self.num_clusters()`.
    pub fn cluster_dir(&self, cluster_idx: usize) -> ClusterDirEntry {
        assert!(cluster_idx < self.num_clusters, "cluster_idx out of range");
        let region = self.cluster_dir_region();
        let start = cluster_idx * CLUSTER_DIR_ENTRY_LEN;
        let entry = &region[start..start + CLUSTER_DIR_ENTRY_LEN];
        ClusterDirEntry {
            region_offset: u64::from_le_bytes(entry[0..8].try_into().unwrap()),
            code_bytes_length: u64::from_le_bytes(entry[8..16].try_into().unwrap()),
            vector_count: u32::from_le_bytes(entry[16..20].try_into().unwrap()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let padded_dims = 4;
        let centroids: Vec<f32> = vec![1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0];
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

        let reader = NavigationTierReader::new(&bytes, padded_dims).expect("valid blob");
        assert_eq!(reader.num_clusters(), 2);
        assert_eq!(reader.centroid(0), vec![1.0, 1.0, 1.0, 1.0]);
        assert_eq!(reader.centroid(1), vec![-1.0, -1.0, -1.0, -1.0]);
        assert_eq!(reader.cluster_dir(0), dirs[0]);
        assert_eq!(reader.cluster_dir(1), dirs[1]);
    }

    #[test]
    fn rejects_truncated_bytes() {
        assert_eq!(
            NavigationTierReader::new(&[0u8; 4], 4).unwrap_err(),
            NavigationTierError::Truncated
        );
    }
}
