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
  previously stated ~100 MB, `docs/data-structures.md`), and a replica-8-equivalent
  estimate of ~227 MB (~2.27× the budget) — over half the margin to the kill
  criterion below, not close to tripping it, but real headroom consumed. This
  estimate's 1.73× replication ratio (13.0 GB vs 7.5 GB at replica 8 vs 2) is now
  a real, body-sourced figure (2026-08-19): SPANN's own paper
  (`arxiv.org/abs/2111.08566`) was fetched in full and confirmed to contain no
  GIST1M dataset and no index-size figure at any replica count; the real figure
  lives in the companion cloud-native benchmark paper (Li et al.,
  `arxiv.org/abs/2511.14748`, Table 4, §5.3), also fetched in full and quoted
  verbatim (`references/spann-body-figures.md`). The ratio itself is unchanged by
  this fetch — only its citation and confidence label are — and a real gap
  remains: neither paper reports a replica=1 index size, so the ~227 MB figure is
  still an extrapolation across the unmeasured 1→2 step, stated as a conservative
  lower bound, not a measurement. Kill criterion, falsifiable: if tier-1
  exceeds `CLAUDE.md` §7's provisional 100 MB cold-open byte budget — or its
  measured M0 replacement — by more than ~4× at target segment scale, cold vector
  search is narrower and the mission sentence changes again. A real M0-style
  measured byte-open benchmark for this blob family is now done, not just arithmetic
  (`bench/src/vector_cold_open.rs`, 2026-08-19): a real four-blob-type segment
  (10,000 real 768d vectors, real k-means into 400 clusters, real `FhtKacRotator`
  rotation, real 1-bit RaBitQ quantization) committed to real MinIO and reopened
  cold 30 times measured 1,238,808 open-wave bytes (1.24% of the 100 MB budget) in
  a constant 3 GETs/open, and this run's own real per-cluster byte cost
  extrapolates to the RFC's 1,000,000-vector scale at 12,384,408 bytes — matching
  RFC 0010's hand-computed ≈12.4 MB figure to the byte
  (`bench/results/vector-cold-open.json`; RFC 0010 Discussion). Still open: a
  Range-GET method on `strand-core`'s `ConditionalStore` and a real-network tail-
  latency figure (this benchmark, like `bench/src/cold_open.rs` and `bench/src/
  field_cold_open.rs` before it, fetches the whole segment object at open and runs
  against MinIO on localhost with no injected network latency); the graph-blob
  ordering algorithm (Starling's block shuffling is the literature; pick with
  evidence), entirely
  untouched by RFC 0010, which is 1-bit cluster-family only. **Note redirected
  from R9's ALP/GPU sub-item (2026-08-19, `docs/roadmap.md` X-6):** ALP
  (Afroozeh, Kuffó, Boncz, SIGMOD '24, `references/r9-fastlanes-core-alp-damon-
  license.md`) — the FastLanes-family lossless *floating-point* compression
  codec — has no relevance to postings (R9's original framing) but is a
  plausible future candidate specifically for this blob family's flat-vector
  storage: invariant 7's raw full-precision vectors (float32, `spec/vectors.md`
  §3) are exactly the kind of floating-point data ALP targets, unlike the
  cluster blob's RaBitQ-quantized codes, which are bit-packed integers ALP does
  not apply to either. One real tension worth naming rather than glossing over:
  the flat-vector blob is currently `storage-class: raw-mappable`
  (`spec/vectors.md` §3), chosen precisely for direct mmap without
  decompression (invariant 10) — adopting ALP there would mean reclassifying it
  `chunk-compressed` and paying a decompression step before rerank, a real
  design trade this note does not evaluate. This is a pointer for future
  scoping only: no RFC, no measurement, no adoption decision made here.
- **M1-2 (`docs/roadmap.md`) — FST term-dictionary size at realistic vocabulary
  scale, measured 2026-08-19, not guessed.** RFC 0005's own Open questions item.
  `bench/src/term_dict_size.rs` built the real, production `build_term_dictionary`
  FST over the real, full MS MARCO corpus (8,841,823 passages): **2,669,086
  distinct terms, 19,423,389 bytes (≈18.5 MB FST, 7.277 bytes/term)**. A
  100,476-passage cross-check scale in the same run (136,777 terms, 963,258 bytes,
  7.043 bytes/term) closely reproduces `bench/src/field_end_to_end.rs`'s own
  independent real run at a comparable scale (135,874 terms / 956,446 bytes),
  confirming the harness is wired correctly. Full data:
  `bench/results/term-dict-size.json`; `rfcs/0005-term-dictionary.md` Discussion.
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
  definition. **Both halves are now resolved.** The Lucene half: RFC 0004
  (`rfcs/0004-analyzer-descriptors.md`) grounds `discountOverlaps` byte-exact against
  Lucene 10.5.1 source and maps it to the descriptor's own
  `counts_overlaps_in_length` field. The tantivy half — **resolved 2026-08-19**
  (`references/tantivy-fieldnorm-overlap-accounting.md`, fetched against tantivy tag
  `0.26.1`, the same tag `references/r11a-tantivy-reader-surface-and-lucene-codec-
  spi.md` already pinned): tantivy has **no `discountOverlaps`-equivalent concept at
  all** — a repo-wide `grep -rn "discount"` returns zero matches. Its field-length
  accounting (`IndexingPosition::num_tokens`, `src/postings/postings_writer.rs:97-
  161`, fed to `FieldNormsWriter::record` at `src/indexer/segment_writer.rs:218-220`)
  increments unconditionally for every token the token stream yields, never
  inspecting `token.position` or `token.position_length` to exclude same-position
  (overlapping) tokens — confirmed both by the indexing code path and by tantivy's
  own `position_length` unit tests, which assert only position placement, never a
  reduced token count. Mapped onto the descriptor: **tantivy's native behavior is
  equivalent to `counts_overlaps_in_length = true`**, the opposite of what
  `lucene-parity` scoring requires. This gates M4's tantivy-fork parity work
  concretely now, not just as an open question: the fork must patch
  `PostingsWriter::index_text` (or its `segment_writer.rs` call site) to subtract
  same-position tokens before recording fieldnorm, since no existing tantivy hook
  does this. Inert for STRAND's own v0.1 tokenizer profile, which has no
  synonym-expansion step, so this remains M4 scope, not M1, per RFC 0004's own
  adversarial review. The same track's other named gap — which CJK/Thai/Lao
  `segmentation_dictionary` STRAND recommends as a default (`docs/roadmap.md`
  M1-1) — is now also resolved, license-side and design-side: RFC 0004 Discussion
  — post-approval amendments (2026-08-19) license-audits five live candidates
  (MeCab, Lindera, Jieba/`jieba-rs`, ICU4C via `rust_icu`, ICU4X's `icu_segmenter`)
  and recommends ICU4X's `icu_segmenter` (`WordSegmenter::new_dictionary()` — the
  RFC's own amendment first named this `try_new_dictionary()`; fetching the real
  2.3.0 source at implementation time (M1-6, below) found the infallible
  compiled-data constructor is actually `new_dictionary()`, corrected in the RFC
  and here, dictionary-vs-LSTM distinction unaffected — Unicode-3.0 license,
  determined Apache-2.0-compatible here for the first time in this project,
  `references/icu4x-icu-segmenter-crate.md`) over Lindera's CC-CEDICT-backed
  Chinese path (CC BY-SA 4.0, share-alike, rejected — `references/cc-cedict-and-
  lindera-cc-cedict-license.md`) and over `rust_icu`'s native-C-dependency shape.
  `spec/analyzer-descriptors.md` §5 names the default. **M1-6 (2026-08-19,
  `docs/roadmap.md`) implements it**: `crates/strand-lexical/src/analyzer.rs`
  populates `segmentation_dictionary` for Han-script content via `icu_segmenter`
  2.3.0 (`compiled_data` feature only — `auto`/`lstm` disabled at the Cargo.toml
  level so the LSTM constructors are not even reachable), and
  `conformance/analyzers/icu4x-dictionary-zh-01.json` pins one real,
  non-predicted Simplified Chinese dictionary-segmentation vector.
  `segmentation_dictionary.version` is pinned as the `icu_segmenter`/
  `icu_segmenter_data` crate-semver pair, not a content hash (`spec/
  analyzer-descriptors.md` §5 states the reasoning and leaves a content-hash
  upgrade open). **Chinese bake-off run 2026-08-19** (`docs/roadmap.md` M1-7):
  `bench/src/cjk_segmentation_bakeoff.rs` measured ICU4X's `icu_segmenter 2.3.0`
  dictionary path against `jieba-rs 0.10.3` (MIT,
  `references/jieba-and-jieba-rs-license.md`) over 8 real Chinese Wikipedia
  sentences (fetched live via the MediaWiki API,
  `bench/results/cjk-segmentation-bakeoff.json`). Result: 0.9154 micro-average /
  0.9139 macro-average interior-boundary agreement (an inter-segmenter agreement
  metric, not accuracy against a gold standard — no verifiably fetchable
  gold-segmented Chinese corpus was used). ICU4X measurably under-merges
  compound nouns jieba-rs keeps whole (e.g. `人工智能`, `计算机`,
  `中华人民共和国` each split by ICU4X, kept whole by jieba-rs) — a real,
  moderate accuracy cost for the dependency-shape/license win, not a
  hypothetical one, but not large enough on this sample to overturn the M1-1
  recommendation. Still open, genuinely: Japanese (Lindera+IPADIC) and Thai
  (PyThaiNLP) remain genuinely untested — Thai has no maintained Rust binding at
  all, and Japanese would need a real IPADIC data-license audit this pass did
  not do — and Thai/Lao/Hiragana/Katakana remain unvectored in
  `conformance/analyzers/`, since one Han-script vector does not cover the whole
  default.
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
  one machine, not the full R9 answer. The license half of this gate is separately
  resolved: `cwida/FastLanes` is confirmed MIT (`references/r9-fastlanes-core-alp-
  damon-license.md`), Apache-2.0-compatible.

  **Granularity and ALP/GPU application-fit — resolved/refined 2026-08-19
  (`docs/roadmap.md` X-6; `references/r9-fastlanes-core-alp-damon-license.md` plus
  a live refetch of the ALP and DaMoN '24 papers this session, not the earlier
  vendored excerpt alone).** Two of R9's three named-open sub-items are settled
  below; ARM/non-AVX2 hardware measurement is untouched by this pass and remains
  completely open, exactly as stated above — no ARM hardware is available in this
  environment, and Docker's arm64 emulation was confirmed non-functional here
  ("exec format error," binfmt_misc not registered), so it was not attempted.

  **ALP does not apply to postings/positions at all — this is a redirect, not a
  postings answer.** ALP (`ir.cwi.nl/pub/33334/33334.pdf`, SIGMOD '24, refetched
  live) is a floating-point-specific codec: its mechanism is factor/exponent
  encoding of "decimal-like doubles" plus exception patching for outliers, and its
  entire evaluation is floating-point columnar/scientific/time-series data — zero
  mention anywhere of inverted-index postings, doc-ID compression, or any integer
  workload. STRAND's postings/positions blob stores integers exclusively (doc-ID
  delta-gaps, term frequencies, `spec/postings.md` §2), so treating ALP's own
  evaluation as evidence for a postings-granularity decision would have been
  exactly the class of error `CLAUDE.md` §3 exists to prevent: a float-compression
  technique's published numbers, forced onto an integer question they never
  measured. ALP's one genuine relevance in this project is the flat vector blob's
  raw float32 storage, not postings — redirected to a new note under R1 above
  rather than answered here, since forcing it into R9 would misfile the finding.
  **This closes R9's ALP sub-item for postings: not applicable, no further
  postings-side work needed.**

  **DaMoN '24's GPU warp caveat is real, not fatal, and separately doesn't move
  STRAND's own decision because STRAND targets CPU, not GPU.** Reading past the
  one-sentence caveat already vendored (full PDF fetched live this session,
  `ir.cwi.nl/pub/34260/34260.pdf`, excerpts now vendored verbatim in
  `references/r9-fastlanes-core-alp-damon-license.md`): the paper's own fix is
  *mini-vectors* — splitting the 1024-value FastLanes vector into smaller
  sub-vectors (they measure 256-wide) so each GPU thread holds fewer registers per
  column (32 → 8 at 256-wide, a measured 4× register-pressure reduction, §5.1
  "FLS-GPU-opt," quoted verbatim in the reference file). This is a real,
  implemented, *measured* mitigation, not a dead end, but its win is narrower and
  more hardware-specific than a first read suggests, and the two hardware targets
  diverge: on the Tesla T4, "FLS-GPU performs significantly better than Tile-Based
  for all queries" — the un-optimized kernel already wins outright, before
  mini-vectors are even applied. On the V100, the *un-optimized* `FLS-GPU` loses to
  Tile-Based specifically "on Q3.1 and Q4.1" (two of the four benchmarked SSB
  queries, Q1.1/Q2.1/Q3.1/Q4.1) — and after applying the mini-vector mitigation at
  the smaller 256×4 configuration V100's tighter register budget forces, the
  paper's own textual conclusion is that "register pressure remained problematic
  for the occupancy — only little performance improvement is observed." The paper
  does not state a clean post-optimization win/loss count by query family in
  prose — only a bar chart (Figure 6) that pdftotext cannot reliably quantify —
  so this entry cites the paper's own textual conclusion rather than a number read
  off that chart (an earlier draft of this entry claimed "Tile-Based still wins on
  2 of the 4 benchmarked query families even after the mini-vector optimization,"
  conflating the un-optimized `FLS-GPU`'s stated 2-of-4 loss with the optimized
  `FLS-GPU-opt` case; corrected here rather than left to stand, per `CLAUDE.md` §2).
  The paper's own framing of what remains unresolved, from its introduction: "we
  mitigate this here using mini-vectors — a future work question is how to further
  reduce this granularity with minimal impact on efficiency."

  Two things matter for STRAND specifically. **First**, this DaMoN paper evaluates FastLanes'
  *core* integer encodings (bit-packing, DELTA, RLE, DICT — SSB benchmark columns
  such as `lo_orderdate`) on GPU; ALP has no role in this paper at all, so it does
  not reopen the postings question the ALP finding above just closed — it is a
  separate, integer-relevant result that happens to live in the same reference
  file. **Second, and decisive for scope:** invariant 9 commits STRAND's kernels
  to stable-Rust CPU SIMD (`wide`/`pulp` with runtime dispatch), and no milestone,
  RFC, or roadmap item proposes a GPU decode path for v0.1. The mini-vector/warp
  constraint is real and instructive — independent confirmation that FastLanes'
  native 1024 width is a tuned choice for one hardware target's lane occupancy,
  not a universal optimum, since this same paper had to shrink it for a different
  target — but it answers a question STRAND isn't asking yet. **This closes the
  GPU-decode sub-question as informational: no v0.1 code or spec consequence.**

  **Granularity — refined with a grounded recommendation, not left as a bare open
  question.** R9's original framing ("1024-native... vs. a nested 8×128 scheme")
  already carried a stale figure this pass corrects: RFC 0007 (Approved)
  registers `BitPacker8x` at **256**-value blocks as the shipped default, not 128
  (`spec/postings.md` §3; `docs/roadmap.md` X-6's own correction) — so the real
  nested alternative to weigh is 1024-native vs. a scheme preserving today's 256,
  not 128. More important than the arithmetic fix: **a "nested" scheme that keeps
  fine-grained block-max pruning while decoding at FastLanes' native 1024 width is
  not free, and the DaMoN paper's own mini-vector experience shows why.**
  FastLanes' interleaved bit-packing distributes consecutive logical values
  round-robin across lanes spanning the *whole* vector configured at encode time
  (`references/r9-fastlanes-core-alp-damon-license.md`, the Unified Transposed
  Layout) — there is no free "encode at 1024, decode only 256 of it when a
  block-max skip lands mid-block": a genuinely independent 256-wide (or
  narrower) decode unit has to be *built* at that width at encode time, which is
  exactly what `BitPacker8x` at 256 already does, and exactly what DaMoN's own
  mini-vectors do on the GPU side for an unrelated reason (register pressure, not
  pruning granularity). So the real choice is not "1024-wide with nested
  fine-grained stats" — that combination isn't on offer — it is a straight
  tradeoff between two actual block widths. **(a) 1024-native:** matches
  FastLanes' own measured operating point, but coarsens the doc-ordinal skip
  bound (and any future WAND-style tf/doc-length bound, RFC 0007 Non-goals) by 4×
  versus today — one `block_max` entry now covers 1024 postings instead of 256 —
  meaning a skip hit decodes up to 4× more postings before reaching the target, a
  real partial reversal of the ~7× skip-cost win RFC 0007's own R2 measurement
  already banked. **(b) Keep 256** (or narrower): preserves today's pruning
  precision, but then FastLanes was never actually adopted at the width its own
  paper stakes its auto-vectorizing-portability claim on ("storing data in
  1024-value vectors... decompressed completely independently" is the paper's own
  stated contribution) — whether a 256-wide FastLanes vector still auto-vectorizes
  as well across ISAs is genuinely unmeasured, not assumed here. **Either way,
  invariant 4's raw-statistics principle holds regardless of block width**:
  `block_max` (today, max doc-ordinal; a future RFC's WAND-style bound) is a raw
  per-block statistic whether the block is 256 or 1024 wide, so this is purely a
  decode-efficiency/pruning-precision tradeoff, not an invariant-4 compliance
  question.

  **Recommendation:** keep the current 256-value block-max granularity. Do not
  adopt 1024-native granularity now, and do not build a "nested" scheme — the
  mechanism argument above shows there is no free version of it, and building one
  speculatively would spend implementation effort supporting a codec (FastLanes)
  this project's own R9 measurement already rejected as the default on the only
  hardware tested (74–77% of `BitPacker8x`'s throughput, RFC 0007). Granularity is
  downstream of, and gated by, the codec-adoption question, not independent of
  it: **only if a future RFC actually adopts FastLanes (or another 1024-native
  codec) as the default postings codec** does the 1024-vs-256 tradeoff above
  become live, and that RFC must make the explicit choice named here — coarsen
  pruning to 1024, or shrink the vector to 256 and re-verify the portability claim
  at that narrower width — rather than assume a nested scheme sidesteps it. This
  closes the granularity sub-item as **refined with a recommendation**: no code or
  spec change follows from it today, since 256 is already the shipped default
  (`spec/postings.md` §3) this recommendation confirms rather than changes.
- **R10** — cross-segment scale: should the manifest carry optional per-segment
  summary metadata (term-statistics sketches, centroid summaries, min/max-style
  pruning stats) so a reader can prune segments before opening them, and what does
  target segment size look like when the 100 MB budget pushes segment count up while
  open-amortization pushes it down? Fed by the M3 multi-segment benchmark; any
  summary blob must stay index-internals-agnostic per `CLAUDE.md` §6.
- **R11 — Adapter feasibility audit (gates all adapter work).** Verify against
  current source, not memory: (a) **resolved 2026-08-19** — tantivy's reader
  surface — map the modules a STRAND read path must replace, settling the codec-SPI
  question as a by-product, and confirm the exact Lucene codec SPI class surface for
  `StrandCodec`; (b) **resolved 2026-08-19** — FAISS per-kernel feasibility — whether
  the generic `InvertedLists` path serves `IndexIVFRaBitQ` search over external
  storage, and whether the FastScan path can run over external lists at all given its
  `CodePacker`/`BlockInvertedLists` packing, including the load-time repack cost if
  so; (c) Quickwit split/hotcache internals post-relicense, testing the
  inherits-from-the-fork hypothesis — **resolved 2026-08-19**
  (`references/r11c-quickwit-relicense-and-hotcache-source.md`): Quickwit's license
  is confirmed Apache-2.0 both byte-level (current `LICENSE` file) and commit-level
  (PR #5645, "Relicense to Apache 2.0," 2025-01-23, removed `LICENSE_AGPLv3.0.txt`
  and `LICENSE.md`, added `LICENSE`), fully compatible with this project's
  Apache-2.0-only dependency policy, no exceptions needed. The hypothesis itself
  splits in two: the *mechanism* is confirmed — Quickwit's `quickwit-directories`
  crate (`HotDirectory`, `BundleDirectory`, `CachingDirectory`) implements
  tantivy's public `Directory`/`FileHandle` traits only, with no patch to tantivy
  internals, the identical extension point a STRAND tantivy fork already plans to
  use (`docs/benchmarks.md`'s engine-constant rule) — but the *code* does not
  transfer: Quickwit's split footer and hotcache are its own `postcard`-serialized
  wire format, unrelated to STRAND's own footer/hotcache byte layout
  (`spec/container.md` §4), so a STRAND fork still needs its own `Directory` impl
  written from scratch, the same way Quickwit's was. One complication surfaced for
  the fork RFC: tantivy's canonical repo (`github.com/quickwit-oss/tantivy`) is
  itself Quickwit-maintained, ships an opt-in `quickwit` Cargo feature, and
  Quickwit's own build pins an arbitrary unreleased git commit rather than a
  numbered crates.io release — reinforcing (not creating) the already-planned
  practice of the STRAND fork pinning its own explicit commit rather than assuming
  crates.io and tantivy's git trunk behave identically; (d) the fork reader-module
  list that arms the fork failure triggers (docs/benchmarks.md) — **resolved
  2026-08-19** (`references/r11d-tantivy-fork-reader-module-list-and-failure-
  triggers.md`); (e) the warm-tier graph host choice. Adapter milestones remain
  conditional on R11 (e), still open — (b) and (d) are no longer blockers.

  **R11(d) finding (`references/r11d-tantivy-fork-reader-module-list-and-failure-
  triggers.md`, fetched against tantivy tag `0.26.1`, plus one explicitly labeled
  excursion onto unreleased `main`, 2026-08-19):** a fork splits into two layers.
  File virtualization (which physical component files exist, where their bytes
  live) needs no tantivy-internals patch — a custom `Directory` impl suffices,
  the same extension point R11(c) already confirmed Quickwit uses unmodified. The
  byte layout inside each component does need patching, and the Layer-2
  reader-module list for a lexical-only fork is thirteen files: segment-open
  orchestration (`src/index/segment_reader.rs`, `inverted_index_reader.rs`,
  `segment_component.rs`, `segment.rs`, `src/directory/composite_file.rs`),
  postings decode (`src/postings/compression/mod.rs`, `block_segment_postings.rs`,
  `skip.rs`, `serializer.rs`, `postings.rs`, `segment_postings.rs`), positions
  decode (`src/positions/mod.rs`, `reader.rs`), the term dictionary
  (`src/termdict/mod.rs`, `fst_termdict/mod.rs`, `fst_termdict/termdict.rs`), and
  field norms (`src/fieldnorm/reader.rs`, `code.rs`, needed for the invariant-5
  Lucene-parity profile) — plus one new `StrandDirectory` (additive, Layer 1).
  `src/fastfield/*`, `src/store/*`, and `src/schema/document/*` are named
  out-of-scope, since STRAND has no stored-field or generic-fast-field
  equivalent. Two real mismatches were found while grounding the list, not
  merely a module inventory: tantivy's postings codec bit-packs 128-value
  blocks (`BitPacker4x::BLOCK_LEN`, confirmed by reading
  `src/postings/compression/mod.rs`) against STRAND's registered 256-value
  `BitPacker8x` blocks (RFC 0007, R9's correction) — a real granularity
  mismatch, not a relabeling — and tantivy's default read path already
  computes and uses a BM25 block-pruning bound (block-max-WAND: a
  representative fieldnorm id and max term frequency welded inline into each
  bit-packed block's header, `src/postings/skip.rs`'s `BlockInfo::BitPacked`)
  that STRAND's own postings blob does not register at all yet — RFC 0007 §6
  registers only a doc-ordinal skip bound and names the scoring-aware bound an
  explicit non-goal, "real, separate, future work" — so the fork inherits an
  open cross-project dependency here, not a byte-layout translation, named as
  the parity-gate trigger's most likely ten-session risk. Module churn was
  checked against real git history, not
  assumed: `src/postings/skip.rs` (the file carrying both mismatches) and
  `src/postings/mod.rs` (which re-exports `skip.rs`'s `BlockInfo`/`SkipReader`
  and has its own real algorithmic churn) each took real, non-lint commits
  roughly monthly through 2026, and — the sharpest data point — a 44-file
  upstream PR,
  "Extensible segment components via plugin trait" (#2993, ParadeDB, merged
  2026-08-10, nine days before this grounding), touched every file in the
  segment-open-orchestration group at once. That PR does not overturn R11(a)'s
  finding (its own text: "the read side needs no plugin hook... read back
  through the existing public surface `SegmentReader::open_read`" — no codec
  SPI for *existing* components), but it does date one sentence of it: on
  unreleased `main`, `SegmentComponent` gained an eighth `Custom(String)`
  variant and is no longer the closed seven-variant enum R11(a) describes at
  the pinned tag — recorded here as a fact for a future session to fold back
  into R11(a) itself, not corrected in place, since amending an
  already-approved finding is its own edit. `docs/benchmarks.md`'s
  scope-leak failure trigger ("modifying files outside the pinned
  reader-module list") is now checkable against this list; the other two
  triggers (the ten-session parity gate, the 15%-of-commits maintenance gate)
  are grounded, not restated, by the churn evidence above. This does not
  start M4-4 (the fork itself) — it only unblocks it structurally, alongside
  the already-done M4-1(a).

  **R11(a) finding (`references/r11a-tantivy-reader-surface-and-lucene-codec-spi.md`,
  fetched against tantivy tag `0.26.1` and Lucene tag `releases/lucene/10.5.1`,
  2026-08-19):** tantivy has **no codec SPI**. A repo-wide search of the full
  recursive tree for "codec"/"format" registration concepts turns up nothing but a
  bare version-number constant in the columnar module. `Directory`
  (`src/directory/directory.rs`) is a byte-range storage abstraction (open/read/
  write/delete/lock/watch), not a format-plugin point. `SegmentComponent`
  (`src/index/segment_component.rs`) is a closed seven-variant enum (Postings,
  Positions, FastFields, FieldNorms, Terms, Store, Delete) with exactly one
  concrete reader/writer type wired to each variant at compile time
  (`SegmentReader::open_with_custom_alive_set`, `InvertedIndexReader`); the
  `Postings` trait in `src/postings/postings.rs` is a runtime query-result iterator
  (doc id / term freq / positions), not a wire-format registration point. The one
  real configuration knob — `store::Compressor` (None/Lz4/Zstd) — is a closed,
  compile-time-feature-gated enum for the doc store only, not an open registry. This
  confirms `docs/milestones.md` M4's "tantivy fork" language literally: a
  STRAND-compatible tantivy reader means forking `quickwit-oss/tantivy` and modifying
  its internal reader/writer modules directly, never registering a plugin against a
  stable extension point — there is no such point to register against.

  Lucene, by contrast, has exactly the SPI `StrandCodec` was planned against, current
  as of 10.5.1 (2026-08-12, whose default codec is `Lucene104`): `Codec`
  (`org.apache.lucene.codecs.Codec`) declares eleven abstract format-returning
  methods (`postingsFormat`, `docValuesFormat`, `storedFieldsFormat`,
  `termVectorsFormat`, `fieldInfosFormat`, `segmentInfoFormat`, `normsFormat`,
  `liveDocsFormat`, `compoundFormat`, `pointsFormat`, `knnVectorsFormat`), each
  independently resolved by name through `java.util.ServiceLoader` via a thin
  `NamedSPILoader` wrapper. A `StrandCodec` extends `FilterCodec` (the documented
  delegation base class), overrides `postingsFormat()` to return a
  `PostingsFormat` subclass whose `fieldsProducer(SegmentReadState)` builds a
  `FieldsProducer` over STRAND's own postings/term-dictionary blobs, delegates the
  other ten methods to an existing codec (`Lucene104Codec` today), and registers its
  fully-qualified class name in `META-INF/services/org.apache.lucene.codecs.Codec` —
  the exact mechanism confirmed by reading Lucene's own shipping
  `META-INF/services/org.apache.lucene.codecs.Codec` file, which names
  `org.apache.lucene.codecs.lucene104.Lucene104Codec`.

  **R11(b) finding (`references/r11b-faiss-invertedlists-external-storage-feasibility.md`,
  fetched against FAISS tag `v1.15.0`, 2026-08-19):** both kernels are feasible
  without forking FAISS, and the split is narrower than the open question assumed.
  **Plain `IndexIVFRaBitQ`:** a STRAND-cluster-blob-backed `InvertedLists` subclass
  (deriving from FAISS's own `ReadOnlyInvertedLists`, which already supplies the
  three throwing write-method stubs a read-only backend needs to satisfy the
  interface's pure-virtual contract) fully serves search over external storage — a
  full-file read of `faiss/IndexIVF.cpp`'s `search_preassigned` (the code path
  `IndexIVFRaBitQ` inherits unmodified) found zero `dynamic_cast` to any concrete
  `InvertedLists` subtype anywhere in the file; every list read goes through the
  plain virtual `list_size`/`get_codes`/`get_ids` trio via `ScopedCodes`/`ScopedIds`.
  **FastScan (`IndexIVFRaBitQFastScan`):** the same is true at *search* time — an
  exhaustive grep of `IndexIVFFastScan.cpp` for `dynamic_cast<BlockInvertedLists`
  found exactly two hits, both inside code-*writing* functions
  (`init_code_packer`, `add_with_ids`), never in any `search_implem_*` path; the
  RaBitQ-FastScan specialization's own `postprocess_packed_codes`
  (`IndexIVFRaBitQFastScan.cpp`) adds one more `dynamic_cast<BlockInvertedLists>`,
  also strictly in the write path — so a custom `InvertedLists` whose `get_codes`
  returns bytes already in FAISS's block-packed layout is read correctly by FastScan
  search with no FAISS-side change. What does force a literal `BlockInvertedLists` is
  the *build* path: `add_with_ids` unconditionally throws `"only block inverted lists
  supported"` if `dynamic_cast<BlockInvertedLists*>(invlists)` fails, then writes
  through that class's public `codes`/`ids` members directly, bypassing
  `add_entries` entirely. Since STRAND's wire bytes are never themselves
  block-packed (invariant 10 forbids baking FAISS's register-shuffle layout into the
  spec), every STRAND→FastScan path needs a repack, and FAISS's own conversion
  constructor `IndexIVFRaBitQFastScan(const IndexIVFRaBitQ&, int bbs)` is real,
  quoted, load-bearing evidence for its cost: it reads the source `InvertedLists`
  generically (confirming a STRAND-backed list works as the repack's *source*) and
  does `O(ntotal · d)` bit-level re-derivation plus one `CodePacker::pack_1` call per
  vector to produce a fresh, owned `BlockInvertedLists` — a one-time, whole-segment
  cost best paid once at segment open (matching invariant 3's parallel-wave model),
  not per query. No FAISS fork is required for either kernel; the R11(b) open
  question is resolved in the more favorable direction than
  `docs/benchmarks.md`'s prior hedge assumed.
- **Postings block size** (conditional): this entry previously said "128 was the
  default by shared lineage" — stale, predating RFC 0007's approval; corrected here
  (found during `docs/roadmap.md`'s own adversarial review, 2026-08-19). RFC 0007
  registers **256**-value blocks (`BitPacker8x`, `spec/postings.md` §3,
  `crates/strand-lexical/src/postings.rs`'s `BLOCK_LEN`), the shipped default,
  conditional on R9's granularity outcome — the block-max sibling-blob *pattern*
  stays settled (invariant 4), only the granularity number is open. (128 remains
  the real, separate default for the positions blob family, RFC 0008 — not to be
  conflated with postings' own 256.)
- **~250ms p90 tail figure — resolved 2026-08-19.** The figure was never traced to
  an AWS source and is retracted per `CLAUDE.md` §2 rather than kept. A real, current
  primary source was located instead: the AWS whitepaper "Best Practices Design
  Patterns: Optimizing Amazon S3 Performance" states applications can achieve
  "consistent small object latencies... of roughly 100–200 milliseconds"
  (`references/aws-s3-small-object-latency.md`, vendored 2026-08-19). `CLAUDE.md` §7
  now cites this figure, honestly labeled — the source does not name a percentile, so
  it is not claimed as a p90. This resolves the "vendor the source sentence" half of
  the original pending item. The "or replace with a measured MinIO/S3 tail figure"
  half — **resolved 2026-08-19, roadmap item X-4.** `bench/` still measures a real
  cold-open p50/p90/p99 against plain localhost MinIO (`bench/results/cold-open.json`),
  confirming the GET-count half of invariant 3 (3 GETs/open) with a real baseline
  (p50 = 10.0ms). The real-network half is now measured too, real S3 credentials
  being unavailable in this environment: the same MinIO container with a real `netem`
  delay injected onto its network interface (`bench/src/cold_open_injected_latency.rs`,
  `strand_bench::inject_netem_delay` in `bench/src/lib.rs` — a throwaway `alpine`
  sidecar joining the target's network namespace via `docker run --net
  container:<id> --cap-add NET_ADMIN`, since the MinIO image itself has no package
  manager or `tc` to install one; feasibility was verified directly against a plain
  `alpine` container and separately against a real MinIO container's `curl`-measured
  latency before this benchmark was written). The injection is one-way and egress-only
  (stated honestly, not hidden: it delays only each already-open request's response
  leg, so a 100ms injected delay targets a ~100ms measured round trip per warm GET, not
  a symmetric 100ms each way). Thirty real cold opens against this 100ms-delay MinIO
  measured **p50 = 344.2ms, p90 = 375.3ms, p99 = 489.8ms** (min 326.7ms, max 489.8ms;
  `bench/results/cold-open-injected-latency.json`) — inside RFC 0001's own
  "~300–400ms" napkin-math prediction for this exact pointer/snapshot/segment
  sequence at p50 and p90, and only modestly above it at p99, a real confirmation of
  the napkin-math rule's arithmetic itself, not just of the ~100ms planning figure it
  is built from. It does not confirm the AWS SLO figure's absolute ~100–200ms number
  against real S3 — the 100ms delay was chosen to target that figure, not discovered
  independently — so that figure stands on the AWS source alone, per the paragraph
  above. Full numbers and the mechanism's honest limitations are in `CLAUDE.md` §7.
- **Other pending figures:** the parallel-wave aggregate throughput behind the 100 MB
  budget rationale — **resolved 2026-08-19, roadmap item X-5.** Real N-way parallel
  byte-range GETs against real MinIO (`bench/src/parallel_range_fetch.rs`, using
  `strand-core`'s new `RangeGetStore` trait) measured p50 = 50.6 MB/s for one
  sequential stream fetching a real 100 MB object versus p50 = 159.7 MB/s at 32-way
  parallelism — a real 3.15x aggregate-throughput speedup, confirming the mechanism.
  Full numbers, and what this measurement does *not* confirm (the absolute "low
  hundreds of milliseconds" figure, and non-monotonic scaling past 32-way), are in
  `CLAUDE.md` §7 and `bench/results/parallel-range-fetch.json`; the real-S3-network
  half of *this specific* 100 MB parallel-fetch question remains open — X-4 (below)
  resolved the injected-latency mechanism and applied it to `cold_open.rs`'s
  pointer/snapshot/segment sequence, not to `parallel_range_fetch.rs`'s own 100 MB
  wave, so that absolute figure is not yet re-measured under injected latency; the
  mechanism now exists in `bench/src/lib.rs` (`inject_netem_delay`,
  `with_minio_latency`) for a future session to point at that benchmark too. tantivy
  codec-SPI
  absence — resolved, R11(a) above; FAISS FastScan external-list feasibility —
  resolved, R11(b) above. The Quickwit inheritance hypothesis
  (R11(c)) is no longer pending — resolved above with vendored primary sources. The
  M0 vendoring deliverable is partially
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
- **`strand-vector` implemented — 2026-08-19, prompted by "yes, start
  implementing strand-vector."** All four RFC 0010 blob types' wire format:
  `descriptor.rs` (quantization descriptor, both registered rotator
  types — `MatrixRotator`'s realized-matrix *generation* is out of scope,
  callers supply pre-computed bytes), `navigation.rs` (cluster navigation
  tier), `posting_list.rs` (cluster posting lists, per-cluster code region
  plus row-id array), `flat.rs` (flat vectors), and `fastscan.rs` (the
  FastScan pack/unpack codec — `pack_batch` adopted verbatim from
  `references/rabitq-library-fastscan-pack-codes-source.md`;
  `unpack_batch` is this crate's own derived inverse, proven correct by
  round-trip tests rather than trusted by construction). 21 tests plus a
  2-case proptest suite (`fastscan_round_trip.rs`, hundreds of random
  cases across `cols`/`num`/content), all passing; `cargo clippy
  --workspace --all-targets -- -D warnings` clean. Deliberately out of
  scope, matching RFC 0010's own Design §4 (the actual RaBitQ quantization
  math is a separate concern from this container-layer crate): this crate
  packs and unpacks already-quantized codes and factors, however they were
  produced — no rotation application, no sign-based bit selection, no
  `f_add`/`f_rescale`/`f_error` formulas, no k-means, no query-resolution
  pipeline.

  **Real, independent confirmation of RFC 0010's own worked example**: two
  new golden files (`conformance/vectors/toy-descriptor.bin`, 48 bytes;
  `toy-navigation-tier.bin`, 568 bytes) were generated directly from the
  RFC's own stated hex bytes, and `tests/worked_example.rs` proves this
  crate's real `build_fht_kac_with_payload`/`build_navigation_tier`
  functions — given the RFC's own worked-example inputs — produce
  byte-identical output, not merely bytes that happen to match a
  hand-derivation. A third worked-example test confirms every non-opaque
  byte of the posting-list blob (directory offsets, `code_bytes_length`,
  row-id arrays) against the RFC's own figures, using synthetic code
  content in place of the RFC's own deliberately-opaque payload.

  **The FastScan micro-example from RFC 0010's Discussion amendment is now
  pinned as a real Rust test** (`fastscan::tests::
  matches_rfc_0010_discussion_micro_example`), not just prose — the same
  2-vector synthetic input, independently re-executed in Rust rather than
  only Python, producing byte-identical output.

  **A full end-to-end integration test**
  (`tests/segment_assembly.rs`) assembles all four blob types into a real
  segment via `strand-core`'s actual `SegmentBuilder`, opens it cold (a
  real footer and hotcache decode, matching every prior blob family's own
  end-to-end test pattern — `field_end_to_end.rs`), and simulates a real
  one-wave query resolution: selects both clusters from the already-decoded
  navigation tier, then reads each cluster's region directly from the
  already-fetched posting-list blob bytes with no further round trip,
  confirming invariant 3's one-wave rule holds in code, not just in the
  RFC's own prose.
- **Real RaBitQ 1-bit quantization math implemented — 2026-08-19, same
  day, prompted by "implement the real RaBitQ quantization math next."**
  `crates/strand-vector/src/quantize.rs`'s `quantize_one_bit` is the piece
  RFC 0010 Design §4 explicitly left as "the algorithm's concern, not this
  container-layer RFC's" — the sign-based binary-code rule and the
  `f_add`/`f_rescale`/`f_error` distance-correction factor formulas,
  grounded by fetching the reference implementation's actual
  `one_bit_code_with_factor`/`one_bit_compact_code`/`pack_binary` source
  (`references/rabitq-library-one-bit-quantization-source.md`).
  **Verification went beyond transcription-and-trust**: this session wrote
  a standalone, dependency-free C++ reimplementation of the identical
  fetched formula (plain loops, no Eigen), compiled it with `g++ -O2`, and
  ran it against three real test cases (dim 8 and 16, both `METRIC_L2` and
  `METRIC_IP`) to obtain real reference values — then tested the Rust
  transcription against those executed outputs, not against its own
  derivation. One example, hand-traced independently of both
  implementations as a third check: `data = [1.0, -2.0, 3.5, 0.5, -1.5,
  2.0, -0.25, 4.0]` against `centroid = [0.5, -1.0, 2.0, 1.0, -1.0, 1.5,
  0.0, 3.0]` (dim 8) → residual signs `[1,0,1,0,0,1,0,1]` → packed
  MSB-first → `0xA5`, confirmed by both the compiled C++ and the Rust
  implementation. A new `tests/quantize_to_posting_list.rs` end-to-end test
  quantizes 37 real (synthetic-noise) vectors against a real centroid,
  packs them into an actual posting-list blob spanning two FastScan
  batches, and reads every code and factor back byte-for-byte — proving
  the quantizer and the wire format actually compose. 6 new tests, all
  passing; workspace total now 125, clippy clean.

  Precondition made explicit, not glossed: `quantize_one_bit`'s `data`/
  `centroid` inputs MUST already be rotated (confirmed by the reference
  IVF construction path's own comment, "we first rotate... then compute
  the 1-bit codes") — rotation *application* itself (as opposed to the
  rotation *payload's* storage format, which `descriptor.rs` already
  handles) is not implemented by this work and remains real, separate,
  unwritten work, alongside the query-side distance estimator that
  consumes these factors (FastScan's `accumulate()` plus the formula built
  on top of it), `MatrixRotator`'s matrix generation, k-means clustering,
  and multi-bit Extended-RaBitQ (all unchanged Non-goals from RFC 0010).
- **The `nprobe` cluster-selection pipeline implemented — 2026-08-19, same
  day, prompted by "implement the nprobe cluster-selection pipeline
  next."** `crates/strand-vector/src/query.rs` implements `spec/
  vectors.md` §6 steps 1–3 directly: unlike every other module built this
  session, there is no external reference implementation to fetch or
  match here — this is STRAND's own query-resolution algorithm, already
  fully specified in the approved RFC 0010 Design §6 before any code
  existed. `select_nprobe_clusters` computes the query's distance to every
  centroid already in hand (no I/O) and picks the closest `nprobe`,
  metric-aware (L2: nearest by squared distance; inner product: highest
  true inner product, via the same "smaller estimate is better" convention
  `estimate.rs` already established). `scan_selected_clusters` decodes and
  estimates every candidate across the selected clusters, deduplicating by
  row-id under closure replication and keeping each row-id's best estimate
  — the spec's own literal requirement, tested directly with a synthetic
  two-cluster case sharing a row-id.

  Deletion-vector filtering (spec step 4) and reranking (step 5) are not
  implemented here, named rather than glossed: this family has no wired-up
  connection to invariant 2's general deletion-vector machinery yet (an
  already-named Non-goal), and reranking is a thin wrapper over
  `crate::flat` any caller can already build once it has a surviving
  candidate set.

  **The real property this whole feature exists for, tested directly**:
  recall is monotonically non-decreasing as `nprobe` grows. Across four
  real query points against a real 400-vector, 10-blob clustered index,
  once the true nearest neighbor was found at some `nprobe`, it was never
  lost again at any larger `nprobe` — the actual guarantee the `nprobe`
  knob trades I/O against, not merely "the code runs." A second test
  confirms `nprobe = num_clusters` recovers exactly the same candidate set
  as a manual exhaustive scan (tying back to `build_a_real_index.rs`'s own
  approach), and a third confirms a genuinely bounded `nprobe` (3, against
  6 real clusters) still finds a deliberately-nearest vector when the
  query sits close to its own cluster. 7 new tests, all passing; workspace
  total now 162, clippy clean.

  This closes RFC 0010's Design §6 query-resolution steps 1–3 completely.
  What remains from the original Non-goals list: `MatrixRotator`'s matrix
  *generation*; the multi-bit Extended-RaBitQ path; deletion-vector
  integration; and reranking against the flat-vector blob (steps 4–5) —
  all real, separate, unwritten work, now the complete and accurate list.
- **K-means clustering implemented — 2026-08-19, same day, prompted by
  "implement k-means clustering next."** `crates/strand-vector/src/
  kmeans.rs`'s `kmeans` produces the centroids and cluster assignments
  every earlier module in this crate had to receive pre-made in every
  test until now. **A genuinely different grounding situation from every
  other module built this session**: `descriptor.rs`, `rotate.rs`,
  `quantize.rs`, and `estimate.rs` all had to match the reference
  implementation byte-for-byte or bit-for-bit, because their output is
  wire-visible. Clustering has no such constraint — the reference library
  itself ships no C++ k-means at all; its own construction-side tooling
  (`python/ivf.py`) delegates to Faiss, a separate library this project
  has no reason to vendor or byte-match, and RFC 0010 Design §3 already
  named the clustering algorithm as construction-side and wire-format-
  irrelevant. What was owed instead: mathematical correctness against the
  well-known standard algorithm — Lloyd's algorithm with k-means++ seeding
  (Arthur & Vassilvitskii, SODA 2007) — verified by testing the properties
  a correct implementation must have (inertia never increases as more
  iterations run from the same seed; every returned cluster is non-empty,
  including under a real empty-cluster-recovery test that deliberately
  requests more clusters than natural groups exist; deterministic given a
  seed; well-separated synthetic blobs are recovered exactly), not by
  cross-checking against a compiled reference — the first module this
  session where that verification style was the right one, not a
  compromise.

  **The capstone test for the crate so far**
  (`tests/build_a_real_index.rs`): 200 raw vectors, clustered from
  nothing (no pre-given centroid, for the first time), rotated, quantized,
  assembled into a real four-blob-type `strand-core` segment, opened cold,
  and queried across every real cluster — correctly ranking a
  deliberately-nearest vector first, confirmed against brute-force ground
  truth over the original raw vectors. 8 new tests, all passing; workspace
  total now 155, clippy clean.

  What remains from RFC 0010's original Non-goals list, narrower again:
  `MatrixRotator`'s matrix *generation* (its application is implemented);
  the multi-bit Extended-RaBitQ path; and the `nprobe`-bounded cluster-
  selection step itself (this session's own capstone test scans *every*
  cluster for its own exhaustive correctness check, rather than selecting
  the `nprobe` nearest by centroid distance first — a real, deliberately
  named simplification, not yet the bounded, parallel-wave query RFC 0010
  Design §6 actually specifies).
- **Query-side distance estimator implemented — 2026-08-19, same day,
  prompted by "implement the query-side distance estimator next."**
  `crates/strand-vector/src/estimate.rs`'s `estimate_distance` closes the
  last Non-goal RFC 0010 named specifically as "the algorithm's concern":
  given a rotated query, a rotated centroid, and one database vector's
  stored factors, it estimates the true distance with a two-sided error
  bound, using the reference implementation's own formally-derived
  estimator (`docs/docs/rabitq/estimator.md`) and its real query-factor
  computation (`include/rabitqlib/index/query.hpp`), both vendored at
  `references/rabitq-library-estimator-source.md`.

  **A real ambiguity in the math notation was resolved by reading code,
  not by picking an interpretation.** The formal derivation writes the
  query term as `q_r' = P^{-1} q_r` — a reverse-rotated query — which read
  in isolation looks like it demands a second, inverse-rotation pipeline
  distinct from the forward `rotate_fht_kac` this crate already built.
  Reading `query.hpp`'s actual code settled it in the other direction:
  every query-side class takes a parameter literally named
  `rotated_query` and uses it directly, with no inverse rotation anywhere
  — the identical forward transform already applied to database vectors
  and centroids at index-build time is applied to the query too. Because
  that rotation is orthogonal, `<x_u, P^{-1}q_r> = <P x_u, q_r>`, so
  building the lookup table from the forward-rotated query and dotting it
  against the unrotated code bits computes the same quantity the notation
  describes, by a cheaper route. Getting this wrong would have meant
  building and grounding an entirely separate inverse-rotation pipeline
  that the real implementation doesn't have.

  **Verified beyond transcription, a third time this session**: a
  standalone C++ reimplementation of the full encode-then-estimate
  pipeline (reusing the already-verified `quantize.rs` formula) was
  compiled and run for a real dim=64 case, both metrics. The real,
  load-bearing check wasn't value-matching alone — it was that **the true
  distance fell inside `[lb, ub]`** for both metrics, the actual
  theoretical guarantee this estimator exists to provide. A large,
  fixed-seed statistical test (2,000 random trials, not a hand-picked
  case) then confirmed the same containment property holds for 96.3%
  (L2) and 96.25% (IP) of random cases — deliberately checked
  statistically rather than as a `proptest` property, since RaBitQ's own
  documentation states its confidence constant gives "nearly perfect
  confidence," not literal 100%, and asserting 100% containment across a
  proptest run's many trials would produce real, expected occasional
  failures that are not implementation bugs.

  **The first genuinely full end-to-end test** (`tests/
  query_a_real_cluster.rs`): a real cluster of 50 quantized vectors is
  written into a real posting-list blob; a real query is rotated once and
  scanned against every candidate in the already-fetched blob bytes (no
  further I/O, matching RFC 0010 Design §6's one-wave query resolution);
  the estimator correctly ranks a deliberately-nearest vector first,
  confirmed against brute-force ground truth. 8 new tests, all passing;
  workspace total now 147, clippy clean.

  This closes every Non-goal RFC 0010's own Design §4 named as "the
  algorithm's concern, not this container-layer RFC's" — the sign-based
  binary code, the encode-side factors, rotation application, and now the
  query-side estimator that consumes those factors. What remains from
  RFC 0010's original Non-goals list: `MatrixRotator`'s matrix
  *generation* (its application is implemented), real k-means clustering,
  the multi-bit Extended-RaBitQ path, and the actual `nprobe` cluster-
  selection/scan-orchestration pipeline that would wire this estimator up
  to a real multi-cluster query end to end (Design §6 steps 1–2, as
  opposed to the single-cluster scan this session's own test already
  exercises).
- **Rotation application implemented — 2026-08-19, same day, prompted by
  "implement rotation application next."** `crates/strand-vector/src/
  rotate.rs`'s `rotate_fht_kac` and `rotate_matrix` close the precondition
  every earlier module in this crate stated but didn't implement:
  `quantize_one_bit`'s "already rotated" requirement. `rotate_fht_kac` (the
  registered default) is a genuinely complex piece — the reference
  implementation's `FhtKacRotator::rotate()`, a 4-stage pipeline of sign
  flips, a Fast Walsh-Hadamard Transform, and (in the general case) a
  Kac's-walk mixing butterfly — grounded by fetching the actual `rotate()`
  method plus its three primitives (`fht_avx.hpp`'s `helper_float_1`/
  `helper_float_2`, confirmed to be the standard, textbook in-place FWHT
  butterfly network, generalized to any power-of-two size per invariant 9
  rather than replicating the library's later hand-vectorized AVX
  variants; and `flip_sign`/`kacs_walk`, whose only available source is
  AVX2 intrinsics — no portable scalar fallback ships in the library — so
  their scalar semantics were read out of the vector code rather than
  transcribed from an existing scalar reference, including confirming
  `flip_sign`'s bit order is LSB-first, the *opposite* convention from
  `quantize.rs`'s `pack_binary`). All vendored at
  `references/rabitq-library-rotation-application-source.md`.

  **Verified beyond transcription, again**: a standalone C++
  reimplementation of the full two-branch pipeline was compiled and run
  against three cases — a general-branch case (`dim=100, padded_dim=128`),
  the degenerate branch where `padded_dim` is itself a power of two
  (`dim=padded_dim=64`), and the realistic embedding case (`dim=padded_dim
  =768` — not a power of two, so this is actually the general branch for
  most real embedding widths, not the simple one). The `dim=768` case
  surfaced a real, independent mathematical check no value-matching alone
  provides: a true rotation preserves L2 norm, and the compiled reference's
  own input/output sums of squares matched to four decimal places
  (`1549.8966` vs `1549.8970`) — real, measured evidence the transcription
  is a genuine orthogonal transform, not just "produces the same numbers
  as another buggy implementation." A `proptest` property test then
  confirmed norm preservation across hundreds of random inputs spanning
  both branches, not just the three hand-picked cases. A real transcription
  bug was caught and fixed during this work, before it reached a commit: a
  test helper's flip-byte generator used the wrong multiplier for one of
  three cases (copy-paste from an adjacent test), which would have silently
  validated the Rust output against the wrong C++ reference values —
  caught by checking the helper's formula against each test's own printed
  C++ generator line, not merely re-running the test.

  A new `tests/full_pipeline.rs` connects every piece this crate has
  grounded so far — descriptor, rotation, quantization, and the
  posting-list wire format — into one real chain: raw, unrotated vectors
  in, real posting-list blob bytes out, read back bit-exact, plus a
  sanity check that rotation approximately preserves residual distance
  between a vector and its centroid (the property RaBitQ's whole
  quantization scheme depends on). 7 new tests, all passing; workspace
  total now 141, clippy clean.

  Deliberately unimplemented, named rather than glossed: the query-side
  distance estimator (FastScan's `accumulate()` plus the formula built on
  top of it), `MatrixRotator`'s realized-matrix *generation* (QR
  decomposition — `rotate_matrix`'s own *application* of an
  already-supplied matrix is implemented and tested), k-means clustering,
  and multi-bit Extended-RaBitQ remain real, separate, unwritten work —
  the same Non-goals RFC 0010 named at Approval, now narrower by exactly
  one item.
- **`quantize.rs` adversarially reviewed and fixed — 2026-08-19, same day,
  prompted by "does it needs and ACPR?" then "go."** Not the RFC-style
  ACPR gate (this module transcribes an external, already-published
  algorithm rather than making a STRAND design decision), but a real
  review was warranted for numerically load-bearing code, and one was run.
  Found **2 Critical, 7 Important, 5 Minor**, all fixed or explicitly
  deferred with reasoning recorded. The reviewer's own claims were
  independently re-verified, not trusted: the Critical finding's central
  claim (an f32-rounding-induced negative `sqrt` argument) was checked
  with a real NumPy `float32` trace before any fix was written, confirming
  `bracket = -1.703e-8` for `data=[0.1;8]`, `centroid=[0.0;8]` — real,
  reproducible, not a false positive.

  **Critical (2):** (1) the pre-`sqrt` bracket in `tmp_error`'s formula is
  `>= 0` by Cauchy-Schwarz *mathematically*, with equality exactly when
  every `|residual_i|` is equal — a reachable boundary, not an asymptote —
  and f32 rounding pushes it a few ulps negative there, producing `NaN`
  that would have silently poisoned real posting-list blobs. Fixed with a
  documented `.max(0.0)` clamp, proven a no-op everywhere the unclamped
  reference value would have been correct (Cauchy-Schwarz), so it cannot
  change a value the reference would have gotten right. (2) The
  zero-residual case (`data == centroid` exactly — real and expected for
  any singleton k-means cluster, not exotic) previously panicked; traced
  the reference implementation's own `+inf` substitution all the way
  through the formula by hand and confirmed two of the three resulting
  factors are already correct as-is (`f_add = 0.0`/`1.0`, `f_rescale =
  -0.0`, both metrics) and the third (`f_error`) was only `NaN` because of
  finding (1)'s same root cause — so fixing (1) fixes (2) for free, and the
  panic was simply removed (`assert!` and its test deleted). Both findings
  cross-checked against a second, independently compiled-and-run C++
  program (this time with both fixes applied identically), not just
  re-derived by hand.

  **Important (7), summarized:** a real finiteness guard now lives at
  `build_posting_lists`'s own write boundary (`posting_list.rs`), not just
  inside the quantizer, so a future caller bypassing `quantize_one_bit`
  can't silently write `NaN`/`inf` factors either; the module doc's
  "cross-checked... independently" claim was overstated — corrected to
  state precisely that the cross-check validates the *formula and
  operation grouping*, not RaBitQ-Library's own Eigen-reduction numeric
  output, which differs in the last few ulps due to summation-order
  differences invariant 9 already anticipates (scalar-normative, not
  bit-identical-to-every-other-implementation); `f_rescale`'s `-0.0` sign
  bit is now pinned as deterministic (invariant 11) with a
  `to_bits()`-comparing test, not left to `==`, which cannot distinguish
  `-0.0` from `0.0`; `padded_dims % 64 == 0` (the registered FastScan
  codec's own real requirement, not just a STRAND alignment convenience)
  is now enforced at `build_posting_lists` — the actual wire-format
  boundary — rather than nowhere; shape-precondition panics were kept
  (consistent with every sibling module in this crate, and `dim` is always
  writer-controlled) while value-domain failures (non-finite input) were
  fixed to produce correct output instead of requiring a `Result` type;
  test coverage gained a `proptest` suite (this crate's first use of the
  dev-dependency it already had) that reproduces the Critical finding
  immediately rather than requiring a hand-picked case, plus explicit
  edge-case tests (underflow, all-ones code, a large-negative-dot-product
  `METRIC_IP` case); a real conformance test against RaBitQ-Library's own
  compiled binary output remains genuinely open — named in the module doc
  as owed work, not attempted this session (the one Important finding
  deferred rather than fixed, since it requires building the full
  reference library with its Eigen dependency, a real undertaking of its
  own).

  **Minor (5):** doc-comment inaccuracies fixed (`dim/8` not `ceil(dim/8)`
  since `dim` is already asserted a multiple of 8; the `cb = -0.5`
  constant's general `bit_width`-dependent form now noted); a non-finite-
  input guard added with a clear message naming the actual culprit;
  `pack_binary`'s length check upgraded from `debug_assert!` to a real
  `assert!`; `spec/vectors.md` §4 gained an explicit "factor computation is
  non-normative at this layer" paragraph naming `quantize.rs` as the
  de-facto normative scalar reference (invariant 9), closing a real gap —
  before this, a wire-visible value had no spec chapter and no stated
  cross-writer byte-identity guarantee; a batch-shaped `quantize_batch` API
  (avoiding four small heap allocations per vector at index-build scale)
  remains a real, deferred efficiency improvement, not implemented this
  session. Workspace total now 134 tests, clippy clean.
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
  retraining at merge time is named as a real, load-bearing, unresolved gap
  rather than assumed away. `CLAUDE.md` §7's own "roughly one segment per
  million 768d vectors" line is updated in place to ~760,000, the direct
  consequence of the corrected sizing law.
  **Intra-batch bit/lane order resolved — 2026-08-19, same day, prompted by
  "start with the FastScan grounding fetch."** The one real gap the review
  left open at Approval — the FastScan code region's byte *offsets* were
  grounded but the bit-level layout within them was adopted by reference,
  unverified — is closed: `fastscan::pack_codes`'s full algorithm (`kPerm0`,
  the nibble-split-and-interleave logic) is fetched and vendored
  (`references/rabitq-library-fastscan-pack-codes-source.md`), confirmed as
  the genuine 1-bit path via its call site in `one_bit_batch_code`, and
  **independently re-executed** against a synthetic 2-vector input (not
  merely transcribed) — a Python port produced 256 real bytes at
  `padded_dim = 64`, hand-verified column by column against the algorithm's
  own definition. Two findings fell out for free: `one_bit_batch_code`'s
  own `padded_dim % 64 == 0` requirement confirms RFC 0010 Design §2's
  `MatrixRotator` padding rule is load-bearing for the codec itself, not
  merely a STRAND alignment convenience; and `pack_codes`'s own
  `get_column` helper zero-fills absent batch slots, confirming RFC 0010's
  zero-fill padding-determinism rule matches the reference implementation's
  actual behavior exactly, not a STRAND-invented compatible convention.
  `spec/vectors.md` §4 now states the complete algorithm normatively.
  **`kBatchSize = 32` hardware-vs-algorithm question resolved — same
  day, 2026-08-19.** The SIMD `accumulate()` decode kernels
  (`src/simd/fastscan_avx2.cpp`/`fastscan_avx512.cpp` — not
  `include/rabitqlib/simd/`, which holds only dispatch declarations) are
  fetched and vendored
  (`references/rabitq-library-fastscan-accumulate-source.md`). Finding:
  `accumulate_avx2` and `accumulate_avx512`, selected by one runtime
  function pointer of a single shared signature, both produce exactly 32
  result values per call despite AVX512's accumulator being twice AVX2's
  register width — a register-width-driven batch size would predict
  AVX512 naturally batching 64 vectors, and it does not. The batch size
  traces instead to `pack_lut`'s fixed 16-entry table (`2^4`, the sub-code
  width) doubled by `pack_codes`'s hi/lo nibble packing, a FastScan/PQ
  lookup-trick shape attributed in the source to Faiss's own FastScan
  design and rooted in SSSE3's 128-bit `pshufb`, predating AVX2 and AVX512.
  `kBatchSize = 32` is algorithm-shaped, not hardware-shaped — resolved,
  not merely evidenced. RFC 0010's "How this could be wrong" and Open
  questions sections, and this ledger entry, are updated accordingly; no
  wire format, arithmetic, or shipped code changed as a result (`kBatchSize
  = 32` in `crates/strand-vector/src/fastscan.rs` was already correct).
  **M2-1 (SPANN-style closure replication's metadata slot and construction
  algorithm) resolved — 2026-08-19, `docs/roadmap.md`.** The one Non-goal
  this RFC's own text had already flagged as "the most consequential
  Non-goal in this RFC, not the least" is closed. Grounded live against
  SPANN's own §3.2.2 closure-assignment criterion (Eq. 2, fetched via
  `ar5iv`'s HTML rendering since the raw-PDF extraction already vendored
  for this paper mangles subscripted equations) and independently
  cross-checked against the paper's own reference implementation,
  `microsoft/SPTAG` (`ReplicaCount` defaults to `8`, matching the paper's
  own stated closure-replica cap exactly) — both vendored in the new
  `references/spann-closure-assignment-algorithm.md`. New module
  `crates/strand-vector/src/closure.rs` (`closure_replicate`,
  `group_by_cluster`, `ClosureConfig`) implements the real algorithm:
  Eq. 2's distance-ratio test (squared-Euclidean, matching this crate's own
  established L2 convention — a real, stated interpretation choice since
  the paper doesn't disambiguate), the paper's RNG-rule redundancy pruning
  (given only partial corroboration from SPTAG — a real, named, bounded-
  consequence gap, since the rule can only ever reduce the realized
  replication count, never the format's worst-case byte-cost bound), and
  the replica cap. The metadata slot is a new, always-present, fixed
  8-byte `replication_descriptor` trailer appended after `cluster_dir` in
  the cluster navigation tier blob (`crates/strand-vector/src/
  navigation.rs`'s new `ReplicationDescriptor`/`ReplicationPolicy`,
  `spec/vectors.md` §3) — deliberately *not* a reuse of that blob's
  existing reserved bytes, whose own normative text already promises
  readers "MUST NOT interpret" them, and deliberately *without* a
  redundant realized-replication-factor field, since the flat-vector
  blob's dense-per-row-id contract means a reader already has everything
  needed to compute that statistic from data already resident after the
  cold-open wave. Both were real near-misses an earlier draft of this
  amendment caught and fixed before implementation, named in RFC 0010's
  own Discussion entry. Every pre-existing call site of
  `build_navigation_tier` needed no changes (a new
  `build_navigation_tier_with_replication` function carries the real
  policy; the old function now simply calls it with
  `ReplicationDescriptor::none()`), so this is a purely additive change:
  `conformance/vectors/toy-navigation-tier.bin` grew from 568 to 576 bytes
  (regenerated), and RFC 0010's Napkin math figures moved by the same fixed
  `+16` bytes (the new trailer plus a pre-existing 8-byte header omission
  in one particular sum, closed in the same edit) — immaterial at any real
  scale. Real tests, including a hand-checked worked example, in
  `crates/strand-vector/tests/closure_replication_end_to_end.rs`: exact
  byte-layout assertions for a small hand-placed-centroid case, and a
  real-k-means, real-quantization end-to-end query proving a deliberately
  boundary-placed vector is found via either of its two clusters and
  deduplicated to exactly one candidate by query resolution's existing
  dedup step (RFC 0010 Design §6 step 3, unchanged). Still open, named
  precisely rather than folded into this resolution: compaction-time
  re-replication after a `rebalance` merge, and cross-segment codebook
  sharing (`docs/roadmap.md` M2-8, unchanged).
  **M2-8: a cross-segment codebook-identity mechanism and cheap pre-merge
  compatibility check — resolved 2026-08-19 (`docs/roadmap.md`).** RFC
  0010's own "How this could be wrong" item 4 and Open questions named a
  real gap: `concatenate + remap` is only valid when two segments'
  quantization descriptors are byte-identical (Design §7), but nothing let
  a reader or merge planner check that cheaply before attempting a merge.
  `crates/strand-vector/src/codebook.rs` closes it with no wire-format
  change: `CodebookIdentity` is a computed (not serialized) summary of a
  resident descriptor — the four scalar fields (`dims`, `distance_metric`,
  `bit_width`, `rotator_type`) plus an XxHash3-64 content hash (invariant
  11's own registered default checksum algorithm, reused rather than
  adding a second one, invariant 8) over exactly the fields Design §7
  names as the byte-identity criterion, deliberately excluding the
  reserved byte (which `spec/vectors.md` §2 already forbids readers from
  interpreting) and `padded_dims` (fully derived from `dims`). Building
  one identity is `O(n)` in `rotation_payload`'s length — no new I/O
  beyond what a reader already fetches to use the codebook at all;
  comparing two built identities (`check_compatibility`) is `O(1)`,
  touching neither segment's payload again. A considered-and-rejected
  alternative, argued in the RFC's own Discussion amendment: a
  construction-time generation/version counter, rejected because it only
  proves temporal common ancestry, not the byte-identity Design §7
  actually requires. Real tests build three pairs of genuinely independent,
  footer/hotcache-decodable segments via `strand-core`'s actual
  `SegmentBuilder` — one pair sharing a real codebook (`Compatible`), one
  pair with independently-trained real codebooks sharing every scalar
  field but a different RNG-drawn rotation (`Incompatible(ContentHash)`,
  the case a bare scalar check would miss), and one pair with different
  `dims` (`Incompatible(Dims)`, the cheap short-circuit case) — and confirm
  the check distinguishes all three
  (`crates/strand-vector/tests/codebook_compatibility_across_segments.rs`),
  plus unit tests for every `CodebookMismatch` variant in isolation. Full
  adversarial review — hash-collision risk (bounded, and the same
  non-cryptographic-hash risk tolerance invariant 11 already accepts for
  chunk checksums), whether the check is cheap enough to run before every
  merge (yes, argued quantitatively), and the nearest-grave framing — lives
  in RFC 0010's Discussion — post-approval amendments. Deliberately
  narrow, matching this project's discipline: the codebook-sharing
  *policy* question, cluster-assignment compatibility with a rebalanced
  navigation tier, and the actual merge-planner code that would call this
  function are all named precisely as still M3-1's, not claimed resolved
  here. `cargo test --workspace` and `cargo clippy --workspace
  --all-targets -- -D warnings` both clean.
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
- **`MatrixRotator`'s matrix generation implemented — 2026-08-19, same day,
  prompted by "implement MatrixRotator's matrix generation next."**
  `crates/strand-vector/src/orthogonal.rs`'s `generate_matrix_rotation`
  closes the last item `rotate.rs`'s own entry above named as remaining:
  `rotate_matrix` already applied a caller-supplied matrix, but nothing in
  this crate produced one. The reference implementation's algorithm
  (already vendored at `references/rabitq-library-rotator-source.md` during
  RFC 0010 drafting, so no new fetch was needed) is: sample a random
  Gaussian `padded_dims × padded_dims` matrix, QR-decompose it, transpose
  `Q`, keep the first `dims` rows — "the random matrix only need the first
  dim rows, since we just pad zeros for the vector to be rotated to padded
  dimension." `sample_standard_normal` draws via Box-Muller (no new
  dependency for one call site); `householder_qr` is a from-scratch
  Householder QR implementation (Golub & Van Loan's numerically-stable sign
  convention: `alpha = -x[0].signum() * ||x||`), computed internally in f64
  and truncated to f32 only for the final wire-format payload.

  **A genuinely different grounding situation from every other
  numerically-precise module this session, reasoned through explicitly
  rather than defaulted into.** `quantize.rs`, `rotate.rs`, and
  `estimate.rs` all had one correct output to match, verified by
  cross-checking against a compiled reimplementation of the reference's
  exact algorithm. QR decomposition has no such single correct output: it
  is not unique — two correct implementations routinely disagree on the
  sign of individual columns — so matching Eigen's `HouseholderQR` output
  bit-for-bit isn't even the right correctness criterion, and would in fact
  be a *fourth* independent implementation's opinion, no more authoritative
  than this crate's own. `descriptor.rs`'s design already anticipated this:
  any `MatrixRotator` implementation need only produce *a* valid orthogonal
  matrix, serialized verbatim once realized, not any *particular* one. What
  was owed instead: verifying the three properties that *define* a valid
  QR decomposition, directly — `Q` orthogonal (`QᵀQ = I`), `R` upper
  triangular, `QR` reconstructs the input — checked at sizes `[1, 2, 3, 5,
  8, 32, 64]` in the default suite and, separately, at `n = 768` (realistic
  embedding scale) in a test marked `#[ignore]` with a documented rationale
  (O(n³) is ~40s in debug mode, ~3s release) and run explicitly for this
  work — a stronger, more directly relevant check here than bit-matching
  any single other implementation would have been.

  A new `tests/matrix_rotator_pipeline.rs` is the `MatrixRotator`
  counterpart to `full_pipeline.rs`: a *freshly generated* (not
  caller-supplied) matrix carried through `descriptor::
  build_matrix_generated` (new convenience wrapper, matching
  `build_fht_kac`'s pattern) → descriptor serialization → parse-back →
  `rotate_matrix` (with an L2-norm-preservation check, the same property
  `rotate.rs`'s own tests already verify for `FhtKacRotator`) →
  `quantize_one_bit` over 30 real vectors → `build_posting_lists` →
  `PostingListReader::read_cluster`, confirming bit-exact round-trip of
  every code, factor, and row-id. 8 new tests (`orthogonal.rs`'s 5
  non-ignored plus 1 ignored, `descriptor.rs`'s
  `matrix_generated_produces_a_real_orthogonal_matrix`, and this pipeline
  test), all passing; workspace total now 169, clippy clean, plus the
  ignored `n = 768` test independently confirmed passing in release mode
  (3.41s).

  This closes every item RFC 0010's Non-goals list named as belonging to
  `MatrixRotator` specifically. What remains from the original list: the
  multi-bit Extended-RaBitQ path, deletion-vector integration, and
  reranking against the flat-vector blob (Design §6 steps 4–5) — unchanged
  from the prior entry, narrower by exactly this one item.
- **Multi-bit Extended-RaBitQ (RaBitQ+) registered and implemented —
  2026-08-19, same day, prompted by "implement multi-bit Extended-RaBitQ
  next."** Unlike every other module built this session, this genuinely
  needed a follow-on RFC before any code: `spec/vectors.md`'s own opening
  line and RFC 0010's Non-goals explicitly required "its own follow-on RFC
  before any `bit_width` other than `1` is valid," since
  `docs/data-structures.md` already commits the multi-bit path to a
  *different* kernel (classical scalar-quantization distance, not FastScan
  LUT) — a real wire-format design decision, not a container-layer
  extension. **RFC 0011** (`rfcs/0011-multibit-extended-rabitq.md`)
  registers it: `bit_width` widens from a fixed `1` to `1..=8`, and a new
  ex-code region (`spec/vectors.md` §4.1) is appended inside the existing
  cluster posting-list blob — no new blob type, since the region is always
  fetched in the same Range GET as its cluster's 1-bit region (invariant 3
  unaffected).

  **A new primary source was fetched and vendored**
  (`references/rabitq-library-multibit-quantization-source.md`:
  `rabitq_impl.hpp`'s `ex_bits` namespace, `estimator.hpp`, `query.hpp`,
  `pack_excode.hpp`, `pack_excode_dispatch.hpp`) — grounding the
  encode-side `best_rescale_factor`/`ex_bits_code_with_factor` algorithm
  and the query-side `split_distance_boosting` formula, and confirming a
  genuinely surprising, real finding: the reference encoder computes an
  `f_error_ex` factor that its own query path *never reads* — the boosted
  estimate reuses the 1-bit region's already-stored `f_error`, scaled by
  `1 / 2^ex_bits`, instead. `spec/vectors.md` §4.1 registers only the two
  factors actually read (`f_add_ex`, `f_rescale_ex`), matching
  `ExDataMap<T>::data_bytes()`'s own real byte budget.

  **The RFC's own adversarial review found a real Critical bug before any
  code existed**: the reference's `ex_bits_code` normalizes the residual
  before quantizing (`abs_res = abs(residual / ||residual||)`), and a
  zero residual (`data == centroid` exactly — real for any singleton
  k-means cluster, the identical scenario `quantize.rs`'s own ACPR found
  Critical for the 1-bit path) makes this a `0.0 / 0.0` division,
  producing `NaN` in every dimension *before* the reference's own
  `ip_resi_xucb == 0 → infinity` guard ever runs (`NaN == 0.0` is false,
  so the guard is silently bypassed). Fixed, in both the RFC and
  `quantize_ex.rs`, with an explicit pre-normalization zero-norm guard
  producing `ex_code = [0; dim]`, `f_add_ex = 0.0`, `f_rescale_ex = -0.0`
  — bit-for-bit the same degenerate values the 1-bit path's own existing
  fix already established, and provably correct here too (substituting
  them into the boosted formula gives `ex_dist = G_add` exactly, the true
  answer when the database vector *is* the centroid). The review also
  found 4 Important gaps (concrete `QueryFactors`/`posting_list.rs`/
  `query.rs` signatures the first draft left too abstract to implement
  against directly) and 2 Minor findings, all fixed — see RFC 0011's own
  Status line for the full itemization.

  **A genuinely different byte-determinism situation from every other
  module this session, reasoned through in the RFC itself.**
  `best_rescale_factor` is an event-driven greedy numerical search, not a
  closed-form formula — two independent, conforming implementations may
  legitimately converge on different, equally valid ex-codes for the same
  input, because floating-point tie-breaking order isn't pinned. `spec/
  vectors.md` §4.1 extends the existing factor-only non-normativity
  carve-out to cover ex-code *values* too, justified because the format's
  error-bound guarantee is self-consistent for whichever valid
  quantization a writer's search actually produces — the same reasoning
  that already let `kmeans.rs`'s clustering go unstandardized. A second,
  real design decision: the reference's own ex-code packing
  (`packing_2bit_excode` through `packing_7bit_excode`) has no portable
  scalar source anywhere in the fetched repository, only AVX2/AVX512
  intrinsics — adopting it verbatim would have repeated the Optane-era
  formats' mistake (`docs/lineage.md`, baking a vendor's SIMD
  register-shuffle pattern into wire bytes) for a kernel STRAND's own
  design already routes through a scalar path. `spec/vectors.md` §4.1
  instead registers a plain, bit-contiguous, MSB-first-per-byte packing —
  STRAND's own convention, matching `quantize.rs`'s `pack_binary`.

  **Implementation**: `crates/strand-vector/src/quantize_ex.rs` (new)
  transcribes the encode algorithm, cross-checked against the same
  standalone-compiled-C++-reimplementation discipline every other
  RaBitQ-specific module this session used — real executed reference
  values for dim=8/ex_bits=2 (both metrics) and dim=16/ex_bits=3, all
  matching to 9 significant figures. `posting_list.rs` gained the ex-code
  region's pack/unpack (`ExRegionInput`, `ex_entry_len`,
  `code_bytes_length_for`/`read_cluster` widened with an `ex_bits`
  parameter). `estimate.rs` gained `estimate_distance_boosted` and
  widened `QueryFactors::new` with a required `bit_width` parameter
  (`g_kbx_sumq`, alongside the existing `g_k1x_sumq`). `descriptor.rs`'s
  `encode`/`build_*` functions gained a `bit_width` parameter with a
  `1..=8` range assertion, replacing the old fixed constant. `query.rs`'s
  `scan_selected_clusters` — the actual integration point, named
  explicitly in the RFC rather than left to spec prose — gained an
  `ex_bits` parameter and now calls the boosted estimator whenever
  `ex_bits > 0`, per `spec/vectors.md` §6 step 3's new normative
  requirement (a reader MUST use the boosted estimate when available, not
  silently fall back to the cheaper 1-bit-only one).

  **The real property this feature exists for, tested directly**
  (`tests/multibit_query.rs`): a real 50-vector cluster is quantized with
  both the 1-bit code and a `bit_width = 4` (`ex_bits = 3`) ex-code
  region, written into a real posting-list blob, and queried through
  `scan_selected_clusters` (the actual crate integration point, not a
  hand-rolled loop). The deliberately-nearest vector ranks first, and —
  the load-bearing check, not just "it ranks correctly" — the boosted
  estimate's mean-squared error against the true (unquantized) distance
  is measurably smaller than the 1-bit-only estimate's mean-squared error
  over the same 50 vectors, confirming the extra bytes bought real
  accuracy, not just extra wire weight. A second test confirms
  `read_cluster` rejects a wrong `ex_bits` as a length mismatch rather
  than silently misparsing. 8 new tests total (6 in `quantize_ex.rs`, 2 in
  `multibit_query.rs`), all passing; workspace total now 177, clippy
  clean.

  This closes RFC 0010's Non-goals' single largest remaining item. What
  remains from the original list: `t_const`/`faster_quantize_ex`'s
  construction-time speedup (unregistered, real writer-side optimization
  work), deletion-vector integration, and reranking against the
  flat-vector blob (Design §6 steps 4–5).
- **Deletion-vector integration implemented — 2026-08-19, same day,
  prompted by "implement deletion-vector integration next."** Unlike
  every module built so far this session, this one wasn't scoped to
  `strand-vector` at all: invariant 2 (`CLAUDE.md` §5) has stated "Deletes
  are deletion-vector blobs (Roaring)" since RFC 0001, but nothing
  anywhere in the codebase implemented it — not a blob format, not a
  manifest slot, not a reader. `spec/vectors.md` §6 step 4 and `spec/
  row-ids.md` §3 had both been citing a mechanism that didn't exist.
  **RFC 0012** (`rfcs/0012-deletion-vectors.md`) registers it, and its own
  central design fact drove everything downstream: a segment is one
  immutable object (`spec/container.md` §1), so a deletion vector —
  necessarily revisable, as deletes accumulate — cannot live inside a
  segment's own container bytes. It has to be its own object, referenced
  from the manifest, mirroring Iceberg/Delta's own position-delete-file
  pattern.

  **A new chapter, `spec/deletion.md`,** registers a standalone
  deletion-vector object (`family_id = 4`, `blob_type_id = 0`): no
  footer, no hotcache, just a standard 32-bit Roaring bitmap under
  `SERIAL_COOKIE_NO_RUNCONTAINER`, citing `spec/filter-bitmaps.md` §3's
  identical MUST rule by reference — that chapter's own §7 had explicitly
  anticipated this exact follow-on RFC and invited the citation. `spec/
  manifest.md`'s `SegmentRef` gains an optional `deletion_vector` field
  (`DeletionVectorRef`: path/byte_length/checksum, shaped like
  `SegmentRef`'s own).

  **The adversarial review caught a real, self-contradicting Critical
  bug**: the first draft's `commit_deletion_vector` closure took no
  parameters (`impl Fn() -> DeletionVectorRef`), yet the same draft's own
  "how this could be wrong" section required the closure to read current
  state fresh on every CAS retry to avoid a lost-update race — a
  signature that cannot do what the RFC's own text said it must.
  Implementing the RFC exactly as first drafted would have reproduced the
  race it claimed to prevent. Fixed by widening the signature to
  `impl Fn(&SegmentRef) -> DeletionVectorRef`, mirroring how `commit`'s
  own `build_segments` closure receives `next_row_id` fresh each retry —
  and this fix is verified directly, not just argued: `crates/strand-core/
  src/manifest.rs`'s `commit_deletion_vector_recomputes_against_a_
  concurrent_rivals_write_without_losing_a_tombstone` test injects a rival
  `commit_deletion_vector` call mid-retry (mirroring the existing
  `commit_recomputes_row_id_range_when_a_rival_commits_first` pattern) and
  confirms both writers' tombstones survive in the final bitmap. Three
  further Important gaps were fixed: a false claim that checksum
  verification-on-read already had precedent (`segment::open` doesn't
  exist anywhere in the codebase; `deletion::read` is genuinely the first
  code here to verify a stored checksum, not a precedent-follower); an
  unnamed error variant for "segment not found" (fixed:
  `CommitError::SegmentNotFound`); and a real, unreconciled tension
  between `spec/row-ids.md` §3 ("marks row-IDs, not local ordinals... 
  survives a merge... without needing a remap") and this RFC's
  local-ordinal wire encoding — resolved by distinguishing logical row-ID
  identity (stable) from the per-segment bitmap encoding (rebuilt at
  merge, not translated), made explicit in `spec/deletion.md` §2 rather
  than left for a reader to reconcile alone.

  **A genuine, load-bearing formal-verification gap was found by
  inspection, not assumed away**: `verification/manifest.tla`'s
  `ProposeSnapshot` action models a segment as `[base: Nat, count: Nat]`
  and its only transition is `Append` — there is no modeled shape for
  revising an existing entry's fields in place while leaving the segment
  count and row-ID allocation untouched. `commit_deletion_vector`'s new
  commit shape is real, unmodeled territory; RFC 0002's existing Approval
  does not cover it. Named precisely in RFC 0012's own "how this could be
  wrong" and in `spec/manifest.md`'s own new paragraph, not silently
  assumed covered — a real follow-on for a future formal-verification
  session, not resolved here.

  **Implementation**: `crates/strand-core/src/deletion.rs` (new) —
  `build_deletion_vector`/`DeletionVector::decode`/`is_deleted`/`read`,
  reusing `filter_bitmaps.rs`'s own `remove_run_compression`-before-
  serialize discipline for the identical no-run-container reason. Its own
  worked-example test serializes `{2, 5, 100}` and checks the *exact*
  real bytes RFC 0012's own worked example cites (`22` bytes,
  `3a 30 00 00 01 00 00 00 00 00 02 00 10 00 00 00 02 00 05 00 64 00`) —
  not just a round-trip check, a byte-exact one. `manifest.rs`'s
  `commit` was refactored (behavior-preserving; the existing property
  test `commit_invariants_hold_across_randomized_concurrent_rounds` and
  all prior hand-picked scenario tests still pass unchanged) to extract
  the write-snapshot-and-race-the-pointer mechanics into a shared
  `propose_snapshot` helper, so `commit_deletion_vector` reuses the exact
  same CAS code path rather than a parallel reimplementation that could
  drift from it. `crates/strand-vector/src/query.rs` gained
  `filter_deleted` — a small, separate function, not folded into
  `scan_selected_clusters`, matching `spec/vectors.md` §6's own step 3/
  step 4 boundary. `strand-vector` gains no new dependency of its own
  (no `roaring`, no `twox-hash`): `strand-core::deletion` re-exports
  `RoaringBitmap` and exposes a `checksum` helper, keeping the general
  invariant-2 machinery's dependencies out of every family that merely
  consumes it.

  **A real, full end-to-end test, not a unit test dressed up as one**
  (`crates/strand-vector/tests/deletion_end_to_end.rs`): a real segment
  is committed through `strand-core`'s actual manifest CAS protocol
  (`InMemoryStore`, real `commit`), a real cluster is built and queried —
  confirming a deliberately-nearest vector wins — then a real deletion
  vector is committed through `commit_deletion_vector`, the manifest is
  re-read fresh (as a real reader would, not by reusing in-hand state),
  and the same query now excludes the deleted row and promotes the
  runner-up. 14 new tests total (8 in `deletion.rs`, 3 new
  `commit_deletion_vector` tests in `manifest.rs`, 2 in `query.rs`, 1
  end-to-end), all passing; workspace total now 191, clippy clean.

  This closes RFC 0010's last remaining Non-goal besides reranking. What
  remains, named as RFC 0012's own Non-goals: compaction-time physical
  removal of tombstoned rows and deletion-vector merge semantics (both
  M3), retention-policy-driven expiry, the orphan-sweep tool's handling
  of superseded deletion-vector objects, extending `verification/
  manifest.tla` to model `commit_deletion_vector`'s new transition shape,
  and reranking against the flat-vector blob (RFC 0010 Design §6 step 5)
  — the one item left from the original vector-family Non-goals list.
- **Reranking against the flat-vector blob implemented — 2026-08-19, same
  day, prompted by "implement reranking against the flat-vector blob
  next."** The last item on RFC 0010's original Non-goals list, and
  deliberately *not* a new design question: unlike every RFC-gated module
  this session, this one required no new RFC and no new spec text — RFC
  0010 already designed, cited, and adversarially reviewed the
  flat-vector blob's own byte format (`spec/vectors.md` §5) and the exact
  query-resolution step this closes (`spec/vectors.md` §6 step 5: "fetch
  the flat-vector blob's rows for the surviving candidates and recompute
  exact distances"). The pattern itself — quantize aggressively for the
  cheap cold scan, recompute exact distances against full-precision
  vectors for the small surviving set — is not a novel technique either;
  it is what DiskANN, SPANN, turbopuffer, and Faiss IVF+PQ all do, already
  cited in `docs/lineage.md`/`docs/data-structures.md` as "the shape the
  entire cloud-native evidence base converges on." This was purely wiring
  already-approved design into real code, not a research task.

  `crates/strand-vector/src/query.rs` gained `exact_distance` (a private
  helper: squared L2 or negative inner product between raw, unrotated
  vectors — no separate `Cosine` case, since `spec/vectors.md` §8 already
  requires a cosine-descriptor writer to pre-normalize before
  quantization, making cosine search inner-product search over normalized
  vectors, the identical convention `quantize.rs`/`estimate.rs` already
  use for the same reason) and `rerank` (public): fetches each surviving
  candidate's row from the resident `flat::FlatVectorsReader`, recomputes
  the exact distance, and re-sorts. `Candidate.estimate`'s `lower_bound`/
  `upper_bound` collapse to the exact value post-rerank — reranking is
  precisely what removes the estimation uncertainty those bounds existed
  to describe, so collapsing them (rather than leaving stale quantized
  bounds) is the only self-consistent choice.

  **The real property this feature exists for, tested directly**
  (`tests/rerank_end_to_end.rs`): a real, deliberately tight and
  ambiguous 40-vector cluster (real quantization, real posting list, real
  flat-vector blob, all keyed by the same real row-ids — the regime where
  1-bit RaBitQ's own lossiness has a real chance to misorder close
  candidates, unlike `query_a_real_cluster.rs`'s earlier, deliberately
  well-separated case) is scanned, filtered, and reranked; the reranked
  order is asserted equal to an independently computed brute-force
  ordering over the original raw vectors, **not just "plausible" — exact,
  row-id for row-id**, and every reranked distance matches the true exact
  distance to within `1e-4`. A second, synthetic unit test
  (`rerank_fixes_a_ranking_the_quantized_estimate_got_wrong`) demonstrates
  the mechanism directly: two candidates deliberately handed to `rerank`
  in the *wrong* order (as a lossy quantized scan plausibly could) come
  back correctly ordered. 3 new tests, all passing; workspace total now
  194, clippy clean.

  This closes RFC 0010's original Non-goals list completely. What remains
  project-wide: everything RFC 0012 itself named as open (compaction,
  deletion-vector merge semantics, retention/orphan-sweep, the TLA+
  model gap) — all M3 scope, unchanged by this entry.
- **`verification/manifest.tla` extended to cover `commit_deletion_vector`
  — 2026-08-19, same day, prompted by "work in autobot" following the
  user's own recommended sequencing (close the model gap before starting
  either the DST harness or a TLAPS proof, since sinking effort into
  either against a model already known to be incomplete means redoing
  that work once the model catches up).** Closes the exact gap RFC 0012's
  own adversarial review found and named: `ProposeSnapshot`'s only
  transition is `Append`, with no shape for revising an existing entry in
  place.

  `SegmentRec` gained a `delVer: Nat` field — a bare generation counter
  standing in for a segment's `DeletionVectorRef`; the model has no reason
  to represent actual Roaring-bitmap content, only whether a revise-in-
  place commit can safely interleave with append-shaped commits through
  the shared pointer CAS. A new action, `ProposeDeletionVectorCommit(w)`,
  guarded by a new CONSTANT `DeleteWriter` (the same established pattern
  `DistinguishedWriter` already uses for varying one writer's shape
  without a combinatorial `CONSTANTS` explosion), revises the first
  segment's `delVer` in place, touching nothing else. `TryAdvancePointer`/
  `ResolveAmbiguity` needed no changes at all — they already operate
  generically on `wLocal[w].proposed`, regardless of which action produced
  it, confirming the real code's own "second commit path, same CAS
  mechanics" design (`spec/manifest.md`) holds at the model level too.

  **A real config mistake caught before it shipped, not after**: the
  first version pinned `DeleteWriter` to `w2` in the existing 2-writer
  config, which silently *removed* coverage rather than only adding it —
  with `w2` restricted to the revise shape, only `w1` remained
  append-capable, eliminating the append-vs-append racing this model
  existed to check in the first place. Fixed by adding a third writer
  (`w3 = DeleteWriter`) so the original 2-append-writer scenario is
  preserved exactly, with the new revise-shaped writer layered on top of
  it, not swapped in for part of it.

  **Two new invariants, both confirmed load-bearing by mutation test, not
  assumed to hold merely by construction** — this file's own established
  discipline, applied here exactly as it was for every invariant before
  it: `SegmentCountNeverDecreases` (segment count is monotonic
  non-decreasing across committed history) and
  `DeletionVectorCommitsOnlyReviseOneEntry` (between two consecutive
  snapshots with unchanged segment count, every segment's `base`/`count`
  must be identical and at most one segment's `delVer` may differ). Three
  mutation tests were actually run, not merely reasoned about: a mutation
  bumping a revised segment's `count` alongside its `delVer` was caught by
  the pre-existing `NextRowIdMatchesSegments` (a real, if incidental,
  catch); a mutation revising *every* segment's `delVer` at once passed
  every pre-existing invariant clean and was caught by
  `DeletionVectorCommitsOnlyReviseOneEntry` alone, confirming it adds real,
  independent coverage; a surgical mutation that drops a segment while
  folding its row count into a survivor (keeping
  `NextRowIdMatchesSegments` satisfied) passed every pre-existing
  invariant clean and was caught by `SegmentCountNeverDecreases` alone,
  confirming the same for it.

  TLC re-verified clean: **5,943 distinct states (22,286 generated), depth
  18** (up from 591/1,793, depth 14), all nine invariants holding.
  `verification/README.md`, `spec/manifest.md`, and RFC 0012's own
  Discussion section all carry the new baseline and the closed gap.

  What RFC 0002's own remaining scope still owes, unchanged by this entry:
  a TLAPS mechanized proof and a DST cross-validation harness, both
  against this now-extended model, neither started.
- **Real M0-style byte-budget measurement for the vector blob family —
  2026-08-19, prompted by "build a real M0-style byte-budget measurement
  for the vector blob family, the same way `bench/src/cold_open.rs` already
  gave invariant 3 a measured baseline for the generic container open."**
  Closes RFC 0010's own Open questions item naming exactly this gap
  ("M2's own milestone gate should not be considered met until one
  exists") and the matching "still open" line this file's own R1 entry
  carried above.

  `bench/src/vector_cold_open.rs` (new) follows the established pattern of
  `bench/src/cold_open.rs` and `bench/src/field_cold_open.rs`: a real
  segment, committed to real MinIO (`testcontainers`) via `strand-core`'s
  actual manifest CAS protocol, reopened cold and measured — not a segment
  of literal placeholder bytes. What makes it real here specifically: 10,000
  random 768-dimensional vectors clustered by the crate's own real
  `strand_vector::kmeans::kmeans` (Lloyd's algorithm, k-means++ seeding,
  `k = 400` from `recommended_cluster_count`'s `4·√N` rule, capped at 5
  iterations — enough for non-degenerate cluster sizes at this scale, not
  convergence, since blob *byte sizes* depend on cluster *count and
  occupancy*, not clustering *quality*), a real `FhtKacRotator` descriptor
  (`descriptor::build_fht_kac`), real rotation applied to both centroids and
  vectors (`rotate::rotate_fht_kac`), and real 1-bit RaBitQ quantization per
  vector against its assigned rotated centroid (`quantize::
  quantize_one_bit`) — the same functions this crate's own tests already
  exercise, reused rather than reimplemented. 10,000 vectors is the lower
  end of the RFC's own suggested ~10,000–50,000 range, chosen because real
  k-means is `O(n·k·dims)` per iteration and this scale keeps a single-core
  run bounded (k-means alone took 22.4s) while still assembling a genuine
  multi-cluster, multi-batch segment.

  **Real, measured result** (`bench/results/vector-cold-open.json`, MinIO
  on localhost, no injected network latency — the same standing caveat
  every prior M0 benchmark in this repository carries): the descriptor blob
  (400 bytes) and navigation-tier blob (1,238,408 bytes), read back from
  the real committed segment's own hotcache registry after a real
  footer/hotcache decode, total **1,238,808 open-wave bytes — 1.24% of the
  `CLAUDE.md` §7 100 MB cold-open byte budget** — fetched in a constant,
  asserted **3 GETs per open** (pointer, snapshot, segment), matching
  invariant 3's ≤4-GET bound. Extrapolating this run's own real per-cluster
  navigation-tier byte cost to RFC 0010's own 1,000,000-vector napkin-math
  scale via the same `4·√N` rule gives **12,384,408 bytes** — matching RFC
  0010's hand-computed **≈12.4 MB** figure to the byte. The formula was
  already right; this is the first time real, executed code confirmed it
  rather than arithmetic alone.

  **One limitation carried over honestly, not newly discovered**:
  `strand-core`'s `ConditionalStore` trait has no Range-GET method yet, so
  the real network fetch this benchmark issues at "open" downloads the
  *entire* 33,984,732-byte segment object — the 2,025,728-byte posting-list
  blob and the 30,720,000-byte flat-vector blob included, both correctly
  excluded from the open-wave byte *count* per `spec/vectors.md` §1's own
  tier distinction, but not from the actual network fetch this harness
  issues, since neither `bench/src/cold_open.rs` nor `bench/src/
  field_cold_open.rs` needed one before. Measured whole-segment-GET latency
  (p50 = 92.6ms, p90 = 102.2ms, p99 = 114.1ms, n = 30) is therefore a
  strictly harder number than a Range-GET-only open-wave fetch would show:
  real byte-count separation by blob type, not yet a real
  separated-latency measurement. Implementing a Range-GET path and
  re-measuring against real S3 or MinIO with injected latency (the same
  real-network-tail-latency follow-on `CLAUDE.md` §7's own placeholder
  already calls for) remain real, separate, unimplemented follow-on work.

  RFC 0010's Napkin math, Open questions, and Discussion sections all carry
  the new numbers (RFC 0010 Discussion, 2026-08-19); `docs/milestones.md`'s
  M2 entry and this file's own R1 entry above are updated in place rather
  than left stale.
- **`faster_quantize_ex`'s construction-time speedup implemented — M2-6,
  `docs/roadmap.md`, 2026-08-19.** RFC 0011's Non-goals named this as "a
  real, legitimate writer-side performance optimization this RFC does not
  standardize... any writer wanting it can implement it without a format
  change" — closing it therefore started with confirming that framing
  actually held, before writing any code, per `CLAUDE.md` §3: does this
  optimization change wire bytes, the quantization codec's output shape,
  or anything a reader needs to know? No. `quantize_ex_fast`'s output is a
  *different, equally valid* quantization of the same vector in general —
  a different rescale factor `t`, chosen once and shared across many
  vectors instead of searched per-vector — which is exactly the writer
  degree of freedom RFC 0011 Design §3's own byte-determinism carve-out
  already grants ("a reader MUST NOT assume two independent, conforming
  writers produce identical ex-code region bytes for the same logical
  vectors"). No RFC Discussion amendment was needed: the design question
  was already asked and answered, in RFC 0011's own approved text, before
  this session began.

  **Grounded against the reference's real function bodies, not the RFC's
  own paraphrase of them** — the vendored excerpt
  (`references/rabitq-library-multibit-quantization-source.md`) named
  `faster_quantize_ex`/`get_const_scaling_factors` by name but had not
  transcribed either function; both were re-fetched live from `include/
  rabitqlib/quantization/rabitq_impl.hpp` on the `RaBitQ-Library`
  repository's `main` branch (2026-08-19) and added to that reference file
  verbatim before any Rust was written, per `CLAUDE.md` §3's "never
  implement against a remembered spec" rule — the RFC's own Non-goals
  prose was a correct summary, but summarizing is not the same as having
  the actual source in the session.

  **Implementation**: `crates/strand-vector/src/quantize_ex.rs` gained
  `calibrate_rescale_factor` (100 random unit-norm Gaussian direction
  samples, `best_rescale_factor` run on each, averaged — `get_const_
  scaling_factors`, transcribed, with a caller-supplied seeded `rand::Rng`
  in place of the reference's own hardcoded seed, matching this crate's
  established RNG-plumbing convention from `kmeans.rs`/`orthogonal.rs`;
  inconsequential for byte-determinism since `t_const` is never itself
  wire-visible) and `quantize_ex_fast` (`faster_quantize_ex`, transcribed:
  the identical per-dimension clamp-and-round arithmetic `quantize_ex`
  already used, against a caller-supplied `t_const` instead of a fresh
  per-vector search). The existing `quantize_ex` was refactored — behavior-
  preserving, confirmed by the existing dim8/dim16 worked-example tests
  passing unchanged — to extract the shared downstream factor computation
  (sign complement, `total_code`/`xu_cb` reconstruction, `f_add_ex`/
  `f_rescale_ex` formulas) into `finish_quantization`, called by both entry
  points, so the only place the two functions' output can possibly differ
  is in how the magnitude code and `ipnorm_inv` are obtained — mirroring
  the reference's own `ex_bits_code(..., t_const = -1)` branch structure.
  `orthogonal.rs`'s `sample_standard_normal` widened from private to
  `pub(crate)` so `calibrate_rescale_factor` reuses the crate's one
  existing Box-Muller transcription rather than adding a second.

  **Proof of equivalence, not just assertion — invariant 9's discipline
  extended to a non-SIMD writer optimization, as the task itself framed
  it.** Bit-for-bit equality between `quantize_ex_fast` and `quantize_ex`
  for the *same input* does not hold in general and is not claimed — that
  would contradict RFC 0011's own "changes which valid `t` a writer
  converges to" framing. What is proved bit-exact instead: given the
  *identical* `t` (the exact value `best_rescale_factor` finds for a
  specific vector, fed to `quantize_ex_fast` as `t_const`), the two entry
  points produce identical `ExQuantizedVector` output, over three real
  worked-example vectors at `ex_bits` 2 and 3, both metrics
  (`fast_path_matches_the_full_search_path_given_the_same_t`) — proving
  the shared downstream arithmetic is one correct implementation shared by
  both callers, not two independently drifting ones. A second test
  confirms the zero-residual guard fires identically on both entry points
  regardless of `t_const`. A third confirms `calibrate_rescale_factor` is
  deterministic given the same seed and differs across seeds (ruling out a
  constant-output bug). A fourth measures reconstruction error against the
  true residual across a batch of synthetic vectors and confirms the
  calibrated `t_const` stays within 5x of the full search's mean-squared
  error — a real quality floor, not just "doesn't crash." 6 new tests in
  `quantize_ex.rs`.

  **Measured speedup, not a plausibility claim**: a 7th test (`#[ignore]`d
  by default, timing-based, following `orthogonal.rs`'s own convention for
  scale-dependent checks — run explicitly with `cargo test -p strand-vector
  --release -- --ignored`) quantizes 2,000 synthetic vectors at
  `dim = 768, ex_bits = 7` (the widest registered `ex_bits`, the worst case
  for the full search's heap-refill cost) with both paths. Real, executed,
  `--release` result: full per-vector search 5.947s (2,973 µs/vector);
  `quantize_ex_fast` against a pre-calibrated `t_const` 0.259s
  (129 µs/vector) — **23.0x measured speedup**. `rfcs/0011-multibit-
  extended-rabitq.md` Non-goals carries the same numbers in place, marked
  closed rather than left stale; `docs/roadmap.md`'s M2-6 entry likewise.

  This item was writer-side-only exactly as RFC 0011 predicted, and did
  not touch `posting_list.rs`, `estimate.rs`, `query.rs`, or `spec/
  vectors.md` — no production call site wires `quantize_ex_fast` into a
  default construction path yet, since no such end-to-end segment-building
  writer pipeline exists in this crate today (`quantize_ex` itself is
  presently exercised only by its own tests, not by any orchestrating
  writer); wiring either quantization strategy into a real writer pipeline
  remains future work, unblocked by this closure.

- **X-2 (`docs/roadmap.md`) — RFC 0001's three remaining Open Questions items,
  resolved 2026-08-19 against real measurement, not guessed.** All three named
  in RFC 0001's own Open Questions section as owed a real value, not just a
  bound's existence.

  **Speculative tail-read size `N` and the hotcache-size ceiling.**
  `bench/src/hotcache_tail_read.rs` built real segments across a blob-count
  sweep (1, 12, 50, 100, 250, 500, 1000 — 12 is today's real maximum for one
  field spanning every registered family, `spec/container.md` §9), committed
  each to real MinIO, and executed RFC 0001 §1's actual two-phase open
  protocol (`S3Store::get_tail_range`, a real `Footer`/`Hotcache` decode, a
  conditional second range GET) across candidate `N` values from 512 B to
  16 KB. The measured one-RTT/two-RTT transition tracked the RFC's own
  `hotcache_length + 40 <= N` check exactly: 100 blob entries (3,420-byte
  hotcache) stayed one RTT by `N = 4096`; 250 blob entries (8,520-byte
  hotcache) stayed one RTT by `N = 16384`; 500 and 1,000 blob entries needed
  two RTTs at every tested `N`. Recommended default: **`N = 16384` bytes
  (16 KiB)**, implying a hotcache-size ceiling of **16,344 bytes (≈480 blob
  entries)** before an open silently degrades from one RTT to two —
  comfortably above today's real 12-blob-entry maximum (428 bytes, ≈40x
  headroom), chosen over smaller candidates because measured latency showed
  no real marginal cost from the larger window. Full data:
  `bench/results/hotcache-tail-read.json`;
  `rfcs/0001-container-rowid-manifest.md` Discussion.

  **Reader 404-refresh retry bound.** `bench/src/reader_refresh_contention.rs`
  ran 4 concurrent writers committing back-to-back against real MinIO (60
  total commits), a compactor deleting each snapshot the instant a newer one
  became current (the tightest race window the deletion-safety rule,
  `CLAUDE.md` §6, allows), and 4 concurrent readers hammering `read_snapshot`
  throughout, recovering each real call's internal retry count from
  `CountingStore`'s GET count. Across **691 reads** sampled, only **1** needed
  a single internal retry and none exhausted the bound.
  `manifest::READER_REFRESH_RETRY_LIMIT` stays at **5** — already ≈5x the
  observed worst case, so this measurement confirms the provisional value
  rather than changing it — and is now `pub`. Same standing caveat every
  `bench/` cold-path measurement in this file carries: MinIO on localhost, no
  injected network round-trip latency, so the observed race window is a lower
  bound on a real deployment's, not an upper one. Full data:
  `bench/results/reader-refresh-contention.json`.

  **Suffix-range server support.** AWS's `GetObject` API reference
  (`references/aws-s3-getobject-range-parameter.md`, fetched 2026-08-19)
  demonstrates only the explicit-end range form and is silent on the suffix
  form either way — confirming RFC 0001's original framing rather than
  changing it. The server-support question itself is now closed empirically
  for MinIO: `crates/strand-core/tests/s3_store.rs`'s
  `suffix_range_get_is_honored_by_minio` issues a raw `bytes=-10` suffix-range
  GET against real MinIO and confirms it is honored correctly (right bytes,
  correct `Content-Range`). Real S3 remains untested — no AWS credentials were
  available in this environment — so the finding is MinIO-specific, not a
  claim about S3. RFC 0001's open protocol is unaffected either way, since it
  was designed specifically not to depend on the answer.

  `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D
  warnings` both clean alongside this work. `rfcs/0001-container-rowid-
  manifest.md` Discussion carries the full methodology; `docs/roadmap.md`'s
  X-2 entry is updated to reflect this as done.
- **X-1 (`docs/roadmap.md`) — multi-field blob addressing, resolved
  2026-08-19.** The real, load-bearing gap the adversarial review found:
  `spec/container.md` §5's blob registry entry carried no field
  identifier, so two fields' blobs sharing a `blob_type_id` (a real-world
  requirement for any index with more than one text field) could not
  coexist in one segment — RFC 0008's and RFC 0009's own Non-goals both
  named this as unsolved project-wide, inherited from RFC 0005/0006/0007.

  **Design.** A new `field_id: u64` field on `blob_entry`
  (`spec/container.md` §5a), growing it from 34 to 42 bytes. `0`
  (`FIELD_ID_NONE`) is reserved for "no field association" (segment-scoped
  blobs — deletion vectors, RFC 0012 — or RFC 0001's own anonymous
  worked-example blob, unaffected). Every other value is `xxHash3-64`
  (`crates/strand-core/src/container.rs`'s `field_id_from_name`) over the
  field's declared name, raw UTF-8 bytes, no normalization — the same
  checksum algorithm the footer already names, reused rather than
  registering a second one (invariant 8). A reader now matches
  `(family_id, blob_type_id, field_id)`, not the first two alone. Two
  alternatives were considered and rejected in
  `rfcs/0001-container-rowid-manifest.md` Discussion: a per-segment
  ordinal (rejected — not stable across independently built segments
  without external coordination, defeating a future cross-segment reader
  like M5-1's `TableProvider`) and an explicit name-to-ID catalog blob
  (rejected — either grows the hotcache's own one-wave budget or adds a
  dependent fetch invariant 3 rules out, for no benefit over a
  self-computing hash). The residual collision risk is quantified, not
  waved away: at 100 fields in one segment, the birthday-bound collision
  probability is ≈2.7 × 10⁻¹⁶; the design's nearest grave from
  `docs/lineage.md` is named as Pilosa ("a good structure with a spec is
  not a distribution strategy") — the real risk is real deployments
  needing an external field-name catalog anyway, which this format does
  not specify, the same layering choice invariant 6 already makes for
  analyzer descriptor agreement.

  **Wire change and its downstream effects.** `crates/strand-core/src/
  container.rs`'s `BlobEntry`/`Hotcache` encode/decode updated
  (`BLOB_ENTRY_SIZE = 42`); `crates/strand-core/src/segment.rs`'s
  `BlobSpec`/`SegmentBuilder` thread `field_id` through to the registry.
  `crates/strand-lexical/src/field.rs`'s `build_field`/
  `build_field_without_positions`/`build_field_from_postings` now take a
  `field_name: &str`, storing `field_id_from_name(field_name)` on the
  returned `FieldBlobs` and every `BlobSpec` `to_blob_specs` emits;
  `FieldReader::open` takes an explicit `field_id`, and a new
  `FieldReader::open_by_name` computes it from a name the same way a
  writer did. Every caller across `crates/strand-tools`, `bench/`, and
  `crates/strand-lexical/tests` updated to pass a field name (`"passage"`,
  `"body"`, `"title"`, or, for `strand-tools convert`, the real imported
  tantivy field name). `format_minor` was deliberately left at `1`,
  unbumped — consistent with RFC 0009's own Fix 1 (an equally breaking,
  in-place wire change) also leaving it unbumped — named as a real,
  project-wide inconsistency in RFC 0001's Discussion rather than fixed
  unilaterally here.

  **Proof.** `conformance/container/toy-segment.bin` regenerated
  (34-byte → 42-byte `blob_entry`, `field_id = 0` for that blob's
  unaffected anonymous case). A new golden file,
  `conformance/container/multi-field-segment.bin`, pins a real two-field
  worked example (`"title"`/`"body"`, both `family_id = 1, blob_type_id =
  0`, disambiguated only by `field_id`) —
  `crates/strand-core/tests/multi_field_worked_example.rs` checks both the
  byte-exact golden file and that reading it back resolves each field's
  own 4-byte payload, never the other's. Property-based round-trip tests
  (`crates/strand-core/src/container.rs`, `proptest`) cover `BlobEntry`
  and `Hotcache` encode/decode for arbitrary `field_id` values, including
  arbitrary multi-blob, multi-field-id hotcaches, plus a dedicated
  property test that `field_id_from_name` never produces the reserved
  sentinel for any non-empty input. A new end-to-end test,
  `crates/strand-lexical/tests/field_end_to_end.rs`'s
  `two_fields_with_the_same_blob_type_ids_coexist_in_one_segment_and_stay_disambiguated`,
  builds two real fields (different documents, different postings for a
  shared term "dog") into one segment and confirms each field's reader
  resolves only its own matches — including the negative case, a term real
  in only one field missing cleanly in the other rather than silently
  falling through. `cargo test --workspace` and `cargo clippy --workspace
  --all-targets -- -D warnings` both clean. Full design, the alternatives
  considered, and the adversarial "how this could be wrong" review live in
  `rfcs/0001-container-rowid-manifest.md` Discussion (the RFC that owns
  `spec/container.md` §5); short pointer notes were added to RFC 0008's
  and RFC 0009's own Discussion sections, both left otherwise unmodified
  as an accurate record of the gap at their own approval. Directly
  unblocks M5-1 (a `TableProvider` reading a multi-field index needs this)
  — M5-1 itself is not implemented here. `docs/roadmap.md`'s X-1 entry is
  updated to reflect this as done.
- **Table metadata and retention-eligibility implemented (M3-4) —
  2026-08-19.** `_strand/metadata.json` (`crates/strand-core/src/
  table_metadata.rs`): `TableMetadata`, `CasHost` (the two JSON shapes RFC
  0001 Design §3 already named — `{"type": "native", "store": ...}` /
  `{"type": "catalog", "uri": ...}`, verified byte-for-byte against those
  exact literals, not just round-tripped through the same code), and
  `RetentionPolicy` (`min_snapshots_to_keep`, `max_snapshot_age_millis`,
  spec's own "a count, a duration, or both"). `write_table_metadata`/
  `read_table_metadata` are a write-once create (`put_if_absent`) plus a
  plain read — no pointer, no proposed-vs-current distinction, no retry
  loop, and therefore no new action on the `_strand/current` CAS protocol
  `verification/manifest.tla` models: this object sits entirely outside
  that protocol's shape, so the model needed no change. An `Ambiguous`
  create outcome is resolved the same way `manifest.rs`'s pointer CAS
  already does — a follow-up read checking whether this attempt's own
  bytes are the ones now present — applied to a plain create instead of a
  compare-and-swap.

  `table_metadata::retained_snapshots` is the pure, I/O-free
  retention-eligibility function `CLAUDE.md` §6's deletion-safety rule
  depends on and the M3-5 orphan-sweep tool will call directly: given a
  `RetentionPolicy`, a snapshot list, and a "now" timestamp, which
  snapshots are still retained (and therefore which files a sweep MUST
  NOT delete).

  Implementing this surfaced two real gaps the original approved shape
  left unstated, both resolved through RFC 0001's Discussion section per
  `CLAUDE.md` §3 rather than decided silently inside this module — neither
  is a new manifest commit action, so neither touches
  `verification/manifest.tla`. First, `SnapshotMetadata` had no wall-clock
  field at all, so a duration-based policy had nothing to measure age
  against; `committed_at_millis: u64` was added, stamped by the proposing
  writer immediately before each snapshot object is written (`manifest::
  now_millis`) — additive only, explicitly carved out of invariant 11's
  byte-determinism pins the same way `writer_nonce` already is, since it
  is real time, not part of the logical input two implementations must
  converge on. Second, the spec named two retention knobs but never said
  how "both" combine; resolved as the union (a snapshot retained by either
  criterion is retained), argued from the deletion-safety rule's own
  asymmetric cost — under-retaining risks real, unrecoverable data loss,
  over-retaining only costs storage, a cost this project already accepts
  elsewhere — and confirmed against Apache Iceberg's documented behavior
  for its own equivalent pair of knobs
  (`references/iceberg-snapshot-expiration-retention-properties.md`: its
  `expire_snapshots` procedure keeps the last N snapshots "regardless of"
  the age cutoff, a real quoted union), the same prior art RFC 0001
  already cites for this protocol's optimistic-concurrency shape. The
  current snapshot is additionally always retained regardless of either
  policy field, a floor the spec text implied but never stated outright.

  16 new tests, `InMemoryStore`-backed: JSON round-trips for
  `TableMetadata` and both `CasHost` variants, a real write/read round
  trip through a store, write-once rejection of a second create, rejection
  of a retention policy with neither field set, and `retained_snapshots`
  exercised against a snapshot within the duration window, one outside it,
  the inclusive boundary and one millisecond past it, the count-only
  floor, the union case, the always-retain-current floor even when every
  policy field would otherwise expire it, an empty snapshot list, and
  return-order. `crates/strand-core/src/manifest.rs`'s own
  `SnapshotMetadata` round-trip test and `tests/s3_store.rs`'s orphan
  crash-test literal were updated for the new field; workspace-wide
  `cargo test --workspace` and `cargo clippy --workspace --all-targets --
  -D warnings` both clean. Real-MinIO coverage for this object (matching
  `tests/s3_store.rs`'s coverage of the CAS protocol itself) remains open,
  named in `spec/manifest.md` §5 rather than left implicit. M3-5 (the
  orphan-sweep tool) was explicitly out of scope for this session — its
  own roadmap entry records what it now has to build on top of this.
- **M4-5 (`docs/roadmap.md`) — Puffin blob-type packaging RFC drafted, not
  yet approved — 2026-08-20.** `rfcs/0013-puffin-export-sidecar.md`
  answers the scoping question `docs/milestones.md`'s M4 entry left open
  ("Puffin blob-type packaging RFC," no further detail): a one-way,
  on-demand STRAND → Puffin **export sidecar**, never a redefinition of
  STRAND's own container format around Puffin's shape. Grounded against
  the real, current Puffin v1 spec and the real, official
  `apache/iceberg-rust` crate's `puffin` module (both fetched and vendored,
  `references/puffin-spec-and-iceberg-rust-implementation.md`), not a
  remembered shape (`CLAUDE.md` §3). The RFC walks Puffin's real
  `BlobMetadata` schema field by field against STRAND's own invariants and
  finds it cannot host `spec/container.md`'s registry contract without
  silently dropping invariants 7, 10, and 11's per-blob guarantees and
  `spec/container.md` §5a's field-disambiguation mechanism — no `field_id`
  slot, no `storage_class`/`tier`/`alignment`, and, checked by
  direct read of the schema table, no per-blob checksum field at all — so
  a "STRAND segment is a Puffin file" container profile is rejected in the
  RFC's own Design §1, not left undiscussed. The one real translation
  target is STRAND's deletion vector: Puffin's own registered
  `deletion-vector-v1` blob type turns out to need no repacking of
  STRAND's existing Roaring bitmap bytes at all (`spec/deletion.md` §1's
  own `row_id_count <= 2^32` cap means exactly one Puffin position-key
  group), only a wrapping layer this RFC's own worked example computes
  byte-exact (a real 46-byte translated blob and a real 345-byte Puffin
  file, Python-script-computed, reproducible by anyone from the RFC's own
  tables). Everything else gets one STRAND-namespaced opaque passthrough
  type (`strand-segment-blob-v1`), honestly labeled as structural
  visibility only, not semantic interop. A genuine, checked-not-assumed
  finding grounds the RFC's own adversarial section: no crate named
  `puffin*` on crates.io implements this file format (confirmed via the
  crates.io API directly, not the JS-rendered search page — every result
  under that name is an unrelated game-profiler crate family), but the
  real implementation lives inside `apache/iceberg-rust`'s own `iceberg`
  crate (v0.10.1, 1.85M downloads, Apache-2.0), whose
  `PuffinReader::new(input_file)` opens a **bare** file with no Iceberg
  table required — real, checked evidence weighed directly against
  `docs/lineage.md`'s own standing skepticism ("Puffin's registry has
  spawned essentially no third-party blob ecosystem... a good container,
  not a distribution strategy," nearly the same sentence the graveyard
  uses for Pilosa) in the RFC's own "How this could be wrong," rather than
  either dismissed or oversold. **Left Draft, deliberately, not
  self-declared Approved**: the RFC's own Status bullet states plainly
  that committing STRAND to a second wire format and a second checksum
  algorithm (Puffin's own CRC-32, scoped only to this RFC's export bytes —
  invariant 11's xxHash3-64 default is untouched elsewhere) for narrow,
  unproven interop value is exactly the class of decision `CLAUDE.md` §3's
  "not in the same breath" principle means to protect from being
  rubber-stamped by the same pass that drafted it; a genuine independent
  adversarial review, not merely this session's own inline one, is what
  Approval requires here. Explicitly out of scope, named in the RFC's own
  Non-goals: a Puffin → STRAND importer, chunked/per-block export of large
  blobs (postings, vector blobs — Puffin supports only whole-blob lz4/zstd,
  no chunk index), any change to `spec/manifest.md`'s snapshot metadata (the
  sidecar is never referenced by a `SegmentRef`), and registering new blob
  types with the Iceberg project itself. No crate code was written — the
  deliverable is the RFC draft, per this task's own instruction and
  `CLAUDE.md` §3.
