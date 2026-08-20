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

## M1 — Lexical (implemented; three real grounding/design gaps remain)

M1 shipped (BP128 postings, positions, FST term dictionary, block-max,
filter bitmaps, analyzer descriptors, RFC 0009's per-term overhead
reduction) — but four items its own approving RFCs left explicitly open
were missing from this document's first draft, found by this document's
own adversarial review. One of the four, M1-1, is now resolved as a
format-design decision (2026-08-19); its own resolution surfaced two new,
narrower implementation/verification tasks, M1-6 and M1-7, listed after
M1-5 below. Both are now done (2026-08-19): M1-6 implemented the default;
M1-7's Chinese-only bake-off confirmed it without overturning it — Japanese
and Thai remain untested and are the real residue of this thread, tracked
in M1-7's own entry, not a fresh open item.

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
  this RFC"). **Status: Approved (2026-08-20)**
  (`rfcs/0014-graph-blob-family.md`, `docs/ledger.md`) — a
  full new blob family, the same weight RFC 0010 itself carried for the
  cluster family, registering `family_id = 5` ("graph") with two blob
  types (graph node records; node-order permutation directory), grounded
  against the real, live-fetched DiskANN NeurIPS 2019 paper (Algorithms
  1–3, the on-disk layout, the billion-point experiment parameters —
  `references/diskann-neurips2019.md`, re-fetched in full this session:
  the earlier vendoring was abstract-only and insufficient to ground a
  wire-format RFC) and the real, live-fetched Starling SIGMOD 2024 paper
  (the NP-hardness theorem, the BNP/BNF/BNS block-shuffling algorithms
  with BNF's own pseudocode transcribed in full — `references/starling-
  sigmod2024.md`, likewise re-fetched in full). Resolves R1's node-order-
  permutation question with a real comparison, not a straw man: Starling's
  BNF is registered as the default, argued against a genuine, cited,
  unmeasured alternative (reusing an existing cluster-family k-means
  assignment as physical placement order) rather than only against Gorder
  (already ruled out on record, `docs/data-structures.md`). Confirms and
  details invariant 1's `rebuild` merge-strategy label for this family
  with three independent arguments (RobustPrune's global pruning property,
  DiskANN's own measured single-vs-merged latency finding, and
  IP-DiskANN's own framing of batch consolidation as the standard prior
  behavior). Finds, and states honestly rather than glossing over, a real
  v0.1 design cost: because the RFC defers an in-memory compressed-code
  cache (a real, named Non-goal, follow-on work), its query-resolution
  path must fetch every candidate node it discovers, not only nodes it
  expands, a real round-trip regression against DiskANN's own published
  figures, quantified in the RFC's own worked example (a 5-node graph
  trace: 2 real hops, 4 real fetches) and Napkin math (an honest
  worst-case bound reaching the tens of thousands of fetches at realistic
  `R` and hop counts). **Left Draft when first written, now Approved**
  following an independent adversarial review distinct from the drafting
  session, per `CLAUDE.md` §3 — every citation against both papers
  independently re-fetched and confirmed byte-accurate, the worked example
  independently reproduced from scratch, and one real arithmetic defect
  found and fixed: the pessimistic `10,000`-fetch warm-tier bound had been
  compared directly against `CLAUDE.md` §7's `5–20`-second cold figure as
  if the same fetch-count scale, when that figure calibrates a
  `50`–`200`-fetch pattern; corrected to the true cold-equivalent
  arithmetic for `10,000` fetches (≈`1,000` seconds), yielding a stronger,
  correctly computed ≈2.5–3-orders-of-magnitude gap rather than the
  erroneous "one to four" an earlier draft stated. Two further, non-
  blocking tightenings applied (a third node-order-permutation candidate
  named in Design §4; the Invariant-11 checklist's provenance argument
  reframed against invariant 11's actual text). Nothing about the core
  design needed to change. No crate code was written; the next step is
  implementation (real Vamana/BNF construction code, a `bench/`
  measurement replacing this RFC's own literature-translated arithmetic —
  both named in the RFC's own Open questions). Depends on: nothing
  technically, but sequencing it after M2-1/M2-2 was followed here so the
  cluster family's own remaining loose ends didn't compete for the same
  design attention.
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
  not surprised by an expensive `rebuild` only at merge time"). **The
  scoping tension this entry originally stated both ways rather than
  resolved — construction-side (M2, the codebook is fixed at write time)
  vs. merge-time (M3-1, the cost of getting it wrong is only paid at
  merge) — is resolved by doing the construction-side half now and
  naming the merge-time half precisely as still M3-1's, rather than by
  arguing either reading was wrong.** The half that does not depend on a
  real merge/compaction code path existing — a codebook **identity**
  mechanism and a **compatibility check function** a future merge planner
  can call — is real, self-contained, construction-adjacent work
  (`crates/strand-vector` already holds every type this touches), so it
  does not need to wait on M3-1 starting. The half that genuinely does
  depend on M3-1 — codebook-sharing *policy* (one codebook per table by
  convention vs. an explicit requantization path), cluster-assignment
  compatibility with a merged, rebalanced navigation tier (RFC 0010
  Design §7's own second `concatenate + remap` clause), and the actual
  merge-planner code that would call this check — still does, and is not
  claimed done here.

  **Status: done (2026-08-19) for the identity mechanism and compatibility
  check; open, and re-homed to M3-1, for the policy and merge-planner
  halves.** `crates/strand-vector/src/codebook.rs` adds `CodebookIdentity`
  (four scalar fields plus an XxHash3-64 content hash over the descriptor's
  own byte-identity criterion, RFC 0010 Design §7 — invariant 11's
  registered default checksum algorithm, reused rather than adding a
  second one, invariant 8) and `check_compatibility`/
  `check_descriptor_compatibility`, returning `Compatible` or
  `Incompatible(CodebookMismatch)` with the first disagreeing field named.
  No wire-format change: the identity is computed from the existing
  quantization-descriptor blob's bytes (Design §2, unchanged), not
  serialized as new bytes. Building one identity is `O(n)` in
  `rotation_payload`'s length (the same bytes a reader already fetches
  wholesale to use the codebook at all, invariant 7 — no new I/O);
  comparing two built identities is `O(1)`, touching neither segment's
  payload again — the concrete cheap-check mechanism this entry's Source
  citation asked for. Three pairs of real, independently-committed,
  footer/hotcache-decodable segments (`strand-core`'s actual
  `SegmentBuilder`) — one pair sharing one real codebook, one pair with
  independently-trained codebooks (same knobs, different RNG draw), and
  one pair with genuinely different `dims` — and the check is proven to
  distinguish all three cases correctly
  (`crates/strand-vector/tests/codebook_compatibility_across_segments.rs`).
  Full adversarial review (does it catch every real incompatibility,
  hash-collision risk, is the check itself cheap enough) in RFC 0010
  Discussion — post-approval amendments, below `docs/roadmap.md`'s own
  citation there. `cargo test --workspace` and `cargo clippy --workspace
  --all-targets -- -D warnings` both clean. Depends on: nothing —
  done. **M3-1 still owns**: the sharing/requantization policy decision,
  cluster-assignment compatibility, and wiring this check into a real
  merge-planning code path — M3-1's own entry, below, is unchanged by
  this resolution and should not be read as narrowed by it.

## M3 — Hybrid + deletes + merge

- **M3-1** — Compaction: per-family merge semantics (concatenate+remap
  for lexical/cluster-vector blobs, rebuild for graph blobs, rebalance
  for centroids), respecting `CLAUDE.md` §6's deletion-safety rule, merge
  cost benchmarked per strategy. Includes, as a named design sub-question
  RFC 0012's own Non-goals raises directly rather than leaving implicit:
  deletion-vector merge semantics specifically — a merged segment needs a
  freshly built deletion vector from the union of its source segments'
  surviving row-IDs, re-encoded against the new segment's own
  local-ordinal space (`spec/deletion.md` §2, `rfcs/0012` Non-goals). For
  the cluster-vector family specifically, M2-8 (above, done) already
  provides the codebook-identity/compatibility-check half of the
  `concatenate+remap`-vs-`rebuild` decision
  (`crates/strand-vector::codebook`) — M3-1 still owns calling it from a
  real merge planner, the codebook-sharing policy question, and cluster-
  assignment compatibility with a rebalanced navigation tier (RFC 0010
  Design §7, Open questions).
  Source: `docs/milestones.md` M3 entry; invariant 1. Status: **blocked**
  on M3-2 (M3-3, the DST harness, is done as of 2026-08-20 — see its own
  entry below).

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
  remaining artifact; `docs/milestones.md` M3 entry (gates M3-1). Depends
  on: nothing (the model exists and is TLC-checked at 5,943 states).
  **Honest sizing note**: the comparable case study RFC 0002 itself cites
  (FMDSE) ran to a 1,282-line TLAPS proof for a similarly-scoped protocol.
  This is real, substantial, sequential proof-engineering work — not a
  task that benefits from parallel-agent decomposition the way
  independent research tasks do.

  **Status: partially done (2026-08-20, obligation count corrected
  2026-08-20).** `verification/manifest_proofs.tla` (1,402 lines) proves
  `IndInv1` (`TypeOK` plus five safety properties) inductive across `Init`
  and all five **writer**-path actions (`ReadCurrent`, `ProposeSnapshot`,
  `ProposeDeletionVectorCommit`, `TryAdvancePointer`, `ResolveAmbiguity` —
  matching `commit()`'s and `commit_deletion_vector()`'s real control
  flow), confirmed by a clean `tlapm` run reporting `[INFO]: All 1261
  obligations proved.`, exit code 0, reproduced identically on two
  separate cache-cleared runs. A real toolchain fix (TLAPS 1.5.0 cannot
  process a `RECURSIVE`-operator definition; `SumCounts` rewritten to a
  recursive function, semantics-preserving, re-confirmed against TLC) was
  required first. **A first pass of this task reported 1,247 obligations
  from a run that did not, in fact, reproduce** — caught by an independent
  adversarial review that re-ran `tlapm` fresh and found one real failing
  obligation; fixed with a different proof strategy for that one step
  (`ExceptSegmentDelVer`, field-by-field `EXCEPT`-membership rather than a
  literal-record-equality detour that looked like it worked but was not
  reliable), independently re-verified twice before this entry was
  corrected — full account in `verification/README.md`'s "Lessons"
  section and RFC 0002's own Discussion addendum. **Not yet done**, named
  specifically and unaffected by the correction above: the reader-path
  actions (`ReadPointer`, `ReadSnapshotObject`); the `Next`-level
  composition and the temporal invariance theorem (`Spec => []IndInv1`);
  and the model's other six TLC-checked invariants, most notably
  `NoOverlappingRowIds` (needs a materially harder segment-packing
  inductive strengthening not yet attempted). Full accounting:
  `verification/README.md`'s "TLAPS proof" section, RFC 0002's Discussion
  section, `docs/ledger.md`. M3-2 is not yet complete, and the compaction
  gate below still needs the remainder of this item plus M3-3 (below, now
  done).
- **M3-3** — DST (Deterministic Simulation Testing) cross-validation
  harness, Workflow II first per RFC 0002 §2's approved sequencing
  (TLC-generated action sequences from the model, replayed against the
  real Rust `commit`/`commit_deletion_vector` code). Source: RFC 0002's
  remaining artifact; `docs/milestones.md` M3 entry (gates M3-1).
  **Status: done (2026-08-20).** Depends on: nothing. Workflow I is
  explicitly sequenced after Workflow II succeeds — not part of this task,
  and still unstarted.

  **Mechanism**: TLC's `-simulate num=N,file=PREFIX` real random-simulation
  trace-file output (`java -cp tla2tools.jar tlc2.TLC -help`'s actual flag
  list, checked live rather than assumed — `-dump` writes the whole
  reachable graph, not a sequence, and the heavier TLA+ Trace Validation
  tooling this RFC's own Open Questions flagged as possibly unnecessary is
  built for Workflow I's observe-and-reconcile shape, not this one). Each
  trace is a numbered sequence of full model states, not action labels;
  the harness reconstructs which action fired between consecutive states
  by diffing writer/reader process-counter variables, sound because
  `manifest.tla`'s `Next` never steps two processes at once and every
  `(from_pc, to_pc)` pair in RFC 0002 §4's grammar is unique.

  **Harness**: `crates/strand-core/src/bin/dst_manifest_harness/`
  (`trace.rs` parser, `replay.rs` replay engine, `main.rs` CLI/report), a
  new `dst-manifest-harness` binary target — outside `strand-core`'s own
  `#[cfg(test)]` boundary on purpose, so it only ever calls the same public
  `commit`/`commit_deletion_vector`/`read_snapshot`/`ConditionalStore` API
  an external consumer would. Resolves the granularity mismatch between
  §4's individually-firable model actions and the real `commit()`'s own
  monolithic retry loop by replaying one writer's whole trajectory as one
  real call, writers ordered by when each first reaches its own terminal
  process-counter value — real staleness (`PreconditionFailed`) then
  emerges from the real `InMemoryStore`'s own ETag comparison, never
  injected; only `Io`/`Ambiguous`/reader-side `Expired` are scripted.
  Comparison is at outcome level (final `Ok`/`Err`, version, segments),
  matching RFC 0002 §2's own "the real code's outcome matches what the
  spec predicted" phrasing. Full detail, including the ordering argument
  and its named scope limit (sub-cycle mid-flight rival interleaving
  beyond the `Ambiguous` axis is not reproduced), lives in `replay.rs`'s
  module doc and RFC 0002's Discussion section.

  **The run**: seed 20260819 (the harness's own default — a bare
  invocation reproduces it), `-workers 1`, 1,000 traces, `-depth 40`.
  1,000 trace files parsed clean, 8,396 total states. **3,000 writer
  trajectories replayed: 2,511 matched, 489 skipped (never reached a
  terminal pc within this trace's depth — reported, not a failure), 0
  drift. 1,000 reader trajectories replayed: 1,000 matched, 0 skipped, 0
  drift.** 2,335/3,000 writer and 501/1,000 reader trajectories required
  injecting at least one non-trivial fault outcome to reach their
  predicted terminal — most of the corpus exercised RFC 0002 §4's fault
  branches, not only the happy path. Determinism (a run is a seed, a seed
  is exactly replayable) confirmed directly: re-running the identical
  command line (seed 111, `-workers 1`, `num=100`, `-depth 25`) twice
  produced byte-identical trace files and an identical report.

  Two real bugs were found and fixed **in the harness itself** during
  construction — an off-by-one between TLA+'s 1-indexed sequence length
  and Rust's 0-indexed `Vec` in reader-replay seeding, and an overly eager
  skip in `DeleteWriter` replay — both caught by the harness's own
  skip-reason counts moving in ways the trace content didn't justify,
  neither a manifest protocol bug; full account in RFC 0002's Discussion
  section, which also states plainly that automatic drift classification
  into RFC 0002 §3's four types is not implemented (the harness reports
  human-readable mismatch detail for a human to classify by hand) and that
  zero drift in this run is real, positive evidence for the trace
  vocabulary's correspondence to the real code — not proof the model is
  complete, and not proof Workflow I will succeed once attempted.
- **M3-4** — Table metadata (`_strand/metadata.json`) and retention-
  policy-driven snapshot expiry. Source: `spec/manifest.md` §1 ("Not yet
  implemented... table-metadata-driven retention is M3 scope"), restated
  in RFC 0012 Non-goals as a named, inherited gap. **Missing from this
  document's first draft entirely** — found by the adversarial review.
  **Status: done (2026-08-19).** `crates/strand-core/src/table_metadata.rs`:
  `TableMetadata`/`CasHost`/`RetentionPolicy` (the exact shapes RFC 0001
  Design §3 already named), `write_table_metadata`/`read_table_metadata`
  (write-once `put_if_absent` to `_strand/metadata.json`, outside the
  `_strand/current` CAS protocol entirely — no new manifest commit action,
  `verification/manifest.tla` unchanged), and `retained_snapshots`, the
  pure retention-eligibility function M3-5 depends on. Implementing this
  surfaced a real spec gap — `spec/manifest.md` §1 named "a count, a
  duration, or both" but never said how both combine, and `SnapshotMetadata`
  had no timestamp field for a duration policy to compare against — both
  resolved properly through RFC 0001's Discussion section (2026-08-19,
  `committed_at_millis` added additively, both-set retention resolved as a
  union, current snapshot always retained), not decided silently; see that
  RFC's Discussion entry and `spec/manifest.md` §1 for the normative
  result. 16 new tests (round-trips, the two `CasHost` JSON shapes checked
  byte-for-byte, and `retained_snapshots` against a snapshot inside/outside
  the duration window, the inclusive/exclusive boundary, the count-only
  floor, the union case, and the always-retain-current floor), all
  `InMemoryStore`-backed — real-MinIO coverage for this object remains
  open, unlike the CAS protocol's own `tests/s3_store.rs` coverage.
- **M3-5** — The orphan-sweep tool (`strand-tools`). Source:
  `docs/milestones.md` M3 entry; `spec/manifest.md`'s "Orphan files" rule,
  already stated, now implemented. Status: **done, 2026-08-19**
  (`crates/strand-tools/src/orphan_sweep.rs`'s `sweep_orphans`, the
  `strand-tools sweep` CLI subcommand). Implementing this surfaced a real
  gap the stated rule left open — "the retention window" named as if
  already defined, but nothing said what it was, or how it related to
  `RetentionPolicy` — resolved through RFC 0001's Discussion section
  (2026-08-19) rather than picked silently: the orphan retention window is
  its own sweep-time parameter (`--retention-window-secs`, defaulting to
  Apache Iceberg's own `remove_orphan_files` default of 3 days,
  `references/iceberg-remove-orphan-files-procedure.md`), never a
  `TableMetadata` field, since it protects something `RetentionPolicy`
  doesn't (an in-flight writer's not-yet-pointed-at objects, not snapshot
  eligibility). `crates/strand-core/src/store.rs` gained the `ListableStore`/
  `DeletableStore` traits (`S3Store` implements both — real `ListObjectsV2`
  pagination and object deletion) the sweep needs beyond
  `ConditionalStore`/`RangeGetStore`. Verified against real MinIO
  (matching this layer's own verification bar): a crashed-writer-orphan
  sweep (the same pattern `tests/s3_store.rs`'s
  `orphaned_writer_crash_is_harmless_to_readers` established, now for the
  sweep instead of a reader) and the retention-window safety margin (a
  young, unreferenced orphan survives). Still more valuable at realistic
  volume once M3-1 (compaction) lands.
- **M3-6** — End-to-end hybrid RRF fusion across both blob families over
  one row-ID space. Source: `docs/milestones.md` M3 entry — this is the
  project's actual thesis, exercised for the first time. Status: **done,
  2026-08-20**.

  **The gap this entry itself named is closed.** The vector query path
  already resolved row-IDs directly (`crates/strand-vector/src/query.rs`'s
  `Candidate` carries a real `row_id`); the lexical query path
  (`crates/strand-lexical/src/field.rs`) returned only local doc ordinals.
  `FieldReader::search_bm25_row_ids` now translates `row_id_base +
  doc_ordinal` — `spec/row-ids.md` §1's own normative arithmetic, the same
  translation `crates/strand-datafusion/src/lexical_table.rs` (M5-1)
  already performs for its own purpose — into real row-IDs, so no RFC was
  owed for it, per that entry's own precedent. Verified in
  `crates/strand-lexical/tests/field_end_to_end.rs` against a real
  segment's hotcache-declared `row_id_base`, not a hardcoded one.

  **New `crates/strand-core/src/fusion.rs`**: Reciprocal Rank Fusion,
  deliberately crate-level glue rather than a spec chapter (`CLAUDE.md`
  §1) and deliberately independent of both `strand-lexical` and
  `strand-vector` — it operates only on `u64` row-IDs and rank positions,
  which is all invariant 1's row-ID space actually requires two blob
  families to agree on. The formula and the `k = 60` constant are taken
  from the paper that defines RRF (Cormack, Clarke & Büttcher, "Reciprocal
  Rank Fusion outperforms Condorcet and individual Rank Learning Methods,"
  SIGIR'09), fetched from the authors' own institutional host and vendored
  rather than trusted from memory (`CLAUDE.md` §3):
  `references/cormack-clarke-buettcher-2009-reciprocal-rank-fusion.md`.
  `DEFAULT_RRF_K` is stated as the paper's own fixed constant, not an
  independently-tuned one — the vendored reference records that the
  paper's own pilot sweep's actual MAP peak was at `k = 80`, with `k = 60`
  close behind on a wide plateau from roughly `k = 30` to `k = 100`,
  matching the paper's own "near-optimal... but not critical" language
  rather than presenting `60` as uniquely best.

  **End-to-end proof**: `crates/strand-core/tests/hybrid_rrf_end_to_end.rs`
  builds one real segment (composing `field_end_to_end.rs` and
  `segment_assembly.rs`'s own closest existing patterns, not reinventing
  either) carrying both a lexical field and a vector field over one shared
  row-ID space — one `SegmentBuilder`, one commit, `row_id_count = 6` —
  runs a real BM25 query against the lexical side and a real ANN query
  (nprobe scan + exact rerank against the flat-vector blob) against the
  vector side, and fuses the two real, row-ID-resolved result lists with
  `fusion::fuse`. The fused order and every score are asserted against a
  hand-computed expectation — the paper's formula applied directly, in the
  test file itself, to the ranks the real queries actually produced,
  independent of `fuse`'s own implementation — at the same worked-example
  rigor `docs/ledger.md` already holds M2-1's SPANN closure-replication
  test and M5-1's SQL end-to-end test to. The worked example is
  constructed so the actual thesis is visible in the assertions, not just
  implied: two documents (row 0, row 1) that are each other's lexical
  matches but only middling vector matches (vector ranks 3 and 5) outrank
  the vector ranking's own single nearest neighbor (row 3) once both
  signals are fused — fusion changes the answer, not just its
  presentation. (This same test-writing caught a real arithmetic error in
  an earlier draft of `fusion.rs`'s own unit tests — a claimed "symmetry"
  between two documents in a hand-computed three-document example that
  does not actually hold for a cyclic rank permutation — fixed by
  recomputing the real values rather than softening the claim, per
  `CLAUDE.md` §2.)

  `crates/strand-core/Cargo.toml` gained `strand-lexical` and
  `strand-vector` as dev-dependencies — a supported Cargo dev-dependency
  cycle, since both already depend on `strand-core` as a normal
  dependency — the only place in the project that needs both blob-family
  crates together to prove the shared row-ID space actually works.

  `cargo test --workspace` and `cargo clippy --workspace --all-targets --
  -D warnings` both clean. Depends on: nothing further. Sequencing after
  M3-1 remains a real, not-yet-done recommendation for exercising this at
  realistic multi-segment scale — this task proves the mechanism on one
  segment; the multi-segment amplification curve stays M3-7's job.
- **M3-7** — The multi-segment benchmark: the same corpus at 1, 16, and
  ~128 segments, cold and warm, producing a measured segment-count-
  amplification curve. Source: `docs/milestones.md` M3 entry; feeds R10.
  Status: the **full** version — realistic multi-segment shape reached via
  commits *and* merges — remains **blocked** on M3-1 (compaction, not yet
  built). The **without-compaction partial version** this entry's own text
  named as runnable earlier is **done, 2026-08-20**
  (`bench/src/multi_segment_query.rs`,
  `bench/results/multi-segment-query-partial.json`): the same 12,800-document
  corpus, committed as 1, 16, and 128 independent segments via repeated
  small `manifest::commit` calls with no merge step (compaction genuinely
  does not exist yet), queried cold and warm against real MinIO. Real,
  measured result: cold GETs were exactly `2 + segment_count` at every
  point — 3, 18, 130 — confirming `CLAUDE.md` §7's O(segments) model exactly
  on this corpus; cold bytes fetched grew too (643,158 → 713,164 →
  1,291,500 bytes, ~2.0x from 1 to 128 segments, on the *same* total
  document count) from each additional segment's fixed per-segment
  overhead; cold latency grew but not proportionally to GET count at every
  step (1→16 segments: 6x the GETs, 1.5x the p50 latency; 16→128: 7.2x the
  GETs, 9.6x the p50 latency), and warm-cached (zero-GET) latency did not
  grow monotonically at all (44.8ms → 11.8ms → 57.2ms p50) — both reported
  as measured, not smoothed, per `CLAUDE.md` §2's rule for this project's
  own numbers, not only vendored ones. This partial run does not close
  M3-7: the full, post-compaction version — which would additionally show
  compaction's effect on the curve — remains open, blocked on M3-1.
- **M3-8** — R10 resolution: should the manifest carry optional
  per-segment summary metadata (term-statistics sketches, centroid
  summaries, min/max pruning stats) for cross-segment query pruning?
  Source: `docs/ledger.md` R10. Status: **blocked** on M3-7's **full**
  version — the without-compaction partial measurement above confirms the
  cost problem R10 exists to address is real, but says nothing about how
  compaction changes the curve, so it does not unblock this item.

## M4 — Interchange + independence

- **M4-1** — R11 adapter feasibility audit (gates all adapter work):
  (a) tantivy's reader surface + codec-SPI question, **and the exact
  Lucene codec SPI class surface for `StrandCodec`** (both halves of
  R11(a) — the roadmap's first draft dropped the Lucene half, which
  M4-6 below actually depends on); (b) FAISS per-kernel feasibility
  (`InvertedLists`/FastScan over external storage); (c) Quickwit
  split/hotcache internals post-relicense; (d) the fork reader-module
  list / fork failure triggers; (e) warm-tier graph host choice. Source:
  `docs/ledger.md` R11 ("gates all adapter work"). Status: **(a), (b), (c),
  and (d) done** (2026-08-19) — (a): tantivy has no codec SPI (`Directory` is a
  byte-range abstraction, `SegmentComponent` a closed enum); Lucene's
  `Codec`/`PostingsFormat` SPI is real and confirmed current, resolved via
  `ServiceLoader`
  (`references/r11a-tantivy-reader-surface-and-lucene-codec-spi.md`). (b):
  a STRAND-backed `InvertedLists` subclass (deriving from FAISS's own
  `ReadOnlyInvertedLists`) fully serves plain `IndexIVFRaBitQ` search over
  external storage — confirmed by reading every call site in
  `IndexIVF.cpp`'s generic `search_preassigned`, zero `dynamic_cast` to any
  concrete type. FastScan search is equally generic (only two
  `dynamic_cast<BlockInvertedLists>` sites in `IndexIVFFastScan.cpp`, both
  in the *write* path, none in `search_implem_*`), so a custom
  `InvertedLists` returning already-block-packed bytes is read correctly at
  query time with no FAISS-side change; building those packed bytes still
  requires a literal `BlockInvertedLists` (`add_with_ids` throws "only
  block inverted lists supported" otherwise), so every STRAND→FastScan path
  needs a repack, quantified at `O(ntotal · d)` bit work plus one
  `CodePacker::pack_1` call per vector via FAISS's own conversion
  constructor, paid once per segment open, not per query — no FAISS fork
  required for either kernel
  (`references/r11b-faiss-invertedlists-external-storage-feasibility.md`).
  (c): Quickwit is confirmed Apache-2.0 both byte-level and commit-level (PR
  #5645, 2025-01-23); the inherits-from-the-fork hypothesis's *mechanism*
  half is confirmed (ordinary `Directory`/`FileHandle` consumer, no
  tantivy-internals patch), its *code* half is not (Quickwit's split/
  hotcache wire format doesn't transfer)
  (`references/r11c-quickwit-relicense-and-hotcache-source.md`). (d): the
  Layer-2 reader-module list is pinned — thirteen files across segment-open
  orchestration, postings decode, positions decode, the term dictionary, and
  field norms, plus one new `Directory` impl for Layer-1 file virtualization
  (no tantivy-internals patch needed there) — arming `docs/benchmarks.md`'s
  scope-leak failure trigger; two real mismatches (postings block
  granularity, 128 vs. STRAND's 256; tantivy's default read path already
  uses a BM25 block-pruning bound, block-max-WAND, that STRAND's own
  postings blob does not register yet, RFC 0007 deferring it as future
  work) and real recent module churn (a 44-file upstream refactor,
  PR #2993, merged 9 days before this grounding) ground the other two
  triggers
  (`references/r11d-tantivy-fork-reader-module-list-and-failure-
  triggers.md`). (e) remains open — the warm-tier graph host choice, pure
  research/verification, no code dependency. Depends on: (e) depends on
  M2-3 existing at least in RFC-draft form — the other sub-parts have no
  dependency.
- **M4-2** — CIFF importer (lossless where CIFF permits). Source:
  `docs/milestones.md` M4 entry. Status: **done** (2026-08-20,
  `docs/ledger.md`'s CIFF importer entry). `crates/strand-tools/src/
  ciff.rs`'s `import_ciff` follows the tantivy importer's own pattern
  (`convert.rs`) end to end: a real, live-fetched CIFF `.proto` schema
  (`references/ciff-common-index-file-format.md`), hand-written
  `prost::Message` structs, and the same `build_field_from_postings`/
  `SegmentBuilder` path every importer uses. Per-term document frequency
  and per-document length round-trip losslessly, cross-checked against
  CIFF's own integrity totals; positions and external document IDs are
  named, honest gaps (CIFF carries neither). Depends on: M4-1(a)/(c)
  (now done) informed exact scope, not strictly blocking a first pass.
- **M4-3** — Conformance manifest frozen at spec v0.1. Source:
  `docs/milestones.md` M4 entry. Status: **blocked** on every spec
  chapter that's still gaining golden files — practically, this should
  be the last M4 task, after M2/M3's work lands, since freezing before
  that means re-opening the freeze.
- **M4-4** — Second-reader independence: tantivy fork (primary path) or
  clean-room implementation (fallback, activates on an R11(d) failure
  trigger). Source: `docs/milestones.md` M4 entry. Status: **unblocked on
  M4-1(d)**, which is now done (2026-08-19,
  `references/r11d-tantivy-fork-reader-module-list-and-failure-triggers.md`)
  — the reader-module list and the grounding for all three
  `docs/benchmarks.md` failure triggers exist; M1-5 (tantivy
  length-accounting grounding) is likewise resolved and no longer a
  blocker, though the fork still has to implement the patch M1-5
  identified. Structurally ready to start; still most sensibly built
  against a frozen manifest (M4-3, below), and this task itself does not
  start the fork — that is separate, larger work. **Scope
  correction, per the adversarial review**: this document's first draft
  stated the fork depends on the *full* v0.1 spec freeze (M4-3);
  precisely, it needs the freeze only for the spec chapters the fork
  actually reads — tantivy has no equivalent to STRAND's vector-blob
  family at all, so a lexical-only fork's dependency on M4-3 is real but
  narrower than "the whole spec" as originally stated. Recorded here
  rather than re-scoped outright, since the fork's own eventual scope
  (lexical-only vs. full-format) is itself still an open design choice.
- **M4-5** — Puffin blob-type packaging RFC. Source: `docs/milestones.md`
  M4 entry. **Status: Implemented (2026-08-20)**
  (`rfcs/0013-puffin-export-sidecar.md`, `spec/puffin-export.md`,
  `docs/ledger.md`) — a one-way, on-demand STRAND → Puffin export sidecar
  (deletion vectors translated byte-exact into Puffin's own registered
  `deletion-vector-v1` type; every other blob family passed through
  opaquely under one STRAND-namespaced type), grounded against the real,
  fetched Puffin v1 spec and the real `apache/iceberg-rust` crate
  (`references/puffin-spec-and-iceberg-rust-implementation.md`), with a
  container-profile alternative (STRAND's own segment format redefined
  around Puffin's shape) considered and rejected in the RFC's own Design
  §1. Left Draft deliberately when first written — the RFC's own original
  Status bullet argued this design (a second wire format, a second
  checksum algorithm, for narrow, unproven interop value) needed a genuine
  independent adversarial review before Approval, not a self-declaration
  by the same session that drafted it, per `CLAUDE.md` §3. That review
  happened first: every external citation independently re-fetched and
  confirmed accurate, the worked example independently reproduced
  byte-for-byte, and a verdict of "approve with minor fixes" — a missing
  sidecar staleness/invalidation disclaimer and a missing integrity-
  checksum property on the opaque-passthrough blob type, both fixed in
  place, no change to the core design. A separate implementation session
  then wrote the spec chapter, real code
  (`crates/strand-tools/src/puffin_export.rs`, a new
  `strand-tools export-puffin` CLI verb), and a byte-exact conformance test
  — see `docs/ledger.md` for the full writeup. Depends on: nothing
  further; followed `spec/deletion.md` (already landed, M2) and needed no
  other M4 item first.
- **M4-6** — Lucene `StrandCodec` (JVM parity vehicle). Source:
  `docs/milestones.md` M4 entry. Status: **unblocked on M4-1(a)**, which
  is now done (including its Lucene-codec-SPI half, above); still, like
  M4-4, most sensibly built against a frozen manifest (M4-3).

## M5 — The consumer

- **M5-1** — A thin, read-only DataFusion `TableProvider` over STRAND
  segments. Source: `docs/milestones.md` M5 entry. Status: **done, lexical
  slice (2026-08-20)** — this task's own scope was always "can read
  lexical blobs as early slices," and that slice is what shipped: a new
  crate, `crates/strand-datafusion`, whose `StrandLexicalTable` opens one
  field of one resident segment (footer → hotcache → the same
  `FieldReader::open_by_name` cold-open path `field_end_to_end.rs` already
  proved) and exposes `(row_id, term, term_freq)` as a real Apache
  DataFusion table — `row_id = hotcache.row_id_base + doc_ordinal`, a
  direct reading of `spec/row-ids.md` §1, not a new design decision.
  Proven against a real `datafusion::prelude::SessionContext` running real
  SQL, not a mock: an equality-filter scan, a `GROUP BY`/`COUNT`
  aggregate reproducing hand-computed document frequencies, and a
  clean-error case for an unbuilt field name
  (`crates/strand-datafusion/tests/lexical_table_sql.rs`). DataFusion's
  `TableProvider` trait and the `MemorySourceConfig`/`DataSourceExec`
  scan pattern were fetched live from `apache/datafusion` tag `55.0.0`
  rather than implemented from memory (`CLAUDE.md` §3) — full detail,
  including a WebFetch summary of docs.rs that had to be corrected against
  the real source, in `docs/ledger.md`. Full remaining M5 scope, named
  honestly rather than dropped: the vector family
  (`crates/strand-vector`), multi-segment tables (mechanical given
  `spec/row-ids.md` §1's disjoint ranges, but unbuilt), deletion-vector
  filtering, and multi-field tables — all listed in
  `crates/strand-datafusion/src/lib.rs`'s and `lexical_table.rs`'s own doc
  comments, not silently scoped away. Depends on: nothing further for this
  slice; the remaining scope depends on nothing new technically (M2/M3
  blob families it would read are already shipped) but is unbuilt work.
- **M5-2** — The hybrid-fusion benchmark, run through the M5-1
  TableProvider, `CLAUDE.md` §7's fusion workload with its selectivity
  sweep. Source: `docs/milestones.md` M5 entry. Status: **blocked** on
  M5-1's still-open vector-family slice — a *hybrid*-fusion benchmark
  needs both blob families queryable through the TableProvider, and only
  the lexical slice exists today (M5-1 above). M3-6 itself is no longer a
  blocker: hybrid RRF fusion exists and is proven end-to-end (done,
  2026-08-20), just not yet reachable through the TableProvider this task
  would benchmark against.
- **M5-3** — FAISS adapter. Source: `docs/milestones.md` M5 entry, "per
  R11(b)'s feasibility finding." Status: **unblocked on M4-1(b)**, now
  resolved (`references/r11b-faiss-invertedlists-external-storage-feasibility.md`):
  a STRAND-backed `InvertedLists` subclass serves both `IndexIVFRaBitQ` and
  `IndexIVFRaBitQFastScan` search with no FAISS fork; the FastScan leg needs
  a one-time per-segment-open repack into `BlockInvertedLists`, whose cost
  is now quantified. Still most sensibly built against a frozen manifest
  (M4-3) and an RFC of its own (the finding settles feasibility and design
  shape, not the RFC text).

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
