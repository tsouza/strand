# Research grounding (condensed)

For each track: the findings the constitution depends on, and the primary sources to
vendor into `references/` at M0. This is the standing source sessions cite — no
separate, longer report exists to supersede it; an earlier draft of this repo's
governance docs claimed one was owed, but nothing behind that claim was ever produced,
and it was retracted (`docs/ledger.md`). Extracted at repository seeding
from the seed constitution's research-grounding appendix — now the second half of
`CLAUDE.md`'s single Appendix, which points here.

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
turbopuffer, CockroachDB, VectorChord, Volcengine — and both reference repositories
named below (an earlier draft said "all three"; no third repository was ever named
anywhere in this project's text, and the claim is corrected here) are Apache-2.0,
confirmed byte-exact via GitHub's license API
(`references/rabitq-and-extended-rabitq.md`), resolving the earlier "byte-for-byte
header reads were blocked" caveat. The 1-bit
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
August 20, 2024; If-Match ETag CAS November 25, 2024 (`references/r5-manifest-commit-sources.md`).
Lance demonstrates the
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

**R11 — Adapter feasibility (partial grounding; audit open; (a) resolved).**
Verified 2026-08-18 against live source: tantivy and FAISS are both MIT
(byte-level LICENSE reads, `quickwit-oss/tantivy`, `facebookresearch/faiss`);
FAISS ships the inverted-lists extension surface at `faiss/invlists/`
(`InvertedLists.h`, `OnDiskInvertedLists.h`, `BlockInvertedLists.h`);
`IndexIVFRaBitQ` extends `IndexIVF` and its FastScan variant
(`IndexIVFRaBitQFastScan` → `IndexIVFFastScan`) initializes a `CodePacker` in
its inverted lists and retains an `orig_invlists` pointer — the source basis
for splitting the adapter claim by kernel. **R11(a) resolved 2026-08-19**
against tantivy tag `0.26.1` and Lucene tag `releases/lucene/10.5.1`
(`references/r11a-tantivy-reader-surface-and-lucene-codec-spi.md`): tantivy has
no codec SPI — `Directory` is a byte-range storage abstraction, `SegmentComponent`
is a closed seven-variant enum with one concrete reader/writer wired per variant,
and the `Postings` trait is a runtime query-result iterator, not a wire-format
registration point; the named "tantivy fork" M4 path means forking and modifying
tantivy's internal reader/writer modules directly. Lucene's `Codec`/`PostingsFormat`
SPI, by contrast, is real and confirmed current: eleven abstract format methods on
`Codec`, resolved via `java.util.ServiceLoader` through `META-INF/services/
org.apache.lucene.codecs.Codec`, with `FilterCodec` as the documented delegation
base a `StrandCodec` would extend. Still unverified and owned by R11: whether
FastScan search can execute over externally hosted lists (b); Quickwit
post-relicense internals (c); the fork reader-module list (d); the warm-tier graph
host (e). Sources to vendor at M0: both LICENSE files; `faiss/invlists/InvertedLists.h` and
`OnDiskInvertedLists.h`; `faiss/IndexIVFRaBitQ.h`, `faiss/IndexIVFFastScan.h`,
`faiss/IndexIVFRaBitQFastScan.h`; tantivy segment-reader sources at the pinned
commit; Quickwit split/hotcache sources post-relicense.
