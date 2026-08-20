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

//! Library half of `strand-tools`: `main.rs` is a thin CLI wrapper around
//! these modules, exposed here too so other crates (`bench/`'s real-scale
//! verification tooling) can call the same real logic directly rather than
//! shelling out to the built binary.

pub mod ciff;
pub mod convert;
pub mod inspect;
