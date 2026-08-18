# CLAUDE.md — Project Constitution

**STRAND** — **S**parse-**T**erm **R**etrieval **A**nd **N**earest-neighbor
**D**ense-vector. The name is the mission statement: one format carrying both
retrieval families over one row-ID space, the strands woven together at fusion time.

This file governs every Claude Code session in this repository. Read it fully before
acting. When an instruction here conflicts with anything else, this file wins. The
detailed reference material this file draws on — design lineage, the data-structure
baseline, benchmark and adapter detail, per-milestone deliverables, the settled/open
ledger, and the condensed research grounding — lives in `docs/`, cited by pointer
throughout. The full research report and the vendored primary sources are brought into
`docs/research/` and `references/` at M0.

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

The mission sentence: *CIFF you can query in place on S3, extended to
vectors — where "in place" means chunk-shaped access: a small, bounded number of large,
independent fetches, never dependent pointer-chasing.* Cold vector search in v0.1 is
cluster-shaped. Graph indexes are in the format as a warm-tier blob family and are
explicitly not the cold-open story. The v0.1 cold story is additionally **per-segment**:
the format makes each segment cheap to open and query cold, and reports segment-count
amplification honestly (§8); a manifest-level cross-segment navigation layer is open
research (R10), not a v0.1 promise. This is a narrower claim, deliberately, and it is
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
  already caught real errors in drafting this document — embellished figures, an
  uncorrected cold-path latency figure, an unmeasured throughput claim, flawed
  single-stream bandwidth arithmetic, and a conflation of two distinct FAISS kernels —
  and it stays.

Test for every document: could a competent engineer who has never seen this repo read
it once and implement against it? If not, rewrite.

---

## 3. Working method (lessons from prior AI-built OSS)

The closest successful precedent is Cloudflare's `workers-oauth-provider` — a
production OAuth 2.1 library largely written by Claude in 2025. Its lessons, sharpened
by our own review cycles:

**Agent designs, agent implements — but not in the same breath.** Format design
decisions land in RFCs, and an RFC earns "approved" status by passing its own
adversarial review (the "how this could be wrong" section below, argued out and
resolved) before any session implements it — design and implementation are separate
passes, even when the same agent does both. Never invent a format decision mid-session
inside an implementation task. If implementation reveals a design problem, stop, write
it up in the RFC's Discussion section, and resolve it there — through the same
adversarial review — before continuing.

**Provenance in the commit log.** Each commit message states the task or prompt that
produced it. This repo should be readable as a record of how an AI-built format
actually got built.

**The model's memory of standards is not a source.** The sharpest external criticism of
the Cloudflare library found it implementing a deprecated OAuth grant from stale memory.
This document's own drafting committed the same sin twice: an early draft named
SIMD-BP128 while describing a patched-exception codec that does not exist under that
name, and prescribed Gorder — a graph-analytics reordering — for ANN beam search, a
different workload. Both were the texture of a model blending adjacent techniques from
memory. Rule: **never implement against a remembered spec.** Fetch the primary source
into the session, vendor it in `references/`, and cite it.

**Start from usage, not structure.** Every RFC includes at least one worked example:
actual bytes, actual offsets, a real tiny index a human can check by hand.

**Review is adversarial, not ceremonial.** Every RFC ships with a "how this could be
wrong" section, which must also name which death from the graveyard (`docs/lineage.md`)
it most risks repeating. Fuzzing and round-trip property tests are not optional.

**Do the arithmetic before the design.** The single most expensive error caught while
drafting this document — a graph-ANN cold path over S3 — would have been killed by one
line of multiplication. The napkin-math rule (§8) is the institutionalized form of that
lesson, extended to include the manifest layer, because "roundtrips per query" that
start after the metadata is magically in hand are an engine's accounting, not a
format's.

---

## 4. Design lineage

We stand on prior work openly and say so. The spec's introduction must credit this
lineage; each RFC names the prior art it evolves from. The full map — Lucene, tantivy/
Quickwit, PISA, Lance, Iceberg, Puffin, SPANN/SPFresh/turbopuffer, DiskANN, FastLanes,
CIFF, and the graveyard of formats that didn't survive contact with production (Indri,
Galago, BitFunnel, the Optane-era formats, Pilosa) — lives in `docs/lineage.md`. Every
RFC's "how this could be wrong" section names its nearest grave from that map.

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
   fetches. The one-wave rule: after the open, **every byte range a cold query may
   need MUST be addressable from data already fetched** — footer, hotcache, or
   navigation tier — with no offset lookup that costs a round trip, so that a
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
   storing lossless lengths, or promising parity without pinning a formula, were
   contradictions an earlier draft left unresolved. Both resolutions live here and in
   M1 so they cannot regrow.)
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
   advisory only — a requirement to *prove* autovectorization per RFC would be theater,
   fragile across rustc versions, and is not imposed.
10. **Per-blob storage class; wire bytes are dense.** Every blob declares
    `storage-class: chunk-compressed` or `storage-class: raw-mappable`.
    Chunk-compressed blobs have dense wire bytes; SIMD alignment is a property of the
    decompressed in-memory buffer, which the reader controls by decompressing into
    aligned arenas — padding compressed wire bytes to register widths buys nothing and
    bakes today's hardware into a format meant to outlive it (see the Optane grave,
    `docs/lineage.md`). Raw-mappable blobs — intended for direct mmap or direct-to-device reads
    without decompression — declare a power-of-two byte alignment per blob. No vendor
    register width appears in spec text.
11. **Byte determinism.** Two independent implementations given the same
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

The full starting defaults for postings, term dictionary, quantization, vector index
shape, the chunk/block split, and I/O paths — required background for implementing
against these invariants — live in `docs/data-structures.md`. The settled-vs-open
ledger for everything above lives in `docs/ledger.md`.

---

## 6. The manifest layer

A format that specifies the segment file but skips the table cannot answer: which set of
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
stays out. Safety, unlike policy, cannot be out of scope, hence the following rules.
Each is the smallest rule that closes a real data-loss or 404-mid-query scenario; the
M0 crash tests exercise all of them.

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
format does not have; `docs/benchmarks.md` says so.

**Write amplification is the writer's problem, and the spec says so.** Immutable
segments mean a commit per tiny batch produces a segment per tiny batch, and cold
query cost grows with segment count until compaction. The format ships no WAL and no
memtable; a production writer batches on its own side. This is a real cost of the
design, stated rather than hidden, and it is one reason the segment-count
amplification metric exists in §8.

---

## 7. Performance and benchmarks

Every RFC touching a cold-path structure follows these rules. The full detail —
datasets, the complete metrics list, named baselines, and the benchmark-engine/adapter
strategy (including the tantivy fork, Lucene `StrandCodec`, and the FAISS adapter) —
lives in `docs/benchmarks.md`, which also carries the turbopuffer benchmark targets.

**The napkin-math rule.** Every RFC touching a cold-path structure includes the
arithmetic: expected accesses per query × dependent round-trip depth × round-trip
latency, plus total bytes fetched — **counted end-to-end, from the pointer read**,
because a cold query resolves the manifest before it opens a segment: pointer GET,
snapshot metadata GET, then segment opens (≤2 RTT each per invariant 3, issued in
parallel across segments), then the cold-fetchable waves. Arithmetic that starts after
the metadata is in hand is an engine's accounting, not a format's — the comparison
engine's "3–4 roundtrips" includes its metadata trip. The pinned figures, so no RFC
cherry-picks: **~100ms per object-storage round trip** as the planning figure
(turbopuffer architecture docs, their stated first-principles number, vendored at M0)
and **~250ms p90 for small reads as the tail figure for SLO discussion — pending: this
figure has not been located in a current primary source; vendor the source sentence or
replace it with a measured MinIO/S3 tail figure at M0. Until then it is a placeholder
and is marked as such wherever used.** An RFC without this arithmetic is incomplete.
For calibration: 50–200 dependent fetches at 100ms is 5–20 seconds — the graph-cold
baseline being escaped.

**Provisional cold-open byte budget: 100 MB per segment.** This is the budget that
`tier: cold-fetchable` blobs (invariant 7) and the R1 kill criterion (`docs/ledger.md`)
are measured against: the total bytes a reader must fetch wholesale at open —
navigation tier plus quantized codes — before the first query can execute. Rationale —
after the §2 rule caught flawed earlier arithmetic: a single object-storage stream runs
in the ~50–100 MB/s class, so 100 MB on one stream is a second or more — the budget
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

---

## 8. Repository shape

`CLAUDE.md`, `LICENSE`, and the `docs/` reference files below exist at the start;
everything else — `spec/`, `rfcs/`, `references/`, `crates/`, `bench/`,
`conformance/` — is created at M0 or at its first milestone. The shape is fixed now so
sessions put things in the right place from the first commit.

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
    lineage.md          the prior-art map (exists, §4)
    data-structures.md  data-structure baseline defaults (exists, §5)
    benchmarks.md        benchmark/adapter detail + turbopuffer targets (exists, §7)
    milestones.md        full per-milestone deliverables and gates (exists, §9)
    ledger.md            settled vs open ledger (exists, §5)
    research/
      README.md          condensed research grounding, R1-R11 (exists); the full
                         kickstart report and R9+ memos are vendored here at M0
  references/          vendored primary sources (populated at M0 from
                       docs/research/README.md's source lists — the §2
                       source-sentence rule lives here)
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

## 9. Session workflow

1. **Orient.** Read this file, the relevant `docs/` reference chapter and `spec/`
   chapter, and the RFC you are implementing. If the task has no approved RFC and
   touches format design, your deliverable is the RFC draft, not code.
2. **Ground.** Pull the primary sources you depend on from `references/` (or fetch and
   vendor them; `docs/research/README.md` lists what to fetch per track). Cite them.
   Never rely on memory for another format's byte layout — §3 lists this project's own
   demonstrations of why.
3. **Arithmetic.** If the work touches a cold path, write the §7 napkin math before
   the design, in the RFC, end-to-end from the pointer read, against the pinned
   figures and the cold-open byte budget.
4. **Implement narrow.** One RFC, or one coherent slice, per session. Include a worked
   byte-level example and the tests that pin it.
5. **Prove.** Round-trip property tests for anything serialized. Fuzz targets for
   every reader. Scalar-vs-SIMD equivalence tests for any kernel touched. The
   invariant-11 checklist answered for any new wire structure. Benchmarks updated if
   the change plausibly moves a §7 metric.
6. **Record.** Commit message: what changed, which RFC, the prompt/task that drove the
   session. Update RFC status and `docs/lineage.md` if new prior art was consulted.

Definition of done for any spec chapter: the golden files in `conformance/` exist, the
reference implementation passes them, and the chapter's prose passes the §2 test.

---

## 10. Milestones

Each milestone ends with a short written report in `docs/`: the numbers, what they
mean, what they changed. A benchmark that embarrasses us goes in the report with an
analysis, not in the memory hole. Full per-milestone deliverables, gating conditions,
and benchmarks live in `docs/milestones.md`; this is the orientation summary.

**M0 — Container + manifest.** Container spec chapter, chunk/block split,
byte-determinism pins, footer/hotcache layout, blob registry, the row-ID chapter, and
the manifest chapter with the CAS commit protocol and §6 safety rules; `strand-core`
read/write and `strand-tools inspect`; golden files; `references/` and
`docs/research/` vendored.

**M1 — Lexical.** BP128 postings, positions, FST term dictionary, block-max sibling
blob, Roaring filter bitmaps, the scoring-profiles chapter, and the gating analyzer
descriptor schema plus normative conformance vectors; tantivy importer; the R2 codec
bake-off.

**M2 — Vectors, cluster-first.** Flat vector blob, RaBitQ codecs, and the
cluster-family cold-native blob (navigation tier + wholesale posting lists + rerank
region); the warm-tier graph blob family is in-scope but explicitly second.

**M3 — Hybrid + deletes + merge.** Deletion vectors, per-family compaction, the
orphan-sweep tool, end-to-end hybrid RRF across both blob families, and the
multi-segment amplification benchmark.

**M4 — Interchange + independence.** CIFF importer, conformance manifest frozen at
spec v0.1, real second-reader independence (the tantivy fork or a clean-room
implementation), and Lucene `StrandCodec` as the JVM parity vehicle.

**M5 — The consumer.** A thin, read-only DataFusion TableProvider over STRAND
segments — the hybrid-fusion benchmark host and the project's answer to "name the
second engine."

---

## Appendix — settled vs open ledger, and research grounding

The full settled-vs-open ledger lives in `docs/ledger.md`. The condensed research
grounding for tracks R1–R11 — findings this constitution depends on, and the primary
sources to vendor into `references/` at M0 — lives in `docs/research/README.md`.
