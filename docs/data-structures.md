# Data-structure baseline

Invariant 8 (`CLAUDE.md` §5) says don't invent encodings. This section names the
starting defaults so they aren't re-litigated inside every RFC. Each default is a
specific, published, production-proven technique, verified against primary sources in
the R-track research (`docs/research/README.md`). A session may swap one for a named
alternative via RFC backed by a benchmark; it may not invent one. Extracted at
repository seeding from the seed constitution's data-structure section (pointed to
from what is now the end of `CLAUDE.md` §5 — the seed's numbering differed).

**Postings (provisional default, confirmed by in-repo benchmark — R2).** Fixed
128-integer blocks encoded with **SIMD-BP128**: plain vectorized binary packing at a
per-block bit width, **no exception stream** (Lemire & Boytsov, SPE 2015, vendored at
M0). SIMD-BP128 must not be confused with a patched-exception codec — no such codec
exists under this name. The trade is explicit: BP128 is the
fastest decode in its class (~0.7 cycles/int in the S4 variants) and the simplest
possible from-spec reader — no exception-stream handling — at a compression cost of up
to ~2 bits/int and 5–15% worse ratio than FastPFOR. **SIMD-FastPFOR** (exception
streams, better ratio) is a registered alternative codec, not the default. Two reasons
reader-simplicity wins: a conformance default is code every stranger must implement,
and the concrete second-reader candidate (the tantivy/DataFusion ecosystem, R6) is
believed to ship exception-free bitpacking — a claim `docs/ledger.md` still lists as
**unverified** against current tantivy source; verify it when vendoring for R2, and if
it fails, this argument loses one of its two legs while the conformance-simplicity leg
stands alone. Lucene's PFOR lineage remains reachable via the registered alternative.
128 is the block size of shared lineage — Lucene, tantivy, and PISA all use it,
descended from the same design, which is an argument from compatibility, not
independent convergence. Per invariant 11, the R2 RFC pins the exact d-gap
variant; the name "BP128" alone is not a registration. The default is confirmed or
swapped by the R2 bake-off on MS MARCO distributions; the registry design is the
invariant, the default is a parameter. The bake-off has a third named candidate: the
**FastLanes transposed layout** (`docs/lineage.md`), whose paper reports portable scalar decode at
SIMD-class rates (>40 values per CPU cycle across Intel, AMD, Apple, and AWS chips) —
those headline wins are measured against scalar baselines, and its margin over
hand-vectorized BP128 on postings distributions is **unmeasured**; measuring it is
exactly R9's job. Per the §2 rule, no order-of-magnitude margin over BP128 is claimed
until it is measured. FastLanes carries two further open questions that gate adoption (R9): its 1024-value
granularity conflicts with the 128-int block lineage — the block-max sibling blob must
be redesigned to 1024-granularity or a nested 8×128 scheme, and either must preserve
invariant 4. (Its license is resolved: `cwida/FastLanes` is confirmed MIT,
Apache-2.0-compatible — `references/r9-fastlanes-core-alp-damon-license.md`, 2026-08-18
— leaving only the granularity question and the measured margin open.) If R9
resolves these favorably and the
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
bit-width, because the two paths are different computations that must not be
conflated: the **1-bit** routing path uses FastScan-style LUT/register-shuffle machinery; the
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

**Vector index shape (settled constraint — R1; blob design settled by RFC 0010, 1-bit
only — multi-bit still open).** The
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
law that binds segment scale, corrected by RFC 0010's own napkin math against the
registered codec's real per-vector correction-factor and row-id overhead (not just the
raw code bits the earlier figure below counted alone): 1-bit codes cost dims/8 bytes
per vector for the codes themselves — 96 B at 768d, 128 B at 1024d, 192 B at 1536d —
but the codec's own per-vector distance-correction factors add another 12 B/vector
(a full-batch-amortized floor) and STRAND's row-ids add 8 B/vector, so the real
per-vector floor is dims/8 + 20 B — 116 B at 768d — and, with realistic partial-batch
padding waste plus navigation-tier overhead at a `4·√N` cluster count, tier-1 runs
**~131 MB per million 768d vectors**, not ~100 MB, before replication
(`rfcs/0010-vector-blob-cluster-family.md` Napkin math). SPANN-style closure
replication (up to 8×) is a first-class recall/storage/cost knob in the blob
metadata, not a hidden constant, and its cost is real and now grounded against a
paper's body, not an abstract: applying the GIST1M ratio (13.0 GB at replica 8 vs.
7.5 GB at replica 2, a 1.73× growth from replica 2 to replica 8 — not a naive 4×
linear estimate) as a conservative lower bound on this entry's no-replication
baseline puts a replica-8-equivalent estimate at ~227 MB per million 768d vectors,
~2.27× the provisional 100 MB budget — over half the margin to R1's own 4× kill
criterion, not a wide one. This ratio's real source is the companion cloud-native
benchmark paper's Table 4 (Li et al., `arxiv.org/abs/2511.14748` §5.3), not SPANN's
own paper — SPANN's own paper was fetched in full and confirmed to report no
GIST1M dataset and no index-size figure at any replica count
(`references/spann-body-figures.md`, resolving what RFC 0010 Open questions
previously named as owed work). What remains a real, stated limitation: neither
paper measures a replica=1 index size, so this estimate still extrapolates across
the unmeasured 1→2 step rather than measuring it directly.
A direct, load-bearing consequence of the corrected law: a segment fitting the 100 MB
tier-1 budget now holds ~760,000 768d vectors, not ~1,000,000 — the "roughly one
segment per million 768d vectors" figure `CLAUDE.md` §7 currently states needs
updating to reflect this (RFC 0010 Open questions).
**Graph families** (DiskANN/Vamana-layout, one node's vector + adjacency per I/O unit,
Starling-style block shuffling) are registered as `tier: warm` — legitimate and
specified, but never the cold-open story; pipelining does not rescue them, because
overlapping compute with I/O cannot convert a dependent chain of ~100ms round trips
into parallel fetches. Node order within a graph blob is a **persisted permutation**
(the format decision, settled); the ordering algorithm is explicitly open — Gorder
targets graph-analytics traversal, not ANN beam search, and is not a candidate;
Starling's block shuffling is the relevant literature, and R1's RFC picks with
evidence.

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
published GPU decompression path (DaMoN '24), contingent on R9. Hardware-specific
expectations live in the reader; the
format's only obligations are the storage-class declaration and alignment attributes
of invariant 10. Shard and partition boundaries are first-class in the layout;
NUMA-aware placement is a runtime concern the format merely must not obstruct.
