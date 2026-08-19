# Design lineage

The prior-art map for STRAND's design. Extracted verbatim from `CLAUDE.md` §4 at
repository seeding: the repository shape originally planned this extraction for M0,
but it was pulled forward at seed time so `CLAUDE.md` stays a governable size.
Referenced from `CLAUDE.md`.
Each RFC names the prior art it evolves from; the spec's introduction credits this
lineage.

We stand on prior work openly and say so. The short version:

**From Lucene**: the pluggable-codec model over a shared doc-ID space, and the
separation of flat vector storage from the graph structure (`FlatVectorsFormat`). We
lift the pattern out of one engine into a neutral container and replace the 31-bit
segment doc-ID with a 64-bit row-ID.

**From tantivy / Quickwit**: the immutable segment as the unit of work, and the
hotcache — a footer-first byte-range map that opens a split on S3 in tens of
milliseconds. We make the hotcache a specified, engine-neutral structure. (Quickwit
relicensed from AGPLv3 to Apache-2.0 under Datadog, 2025-01-23 — confirmed byte-level
and commit-level, not just from the announcement,
`references/r11c-quickwit-relicense-and-hotcache-source.md`; its split format remains
the closest lexical relative, and R11(c) confirms it is built as an ordinary consumer
of tantivy's public `Directory` trait, the same extension point, not a patch to
tantivy internals — though its own split/hotcache wire bytes are Quickwit's own
format, not something a STRAND adapter can reuse directly.)

**From PISA**: block-max (WAND) metadata decoupled from postings compression — computed
once, valid under any codec. Adopted as a spec invariant.

**From Lance**: the layering discipline — indexes are redundant, versioned search
structures kept out of the table format — and the **index-aware manifest**: Lance's
versioned manifest references index blobs without coupling to their internals, which is
exactly the shape `CLAUDE.md` §6 adopts.

**From Iceberg**: the atomic-pointer-swap commit model — immutable versioned metadata,
a single current pointer, compare-and-swap on the pointer. S3's conditional writes
(If-None-Match, GA August 2024; If-Match ETag CAS, November 2024) make this achievable
without an external catalog. From **Puffin**, the container pattern of opaque typed
blobs with a JSON footer — adopted with eyes open: Puffin's registry has spawned
essentially no third-party blob ecosystem despite Iceberg's gravity. The pattern is a
good container, not a distribution strategy.

**From SPANN / SPFresh / turbopuffer**: the cluster-shaped cold architecture. SPANN's
centroids-in-memory, posting-lists-on-disk layout; SPFresh's LIRE incremental
rebalancing; turbopuffer's published production evidence that this shape — not graph
traversal — survives object-storage latency. turbopuffer is closed-source; it is an
attributed lesson and a benchmark target (Appendix A), never an implementation source.

**From DiskANN**: the two-tier memory model — compressed codes for routing, full
precision only for reranking — which R1 generalizes into the tiered vector blob. Its
one-node-per-I/O-unit layout is warm-tier prior art, not a cold-path design.

**From FastLanes (CWI)**: the resolution of the hardware-marriage tension. Instead of
targeting a real ISA, the FastLanes layout targets a virtual 1024-bit SIMD register
with a unified transposed tuple order, so the same wire bytes decode at maximum
data-parallelism on 8/16/32/64-bit lanes — and the scalar decoder auto-vectorizes
robustly by construction (>40 values per CPU cycle across Intel, AMD, Apple, and AWS
chips; VLDB 2023, vendored at M0). This is the existence proof that "compute-native"
and "no vendor register width in wire bytes" (invariant 10) are compatible goals, plus
a published GPU decode path (DaMoN '24) and a float codec (ALP, SIGMOD '24) relevant
to the flat vector blob. Candidate status only: no inverted-index application
exists in the literature — adopting it for postings would be a first, which the
R9 RFC must say plainly. Its license is already resolved, not a candidacy gate:
`cwida/FastLanes` is confirmed MIT (`CLAUDE.md` §1, `docs/ledger.md` R9),
Apache-2.0-compatible.

**From CIFF**: the negative lesson, now with company. CIFF is a well-made exchange
format no engine runs operationally: conversion required, no positions, no pruning
bounds, no analyzer metadata, lossy doc lengths. Every gap is a MUST here.

**The graveyard.** Indri and Galago: well-specified academic formats that died with
their labs, because a format nobody's production engine is economically forced to read
is a paper artifact. BitFunnel: a hardware-profile bet, published with strong numbers,
adopted by nobody. The Optane-era formats: hardware-specific choices baked into media
layouts, unimplementable the day the hardware died — the standing argument for keeping
register widths out of wire bytes (invariant 10, `CLAUDE.md` §5). Pilosa: a good
structure with a spec is not a
distribution strategy. Every RFC's "how this could be wrong" section names its nearest
grave.
