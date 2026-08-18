# Row-ID space

Normative for STRAND v0.1. Defines the 64-bit row-ID that is the format's
fusion contract (invariant 1): how a segment declares its row-ID range,
how local ordinals map to it, and how each blob family's declared merge
strategy resolves against a stable identity. Approved by RFC 0001
(`rfcs/0001-container-rowid-manifest.md`); this chapter states the settled
result — see the RFC for alternatives considered and the adversarial
review.

Reference implementation: the `row_id_base`/`row_id_count` fields of
`spec/container.md` §4's hotcache region
(`crates/strand-core/src/container.rs`); range assignment during commit,
`crates/strand-core/src/manifest.rs`.

## 1. Definition

A row-ID is a 64-bit unsigned integer. Within one segment, the hotcache
declares a contiguous range `[row_id_base, row_id_base + row_id_count)`
(`spec/container.md` §4), assigned by the writer at build time. For local
ordinal `i` in `[0, row_id_count)`, that segment's row-ID for ordinal `i`
is `row_id_base + i`.

Every blob family that stores per-row data dense-indexed by local ordinal
(a flat vector blob, a lexical doc-length array) MUST use this same
mapping. No family defines its own row-ID-to-position table; `local_ordinal
= row_id - row_id_base` is the one arithmetic every reader needs.

## 2. Global uniqueness

Global uniqueness of row-IDs across all segments in one index is a
manifest-level property (`spec/manifest.md`), not a container-level one. A
writer *proposes* a range read from the current snapshot's `next_row_id`
cursor, but that proposal is only real once its commit wins the pointer
compare-and-swap. A writer that loses the race MUST re-read the winner's
`next_row_id` and recompute its range before retrying — reusing a
previously proposed range after losing the race is non-conforming, since
it can produce two segments claiming the same row-IDs (`spec/manifest.md`
§2, step 3).

## 3. What "stable" means under merge (invariant 1)

Each blob family declares a merge strategy. This section states, for
each, exactly what changes and what does not when segments are merged:

- **Concatenate + remap** (IVF/SPANN posting lists). A merge concatenates
  the source segments' posting lists into the new segment's storage and
  *remaps* each entry's internal position — its offset into the new
  segment's dense arrays. The row-ID values the entries reference are
  copied through unchanged. This is what stable row-IDs buy: a merge
  rewrites *pointers*, never *identities*.
- **Rebuild** (graph indexes). The merged segment's graph is built from
  scratch over the union of surviving row-IDs. Row-IDs are inputs to the
  rebuild; no structure is preserved across it.
- **Rebalance** (centroid layers). Row-IDs move between clusters as
  centroids shift under drift, but the row-ID values themselves are
  unchanged — only which posting list currently contains a given row-ID
  changes.

A deletion vector (Roaring, invariant 2) marks row-IDs, not local
ordinals, as tombstoned. This is why it survives a merge that remaps
local ordinals without needing a remap step of its own.

## 4. Merge cost

Invariant 1 requires merge cost stated honestly per family, not glossed
by the word "compaction." This chapter does not itself define the merge
*procedure* (compaction lands at M3); it states the row-ID-stability
contract that any future merge procedure MUST honor, per family, as
described in §3.
