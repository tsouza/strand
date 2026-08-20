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

//! STRAND's vector blob family (`family_id = 3`): flat vectors, the RaBitQ
//! quantization descriptor, the cluster navigation tier, and cluster
//! posting lists. Layout is normative per `spec/vectors.md`, approved by
//! RFC 0010 (`rfcs/0010-vector-blob-cluster-family.md`). Also hosts the
//! graph blob family (`family_id = 5`, "graph"): Vamana construction
//! (`vamana`), Starling's node-order-permutation algorithms (`reorder`),
//! the wire format tying them together (`graph_blob`), and the cold-open
//! `GreedySearch`/`BeamSearch` query path over that wire format
//! (`graph_query`), approved by RFC 0014
//! (`rfcs/0014-graph-blob-family.md`).

pub mod closure;
pub mod codebook;
pub mod descriptor;
pub mod estimate;
pub mod fastscan;
pub mod flat;
pub mod graph_blob;
pub mod graph_query;
pub mod kmeans;
pub mod navigation;
pub mod orthogonal;
pub mod posting_list;
pub mod quantize;
pub mod quantize_ex;
pub mod query;
pub mod reorder;
pub mod rotate;
pub mod vamana;
