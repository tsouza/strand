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

//! The batch-shaped read interface every reader/merge interface in the core
//! crates exposes (`CLAUDE.md` invariant 9): `next_batch` is the primary
//! interface, and this shape is frozen. Batch size is a per-implementation
//! parameter — the recommended range is still open research (R2,
//! `docs/ledger.md`) — so this trait does not pin one. A plain `Iterator`
//! impl may exist on a type alongside this trait for ergonomics, but it is
//! never the benchmarked path.

/// A reader or merge cursor that yields its items in batches rather than one
/// at a time.
pub trait BatchReader {
    type Item;

    /// Appends the next batch onto `out`, returning how many items were
    /// appended. Returns `0` when exhausted. Never clears `out` — callers
    /// own buffer reuse across calls.
    fn next_batch(&mut self, out: &mut Vec<Self::Item>) -> usize;
}
