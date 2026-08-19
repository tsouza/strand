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
//! RFC 0010 (`rfcs/0010-vector-blob-cluster-family.md`).

pub mod descriptor;
pub mod estimate;
pub mod fastscan;
pub mod flat;
pub mod kmeans;
pub mod navigation;
pub mod orthogonal;
pub mod posting_list;
pub mod quantize;
pub mod query;
pub mod rotate;
