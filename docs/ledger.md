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
  **Phase 0 executed 2026-08-18, resolved.** A real, structurally on-point
  candidate was found (LICO, SIGMOD/PACMMOD 2026 — a learned codec with a genuine
  compressed-domain `NextGeq`, confirmed by reading its actual source, plus SIMD
  decode/intersection). This session's own automated attempts to read the paper
  were blocked by ACM's Cloudflare bot-challenge (despite contradictory
  open-access metadata — Unpaywall says CC-BY, DBLP says closed, neither
  resolved); the project owner then obtained the full paper directly and it was
  read in full. **Verdict: candidate found, checked against real numbers, fails
  the bar** — clears compressed-domain search with real margin (up to 2.64×
  faster intersection than the fastest non-learned SIMD baseline the paper
  itself tests, 5.52× aggregate) but fails full-scan decode throughput by roughly
  an order of magnitude: LICO's own best configuration decodes at 0.61 ns/int
  (Table 7, AVX-512) against this project's own real, measured `BitPacker8x`
  throughput of 0.052–0.070 ns/int
  (`bench/results/codec-decode-throughput.json`) — an 8.7–11.7× gap computed
  directly from real numbers on both sides, not estimated. This confirms, with a
  concrete measured number, the same conclusion this entry already recorded from
  the Lemire/Boytsov literature before the investigation started: no established
  technique fuses BP128-class decode speed with compressed-domain searchability.
  Full detail: `references/zhu-etal-2026-lico-learned-inverted-index-compression.md`.
  Per Phase 0's own GO/NO-GO, the investigation now proceeds to the Track A/B fork
  (`docs/research/r2-hybrid-codec-methodology.md`). **Phase 1 executed
  2026-08-18, Phase 2B checkpoint executed 2026-08-19**
  (`bench/src/hybrid_codec_pilot.rs`, real MS MARCO postings lists, existing
  unmodified codecs — `bitpacking`'s `BitPacker8x` and
  `sucds::mii_sequences::EliasFano`, no new engineering). Three real
  measurement bugs were found and fixed in the course of this, each caught by
  an implausible result rather than trusted on first pass — the corrected
  numbers below supersede everything reported before each fix, including to
  the project owner mid-session:

  1. Skip timing amortized one decode across three per-list queries for BP128
     only, deflating its cost — fixed, and **EF wins skip on 100% of lists**,
     every rerun.
  2. Decode and size both padded every list to `BitPacker8x`'s 256-value
     block regardless of real length — inflating BP128 enormously on this
     short-list-dominated corpus. Fixed with a variable-length final block
     (`bp128_variable_bench`, scalar, no SIMD, no forced block size).
  3. No BP128 variant counted the per-block bit-width byte a real decoder
     needs — understating BP128's true size further. Fixed, one byte per
     block, in all three BP128 variants.

  **Corrected findings, stable across five reruns after all three fixes:**
  size needs no signal, but the direction is the *opposite* of what an
  earlier pass reported — **BP128 (fairly encoded) wins on size for 100% of
  lists, EF loses on every one** (the earlier "~97.1% EF" figure was the
  padding bug, not a real result). The mechanism: `sucds::mii_sequences::
  EliasFano` is plain, un-partitioned EF, and the already-vendored Ottaviano
  & Venturini paper (`references/ottaviano-venturini-partitioned-elias-fano.md`)
  states plainly that plain EF "fails to exploit the local clustering that
  inverted lists usually exhibit" — exactly what a per-block adaptive-width
  bit-packer does exploit. Decode found a real, stable signal, checked
  directly against the block-padding hypothesis and found to be only
  partially explained by it: with padding fairly removed, EF's decode-win
  rate drops from ~68% to ~55–66% overall (and from ~98% to ~79–94% on
  `n <= 8` lists specifically), but a strong signal survives (`n <= 4`
  predicts the winner at ~97% held-out accuracy, a larger lift than the
  original, uncorrected signal). Skip needs no signal — EF simply wins,
  always, once measured fairly. A real block-max implementation
  (invariant 4, one `u32` max-value per block) gives a genuine ~7× skip-cost
  cut on the 177-of-4,016 lists spanning more than one block — the only
  regime it applies to — closing most but not all of the gap to EF, which
  still wins outright by ~7–8× on those same lists.

  **Phase 2B's own intermediate checkpoint then fired its stated stop
  condition**: computed free from the same corrected data, an oracle
  per-list chooser gives **0.0% ceiling gain on skip** (EF already wins
  every list) and **0.0% ceiling gain on size** (BP128 already wins every
  list) over the better pure baseline — only decode shows a real but modest
  ~8–9% gain. Per the plan's own off-ramp ("if the achievable ceiling is
  already collapsing toward one codec's baseline, stop before finishing the
  full corpus"), **the full Phase 2B harness (alternation-cost and
  chooser-error-cost studies) was not built** — two of three axes show zero
  composition opportunity, and the engineering/verification surface cost
  (doubled conformance surface, a real conflict with invariant 1's
  per-family merge-semantics contract, a new registry field, doubled M4
  burden — drafted without needing benchmark numbers) isn't justified by a
  single-digit gain on the third. **Net recommendation from Phases 0/1/2B
  together**: no codec swap, no adaptive hybrid. BP128 stays the right
  default, its encoder should use a variable-length final block (a small,
  unconditionally good fix, independent of everything else here),
  block-max is worth keeping as-is, and EF's real advantage (native
  compressed-domain skip) is a genuine trade, not a free upgrade — nothing
  measured here shows STRAND should make that trade for its default postings
  codec. Full detail in `docs/research/r2-hybrid-codec-methodology.md` and
  `bench/results/hybrid-codec-pilot.json`. Does not change R2's BP128
  default or require an RFC on its own.
- **R3** — the rotation-provenance mechanism (materialized matrix vs generator+seed,
  M2 RFC); TurboQuant revisit condition.
- **R4** — precise Lucene-vs-tantivy doc-length accounting for the invariant-6 length
  definition. The Lucene half is resolved: RFC 0004
  (`rfcs/0004-analyzer-descriptors.md`) grounds `discountOverlaps` byte-exact against
  Lucene 10.5.1 source and maps it to the descriptor's own
  `counts_overlaps_in_length` field. The tantivy half remains open — no tantivy
  source has been vendored for this yet — and gates M4's tantivy-fork parity work,
  not M1.
- **R5** — exact GCS/Azure conditional-write header semantics (confirm at spec time).
- **R9** — compute-native block layout (gates the R2 bake-off and therefore M1): a
  first decode-throughput measurement now exists
  (`bench/src/codec_decode_throughput.rs`, `bench/results/codec-decode-throughput.json`,
  2026-08-18) — `bitpacking`'s `BitPacker8x` (256-int blocks, hand-tuned AVX2
  intrinsics) versus `spiraldb/fastlanes` (1024-int blocks, portable
  auto-vectorizing scalar) versus `BitPacker4x` (128-int, SSE3), matched bit-width,
  matched 1024-value comparison unit, on one AVX2-capable Intel machine (Core
  i7-10510U). Result, averaged over 10 widths (1–24 bits) of synthetic uniform-random
  data: `BitPacker8x` decodes at ~14.2B values/sec, `FastLanes` at ~10.8B (74–77% of
  `BitPacker8x`'s throughput on average, never faster at any individual width
  measured), `BitPacker4x` at ~8.0B (FastLanes beats this by ~35% on average). **On
  this specific hardware, the hand-tuned AVX2 implementation wins outright** —
  consistent with FastLanes' own pitch being portability (one layout, decent
  performance via auto-vectorization on many ISAs) rather than peak performance on
  any one ISA a hand-tuned kernel specifically targets, not a contradiction of the
  paper's own claims (`references/r9-fastlanes-core-alp-damon-license.md`), which are
  about auto-vectorization eliminating the *need* for hand intrinsics across
  platforms, not about beating hand-tuned intrinsics where they already exist.
  **Measurement reliability, checked rather than assumed:** this ran on a shared,
  multi-user machine (10 concurrent users, load average 3.7–6.7 on 8 cores,
  `powersave` CPU governor, not `performance`) — confirmed bare-metal, not a noisy
  hypervisor (`systemd-detect-virt` reports none, `/proc/stat` steal time is zero),
  but real process contention and frequency scaling are still live confounds. Rerun
  three independent times rather than trusted on one pass: the per-codec average
  throughput has a 0.9–1.8% coefficient of variation across runs, and the ratios
  that actually matter for this comparison are far more stable than the absolute
  numbers — FastLanes/BitPacker8x = 0.74–0.77 and BitPacker8x/BitPacker4x =
  1.77–1.79 in all three runs. The qualitative finding (hand-tuned AVX2 beats both
  portable options; FastLanes beats the SSE3 baseline) holds up under repeated
  measurement on this machine; the precise absolute values/sec figures do not, and
  are not the load-bearing claim here. GHA CI was considered and rejected as a fix:
  its shared runners are also oversubscribed cloud VMs, with the added problem of an
  unpinned, unpredictable CPU generation on every run, which is worse for
  cross-run comparability than a consistent (if loaded) machine. The number that
  actually decides the R2 bake-off default still needs dedicated, reserved hardware,
  not this box and not GHA's shared pool — today's numbers are a directional first
  measurement, not the final one. **FastPFOR now measured too**
  (`bench/src/codec_decode_throughput.rs`, same commit series) — on the same
  uniform-random sweep, `FastPFor256` (pure Rust, `fast-pack/FastPFOR-rs`, Apache-2.0)
  decodes at ~2.7B values/sec, 5.9x slower than `BitPacker8x`'s ~16.1B, which is the
  expected worst case for an adaptive exception-based codec on data with no skew to
  exploit. A second, deliberately skewed distribution (95% of values uniform in
  `[0, 16)`, 5% uniform in `[0, 2^24)` — a synthetic stand-in for delta-gap postings
  with a long tail, not real corpus data) shows the actual tradeoff this codec exists
  for: FastPFOR decodes at ~1.9B values/sec (7.3x slower than `BitPacker8x`'s ~14.0B
  on the same skewed data) but compresses to **178 bytes per 1024 values, versus
  BitPacker8x's 3072** — a 17x size advantage, because plain bit-packing must size an
  entire 256-value block to its single largest outlier, and 5% exception density
  means nearly every block contains one. This 17x gap is a synthetic stress-test
  figure, not a real-corpus one. (A prior version of this entry compared that figure
  against a claimed "FastPFOR's real advantage over BP128-family codecs is 5–15%
  worse ratio for BP128," attributed to the vendored Lemire & Boytsov grounding —
  re-checked while writing the real-corpus measurement below and found not to be in
  that source: `references/lemire-boytsov-simd-bp128.md` states SIMD-FastPFOR's
  ratio is "within 10% of a state-of-the-art scheme (Simple-8b)," a comparison
  against Simple-8b, not BP128, and not the "5–15%" figure. Deleted per `CLAUDE.md`
  §2 rather than left to stand uncorrected — caught by this session's own discipline
  of re-verifying a citation before building on it, the same class of error the
  worked-example TermInfo miscount and the RFC 0006 review's Roaring/CLAUDE.md-M3
  misattribution were.) **Real MS MARCO measurement — 2026-08-18, resolving what the
  entry above calls unmeasured.** `bench/src/msmarco_index.rs` builds a real
  inverted index over a stride-sampled ~520,108-passage subset (every 17th passage,
  spanning the full corpus rather than clustering on its topically-grouped front) of
  the MS MARCO passage collection — fetched via `Tevatron/msmarco-passage-corpus`
  on Hugging Face (the official `msmarco.blob.core.windows.net/msmarcoranking/
  collection.tar.gz` returned HTTP 409 "Public access is not permitted on this
  storage account" as of this fetch date; re-verify before reusing that URL), with
  the exact same 8,841,823-passage count confirmed matching Microsoft's own
  documented corpus size — tokenized with `strand_lexical::analyzer::
  analyze_lucene_en_word_only`, the same chain RFC 0004 implements, not a
  bench-only approximation. Real doc-ID delta-gaps and term frequencies were pooled
  (300 terms sampled per document-frequency decile, all deciles) into 66 and 69 full
  1024-value chunks respectively (68,143 and 71,143 raw pooled values; deciles 0–5
  contributed no gap data since 300-term samples there are entirely doc_freq=1
  hapax legomena, expected under Zipf's law, meaning this measurement is weighted
  toward the mid-to-high document-frequency terms that also dominate real postings
  byte volume). `codec_decode_throughput.rs`'s `real_msmarco_d_gaps`/
  `real_msmarco_term_frequencies` results
  (`bench/results/codec-decode-throughput.json`): on real delta-gaps, FastPFOR
  compresses to 357 bytes/1024 values versus `BitPacker8x`'s 1522 — a **4.26x**
  advantage, substantially narrower than the synthetic 95/5 split's 17x, and
  FastPFOR decodes at ~2.23B values/sec versus `BitPacker8x`'s ~15.6B (7.0x
  slower) — closely matching the synthetic skewed measurement's ~7.3x. On real
  term frequencies, FastPFOR compresses to 77.9 bytes/1024 versus 404.9 (**5.2x**
  advantage) and decodes ~8.3x slower. **The real figure lands well below the
  synthetic 95/5 split's 17x, closer to the same order of magnitude as (though the
  two are not a like-for-like comparison — Simple-8b is not `BitPacker8x`) the
  corrected Lemire & Boytsov figure above**, confirming the synthetic skew
  overstated FastPFOR's real-corpus compression edge while understating nothing
  about its decode-speed cost, which held steady between synthetic and real
  measurements. This is still a ~520K-passage sample (5.9% of the full 8.84M-passage
  corpus), one CPU generation, one shared machine — not the full R2 bake-off, but a
  real, not synthetic, first answer to the specific gap this entry named. ARM/
  non-AVX2 hardware remains completely unmeasured; this is a first, honest data
  point on two axes now (raw decode speed and, for FastPFOR, the decode-speed-vs-
  compression tradeoff, now on both synthetic and real data), one CPU generation,
  one machine, not the full R9 answer. The granularity question (1024-native vs. nested 8×128, preserving
  invariant 4) and the flat-float-vector/GPU-decode assessment (ALP) below remain
  fully open regardless of this measurement — the DaMoN '24 GPU paper's own "1024
  values too large for a single GPU warp" caveat is a real constraint on that
  assessment, not a clean win (`references/r9-fastlanes-core-alp-damon-license.md`).
  The license half of this gate is separately resolved: `cwida/FastLanes` is
  confirmed MIT (`references/r9-fastlanes-core-alp-damon-license.md`),
  Apache-2.0-compatible. Decode-speed and compression measurements now exist on
  both synthetic and real MS MARCO data (above), on shared hardware, one CPU
  generation — the granularity question, the ALP/GPU application-fit assessment,
  and ARM/non-AVX2 hardware all remain open, and the real-corpus measurement above
  is a ~5.9% sample, not the full corpus.
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
  Quickwit inheritance hypothesis (R11(c)). The M0 vendoring deliverable is partially
  done — `references/` holds the R2 and RFC-0002 grounding, all four turbopuffer
  pages (`CLAUDE.md` §7's ~100ms planning figure, the measured p50=874ms/14ms
  cold/cached figures, the published p90s, invariant 9's batched-iterator numbers, and
  the ANN v3 scale claims are now all vendored and checked, not provisional), and both
  adapter LICENSE files (`references/tantivy-LICENSE.txt`,
  `references/faiss-LICENSE.txt`). R1, R3, R4, R5, R6, R7, R8, and R9's core sources
  are now also vendored (2026-08-18; see each track's own `references/r{N}-*.md`
  file). A handful of specific numbers that live in paper bodies rather than
  abstracts were flagged as still-unverified within those files rather than silently
  asserted confirmed: SPANN's replica-vs-index-size figures and its I/O-congestion
  QPS figure, SPFresh's centroid-drift-under-load claim, and Extended-RaBitQ's
  per-bit-width recall figures. `unicode-rs/unicode-segmentation` and the Lucene/ICU
  versioning docs (R4, lower-priority implementation references) were not fetched.
  Remaining M0 vendoring debt is narrow: those flagged numbers, plus whatever R2's
  eventual bake-off and R9's eventual measurement/adoption RFC need beyond what's
  vendored here.
- **Retracted claim: "the full kickstart report."** `CLAUDE.md` and
  `docs/research/README.md` both asserted, at repository seeding, that a longer
  research report existed and would be vendored into `docs/research/` at M0, separate
  from the condensed `README.md` already there. No such document was ever produced,
  and its author confirms none exists — the claim traces to an earlier session
  inventing a deliverable rather than to any real artifact. Both files were corrected
  to drop the claim (2026-08-18); `docs/research/README.md` is the standing research
  source, full stop.
- **Batch-shaped reader trait** (invariant 9's frozen `next_batch()` API shape):
  not yet implemented anywhere in `strand-core`, despite M0's original deliverable
  list claiming it. Carries forward as an M1 prerequisite — M1's postings kernels
  are its first real consumer, and the trait should land with them rather than as
  an unconsumed abstraction (`docs/milestones.md` M0 records the same).
- **Raw-mappable blob alignment — resolved 2026-08-18.** `SegmentBuilder::build`
  (`crates/strand-core/src/segment.rs`) now pads a raw-mappable blob's region up to
  its declared `alignment` with zero bytes before placing it, honoring
  `spec/container.md` §5's MUST; chunk-compressed blobs are never padded regardless
  of their (meaningless) `alignment` field. Test-first, two new tests plus the full
  workspace suite and `cargo clippy -- -D warnings` clean; the existing
  `toy-segment.bin` golden file needed no regeneration (its one blob sits at offset
  0, trivially aligned). Recorded in RFC 0001's Discussion section
  (`rfcs/0001-container-rowid-manifest.md`), which also names the byte-determinism
  reason padding content had to be pinned to zero rather than left unspecified.
- **TLA+ model correspondence gap — resolved 2026-08-18.** `verification/
  manifest.tla`'s `ProposeSnapshot(w)` and `ReadCurrent(w)` both gained the
  outcome branches this entry previously flagged as missing before the TLAPS
  proof phase: `ProposeSnapshot(w)` a collapsed failure branch (real
  `StoreError::Io`/`StoreError::Ambiguous` from `put_if_absent`, both mapping to
  the same terminal outcome since that write's path is attempt-unique and needs
  no disambiguation), `ReadCurrent(w)` an `Expired` self-transition (the real
  `read_current` loops on one unboundedly, unlike the reader path's bounded
  refresh). TLC re-verified clean: 591 distinct states (1793 generated, depth
  14, up from 561/1487), all seven invariants still holding. Recorded in RFC
  0002's new Discussion section (`rfcs/0002-manifest-formal-verification.md`)
  and `verification/README.md`'s state-count baseline. The TLAPS phase can now
  build on a grammar that matches the real code's outcome sets on every writer
  action, not just the pointer CAS.
