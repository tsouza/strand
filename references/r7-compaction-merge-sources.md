# R7 — Compaction and merge: FreshDiskANN and Lance deletion vectors

Vendored excerpts. Fetched 2026-08-18. (SPFresh and the IP-DiskANN paper, R7's other
two named sources, are already vendored: `references/spfresh-sosp2023.md`,
`references/ip-diskann-arxiv-2502.13826.md`.)

Cited by: `docs/research/README.md` R7, `CLAUDE.md` invariant 2 (deletion vectors).

## FreshDiskANN (Singh et al., 2021)

**Source:** `arxiv.org/abs/2105.09613`. Submitted May 20, 2021.

**Title:** FreshDiskANN: A Fast and Accurate Graph-Based ANN Index for Streaming
Similarity Search
**Authors:** Aditi Singh, Suhas Jayaram Subramanya, Ravishankar Krishnaswamy, Harsha
Vardhan Simhadri

> The paper presents "the first graph-based ANNS index that reflects corpus updates
> into the index in real-time without compromising on search performance," and can
> "index over a billion points on a workstation with an SSD and limited memory."

This is the paper `docs/research/README.md` R7 and `verification/manifest.tla`'s
neighboring documentation cite as the state-of-the-art batch-consolidation approach
that IP-DiskANN (already vendored) frames itself against — FreshDiskANN accumulates
deletions and periodically consolidates the graph, rather than updating in place,
which is the direct precedent for invariant 1's "rebuild" merge strategy for graph
indexes.

## Lance — deletion vectors

**Source:** `lance.org/format/table/` (the deletion-vector section specifically;
the standalone anchor URL `#deletion` 404'd, so this was read from the full table-
format page already fetched for R6, `references/r6-second-engine-sources.md`).

Lance marks deleted rows via a per-fragment deletion file rather than rewriting data
immediately, in one of two formats: an Arrow IPC `Int32Array` of deleted row
positions (sparse deletions) or a **Roaring bitmap** (dense deletions).

> "Deletions can be materialized by rewriting data files with deleted rows removed"
> — described as expensive (rewriting data files, invalidating row addresses,
> rebuilding indices) and explicitly deferred rather than immediate.

Confirms two things invariant 2 depends on: soft-delete-then-deferred-physical-
removal as the pattern (matching STRAND's own "deletes are deletion-vector blobs;
updates are delete + reinsert; physical removal is deferred to compaction"), and
Roaring specifically as Lance's own choice for the dense case — corroborating
invariant 2's Roaring default from a second production format, not just from
Roaring's own container-operations grounding
(`references/roaring-bitmaps-container-operations.md`).
