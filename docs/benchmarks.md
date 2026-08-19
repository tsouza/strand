# Performance and benchmarks — detail

The napkin-math rule, the cold-open byte budget, and the segment-count-reporting rule
are load-bearing enough to stay in `CLAUDE.md` §7. This file holds the rest: datasets,
the full metrics list, named baselines, and the benchmark-engine/adapter strategy.
Extracted at repository seeding from the seed constitution's performance section
(now `CLAUDE.md` §7 — the seed's numbering differed) plus its turbopuffer-targets
appendix.

**No performance claim without a reproducible benchmark in-repo.** If a README sentence
says "fast", a `bench/` target backs it: pinned datasets, pinned hardware description,
one-command runner. `bench/` is a workspace member from M0. Criterion for micro, custom
harness for end-to-end; results committed under `bench/results/` with date and commit
hash, machine-readable.

**Datasets**: MS MARCO passage for lexical; BigANN/SIFT/GIST subsets and an embedding
set (e.g., Cohere Wikipedia) for vectors. Small deterministic fixtures for CI; full
runs on demand. MS MARCO passage's own canonical source,
`msmarco.blob.core.windows.net/msmarcoranking/collection.tar.gz`, returned HTTP 409
"Public access is not permitted on this storage account" as of 2026-08-18 —
re-verify before assuming it works; `bench/src/msmarco_index.rs` currently fetches
the same 8,841,823-passage corpus via the `Tevatron/msmarco-passage-corpus` mirror
on Hugging Face instead (`corpus.jsonl.gz`, verified byte-for-byte passage count
match). The corpus itself is never committed (`bench/data/`, gitignored,
~1.07 GB); only derived numeric results land in `bench/results/`.

**The metrics that matter**, in order: (1) cold end-to-end open cost — GETs and bytes
from pointer read to first planned query against S3-class latency, broken out as
manifest resolution + segment opens + cold-fetchable waves (targets: hotcache open
under 100 ms and ≤2 round trips per segment, within the cold-open byte budget, with
the manifest adding at most two round trips ahead of them); (2) cold and warm query
latency p50/p99 for BM25 top-k, ANN top-k, and hybrid RRF, with GETs and bytes-read
per query reported alongside — and warm reported both ways, against a cached snapshot
and with a pointer-freshness check, per `CLAUDE.md` §7; (3) index size vs the same data in tantivy
and Lucene; (4) build and merge throughput, per merge strategy of invariant 1, plus
segment-count amplification at M3; (5) score parity per invariant 5's
profile-based definition; (6) bytes-fetched vs bytes-used, the read-amplification
cost (`docs/data-structures.md`).

**Named baselines**: tantivy (same-process lexical), Lucene (small JVM harness, parity
within norm quantization), Parquet-plus-brute-force (what the index buys). Regressions
against our own previous results fail CI beyond a stated tolerance. Object-storage
behavior is tested against local MinIO with injected latency so round-trip counts are
asserted in CI, not measured ad hoc.

**turbopuffer is an internal benchmark target, not a rival.** The honest sentence,
stated as doctrine: *a format can beat their open cost; only an engine can beat their
steady-state query latency; we are not building an engine.* Their sub-10ms warm p50 is
a property of a caching fleet, a WAL-tail freshness path, and a query planner, none of
which a format spec can touch. Published figures we measure against live below; every
comparison report states our number, theirs, hardware, dataset, date, and the
caching-fleet asymmetry. The word "beating" does not appear in headings or docs.

**Benchmark engines and adapters.** The strongest benchmark of a format holds the
engine constant and varies only the bytes: the same traversal, scoring, and query
code reading its native index versus reading STRAND, so any delta belongs to the
layout. Every borrowed-engine result names the algorithm layer it ran through
(harness, fork, adapter, DataFusion, PISA, FAISS); a number without its engine
named is not a result. Nearest grave, per `docs/lineage.md`: an adapter fork that outlives its
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
2026-08-18, `references/tantivy-LICENSE.txt`), and the fork
carries tantivy's MIT notices alongside our Apache-2.0
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
STRAND's specified one, same engine — the fairest test of the lineage's hotcache
claim (`docs/lineage.md`). Scoped, not built, 2026-08-19
(`references/quickwit-comparison-scoping.md`): deployment, index config, and
ingest are comparably simple to the tantivy comparison (a `whitespace` tokenizer
exists for feeding the same pre-analyzed tokens, and a real
`quickwit_storage_object_storage_gets_total` Prometheus metric gives the GET
count directly, no MinIO-side instrumentation needed); the genuine scope
difference is that Quickwit is a caching server, not a stateless library call —
`bench/src/field_cold_open.rs`'s own methodology (loop a plain store call 30
times) gets a free cold measurement every iteration, while Quickwit's
`fastfields`/`shortlived`/`splitfooter` caches mean a naive repeated query is not
necessarily cold after the first one, and no explicit cache-bypass flag was
found in this pass. A defensible methodology (process restart per iteration, or
N separate small indexes queried once each) has to be chosen and stated as
honestly as STRAND's own `localhost`-not-real-network caveat. Order of
magnitude: a focused, dedicated task, not a same-session extension of the
tantivy comparison.

**PISA via CIFF (M4).** The STRAND→CIFF export lets PISA's MaxScore/WAND/BMW
implementations query STRAND-derived indexes with zero adapter code; algorithm
layer named as PISA.

Vector and fusion adapters:

**FAISS inverted-lists adapter.** FAISS (MIT, verified byte-level 2026-08-18;
LICENSE vendored 2026-08-18, `references/faiss-LICENSE.txt`) exposes an
inverted-lists extension point
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

## turbopuffer benchmark targets (internal)

All figures are turbopuffer's own published claims, to be vendored at M0; none are
independently verified. Every comparison report cites the source sentence, states our
independently measured number, hardware, dataset, and date, and repeats the asymmetry:
their warm numbers are a caching fleet's — with a WAL-tail freshness path and a query
planner — not a format's. A format-based reader pays an explicit pointer round trip
for the freshness their engine gets from its own machinery (`CLAUDE.md` §6); warm comparisons
state which variant of ours is being compared.

Reference points:
- ~100ms per object-storage round trip (their stated first-principles figure); cold
  queries budgeted at 3–4 round trips, "often as little as ~400ms". This is the
  *structured* cold path — metadata, then filter/centroid indexes + WAL tail, then
  clusters — and note it includes their metadata trip, which is why `CLAUDE.md` §7 counts ours.
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
