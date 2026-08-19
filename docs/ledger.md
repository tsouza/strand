# Settled vs open ledger

Extracted (with version self-references removed) at repository seeding from the
seed constitution's ledger appendix — now the first half of `CLAUDE.md`'s single
Appendix, which points here. This is the ledger `CLAUDE.md` §5 and `docs/data-structures.md`
point to: what's settled and not to be re-litigated, and what's open and requires an
RFC backed by the research tracks in `docs/research/README.md`.

**Settled (apply, do not re-litigate):** chunk-shaped cold access with the one-wave
addressability rule (invariant 3); end-to-end cold accounting from the pointer read
(`CLAUDE.md` §7); cluster-family as the cold-native vector shape; graph blobs as warm-tier;
RaBitQ default with kernel-per-bit-width and rotation descriptor; the
rotation-provenance mechanism (materialized state for both registered rotator
types, never a seed, RFC 0010); the cluster-family blob wire format (RFC 0010:
flat vectors, quantization descriptor, navigation tier, posting lists) for
1-bit RaBitQ specifically — multi-bit still open; Roaring; FST
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
- **R1** — the cluster blob's *concrete layout* is now resolved by RFC 0010
  (`family_id = 3`, four blob types, `spec/vectors.md`); what remains open is the
  *sizing law's* validation vs segment scale and replication (gates M3), and the
  graph-blob half untouched by RFC 0010. RFC 0010's own napkin math already moves
  this forward with real numbers, not just leaves it open: real tier-1 cost is
  ~131 MB per million 768d vectors before replication (corrected from the
  previously stated ~100 MB, `docs/data-structures.md`), and a provisional,
  explicitly-unverified replica-8-equivalent estimate of ~227 MB (~2.27× the
  budget) — over half the margin to the kill criterion below, not close to
  tripping it, but real headroom consumed. Kill criterion, falsifiable: if tier-1
  exceeds `CLAUDE.md` §7's provisional 100 MB cold-open byte budget — or its
  measured M0 replacement — by more than ~4× at target segment scale, cold vector
  search is narrower and the mission sentence changes again. Still open: a real
  fetch of SPANN's body figures to replace the provisional replication estimate; a
  real M0-style measured byte-open benchmark for this blob family (RFC 0010's own
  math is arithmetic, not a benchmark); the graph-blob ordering algorithm
  (Starling's block shuffling is the literature; pick with evidence), entirely
  untouched by RFC 0010, which is 1-bit cluster-family only.
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
  padding bug, not a real result). Mean bytes/list, padded `BitPacker8x`
  ~672–673 (unfairly inflated) vs. variable-final-block `BitPacker8x` ~149.1
  (fair) vs. EF ~295.2 (`bench/results/hybrid-codec-pilot.json`, stable across
  reruns). The mechanism: `sucds::mii_sequences::
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
  original, uncorrected signal). This sample is 69% lists of length `<= 8`
  (2,788 of 4,016, `bench/results/hybrid-codec-pilot.json`) — real short-list
  dominance under Zipf's law, not an edge case — and on exactly that
  short-list group a representative rerun gives real mean decode-cost figures
  (nanoseconds/list): `BitPacker8x` fixed-block ~117–137, `BitPacker8x`
  variable-final-block ~89–100, EF ~58–66 (range across three reruns
  2026-08-18/19; a specific rerun: 129.6/94.3/61.3). Skip needs no signal — EF simply wins,
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
  **Correction, 2026-08-19: the ~149-bytes/list mean above, real and
  correctly measured on its own 4,016-list stratified sample, does not
  generalize by linear extrapolation to the full vocabulary.** Answering
  the user's own question — "will we be able to validate this is solid
  work by benchmarking against existing battle-tested software?" — a real
  tantivy index was built over the identical MS MARCO corpus sample and
  token stream (`bench/src/tantivy_index.rs`; every document fed as a
  `PreTokenizedString` from this project's own analyzer output, so
  tantivy's tokenizer is never invoked, isolating the comparison to format
  efficiency): tantivy's real `.idx` (postings) file is `29,504,002` bytes
  (`≈ 29.50 MB`), its real `.pos` (positions) file `18,935,668` bytes
  (`≈ 18.94 MB`), its real `.term` (term dictionary) file `8,045,330` bytes
  (`≈ 8.05 MB`), total on-disk `59,107,627` bytes (`≈ 59.11 MB`;
  `bench/results/tantivy-index-benchmark.json`; mean term-query latency
  ~95.7μs, mean phrase-query latency ~414.7μs, single-threaded, on this
  same sample). RFC 0007's own `~61.6 MB` postings estimate (the
  extrapolation above × the full 413,364-term vocabulary) was `2.09×`
  tantivy's real number — suspecting the extrapolation rather than the
  codec, `bench/src/msmarco_index.rs` was extended to call
  `strand_lexical::postings::build_postings` (the real, shipped RFC 0007
  implementation, not a projection) across every one of the real 413,364
  terms and sum actual bytes: `29,489,488` bytes (`≈ 29.49 MB`,
  `bench/results/msmarco-real-postings-sample.json`'s
  `stats.real_postings_bytes`) — a `0.05%` difference from tantivy's real
  number, essentially an exact match. The codec choice is validated, not
  undermined, by this correction; only the earlier size *estimate* was
  wrong. RFC 0008's own positions bound (`≈ 19.28 MB` tighter bound) held
  up well independently against tantivy's real `.pos` (`≈ 18.94 MB`, `1.8%`
  off) — it was not built on the flawed extrapolation and did not need
  correcting. Both RFC 0007 and RFC 0008 carry this correction in their own
  Discussion sections; the corrected combined cold-open figure for postings
  + term-info + positions is `≈ 60.35 MB` to `≈ 68.46 MB` (60–68% of the
  100 MB budget), not the `~92.5–100.6 MB` RFC 0008 originally reported.
- **R3** — the rotation-provenance mechanism is now resolved: RFC 0010
  (Approved) registers materialized rotation state for both registered rotator
  types (never a seed), grounded in the reference implementation's own
  `rotator.hpp` source (`references/rabitq-library-rotator-source.md`) — real
  evidence, not asserted. Still open: the TurboQuant revisit condition (unchanged
  from before).
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
- **Real cold-open with real, phrase-query-capable content, against real
  MinIO — resolved 2026-08-19.** `bench/src/cold_open.rs` (M0) proves
  invariant 3's ≤4-GET bound against a segment holding 8 literal placeholder
  bytes — real for the container/manifest mechanics, but it has never
  proven the bound holds for a segment with real lexical content, and
  nothing measured how long "cold" to "first real query result" actually
  takes. `bench/src/field_cold_open.rs` closes that gap: a real field
  (`strand_lexical::field::build_field` over real MS MARCO passages,
  RFC 0005/0007/0008's term-dictionary/postings/positions blobs) is
  committed to a real MinIO container (`testcontainers`, self-contained),
  then repeatedly opened cold and queried — a real BM25 search and a real
  phrase query — with GET count asserted, not just measured, per
  `CLAUDE.md` §7. Real results at two scales: 5,002 docs (1.45 MB segment)
  — **3 GETs per open, p50 4.70ms**; 50,238 docs (8.57 MB segment) — **3
  GETs per open, p50 9.12ms**. Critically, **running a real BM25 query and
  a real phrase query after open costs the identical 3 GETs as opening
  alone**, at both scales — a real, measured confirmation of invariant 3's
  one-wave rule for an actual query, not just the open, the first time this
  has been checked with content that can answer a query at all. Segment
  size growing 5.9× (1.45 MB → 8.57 MB) left GET count unchanged and
  roughly doubled latency (bandwidth-bound transfer time, not round-trip
  count) — exactly the round-trip-bound-not-size-bound behavior `CLAUDE.md`
  §7 claims. This is the benchmark that actually tests STRAND's stated
  thesis ("query in place on S3... never dependent pointer-chasing",
  `CLAUDE.md` §1) with real, phrase-searchable content, not a toy segment —
  and it is a claim tantivy cannot be compared against at all, since it has
  no object-storage-native open path; the honest "outperforms" claim this
  project can make right now is this one, not total on-disk size (which
  RFC 0008's positions blob currently loses on, named honestly in the
  entry above this one). MinIO runs on `localhost` (not real internet
  latency), so these numbers confirm the **GET-count** half of the claim
  precisely, matching M0's own already-recorded caveat that the real-network
  tail-latency figure remains a separate, open measurement
  (`CLAUDE.md` §7's placeholder).
- **Tantivy importer — resolved 2026-08-19.** `CLAUDE.md`'s repository
  shape names `strand-tools convert` as the declared home for "tantivy/CIFF
  importers"; it now exists for tantivy. `crates/strand-tools/src/convert.rs`'s
  `import_tantivy_field` opens a real tantivy index via its own real reader
  API — `InvertedIndexReader::terms()`/`TermDictionary::stream()` to
  enumerate every real term, `read_postings_from_terminfo` with
  `IndexRecordOption::WithFreqsAndPositions` to read each term's real
  postings and positions — grounded against the tantivy repository's own
  `examples/iterating_docs_and_positions.rs` (fetched live, not recalled)
  rather than reimplementing tantivy's binary format by hand. Extracted
  `(term, doc_ordinal, positions)` triples feed directly into
  `strand_lexical::field::build_field_from_postings`, a new function
  `field.rs` gained specifically so this importer reuses the exact same
  real, already-tested blob-building code `build_field` uses for text
  input rather than duplicating it — the same reuse discipline this session
  applied throughout (`postings.rs` reusing `scalar_pack`, `positions.rs`
  reusing `postings.rs`'s helpers). Tantivy's segment-local `DocId` becomes
  the STRAND local ordinal directly, since both are already dense
  `0..num_docs` spaces — no remapping needed for the case this importer
  accepts. `strand-tools convert --index-dir <path> --field <name> --output
  <path>` wires it into the CLI, writing a real segment file via
  `SegmentBuilder::build` (no manifest/store involvement — a bare segment
  file, matching what a CLI conversion tool needs, not a commit). Verified
  twice: a real Rust unit test (`crates/strand-tools/src/convert.rs`'s own
  `#[cfg(test)]` module — an inline unit test, matching its existing
  `inspect` module's own convention) builds a real tantivy index, imports
  it, assembles a real STRAND segment, and runs real term and phrase
  queries against it, including a true positional match and a true
  negative (two terms that co-occur but are never adjacent); a manual CLI
  smoke test round-tripped `convert` into `inspect` against a real 527-byte,
  4-blob, checksum-valid segment on disk. One real bug caught and fixed
  along the way: the test's first attempt at building a 3-document tantivy
  index used the default (multi-threaded) writer, which split those 3
  documents across 3 segments — `import_tantivy_field`'s own single-segment
  check correctly rejected it (`MultiSegment(3)`), and the fix was
  `writer_with_num_threads(1, ...)`, matching the exact reasoning tantivy's
  own vendored example already states for using it. Deliberately narrow,
  named scope: single-segment, deletion-free tantivy indexes only
  (multi-segment merge and deletion-vector support are real, separate,
  unattempted follow-on work); positions are always imported, a source
  field indexed without them is out of scope for now. Prompted directly by
  "did you test this?", three real gaps in the above coverage were closed
  the same session: multi-segment rejection had only ever been observed
  *accidentally* (the bug above), never deliberately exercised — fixed with
  `rejects_a_real_multi_segment_index`, which uses `NoMergePolicy` plus two
  separate commits and asserts `segment_readers().len() == 2` as a
  precondition, so the test cannot silently stop testing what it claims to;
  the deletion-rejection path (`HasDeletions`) had no test at all — fixed
  with `rejects_a_real_segment_with_deletions`, using `delete_term` and
  asserting `num_deleted_docs() == 1` as a precondition; and `doc_lengths`
  was computed but never directly asserted — fixed by adding
  `assert_eq!(field_blobs.doc_lengths, vec![3, 3, 3])` to the existing
  happy-path test.
  Prompted directly by "test at real scale with the MS MARCO tantivy
  index," the 3-document synthetic corpus above is not enough to trust the
  importer at scale, so `crates/strand-tools` gained a `[lib]` target
  (`src/lib.rs`, re-exporting `convert` and `inspect`; `main.rs` now
  consumes them via `use strand_tools::{convert, inspect}` instead of
  `mod` declarations) specifically so `bench/` could call
  `import_tantivy_field` as a real function rather than shelling out to the
  built binary. The new `bench/src/tantivy_import_scale.rs` builds the same
  real MS MARCO sample two independent ways — natively via
  `strand_lexical::field::build_field` (the same path
  `bench/src/field_end_to_end.rs` already validated at scale) and via a
  real single-threaded tantivy index fed identical tokens through the same
  `PreTokenizedString` trick `bench/src/tantivy_index.rs` uses, then
  `strand_tools::convert::import_tantivy_field` on that index — and
  compares the resulting `FieldBlobs` byte-for-byte, not just
  query-for-query. Real result: **byte-identical** on both real runs — at
  5,002 real documents (term_dict 137,086 bytes, term_info 586,180 bytes,
  postings 370,293 bytes, positions 271,714 bytes, identical on both paths)
  and at 50,238 real documents (term_dict 604,862 bytes, term_info
  2,417,940 bytes, postings 3,051,551 bytes, positions 2,147,667 bytes,
  identical on both paths, including `doc_lengths`). The larger scale
  specifically exercises multi-block postings/positions encoding — many
  terms have `doc_freq` and `total_term_freq` above 256 at that size, a
  path the smaller scale and the original 3-document test could not stress
  — so two independent construction paths converging on identical bytes at
  that scale is materially stronger evidence of correctness than the
  spot-check queries above, though it is still bounded to the same
  single-segment, deletion-free, positions-always-on scope stated above.
- **RFC 0010 (vector blob family, cluster-native cold-open index) —
  Approved 2026-08-19.** M2's opening RFC, prompted directly by "draft the
  M2 vector RFC" after M1's gating deliverables (postings, positions, term
  dictionary, block-max, Roaring, scoring profiles, analyzer descriptor
  schema, the tantivy importer verified at real scale) were all confirmed
  complete. Registers `family_id = 3` ("vector") with four blob types —
  flat vectors, a quantization descriptor, the cluster navigation tier, and
  the cluster posting lists — implementing R1's already-settled
  cluster-shaped cold architecture (`docs/data-structures.md`) and
  resolving R3's own open rotation-provenance question. Adversarial review
  found **6 Critical, 14 Important, 7 Minor**, all fixed — the largest
  finding count of any RFC this session, matching the scope of introducing
  an entire new blob family with real, non-trivial arithmetic. The single
  most consequential Critical finding: the RFC's own load-bearing sizing
  formula was cited to a vendored reference file
  (`references/rabitq-library-compact-code-source.md`) that documents a
  *different*, non-batched, per-vector code layout — the actual
  FastScan-batched formula (`BatchDataMap`, `data_layout.hpp`) had been
  fetched live in-session but never vendored as its own file, and was cited
  to the wrong one. Fixed by vendoring the real source properly
  (`references/rabitq-library-ivf-and-batch-layout-source.md`,
  `references/rabitq-library-index-overview.md`) and correcting every
  citation — a direct, textbook instance of the exact "sibling fetch, never
  vendored" failure `CLAUDE.md` §3 exists to prevent, caught by the review
  it exists to prevent it from surviving. A second Critical finding
  inverted its own cited precedent: the RFC justified full-precision
  centroids by citing DiskANN's two-tier model as keeping *routing*
  structures full-precision, when `docs/lineage.md`'s own text (which the
  RFC cited) says the opposite — DiskANN quantizes routing and keeps full
  precision for reranking. Fixed by citing SPANN instead (which does keep
  centroids full-precision) and removing the inverted DiskANN claim
  everywhere it appeared. A third recomputed the corrected sizing law
  properly: the original napkin math used the 108/116-bytes-per-vector
  *full-batch floor* as if it were a real-world average, missing the
  partial-batch padding waste its own Design section named two paragraphs
  earlier; recomputed at a realistic 250-vectors/cluster average and 4,000
  clusters, real tier-1 cost is **~131 MB per million 768d vectors**
  (not the RFC's own first-draft ~128.4 MB, and not
  `docs/data-structures.md`'s pre-RFC ~100 MB figure), ~31% over the
  provisional 100 MB cold-open byte budget — corrected in
  `docs/data-structures.md` directly. A fourth found the replication
  estimate's own arithmetic error (a replica-2→8 ratio applied to a
  replica-1 baseline, silently omitting the 1→2 step) and its overclaimed
  grounding (labeled "real, measured" while its own cited source,
  `references/spann-neurips2021.md`, explicitly flags its 13.0/7.5 GB
  figures as not independently re-confirmed) — fixed by relabeling the
  resulting ~227 MB (~2.27× the budget) figure as a provisional,
  conservative lower-bound estimate throughout, with a real fetch of
  SPANN's body figures named as owed follow-on work. A fifth found a real
  byte-determinism gap invariant 11 requires resolved: a partially filled
  FastScan batch's unused lanes had no specified content — fixed with a
  new normative zero-fill rule (RFC Design §4, `spec/vectors.md` §4) and a
  new Invariant-11 checklist item. A sixth found the `cold-fetchable` tier
  label applied to the posting-list blob without the design actually
  fetching it wholesale (only `nprobe` of its clusters) — fixed with an
  explicit clarifying paragraph distinguishing bytes fetched at open
  (~12.4 MB) from the whole-corpus sizing figure (~131 MB), both now stated
  side by side. Important findings fixed: invariant 9 (batch-shaped reads,
  scalar-kernel normativity) was claimed in the header but never engaged —
  fixed with a new Design §9; `MatrixRotator`'s unpadded dimensionality
  contradicted the code-region formula's own padding requirement and left
  the row-id array potentially misaligned — fixed by requiring STRAND's own
  64-multiple padding for both registered rotator types, resolving the
  alignment gap as a side effect; no `alignment` field had been declared
  for any of the four raw-mappable blobs (`spec/container.md` §5's own
  requirement) — fixed (`alignment = 8` on all four); the round-trip count
  didn't reproduce from the RFC's own enumerated stages (5–6 stated, 6–7
  actually enumerated) — fixed; deletion-vector filtering and closure-
  replication deduplication were absent from query resolution — fixed with
  two new normative steps; u64 row-ids vs. cheaper u32 local ordinals had
  no stated rationale — fixed by citing `spec/row-ids.md` §3's own
  rebalance-merge case directly; the RFC had no prior-art paragraph despite
  `CLAUDE.md` §4's requirement, and the one precedent that matches this
  design most closely (Lance's row-ID-linked auxiliary quantized-vector
  file) was never mentioned — fixed; three files (the spec chapter,
  `docs/data-structures.md`, `spec/container.md`) asserted RFC 0010 was
  already Approved while its own Status line read Draft — resolved by this
  entry itself, now that the review is complete and every finding fixed.
  Deliberately narrow scope, matching this project's "implement narrow, one
  coherent slice per session" discipline: 1-bit RaBitQ only (multi-bit
  Extended-RaBitQ, whose ex-code byte formula this session also fetched but
  did not wire-format-register, is a named follow-on RFC); the graph-blob
  warm-tier family (R1's second half) is untouched; SPANN-style closure
  replication's construction algorithm and metadata slot are named but not
  designed, and the review's own findings sharpened this into an explicit
  admission that M2's own milestone deliverable (the replication knob) is
  not fully met by this RFC alone; cross-segment codebook-sharing/
  retraining at merge time and the FastScan codec's intra-batch bit/lane
  order (grounded only at the byte-offset level, not the bit level) are
  both named as real, load-bearing, unresolved gaps rather than assumed
  away. `CLAUDE.md` §7's own "roughly one segment per million 768d
  vectors" line is updated in place to ~760,000, the direct consequence of
  the corrected sizing law.
- **RFC 0009 (per-term overhead reduction) implemented — resolved
  2026-08-19.** Both fixes landed in `crates/strand-lexical/src/positions.rs`
  and `crates/strand-lexical/src/term_dictionary.rs`. Fix 1
  (`postings_block_pos_prefix[0]` omission) is a breaking, in-place change
  to the positions blob's already-shipped layout (`blob_type_id = 3`
  unchanged, but its bytes now mean something different) — RFC 0008's
  original 12-byte worked example and golden file are retired, replaced by
  RFC 0009's 8-byte ones (`conformance/positions/toy-positions.bin`).
  Fix 2 (the 16-byte short term-info record) is additive — a new
  `blob_type_id = 4`, `ShortTermInfoStore`, `TermInfo::encode_short`/
  `decode_short`, and `build_term_dictionary_short`, alongside the
  untouched 28-byte record. Both worked examples confirmed byte-exact
  against new golden files and property-tested (`crates/strand-lexical/
  tests/positions_worked_example.rs`, `positions_round_trip.rs` — updated
  in place for the new layout; `short_term_info_worked_example.rs`,
  `short_term_info_round_trip.rs` — new). **Fix 1's real-world payoff
  confirmed exactly, not just predicted**: re-running `bench/src/
  field_end_to_end.rs` against the same real MS MARCO samples the RFC's
  own napkin math used, the positions blob shrank from `620,503` to
  `493,359` bytes at 10,003 documents and from `4,678,608` to `4,135,112`
  at 100,476 — both matching the RFC's predicted figures to the byte
  (`bench/results/field-end-to-end-10003.json`, `-100476.json`,
  regenerated post-implementation, superseding the pre-fix numbers the RFC
  itself cites as history). **Fix 2 is now wired into `field.rs` too
  (2026-08-19) — `build_field_without_positions` builds the short,
  positions-free term-info record for good, and its predicted payoff is
  now confirmed exactly, the same way Fix 1's was**: re-running the same
  real MS MARCO samples, `term_info` shrank from `890,008` to `508,576`
  bytes at 10,003 documents (`381,432` bytes saved — exactly RFC 0009's
  own predicted `31,786 * 12`) and from `3,804,472` to `2,173,984` at
  100,476 (`1,630,488` bytes saved — exactly `135,874 * 12`), with the
  entire positions blob (`493,359`/`4,135,112` bytes respectively) not
  written at all (`bench/results/field-end-to-end-10003.json`,
  `-100476.json`, both regenerated again). `FieldReader::open` now tries
  `blob_type_id = 1` then falls back to `blob_type_id = 4` (a new
  `TermInfoSource` enum internal to `field.rs`), and
  `crates/strand-lexical/tests/field_end_to_end.rs` gained a real
  end-to-end test proving the opt-out path builds a smaller segment, still
  answers term and BM25 queries correctly, and correctly reports empty (not
  an error) for phrase queries against a field with no positions blob at
  all. `field.rs`'s own multi-field-addressing caveat (RFC 0008's/RFC
  0009's Non-goals) is unchanged: nothing here solves which registry entry
  belongs to which field, only which shape a given entry uses once found.
- **RFC 0008 (positions) implemented — resolved 2026-08-19.**
  `crates/strand-lexical/src/positions.rs` builds and reads the positions
  blob exactly as `spec/positions.md` specifies: `build_positions` takes a
  term's postings-ordered within-document position lists and produces the
  `total_term_freq` header, `postings_block_pos_prefix` bridge, `pos_widths`,
  and delta stream; `PositionsReader::positions_for_doc` resolves a targeted
  lookup (given a postings-block index, a local prefix `tf`, and a target
  `tf` — exactly what a real `spec/postings.md` §6 skip query already
  yields) without decoding any block it doesn't need, and `decode_all` walks
  the whole blob alongside a term's postings `tf` array. Reuses
  `postings.rs`'s `scalar_pack`/`scalar_unpack`/`block_count_for`/
  `block_real_len` directly (now `pub(crate)`), per the RFC's own stated
  plan, rather than duplicating them. RFC 0008's own worked example
  round-trips byte-exact against the new
  `conformance/positions/toy-positions.bin` golden file (`06 00 00 00 00 00
  00 00 03 33 32 03`, 12 bytes — the RFC's own ACPR-corrected figure), with
  targeted-lookup resolution checked for all three documents
  (`crates/strand-lexical/tests/positions_worked_example.rs`).
  Property-tested (`crates/strand-lexical/tests/positions_round_trip.rs`):
  full decode and targeted lookups on arbitrary multi-document,
  multi-position-per-document inputs, checked against the *original input
  directly* (not against this blob's own decode path, avoiding a tautology)
  — including boundary cases that stress `doc_freq`-derived postings-block
  counts and `total_term_freq`-derived position-block counts independently,
  since they're genuinely different counts. Not yet wired into
  `crates/strand-lexical/src/field.rs`'s segment-lexical integration
  layer — a field's `build_field`/`FieldReader` still never populate or read
  `positions_offset`/`positions_length`, so phrase queries through the real
  end-to-end path remain a real, separate follow-on, not silently assumed
  done. This closes the one M1 lexical-family blob RFC 0007 explicitly
  deferred; `spec/positions.md` §9 and `spec/term-dictionary.md` are updated
  to reflect implemented, not just designed.
- **Segment↔lexical integration and a first real query path — resolved
  2026-08-19.** Until now, `strand-core`'s segment/container/manifest layer and
  `strand-lexical`'s term-dictionary/term-info/postings blobs had never been
  composed: every existing test built one blob type in isolation and checked
  it against a golden file or an in-memory round trip, and `bench/src/
  cold_open.rs`'s own MinIO benchmark opens a segment containing literal
  placeholder bytes (`0x2A, 0x2B`), not real content — a gap a maturity
  assessment surfaced directly (asked "is STRAND at the maturity/specced/
  implemented enough so we can benchmark it") and confirmed there was no
  function anywhere that resolved a query string to a match set or a score.
  `crates/strand-lexical/src/field.rs` closes it: `build_field(docs: &[&str])`
  analyzes real text (the same `analyze_lucene_en_word_only` chain
  `bench/src/msmarco_index.rs` uses) into a field's term-dictionary, term-info,
  and postings blobs; `FieldBlobs::to_blob_specs()` wraps them as
  `strand_core::segment::BlobSpec`s with each blob's already-registered
  classification (RFC 0005/0007's `family_id = 1`, `blob_type_id` 0/1/2,
  `storage-class: raw-mappable`, `tier: cold-fetchable`); `FieldReader::open`
  reads them back from a resident segment's decoded blob registry, and
  `FieldReader::lookup`/`search_bm25` resolve a real term to real
  `(doc_ordinal, term_freq)` matches or BM25-ranked scores
  (`strand_core::scoring::Bm25Profile`, RFC 0003). `crates/strand-lexical/
  tests/field_end_to_end.rs` is the first real end-to-end test in the repo:
  real document text → real blobs → a real segment → `strand_core::manifest::
  commit` against a real (in-memory) `ConditionalStore` → `read_snapshot` →
  footer/hotcache decode → `FieldReader` → real term lookups and a real
  BM25-ranked search, all passing. Deliberately narrow scope, stated in the
  module's own doc comment rather than silently assumed: one field per
  segment (multi-field blob addressing is still unsolved project-wide, RFC
  0008's own Non-goals), no positions (RFC 0008 is now implemented in
  `crates/strand-lexical/src/positions.rs`, 2026-08-19, but not yet wired
  into `field.rs` — `positions_length` is still always `0` here, a real
  follow-on, not a blocked one), no filter bitmaps, no compaction or
  merge, and `dl`/`avdl` for scoring are caller-supplied rather than
  blob-backed since document-length storage isn't a registered blob anywhere
  yet (RFC 0007's own Non-goals). Does not require an RFC: no new byte
  layout or registry entry is introduced, only composition of what RFC
  0003/0005/0007 already approved. **Scale-tested against real MS MARCO data,
  not just the 3-document toy corpus**: `bench/src/field_end_to_end.rs` runs
  the identical pipeline over real passages. At 10,000 real documents
  (25,737 distinct terms): `build_field` 0.50s, a 1.52 MB segment, cold open
  (in-memory store, so decode cost only, not network latency) 0.28ms, BM25
  queries in 19–65μs. At 100,000 real documents (107,544 distinct terms):
  `build_field` 4.65s, a 9.49 MB segment, cold open 2.64ms, queries in
  184–448μs. This run caught a real, if minor, usability finding: querying
  with a raw, unstemmed word (`"energy"`) returns no matches even though the
  term is genuinely present, because `FieldReader::lookup` does raw-string
  FST lookup and documents are indexed post-stem (`"energy"` → `"energi"`).
  Not a bug in the write or read path — confirmed by running the same word
  through `analyzer::analyze_lucene_en_word_only` before querying, which
  resolves correctly (117 matches at 10K docs) — but a real reminder that a
  future real query-execution layer (not yet built, `docs/milestones.md`
  M5) needs to run query text through the same analyzer chain a field's
  documents were indexed with, not accept raw strings. **Compared directly
  against real tantivy on the identical stride-sampled document set**
  (`bench/src/tantivy_index.rs`, same corpus, same stride formula, so both
  tools index the exact same passages, not just the same count): at 10,003
  docs, STRAND's postings blob (684,248 bytes) is `1.5%` smaller than
  tantivy's real `.idx` (694,748 bytes); at 100,476 docs, `6.2%` smaller
  (5,949,990 vs. 6,340,266 bytes) — consistent with the earlier ~520K-passage
  finding, the postings codec continues to tie or slightly beat tantivy's
  real one at these scales too. Two real, honest asymmetries, not hidden:
  (1) STRAND's `term_dict + term_info` (1.10 MB at 10K, 4.76 MB at 100K) is
  `~46–48%` *larger* than tantivy's real term dictionary (746,482 bytes at
  10K, 3,261,611 at 100K) — `TermInfo`'s 28-byte fixed record reserves 12
  bytes/term for `positions_offset`/`positions_length` that go unused since
  positions isn't implemented, real dead weight at real vocabulary scale,
  worth revisiting once positions lands; (2) total segment size (1.79 MB at
  10K, 10.71 MB at 100K) looks smaller than tantivy's total index (1.97 MB,
  14.12 MB) but this is not a fair win — tantivy's total includes a real
  `.pos` file (positions, functionality STRAND doesn't have yet), a
  `fieldnorm` blob, and a document store STRAND's minimal `build_field`
  never builds; the honest comparison is postings-to-postings, above. Query
  latency is explicitly **not** compared as a competitive claim: STRAND's
  `search_bm25` currently fully decodes and sorts every match with no top-K
  short-circuit, while tantivy's benchmark uses a real, optimized
  `TopDocs::with_limit(10)` collector — different amounts of work, so
  STRAND landing in the same tens-to-hundreds-of-microseconds range (21–85μs
  at 10K, 185–449μs at 100K, vs. tantivy's 500-query means of 20.6μs/50.7μs)
  is a mildly encouraging signal, not a result. Adding real top-K
  short-circuiting to the query path is a real, separate follow-on, not
  done here.
  **Re-run with positions now implemented and wired into `field.rs`
  (2026-08-19), reversing the earlier "STRAND's total is smaller" reading —
  that reading was correctly caveated as unfair at the time (STRAND had no
  positions yet), and this is the honest completion of that caveat, not a
  contradiction.** With both sides now carrying real positions data: at
  10,003 docs, STRAND's total segment (2,408,400 bytes: term_dict 213,641 +
  term_info 890,008 + postings 684,248 + positions 620,503) is `22.5%`
  *larger* than tantivy's real total (1,965,977 bytes); at 100,476 docs,
  `9.0%` larger (15,389,516 vs. 14,119,646 bytes). Postings alone still
  ties/beats tantivy (unchanged: `1.5%`/`6.2%` smaller) — that finding
  holds. The new gap is `positions` itself: STRAND's positions blob is
  `33.2%` larger than tantivy's real `.pos` at 10K, `16.8%` larger at 100K
  (`bench/src/field_end_to_end.rs`'s own real numbers, run on the identical
  stride-sampled corpus `bench/src/tantivy_index.rs` used). This is not a
  surprise RFC 0008 failed to predict — its own Napkin math already named
  per-term fixed overhead as expensive (`total_term_freq`: 4 bytes,
  `postings_block_pos_prefix`: at least 4 bytes, `pos_widths`: at least 1
  byte — a 9-byte-minimum floor per term regardless of how few positions it
  actually has, `spec/positions.md` §4's own stated minimum), and under
  Zipf's law most terms are exactly this low-frequency case. Combined with
  `TermInfo`'s already-named 12-byte-per-term overhead (no longer unused
  dead weight now that positions is real, but `TermInfo`'s 28-byte flat
  record is still less compact than tantivy's FST-based term dictionary at
  these vocabulary sizes — `46–48%` larger, unchanged from the earlier
  finding), **the honest conclusion is: STRAND currently outperforms
  tantivy specifically on postings-list compression, and currently loses on
  total index size once positions and term metadata are counted.** The
  "outperform" claim this project can currently stand behind is narrower
  than "STRAND's format is more compact" — it is "STRAND's registered
  postings codec (RFC 0007) matches or beats a real, battle-tested engine's
  own codec." Shrinking the per-term fixed overhead in positions/term-info
  is real, identified, un-started follow-on work, not attempted in this
  session — see the earlier TermInfo discussion this same investigation
  already had with the user (a shorter per-field term-info record variant,
  gated on an RFC since it touches RFC 0005's shipped byte layout).
- **Batch-shaped reader trait — resolved 2026-08-19.** `crates/strand-core/src/
  batch.rs` now defines `BatchReader` (`next_batch(&mut self, out: &mut Vec<Item>)
  -> usize`, invariant 9's frozen shape), and `PostingsReader::batches()`
  (`crates/strand-lexical/src/postings.rs`) is its first real consumer — one block
  per batch, the natural unit since a block is already the reader's unit of
  independent decode. Property-tested (`crates/strand-lexical/tests/
  postings_batch_reader.rs`): batches concatenate to exactly `decode_all`'s output,
  batch boundaries land on block boundaries (`BLOCK_LEN`, then the remainder), and
  an exhausted reader returns `0` without appending. Invariant 9's own
  "recommended batch-size range... settled by benchmark (R2)" remains open — this
  consumer's per-block batch size is dictated by the postings layout, not a
  general recommendation, and R2 has not yet settled one.
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
