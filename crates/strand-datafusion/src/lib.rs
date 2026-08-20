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

//! A thin, read-only Apache DataFusion `TableProvider` over STRAND segments
//! (roadmap item M5-1, `docs/milestones.md` M5 entry, `docs/roadmap.md`).
//! `CLAUDE.md` §1 is explicit that this milestone is the one deliberate
//! exception to "this is a format, not an engine" — it exists to prove a
//! stranger's query engine can sit on top of a STRAND segment, not to grow
//! STRAND its own query layer. Every design choice here follows from that:
//! no query planning, no custom `ExecutionPlan`, no filter/limit pushdown
//! beyond what falls out of decoding once and handing DataFusion's own
//! in-memory execution machinery the result (`lexical_table`'s module doc
//! comment has the full accounting).
//!
//! Scope, stated honestly (`CLAUDE.md` §1's "narrower claim, deliberately"
//! ethos): this first pass covers one field of one already-resident
//! segment's **lexical** family (`crates/strand-lexical`) only —
//! `docs/roadmap.md` M5-1's own text: "can read lexical blobs as early
//! slices." The vector family (`crates/strand-vector`), multi-segment
//! tables (reading a manifest snapshot's several `SegmentRef`s as one
//! logical table), and deletion-vector filtering (invariant 2 — a resident
//! segment's tombstoned row-IDs are not applied here) are real, named gaps,
//! not silently dropped scope; see `lexical_table`'s doc comment for what
//! each would take.

pub mod lexical_table;

pub use lexical_table::{LexicalTableError, StrandLexicalTable, lexical_schema};
