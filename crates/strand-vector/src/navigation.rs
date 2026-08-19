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
//! centroids plus a per-cluster directory into the posting-list blob,
//! followed by a fixed-size closure-replication descriptor trailer.
//! Layout is normative per `spec/vectors.md` §3, approved by RFC 0010
//! (`rfcs/0010-vector-blob-cluster-family.md`) and extended by its
//! Discussion — post-approval amendment (M2-1, `docs/roadmap.md`) to
//! register the replication descriptor trailer.

/// Fixed size of one `cluster_dir` entry (`spec/vectors.md` §3).
pub const CLUSTER_DIR_ENTRY_LEN: usize = 24;

/// Fixed size of the closure-replication descriptor trailer appended after
/// `cluster_dir` (`spec/vectors.md` §3, M2-1). Always present, regardless
/// of whether closure replication was actually used — a field with no
/// replication has `policy = None`, `max_replicas = 0`, `epsilon = 0.0`,
/// which is exactly what an all-zero trailer decodes to.
pub const REPLICATION_DESCRIPTOR_LEN: usize = 8;

/// The construction-time closure-replication policy actually used for a
/// field's cluster assignments, recorded so a reader can know the real
/// replication knobs a segment was built with instead of assuming a
/// provisional constant (`rfcs/0010-vector-blob-cluster-family.md`
/// Discussion — post-approval amendment, M2-1; `crate::closure`).
///
/// This deliberately does **not** carry a realized replication *factor*
/// (e.g. total assignments / distinct vectors): that statistic is already
/// exactly recoverable from data a cold-open reader has in hand with no
/// new bytes — sum `vector_count` across every `cluster_dir` entry (already
/// fetched wholesale) and divide by the segment's own `Hotcache::
/// row_id_count` (already decoded before this blob family is even
/// touched, since the flat-vector blob's own dense-per-row-id contract,
/// `spec/vectors.md` §5, means this field's distinct vector count *is*
/// the segment's row-id count). Storing it again would be redundant wire
/// bytes for a quantity invariant 8's "novelty budget" discipline argues
/// against paying for (RFC 0010 Discussion — post-approval amendment,
/// "how this could be wrong").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplicationPolicy {
    /// No closure replication: every vector was assigned to exactly its
    /// one primary (nearest) cluster. The historical, pre-M2-1 behavior,
    /// and the all-zero-trailer default.
    None = 0,
    /// SPANN-style closure assignment (`references/
    /// spann-closure-assignment-algorithm.md`, `crate::closure`).
    SpannClosure = 1,
}

impl ReplicationPolicy {
    /// Unrecognized policy values are **not** rejected — query resolution's
    /// existing row-id deduplication (`spec/vectors.md` §6 step 3) already
    /// tolerates duplicate row-ids across scanned clusters regardless of
    /// *why* they're duplicated, so a reader that doesn't recognize a
    /// future policy value still decodes and queries this blob correctly;
    /// it just can't explain the segment's replication cost in that
    /// policy's own terms. Returns `None` (the enum variant) only for the
    /// one input this format guarantees is the "no replication" case (`0`);
    /// any other unrecognized byte is surfaced to the caller as `Err` with
    /// the raw byte, distinct from a real registered policy.
    fn from_u8(v: u8) -> Result<Option<Self>, u8> {
        match v {
            0 => Ok(Some(ReplicationPolicy::None)),
            1 => Ok(Some(ReplicationPolicy::SpannClosure)),
            other => Err(other),
        }
    }
}

/// The closure-replication descriptor trailer's decoded content (`spec/
/// vectors.md` §3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReplicationDescriptor {
    /// `None` for a recognized-but-unregistered-here future policy byte
    /// (forward compatibility, [`ReplicationPolicy::from_u8`]); `Some` for
    /// a policy this chapter registers.
    pub policy_raw: u8,
    pub policy: Option<ReplicationPolicy>,
    /// SPANN's own `ReplicaCount` actually used at construction time — the
    /// total number of posting lists a vector may land in, primary
    /// included (`references/spann-closure-assignment-algorithm.md`). `0`
    /// when `policy_raw == 0`.
    pub max_replicas: u8,
    /// SPANN's own ε₁ actually used (`references/
    /// spann-closure-assignment-algorithm.md`, Eq. 2). `0.0` when
    /// `policy_raw == 0`.
    pub epsilon: f32,
}

impl ReplicationDescriptor {
    /// The "no closure replication" descriptor — an all-zero trailer.
    /// Every navigation tier built before M2-1 is, byte-for-byte, what
    /// this produces (had it carried a trailer at all).
    pub fn none() -> Self {
        ReplicationDescriptor {
            policy_raw: 0,
            policy: Some(ReplicationPolicy::None),
            max_replicas: 0,
            epsilon: 0.0,
        }
    }

    /// A real SPANN-closure descriptor.
    ///
    /// # Panics
    ///
    /// Panics if `max_replicas == 0` (a closure policy with a zero replica
    /// cap is a contradiction — use [`ReplicationDescriptor::none`]
    /// instead) or if `epsilon` is negative or non-finite.
    pub fn spann_closure(max_replicas: u8, epsilon: f32) -> Self {
        assert!(
            max_replicas > 0,
            "max_replicas must be nonzero under SpannClosure; use ReplicationDescriptor::none() for no replication"
        );
        assert!(
            epsilon.is_finite() && epsilon >= 0.0,
            "epsilon must be finite and non-negative"
        );
        ReplicationDescriptor {
            policy_raw: ReplicationPolicy::SpannClosure as u8,
            policy: Some(ReplicationPolicy::SpannClosure),
            max_replicas,
            epsilon,
        }
    }

    fn encode(&self) -> [u8; REPLICATION_DESCRIPTOR_LEN] {
        let mut out = [0u8; REPLICATION_DESCRIPTOR_LEN];
        out[0] = self.policy_raw;
        out[1] = self.max_replicas;
        // out[2..4] reserved, left zero.
        out[4..8].copy_from_slice(&self.epsilon.to_le_bytes());
        out
    }

    fn decode(bytes: &[u8]) -> Self {
        debug_assert_eq!(bytes.len(), REPLICATION_DESCRIPTOR_LEN);
        let policy_raw = bytes[0];
        let max_replicas = bytes[1];
        let epsilon = f32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let policy = ReplicationPolicy::from_u8(policy_raw).unwrap_or_default();
        ReplicationDescriptor {
            policy_raw,
            policy,
            max_replicas,
            epsilon,
        }
    }
}

/// One cluster's directory entry: where its region lives in the
/// posting-list blob, and how many vectors it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClusterDirEntry {
    pub region_offset: u64,
    pub code_bytes_length: u64,
    pub vector_count: u32,
}

/// Builds a cluster navigation tier blob (`spec/vectors.md` §3) with no
/// closure replication (`ReplicationDescriptor::none()`) — the common case,
/// and the only case that existed before M2-1. `centroids` is
/// `num_clusters * padded_dims` f32 values, row-major, in the same
/// cluster-index order as `cluster_dirs`.
///
/// # Panics
///
/// Panics if `centroids.len() != cluster_dirs.len() * padded_dims`.
pub fn build_navigation_tier(
    centroids: &[f32],
    padded_dims: usize,
    cluster_dirs: &[ClusterDirEntry],
) -> Vec<u8> {
    build_navigation_tier_with_replication(
        centroids,
        padded_dims,
        cluster_dirs,
        ReplicationDescriptor::none(),
    )
}

/// Builds a cluster navigation tier blob (`spec/vectors.md` §3), recording
/// `replication` in the trailer (M2-1) — the real closure-replication
/// policy and knobs actually used to produce `cluster_dirs`'s (possibly
/// duplicate-row-id-carrying) assignments, so a reader can know the real
/// replication cost of this segment. `centroids` is `num_clusters *
/// padded_dims` f32 values, row-major, in the same cluster-index order as
/// `cluster_dirs`.
///
/// # Panics
///
/// Panics if `centroids.len() != cluster_dirs.len() * padded_dims`.
pub fn build_navigation_tier_with_replication(
    centroids: &[f32],
    padded_dims: usize,
    cluster_dirs: &[ClusterDirEntry],
    replication: ReplicationDescriptor,
) -> Vec<u8> {
    let num_clusters = cluster_dirs.len();
    assert_eq!(
        centroids.len(),
        num_clusters * padded_dims,
        "centroids must be exactly num_clusters*padded_dims f32 values"
    );
    let mut out = Vec::with_capacity(
        8 + centroids.len() * 4
            + num_clusters * CLUSTER_DIR_ENTRY_LEN
            + REPLICATION_DESCRIPTOR_LEN,
    );
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
    out.extend_from_slice(&replication.encode());
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
        if bytes.len() < 8 + REPLICATION_DESCRIPTOR_LEN {
            return Err(NavigationTierError::Truncated);
        }
        let num_clusters = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let expected_len = 8
            + num_clusters * padded_dims * 4
            + num_clusters * CLUSTER_DIR_ENTRY_LEN
            + REPLICATION_DESCRIPTOR_LEN;
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
        let len = self.num_clusters * CLUSTER_DIR_ENTRY_LEN;
        &self.bytes[start..start + len]
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

    /// The closure-replication descriptor trailer (M2-1) — the real
    /// replication policy and knobs this segment was built with.
    pub fn replication(&self) -> ReplicationDescriptor {
        let start = self.bytes.len() - REPLICATION_DESCRIPTOR_LEN;
        ReplicationDescriptor::decode(&self.bytes[start..])
    }

    /// The realized closure-replication factor: total (cluster, vector)
    /// assignments across every cluster divided by the number of distinct
    /// vectors in this field — computable entirely from data already
    /// resident after the cold-open wave (this navigation tier's own
    /// `cluster_dir` plus the segment's already-decoded `Hotcache::
    /// row_id_count`, per this field's dense-per-row-id contract, `spec/
    /// vectors.md` §5), no further fetch required. Returns `1.0` when
    /// `distinct_row_id_count == 0` (an empty field) rather than dividing
    /// by zero.
    pub fn realized_replication_factor(&self, distinct_row_id_count: u64) -> f64 {
        if distinct_row_id_count == 0 {
            return 1.0;
        }
        let total_assignments: u64 = (0..self.num_clusters)
            .map(|c| self.cluster_dir(c).vector_count as u64)
            .sum();
        total_assignments as f64 / distinct_row_id_count as f64
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

    #[test]
    fn plain_build_navigation_tier_emits_an_all_zero_replication_trailer() {
        // Every navigation tier built before M2-1 is, byte-for-byte, what
        // `ReplicationDescriptor::none()` produces (had it carried a
        // trailer at all) — the whole point of choosing an all-zero
        // encoding for "no replication."
        let padded_dims = 4;
        let centroids: Vec<f32> = vec![1.0, 1.0, 1.0, 1.0];
        let dirs = vec![ClusterDirEntry {
            region_offset: 0,
            code_bytes_length: 0,
            vector_count: 0,
        }];
        let bytes = build_navigation_tier(&centroids, padded_dims, &dirs);
        let trailer = &bytes[bytes.len() - REPLICATION_DESCRIPTOR_LEN..];
        assert_eq!(trailer, &[0u8; REPLICATION_DESCRIPTOR_LEN]);

        let reader = NavigationTierReader::new(&bytes, padded_dims).expect("valid blob");
        assert_eq!(reader.replication(), ReplicationDescriptor::none());
    }

    #[test]
    fn round_trips_a_real_spann_closure_replication_descriptor() {
        let padded_dims = 4;
        let centroids: Vec<f32> = vec![1.0, 1.0, 1.0, 1.0];
        let dirs = vec![ClusterDirEntry {
            region_offset: 0,
            code_bytes_length: 0,
            vector_count: 0,
        }];
        let replication = ReplicationDescriptor::spann_closure(8, 10.0);
        let bytes =
            build_navigation_tier_with_replication(&centroids, padded_dims, &dirs, replication);

        let reader = NavigationTierReader::new(&bytes, padded_dims).expect("valid blob");
        let decoded = reader.replication();
        assert_eq!(decoded.policy_raw, 1);
        assert_eq!(decoded.policy, Some(ReplicationPolicy::SpannClosure));
        assert_eq!(decoded.max_replicas, 8);
        assert_eq!(decoded.epsilon, 10.0);
    }

    #[test]
    fn an_unrecognized_policy_byte_decodes_to_none_without_rejecting_the_blob() {
        // Forward compatibility: query resolution's own row-id
        // deduplication already tolerates duplicates regardless of *why*
        // they're duplicated (spec/vectors.md §6 step 3), so a reader that
        // doesn't recognize a future policy value still decodes this blob
        // correctly. `NavigationTierReader::new` must not reject it, and
        // `replication().policy` must surface `None` (unrecognized) rather
        // than panicking or silently mapping to a registered variant.
        let padded_dims = 4;
        let centroids: Vec<f32> = vec![1.0, 1.0, 1.0, 1.0];
        let dirs = vec![ClusterDirEntry {
            region_offset: 0,
            code_bytes_length: 0,
            vector_count: 0,
        }];
        let mut bytes = build_navigation_tier(&centroids, padded_dims, &dirs);
        let trailer_start = bytes.len() - REPLICATION_DESCRIPTOR_LEN;
        bytes[trailer_start] = 42; // an unregistered future policy byte

        let reader = NavigationTierReader::new(&bytes, padded_dims).expect("still valid");
        let decoded = reader.replication();
        assert_eq!(decoded.policy_raw, 42);
        assert_eq!(decoded.policy, None);
    }

    #[test]
    #[should_panic(expected = "max_replicas must be nonzero")]
    fn spann_closure_rejects_a_zero_replica_cap() {
        ReplicationDescriptor::spann_closure(0, 10.0);
    }

    #[test]
    fn realized_replication_factor_matches_hand_computed_ratio() {
        let padded_dims = 4;
        let centroids: Vec<f32> = vec![0.0; 4 * 2];
        // Two clusters, 3 and 2 assignments respectively -> 5 total
        // assignments over, say, 3 distinct vectors (one replicated twice).
        let dirs = vec![
            ClusterDirEntry {
                region_offset: 0,
                code_bytes_length: 0,
                vector_count: 3,
            },
            ClusterDirEntry {
                region_offset: 0,
                code_bytes_length: 0,
                vector_count: 2,
            },
        ];
        let bytes = build_navigation_tier(&centroids, padded_dims, &dirs);
        let reader = NavigationTierReader::new(&bytes, padded_dims).unwrap();
        assert_eq!(reader.realized_replication_factor(3), 5.0 / 3.0);
        assert_eq!(
            reader.realized_replication_factor(0),
            1.0,
            "an empty field must not divide by zero"
        );
    }
}
