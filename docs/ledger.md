# Settled vs open ledger

Extracted (with version self-references removed) at repository seeding from the
seed constitution's ledger appendix — now the first half of `CLAUDE.md`'s single
Appendix, which points here. This is the ledger `CLAUDE.md` §5 and `docs/data-structures.md`
point to: what's settled and not to be re-litigated, and what's open and requires an
RFC backed by the research tracks in `docs/research/README.md`.

**Settled (apply, do not re-litigate):** chunk-shaped cold access with the one-wave
addressability rule (invariant 3); end-to-end cold accounting from the pointer read
(`CLAUDE.md` §7); cluster-family as the cold-native vector shape; graph blobs as warm-tier;
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

**Open (RFC required; grounding in `docs/research/README.md`):**
- **R1** — the cluster blob's concrete layout and tier-1 sizing law vs segment scale
  and replication (gates M2/M3). Kill criterion, falsifiable: if tier-1 exceeds
  `CLAUDE.md` §7's provisional 100 MB cold-open byte budget — or its measured M0 replacement — by more
  than ~4× at target segment scale, cold vector search is narrower and the mission
  sentence changes again. Also: the graph-blob ordering algorithm (Starling's block
  shuffling is the literature; pick with evidence).
- **R2** — postings default confirmation BP128 vs FastPFOR vs FastLanes on real
  corpora, including verifying tantivy's actual current codec (gates M1, load-bearing
  for the data-structure baseline's default argument); the exact d-gap variant to
  register (invariant 11); the block-max raw-statistics fields (invariant 4);
  recommended batch-size range; current Lucene codec class names for
  `docs/lineage.md`. Grounded ahead of the bake-off itself: SIMD-BP128 is a real,
  exception-free scheme distinct from SIMD-FastPFOR, the actual patched-exception
  scheme an earlier draft misnamed (`references/lemire-boytsov-simd-bp128.md`,
  resolving the ambiguity `CLAUDE.md` §3 names); vertical (interleaved) SIMD layout,
  not horizontal, is the registration this RFC should default to, given a measured
  50–70% speed advantage at bit widths outside 16–26 and equal speed inside that
  range (same source); the `bitpacking` crate (quickwit-oss, MIT, stable Rust,
  genuine runtime CPU-feature dispatch with a scalar fallback, native x86 SSE3/AVX2
  and aarch64 NEON paths, used by tantivy for its own postings) is a concrete,
  ready candidate for the bake-off's SIMD decode path, satisfying invariant 9
  as-is rather than requiring a hand-rolled kernel
  (`references/quickwit-bitpacking-crate.md`). Two implementation cautions for
  whichever kernel is chosen: BMI2's `pext`/`pdep` are not used by any real
  BP128/FastPFOR implementation found (they accelerate a different codec family,
  varint decoding) and carry a real trap if ever considered — AMD Zen 1/2 run them
  at 18–19 cycles (non-pipelined, microcoded) versus 3-cycle-latency/1-cycle-
  throughput on Intel Haswell and AMD Zen 3 onward, so correct dispatch would need
  microarchitecture-generation awareness, not a feature-flag check
  (`references/agner-fog-pdep-pext-latency.md`); and on ARM, default to NEON, not
  SVE — the one direct benchmark found (AWS Graviton3) measured SVE slower than
  NEON for this exact bit-unpacking workload, contradicting the "wider ISA is
  faster" assumption (`references/fastlanes-arm-sve-vs-neon.md`). Separately, and
  worth remembering so it isn't re-proposed as an open, promising avenue: no
  established technique exists for processing BP128/FastPFOR-encoded postings
  while still compressed — the state-of-the-art literature (the same
  Lemire/Boytsov lineage) decodes via SIMD, then intersects via SIMD, as two
  pipelined stages, not a fused decode-free operation
  (`references/lemire-boytsov-simd-bp128.md`). This is unlike Roaring, whose
  AND/OR operations genuinely run on its compressed containers with no
  decompression step at all (`references/roaring-bitmaps-container-operations.md`)
  — confirming invariant 2's choice was correct for the reason it's correct, now
  grounded rather than assumed. Block-max pruning (invariant 4) is a third,
  distinct technique from either of the above — deciding not to touch a block's
  compressed bytes at all via precomputed bounds, not an operation performed on
  bytes once touched (`references/ding-suel-block-max-shallow-pointers.md`, which
  also pins the "shallow" vs "deep" pointer-movement terminology this project
  should use consistently). A separate, narrower question raised alongside this
  grounding — whether a single codec could combine the PFOR/BP128 family's raw
  full-scan decode speed with Elias-Fano's genuine compressed-domain
  searchability, since no published construction achieving both has been found
  (PISA offers them as separate per-index choices, not one fused codec; on
  compression the measured direction favors partitioned Elias-Fano, which is
  the smaller of the two on the reference collections —
  `references/ottaviano-venturini-partitioned-elias-fano.md`) — has a phased,
  gated investigation methodology at `docs/research/r2-hybrid-codec-methodology.md`.
  No phase has been executed; it is a plan, not a result, and does not change
  R2's BP128 default or require an RFC on its own.
- **R3** — the rotation-provenance mechanism (materialized matrix vs generator+seed,
  M2 RFC); TurboQuant revisit condition.
- **R4** — precise Lucene-vs-tantivy doc-length accounting for the invariant-6 length
  definition (needs primary-source confirmation).
- **R5** — exact GCS/Azure conditional-write header semantics (confirm at spec time).
- **R9** — compute-native block layout (gates the R2 bake-off and therefore M1):
  measure FastLanes against hand-vectorized BP128 and FastPFOR on postings
  distributions (the margin is unmeasured), audit the cwida/FastLanes license
  against Apache-2.0-only, reconcile 1024-value granularity with the block-max
  sibling design (1024-native or nested 8×128, preserving invariant 4), and assess
  ALP for the flat float-vector blob and the GPU decode path for raw-mappable blobs.
- **R10** — cross-segment scale: should the manifest carry optional per-segment
  summary metadata (term-statistics sketches, centroid summaries, min/max-style
  pruning stats) so a reader can prune segments before opening them, and what does
  target segment size look like when the 100 MB budget pushes segment count up while
  open-amortization pushes it down? Fed by the M3 multi-segment benchmark; any
  summary blob must stay index-internals-agnostic per `CLAUDE.md` §6.
- **R11 — Adapter feasibility audit (gates all adapter work).** Verify against
  current source, not memory: (a) tantivy's reader surface — map the modules a
  STRAND read path must replace, settling the codec-SPI question as a by-product,
  and confirm the exact Lucene codec SPI class surface for `StrandCodec`; (b) FAISS
  per-kernel feasibility — whether the generic `InvertedLists` path serves
  `IndexIVFRaBitQ` search over external storage, and whether the FastScan path can
  run over external lists at all given its `CodePacker`/`BlockInvertedLists`
  packing, including the load-time repack cost if so; (c) Quickwit split/hotcache
  internals post-relicense, testing the inherits-from-the-fork hypothesis; (d) the
  fork reader-module list that arms the fork failure triggers (docs/benchmarks.md); (e) the warm-tier graph
  host choice. Adapter milestones are conditional on R11.
- **Postings block size** (conditional): 128 was the default by shared
  lineage, now conditional on R9's granularity outcome — the block-max sibling-blob
  *pattern* stays settled (invariant 4), only the granularity number is open.
- **Pending figures:** the ~250ms p90 tail figure — re-locate the source sentence or
  replace with a real-network M0 measured figure (`CLAUDE.md` §7); `bench/` measures
  a real cold-open p50/p90/p99 against MinIO on localhost (`bench/results/cold-open.json`),
  which confirms the GET-count half of invariant 3 but not yet the real-network tail
  latency this figure needs — that still wants MinIO with injected latency, or real S3;
  the parallel-wave aggregate throughput behind the 100 MB budget rationale — not yet
  measurable at all until a `tier: cold-fetchable` vector blob exists (M2); tantivy
  codec-SPI absence (R11(a)); FAISS FastScan external-list feasibility (R11(b)); the
  Quickwit inheritance hypothesis (R11(c)).
- **Batch-shaped reader trait** (invariant 9's frozen `next_batch()` API shape):
  not yet implemented anywhere in `strand-core`, despite M0's original deliverable
  list claiming it. Carries forward as an M1 prerequisite — M1's postings kernels
  are its first real consumer, and the trait should land with them rather than as
  an unconsumed abstraction (`docs/milestones.md` M0 records the same).
- **TLA+ model correspondence gap, to close before the TLAPS proof phase** (RFC
  0002): `verification/manifest.tla`'s `ProposeSnapshot(w)` models the snapshot-
  object write as always succeeding, but the real `put_if_absent` in `commit()`
  can fail definitely or ambiguously and end the attempt
  (`crates/strand-core/src/manifest.rs:131-139`). The omission traces to RFC 0002
  §4's approved action grammar, which — unlike every sibling action — listed no
  outcome set for `ProposeSnapshot`. Harmless to the current TLC-checked safety
  invariants (a failed propose reaches a terminal state no invariant observes),
  but a TLAPS proof built on the current grammar would entrench the gap; add the
  failure outcome to the model (and record it against RFC 0002) before that phase
  starts.
