# Milestones

Extracted at repository seeding from the seed constitution's milestones section
(now `CLAUDE.md` §10 — the seed's numbering differed). `CLAUDE.md` keeps a
one-line summary per milestone; this file is the full detail each milestone session
works from. Each milestone ends with a short written report in `docs/`: the numbers,
what they mean, what they changed. A benchmark that embarrasses us goes in the report
with an analysis, not in the memory hole.

**M0 — Container + manifest.** Container spec chapter, chunk/block split, storage-class
and alignment attributes, invariant-11 byte-determinism pins (endianness, chunk codec,
checksums), footer/hotcache layout, blob registry, row-ID chapter with per-family
merge-semantics declarations, **manifest chapter with the CAS commit protocol and the
`CLAUDE.md` §6 safety rules (declared CAS host, deletion-safety retention, reader 404-refresh,
orphan rule)**. `strand-core` read/write; `strand-tools inspect`. Golden files. Vendor
`references/` and `docs/research/` from `docs/research/README.md` — **partially done**:
`references/` holds the R2 and RFC-0002 grounding, all four turbopuffer pages, both
adapter LICENSE files, and — as of 2026-08-18 — R1/R3–R9's core primary sources (the
"full kickstart report" this entry previously also listed as owed was a retracted
phantom deliverable, never a real document — `docs/ledger.md`). A small residue of
paper-body-only figures and two lower-priority R4 sources remain flagged as owed;
`docs/ledger.md` lists them precisely. The batch-shaped
reader trait (invariant 9's frozen API shape) was originally listed here but is **not
yet implemented** — no `next_batch()` interface exists in the code; it carries forward
as an M1 prerequisite, since M1's postings kernels are its first real consumer
(tracked as an open item in `docs/ledger.md`). Benchmarks and tests: cold end-to-end
open (pointer → planned query) GET count and latency against MinIO; manifest commit
contention (two writers racing the pointer); crash tests (writer dies before commit →
orphans, reader on expired snapshot → 404-refresh path). Two originally-listed items
are structurally deferred, not missed: parallel-wave aggregate throughput cannot be
measured until a `tier: cold-fetchable` vector blob exists (M2 — `docs/ledger.md`
already records this), and the compaction crash test (deleting a file under a retained
snapshot must be impossible) requires M3's compaction — the deletion-safety rule is
normative in `spec/manifest.md` §4 but untestable until the sweep exists. The
tail-latency deliverable is partially met: local-MinIO p50/p90/p99 exist in
`bench/results/cold-open.json`, but the real-network tail figure that would confirm or
replace `CLAUDE.md` §7's placeholder remains open, per that section's own admission. Implementation went
beyond this list in one respect: the store abstraction distinguishes definite from
ambiguous backend failures (`StoreError::Ambiguous`), `commit()` disambiguates an
ambiguous pointer CAS with a follow-up read (RFC 0001's Discussion section records
the amendment), and a `proptest`-based fuzzer drives randomized concurrent-writer
rounds through the protocol's safety invariants.

**M1 — Lexical.** BP128 postings + positions + FST term dictionary + block-max
sibling blob + Roaring filter bitmaps. The **term-dictionary FST and term-info
store** are now RFC 0005 (`rfcs/0005-term-dictionary.md`, Approved) — adopts
tantivy's real two-part design (dense per-term FST, separate ordinal-indexed
term-info array) directly rather than inventing one, with a worked example built
from the actual `fst` crate (not illustrative bytes) and independently reproduced
byte-for-byte during review; also the first RFC to populate the blob-type registry
(`spec/container.md` §9). FST size at realistic vocabulary scale and the `fst`
crate's cross-platform byte-determinism are both still open (RFC 0005's own Open
questions). A reference implementation now exists (`crates/strand-lexical`,
new crate): `TermInfo` encode/decode, `TermInfoStore`'s direct-indexed reads, and
`TermDictionary`'s FST build/lookup, with the cat/dog/fish worked example pinned as
`conformance/term-dictionary/` golden files and reproduced byte-exact by the crate's
own tests — a third independent reproduction of the same bytes (RFC draft, ACPR
review, now the library) — plus a `proptest` round-trip property test over
arbitrary term sets. The postings/positions blobs those `TermInfo` offsets point
into remain unimplemented, gated on the still-open R2 codec RFC. The **filter-bitmap blobs** are now RFC 0006
(`rfcs/0006-filter-bitmaps.md`, Approved) — a value-dictionary FST (identical shape
to RFC 0005 §2) paired with a small directory plus one standard 32-bit Roaring
bitmap per distinct value, indexed by local ordinal; the second RFC to extend the
blob-type registry (`family_id = 2`, "filter"). Its review surfaced a real,
same-version/same-platform byte-determinism risk in the `roaring` crate's
run-container promotion (`insert_range` can select a differently-serialized
container for an ordinary contiguous write), independently confirmed against the
crate's own source and closed with a normative MUST to always serialize without
run containers — the same class of risk RFC 0005 named for the `fst` crate, but
sharper: same build, same platform, different insertion API, different bytes,
absent the MUST. Cross-platform/cross-version determinism for both the `fst` and
`roaring` halves remains open (RFC 0006's own Open questions). Both RFCs now also
have a reference implementation (`crates/strand-lexical`): value-dictionary FST
build/lookup reuses the term-dictionary FST code directly (validating RFC 0006's
own "identical in shape" claim in working code), `build_filter_bitmap_store`
normalizes every bitmap with `remove_run_compression` before serializing, and a
dedicated test builds the identical logical bitmap through two different `roaring`
insertion APIs and asserts byte-identical output — a mechanical, not merely
prose, check that the no-run-containers MUST actually closes the gap the RFC 0006
review found. The blue/red worked example is pinned as `conformance/filter-
bitmaps/` golden files and reproduced byte-exact. The R2 RFC pins the exact d-gap variant
(invariant 11) and the block-max RFC pins the raw-statistics fields (invariant 4);
neither is drafted yet, both gated on R9's still-unmeasured margin
(`docs/ledger.md`) — though a maintained, Apache-2.0 Rust FastLanes implementation
now exists to measure against (`references/spiraldb-fastlanes-rust-crate.md`),
lowering what running that measurement actually costs, and a first real
decode-throughput measurement (`bench/results/codec-decode-throughput.json`) now
covers `BitPacker4x`/`BitPacker8x`/`FastLanes`/`FastPFOR` on both uniform and
synthetic-skewed data. The **scoring-profiles
chapter** (RFC 0003, `rfcs/0003-scoring-profiles.md`, Approved) defines the `bm25`
profile normatively and the Lucene-parity profile, both grounded byte-exact against
Robertson & Zaragoza's own formula and Lucene 10.5.1's real source, with a worked
example. **Analyzer descriptor schema and the normative token-stream
vectors in `conformance/analyzers/` are gating deliverables of this milestone, not
metadata afterthoughts** — without them invariant 6 is a label; the schema and
per-document-length definition are now RFC 0004 (`rfcs/0004-analyzer-descriptors.md`,
Approved), with one real worked example as the first conformance vector, though the
full vector suite across languages and scripts is still M1 execution work, not done
by the RFC alone, and the CJK/Thai/Lao segmentation-dictionary choice remains
unresolved (RFC 0004's own Non-goals). Tantivy importer. R2
codec bake-off lands here and confirms or swaps the postings default (including
verifying tantivy's actual current codec, per `docs/data-structures.md`); the R9 layout evaluation and
license audit MUST complete before the bake-off freezes the default, since a
FastLanes outcome changes both the default and the block granularity — the license
half is now resolved (`docs/ledger.md` R9), the layout-evaluation half is not.
Benchmarks: MS
MARCO BM25 latency and size vs tantivy; Lucene parity per invariant 5;
bytes-fetched vs bytes-used across term frequency deciles (the read-amplification
number, `docs/data-structures.md`). Adapter-based results appear in this
milestone's report only after R11 verifies the respective extension point and the
build-equivalence gate passes; until then, harness and published-numbers baselines
only.

**M2 — Vectors, cluster-first.** Flat vector blob; RaBitQ codecs with kernel-per-
bit-width, the rotation descriptor field, and the rotation-provenance mechanism
(invariant 11); the **cluster-family cold-native blob** (navigation tier + wholesale
posting lists + rerank region) per the R1 RFC, with all posting-list offsets
resolvable from the navigation tier (invariant 3's one-wave rule), the replication
knob and tier-1 sizing limits in blob metadata, computed against `CLAUDE.md` §7's cold-open byte
budget. The warm-tier graph blob family (persisted-permutation node order, ordering
algorithm per R1's evidence) is in-scope but explicitly second. Benchmarks: cold and
warm ANN recall/latency with GET counts asserted; codec comparison RaBitQ vs
PQ-FastScan; the cold target to measure against is turbopuffer's published figures
(`docs/benchmarks.md`), with the asymmetry stated. Adapter-based results appear in this
milestone's report only after R11 verifies the respective extension point and the
build-equivalence gate passes; until then, harness and published-numbers baselines
only.

**M3 — Hybrid + deletes + merge.** Deletion vectors; compaction implementing the
per-family merge semantics of invariant 1 (concatenate+remap for cluster blobs,
rebuild for graph blobs, rebalance for centroids), respecting `CLAUDE.md` §6's deletion-safety
rule, with merge cost benchmarked per strategy; the orphan-sweep tool; end-to-end
hybrid RRF across both blob families over one row-ID space. **The multi-segment
benchmark**: the same corpus at 1, 16, and ~128 segments, cold and warm, so
segment-count amplification is a measured curve feeding R10. Deliverable: **a
benchmark report measured against published figures, with the caching-fleet
asymmetry stated.**

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
TableProvider is additionally the hybrid-fusion benchmark host, running the `CLAUDE.md` §7 fusion
workload with its selectivity sweep. The FAISS adapter lands alongside M2's
benchmarks or here, per R11(b)'s feasibility finding.
