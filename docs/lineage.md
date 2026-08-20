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

**From NOFireAI/ravel**: a living, real, Apache-2.0 (confirmed via GitHub's license
API, `references/nofireai-ravel-storage-architecture.md`), actively-developed
object-storage-first observability datastore for metrics, logs, and traces — not a
grave, and not a search-index format, but the closest real system this project has
found at the catalog/commit layer specifically. Its key layout puts every data
object, commit record, and snapshot part behind a content-addressed, immutable key,
with exactly one mutable object per tenant per signal — the `HEAD` pointer, updated
only by version-CAS — which is the same shape as `CLAUDE.md` §6's manifest: immutable
segments, immutable snapshot metadata, one current pointer, CAS to advance it. Its
"sealed hours" mechanism (a wall-clock formula plus a proven "seal lemma": once a
time bucket is provably past every writer's flush deadline plus clock-skew and
safety margins, one strongly-consistent LIST of that bucket is final forever) and its
bounded "fold reconcile" pass (a fixed lookback window, defaulted to 26 hours in the
real source, that catches a late compaction or tombstone landing in an
already-sealed bucket) are real solutions to a problem STRAND's own manifest layer
does not yet have to solve, because STRAND has no time-bucketed ingest windows — but
the shape of the problem (a background summarization pass that must not silently
miss a late-arriving mutation to already-summarized state) is exactly the shape
`docs/ledger.md`'s R10 (manifest-level cross-segment navigation) will eventually
have to answer for segment-count growth. What STRAND does **not** share with ravel:
ravel is a full telemetry datastore with its own OTLP/Prometheus-Remote-Write
ingest, PromQL and SQL query engines, alerting, multi-tenancy, and a Kubernetes
operator — an engine, not a format — where STRAND is a storage format only, with no
query engine of its own beyond the M5 DataFusion `TableProvider` proof-of-concept.
The comparison is at the storage/catalog layer alone.

**From Zoekt** (`sourcegraph/zoekt`, forked in 2017 from `google/zoekt`; both
Apache-2.0, confirmed via GitHub's license API and the repository's own `LICENSE`
file, `references/zoekt-code-search-engine.md`): the production counter-lesson on
row granularity for source code, directly relevant to STRAND's narrowed logs/code
domain. Zoekt indexes source at file granularity — a shard's data is file contents,
filenames, and content/filename posting lists over a positional-trigram index, with
symbol information supplied at index time by an external, sandboxed `ctags`
invocation used purely as a ranking signal ("does the match fall on a symbol
definition?"), never as a persisted, addressable identity that survives a re-index.
This independently confirms, from Zoekt's own real design documentation rather than
from memory, `docs/ledger.md`'s "code row-IDs stay file-granular" settled decision
and its characterization of Zoekt as a system that "recompute[s] symbol positions
wholesale, with no persisted identity across a re-index at all." Zoekt is prior art
for the lexical side specifically — a real, production, trigram-indexed code-search
engine — not for the manifest/catalog layer ravel informs.

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
