# Roadmap

A task-level breakdown of everything remaining in the project, as of
2026-08-19. This document does not replace `docs/milestones.md` — that
file states each milestone's *scope and gates*, settled at RFC-approval
time; this one decomposes that scope into discrete, individually
completable tasks, states what each depends on, and is the input to the
dependency graph and execution plan that follow it. Every item here is
sourced: a milestone entry, an RFC's Non-goals/Open-questions section, or
a `docs/ledger.md` R-track entry — nothing here is invented fresh.

**This document has been through one adversarial-review pass** (four
independent reviewers — roadmap-accuracy against primary sources,
dependency-graph correctness, whole-project consistency, and the
document's own internal soundness — each finding then independently
re-verified). 25 of 29 candidate findings were confirmed and are folded
into the text below; the whole-project consistency findings (stale
cross-references in `docs/lineage.md`, `docs/research/README.md`,
`docs/data-structures.md`, `CLAUDE.md` §8, a misaligned spec table, an
empty scratch directory, and `docs/superpowers/plans/`'s own staleness)
were fixed directly in their own files, not here — see the commit that
lands alongside this revision for the full list.

## How to read this

Each task has: an ID, a one-line description, its source citation, its
current status, and its dependencies (by ID). Status is one of `done`
(already shipped, listed for completeness and dependency-graph accuracy
only), `open` (real, scoped, unstarted work), or `blocked` (open, but
gated on another task — used whenever a task's own text uses hard-gating
language like "only actionable once" or "cannot start before"; softer
sequencing preferences ("more valuable after," "more meaningful once")
keep a task `open` with the preference noted in prose, not `blocked`).
Tasks are grouped by the milestone that owns them, but dependencies cross
milestone boundaries freely.

## Recently shipped (this session) — `done`, for dependency-graph completeness

Listed because several `open` tasks below depend on these having landed,
and because the `done` status this document defines was never otherwise
exercised — a real gap the internal-soundness review caught. Full detail
in `docs/ledger.md`; not repeated here.

- **done-1** — 1-bit and multi-bit RaBitQ quantization, `MatrixRotator`
  generation, the `nprobe` cluster-selection pipeline (RFC 0010, RFC 0011).
- **done-2** — Deletion-vector integration: the general invariant-2
  mechanism (`spec/deletion.md`, RFC 0012) and the vector family's own
  `filter_deleted` integration.
- **done-3** — Reranking against the flat-vector blob (RFC 0010 Design §6
  step 5).
- **done-4** — The TLA+ manifest model's `commit_deletion_vector` extension
  (`ProposeDeletionVectorCommit`, two new invariants, both mutation-tested;
  RFC 0002 Discussion — post-approval amendments).

## M1 — Lexical (implemented; three real grounding/design gaps remain, plus one open implementation follow-on from M1-1's resolution)

M1 shipped (BP128 postings, positions, FST term dictionary, block-max,
filter bitmaps, analyzer descriptors, RFC 0009's per-term overhead
reduction) — but four items its own approving RFCs left explicitly open
were missing from this document's first draft, found by this document's
own adversarial review. One of the four, M1-1, is now resolved as a
format-design decision (2026-08-19); its own resolution surfaced two new,
narrower implementation/verification tasks, M1-6 and M1-7, listed after
M1-5 below. M1-6 is itself now done (2026-08-19); M1-7 (the
segmentation-accuracy bake-off) remains open.

- **M1-1** — ~~Which CJK/Thai/Lao segmentation dictionary STRAND adopts as
  a default~~ — resolved. Source: RFC 0004 Non-goals and Open questions
  ("gates real conformance for those scripts, not an edge case"). Status:
  **done** — RFC 0004 Discussion — post-approval amendments license-audits
  five live candidates (MeCab, Lindera, Jieba/`jieba-rs`, ICU4C via
  `rust_icu`, ICU4X's `icu_segmenter`) and recommends ICU4X's
  `icu_segmenter` (`WordSegmenter::try_new_dictionary()`, Unicode-3.0
  license) as the default, named in `spec/analyzer-descriptors.md` §5.
  This closes the *design* gap only — see M1-6 and M1-7 below for the
  implementation and accuracy-verification work this resolution itself
  named as still open. Depends on: nothing.
- **M1-2** — FST term-dictionary size at realistic vocabulary scale
  (MS MARCO or larger) is unmeasured. Source: RFC 0005 Open questions
  ("needed before the cold-open byte budget question... can be answered
  with a real number instead of a structural argument"). Status: **done**
  (2026-08-19) — `bench/src/term_dict_size.rs`, reusing `bench/`'s existing
  real MS MARCO indexing infrastructure (`docs/ledger.md`'s field-end-to-end
  benchmarks) directly: real full MS MARCO corpus (8,841,823 passages)
  tokenized and built into the real production FST: 2,669,086 distinct
  terms, 19,423,389 bytes (≈18.5 MB, 7.277 bytes/term). Depends on: nothing.
- **M1-3** — Cross-crate-version and cross-platform determinism of the
  `fst` crate's compiled output: build the same logical FST on x86_64 and
  ARM, byte-compare. Source: RFC 0005 Open questions, repeated in RFC 0006
  Open questions ("before this blob's invariant-11 conformance is fully,
  not provisionally, satisfied"). Status: open, verification work,
  requires ARM hardware or emulation access. Depends on: nothing.
- **M1-4** — Cross-platform/cross-version determinism for the `roaring`
  crate half of the filter-bitmap store (a narrower, same-version/
  same-platform risk than M1-3's `fst` half, already closed for the
  no-run-container MUST rule by a real round-trip test — the remaining
  risk is genuinely cross-version/cross-platform only). Source: RFC 0006
  Open questions. Status: open, verification work. Depends on: nothing.

Also grounding debt, not M1-blocking but M1-adjacent and feeding M4:

- **M1-5** — R4's tantivy-half doc-length-accounting gap: the Lucene half
  is resolved (RFC 0004, `discountOverlaps` grounded byte-exact); no
  tantivy source has been vendored for the tantivy half. Source: `docs/
  ledger.md` R4; RFC 0004's own adversarial review ("gates M4's
  tantivy-fork parity work, not M1"). **Status: resolved 2026-08-19**
  (`references/tantivy-fieldnorm-overlap-accounting.md`, tantivy tag
  `0.26.1`) — tantivy has no `discountOverlaps`-equivalent mechanism at
  all; its field-length count increments unconditionally per token,
  equivalent to `counts_overlaps_in_length = true` in RFC 0004's own
  vocabulary, the opposite of what `lucene-parity` scoring requires.
  Depends on: nothing directly; **unblocks M4-4** (the tantivy fork now
  has the grounded fact it needs to implement length-accounting parity,
  even though the patch itself is still M4 work) — tracked here, under
  M1, because the gap itself was M1-era grounding debt even though its
  consequence lands in M4.
- **M1-6** — Implement M1-1's resolved default: populate
  `segmentation_dictionary` in `crates/strand-lexical/src/analyzer.rs`
  using ICU4X's `icu_segmenter` (`WordSegmenter::new_dictionary()` — not
  `new_auto()`/`try_new_auto()`, which silently substitute LSTM for
  Thai/Lao, RFC 0004 Discussion), and add at least one real
  dictionary-segmented vector to `conformance/analyzers/` (a CJK or
  Thai/Lao worked example, the same raw-text-in/token-stream-out shape as
  `lucene-en-word-only-01.json`). Also pin `segmentation_dictionary.version`
  concretely — crate semver, a compiled-data content hash, or both, per
  invariant 11 — a decision RFC 0004's amendment left to this task.
  Source: RFC 0004 Discussion — post-approval amendments; `spec/
  analyzer-descriptors.md` §5. Status: **done** (2026-08-19) —
  `crates/strand-lexical/src/analyzer.rs` (`tokenize_dictionary_segmented`,
  `analyze_dictionary_segmented_word_only`), `icu_segmenter 2.3.0` added to
  `crates/strand-lexical/Cargo.toml` with `default-features = false,
  features = ["compiled_data"]` (excludes `auto`/`lstm`, a compile-time
  guardrail against the LSTM trap); one real Han-script conformance vector,
  `conformance/analyzers/icu4x-dictionary-zh-01.json`; `version` pinned as
  the `icu_segmenter`/`icu_segmenter_data` crate-semver pair, not a content
  hash (`spec/analyzer-descriptors.md` §5 states the reasoning). Fetching
  the real crate source at implementation time found RFC 0004's own
  amendment had named the wrong constructor
  (`try_new_dictionary()` vs. the real `new_dictionary()`); corrected in
  the RFC and spec, dictionary-vs-LSTM distinction unaffected. Thai, Lao,
  Hiragana, and Katakana remain unvectored — one script's vector does not
  cover the whole default; see M1-7. Depends on: M1-1 (done).
- **M1-7** — Run a real segmentation-accuracy bake-off validating (or
  revising) M1-1's default: ICU4X's dictionary path vs. Lindera+IPADIC for
  Japanese, vs. Jieba for Chinese, vs. PyThaiNLP for Thai, the same
  discipline R2 applied to postings codecs (`docs/ledger.md` R2). M1-1's
  recommendation was made on license and dependency-shape grounds, not
  measured accuracy — RFC 0004's own amendment names this gap explicitly.
  Source: RFC 0004 Discussion — post-approval amendments, "How this could
  be wrong." Status: **done, Chinese-only** (2026-08-19) —
  `bench/src/cjk_segmentation_bakeoff.rs` measured ICU4X's `icu_segmenter
  2.3.0` dictionary path against `jieba-rs 0.10.3` over 8 real Chinese
  Wikipedia sentences: 0.9154 micro-average interior-boundary agreement (an
  inter-segmenter agreement metric, not gold-standard accuracy — no
  verifiably fetchable gold-segmented Chinese corpus was available in this
  pass). ICU4X measurably under-merges compound nouns relative to jieba-rs,
  a real but moderate accuracy cost that does not overturn M1-1's
  recommendation on this sample. Full write-up: `rfcs/
  0004-analyzer-descriptors.md` Discussion, `docs/ledger.md` R4,
  `bench/results/cjk-segmentation-bakeoff.json`. **Japanese
  (Lindera+IPADIC) and Thai (PyThaiNLP) are explicitly NOT covered** —
  Thai has no maintained Rust binding at all (`references/
  pythainlp-license-and-rust-gap.md`), and a Japanese run needs a real
  IPADIC data-license audit beyond the existing code-license check
  (`references/lindera-rust-morphological-analyzer.md`); both remain open,
  named follow-on work, not gating M1-6 (a default need only be declared
  correctly, not proven best) but should land before M1's analyzer work is
  considered fully closed. Depends on: nothing.

## M2 — Vectors (nearly closed; loose ends remain)

`docs/milestones.md`'s M2 entry is now long and mostly a record of
completed work (see "Recently shipped," above). What RFC 0010/0011's own
Non-goals and Open questions still leave genuinely open:

- **M2-1** — SPANN-style closure-replication's metadata slot and
  construction algorithm. Source: RFC 0010 Non-goals and Open questions
  ("a stated M2 milestone deliverable this RFC does not complete");
  `docs/milestones.md`'s M2 text names "the replication knob and tier-1
  sizing limits in blob metadata" as a stated deliverable. **Status: done**
  (2026-08-19) — RFC 0010 Discussion — post-approval amendment; the
  metadata slot is a new, always-present 8-byte `replication_descriptor`
  trailer on the cluster navigation tier blob (`spec/vectors.md` §3,
  `crates/strand-vector/src/navigation.rs`), and the construction
  algorithm is `crates/strand-vector/src/closure.rs`'s `closure_replicate`
  — SPANN's own Eq. 2 closure criterion plus RNG-rule pruning, grounded
  against the paper's own text and cross-checked against its reference
  implementation `microsoft/SPTAG` (`references/
  spann-closure-assignment-algorithm.md`). Real tests including a
  hand-checked worked example and a real end-to-end query proving
  deduplication (`crates/strand-vector/tests/
  closure_replication_end_to_end.rs`). Every pre-existing call site of
  `build_navigation_tier` needed no changes. Two real, named residual
  gaps, not folded into this resolution: compaction-time re-replication
  after a `rebalance` merge (real, separate, unimplemented — the
  construction algorithm runs at initial segment build time only), and the
  RNG-rule's exact call-site fidelity to SPTAG's own closure-assignment
  code (partial corroboration only — bounded consequence, since the rule
  only ever reduces the realized replication count, never the format's
  worst-case byte-cost bound). Depends on: nothing (design work against
  already-shipped blob formats).
- **M2-2** — A real M0-style byte-budget measurement for the vector blob
  family (the same way `bench/src/cold_open.rs` gave invariant 3 a
  measured baseline). Source: RFC 0010 Open questions ("M2's own
  milestone gate should not be considered met until one exists"). Status:
  **done** (2026-08-19) — `bench/src/vector_cold_open.rs`; real
  four-blob-type segment (10,000 real 768d vectors) committed to real
  MinIO and reopened cold 30 times: 1,238,808 open-wave bytes (1.24% of
  the 100 MB budget), 3 GETs/open, p50=92.6ms. Depends on: nothing.
- **M2-3** — The graph-blob family (warm tier, DiskANN/Vamana), including
  R1's second half (the node-order permutation algorithm question,
  Starling vs. an untested alternative). Source: `CLAUDE.md`'s own mission
  statement ("the warm-tier graph blob family is in-scope but explicitly
  second") and RFC 0010 Non-goals/Open questions ("entirely untouched by
  this RFC"). **Confirmed by adversarial review**: this is not a small
  task — it is a full new blob family requiring its own RFC (design,
  worked example, napkin math, adversarial review) before any code, the
  same weight RFC 0010 itself carried for the cluster family, and
  `docs/milestones.md` genuinely names no explicit M2/M3 boundary sentence
  for it (verified directly against the file's M2 and M3 paragraphs).
  Status: open, RFC-sized. Depends on: nothing technically, but sequencing
  it after M2-1/M2-2 is recommended so the cluster family's own remaining
  loose ends don't compete for the same design attention.
- **M2-4** — Fetch SPANN's real body figures (`arxiv.org/abs/2111.08566`
  PDF) to replace the provisional, flagged-unverified 1.73×/≈227 MB
  replication estimate. Source: RFC 0010 Open questions. Status: **done**
  (2026-08-19) — SPANN's own paper contains no GIST1M/index-size figure;
  the real 13.0 GB/7.5 GB replica-8/replica-2 figures live in the
  companion Li et al. benchmark paper (`references/spann-body-figures.md`).
  The 1.73× ratio is unchanged; only its citation and confidence label
  are. Depends on: nothing (informed M2-1 but didn't block it).
- **M2-5** — FastScan `kBatchSize = 32`'s hardware-vs-algorithm
  provenance: fetch `src/simd/fastscan_avx2.cpp`/`fastscan_avx512.cpp` to
  settle whether 32 is algorithm-shaped or has residual hardware
  provenance. Source: RFC 0010 "How this could be wrong." Status: **done**
  (2026-08-19) — both `accumulate_avx2` and `accumulate_avx512` produce
  exactly 32 results per call regardless of register width; 32 traces to
  the FastScan/PQ nibble-LUT's fixed 16-entry table doubled by hi/lo
  nibble packing, not to any ISA
  (`references/rabitq-library-fastscan-accumulate-source.md`).
  Algorithm-shaped, confirmed. Depends on: nothing.
- **M2-6** — `faster_quantize_ex`'s construction-time speedup
  (unregistered writer-side optimization). Source: RFC 0011 Non-goals.
  Status: **done** (2026-08-19) — `crates/strand-vector/src/quantize_ex.rs`
  gained `calibrate_rescale_factor`/`quantize_ex_fast`, confirmed
  writer-side-only and reader-invisible (already licensed by RFC 0011
  Design §3's byte-determinism carve-out, so no RFC amendment was needed),
  proved equivalent to the registered `quantize_ex` given the same `t`
  (bit-for-bit, not just plausibly), and measured at 23.0x faster at
  `dim = 768, ex_bits = 7` (2,973 µs/vector to 129 µs/vector, `cargo test
  --release`). Full writeup: `docs/ledger.md`, `rfcs/0011-multibit-
  extended-rabitq.md` Non-goals. Depends on: nothing.
- **M2-7** — ARM/SIMD kernel validation for the FastScan decode path
  (distinct from M1-3's `fst` determinism question and X-4's postings-
  specific ARM gap — this one is the vector family's own decode kernel).
  Source: RFC 0010 Non-goals. Status: open, verification work, same
  category as M1-3/M1-4. Depends on: nothing.
- **M2-8** — Cross-segment codebook-sharing policy and a cheap pre-merge
  compatibility check. Source: RFC 0010 Open questions ("so a writer is
  not surprised by an expensive `rebuild` only at merge time"). **A real
  scoping tension, not a one-sided call — stated both ways rather than
  resolved here**, per the adversarial review's own finding that this
  document's first draft argued only one side: RFC 0010's own text (Design
  §7, "How this could be wrong" item 4, Open questions) frames codebook
  identity as a *construction-side* concern most naturally settled
  alongside M2's own writer/clustering work (the codebook is fixed at
  write time, by the same writer that runs k-means) — arguing for keeping
  this under M2. Against that: the actual *cost* of getting it wrong (an
  expensive, surprise `rebuild`) is only paid at merge time, which is M3
  — arguing for treating it as an M3-1 design input instead. Both
  readings are defensible; this document does not adjudicate between
  them. Status: **blocked** — on M2's own remaining design bandwidth if
  kept under M2, or on M3-1's design work starting if re-homed there;
  either way it is not simply "open" today. M2-1 (above) is now done,
  which removes the "shares design attention with M2-1" reason for the
  M2 reading specifically — if kept under M2, this item is now genuinely
  open rather than blocked; the M3-1 reading is unaffected, since M3-1
  itself has not started. Depends on: M3-1 if re-homed there.

## M3 — Hybrid + deletes + merge

- **M3-1** — Compaction: per-family merge semantics (concatenate+remap
  for lexical/cluster-vector blobs, rebuild for graph blobs, rebalance
  for centroids), respecting `CLAUDE.md` §6's deletion-safety rule, merge
  cost benchmarked per strategy. Includes, as a named design sub-question
  RFC 0012's own Non-goals raises directly rather than leaving implicit:
  deletion-vector merge semantics specifically — a merged segment needs a
  freshly built deletion vector from the union of its source segments'
  surviving row-IDs, re-encoded against the new segment's own
  local-ordinal space (`spec/deletion.md` §2, `rfcs/0012` Non-goals).
  Source: `docs/milestones.md` M3 entry; invariant 1. Status: **blocked**
  on M3-2 and M3-3.

  **The gate's rationale, stated explicitly because a fair reading of
  precedent could ask why THIS extension is gated when the deletion-vector
  extension (`commit_deletion_vector`, "Recently shipped," above) was
  not** — confirmed as a real, worth-answering question by the
  adversarial review, not brushed aside: the deletion-vector extension
  closed its own model gap *in the same session*, before any other code
  was built on top of the unmodeled shape, at effectively zero elapsed
  risk. Compaction is categorically different — it is real, substantial,
  separately-scoped implementation work (per M3-2's own sizing note,
  below) that will take meaningfully longer to land, during which a wrong
  or incomplete model of a *merge* commit (the highest-consequence
  operation this protocol has — it can destroy data if guarded
  incorrectly) would sit unverified for that whole span. The gate is
  proportionate to the operation's own risk, not a blanket rule that every
  future protocol extension must be pre-verified before any of its code is
  written.
- **M3-2** — TLAPS mechanized proof of the TLA+ manifest model as it
  stands (`commit` + `commit_deletion_vector`). Source: RFC 0002's
  remaining artifact; `docs/milestones.md` M3 entry (gates M3-1). Status:
  open. Depends on: nothing (the model exists and is TLC-checked at 5,943
  states). **Honest sizing note**: the comparable case study RFC 0002
  itself cites (FMDSE) ran to a 1,282-line TLAPS proof for a
  similarly-scoped protocol. This is real, substantial, sequential
  proof-engineering work — not a task that benefits from parallel-agent
  decomposition the way independent research tasks do.
- **M3-3** — DST (Deterministic Simulation Testing) cross-validation
  harness, Workflow II first per RFC 0002 §2's approved sequencing
  (TLC-generated action sequences from the model, replayed against the
  real Rust `commit`/`commit_deletion_vector` code). Source: RFC 0002's
  remaining artifact; `docs/milestones.md` M3 entry (gates M3-1). Status:
  open. Depends on: nothing. Workflow I is explicitly sequenced after
  Workflow II succeeds — not part of this task.
- **M3-4** — Table metadata (`_strand/metadata.json`) and retention-
  policy-driven snapshot expiry. Source: `spec/manifest.md` §1 ("Not yet
  implemented... table-metadata-driven retention is M3 scope"), restated
  in RFC 0012 Non-goals as a named, inherited gap. **Missing from this
  document's first draft entirely** — found by the adversarial review.
  Status: open. Depends on: nothing technically, but M3-6/M3-7's orphan-
  adjacent accounting and the deletion-safety retention rule
  (`CLAUDE.md` §6) are more meaningful once real retention policy exists
  to enforce, so sequencing alongside M3-1 is sensible.
- **M3-5** — The orphan-sweep tool (`strand-tools`). Source:
  `docs/milestones.md` M3 entry; `spec/manifest.md`'s "Orphan files" rule,
  already stated, unimplemented. Status: open. Depends on: M3-4 (retention
  policy governs what "old enough to sweep" means); more valuable once
  M3-1 (compaction) exists to produce orphans at realistic volume.
- **M3-6** — End-to-end hybrid RRF fusion across both blob families over
  one row-ID space. Source: `docs/milestones.md` M3 entry — this is the
  project's actual thesis, exercised for the first time. **A real,
  previously-missing dependency, found by the adversarial review's
  dependency-correctness check**: the vector query path already resolves
  row-IDs directly (`crates/strand-vector/src/query.rs`'s `Candidate`
  carries a real `row_id`), but the lexical query path
  (`crates/strand-lexical/src/field.rs`) currently returns local ordinals
  from a single field's own dense arrays, not row-IDs — fusing the two
  result sets requires the lexical side to translate `local_ordinal +
  row_id_base` into the same row-ID space first, a real, small, currently
  unbuilt piece of glue, not just "call both and merge," Source for the
  gap: direct inspection of `field.rs`'s current return shape. Status:
  open. Depends on: the row-ID-translation glue just described (no
  separate task ID — small enough to fold into this task's own scope, but
  named here so it isn't silently assumed away). Sequencing after M3-1 is
  recommended, not required, for a realistic multi-segment corpus.
- **M3-7** — The multi-segment benchmark: the same corpus at 1, 16, and
  ~128 segments, cold and warm, producing a measured segment-count-
  amplification curve. Source: `docs/milestones.md` M3 entry; feeds R10.
  Status: **blocked** on M3-1 (compaction) for the realistic multi-segment
  corpus shape, though a *without-compaction* version (many small segments
  from repeated small commits, no merge) could run earlier as a partial
  measurement.
- **M3-8** — R10 resolution: should the manifest carry optional
  per-segment summary metadata (term-statistics sketches, centroid
  summaries, min/max pruning stats) for cross-segment query pruning?
  Source: `docs/ledger.md` R10. Status: **blocked** on M3-7.

## M4 — Interchange + independence

- **M4-1** — R11 adapter feasibility audit (gates all adapter work):
  (a) tantivy's reader surface + codec-SPI question, **and the exact
  Lucene codec SPI class surface for `StrandCodec`** (both halves of
  R11(a) — the roadmap's first draft dropped the Lucene half, which
  M4-6 below actually depends on); (b) FAISS per-kernel feasibility
  (`InvertedLists`/FastScan over external storage); (c) Quickwit
  split/hotcache internals post-relicense; (d) the fork reader-module
  list / fork failure triggers; (e) warm-tier graph host choice. Source:
  `docs/ledger.md` R11 ("gates all adapter work"). Status: **(a) and (c)
  done** (2026-08-19) — (a): tantivy has no codec SPI (`Directory` is a
  byte-range abstraction, `SegmentComponent` a closed enum); Lucene's
  `Codec`/`PostingsFormat` SPI is real and confirmed current, resolved via
  `ServiceLoader`
  (`references/r11a-tantivy-reader-surface-and-lucene-codec-spi.md`). (c):
  Quickwit is confirmed Apache-2.0 both byte-level and commit-level (PR
  #5645, 2025-01-23); the inherits-from-the-fork hypothesis's *mechanism*
  half is confirmed (ordinary `Directory`/`FileHandle` consumer, no
  tantivy-internals patch), its *code* half is not (Quickwit's split/
  hotcache wire format doesn't transfer)
  (`references/r11c-quickwit-relicense-and-hotcache-source.md`). (b), (d),
  (e) remain open — pure research/verification, no code dependency, and
  genuinely independent of each other and of (a)/(c). Depends on: (e)
  depends on M2-3 existing at least in RFC-draft form — the other
  sub-parts have no dependency.
- **M4-2** — CIFF importer (lossless where CIFF permits). Source:
  `docs/milestones.md` M4 entry. Status: open. Depends on: M4-1(a)/(c)
  (now done) informed exact scope, not strictly blocking a first pass.
- **M4-3** — Conformance manifest frozen at spec v0.1. Source:
  `docs/milestones.md` M4 entry. Status: **blocked** on every spec
  chapter that's still gaining golden files — practically, this should
  be the last M4 task, after M2/M3's work lands, since freezing before
  that means re-opening the freeze.
- **M4-4** — Second-reader independence: tantivy fork (primary path) or
  clean-room implementation (fallback, activates on an R11(d) failure
  trigger). Source: `docs/milestones.md` M4 entry. Status: **blocked** on
  M4-1(d) (M4-1(a) is now done); M1-5 (tantivy length-accounting grounding)
  is now resolved and no longer a blocker, though the fork still has to
  implement the patch M1-5 identified. **Scope
  correction, per the adversarial review**: this document's first draft
  stated the fork depends on the *full* v0.1 spec freeze (M4-3);
  precisely, it needs the freeze only for the spec chapters the fork
  actually reads — tantivy has no equivalent to STRAND's vector-blob
  family at all, so a lexical-only fork's dependency on M4-3 is real but
  narrower than "the whole spec" as originally stated. Recorded here
  rather than re-scoped outright, since the fork's own eventual scope
  (lexical-only vs. full-format) is itself still an open design choice.
- **M4-5** — Puffin blob-type packaging RFC. Source: `docs/milestones.md`
  M4 entry. Status: open, design work. Depends on: nothing.
- **M4-6** — Lucene `StrandCodec` (JVM parity vehicle). Source:
  `docs/milestones.md` M4 entry. Status: **unblocked on M4-1(a)**, which
  is now done (including its Lucene-codec-SPI half, above); still, like
  M4-4, most sensibly built against a frozen manifest (M4-3).

## M5 — The consumer

- **M5-1** — A thin, read-only DataFusion `TableProvider` over STRAND
  segments. Source: `docs/milestones.md` M5 entry. Status: open. Depends
  on: nothing structurally to *start* (can read lexical blobs as early
  slices) — full scope depends on M2/M3 blob families being stable.
- **M5-2** — The hybrid-fusion benchmark, run through the M5-1
  TableProvider, `CLAUDE.md` §7's fusion workload with its selectivity
  sweep. Source: `docs/milestones.md` M5 entry. Status: **blocked** on
  M5-1 and M3-6 (hybrid RRF must exist to benchmark).
- **M5-3** — FAISS adapter. Source: `docs/milestones.md` M5 entry, "per
  R11(b)'s feasibility finding." Status: **blocked** on M4-1(b).

## Cross-cutting grounding debt (not milestone-gating, real nonetheless)

- **X-1** — Multi-field blob addressing. **Missing from this document's
  first draft entirely — a real, load-bearing gap, found by the
  adversarial review.** A segment's flat blob registry has no field
  identifier and cannot disambiguate two fields' blobs sharing a
  `blob_type_id`; the real implementation is hard-limited to one field
  per segment today (`docs/ledger.md`). Source, precisely (the review's
  own verification corrected an over-attribution in an earlier pass of
  this finding): RFC 0008's Non-goals is the first to name the gap
  explicitly and asserts RFC 0005/0006/0007 each left it unsolved for
  their own blob types; RFC 0009's Non-goals repeats and inherits it
  directly ("Design §2's mutual-exclusivity rule... is therefore a real,
  correct requirement, but not yet a checkable one"). RFC 0005/0006/0007
  do not discuss the gap in their own text. Directly relevant to M5-1
  (a `TableProvider` cannot usefully read a multi-field index without
  it). Status: **done (2026-08-19)** — a `field_id: u64` field added to
  `spec/container.md` §5's blob registry entry (§5a), computed as
  `xxHash3-64` over a field's declared UTF-8 name (`field_id_from_name`,
  `crates/strand-core/src/container.rs`), with `0` reserved for "no field
  association." A reader now selects a blob by `(family_id, blob_type_id,
  field_id)`, not the first two alone. Design, the alternatives considered
  (a per-segment ordinal; an explicit name-catalog blob — both rejected)
  and their rejection reasons, the quantified collision-risk analysis, and
  a byte-exact worked example (two fields sharing one `blob_type_id`,
  disambiguated) live in `rfcs/0001-container-rowid-manifest.md`
  Discussion, the RFC that owns the blob registry structure; short pointer
  notes were added to RFC 0008's and RFC 0009's own Discussion sections
  since their Non-goals first named the gap. `crates/strand-lexical/
  src/field.rs`'s `build_field`/`build_field_without_positions`/
  `build_field_from_postings` now take a field name and
  `FieldReader::open`/`open_by_name` select by `field_id`, proven by a new
  end-to-end test
  (`two_fields_with_the_same_blob_type_ids_coexist_in_one_segment_and_stay_disambiguated`,
  `crates/strand-lexical/tests/field_end_to_end.rs`) building two real
  fields with different postings for a shared term and confirming neither
  reader ever resolves the other's blob. `conformance/container/
  toy-segment.bin` regenerated (34→42-byte `blob_entry`); a new golden
  file, `conformance/container/multi-field-segment.bin`, pins the
  two-field worked example. `cargo test --workspace` and `cargo clippy
  --workspace --all-targets -- -D warnings` both clean. Directly unblocks
  M5-1, not implemented here. Depends on: nothing technically — done.
- **X-2** — RFC 0001's three own remaining open items: the hotcache size
  ceiling / speculative tail-read default `N`'s actual value, the
  404-refresh retry bound's actual value (both currently "a reader
  parameter this chapter does not pin... but the bound itself is not
  optional" — meaning a value is owed, not just the existence of a
  bound), and empirical confirmation of the explicit-end vs. suffix-range
  request-form choice against real S3 behavior. **Missing from this
  document's first draft — found by the adversarial review.** Source:
  RFC 0001 Open questions. Status: **done (2026-08-19)** — all three
  resolved against real measurement, not guessed: `N`/hotcache ceiling via
  `bench/src/hotcache_tail_read.rs`, the retry bound via `bench/src/
  reader_refresh_contention.rs`, and suffix-range support confirmed for
  MinIO specifically (real S3 untested, no credentials available) via
  `crates/strand-core/tests/s3_store.rs`'s
  `suffix_range_get_is_honored_by_minio`. Full methodology and numbers:
  RFC 0001's Discussion section and `docs/ledger.md`'s X-2 entry. Depends
  on: nothing.
- **X-3** — The ~250ms p90 tail-latency figure: currently a flagged
  placeholder (`CLAUDE.md` §7). Replace with a real vendored source
  sentence or a real measured MinIO/S3 tail figure. Status: **done
  (2026-08-19)** — the "vendor a source sentence" half landed first: the AWS
  whitepaper "Best Practices Design Patterns: Optimizing Amazon S3
  Performance" states "consistent small object latencies... of roughly
  100–200 milliseconds" (`references/aws-s3-small-object-latency.md`),
  honestly labeled as not a named percentile. The "or a real measured
  MinIO/S3 tail figure" half is X-4's job, done the same day — see X-4's
  own entry below and `CLAUDE.md` §7 for the real measured p50/p90/p99.
  Depends on: nothing.
- **X-4** — Real-network cold-open tail latency (MinIO with injected
  latency, or real S3) — `bench/`'s existing cold-open measurement is
  against localhost MinIO with no injected latency, confirming the
  GET-count half of invariant 3 but not the real-network tail. Status:
  **done (2026-08-19)** — real S3 credentials are not available in this
  environment, so the substitute is a real MinIO container with a real
  `netem` delay injected onto its network interface via a throwaway
  `alpine` sidecar sharing its network namespace (`docker run --net
  container:<id> --cap-add NET_ADMIN`; the MinIO image itself has no
  package manager to install `tc` with, confirmed empirically before
  building the mechanism). `strand_bench::inject_netem_delay` and
  `with_minio_latency` (`bench/src/lib.rs`) and the new
  `cold-open-injected-latency` bench binary (`bench/src/
  cold_open_injected_latency.rs`) run the identical pointer/snapshot/
  segment open sequence `cold-open` runs, against MinIO with a 100ms
  one-way (egress-only, honestly not symmetric) delay chosen to target a
  ~100ms measured round trip per warm GET. Thirty real cold opens
  measured p50 = 344.2ms, p90 = 375.3ms, p99 = 489.8ms (min 326.7ms, max
  489.8ms; `bench/results/cold-open-injected-latency.json`) — inside RFC
  0001's own "~300–400ms" napkin-math prediction for this exact sequence
  at p50/p90, modestly above it at p99. Full numbers, the mechanism's
  honest asymmetry caveat, and what this does and does not confirm
  against the AWS SLO figure: `CLAUDE.md` §7 and `docs/ledger.md`.
  Depends on: nothing.
- **X-5** — The parallel-wave aggregate throughput measurement behind the
  100 MB cold-open budget rationale. Source: `CLAUDE.md` §7. **Status:
  done, resolved 2026-08-19.** `bench/src/parallel_range_fetch.rs`
  measured real N-way parallel byte-range GETs against real MinIO (via
  `strand-core`'s new `RangeGetStore` trait): one sequential stream over a
  real 100 MB object measured p50 = 50.6 MB/s; N-way parallel range GETs
  of the same object peaked at p50 = 159.7 MB/s at 32-way parallelism, a
  real 3.15x aggregate-throughput speedup — confirming the mechanism the
  budget depends on. Full numbers and honest caveats (the absolute "low
  hundreds of milliseconds" figure is not confirmed by a localhost run,
  and throughput did not scale monotonically past 32-way) are in
  `CLAUDE.md` §7 and `bench/results/parallel-range-fetch.json`. Depends
  on: nothing. The real-S3-network half of *this specific* 100 MB
  parallel-fetch figure is still open: X-4 (above) built and proved the
  injected-latency mechanism (`strand_bench::inject_netem_delay`,
  `bench/src/lib.rs`) and applied it to `cold_open.rs`'s sequence, not to
  `parallel_range_fetch.rs`'s own wave — a future session can point the
  same mechanism at that benchmark.
- **X-6** — R9's postings-block-size granularity conditional (ALP/GPU
  application-fit assessment, ARM/non-AVX2 hardware). Source: `docs/
  ledger.md` R9. **Corrected by the adversarial review**: the shipped,
  registered default is **256** (RFC 0007's `BitPacker8x`,
  `spec/postings.md` §3, `crates/strand-lexical/src/postings.rs`'s
  `BLOCK_LEN`), not the stale "128" figure `docs/ledger.md`'s own R9
  entry previously carried from before RFC 0007's approval — fixed there
  directly alongside this revision. **Granularity and ALP/GPU
  application-fit resolved 2026-08-19** (`docs/ledger.md` R9, live refetch
  of the ALP and DaMoN '24 papers): ALP is a floating-point-only codec
  with no applicability to postings' integer d-gaps/term-frequencies —
  finding redirected to a new note under R1 (flat vector blob, the actual
  floating-point storage in this project) rather than forced onto R9.
  DaMoN '24's GPU warp-granularity caveat is real but non-fatal (the
  paper's own mini-vector mitigation resolves it, measured) and is
  informational only for STRAND, which targets CPU SIMD (invariant 9), not
  GPU decode, in v0.1. The granularity question itself is refined to a
  grounded recommendation: keep 256 now; 1024-native (or any scheme
  approximating it) is gated on FastLanes actually being adopted as the
  default postings codec, which R9's own measurement does not currently
  support (FastLanes underperforms `BitPacker8x` on the only hardware
  tested). No spec or code change follows — 256 was already the default.
  **Status: ALP/GPU and granularity sub-items closed; ARM/non-AVX2
  hardware measurement remains open and out of scope (no ARM hardware or
  working emulation in this environment)**, low priority regardless (256
  remains the working default regardless of that outcome too). Depends
  on: nothing.
- **X-7** — R5, GCS/Azure conditional-write header semantics — both
  halves: the exact header semantics themselves, **and the external-
  catalog fallback protocol** for stores without native conditional
  writes (`spec/manifest.md` §1's own stated fallback shape) — the first
  draft of this item covered only the header-semantics half. Source:
  `docs/ledger.md`/RFC 0001. Status: open, but genuinely not actionable
  until a GCS or Azure backend is actually being built — no current
  milestone calls for one. Depends on: nothing; recommend leaving dormant
  until an actual GCS/Azure adapter is scoped.

## Also corrected by this revision, outside this document

The adversarial review's whole-project-consistency findings were fixed in
their own source files directly, not routed through this roadmap: stale
"FastLanes license unaudited" claims in `docs/research/README.md` and
`docs/lineage.md` (the license was resolved earlier this project, per
`CLAUDE.md` §1 and `docs/ledger.md` R9); a stale note in
`docs/data-structures.md` claiming `CLAUDE.md` §7's vector-sizing figure
still needed updating (it was updated at RFC 0010's Approval);
`CLAUDE.md` §8's repository-shape block, which omitted the committed
`verification/` directory and overclaimed "no separate full report
exists" under `docs/research/` (a second real methodology document,
`r2-hybrid-codec-methodology.md`, exists and is cited normatively by RFC
0007); a misaligned row in `spec/container.md` §9's blob-type registry
table; an empty, untracked `states/` scratch directory at the repo root
(removed); and `docs/superpowers/plans/2026-08-18-tla-manifest-model.md`,
marked completed in place rather than left with stale unchecked boxes.
