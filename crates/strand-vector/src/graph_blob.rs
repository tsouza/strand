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

//! The graph blob family's wire format (`rfcs/0014-graph-blob-family.md`
//! Design §§1–3, `family_id = 5`, "graph"): the graph node-record blob
//! (`blob_type_id = 0`) and the node-order permutation directory
//! (`blob_type_id = 1`), plus the real construction-to-wire integration
//! that takes `crate::vamana::build_vamana`'s output (task 1 of this
//! implementation sequence) and `crate::reorder::bnf`/`bnp`'s output (task
//! 2) and produces the two `strand_core::segment::BlobSpec`s a real segment
//! is built from. Deliberately **not** the query path: `GreedySearch`/
//! `BeamSearch` over this wire format (Design §5) is a separate, later
//! task in this same sequence — this module only proves a writer and a
//! reader agree on what the bytes mean, the same scope every other blob
//! family's own read-path proves before its query layer is built.
//!
//! **The permutation convention this module assumes, stated precisely
//! because it is load-bearing for every byte offset below.**
//! `crate::reorder::Permutation` is documented as "a dense mapping from
//! **logical node index** (`0..n`, `crate::vamana::Graph`'s own
//! point-array-index convention) to **physical slot index**." RFC 0014
//! Design §3's own permutation-directory blob is "indexed by **local
//! ordinal** (row-id order)." This module identifies the two: the `points`/
//! `row_ids`/`graph` arrays a caller passes in are assumed to already be in
//! local-ordinal order (array index `i` *is* local ordinal `i`), which is
//! the same assumption `crate::vamana::build_vamana` itself makes (`points`
//! is "`n * dims` f32 values, row-major," fed in whatever order the caller
//! chose — a writer that wants this module's `row_ids[i]` to be local
//! ordinal `i`'s real row-id simply passes vectors and row-ids in that
//! order to `build_vamana` in the first place). This is a documented
//! integration choice, not a new format decision: RFC 0014 itself does not
//! specify how a writer obtains array-index-to-row-id association, only
//! that the *wire* permutation directory is row-id-order-indexed (Design
//! §3) — the choice made here is the direct, obvious one.

use crate::vamana::{Graph, VamanaResult};
use strand_core::container::{ChunkCodec, StorageClass, Tier};
use strand_core::segment::BlobSpec;

/// `family_id = 5`, "graph" (`rfcs/0014-graph-blob-family.md` Design §1,
/// `spec/container.md` §9).
pub const FAMILY_ID: u16 = 5;
/// `blob_type_id = 0`: graph node records (Design §2).
pub const BLOB_TYPE_NODE_RECORDS: u16 = 0;
/// `blob_type_id = 1`: node-order permutation directory (Design §3).
pub const BLOB_TYPE_PERMUTATION_DIRECTORY: u16 = 1;

/// The graph node-record blob's fixed header size (Design §2).
pub const NODE_RECORD_HEADER_LEN: usize = 16;

/// Design §2's `shuffle_algorithm` byte: which node-order-permutation
/// algorithm (if any) produced this field's physical slot assignment.
/// Affects only I/O locality, never correctness (Design §4) — a reader
/// never needs to interpret this to decode or traverse the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShuffleAlgorithm {
    /// `0`: no shuffling — physical order is ID/insertion-contiguous.
    Unshuffled = 0,
    /// `1`: Starling's BNP (Block Neighbor Padding, `crate::reorder::bnp`).
    Bnp = 1,
    /// `2`: Starling's BNF (Block Neighbor Frequency, `crate::reorder::
    /// bnf`) — RFC 0014's own registered v0.1 default.
    Bnf = 2,
    /// `3`: Starling's BNS (Block Neighbor Swap) — registered as
    /// conforming by the RFC, though `crate::reorder` does not implement it
    /// (that module's own top-level documentation: the vendored reference
    /// gives BNS as prose with no concrete mechanism, so implementing one
    /// would be presenting a guess as Starling's own algorithm).
    Bns = 3,
}

impl ShuffleAlgorithm {
    /// Unrecognized values are **not** rejected, matching
    /// `crate::navigation::ReplicationPolicy::from_u8`'s own forward-
    /// compatibility precedent: `shuffle_algorithm` affects only I/O
    /// locality (Design §4), never correctness, so a reader that doesn't
    /// recognize a future value still decodes and traverses this blob
    /// correctly — it just can't name which algorithm produced the layout.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(ShuffleAlgorithm::Unshuffled),
            1 => Some(ShuffleAlgorithm::Bnp),
            2 => Some(ShuffleAlgorithm::Bnf),
            3 => Some(ShuffleAlgorithm::Bns),
            _ => None,
        }
    }
}

/// `record_size` bytes, derived identically by every writer and reader
/// from `dims` and `max_out_degree` alone (Design §2: "a value every
/// reader derives from the header rather than a separately stored
/// redundant field").
pub fn node_record_size(dims: usize, max_out_degree: usize) -> usize {
    8 + dims * 4 + 4 + 4 + max_out_degree * 4
}

/// The real construction-to-wire integration inputs: `build_vamana`'s own
/// result (task 1) plus a computed node-order permutation (task 2's
/// `bnf`/`bnp`, or the identity permutation for `shuffle_algorithm =
/// Unshuffled`), bundled into one struct so [`build_node_records`] and
/// [`build_graph_blob_specs`] stay under this crate's `clippy::
/// too_many_arguments` threshold — the same discipline `crate::vamana`'s
/// own private `Dataset` struct established for [`crate::vamana::
/// build_vamana`]'s internal helpers.
///
/// `permutation`, `points`, and `row_ids` MUST all be indexed by the same
/// **local ordinal** convention `vamana.graph`'s own array indices already
/// use (this module's own top-level documentation).
#[derive(Debug, Clone, Copy)]
pub struct GraphBlobInput<'a> {
    pub vamana: &'a VamanaResult,
    pub permutation: &'a [usize],
    pub points: &'a [f32],
    pub dims: usize,
    pub row_ids: &'a [u64],
    pub max_out_degree: usize,
    pub shuffle_algorithm: ShuffleAlgorithm,
}

/// Builds the graph node-record blob (Design §2): a 16-byte header
/// followed by `node_count` fixed-width records, one per node, laid out in
/// **physical slot order** — record `k`'s neighbors are the physical slot
/// indices of `k`'s out-neighbors, and every unused `neighbor_slots` entry
/// beyond `neighbor_count` is zero-filled (the padding-determinism rule,
/// Design §2).
///
/// # Panics
///
/// Panics if `input.permutation`/`input.row_ids` don't have exactly
/// `input.vamana.graph.len()` entries, if `input.points.len() !=
/// input.vamana.graph.len() * input.dims`, if `input.permutation` is not a
/// bijection onto `0..n`, if `input.entry_point >= n`, if any node's real
/// out-degree exceeds `input.max_out_degree`, or if `input.max_out_degree`
/// does not fit in a `u8`.
pub fn build_node_records(input: &GraphBlobInput) -> Vec<u8> {
    let graph: &Graph = &input.vamana.graph;
    let n = graph.len();
    assert_eq!(
        input.permutation.len(),
        n,
        "permutation must cover every node in graph"
    );
    assert_eq!(
        input.row_ids.len(),
        n,
        "row_ids must cover every node in graph"
    );
    assert_eq!(
        input.points.len(),
        n * input.dims,
        "points must be exactly n*dims f32 values"
    );
    assert!(
        input.max_out_degree <= usize::from(u8::MAX),
        "max_out_degree must fit in a u8"
    );
    assert!(input.vamana.entry_point < n, "entry_point out of range");

    // physical_slot_of[logical] = input.permutation[logical]; invert it to
    // recover, for each physical slot, which logical node occupies it —
    // the order this blob's own records are actually written in.
    let mut logical_at_slot = vec![usize::MAX; n];
    for (logical, &slot) in input.permutation.iter().enumerate() {
        assert!(slot < n, "slot {slot} out of range for n={n}");
        assert_eq!(
            logical_at_slot[slot],
            usize::MAX,
            "permutation must be a bijection: slot {slot} claimed by more than one node"
        );
        logical_at_slot[slot] = logical;
    }

    let record_size = node_record_size(input.dims, input.max_out_degree);
    let mut out = Vec::with_capacity(NODE_RECORD_HEADER_LEN + n * record_size);

    let entry_point_slot = input.permutation[input.vamana.entry_point];
    out.extend_from_slice(&(n as u32).to_le_bytes());
    out.extend_from_slice(&(input.dims as u32).to_le_bytes());
    out.push(input.max_out_degree as u8);
    out.push(input.shuffle_algorithm as u8);
    out.extend_from_slice(&[0u8; 2]); // reserved
    out.extend_from_slice(&(entry_point_slot as u32).to_le_bytes());

    for &logical in &logical_at_slot {
        let row_id = input.row_ids[logical];
        out.extend_from_slice(&row_id.to_le_bytes());

        let vector = &input.points[logical * input.dims..(logical + 1) * input.dims];
        for &v in vector {
            out.extend_from_slice(&v.to_le_bytes());
        }

        let neighbors = &graph[logical];
        assert!(
            neighbors.len() <= input.max_out_degree,
            "node {logical} has {} neighbors, exceeding max_out_degree {}",
            neighbors.len(),
            input.max_out_degree
        );
        out.extend_from_slice(&(neighbors.len() as u32).to_le_bytes());
        out.extend_from_slice(&[0u8; 4]); // reserved

        for i in 0..input.max_out_degree {
            let value = neighbors
                .get(i)
                .map(|&neighbor_logical| input.permutation[neighbor_logical] as u32)
                .unwrap_or(0); // zero-padding, Design §2's determinism rule
            out.extend_from_slice(&value.to_le_bytes());
        }
    }

    out
}

/// Builds the node-order permutation directory blob (Design §3): a dense,
/// local-ordinal-indexed array of little-endian `u32` physical slots.
/// `permutation[i]` is already exactly this blob's `i`-th entry — the
/// wire format *is* [`crate::reorder::Permutation`] serialized densely, no
/// further transformation needed.
///
/// # Panics
///
/// Panics if any entry of `permutation` doesn't fit in a `u32`.
pub fn build_permutation_directory(permutation: &[usize]) -> Vec<u8> {
    let mut out = Vec::with_capacity(permutation.len() * 4);
    for &slot in permutation {
        let slot_u32 = u32::try_from(slot).expect("physical slot must fit in a u32");
        out.extend_from_slice(&slot_u32.to_le_bytes());
    }
    out
}

/// The real construction-to-wire integration point: [`build_node_records`]
/// and [`build_permutation_directory`], wrapped as two
/// `strand_core::segment::BlobSpec`s ready for
/// `strand_core::segment::SegmentBuilder::add_blob`, per Design §1's
/// registration table (both `storage-class: raw-mappable`, `tier: warm`;
/// node records 8-byte aligned for the `row_id: u64` field at record
/// offset 0, the permutation directory 4-byte aligned for its `u32`
/// entries).
///
/// # Panics
///
/// Same conditions as [`build_node_records`].
pub fn build_graph_blob_specs(field_id: u64, input: &GraphBlobInput) -> (BlobSpec, BlobSpec) {
    let node_records = build_node_records(input);
    let permutation_directory = build_permutation_directory(input.permutation);

    let node_records_spec = BlobSpec {
        family_id: FAMILY_ID,
        blob_type_id: BLOB_TYPE_NODE_RECORDS,
        field_id,
        storage_class: StorageClass::RawMappable,
        tier: Tier::Warm,
        alignment: 8,
        chunk_codec: ChunkCodec::None,
        chunk_codec_level: 0,
        data: node_records,
    };
    let permutation_directory_spec = BlobSpec {
        family_id: FAMILY_ID,
        blob_type_id: BLOB_TYPE_PERMUTATION_DIRECTORY,
        field_id,
        storage_class: StorageClass::RawMappable,
        tier: Tier::Warm,
        alignment: 4,
        chunk_codec: ChunkCodec::None,
        chunk_codec_level: 0,
        data: permutation_directory,
    };
    (node_records_spec, permutation_directory_spec)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphBlobError {
    Truncated,
}

/// A resident graph node-record blob (Design §2).
#[derive(Debug, Clone, Copy)]
pub struct NodeRecordReader<'a> {
    bytes: &'a [u8],
    node_count: usize,
    dims: usize,
    max_out_degree: usize,
    shuffle_algorithm_raw: u8,
    entry_point_slot: u32,
    record_size: usize,
}

impl<'a> NodeRecordReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, GraphBlobError> {
        if bytes.len() < NODE_RECORD_HEADER_LEN {
            return Err(GraphBlobError::Truncated);
        }
        let node_count = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let dims = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let max_out_degree = bytes[8] as usize;
        let shuffle_algorithm_raw = bytes[9];
        let entry_point_slot = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
        let record_size = node_record_size(dims, max_out_degree);
        let expected_len = NODE_RECORD_HEADER_LEN + node_count * record_size;
        if bytes.len() != expected_len {
            return Err(GraphBlobError::Truncated);
        }
        Ok(NodeRecordReader {
            bytes,
            node_count,
            dims,
            max_out_degree,
            shuffle_algorithm_raw,
            entry_point_slot,
            record_size,
        })
    }

    pub fn node_count(&self) -> usize {
        self.node_count
    }

    pub fn dims(&self) -> usize {
        self.dims
    }

    pub fn max_out_degree(&self) -> usize {
        self.max_out_degree
    }

    /// The raw `shuffle_algorithm` byte — see [`ShuffleAlgorithm::from_u8`]
    /// for the tolerant, forward-compatible decode.
    pub fn shuffle_algorithm_raw(&self) -> u8 {
        self.shuffle_algorithm_raw
    }

    pub fn entry_point_slot(&self) -> u32 {
        self.entry_point_slot
    }

    fn record(&self, slot: usize) -> &'a [u8] {
        assert!(slot < self.node_count, "slot out of range");
        let start = NODE_RECORD_HEADER_LEN + slot * self.record_size;
        &self.bytes[start..start + self.record_size]
    }

    /// `slot`'s stable row-id.
    ///
    /// # Panics
    ///
    /// Panics if `slot >= self.node_count()`.
    pub fn row_id(&self, slot: usize) -> u64 {
        let record = self.record(slot);
        u64::from_le_bytes(record[0..8].try_into().unwrap())
    }

    /// `slot`'s full-precision `dims`-length vector.
    ///
    /// # Panics
    ///
    /// Panics if `slot >= self.node_count()`.
    pub fn vector(&self, slot: usize) -> Vec<f32> {
        let record = self.record(slot);
        (0..self.dims)
            .map(|i| {
                let off = 8 + i * 4;
                f32::from_le_bytes(record[off..off + 4].try_into().unwrap())
            })
            .collect()
    }

    /// `slot`'s real out-degree (`<= max_out_degree`).
    ///
    /// # Panics
    ///
    /// Panics if `slot >= self.node_count()`.
    pub fn neighbor_count(&self, slot: usize) -> u32 {
        let record = self.record(slot);
        let off = 8 + self.dims * 4;
        u32::from_le_bytes(record[off..off + 4].try_into().unwrap())
    }

    /// `slot`'s real out-neighbor physical slots — exactly `neighbor_count`
    /// entries, excluding the zero-padding beyond it (Design §2's
    /// padding-determinism rule).
    ///
    /// # Panics
    ///
    /// Panics if `slot >= self.node_count()`.
    pub fn neighbor_slots(&self, slot: usize) -> Vec<u32> {
        let record = self.record(slot);
        let count = self.neighbor_count(slot) as usize;
        let start = 16 + self.dims * 4;
        (0..count)
            .map(|i| {
                let off = start + i * 4;
                u32::from_le_bytes(record[off..off + 4].try_into().unwrap())
            })
            .collect()
    }
}

/// A resident node-order permutation directory (Design §3): `physical_slot
/// = permutation[local_ordinal]`.
#[derive(Debug, Clone, Copy)]
pub struct PermutationDirectoryReader<'a> {
    bytes: &'a [u8],
    node_count: usize,
}

impl<'a> PermutationDirectoryReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, GraphBlobError> {
        if !bytes.len().is_multiple_of(4) {
            return Err(GraphBlobError::Truncated);
        }
        Ok(PermutationDirectoryReader {
            bytes,
            node_count: bytes.len() / 4,
        })
    }

    pub fn node_count(&self) -> usize {
        self.node_count
    }

    /// `local_ordinal`'s physical slot.
    ///
    /// # Panics
    ///
    /// Panics if `local_ordinal >= self.node_count()`.
    pub fn physical_slot(&self, local_ordinal: usize) -> u32 {
        assert!(
            local_ordinal < self.node_count,
            "local_ordinal out of range"
        );
        let off = local_ordinal * 4;
        u32::from_le_bytes(self.bytes[off..off + 4].try_into().unwrap())
    }

    /// The full permutation, densely, `local_ordinal` order.
    pub fn to_vec(&self) -> Vec<u32> {
        (0..self.node_count)
            .map(|i| self.physical_slot(i))
            .collect()
    }
}

/// The decoded content of both graph-family blobs: an adjacency list plus
/// the permutation, recovered from real wire bytes — the correctness
/// property this task proves and the query path (a later task) depends on
/// completely. Deliberately **not** a query result: no traversal, no
/// distance computation, just "what did the writer actually write."
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedGraph {
    pub node_count: usize,
    pub dims: usize,
    pub max_out_degree: usize,
    pub shuffle_algorithm_raw: u8,
    pub entry_point_slot: u32,
    /// Indexed by physical slot.
    pub row_ids: Vec<u64>,
    /// Indexed by physical slot; `dims` values each.
    pub vectors: Vec<Vec<f32>>,
    /// Indexed by physical slot: real out-neighbor physical slots, padding
    /// already stripped (`NodeRecordReader::neighbor_slots`).
    pub adjacency: Vec<Vec<u32>>,
    /// Indexed by local ordinal: physical slot
    /// (`PermutationDirectoryReader::to_vec`).
    pub permutation: Vec<u32>,
}

/// Decodes both graph-family blobs' real, on-disk bytes back into
/// in-memory structures — the reader half of this module, proving a reader
/// recovers exactly what [`build_node_records`]/
/// [`build_permutation_directory`] wrote. Not the query path (Design §5):
/// no `GreedySearch`/`BeamSearch` traversal happens here.
pub fn decode_graph_blobs(
    node_record_bytes: &[u8],
    permutation_directory_bytes: &[u8],
) -> Result<DecodedGraph, GraphBlobError> {
    let node_reader = NodeRecordReader::new(node_record_bytes)?;
    let perm_reader = PermutationDirectoryReader::new(permutation_directory_bytes)?;
    let n = node_reader.node_count();

    let row_ids = (0..n).map(|slot| node_reader.row_id(slot)).collect();
    let vectors = (0..n).map(|slot| node_reader.vector(slot)).collect();
    let adjacency = (0..n)
        .map(|slot| node_reader.neighbor_slots(slot))
        .collect();
    let permutation = perm_reader.to_vec();

    Ok(DecodedGraph {
        node_count: n,
        dims: node_reader.dims(),
        max_out_degree: node_reader.max_out_degree(),
        shuffle_algorithm_raw: node_reader.shuffle_algorithm_raw(),
        entry_point_slot: node_reader.entry_point_slot(),
        row_ids,
        vectors,
        adjacency,
        permutation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reorder::{BnfConfig, bnf};
    use crate::vamana::{VamanaConfig, build_vamana};
    use rand::Rng;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    // ---- RFC 0014's own 5-node worked example, constructed directly from
    // ---- the RFC's own stated (illustrative, not build_vamana-derived)
    // ---- topology and physical slot assignment -- not from build_vamana,
    // ---- per this task's own instruction (the RFC's own topology is "a
    // ---- plausible Vamana output for hand-checkability, not a
    // ---- hand-execution of Algorithm 3", exactly the same convention
    // ---- crate::vamana's own tests already found for this same graph).

    fn worked_example_input() -> (VamanaResult, Vec<usize>, Vec<f32>, Vec<u64>) {
        // A=0 (row_id 10), B=1 (row_id 11), C=2 (row_id 12), D=3 (row_id
        // 13), E=4 (row_id 14).
        let graph: Graph = vec![
            vec![1, 2], // A -> B, C
            vec![0, 3], // B -> A, D
            vec![3, 0], // C -> D, A
            vec![1, 4], // D -> B, E
            vec![3],    // E -> D
        ];
        let vamana = VamanaResult {
            graph,
            entry_point: 0, // A
        };
        // "slot 0 = B, slot 1 = A, slot 2 = C, slot 3 = D, slot 4 = E" ->
        // permutation[logical] = physical slot: A->1, B->0, C->2, D->3,
        // E->4.
        let permutation = vec![1, 0, 2, 3, 4];
        let points = vec![
            0.0, 0.0, // A
            1.0, 0.0, // B
            0.0, 1.0, // C
            1.0, 1.0, // D
            2.0, 2.0, // E
        ];
        let row_ids = vec![10u64, 11, 12, 13, 14];
        (vamana, permutation, points, row_ids)
    }

    #[test]
    fn node_records_match_the_rfcs_worked_example_byte_for_byte() {
        let (vamana, permutation, points, row_ids) = worked_example_input();
        let input = GraphBlobInput {
            vamana: &vamana,
            permutation: &permutation,
            points: &points,
            dims: 2,
            row_ids: &row_ids,
            max_out_degree: 2,
            shuffle_algorithm: ShuffleAlgorithm::Bnf,
        };
        let bytes = build_node_records(&input);

        assert_eq!(bytes.len(), 176, "16-byte header + 5*32-byte records");

        // Header, byte for byte, per the RFC's own "Graph node-record blob
        // header" table.
        assert_eq!(
            &bytes[0..16],
            &[
                0x05, 0x00, 0x00, 0x00, // node_count = 5
                0x02, 0x00, 0x00, 0x00, // dims = 2
                0x02, // max_out_degree = 2
                0x02, // shuffle_algorithm = 2 (BNF)
                0x00, 0x00, // reserved
                0x01, 0x00, 0x00, 0x00, // entry_point_slot = 1 (A's slot)
            ],
            "header"
        );

        // Slot 0 = B.
        assert_eq!(
            &bytes[16..48],
            &[
                0x0B, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // row_id = 11
                0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x00, 0x00, // vector (1.0, 0.0)
                0x02, 0x00, 0x00, 0x00, // neighbor_count = 2
                0x00, 0x00, 0x00, 0x00, // reserved
                0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, // neighbor_slots [1, 3]
            ],
            "slot 0 = B"
        );

        // Slot 1 = A.
        assert_eq!(
            &bytes[48..80],
            &[
                0x0A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // row_id = 10
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // vector (0.0, 0.0)
                0x02, 0x00, 0x00, 0x00, // neighbor_count = 2
                0x00, 0x00, 0x00, 0x00, // reserved
                0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, // neighbor_slots [0, 2]
            ],
            "slot 1 = A"
        );

        // Slot 2 = C.
        assert_eq!(
            &bytes[80..112],
            &[
                0x0C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // row_id = 12
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x3F, // vector (0.0, 1.0)
                0x02, 0x00, 0x00, 0x00, // neighbor_count = 2
                0x00, 0x00, 0x00, 0x00, // reserved
                0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, // neighbor_slots [3, 1]
            ],
            "slot 2 = C"
        );

        // Slot 3 = D.
        assert_eq!(
            &bytes[112..144],
            &[
                0x0D, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // row_id = 13
                0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F, // vector (1.0, 1.0)
                0x02, 0x00, 0x00, 0x00, // neighbor_count = 2
                0x00, 0x00, 0x00, 0x00, // reserved
                0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, // neighbor_slots [0, 4]
            ],
            "slot 3 = D"
        );

        // Slot 4 = E. Second neighbor_slots entry is zero-padding, not a
        // real edge to slot 0.
        assert_eq!(
            &bytes[144..176],
            &[
                0x0E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // row_id = 14
                0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x40, // vector (2.0, 2.0)
                0x01, 0x00, 0x00, 0x00, // neighbor_count = 1
                0x00, 0x00, 0x00, 0x00, // reserved
                0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // neighbor_slots [3, pad]
            ],
            "slot 4 = E"
        );
    }

    #[test]
    fn permutation_directory_matches_the_rfcs_worked_example_byte_for_byte() {
        let (_vamana, permutation, _points, _row_ids) = worked_example_input();
        let bytes = build_permutation_directory(&permutation);
        assert_eq!(
            bytes,
            vec![
                0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x03, 0x00,
                0x00, 0x00, 0x04, 0x00, 0x00, 0x00,
            ]
        );
    }

    #[test]
    fn node_records_and_permutation_directory_match_the_pinned_conformance_golden_files() {
        let (vamana, permutation, points, row_ids) = worked_example_input();
        let input = GraphBlobInput {
            vamana: &vamana,
            permutation: &permutation,
            points: &points,
            dims: 2,
            row_ids: &row_ids,
            max_out_degree: 2,
            shuffle_algorithm: ShuffleAlgorithm::Bnf,
        };
        let node_records = build_node_records(&input);
        let permutation_directory = build_permutation_directory(&permutation);

        let golden = |name: &str| {
            std::fs::read(
                concat!(env!("CARGO_MANIFEST_DIR"), "/../../conformance/graph/").to_owned() + name,
            )
            .unwrap_or_else(|e| panic!("golden file conformance/graph/{name} must exist: {e}"))
        };
        assert_eq!(node_records, golden("toy-node-records.bin"));
        assert_eq!(
            permutation_directory,
            golden("toy-permutation-directory.bin")
        );
    }

    #[test]
    fn decode_recovers_the_worked_example_exactly() {
        let (vamana, permutation, points, row_ids) = worked_example_input();
        let input = GraphBlobInput {
            vamana: &vamana,
            permutation: &permutation,
            points: &points,
            dims: 2,
            row_ids: &row_ids,
            max_out_degree: 2,
            shuffle_algorithm: ShuffleAlgorithm::Bnf,
        };
        let node_records = build_node_records(&input);
        let permutation_directory = build_permutation_directory(&permutation);

        let decoded = decode_graph_blobs(&node_records, &permutation_directory).unwrap();
        assert_eq!(decoded.node_count, 5);
        assert_eq!(decoded.dims, 2);
        assert_eq!(decoded.max_out_degree, 2);
        assert_eq!(decoded.shuffle_algorithm_raw, 2);
        assert_eq!(
            ShuffleAlgorithm::from_u8(decoded.shuffle_algorithm_raw),
            Some(ShuffleAlgorithm::Bnf)
        );
        assert_eq!(decoded.entry_point_slot, 1);
        assert_eq!(decoded.row_ids, vec![11, 10, 12, 13, 14]); // slot order B,A,C,D,E
        assert_eq!(decoded.vectors[0], vec![1.0, 0.0]); // slot 0 = B
        assert_eq!(decoded.vectors[1], vec![0.0, 0.0]); // slot 1 = A
        assert_eq!(decoded.adjacency[0], vec![1, 3]); // B -> {A, D} physical
        assert_eq!(decoded.adjacency[4], vec![3]); // E -> {D} physical, no padding entry
        assert_eq!(decoded.permutation, vec![1, 0, 2, 3, 4]);
    }

    // ---- Real segment integration: SegmentBuilder, footer/hotcache open,
    // ---- blob-registry lookup, decode -- the same discipline
    // ---- crates/strand-vector/tests/segment_assembly.rs already
    // ---- established for RFC 0010's own cluster family. Kept here (a
    // ---- unit test, not a separate integration test file) since it needs
    // ---- no fixtures beyond this module's own worked-example helper.

    #[test]
    fn assembles_and_reopens_the_worked_example_as_a_real_segment() {
        use strand_core::container::{Footer, Hotcache};
        use strand_core::segment::SegmentBuilder;

        let (vamana, permutation, points, row_ids) = worked_example_input();
        let input = GraphBlobInput {
            vamana: &vamana,
            permutation: &permutation,
            points: &points,
            dims: 2,
            row_ids: &row_ids,
            max_out_degree: 2,
            shuffle_algorithm: ShuffleAlgorithm::Bnf,
        };
        let field_id = strand_core::container::field_id_from_name("vec");
        let (node_records_spec, permutation_directory_spec) =
            build_graph_blob_specs(field_id, &input);
        let expected_node_records = node_records_spec.data.clone();
        let expected_permutation_directory = permutation_directory_spec.data.clone();

        let mut builder = SegmentBuilder::new(5);
        builder.add_blob(node_records_spec);
        builder.add_blob(permutation_directory_spec);
        let segment_bytes = builder.build(1000);

        // Real cold-open path: footer -> hotcache -> blob registry.
        let footer_bytes: [u8; 40] = segment_bytes[segment_bytes.len() - 40..]
            .try_into()
            .unwrap();
        let footer = Footer::decode(&footer_bytes).expect("valid footer");
        let start = footer.hotcache_offset as usize;
        let end = start + footer.hotcache_length as usize;
        let hotcache = Hotcache::decode(&segment_bytes[start..end]).expect("valid hotcache");
        assert_eq!(hotcache.row_id_count, 5);

        let find = |blob_type_id: u16| {
            let entry = hotcache
                .blobs
                .iter()
                .find(|b| {
                    b.family_id == FAMILY_ID
                        && b.blob_type_id == blob_type_id
                        && b.field_id == field_id
                })
                .unwrap_or_else(|| {
                    panic!("no family_id=5 blob_type_id={blob_type_id} in registry")
                });
            let o = entry.offset as usize;
            let l = entry.length as usize;
            &segment_bytes[o..o + l]
        };

        let node_records_bytes = find(BLOB_TYPE_NODE_RECORDS);
        let permutation_directory_bytes = find(BLOB_TYPE_PERMUTATION_DIRECTORY);
        assert_eq!(node_records_bytes, &expected_node_records[..]);
        assert_eq!(
            permutation_directory_bytes,
            &expected_permutation_directory[..]
        );

        let decoded = decode_graph_blobs(node_records_bytes, permutation_directory_bytes).unwrap();
        assert_eq!(decoded.entry_point_slot, 1);
        assert_eq!(decoded.row_ids, vec![11, 10, 12, 13, 14]);
        assert_eq!(decoded.permutation, vec![1, 0, 2, 3, 4]);
    }

    // ---- Round-trip property test at larger, realistic scale: a real
    // ---- build_vamana + bnf graph, written and read back through the real
    // ---- wire format, decoded structure asserted to exactly match the
    // ---- in-memory construction output -- the correctness property the
    // ---- query path (a later task) depends on completely.

    #[test]
    fn round_trips_a_real_vamana_plus_bnf_graph_through_the_wire_format_at_scale() {
        let n = 300;
        let dims = 16;
        let mut rng = StdRng::seed_from_u64(99);
        let points: Vec<f32> = (0..n * dims)
            .map(|_| rng.random_range(-20.0f32..20.0))
            .collect();
        let config = VamanaConfig {
            r: 12,
            l_param: 24,
            alpha: 1.2,
        };
        let vamana = build_vamana(&points, n, dims, &config, &mut rng);

        let bnf_config = BnfConfig {
            block_size: 16,
            beta: 10,
            tau: 0.0,
        };
        let permutation: Vec<usize> = bnf(&vamana.graph, &bnf_config);

        let row_ids: Vec<u64> = (0..n as u64).map(|i| 1_000_000 + i * 7).collect();

        let input = GraphBlobInput {
            vamana: &vamana,
            permutation: &permutation,
            points: &points,
            dims,
            row_ids: &row_ids,
            max_out_degree: config.r,
            shuffle_algorithm: ShuffleAlgorithm::Bnf,
        };
        let node_records = build_node_records(&input);
        let permutation_directory = build_permutation_directory(&permutation);

        // Wrap into a real segment and reopen cold, exactly like the
        // worked-example test above, so this proves the same real
        // SegmentBuilder/Footer/Hotcache path at realistic scale, not just
        // the raw byte builder functions in isolation.
        use strand_core::container::{Footer, Hotcache};
        use strand_core::segment::SegmentBuilder;

        let field_id = strand_core::container::field_id_from_name("vec");
        let (node_records_spec, permutation_directory_spec) =
            build_graph_blob_specs(field_id, &input);
        assert_eq!(node_records_spec.data, node_records);
        assert_eq!(permutation_directory_spec.data, permutation_directory);

        let mut builder = SegmentBuilder::new(n as u64);
        builder.add_blob(node_records_spec);
        builder.add_blob(permutation_directory_spec);
        let segment_bytes = builder.build(0);

        let footer_bytes: [u8; 40] = segment_bytes[segment_bytes.len() - 40..]
            .try_into()
            .unwrap();
        let footer = Footer::decode(&footer_bytes).expect("valid footer");
        let start = footer.hotcache_offset as usize;
        let end = start + footer.hotcache_length as usize;
        let hotcache = Hotcache::decode(&segment_bytes[start..end]).expect("valid hotcache");

        let find = |blob_type_id: u16| {
            let entry = hotcache
                .blobs
                .iter()
                .find(|b| b.family_id == FAMILY_ID && b.blob_type_id == blob_type_id)
                .unwrap();
            let o = entry.offset as usize;
            let l = entry.length as usize;
            &segment_bytes[o..o + l]
        };
        let node_records_bytes = find(BLOB_TYPE_NODE_RECORDS);
        let permutation_directory_bytes = find(BLOB_TYPE_PERMUTATION_DIRECTORY);

        let decoded = decode_graph_blobs(node_records_bytes, permutation_directory_bytes).unwrap();

        // Full structural equality: for every logical node, its decoded
        // physical slot must carry exactly the row-id, vector, and
        // (physical-slot-translated) neighbor set build_vamana/bnf actually
        // produced.
        assert_eq!(decoded.node_count, n);
        assert_eq!(decoded.dims, dims);
        assert_eq!(decoded.max_out_degree, config.r);
        assert_eq!(
            decoded.entry_point_slot as usize,
            permutation[vamana.entry_point]
        );
        assert_eq!(
            decoded.permutation,
            permutation.iter().map(|&s| s as u32).collect::<Vec<u32>>()
        );

        for logical in 0..n {
            let slot = permutation[logical];
            assert_eq!(
                decoded.row_ids[slot], row_ids[logical],
                "row_id mismatch at logical node {logical} (slot {slot})"
            );
            assert_eq!(
                decoded.vectors[slot],
                points[logical * dims..(logical + 1) * dims].to_vec(),
                "vector mismatch at logical node {logical} (slot {slot})"
            );
            let expected_neighbors: Vec<u32> = vamana.graph[logical]
                .iter()
                .map(|&neighbor_logical| permutation[neighbor_logical] as u32)
                .collect();
            assert_eq!(
                decoded.adjacency[slot], expected_neighbors,
                "adjacency mismatch at logical node {logical} (slot {slot})"
            );
        }
    }
}
