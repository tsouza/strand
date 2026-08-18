# CLAUDE.md — Project Constitution (v3.2)

**STRAND** — **S**parse-**T**erm **R**etrieval **A**nd **N**earest-neighbor
**D**ense-vector. The name is the mission statement: one format carrying both
retrieval families over one row-ID space, the strands woven together at fusion time.
This is the project's real name; it is no longer a working name.

This file governs every Claude Code session in this repository. Read it fully before
acting. When an instruction here conflicts with anything else, this file wins. This
document is self-contained: it is the only file seeding the repository, and everything
a session needs to start — including the condensed research grounding (Appendix C) —
is in it. The full research report and the vendored primary sources are brought into
`docs/research/` and `references/` at M0.

**Provenance of v2.** This revision applied the adversarial review of v1 and the R1–R8
research report (both vendored into `docs/` at M0; their applied conclusions are
captured in this file). Two of the review's "apply verbatim" corrections were
themselves overturned by the research and are amended here: the turbopuffer "60×"
figure was located verbatim in the primary source and is kept with citation (reversing
the review's deletion), and the napkin-math round-trip figure is pinned to sourced
numbers so no RFC can pick whichever latency flatters its design. Everything in this
file marked **settled** was verified against primary sources; sessions apply it and do
not re-litigate it. Everything marked **open** requires an RFC backed by the research
tracks. Appendix B is the ledger.

**Provenance of v2.1.** A post-rewrite fact-check against the live turbopuffer
architecture page corrected Appendix A's cold/warm p50 figures (874ms/14ms true-cold,
not 343ms/8ms), pinned a provisional cold-open byte budget so the R1 kill criterion is
falsifiable (§8), and marked two figures as pending verification rather than settled
(the ~250ms p90 tail figure in §8; tantivy's current postings codec in §6). No design
decision changed.

**Provenance of v2.2.** Added research track R9 (compute-native block layout): the
FastLanes transposed layout (Afroozeh & Boncz, VLDB 2023) became a named candidate in
the postings bake-off and in the lineage, because it demonstrates that a layout can be
hardware-native and ISA-portable simultaneously — resolving the tension invariant 10
previously managed by prohibition alone. One ledger change: the 128-int postings block
size moved from settled to conditional, since FastLanes operates in 1024-value units
and the R9 granularity evaluation may overturn 128. The BP128 provisional default is
unchanged until R2/R9 report; the FastLanes license is unaudited and gates any
adoption.

**Provenance of v3 (final).** This revision applies a third adversarial review, aimed
at the gap between a format spec and the running engine whose published numbers ground
it. Blocking fixes: cold-path accounting now includes manifest resolution, which the
engine's "3–4 roundtrips" always included and ours silently didn't (§8); invariant 3
gains a one-wave addressability rule, without which "one massive roundtrip" is engine
behavior the format merely hopes for; invariant 5's score-parity promise is made
implementable via named scoring profiles; invariant 4 now requires raw,
scoring-independent bounds; a new invariant 11 pins byte determinism (endianness, term
sort order, chunk codec, checksums, codec-variant and stochastic-transform
provenance), closing the clean-room gaps where two correct implementers would produce
different bytes; and §7 gains the minimal safety rules without which multi-writer
compaction loses data or 404s live readers (deletion-safety retention, reader
404-refresh, a declared CAS host, an orphan-sweep rule). The §2 source-sentence rule
caught its fourth and fifth embellishments: v2.2's claim that FastLanes exceeds BP128
"by an order of magnitude" (the paper's headline wins are against scalar baselines;
the margin over hand-vectorized BP128 on postings is unmeasured and is now R9's stated
question) and §8's "GB/s single-stream" budget rationale (restated on parallel fetch,
to be measured at M0). One honest narrowing, stated rather than hidden: the format is
per-segment, the manifest carries nothing that prunes segments at query time, so query
cost grows O(segments) — §8 says so and a new track R10 studies manifest-level
pruning. No settled-ledger entry was overturned; invariants 3, 4, and 5 were amended
to be implementable, which this note records. External-document references were
replaced by Appendix C so this file stands alone.

**Provenance of v3.1.** This revision adds the benchmark-adapter strategy (§8,
"Benchmark engines and adapters") after an adversarial review of its draft. The §2
source-sentence rule caught its sixth embellishment: the draft claimed FAISS's
"IVF+RaBitQ-class scanning" could run over the generic `InvertedLists` extension
point, blending the two kernels §6 had already separated — source inspection
(2026-08-18) shows the 1-bit FastScan path runs over `CodePacker`-packed lists, a
different byte contract, so the engine-constant vector claim is narrowed to the
multi-bit path pending R11. Three rules were added that the draft lacked: a
one-binary/two-formats requirement for the tantivy fork, without which
"engine-constant" is a hope rather than a protocol; a build-equivalence gate
(build once, convert, prove statistical parity and identical top-k before any
timing), without which every adapter number is confounded at the build path; and
a conformance obligation making every adapter a real second reader under
invariant 11. The fork's "failed experiment" clause gained measurable triggers.
Two license claims were upgraded from memory to source: tantivy and FAISS are both
MIT, verified byte-level against their LICENSE files on 2026-08-18; the files are
vendored at M0. No settled-ledger entry was overturned.

**Provenance of v3.2.** The project is named: **STRAND** (Sparse-Term Retrieval And
Nearest-neighbor Dense-vector), replacing the working name "sextant" everywhere —
prose as STRAND, crates as `strand-core`, `strand-lexical`, `strand-vector`,
`strand-tools`, the Lucene adapter as `StrandCodec`. Nothing else changed; no
design decision, ledger entry, or figure was touched.

---

## 1. What this project is

An open, engine-agnostic **storage format for search indexes**, designed for object
storage first. One container, one stable row-ID space, one minimal commit layer,
pluggable index blobs:

- **Lexical**: BM25-ready inverted index (postings, positions, term stats, block-max bounds)
- **Vector**: tiered, chunk-fetchable ANN indexes (cluster/IVF family cold-native;
  graph family registered as warm-tier), with flat vector storage separated from index
  structures
- **Hybrid**: the shared row-ID space is the fusion contract; the format guarantees
  identity across blob types so RRF and score fusion work across engines

It is a **format** — a specification plus a Rust reference implementation. It is not a
search engine, not a database, not a query planner. If a session drifts toward building
engine features, stop and return to the format. The one deliberate exception is the
consumer milestone (M5): a thin DataFusion TableProvider exists to prove a stranger's
engine can sit on top, because the research concluded the second reader must be built,
not found.

The mission sentence, revised by R1: *CIFF you can query in place on S3, extended to
vectors — where "in place" means chunk-shaped access: a small, bounded number of large,
independent fetches, never dependent pointer-chasing.* Cold vector search in v0.1 is
cluster-shaped. Graph indexes are in the format as a warm-tier blob family and are
explicitly not the cold-open story. The v0.1 cold story is additionally **per-segment**:
the format makes each segment cheap to open and query cold, and reports segment-count
amplification honestly (§8); a manifest-level cross-segment navigation layer is open
research (R10), not a v0.1 promise. This is a narrower claim than v1 made, and it is
the honest one.

**License: Apache-2.0.** Every file carries the header. Every dependency must be
Apache-2.0-compatible. No exceptions. (The RaBitQ reference implementations were
license-audited in R3: all three repositories are Apache-2.0. Standard obligations
apply — retain LICENSE/NOTICE, state changes. The FastLanes license is unaudited and
gates any R9 adoption.)

### What stays out of the format

Query-time fusion logic, ranking models, and analyzer *implementations* do not belong
in the spec. What belongs in: row-ID mapping, deletion vectors, term and collection
statistics, block-max bounds, scoring-profile descriptors, distance-metric metadata,
quantization codebooks and kernel selection, analyzer *descriptors with normative
conformance vectors*, per-blob storage-class and tier declarations, per-blob-family
merge semantics, the blob-type registry, and the snapshot manifest with its safety
rules (§7).

---

## 2. How to write (this is a hard rule)

All prose in this repository — spec text, docs, RFCs, commit messages, code comments —
is written for **human readers**. Specifically:

- Be economical. Say a thing once, in the clearest place, and reference it elsewhere.
- Write plain, well-formed sentences. Never compress prose into keyword-dense fragments
  optimized for machine parsing. A paragraph a tired engineer can read at 11pm beats a
  dense one that saves forty tokens.
- Prefer prose over bullet walls. Use a list only when items are genuinely parallel.
- One idea per paragraph. Short paragraphs.
- No filler: no "it's important to note", no "in order to", no restating what the
  previous section said.
- Spec language uses RFC 2119 keywords (MUST, SHOULD, MAY) and uses them precisely.
- A number without a vendored source sentence is deleted, not softened. This rule has
  now caught six real errors across four revisions — two v1 embellishments, v2's own
  cold-p50 error, v2.2's FastLanes throughput claim and single-stream bandwidth
  arithmetic, and the A1 draft's FAISS kernel conflation — and it stays.

Test for every document: could a competent engineer who has never seen this repo read
it once and implement against it? If not, rewrite.

---

## 3. Working method (lessons from prior AI-built OSS)

The closest successful precedent is Cloudflare's `workers-oauth-provider` — a
production OAuth 2.1 library largely written by Claude in 2025. Its lessons, sharpened
by our own review cycles:

**Humans design, the agent implements.** Format design decisions land in RFCs approved
by the maintainer; sessions implement approved RFCs. Never invent a format decision
mid-session. If implementation reveals a design problem, stop, write it up in the RFC's
Discussion section, and wait.

**Provenance in the commit log.** Each commit message states the task or prompt that
produced it. This repo should be readable as a record of how an AI-built format
actually got built.

**The model's memory of standards is not a source.** The sharpest external criticism of
the Cloudflare library found it implementing a deprecated OAuth grant from stale memory.
Our own v1 committed the same sin twice: it named SIMD-BP128 while describing a
patched-exception codec that does not exist under that name, and it prescribed Gorder —
a graph-analytics reordering — for ANN beam search, a different workload. Both were the
texture of a model blending adjacent techniques from memory. Rule: **never implement
against a remembered spec.** Fetch the primary source into the session, vendor it in
`references/`, and cite it.

**Start from usage, not structure.** Every RFC includes at least one worked example:
actual bytes, actual offsets, a real tiny index a human can check by hand.

**Review is adversarial, not ceremonial.** Every RFC ships with a "how this could be
wrong" section, which must also name which death from the graveyard (§4) it most risks
repeating. Fuzzing and round-trip property tests are not optional.

**Do the arithmetic before the design.** The single most expensive v1 error — a
graph-ANN cold path over S3 — would have been killed by one line of multiplication.
The napkin-math rule (§8) is the institutionalized form of that lesson. The v3 review
extended it: the arithmetic must include the manifest layer, because "roundtrips per
query" that start after the metadata is magically in hand are an engine's accounting,
not a format's.

---

## 4. Design lineage (evolution, with attribution)

We stand on prior work openly and say so. The spec's introduction must credit this
lineage; each RFC names the prior art it evolves from; `docs/lineage.md` (created at
M0 from this section) maintains the full map. The short version:

**From Lucene**: the pluggable-codec model over a shared doc-ID space, and the
separation of flat vector storage from the graph structure (`FlatVectorsFormat`). We
lift the pattern out of one engine into a neutral container and replace the 31-bit
segment doc-ID with a 64-bit row-ID.

**From tantivy / Quickwit**: the immutable segment as the unit of work, and the
hotcache — a footer-first byte-range map that opens a split on S3 in tens of
milliseconds. We make the hotcache a specified, engine-neutral structure. (Quickwit is
now Apache-2.0 under Datadog; its split format remains the closest lexical relative.)

**From PISA**: block-max (WAND) metadata decoupled from postings compression — computed
once, valid under any codec. Adopted as a spec invariant.

**From Lance**: the layering discipline — indexes are redundant, versioned search
structures kept out of the table format — and the **index-aware manifest**: Lance's
versioned manifest references index blobs without coupling to their internals, which is
exactly the shape §7 adopts.

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
to the flat vector blob. Candidate status only: license unaudited, and no
inverted-index application exists in the literature — adopting it for postings would
be a first, which the R9 RFC must say plainly.

**From CIFF**: the negative lesson, now with company. CIFF is a well-made exchange
format no engine runs operationally: conversion required, no positions, no pruning
bounds, no analyzer metadata, lossy doc lengths. Every gap is a MUST here.

**The graveyard.** Indri and Galago: well-specified academic formats that died with
their labs, because a format nobody's production engine is economically forced to read
is a paper artifact. BitFunnel: a hardware-profile bet, published with strong numbers,
adopted by nobody. The Optane-era formats: hardware-specific choices baked into media
layouts, unimplementable the day the hardware died — the standing argument for keeping
register widths out of wire bytes (§6). Pilosa: a good structure with a spec is not a
distribution strategy. Every RFC's "how this could be wrong" section names its nearest
grave.

---

## 5. Non-negotiable design invariants

These are settled. A session may propose changing one only via RFC, never by drifting.

1. **64-bit stable row-IDs, with per-blob-family merge semantics.** The row-ID space is
   the format's core contract. The spec MUST define ID behavior across merges and
   compactions explicitly — and each blob family MUST declare its merge strategy:
   **rebuild** (graph indexes — neighbor structure does not compose under
   concatenation), **concatenate + remap** (IVF/SPANN posting lists — this is what
   stable row-IDs buy), or **rebalance** (centroid layers, LIRE-style). Merge cost is
   stated honestly per family, not glossed by the word "compaction."
2. **Immutable segments + deletion vectors.** No in-place mutation, ever. Deletes are
   deletion-vector blobs (Roaring); updates are delete + reinsert; physical removal is
   deferred to compaction and governed by §7's deletion-safety rule.
3. **Object storage is the primary target, and cold access is chunk-shaped and
   wave-addressable.** Opening a segment MUST cost at most two round trips before
   query planning can begin, counted from the segment footer read (the manifest layer
   above it has its own accounting, §8). Beyond the open, no cold read path may depend
   on data-dependent pointer chasing: cold structures are navigable from a small tier
   fetched wholesale, followed by a bounded number of independent, parallelizable
   fetches. New in v3, the one-wave rule: after the open, **every byte range a cold
   query may need MUST be addressable from data already fetched** — footer, hotcache,
   or navigation tier — with no offset lookup that costs a round trip, so that a
   conforming reader can issue each fetch stage as one parallel wave. The format can
   only make the wave possible; the M0 benchmark asserts a reader that actually issues
   it, and every cold-path RFC includes the round-trip arithmetic of §8. Design
   decisions are justified in GETs and bytes read, not only in CPU.
4. **Pruning metadata is codec-independent and scoring-independent.** Block-max bounds
   live beside postings as a sibling blob, never inside a codec's private structures —
   and the bounds themselves are **raw statistics** (for example, per-block maximum
   term frequency and minimum document length), never precomputed impact scores, so
   they survive codec swaps, merges, and scoring-profile changes without recomputation.
   The exact fields are pinned in the M1 block-max RFC; the raw-statistics principle is
   settled here.
5. **Scoring inputs survive interchange; parity is defined per scoring profile.**
   Collection stats, per-term doc frequencies, and per-document lengths are stored
   losslessly. Because "BM25" is a family, not a function, the format defines **named
   scoring profiles**: an identifier plus parameters (initially `bm25`, whose exact
   idf and tf formulas and k1/b parameters are written normatively in the spec
   chapter), carried in blob metadata. Score parity means: engine B evaluating the
   index's declared profile MUST match engine A within a stated floating-point
   tolerance. Parity with Lucene specifically is a profile of its own, defined as
   *parity within Lucene's one-byte norm quantization*: the parity harness computes
   Lucene's quantized norm from our lossless lengths and compares like with like.
   (Lucene's norms are lossy on purpose; demanding byte-parity against them while
   storing lossless lengths was v1's internal contradiction, and promising parity
   without pinning a formula was v2's. Both resolutions live here and in M1 so they
   cannot regrow.)
6. **Analyzer descriptors with normative conformance vectors.** A lexical blob carries
   a structured description of its analysis chain that pins, at minimum: Unicode
   version, ICU/CLDR version, tokenizer profile including any UAX #29 deviations,
   stopword list identity, stemmer name and version, and — for dictionary-segmented
   scripts (CJK, Thai, Lao) — the segmentation dictionary identity and version.
   Version pinning alone is insufficient (UAX #29 is explicitly unstable across
   Unicode versions; U+202F's Word_Break class changed in Unicode 9.0), so golden
   token-stream vectors in `conformance/analyzers/` — raw text in, exact token stream
   out, per descriptor — are **normative**. An engine that fails the vectors does not
   conform, whatever its descriptor says. The spec also defines per-document length
   precisely (which tokens are counted, at which point in the chain, per field), since
   invariant 5 depends on it. Undeclared analysis renders an index non-portable; the
   spec treats it as invalid.
7. **Flat vectors are separate from index structures, and every vector blob declares
   its tier.** Raw full-precision vectors are one blob, fetched only for reranking.
   Index blobs declare `tier: cold-fetchable` (the whole navigation tier fits the
   cold-open byte budget of §8 and is fetched wholesale) or `tier: warm` (assumes
   NVMe-class latency; graph families live here). Quantization codebooks, kernel
   selection, and distance-metric metadata travel with the index blob.
8. **Don't invent encodings.** Postings compression, adjacency layouts, and
   quantization schemes come from the literature and are registered as named codecs.
   Novelty budget is spent on the container, the ID contract, the manifest, and the
   metadata — the parts nobody has standardized.
9. **Batch-shaped read APIs; scalar reference is normative; SIMD is an optimization.**
   Every reader/merge interface in the core crates exposes `next_batch()` as the
   primary interface — the API *shape* is frozen; the batch *size* is a stated
   per-implementation parameter with a recommended range settled by benchmark (R2).
   turbopuffer's production fix used 512-value batches and is the validated reference
   point: their scan benchmark went from 6.5ms to ~110μs on 100k values — "60× faster
   than before," verbatim — and the production query from 220ms to 47ms
   (turbopuffer.com/blog/zero-cost, vendored at M0). A plain `Iterator` impl may exist
   for ergonomics but is never the benchmarked path. For the kernels themselves: the
   **scalar implementation is normative** — it defines the bit-exact result — and SIMD
   paths (stable Rust: `wide` or `pulp` with runtime multiversion dispatch; nightly
   `portable_simd` is not a dependency) MUST pass property-based equivalence tests
   against the scalar reference in CI. Autovectorization checks (`cargo asm`) are
   advisory only; v1's requirement to *prove* autovectorization per RFC was theater,
   fragile across rustc versions, and is withdrawn.
10. **Per-blob storage class; wire bytes are dense.** Every blob declares
    `storage-class: chunk-compressed` or `storage-class: raw-mappable`.
    Chunk-compressed blobs have dense wire bytes; SIMD alignment is a property of the
    decompressed in-memory buffer, which the reader controls by decompressing into
    aligned arenas — padding compressed wire bytes to register widths buys nothing and
    bakes today's hardware into a format meant to outlive it (see the Optane grave,
    §4). Raw-mappable blobs — intended for direct mmap or direct-to-device reads
    without decompression — declare a power-of-two byte alignment per blob. No vendor
    register width appears in spec text.
11. **Byte determinism (new in v3).** Two independent implementations given the same
    logical input MUST produce the same index — pinned as follows. All multi-byte
    integers in wire structures are **little-endian**. Term dictionaries sort terms by
    **unsigned byte order of their UTF-8 encoding**. Chunk compression is a declared
    codec from the registry; the default is **zstd** with the level recorded in
    metadata. Every chunk carries a declared checksum (default **xxHash3-64**) over
    its uncompressed content. A codec registration pins its *complete* layout,
    including delta/d-gap variant — "SIMD-BP128" is not a registration; "BP128,
    128-int blocks, D-variant X, little-endian" is (the variant is picked in the R2
    RFC). Every stochastic transform — RaBitQ's random rotation is the standing
    example — pins its provenance in the descriptor: either the materialized transform
    or a normatively specified generator plus seed (mechanism chosen in the R3/M2
    RFC). Golden files pin **uncompressed wire structures and decoded content**;
    compressed chunk bytes may vary across compressor versions and are verified by
    checksum and round-trip, not byte-comparison. Anything a spec chapter serializes
    without answering these questions is a spec bug, and the clean-room read at M4 is
    the test.

---

## 6. Data-structure baseline

Invariant 8 says don't invent encodings. This section names the starting defaults so
they aren't re-litigated inside every RFC. Each default is a specific, published,
production-proven technique, verified against primary sources in the R-track research
(Appendix C). A session may swap one for a named alternative via RFC backed by a
benchmark; it may not invent one.

**Postings (provisional default, confirmed by in-repo benchmark — R2).** Fixed
128-integer blocks encoded with **SIMD-BP128**: plain vectorized binary packing at a
per-block bit width, **no exception stream** (Lemire & Boytsov, SPE 2015, vendored at
M0). v1 described a patched-exception codec under this name; that codec does not
exist, and the description is corrected here. The trade is explicit: BP128 is the
fastest decode in its class (~0.7 cycles/int in the S4 variants) and the simplest
possible from-spec reader — no exception-stream handling — at a compression cost of up
to ~2 bits/int and 5–15% worse ratio than FastPFOR. **SIMD-FastPFOR** (exception
streams, better ratio) is a registered alternative codec, not the default. Two reasons
reader-simplicity wins: a conformance default is code every stranger must implement,
and the concrete second-reader candidate (the tantivy/DataFusion ecosystem, R6) is
believed to ship exception-free bitpacking — a claim Appendix B still lists as
**unverified** against current tantivy source; verify it when vendoring for R2, and if
it fails, this argument loses one of its two legs while the conformance-simplicity leg
stands alone. Lucene's PFOR lineage remains reachable via the registered alternative.
128 is the block size of shared lineage — Lucene, tantivy, and PISA all use it,
descended from the same design, which is an argument from compatibility, not (as v1
claimed) independent convergence. Per invariant 11, the R2 RFC pins the exact d-gap
variant; the name "BP128" alone is not a registration. The default is confirmed or
swapped by the R2 bake-off on MS MARCO distributions; the registry design is the
invariant, the default is a parameter. The bake-off has a third named candidate: the
**FastLanes transposed layout** (§4), whose paper reports portable scalar decode at
SIMD-class rates (>40 values per CPU cycle across Intel, AMD, Apple, and AWS chips) —
those headline wins are measured against scalar baselines, and its margin over
hand-vectorized BP128 on postings distributions is **unmeasured**; measuring it is
exactly R9's job, and v2.2's "order of magnitude" claim is deleted under the §2 rule.
FastLanes carries two further open questions that gate adoption (R9): its 1024-value
granularity conflicts with the 128-int block lineage — the block-max sibling blob must
be redesigned to 1024-granularity or a nested 8×128 scheme, and either must preserve
invariant 4 — and its license is unaudited. If R9 resolves these favorably and the
bake-off confirms a real margin on postings, the default swaps and the conformance
story changes with it; until then BP128 stands.

**Filter and set bitmaps (settled).** Roaring bitmap serialization — the interoperable
wire format used by Lucene, ClickHouse, Doris, and Druid. One of the few places where
reusing an exact wire format, not just a technique, buys real interoperability.

**Term dictionary (settled default).** An FST mapping term to ordinal, as in Lucene and
tantivy: compact, prefix/range-capable, enumerable in sorted order (sorted per
invariant 11's UTF-8 byte order). Caveat: FST construction requires sorted input and
is memory-hungry at build time; the spec's build-side notes must say so.

**Vector quantization (settled — R3).** **RaBitQ**, with kernel selection pinned by
bit-width, because the two paths are different computations and v1 conflated them:
the **1-bit** routing path uses FastScan-style LUT/register-shuffle machinery; the
**multi-bit** Extended-RaBitQ path computes distances exactly as classical scalar
quantization (the paper's own selling point — 4/5/7-bit typically reaching 90/95/99%
recall without reranking). The spec MUST say which kernel applies at which width, MUST
carry the optional random rotation as an explicit descriptor field — canonical RaBitQ
applies it, Elasticsearch's BBQ variant removes it, and its presence changes
bit-compatibility — and, per invariant 11, MUST pin the rotation's provenance
(materialized matrix or generator + seed; mechanism chosen in the M2 RFC). Adoption
gravity (Milvus, Faiss, Lucene/Elasticsearch, turbopuffer, CockroachDB) and the
Apache-2.0 license audit both favor RaBitQ; the TurboQuant priority dispute is
procedural, not a correctness or licensing cloud. PQ-FastScan remains a registered
alternative. Revisit only if independent at-scale reproduction shows a competitor
materially winning on *embedding* workloads.

**Vector index shape (settled constraint — R1; blob design open, RFC required).** The
cold-native family is cluster-shaped: a navigation tier (hierarchical centroids,
SPANN/SPFresh family) small enough to live in or beside the hotcache, then posting
lists of quantized codes fetched wholesale in large independent reads, then a rerank
fetch against full-precision vectors. This is the shape the entire cloud-native
evidence base converges on — turbopuffer's architecture states it plainly (centroid
index downloaded cold, then each cluster fetched "in one, massive roundtrip"), and the
2026 survey and benchmark literature (arXiv:2601.01937, arXiv:2511.14748) confirm
cluster indexes' fetch granularity fits object storage where graph traversal does not.
Note what "one massive roundtrip" assumes and invariant 3 now requires: all cluster
offsets resolvable from the navigation tier, so the fetches go out as one parallel
wave — wall time of one round trip, request count of nprobe, both reported. The sizing
law that binds segment scale: 1-bit codes cost dims/8 bytes per vector — 96 B at
768d, 128 B at 1024d, 192 B at 1536d — so tier-1 alone runs ~100 MB per million 768d
vectors, before navigation structure. SPANN-style closure replication (up to 8×) is a
first-class recall/storage/cost knob in the blob metadata, not a hidden constant.
**Graph families** (DiskANN/Vamana-layout, one node's vector + adjacency per I/O unit,
Starling-style block shuffling) are registered as `tier: warm` — legitimate and
specified, but never the cold-open story; pipelining does not rescue them, because
overlapping compute with I/O cannot convert a dependent chain of ~100ms round trips
into parallel fetches. Node order within a graph blob is a **persisted permutation**
(the format decision, settled); the ordering algorithm is explicitly open — v1 froze
Gorder, which targets graph-analytics traversal, not ANN beam search; Starling's block
shuffling is the relevant literature, and R1's RFC picks with evidence.

**The chunk/block split (settled for lexical; constrained for vectors).** A **chunk**
is the unit fetched from S3 (megabyte-scale, compressed per invariant 11's declared
codec); a **block** is the unit a CPU kernel consumes (fixed-width, decompressed into
aligned arenas per invariant 10). Chunks decompress into blocks; the footer's offset
index maps between them. This genuinely fits lexical search — block-max BM25 has
bounded, front-loaded access — and fits cluster-shaped vector search, whose posting
lists are chunk-shaped by construction. It does not fit graph traversal, which is
precisely why graphs are warm-tier. The split has a cost the spec states rather than
hides: **read amplification for rare terms** — a term whose postings occupy one
512-byte block still costs its whole chunk. Chunk sizing is therefore a stated
trade-off, layouts SHOULD co-locate data accessed together (a term's postings and
positions contiguous within a chunk), and every benchmark reports bytes-fetched
alongside bytes-used so the amplification is a measured number, not an anecdote. An
RFC whose layout only optimizes one granularity is incomplete.

**I/O paths.** Blocks are independently byte-range addressable so the same file serves
memory-mapped, `io_uring`-batched, and direct-to-device access patterns without format
changes. A raw-mappable blob in a FastLanes-style transposed layout additionally has a
published GPU decompression path (DaMoN '24) — the legitimate form of v1's GPUDirect
ambition, contingent on R9. Hardware-specific expectations live in the reader; the
format's only obligations are the storage-class declaration and alignment attributes
of invariant 10. Shard and partition boundaries are first-class in the layout;
NUMA-aware placement is a runtime concern the format merely must not obstruct.

---

## 7. The manifest layer

v1 specified the file and skipped the table. Segments alone cannot answer: which set of
segment files is the current index? Is a compaction atomically visible? Can two writers
coexist? Cross-engine sharing lives at this layer — Iceberg is the layer above Parquet,
and that layer is why engines share storage — so leaving it out would mean every
adopter builds an incompatible manifest and interop dies exactly where the spec can't
see it.

The format therefore includes a **minimal snapshot manifest**, deliberately the
smallest protocol that makes multi-writer interop true and nothing more:

- Immutable, versioned snapshot metadata files listing the segment set and, per the
  Lance model, referencing index blobs and their versions without coupling to their
  internals (index-aware, index-internals-agnostic).
- A single current pointer.
- Commit by compare-and-swap on the pointer. On S3 this is native since 2024
  (If-None-Match for create, If-Match ETag CAS for advance); GCS and Azure have
  long-standing equivalents (confirm exact header semantics at spec time). For stores
  without conditional writes, the fallback is an external catalog holding the pointer —
  the protocol is the same, only the CAS host moves.

Readers use the snapshot current at load time; writers race on the pointer and losers
retry. That is the whole database ambition of this project. Anything more — snapshot
expiry *policy*, schema evolution, multi-table transactions — is out of scope and
stays out. The v3 review found that safety, unlike policy, cannot be out of scope, and
added the following rules. Each is the smallest rule that closes a real data-loss or
404-mid-query scenario; the M0 crash tests exercise all of them.

**One declared CAS host.** The table metadata declares where the pointer lives:
native conditional writes on the store, or a named external catalog. All writers MUST
use the declared host. A writer using a different host is not "falling back" — it is
split-brain, and its commits are non-conforming. Moving the CAS host is itself a
committed metadata change through the old host.

**Deletion safety and reader 404-refresh.** A file MUST NOT be physically deleted
while any retained snapshot references it. Table metadata declares a minimum snapshot
retention (a count, a duration, or both); compaction may only physically delete files
unreferenced by every retained snapshot. A reader that gets 404 on a file its snapshot
references MUST treat the snapshot as expired — refresh to the current snapshot and
retry — rather than report the index corrupt. Expiry *policy* (how long to retain) is
the deployment's choice; the safety rule and the reader behavior are the spec's.

**Orphan files.** A writer that crashes — or loses the CAS race — after writing
segment or metadata files but before its commit lands leaves orphans. Orphans are
harmless to correctness (nothing references them) and cost only storage. The rule for
removing them: list the prefix, subtract everything referenced by retained snapshots,
delete the remainder older than the retention window. `strand-tools` grows this sweep
at M3; until then the rule is stated so no one invents a more clever, less safe one.

**Reader freshness has a price, and it is stated.** A reader's consistency model is
snapshot-at-load. Freshness costs one conditional GET of the pointer per refresh; the
format defines no push or notification mechanism, on purpose. A "warm query with
read-your-writes" therefore costs one pointer round trip more than a query against a
cached snapshot, and §8's metrics report both numbers. This is one of the places the
comparison engine's warm figures embed machinery (a WAL tail and a caching fleet) the
format does not have; Appendix A says so.

**Write amplification is the writer's problem, and the spec says so.** Immutable
segments mean a commit per tiny batch produces a segment per tiny batch, and cold
query cost grows with segment count until compaction. The format ships no WAL and no
memtable; a production writer batches on its own side. This is a real cost of the
design, stated rather than hidden, and it is one reason the segment-count
amplification metric exists in §8.

---

## 8. Performance and benchmarks

This format's credibility will be earned with reproducible numbers. Rules:

**The napkin-math rule (would have caught v1's worst error).** Every RFC touching a
cold-path structure includes the arithmetic: expected accesses per query × dependent
round-trip depth × round-trip latency, plus total bytes fetched — **counted
end-to-end, from the pointer read**, because a cold query resolves the manifest before
it opens a segment: pointer GET, snapshot metadata GET, then segment opens (≤2 RTT
each per invariant 3, issued in parallel across segments), then the cold-fetchable
waves. Arithmetic that starts after the metadata is in hand is an engine's accounting,
not a format's — the comparison engine's "3–4 roundtrips" includes its metadata trip.
The pinned figures, so no RFC cherry-picks: **~100ms per object-storage round trip**
as the planning figure (turbopuffer architecture docs, their stated first-principles
number, vendored at M0) and **~250ms p90 for small reads as the tail figure for SLO
discussion — pending: this figure came from v1's review and has not been re-located
in a current primary source; vendor the source sentence or replace it with a measured
MinIO/S3 tail figure at M0. Until then it is a placeholder and is marked as such
wherever used.** An RFC without this arithmetic is incomplete. For calibration:
50–200 dependent fetches at 100ms is 5–20 seconds — the graph-cold baseline being
escaped.

**Provisional cold-open byte budget: 100 MB per segment.** This is the budget that
`tier: cold-fetchable` blobs (invariant 7) and the R1 kill criterion (Appendix B) are
measured against: the total bytes a reader must fetch wholesale at open — navigation
tier plus quantized codes — before the first query can execute. Rationale, restated in
v3 after the §2 rule caught the old arithmetic: a single object-storage stream runs in
the ~50–100 MB/s class, so 100 MB on one stream is a second or more — the budget
assumes a reader that fetches ranges **in parallel** (which invariant 3's one-wave
rule makes possible), bringing the tier-1 wave into the low hundreds of milliseconds,
the same order as one round trip, so the open stays round-trip-bound rather than
bandwidth-bound. The achieved aggregate throughput of a parallel wave is measured at
M0, and the budget's expiry condition stands: the R1 RFC MUST confirm or replace this
figure with numbers measured in the M0 MinIO benchmarks; until then every cold-path
RFC computes against 100 MB and says so.

**Segment count is reported, never hidden.** The budget binds per segment, so scale
forces many segments — at the R1 sizing law, roughly one segment per million 768d
vectors — and the manifest carries nothing that prunes segments at query time, so
query cost grows O(segments). Every benchmark states its segment count; cold metrics
are reported per-segment and per-index; and M3 runs the multi-segment benchmark (on
the order of a hundred segments) that makes the amplification a measured curve.
Whether the manifest should carry optional per-segment summary metadata to prune that
curve is R10, an open question, not a v0.1 feature.

**No performance claim without a reproducible benchmark in-repo.** If a README sentence
says "fast", a `bench/` target backs it: pinned datasets, pinned hardware description,
one-command runner. `bench/` is a workspace member from M0. Criterion for micro, custom
harness for end-to-end; results committed under `bench/results/` with date and commit
hash, machine-readable.

**Datasets**: MS MARCO passage for lexical; BigANN/SIFT/GIST subsets and an embedding
set (e.g., Cohere Wikipedia) for vectors. Small deterministic fixtures for CI; full
runs on demand.

**The metrics that matter**, in order: (1) cold end-to-end open cost — GETs and bytes
from pointer read to first planned query against S3-class latency, broken out as
manifest resolution + segment opens + cold-fetchable waves (targets: hotcache open
under 100 ms and ≤2 round trips per segment, within the cold-open byte budget, with
the manifest adding at most two round trips ahead of them); (2) cold and warm query
latency p50/p99 for BM25 top-k, ANN top-k, and hybrid RRF, with GETs and bytes-read
per query reported alongside — and warm reported both ways, against a cached snapshot
and with a pointer-freshness check, per §7; (3) index size vs the same data in tantivy
and Lucene; (4) build and merge throughput, per merge strategy of invariant 1, plus
segment-count amplification at M3; (5) score parity per invariant 5's
profile-based definition; (6) bytes-fetched vs bytes-used, the read-amplification
column of §6.

**Named baselines**: tantivy (same-process lexical), Lucene (small JVM harness, parity
within norm quantization), Parquet-plus-brute-force (what the index buys). Regressions
against our own previous results fail CI beyond a stated tolerance. Object-storage
behavior is tested against local MinIO with injected latency so round-trip counts are
asserted in CI, not measured ad hoc.

**turbopuffer is an internal benchmark target, not a rival.** The honest sentence v1
lacked, now stated as doctrine: *a format can beat their open cost; only an engine can
beat their steady-state query latency; we are not building an engine.* Their sub-10ms
warm p50 is a property of a caching fleet, a WAL-tail freshness path, and a query
planner, none of which a format spec can touch. Published figures we measure against
live in Appendix A; every comparison report states our number, theirs, hardware,
dataset, date, and the caching-fleet asymmetry. The word "beating" does not appear in
headings or docs.

**Benchmark engines and adapters.** The strongest benchmark of a format holds the
engine constant and varies only the bytes: the same traversal, scoring, and query
code reading its native index versus reading STRAND, so any delta belongs to the
layout. Every borrowed-engine result names the algorithm layer it ran through
(harness, fork, adapter, DataFusion, PISA, FAISS); a number without its engine
named is not a result. Nearest grave, per §3: an adapter fork that outlives its
usefulness or dies with its maintainer is Indri in miniature — infrastructure
nobody is economically forced to keep alive — which is why every adapter below is
pinned, read-only, and disposable by rule.

Two rules govern all adapter results:

*Engine-constant means one binary, two formats.* A fork or adapter claims
"same engine" only if the engine's native read path is retained intact and the
format is selected at open time — same parser, scorer, collector, threads,
allocator, binary; only the bytes differ. A fork that loses the ability to read
its native format has silently become a cross-engine comparison and its
engine-constant results are void. Every engine-constant report pins the engine
commit hash, corpus and document insertion order, analyzer descriptor (both sides
passing the same invariant-6 conformance vectors), scoring path, top-k policy,
thread counts, machine, and cold/warm protocol, and reports bytes read and GETs
alongside latency, plus a decode-isolation microbenchmark (codec decode on
identical postings outside the engine) separating layout cost from integration
cost. For true adapters (FAISS, Quickwit), adapter overhead is additionally
isolated with an identity-backend measurement — the adapter serving
fully-resident data — so a layout win is not confused with an adapter tax, and
vice versa.

*Build equivalence precedes timing.* The compared indexes must be the same
logical index: built once on the native side, converted losslessly by
`strand-tools`, with the harness asserting — before any latency is recorded —
identical term and collection statistics, score parity within the invariant-5
tolerance on a probe query set, and, for vectors, bit-identical codes and
identical top-k results at fixed nprobe. A comparison whose equivalence gate did
not pass is not a result.

*Harnesses are not products.* The in-repo `bench/` harness stays deliberately
naive — a plain top-k heap over the batch API, no clever pruning — because a dumb
harness measures the format and a clever one measures its own cleverness.
Sessions do not "improve" the harness into an engine.

*Adapters are second readers.* Every adapter implements the format's read path in
a foreign codebase and therefore MUST pass the `conformance/` golden files. This
gives the tantivy fork standing as the primary M4 second reader and puts
invariant-11 force behind every adapter; the FAISS adapter is additionally the
format's first non-Rust reader, with everything that implies.

Lexical adapters, in priority order:

**tantivy fork.** tantivy (MIT, verified byte-level 2026-08-18; LICENSE vendored
at M0, and the fork carries tantivy's MIT notices alongside our Apache-2.0
additions) has a storage-level `Directory` abstraction but, per current
understanding, no pluggable codec SPI — a pending claim R11(a) settles by mapping
the actual reader surface a STRAND path must replace; note the asymmetry that a
discovered SPI shrinks the fork rather than killing it. The fork is read-only and
triple-purpose: the engine-constant lexical benchmark, the primary M4
second-reader path, and an adoption artifact ("the same tantivy, reading a
neutral format"). It pins one tantivy commit, stated in every report; it does not
chase upstream HEAD; it never grows a write path (indexes are built by
`strand-tools` conversion); it is never published or promoted as a usable search
engine. In-scope changes are confined to a reader-module list pinned in the R11
RFC. The fork is declared a failed experiment — and M4 reverts to the clean-room
path — if any of the following fires: it cannot pass the frozen v0.1 conformance
manifest and the invariant-5 parity gate within 10 sessions of fork start; fork
maintenance (excluding first bring-up) exceeds 15% of a milestone's commits in
two consecutive milestones, measured from the §3 commit log; or correct reads
require modifying files outside the pinned reader-module list, which falsifies
the engine-constant premise itself.

**Lucene `StrandCodec`.** Lucene's codec SPI makes this a plug-in, not a fork
(exact codec class surface confirmed in R11(a)). It extends the already-planned
JVM parity harness into an engine-constant JVM-side benchmark and carries the
invariant-5 Lucene-parity profile test.

**Quickwit adapter (optional, after the tantivy fork exists).** Hypothesis, to be
tested by R11(c) against post-relicense internals rather than assumed: Quickwit's
read path is close enough to tantivy's that the adapter inherits most of the
fork. Its unique payoff if true: Quickwit's internal hotcache cold-open versus
STRAND's specified one, same engine — the fairest test of the §4 hotcache
lineage claim.

**PISA via CIFF (M4).** The STRAND→CIFF export lets PISA's MaxScore/WAND/BMW
implementations query STRAND-derived indexes with zero adapter code; algorithm
layer named as PISA.

Vector and fusion adapters:

**FAISS inverted-lists adapter.** FAISS (MIT, verified byte-level 2026-08-18;
LICENSE vendored at M0) exposes an inverted-lists extension point
(`faiss/invlists/InvertedLists.h`, with `OnDiskInvertedLists` as prior art), and
`IndexIVFRaBitQ` runs on the generic `IndexIVF` list machinery — so a
STRAND-cluster-blob-backed `InvertedLists` plausibly gives an engine-constant
vector benchmark **for the multi-bit path**. The 1-bit path is FastScan, and
FAISS's FastScan indexes run over `CodePacker`-packed block lists
(`BlockInvertedLists`), a different byte contract; whether external storage can
feed that path, and at what load-time repack cost, is R11(b)'s question, and
until it answers, the engine-constant vector claim extends to multi-bit only,
with the 1-bit path benchmarked by the naive harness. STRAND blobs MUST NOT
adopt FAISS's packed layout as a wire format — that would bake one engine's
register-shuffle layout into wire bytes against invariant 10; any repack is
adapter-side, charged to load, and stated. Fairness protocol: train once in
FAISS; export centroids and codes bit-exactly into the blob; pass the
build-equivalence gate; then run three legs — native lists in memory, adapter
with the blob fully resident (the identity backend), and adapter over MinIO —
with the MinIO leg reported as the format's cold story, never in the same column
as the memory legs; the adapter holds no cross-query cache the native
configuration lacks. FAISS was already the M2 baseline; this upgrades it from
published-numbers baseline to engine-constant adapter.

**Warm-tier graph benchmark host (optional, decided in R11(e)).** The graph blob
family needs a traversal host for warm benchmarks; candidates are hnswlib-class
libraries or a Rust HNSW crate. Graph engines built around their own storage
(Qdrant, Weaviate) are published-numbers baselines, not adapter targets.

**Hybrid fusion host: the M5 DataFusion TableProvider.** RRF across the lexical
and vector blob families over one row-ID space runs as a DataFusion query. This
is a *format* benchmark — it proves the row-ID fusion contract and prices hybrid
retrieval's I/O under a naive planner — and it is not an engine benchmark. The
workload that keeps it representative: a Roaring metadata filter swept across
selectivities (0.1%, 1%, 10%, 100%), BM25 top-100, ANN top-100, RRF (k=60)
top-10, with fused-list recall reported against exhaustive ground truth at each
selectivity — the sweep is mandatory because filtered ANN is the hard case for
cluster-shaped indexes and omitting it would be read, correctly, as hiding it.
OSS hybrid engines (Vespa, Milvus, Weaviate, OpenSearch) appear as
published-numbers context with their machinery named, never in a shared results
table with ours; adapting any of them is out of scope — their size makes a fork
an engine project, which §1 forbids.

---

## 9. Repository shape

Only `CLAUDE.md` and `LICENSE` exist at the start; everything else below is created at
M0 or at its first milestone. The shape is fixed now so sessions put things in the
right place from the first commit.

```
strand/
  CLAUDE.md            this file (exists)
  LICENSE              Apache-2.0 (exists)
  spec/                normative spec, one file per layer (created per milestone):
                       container, manifest, row-ids, lexical, block-max,
                       vectors (tiers + families), deletion, analyzers,
                       scoring-profiles, registry
  rfcs/                numbered design RFCs; status: draft/approved/implemented
  docs/
    lineage.md         the prior-art map (created at M0 from §4, with links)
    research/          the full R1–R8 kickstart report and R9+ memos, vendored
                       at M0; Appendix C is the condensed form sessions use
                       until then
  references/          vendored primary sources (populated at M0 from
                       Appendix C's source lists — the §2 source-sentence rule
                       lives here)
  crates/
    strand-core/      types, row-ID logic, container/chunk/block read-write,
                       manifest commit protocol + safety rules, batch-shaped
                       reader traits, normative scalar kernels
    strand-lexical/   BP128 postings (FastPFOR registered), FST term dict,
                       block-max, Roaring
    strand-vector/    flat vectors, RaBitQ codecs (kernel per bit-width,
                       rotation provenance), cluster-family blobs, warm-tier
                       graph blobs
    strand-tools/     CLI: inspect, verify, convert (tantivy/CIFF importers),
                       orphan sweep (M3)
  bench/               harness, datasets manifest, results/
  conformance/         golden files + language-neutral test manifest, including
                       conformance/analyzers/ token-stream vectors (normative),
                       so a second implementation can verify itself without our
                       code; golden files pin uncompressed structures per
                       invariant 11
```

Rust conventions: edition 2024, `cargo clippy -- -D warnings` clean, explicit types on
public APIs, no unused assignments, `unsafe` only in reviewed, documented blocks. SIMD
per invariant 9: `wide`/`pulp` with runtime dispatch, scalar-equivalence property
tests in CI, no nightly features.

---

## 10. Session workflow

1. **Orient.** Read this file, the relevant `spec/` chapter, and the RFC you are
   implementing. If the task has no approved RFC and touches format design, your
   deliverable is the RFC draft, not code.
2. **Ground.** Pull the primary sources you depend on from `references/` (or fetch and
   vendor them; Appendix C lists what to fetch per track). Cite them. Never rely on
   memory for another format's byte layout — §3 lists this project's own
   demonstrations of why.
3. **Arithmetic.** If the work touches a cold path, write the §8 napkin math before
   the design, in the RFC, end-to-end from the pointer read, against the pinned
   figures and the cold-open byte budget.
4. **Implement narrow.** One RFC, or one coherent slice, per session. Include a worked
   byte-level example and the tests that pin it.
5. **Prove.** Round-trip property tests for anything serialized. Fuzz targets for
   every reader. Scalar-vs-SIMD equivalence tests for any kernel touched. The
   invariant-11 checklist answered for any new wire structure. Benchmarks updated if
   the change plausibly moves a §8 metric.
6. **Record.** Commit message: what changed, which RFC, the prompt/task that drove the
   session. Update RFC status and `docs/lineage.md` if new prior art was consulted.

Definition of done for any spec chapter: the golden files in `conformance/` exist, the
reference implementation passes them, and the chapter's prose passes the §2 test.

---

## 11. Milestones

**M0 — Container + manifest.** Container spec chapter, chunk/block split, storage-class
and alignment attributes, invariant-11 byte-determinism pins (endianness, chunk codec,
checksums), footer/hotcache layout, blob registry, row-ID chapter with per-family
merge-semantics declarations, **manifest chapter with the CAS commit protocol and the
§7 safety rules (declared CAS host, deletion-safety retention, reader 404-refresh,
orphan rule)**. `strand-core` read/write with batch-shaped readers from the start;
`strand-tools inspect`. Golden files. Vendor `references/` and `docs/research/` from
Appendix C. Benchmarks and tests: cold end-to-end open (pointer → planned query) GET
count and latency against MinIO; parallel-wave aggregate throughput (confirms or
replaces the 100 MB budget rationale); manifest commit contention (two writers racing
the pointer); crash tests (writer dies before commit → orphans, compaction deletes a
file under a retained snapshot → must be impossible, reader on expired snapshot →
404-refresh path); the measured tail-latency figure that confirms or replaces §8's
placeholder.

**M1 — Lexical.** BP128 postings + positions + FST term dictionary + block-max
sibling blob + Roaring filter bitmaps. The R2 RFC pins the exact d-gap variant
(invariant 11) and the block-max RFC pins the raw-statistics fields (invariant 4).
The **scoring-profiles chapter** defines the `bm25` profile normatively and the
Lucene-parity profile. **Analyzer descriptor schema and the normative token-stream
vectors in `conformance/analyzers/` are gating deliverables of this milestone, not
metadata afterthoughts** — without them invariant 6 is a label. Tantivy importer. R2
codec bake-off lands here and confirms or swaps the postings default (including
verifying tantivy's actual current codec, per §6); the R9 layout evaluation and
license audit MUST complete before the bake-off freezes the default, since a
FastLanes outcome changes both the default and the block granularity. Benchmarks: MS
MARCO BM25 latency and size vs tantivy; Lucene parity per invariant 5;
bytes-fetched vs bytes-used across term frequency deciles (the §6 read-amplification
number). Adapter-based results appear in this milestone's report only after R11
verifies the respective extension point and the build-equivalence gate passes; until
then, harness and published-numbers baselines only.

**M2 — Vectors, cluster-first.** Flat vector blob; RaBitQ codecs with kernel-per-
bit-width, the rotation descriptor field, and the rotation-provenance mechanism
(invariant 11); the **cluster-family cold-native blob** (navigation tier + wholesale
posting lists + rerank region) per the R1 RFC, with all posting-list offsets
resolvable from the navigation tier (invariant 3's one-wave rule), the replication
knob and tier-1 sizing limits in blob metadata, computed against §8's cold-open byte
budget. The warm-tier graph blob family (persisted-permutation node order, ordering
algorithm per R1's evidence) is in-scope but explicitly second. Benchmarks: cold and
warm ANN recall/latency with GET counts asserted; codec comparison RaBitQ vs
PQ-FastScan; the cold target to measure against is turbopuffer's published figures
(Appendix A), with the asymmetry stated. Adapter-based results appear in this
milestone's report only after R11 verifies the respective extension point and the
build-equivalence gate passes; until then, harness and published-numbers baselines
only.

**M3 — Hybrid + deletes + merge.** Deletion vectors; compaction implementing the
per-family merge semantics of invariant 1 (concatenate+remap for cluster blobs,
rebuild for graph blobs, rebalance for centroids), respecting §7's deletion-safety
rule, with merge cost benchmarked per strategy; the orphan-sweep tool; end-to-end
hybrid RRF across both blob families over one row-ID space. **The multi-segment
benchmark**: the same corpus at 1, 16, and ~128 segments, cold and warm, so
segment-count amplification is a measured curve feeding R10. Deliverable renamed from
v1's "head-to-head": **a benchmark report measured against published figures, with
the caching-fleet asymmetry stated.**

**M4 — Interchange + independence.** CIFF importer (lossless where CIFF permits);
conformance manifest frozen at spec v0.1. **Second-reader parity must be real
independence**: an external contributor implementing from `conformance/` alone, or a
clean-room session given only `spec/` and `conformance/` with the Rust crates
withheld — this is also the acceptance test for invariant 11: two implementations,
same logical input, same index. If a stranger cannot implement from the conformance
manifest, the spec failed §2's test regardless of CI. Puffin blob-type packaging RFC.
The tantivy fork is the named primary second-reader path, built against the frozen
v0.1 conformance manifest, never against a moving spec; the clean-room option remains
the fallback and activates if any R11(d) failure trigger fires. Lucene `StrandCodec`
lands here as the JVM parity vehicle.

**M5 — The consumer.** A thin, read-only **DataFusion TableProvider** over STRAND
segments — the answer to "name the second engine," written into scope on purpose
because the research concluded no forced reader exists and one must be built. Slices
of it should track earlier milestones (reading lexical blobs as M1 lands) so the spec
is stress-tested by a consumer while it can still change cheaply; M5 is where it
becomes a supported, benchmarked artifact. Without this milestone the project is
Indri with better licensing, and we have chosen not to be that in writing. The
TableProvider is additionally the hybrid-fusion benchmark host, running the §8 fusion
workload with its selectivity sweep. The FAISS adapter lands alongside M2's
benchmarks or here, per R11(b)'s feasibility finding.

Each milestone ends with a short written report in `docs/`: the numbers, what they
mean, what they changed. A benchmark that embarrasses us goes in the report with an
analysis, not in the memory hole.

---

## Appendix A — turbopuffer benchmark targets (internal)

All figures are turbopuffer's own published claims, to be vendored at M0; none are
independently verified. Every comparison report cites the source sentence, states our
independently measured number, hardware, dataset, and date, and repeats the asymmetry:
their warm numbers are a caching fleet's — with a WAL-tail freshness path and a query
planner — not a format's. A format-based reader pays an explicit pointer round trip
for the freshness their engine gets from its own machinery (§7); warm comparisons
state which variant of ours is being compared.

Reference points:
- ~100ms per object-storage round trip (their stated first-principles figure); cold
  queries budgeted at 3–4 round trips, "often as little as ~400ms". This is the
  *structured* cold path — metadata, then filter/centroid indexes + WAL tail, then
  clusters — and note it includes their metadata trip, which is why §8 counts ours.
- Architecture page (fetched 2026-08-17): first query to a truly cold namespace is
  **p50 = 874ms** for 1M documents; subsequent cached queries **p50 = 14ms**. The
  874ms true-cold figure — not the ~400ms structured-path figure — is the number our
  cold-open story actually competes with, and the more beatable of the two.
- Published p90s from their benchmark post: 1M × 768d vectors (3GB): 444ms cold /
  10ms warm. 1M docs BM25 (300MB): 285ms cold / 18ms warm.
- Batched-iterator fix: production query 220ms → 47ms; scan benchmark 6.5ms → ~110μs
  on 100k values ("60× faster than before", verbatim); 512-value batches.
- Scale claims (unverifiable, context only): 1T–2.5T documents, 10M+ writes/s,
  25k+ queries/s fleet-wide; ANN v3: 200ms p99 at 1k QPS over 100B vectors. The
  ANN v3 number in particular is an engine-global hierarchical index over the whole
  corpus — the capability R10 asks whether a manifest can even gesture at.

What the format can legitimately target: their cold numbers, via the ≤2-RTT segment
open, end-to-end manifest accounting, and chunk-shaped vector tiers. What it cannot:
their warm p50, which belongs to an engine.

## Appendix B — settled vs open ledger

**Settled (apply, do not re-litigate):** chunk-shaped cold access with the one-wave
addressability rule (invariant 3); end-to-end cold accounting from the pointer read
(§8); cluster-family as the cold-native vector shape; graph blobs as warm-tier;
RaBitQ default with kernel-per-bit-width and rotation descriptor; Roaring; FST
default; batch-shaped API shape; scalar-normative kernels with SIMD equivalence
testing; per-blob storage class with dense wire bytes; persisted node-order
permutation; per-family merge semantics; the manifest CAS protocol shape with one
declared CAS host; the deletion-safety rule and reader 404-refresh; the orphan-file
rule; snapshot-at-load reader consistency with priced freshness; scoring profiles as
the parity mechanism, with Lucene parity within norm quantization as a profile;
block-max bounds as raw scoring-independent statistics (invariant 4's principle);
normative analyzer conformance vectors; byte determinism (invariant 11: little-endian,
UTF-8 byte-order term sort, declared chunk codec with zstd default, declared
checksums with xxHash3-64 default, complete codec registrations,
stochastic-transform provenance, uncompressed golden files); the napkin-math rule and
its pinned ~100ms planning figure; the provisional 100 MB cold-open byte budget with
its parallel-fetch rationale and expiry condition; per-segment scope with reported
segment-count amplification; M5's existence; engine-constant benchmarking as the
preferred end-to-end method, under the one-binary/two-formats rule; the
build-equivalence gate as a precondition for any adapter result; harness-stays-naive;
results name their algorithm layer; adapter overhead isolated by identity-backend
measurement plus decode-isolation microbenchmark; adapters MUST pass conformance
golden files; the fork is read-only, pinned, reader-layer-only, and never published
as a product; no FAISS packed layout in wire bytes; the fusion benchmark is a format
benchmark with the mandatory filter-selectivity sweep, engine numbers context-only;
tantivy and FAISS licenses MIT (verified byte-level 2026-08-18; vendor at M0).

**Open (RFC required; grounding in Appendix C):**
- **R1** — the cluster blob's concrete layout and tier-1 sizing law vs segment scale
  and replication (gates M2/M3). Kill criterion, falsifiable: if tier-1 exceeds §8's
  provisional 100 MB cold-open byte budget — or its measured M0 replacement — by more
  than ~4× at target segment scale, cold vector search is narrower and the mission
  sentence changes again. Also: the graph-blob ordering algorithm (Starling's block
  shuffling is the literature; pick with evidence).
- **R2** — postings default confirmation BP128 vs FastPFOR vs FastLanes on real
  corpora, including verifying tantivy's actual current codec (gates M1, load-bearing
  for §6's default argument); the exact d-gap variant to register (invariant 11);
  the block-max raw-statistics fields (invariant 4); recommended batch-size range;
  current Lucene codec class names for `docs/lineage.md`.
- **R3** — the rotation-provenance mechanism (materialized matrix vs generator+seed,
  M2 RFC); TurboQuant revisit condition.
- **R4** — precise Lucene-vs-tantivy doc-length accounting for the invariant-6 length
  definition (needs primary-source confirmation).
- **R5** — exact GCS/Azure conditional-write header semantics (confirm at spec time).
- **R9** — compute-native block layout (gates the R2 bake-off and therefore M1):
  measure FastLanes against hand-vectorized BP128 and FastPFOR on postings
  distributions (the margin is unmeasured — §6), audit the cwida/FastLanes license
  against Apache-2.0-only, reconcile 1024-value granularity with the block-max
  sibling design (1024-native or nested 8×128, preserving invariant 4), and assess
  ALP for the flat float-vector blob and the GPU decode path for raw-mappable blobs.
- **R10** — cross-segment scale: should the manifest carry optional per-segment
  summary metadata (term-statistics sketches, centroid summaries, min/max-style
  pruning stats) so a reader can prune segments before opening them, and what does
  target segment size look like when the 100 MB budget pushes segment count up while
  open-amortization pushes it down? Fed by the M3 multi-segment benchmark; any
  summary blob must stay index-internals-agnostic per §7.
- **R11 — Adapter feasibility audit (gates all adapter work).** Verify against
  current source, not memory: (a) tantivy's reader surface — map the modules a
  STRAND read path must replace, settling the codec-SPI question as a by-product,
  and confirm the exact Lucene codec SPI class surface for `StrandCodec`; (b) FAISS
  per-kernel feasibility — whether the generic `InvertedLists` path serves
  `IndexIVFRaBitQ` search over external storage, and whether the FastScan path can
  run over external lists at all given its `CodePacker`/`BlockInvertedLists`
  packing, including the load-time repack cost if so; (c) Quickwit split/hotcache
  internals post-relicense, testing the inherits-from-the-fork hypothesis; (d) the
  fork reader-module list that arms the §8 failure triggers; (e) the warm-tier graph
  host choice. Adapter milestones are conditional on R11.
- **Postings block size** (conditional since v2.2): 128 was the default by shared
  lineage, now conditional on R9's granularity outcome — the block-max sibling-blob
  *pattern* stays settled (invariant 4), only the granularity number is open.
- **Pending figures:** the ~250ms p90 tail figure — re-locate the source sentence or
  replace with the M0 measured figure (§8); the parallel-wave aggregate throughput
  behind the 100 MB budget rationale — measure at M0; tantivy codec-SPI absence
  (R11(a)); FAISS FastScan external-list feasibility (R11(b)); the Quickwit
  inheritance hypothesis (R11(c)).

## Appendix C — Research grounding (condensed)

This appendix replaces external research documents so this file stands alone. For
each track: the findings the constitution depends on, and the primary sources to
vendor into `references/` at M0. The full kickstart report is vendored into
`docs/research/` at M0; until then, this is the grounding sessions cite.

**R1 — Cold vector search over object storage.** The entire cloud-native evidence
base converges on cluster-shaped indexes for cold object-storage access: turbopuffer's
architecture states ~100ms per round trip from first principles, uses an
SPFresh/centroid index, downloads the centroid index cold, and fetches each cluster's
range in one massive (parallel) round trip; the 2026 survey and its companion
benchmark independently conclude cluster indexes' fetch granularity and lack of
intra-query dependencies fit object storage where graph beam search — a dependent
chain of 50–200 fetches, i.e. 5–20 seconds at 100ms — does not, and SSD-era rescues
(Starling's block shuffling, PipeANN's pipelining) operate at microsecond scales that
do not transfer. The sizing law: 1-bit RaBitQ codes cost dims/8 bytes per vector
(96/128/192 B at 768/1024/1536 dims), so tier-1 runs ~100 MB per million 768d
vectors; SPANN uses up to 8 closure replicas per vector (13.0 GB vs 7.5 GB index at
replica 8 vs 2 on GIST1M in the benchmark), and finer centroid granularity helped
I/O-congested setups by up to 3.14× QPS — replication and granularity are first-class
knobs. Sources: SPANN (Chen et al., NeurIPS 2021, arxiv.org/abs/2111.08566); SPFresh
(Xu et al., SOSP 2023, doi 10.1145/3600006.3613166); DiskANN (Subramanya et al.,
NeurIPS 2019); Starling (SIGMOD 2024); cloud-native survey (Song et al.,
arxiv.org/abs/2601.01937); benchmark (Li et al., arxiv.org/abs/2511.14748);
turbopuffer.com/docs/architecture, /blog/ann-v3, /blog/turbopuffer; AWS S3 Vectors
docs (internal layout undisclosed — treat all specifics as unverified); Lance vector
index format (lance.org/format/index/vector/).

**R2 — Postings codec.** Lemire & Boytsov (SPE 2015) is canonical: SIMD-BP128 is the
fastest pure bit-packing scheme (~2500 mints/s; S4-BP128-D4 as low as 0.7 cycles/int)
and the simplest from-spec reader (no exception stream), at up to ~2 bits/int and
5–15% worse ratio than SIMD-FastPFOR, whose exception streams buy ratio at decode and
implementation cost. Lucene ships block PFOR (ForUtil/PForUtil lineage, 128-int
blocks) and tantivy is believed to ship exception-free bitpacking — both stated from
prior knowledge and **unverified against current source**; verify when vendoring. The
turbopuffer batched-iterator post is exact: 512-value batches, scan benchmark 6.5ms →
~110μs on 100k values, "60× faster than before" verbatim, production query 220ms →
47ms. Sources: arxiv.org/abs/1209.2137 (SPE 2015, doi 10.1002/spe.2203); Lemire,
Boytsov, Kurz, "SIMD Compression and the Intersection of Sorted Integers"
(boytsov.info/pubs/simdcompressionarxiv.pdf); turbopuffer.com/blog/zero-cost; current
Lucene ForUtil/PForUtil and tantivy postings source.

**R3 — Quantization.** Extended-RaBitQ (SIGMOD 2025) is the de-facto industry choice
— Milvus, Faiss, Elasticsearch/Lucene ("BBQ", which removes the random rotation),
turbopuffer, CockroachDB, VectorChord, Volcengine — and all three reference
repositories are Apache-2.0 (verified via GitHub's license detection plus third-party
corroboration; byte-for-byte header reads were blocked, noted as a caveat). The 1-bit
path uses FastScan-style LUT machinery; the multi-bit path computes distances exactly
as classical scalar quantization, with 4/5/7-bit typically reaching 90/95/99% recall
without reranking — two different kernels the spec must pin by bit-width. The
TurboQuant dispute (ICLR 2026) is procedural: ICLR took no action, Google agreed to
correct the arXiv text, and TurboQuant's published wins are on KV-cache, not embedding
workloads. Sources: Gao & Long, RaBitQ, SIGMOD 2024; Gao et al., Extended-RaBitQ,
arxiv.org/abs/2409.09913; github.com/VectorDB-NTU/RaBitQ-Library (and Extended-RaBitQ,
gaoj0017/RaBitQ); TurboQuant (openreview.net/pdf?id=tO3ASKZlok); the Gao rebuttal and
the Milvus interview on the dispute.

**R4 — Analyzer conformance.** Token streams genuinely diverge across versions: UAX
#29 is explicitly not stable, and Unicode 9.0 changed U+202F NARROW NO-BREAK SPACE's
Word_Break class (PRI #308) — same input, different token boundaries. CJK/Thai/Lao
segmentation is dictionary-defined and thus implementation-defined, so the dictionary
identity must be pinned. No engine ships a cross-engine conformance-vector suite;
a 2023 CIFF follow-up documents tokenization consistency as an unsolved interop
problem — which is why our golden vectors are normative, not advisory. Pending:
precise Lucene-vs-tantivy doc-length/norm accounting differences need each project's
similarity docs. Sources: unicode.org/reports/tr29/; PRI #308 background
(unicode.org/L2/L2015/15295-pri308-bkgnd.html); Hiemstra et al. 2023
(djoerdhiemstra.com/wp-content/uploads/ossym2023.pdf); unicode-rs/unicode-segmentation;
Lucene analysis Version docs; ICU/CLDR versioning docs.

**R5 — Manifest and commit.** Iceberg proves the minimal model: immutable versioned
metadata, one current pointer, compare-and-swap on the pointer, giving optimistic
concurrency and snapshot isolation; readers use the snapshot at load. S3 conditional
writes make it catalog-free: If-None-Match (create-if-absent) GA in all regions
August 20, 2024; If-Match ETag CAS November 26, 2024. Lance demonstrates the
index-aware, index-internals-agnostic manifest shape (indices metadata in a versioned
manifest, committed directly to object storage). CIFF's failure shows what omitting
this layer costs: a directory of files without an atomic table layer forces engines to
infer state, and interop dies there. GCS generation-match and Azure ETag conditionals
are long-standing equivalents — exact header semantics to confirm at spec time.
Sources: iceberg.apache.org/spec/; Vanlightly's Iceberg consistency analysis; the two
AWS conditional-writes announcements (Aug and Nov 2024); lance.org/format/table/.

**R6 — The second engine.** No forced reader exists; one must be built. The realistic
host is the DataFusion ecosystem (ParadeDB embeds tantivy in Postgres; LanceDB
integrates DataFusion and already separates index blobs from the table format);
Quickwit relicensed to Apache-2.0 under Datadog, its split format open but its
community stewardship uncertain. The history: Parquet and Iceberg were adopted because
cost pressure preceded the format; CIFF (explicitly exchange-only, "speed… not
important concerns") achieved research replicability but no production interop;
BitFunnel and Pilosa built no ecosystems. Consequence baked into this constitution:
minimize second-reader cost (simple default codec, normative conformance vectors) and
build the consumer (M5). Sources: CIFF (Lin et al., SIGIR 2020,
arxiv.org/abs/2003.08276; github.com/osirrc/ciff); Datadog/Quickwit announcements;
ParadeDB pg_search; lance.org/format/.

**R7 — Compaction and merge.** Graph indexes are effectively rebuild-on-merge (global
neighbor structure does not compose); IVF/SPANN posting lists concatenate and remap
cheaply under stable row-IDs — the load-bearing reason for invariant 1's per-family
merge strategies and for stable 64-bit IDs. SPFresh's LIRE does incremental
rebalancing with, verbatim, "only 1% of DRAM and less than 10% cores needed at the
peak" vs global rebuild at billion scale, and shows static SPANN centroids degrade
under drift (updating one-third of vectors costs more than a point of recall and 4×
tail latency) — motivating the rebalance strategy for centroid layers. Lance and
Milvus corroborate deletion-vector soft deletes with deferred physical removal.
Sources: SPFresh (doi 10.1145/3600006.3613166); FreshDiskANN; "In-Place Updates of a
Graph Index for Streaming ANN" (arxiv.org/abs/2502.13826); Lance deletion-vector
docs.

**R8 — Rust SIMD policy.** portable_simd remains nightly-only with no stabilization
in sight (tracking issue #364 lists unresolved API questions); the stable options are
`wide` (no multiversioning) and `pulp` (built-in multiversioning, powers faer), with
the `multiversion` crate for runtime dispatch. Autovectorization is fragile across
rustc/LLVM versions and abstraction boundaries — the turbopuffer post is the
production demonstration. The documented best practice, adopted as invariant 9: a
normative scalar reference plus property-based scalar-vs-SIMD equivalence tests.
Sources: Shnatsel, "The state of SIMD in Rust in 2025"
(shnatsel.medium.com/the-state-of-simd-in-rust-in-2025-32c263e5f53d);
github.com/rust-lang/portable-simd (charter, issue #364); wide/pulp/multiversion
crate docs.

**R9 — Compute-native block layout (FastLanes).** FastLanes (Afroozeh & Boncz, VLDB
2023) targets a virtual 1024-bit SIMD register with a unified transposed tuple order:
the same wire bytes decode at full data-parallelism on any lane width, and the
portable scalar decoder auto-vectorizes by construction to >40 values per CPU cycle
across Intel, AMD, Apple, and AWS chips — the existence proof behind invariant 10's
claim that compute-native and ISA-portable are compatible. Its headline comparisons
are against scalar baselines; its margin over hand-vectorized BP128 on postings
distributions is unmeasured, no inverted-index application exists in the literature,
and the cwida/FastLanes license is unaudited — the three gates the R9 RFC must clear,
alongside reconciling 1024-value granularity with the block-max sibling design. ALP
(SIGMOD 2024) is the companion float codec relevant to the flat vector blob; the
DaMoN '24 paper gives the GPU decode path for raw-mappable blobs. Sources: FastLanes
(vldb.org/pvldb/vol16/p2132-afroozeh.pdf); ALP (SIGMOD 2024); FastLanes-on-GPU
(DaMoN '24); github.com/cwida/FastLanes (license audit target).

**R10 — Cross-segment pruning (no research report yet).** The question: what minimal,
index-internals-agnostic summary metadata could the manifest carry so a reader prunes
segments before opening them, and what segment size balances the 100 MB cold-open
budget against per-segment open cost at 100M–10B rows? Nearest prior art to survey:
Quickwit's split pruning via tags and timestamp ranges; Iceberg's manifest-level
partition and column statistics; turbopuffer's ANN v3 engine-global hierarchical
clustering (the capability ceiling a format-level answer is measured against). Input
data: the M3 multi-segment benchmark curve. Deliverable: an RFC that either specifies
an optional summary blob class or states the honest scale ceiling of a summary-free
manifest.

**R11 — Adapter feasibility (partial grounding; audit open).** Verified
2026-08-18 against live source: tantivy and FAISS are both MIT (byte-level
LICENSE reads, `quickwit-oss/tantivy`, `facebookresearch/faiss`); FAISS ships
the inverted-lists extension surface at `faiss/invlists/` (`InvertedLists.h`,
`OnDiskInvertedLists.h`, `BlockInvertedLists.h`); `IndexIVFRaBitQ` extends
`IndexIVF` and its FastScan variant (`IndexIVFRaBitQFastScan` →
`IndexIVFFastScan`) initializes a `CodePacker` in its inverted lists and retains
an `orig_invlists` pointer — the source basis for splitting the adapter claim by
kernel. Unverified and owned by R11: tantivy's reader surface and codec-SPI
absence; whether FastScan search can execute over externally hosted lists;
Quickwit post-relicense internals; the warm-tier graph host. Sources to vendor
at M0: both LICENSE files; `faiss/invlists/InvertedLists.h` and
`OnDiskInvertedLists.h`; `faiss/IndexIVFRaBitQ.h`, `faiss/IndexIVFFastScan.h`,
`faiss/IndexIVFRaBitQFastScan.h`; tantivy segment-reader sources at the pinned
commit; Quickwit split/hotcache sources post-relicense.
