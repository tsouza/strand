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

//! Reference implementation of STRAND's lexical (`family_id = 1`) and filter
//! (`family_id = 2`) blob families: the term-dictionary FST and term-info
//! store (`spec/term-dictionary.md`, RFC 0005), postings
//! (`spec/postings.md`, RFC 0007), and the value-dictionary FST and
//! filter-bitmap store (`spec/filter-bitmaps.md`, RFC 0006). `field` wires
//! the lexical trio into a real `strand-core` segment and back — the first
//! working end-to-end query path.

pub mod analyzer;
pub mod field;
pub mod filter_bitmaps;
pub mod positions;
pub mod postings;
pub mod term_dictionary;
