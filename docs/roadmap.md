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
  this RFC"). **Status: Approved (2026-08-20); implementation done, all
  four slices (2026-08-20)** — the four-slice account below (construction,
  node-order permutation, wire format, cold-open query path) is the full
  RFC 0014 v0.1 implementation. One real, named gap survives completion,
  not silently closed: `spec/graph-vectors.md` — the normative chapter
  slice 3's own report already flagged as genuinely unwritten — still does
  not exist; every byte layout and algorithm this family registers lives
  only in the RFC and in the crate code's own doc comments, not in a spec
  chapter a second implementation could build against without reading
  Rust. This entry does not claim "spec chapter written," only "reference
  implementation complete."
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

  **Implementation, slice 1 of 4 — Vamana graph construction, pure
  in-memory, no wire format — done (2026-08-20).** `crates/strand-vector/
  src/vamana.rs`: `greedy_search` (Algorithm 1), `robust_prune` (Algorithm
  2), and `build_vamana` (Algorithm 3's random-init-then-two-pass loop),
  transcribed against `references/diskann-neurips2019.md`'s own vendored
  pseudocode, deliberately scoped to point array indices `0..n` only — no
  `SegmentBuilder`, no blob, no query path; those are named, separate,
  later tasks in this same implementation sequence (the node-order
  permutation/BNF algorithm, the wire-format blobs of Design §§2–3, and
  the `GreedySearch`/`BeamSearch` query path of Design §5). Two real,
  named findings, not silently absorbed: **first**, the vendored
  reference's own Algorithm 2 pseudocode line reads, verbatim, `p* ←
  argmin_{p' ∈ V} d(p*, p')` — self-referential as written, since `p*` is
  the value being defined — corrected in the implementation (and in the
  module's own doc comment, argued from the algorithm's very next line,
  which only type-checks if the candidate is chosen by distance to `p`,
  the fixed point being pruned) to the well-known published form, per
  `CLAUDE.md` §3's instruction to say so rather than silently patch it.
  **Second**, `RobustPrune`'s `α`-pruning condition (`α · d(p*, p') ≤ d(p,
  p')`) is a linear inequality over true distance, not a bare comparison,
  so this crate's dominant squared-Euclidean convention (`crate::kmeans`,
  `crate::closure`) required squaring `α` too (`α² · d²(p*,p') ≤ d²(p,p')`)
  at that one comparison site to stay equivalent — a real, subtle
  correctness trap a squared-distance shortcut can introduce that a plain
  nearest-neighbor `argmin` cannot, named explicitly in the module's own
  documentation rather than left implicit. The paper's own undefined
  "medoid" computation is resolved as an exact `O(n·dims)` computation (the
  real dataset point nearest the coordinate-wise mean, provably identical
  to minimizing the sum of squared distances to every other point via the
  standard variance-decomposition identity), documented as an
  interpretation choice in the same style `crate::closure`'s own module
  documentation already established for SPANN's epsilon-ratio ambiguity. A
  third, smaller bug found and fixed during testing, not merely reported:
  `robust_prune`'s internal candidate set was originally a `HashSet<usize>`,
  whose iteration order (and therefore `min_by`'s own tie-break) is
  randomized per process by Rust's default hasher, independent of the
  caller's RNG seed — silently non-deterministic exactly at the equidistant
  ties the RFC's own worked example exercises. Switched to `BTreeSet`
  (ascending-index iteration, deterministic ties), confirmed by rerunning
  the affected test three times.

  **Worked-example reproduction: half matched exactly, half honestly
  diverged with cause traced to ground.** RFC 0014's own 5-node query
  trace (`dims=2`, `R=2`, entry point `A`, query `(0.9,0.1)`, `L=2`) is
  reproduced byte-for-byte by `greedy_search` run directly over the RFC's
  own stated illustrative adjacency list: result `B`, 2 real hops (`A`,
  `B` expanded), 4 real fetches (`A`, `B`, `C`, `D` — `C` and `D` fetched
  only to be trimmed away), matching the RFC's own "2 hops, 4 fetches"
  arithmetic exactly. Running the real `build_vamana` construction
  algorithm over the same 5 points, by contrast, does not reproduce the
  RFC's own illustrative topology — expected and stated in the RFC's own
  text ("a plausible Vamana output for hand-checkability, not a
  hand-execution of Algorithm 3") — so the implementation instead asserts
  degree `≤ R` and a correct nearest-neighbor result across 500 seeds, both
  of which hold. Full reachability from the entry point does not hold at
  this exact tiny scale: the same 500-seed loop found the outlier
  `E=(2,2)` deterministically unreachable in 0/500 runs, traced to real
  geometry, not flakiness or a bug — `E` is farther from every other point
  than those points are from each other, so `A`/`B`/`C`/`D`'s own true
  two nearest neighbors always exclude `E`, and `R=2` leaves no spare slot
  once the geometrically closer pair is found (the pairwise distances are
  irrational and pairwise-distinct, so no tie is available for `E` to win).
  Documented in the test itself as a real consequence of `R=2` being the
  RFC's own hand-checkability choice, not a realistic construction
  parameter (DiskANN's own real figures are `R=64`–`128`); full
  reachability is instead verified at a more realistic scale (below).

  **Larger-scale property tests (`n=40`, `dims=4`, `R=6`, 10 seeds; a
  separate `n=50`, `dims=8`, `R=8` recall test).** Degree bound and full
  entry-point reachability both held in every one of the 10 sampled seeds
  at `n=40`/`R=6` — the realistic-scale confirmation the tiny worked
  example's own `R=2` cannot give. Recall@10 against brute-force ground
  truth, measured (not assumed) over 50 random queries against a 50-point
  graph: **1.0** (every query's true top-10 fully recovered) at the exact
  seed the test pins; the test's own asserted floor is set well below that
  observed value (`0.6`) so the test pins a real, measured lower bound
  rather than an unverified hoped-for one, per this task's own instruction
  not to assert an unverified threshold.

  Verification: `cargo check --workspace --all-targets`, `cargo clippy
  --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and
  `cargo test --workspace` all clean (10 new tests in
  `crates/strand-vector/src/vamana.rs`, 0 failures workspace-wide).
  Depends on: nothing further for this slice; the node-order-permutation
  (BNF) algorithm, the wire-format blobs (Design §§2–3), and the query
  path (Design §5) are separate, later slices in this same sequence.

  **Implementation, slice 2 of 4 — Starling's node-order-permutation
  algorithms, pure in-memory, no wire format — done (2026-08-20).**
  `crates/strand-vector/src/reorder.rs`: the overlap-ratio locality metric
  `OR(G)` (Definition 1/Eq. 5), BNP (Algorithm I), and BNF (Algorithm II,
  the RFC's own registered default), transcribed against
  `references/starling-sigmod2024.md`'s own vendored pseudocode — no wire
  format, no `SegmentBuilder`; wire-format blobs (Design §§2–3) and the
  query path (Design §5) remain separate, later slices. **BNS (Algorithm
  III) is explicitly not implemented**: the vendored reference's own BNS
  section is prose, not pseudocode, and names no concrete candidate-pair
  selection rule, iteration order, or termination condition beyond a
  stated complexity class — inventing a mechanism that merely produces the
  right asymptotic shape would be presenting a guess as Starling's own
  algorithm, exactly what this task's own brief and `CLAUDE.md` §3
  forbid. BNP and BNF are this RFC's load-bearing pieces (the simple
  baseline and the registered default); BNS's absence is a real, named
  gap, not a shortfall against this task's actual requirement.

  Three documented interpretation choices for real ambiguities the
  vendored BNF pseudocode leaves open, named rather than silently
  resolved: tie-breaking toward the lower block ID when two candidates
  share the same neighbor frequency; the "add `u` to an empty block"
  fallback read as "the first block with any spare capacity," since
  nothing in the algorithm's own text guarantees a literally empty block
  still exists whenever that branch fires; and — found only by running
  this module's own literal transcription against a real
  `build_vamana`-constructed graph, not anticipated going in — a single
  BNF iteration can measurably *decrease* `OR(G)` below its own BNP
  starting point (a real, paper-acknowledged possibility: "BNF's
  efficiency is contingent on the number of iterations and does not
  ensure the convergence of `OR(G)`," unlike BNS, which Lemma 4.2 proves
  monotonic), so `bnf(...)` tracks the best `OR(G)` layout seen across
  BNP's own starting point and every iteration and returns that one,
  making its own result a guaranteed lower bound of `bnp(...)`'s on every
  input rather than a probabilistic "usually better" — every intermediate
  per-iteration transition still follows Algorithm 1's own rule exactly
  and unmodified, extracted into its own function
  (`bnf_one_iteration`) precisely so the raw mechanism stays independently
  hand-checkable, separately from the best-seen wrapper around it.

  **Required comparative proof, independently re-verified after a real
  documentation error was caught and fixed (see the Addendum below).** On
  a real `build_vamana`-constructed graph (`n=500`, `dims=32`, `R=16`,
  `block_size=16`, seed 42), with `bnf_config = { beta: 50, tau: -1.0 }`
  (a configuration that mathematically never early-stops, since `OR(G)`
  gain always lies in `[-1, 1]` and can never fall below `-1.0`, giving
  BNF's own iterative refinement the full 50 iterations to compound):
  naive/ID-order `OR(G) = 0.0300` (close to the paper's own measured `≈0`
  baseline); BNP `OR(G) = 0.1401`; BNF, after the best-seen fix, `OR(G) =
  0.1756` — **strictly beating BNP**, matching the paper's own aggregate
  claim directly, not merely the weaker `≥` guarantee the best-seen fix
  provides as a floor. A hand-checkable 6-node
  example (two disjoint 2-cycles, interleaved by ID) independently proves
  BNP's own mechanism reaches `OR(G) = 1.0` from a naive `0.0` baseline by
  hand; a second hand-checkable trace over the RFC's own 5-node worked
  example proves BNF's own one-iteration mechanism exactly, including the
  real case where it decreases `OR(G)` from BNP's `0.6` to `0.4` — and
  confirms `bnf(...)`'s own best-seen wrapper correctly returns BNP's
  better `0.6` layout instead, both proven in the same test.

  Verification: `cargo check --workspace --all-targets`, `cargo clippy
  --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and
  `cargo test --workspace` all clean (11 new tests in
  `crates/strand-vector/src/reorder.rs`, including a property-tested
  bijection guarantee over real `build_vamana` graphs at varied scale,
  degree bound, and block size — the correctness property the eventual
  wire-format permutation-directory blob depends on completely). Depends
  on: nothing further for this slice; the wire-format blobs (Design §§2–3)
  and the query path (Design §5) remain separate, later slices in this
  same sequence.

  **Addendum, 2026-08-20 — a real documentation error, caught by
  independent review, corrected.** The commit that landed this slice
  originally recorded `BNF OR(G) = 0.1401`, exactly equal to BNP, with an
  entire (wrong) honesty narrative built on that false equality. The real
  cause: this file's own comparative test's `BnfConfig` (`beta`/`tau`) was
  changed once more, concurrently, after the number `0.1401` was last
  observed and before the code was actually committed, and the docs were
  written from that stale, no-longer-current observation rather than
  re-verified against the final committed state. An independent
  adversarial reviewer re-ran the pinned test fresh and got a different,
  real result — `BNF OR(G) = 0.1756`, strictly greater than BNP — which
  the coordinator then independently reproduced twice before correcting
  this entry. The underlying code, its `bnf(...) ≥ bnp(...)` guarantee,
  and the test's own assertion (`bnf_or > bnp_or`, which the false
  `0.1401` claim was never actually logically consistent with in the
  first place, since the assertion is strict) were never wrong — only the
  number and narrative written down here were, and both are now
  independently re-verified rather than merely re-asserted.

  **Implementation, slice 3 of 4 — the graph-blob wire format and a real
  construction-to-wire integration, no query path — done (2026-08-20).**
  `crates/strand-vector/src/graph_blob.rs`: the graph node-record blob
  (`blob_type_id = 0`) and the node-order permutation directory
  (`blob_type_id = 1`) under a newly registered `family_id = 5` ("graph"),
  per Design §§1–3, plus `build_graph_blob_specs` — the real function that
  takes slice 1's `VamanaResult` and slice 2's `Permutation` and produces
  two `strand_core::segment::BlobSpec`s ready for `SegmentBuilder::
  add_blob`, following the exact registration table (`storage-class:
  raw-mappable`, `tier: warm`, node records 8-byte aligned, the
  permutation directory 4-byte aligned) and the same writer-then-reader
  discipline `crates/strand-vector/src/navigation.rs` established for RFC
  0010's own cluster family. `spec/container.md` §9's blob-type registry
  gained the two new rows; the intro paragraph's own running RFC-count
  narrative was updated to name RFC 0014 as the fifth family registered.
  Deliberately **not** in scope, per this task's own boundary: `GreedySearch`/
  `BeamSearch` traversal (Design §5) — this slice only proves a writer and
  a reader agree on what the bytes mean, the same scope every other blob
  family's own read-path proves before its query layer is built; that
  query path is slice 4, the last slice in this sequence.

  **The one real integration decision this slice had to make that neither
  RFC 0014 nor slices 1–2 pin, named and resolved rather than invented
  silently.** `crate::vamana::Graph` operates on plain array indices
  `0..n`; `crate::reorder::Permutation` is documented as mapping "logical
  node index" to physical slot; RFC 0014 Design §3 itself specifies only
  that the *wire* permutation directory is indexed by row-id-order local
  ordinal. Nothing in any of the three pins how a caller's array index
  relates to a row-id's local ordinal. This module resolves it the direct
  way: a caller is expected to feed `build_vamana` points and row-ids in
  the same order in the first place, so array index *is* local ordinal —
  documented explicitly in the module's own top-level documentation as an
  integration choice, not a new format decision, since RFC 0014 never
  specifies how a writer obtains that association at all.

  **Byte-exact worked-example reproduction, both blobs, every field —
  built directly from the RFC's own stated illustrative topology and
  physical-slot assignment, not from a real `build_vamana` run, per this
  task's own instruction** (slice 1's own report already found the RFC's
  worked example is "a plausible Vamana output for hand-checkability, not
  a hand-execution of Algorithm 3," so reproducing it means constructing
  the RFC's own stated adjacency lists and slot assignment directly).
  `node_records_match_the_rfcs_worked_example_byte_for_byte` asserts the
  full 176-byte node-record blob (16-byte header + 5×32-byte records)
  field by field against the RFC's own worked-example tables, matching
  `crates/strand-tools/src/puffin_export.rs`'s own byte-table-assertion
  discipline; `permutation_directory_matches_the_rfcs_worked_example_byte_
  for_byte` does the same for the 20-byte permutation directory. A second,
  independent confirmation pins the same bytes as real conformance golden
  files (`conformance/graph/toy-node-records.bin`,
  `conformance/graph/toy-permutation-directory.bin`, generated once from
  the RFC's own stated bytes and independently cross-checked byte-for-byte
  against the RFC's own hex tables before being committed) —
  `node_records_and_permutation_directory_match_the_pinned_conformance_
  golden_files` asserts the real builder output against them, the same
  two-pronged proof `crates/strand-vector/tests/worked_example.rs` already
  established for RFC 0010. `decode_recovers_the_worked_example_exactly`
  closes the loop: the real `NodeRecordReader`/`PermutationDirectoryReader`
  decode the same bytes back into row-ids, vectors, physical-slot
  adjacency (padding correctly stripped, e.g. E's real single neighbor `[3]`
  recovered with the zero-padding entry excluded), and the permutation
  itself, exactly matching what the worked example states.
  `assembles_and_reopens_the_worked_example_as_a_real_segment` wires the
  same worked example through a real `SegmentBuilder`, a real footer/
  hotcache decode, and a real blob-registry lookup by
  `(family_id, blob_type_id, field_id)` — the same discipline
  `crates/strand-vector/tests/segment_assembly.rs` already established for
  RFC 0010's own four blob types, now proven for `family_id = 5`.

  **Round-trip property test at realistic scale, closing this task's own
  required proof.** `round_trips_a_real_vamana_plus_bnf_graph_through_the_
  wire_format_at_scale` builds a real graph via `build_vamana`
  (`n=300`, `dims=16`, `R=12`) and a real BNF permutation via `bnf`
  (`block_size=16`), writes both blobs through `build_graph_blob_specs`,
  assembles a real segment, reopens it cold via the same footer/hotcache/
  registry path, decodes both blobs, and asserts full structural equality
  against the original in-memory construction output for every one of the
  300 nodes: row-id, vector, and physical-slot-translated adjacency list
  all match exactly, and the decoded permutation matches the one `bnf`
  produced — the correctness property slice 4's own query path depends on
  completely, proven end to end through the real wire bytes rather than
  only against the raw in-memory structures.

  Verification: `cargo check --workspace --all-targets`, `cargo clippy
  --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and
  `cargo test --workspace` all clean (6 new tests in
  `crates/strand-vector/src/graph_blob.rs`, 0 failures workspace-wide,
  including every other blob family's own existing tests unaffected by
  the new `family_id = 5` registration). Depends on: slices 1 and 2
  (`vamana.rs`, `reorder.rs`) for their real output types; slice 4 (the
  `GreedySearch`/`BeamSearch` query path over this wire format, Design §5)
  is the one remaining slice in this sequence.

  **Implementation, slice 4 of 4 — the cold-open `GreedySearch`/
  `BeamSearch` query path over the real wire format, closing this
  implementation sequence — done (2026-08-20).**
  `crates/strand-vector/src/graph_query.rs`: `greedy_search_cold`, run
  against a real, opened `NodeRecordReader` (slice 3's own per-record
  wire-format accessor) rather than slice 3's `decode_graph_blobs` — a
  deliberate choice, argued in the module's own doc comment, because
  `decode_graph_blobs` eagerly materializes every node before a query
  starts, which would make every candidate "already resident" and
  silently deflate the very fetch counts Design §5 exists to measure.
  Same algorithm as `crate::vamana::greedy_search` (task 1) — same
  candidate-list expand/trim loop, same `min_by`/`sort_by` tie-break
  behavior — with every candidate's vector and physical-slot adjacency
  list read from the segment's real bytes via `NodeRecordReader::
  row_id`/`vector`/`neighbor_slots` rather than indexed out of an
  in-memory array. `filter_deleted` closes Design §5 step 3 (tombstoned
  row-ids removed from the *returned* result only, never from the
  traversal's visited/fetched sets), mirroring `crate::query::
  filter_deleted`'s own already-established pattern for the cluster
  family rather than inventing a second convention.

  **The "array index is local ordinal" landmine slice 3's own reviewer
  flagged for this task, handled by argument, not by adding defensive
  code for its own sake.** That convention governs only the *writer*
  side — a caller of `GraphBlobInput` must pass `points`/`row_ids`/
  `permutation` arrays that agree on what array index `i` means, because
  nothing in `GraphBlobInput`'s own type enforces it. This module never
  constructs a `GraphBlobInput`; `NodeRecordReader`'s entire public API
  (`entry_point_slot`, `neighbor_slots`, `row_id`, `vector`) is indexed by
  physical slot only, with no second, independently-indexed array a slot
  value could be silently mismatched against. The landmine's actual
  precondition — two arrays a caller must keep in sync by convention
  alone — does not exist in this module's own code, so no runtime check
  or marker type was added; a marker type wrapping a `u32` slot with no
  operations to guard against would have been ceremony, not safety.
  `PermutationDirectoryReader` (the one place local ordinal appears at
  all, for the row-id-seeded query variant Design §3 names) already
  exposes a correctly-scoped `physical_slot(local_ordinal)` method this
  module reuses rather than re-deriving.

  **A real bug this task found in its own first draft, fixed, and named
  rather than quietly patched — not in `vamana.rs`/`reorder.rs`/
  `graph_blob.rs` (those stayed untouched, per this task's own
  instruction), but in this slice's own first attempt at the fetch-count
  semantics.** The first draft cached a slot's decoded record permanently
  once read, treating "already fetched this search" as a lifetime
  property. `crate::vamana::greedy_search`'s own proven-correct semantics
  are narrower: a slot is fetched exactly when it is added to the
  *current*, possibly-already-trimmed candidate list `L` — the RFC's own
  literal Design §5 step 2 wording ("for each of `p*`'s `neighbor_slots`
  not already in `L`... fetch that slot's record"). Once a slot is
  trimmed out of `L`, it is no longer "already in `L`," so a later hop
  that rediscovers it as a neighbor genuinely re-fetches it — a real
  double-count the RFC's own algorithm produces, not a bug in either
  implementation. The permanent-cache draft under-counted fetches against
  this behavior (caught by the large-scale equivalence test below: 151
  fetches reported vs. 231 actually required by `crate::vamana::
  greedy_search` on the same real graph and query). Fixed by decoupling
  content memoization (`ensure_decoded`, reused freely, never itself
  logged) from fetch-event logging (`fetched_slots.push`, gated on the
  same `L`-membership check `crate::vamana::greedy_search` uses) — found
  and fixed before landing, not shipped and discovered later.

  **Worked-example reproduction: exact match, both the result and the
  fetch arithmetic.** Built directly from the RFC's own stated
  illustrative 5-node topology and physical-slot assignment (the same
  construction slice 3's own tests use for this graph, not a
  `build_vamana` run), assembled into a real segment via `SegmentBuilder`,
  reopened cold through the real footer/hotcache/registry path, and
  queried with `greedy_search_cold` from the real, wire-decoded
  `entry_point_slot`: result **`B`** (row_id 11), **2 real hops** (`A`
  then `B` expanded), **4 real fetches** (`A`, `B`, `C`, `D` — `C` and `D`
  fetched only to be trimmed away) — the RFC's own "2 hops, 4 fetches"
  figure matched exactly, over real wire bytes, not only in-memory
  structures.

  **Larger-scale equivalence against the already-proven in-memory
  algorithm — the single most important correctness property for this
  task, and the one that caught the bug above.** A real `build_vamana` +
  `bnf` graph (`n=300`, `dims=16`, `R=12`), written through
  `build_graph_blob_specs`, assembled into a real segment, and reopened
  cold; 25 real random queries run through both `greedy_search_cold`
  (over the real wire bytes) and `crate::vamana::greedy_search` (over the
  original in-memory graph, before anything was ever written to disk).
  Every query's result, hop count, and fetch count match exactly across
  all 25 queries — the cold-open path is a faithful reproduction of the
  already-proven-correct in-memory algorithm, confirmed rather than
  assumed.

  **Napkin-math honesty check: a real, measured fetch count at moderate
  scale, checked (not skipped) because it was feasible without heavy
  compute, and reported honestly against the RFC's own pessimistic
  bound rather than forced to confirm it.** A real `build_vamana` + `bnf`
  graph at `n=1,000`, `dims=32`, `R=16` (construction `L=32`), 30 real
  random `k=10` queries at query-time `L=32`: **mean 447.4 fetches/query,
  mean 33.6 hops/query, max 501 fetches/query** — measured directly by
  `measures_a_real_fetch_count_at_moderate_scale_against_the_rfcs_
  pessimistic_bound` (`cargo test -p strand-vector -- --nocapture` prints
  the exact figures). Against the same test's own computed `hops × R`
  pessimistic bound at this scale (`33.6 × 16 ≈ 537`), the real measured
  mean (447.4) is **≈83% of the pessimistic bound** — real inter-hop
  neighbor-set overlap is reducing the fetch count below the "credit no
  overlap between hops" worst case Napkin math's own `10,000`-fetch
  figure assumes, but not dramatically, and not enough to draw a general
  conclusion from. Stated honestly, per this task's own instruction not
  to force a match: this measurement is at `n=1,000`, `R=16` — three to
  four orders of magnitude smaller in `n` and roughly a factor of 4–8
  smaller in `R` than the RFC's own pessimistic-case citation (DiskANN's
  own `R=64`–`128` at "tens of millions of vectors," Starling's own
  "hundreds of hops" figure at that scale). It neither confirms nor
  refutes the RFC's own `10,000`-fetch pessimistic bound at realistic
  production scale — it is real evidence that *some* inter-hop overlap
  exists and measurably shrinks the naive bound at this smaller scale,
  which is worth recording, but Napkin math's own named follow-on ("a
  real inter-hop neighbor-overlap measurement for Vamana graphs," RFC
  0014 Open questions) remains open at realistic `R` and `n` — this is a
  real data point toward it, not a resolution of it.

  Verification: `cargo check --workspace --all-targets`, `cargo clippy
  --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and
  `cargo test --workspace` all clean (5 new tests in
  `crates/strand-vector/src/graph_query.rs`, 0 failures workspace-wide;
  full workspace suite ≈31s wall time with the new tests included, most
  of it this slice's own two graph-construction-backed tests). Depends
  on: slice 3 (`graph_blob.rs`) for `NodeRecordReader`; this is the last
  slice in the RFC 0014 implementation sequence. `spec/graph-vectors.md`
  remains real, named, unwritten follow-on work — this slice completes
  the reference implementation, not the normative spec chapter.

  **Real `bench/` measurement, closing the gap this RFC's own Status line
  and Napkin math named from the start — done (2026-08-20).** Not a fifth
  implementation slice: the four slices above are the reference
  implementation; this is follow-on validation against it, the same
  relationship `bench/src/vector_cold_open.rs` (M2-2) has to RFC 0010's
  own four cluster-family implementation slices. `bench/src/
  graph_warm_query.rs` (registered as `graph-warm-query` in `bench/
  Cargo.toml`) replaces every literature-translated figure in RFC 0014's
  own Napkin math with a real, measured one, following `vector_cold_
  open.rs`'s statistical-rigor pattern (many real trials, p50/p90/p99,
  a `bench/results/*.json` file) rather than `vector_cold_open.rs`'s own
  *governing regime* — argued explicitly, not assumed, because this
  family is different in kind from the ones already benchmarked.

  **Methodology decision, argued before any code was written.** The
  graph family is `tier: warm`, exempted from invariant 3's one-wave,
  ~100ms-object-storage-round-trip cold-path accounting by invariant 7's
  own text ("assumes NVMe-class latency; graph families live here"); RFC
  0014's own Napkin math built its entire cost argument around DiskANN's
  own cited "~100-300μs" retail-SSD figure, not the ~100ms S3 figure
  `cold_open.rs`/`vector_cold_open.rs` measure against. Counting S3 GETs
  per query the way those two benchmarks do would therefore measure the
  wrong regime for this family's *query* path. This benchmark's two
  primary measurements instead are (1) a real fetch-count/hop-count
  distribution from a real Vamana graph queried through the real
  cold-open wire format, and (2) a real local random-read latency
  baseline measured directly against this machine's own storage device
  — replacing DiskANN's 2019-era literature figure with a measured 2026
  one, since this environment turned out to make that fair: `lsblk`
  shows a real `nvme0n1` block device backing `/` via LVM,
  `/sys/block/nvme0n1/queue/rotational` reads `0`, and `/sys/class/dmi/
  id/{sys_vendor,product_name}` report "ASUSTeK COMPUTER INC." /
  "MINIPC PN62" — a real mini-PC motherboard, not a cloud hypervisor's
  virtual-disk identity, confirmed with `systemd-detect-virt` reporting
  `none` too. `O_DIRECT` support on this checkout's own ext4-on-LVM
  filesystem was separately confirmed with `dd ... oflag=direct`/
  `iflag=direct` before any Rust was written. A third, secondary
  measurement was judged cheap enough to include: `graph_blob.rs`'s own
  `build_graph_blob_specs` already produces ready-to-commit `BlobSpec`s,
  so a real MinIO whole-blob-fetch measurement (following `vector_cold_
  open.rs`'s own commit-then-cold-open pattern) was added at near-zero
  extra cost, answering "what does a reader pay to warm-cache this blob
  from S3 once" — a real number `tier: warm` doesn't exempt this family
  from, since object storage remains STRAND's primary target even for a
  blob a reader then serves many local queries against.

  **Construction scale, measured empirically before being chosen, per
  this task's own explicit instruction not to assume `build_vamana`'s
  cost.** A throwaway release-mode probe (not shipped) timed
  `build_vamana` directly: `n=500` (1.83s), `n=1,000` (4.30s), `n=2,000`
  (9.60s), `n=4,000` (20.80s), `n=8,000` (47.18s) at `dims=64, R=32,
  L=64` — a consistent ~2.2–2.35x-per-doubling growth rate, super-linear
  in `n` but empirically closer to `O(n^1.15–1.3)` than to a quadratic
  shape (a 4x-per-doubling signature true `O(n²)` would produce). An
  earlier version of this entry attributed the super-linear growth to a
  `robust_prune` complexity claim ("`O(remaining²·dims)`, `crate::
  vamana`'s own documented" inner loop) that does not actually appear
  anywhere in `crates/strand-vector/src/vamana.rs`'s real documentation —
  a fabricated citation, caught by an independent ACPR review and
  corrected here rather than left standing, per `CLAUDE.md` §2/§3.
  `robust_prune`'s real loop runs at most `r` iterations (a constant),
  each doing an `O(|remaining|·dims)` scan, so a single call's own cost
  is roughly linear in its candidate-set size; the observed growth
  reflects construction's overall shape (`n` GreedySearch-plus-
  RobustPrune calls, each touching a growing average neighborhood), not
  any single call's own asymptotics. At more realistic
  DiskANN-cited values (`dims=128, R=64, L=100`, `references/diskann-
  neurips2019.md`: SIFT1M/GIST1M uses `R=70`; DEEP1M and the SIFT1B
  merge-shard configuration both use `R=64`), `n=2,000` took 80.6s and
  `n=4,000` took 181.3s in release mode; a first attempt at `n=5,000,
  dims=128, R=64, L=100` (the paper's own SIFT1M-adjacent `L≈125`
  region) was killed after exceeding 3 minutes with no sign of
  finishing, confirming this task's own warning that `n=50,000+` was not
  safe to assume tractable. `n=4,000, dims=128, R=64, L=100, α=1.2` was
  chosen as this benchmark's real construction scale — `dims` and `R`
  both real DiskANN-cited values, 4x the toy check's node count, and a
  known-measured ≈181s (confirmed again on the real run below: 183.5s)
  — and the benchmark runs in release mode (`cargo run -p strand-bench
  --release --bin graph-warm-query`), the same discipline M2-6's own
  `cargo test --release` entry already established for another
  CPU-heavy measurement in this codebase; debug-mode timing at this
  scale was not measured and is not expected to be tractable.

  **The real measurement, run against real MinIO on 2026-08-20**
  (`bench/results/graph-warm-query.json`; commit `07f22e9`). Real
  `build_vamana`: 183,468ms (≈3.06 minutes). Real BNF permutation: 48ms.
  Real `OR(G)` (Starling's own locality metric, measured on this run's
  own graph rather than cited from the paper): BNF `0.0795` vs.
  unshuffled `0.0156` — a real 5.1x improvement from BNF over the naive
  baseline on this run's own graph, confirming the *direction* of
  Starling's own claim, but **both figures sit far below the paper's own
  cited real-dataset range (`OR(G) ≈ 0.3–0.6`)**, stated honestly rather
  than smoothed over: this benchmark's points are synthetic uniform
  random noise with no cluster or semantic structure for a block-shuffle
  pass to exploit, and `n=4,000` gives only 125 blocks at
  `block_size=32` — a small, structure-free graph is a genuinely
  different regime from the real embedding datasets Starling's own paper
  measured, and this run does not claim to reproduce their absolute
  numbers, only to confirm BNF still helps relative to no shuffling at
  all on a real, if unfavorable, graph.

  Two real query-time `L` values were measured against the identical
  built graph (300 real queries each, distinct seeds, `k=10`): **`L=32`**
  — mean **2,032.9** fetches/query (p50 2,018, p90 2,127, p99 2,249, min
  1,914, max 2,269), mean **33.5** hops/query (min 32, max 37), **95.0%**
  of the pessimistic `hops×R` bound (`2,140.8`); **`L=100`** (matching
  construction `L`) — mean **5,761.1** fetches/query (p50 5,764, p90
  5,841, p99 5,895, min 5,617, max 5,941), mean **100.8** hops/query (min
  100, max 103), **89.3%** of the pessimistic bound (`6,451.0`). A real,
  honestly-reported surprise, not smoothed away: at **both** `L` values,
  the mean hop count sits almost exactly at `L` itself (`33.5` of `32`,
  `100.8` of `100`) — every query visits essentially the *entire* search
  list before `GreedySearch` terminates, rather than converging early the
  way DiskANN's own "2-3x fewer hops" claim (§4.2) suggests is typical.
  Traced to cause, not left as an anomaly: this benchmark's points are
  uniform random noise in a 128-dimensional cube with no cluster
  structure, a near-worst-case regime for greedy nearest-neighbor search
  (curse-of-dimensionality near-equidistance gives the search little
  gradient to converge on before its search list simply fills up) — a
  real embedding dataset would very plausibly converge in far fewer hops,
  and this is named as a real, load-bearing limitation of this
  benchmark's synthetic data, not papered over. It also means the
  `fetches_as_fraction_of_pessimistic_bound` figures above (95.0%/89.3%)
  are measured in this same adversarial regime and run *higher* than the
  RFC 0014 task-4 unit test's own toy-scale figure (≈83% at `n=1,000,
  R=16`) — real inter-hop neighbor-set overlap is measurably *smaller*
  at this larger, denser `R=64` scale than at the toy scale, the opposite
  of an optimistic "overlap improves with scale" assumption; stated
  plainly rather than assumed away, and still short of a full resolution
  of RFC 0014's own named follow-on ("a real inter-hop neighbor-overlap
  measurement for Vamana graphs," Open questions), since this run's own
  points are adversarial-uniform, not real embeddings either.

  Real local random-read latency (`O_DIRECT`, this machine's own NVMe,
  2,000 samples, 4096-byte blocks): **p50 = 56.2μs, p90 = 61.8μs, p99 =
  81.1μs** (mean 62.1μs, min 23.7μs, max 8,457.2μs — one real outlier
  spike, not excluded, real tail behavior on a real device under
  whatever else the host was doing at that instant). This sits **below**
  DiskANN's own cited "~100-300μs" 2019 retail-SSD range, not merely
  within it — a real, measured confirmation that a 2026 real NVMe device
  is genuinely faster than the paper's own cited figure, not a claim
  that STRAND's own reader is fast: `estimated_query_latency_ms_using_
  local_p50` (mean fetches × the real local p50) comes to **114.3ms** at
  `L=32` and **324.0ms** at `L=100` — using DiskANN's own cited range
  instead gives **203.3–609.9ms** (`L=32`) and **576.1–1,728.3ms**
  (`L=100`). All four numbers are far above DiskANN's own published
  `<3ms` figure, exactly as RFC 0014's own Design §5 and Napkin math
  predicted this v0.1 scope (no compressed-code cache) would cost — a
  real, measured confirmation of that RFC's own honestly-stated
  regression, not a surprise, though the *specific* number (hundreds of
  milliseconds to over a second per query, even at a mere `n=4,000`) is
  new information the RFC's own literature-translated arithmetic did not
  contain, and is driven substantially by the hop-saturation regime
  above rather than purely by scale the way the RFC's own tens-of-
  millions-of-vectors napkin math assumed.

  Real secondary S3 measurement (real MinIO, 10 iterations, whole-segment
  GET, the same limitation `cold_open.rs`/`vector_cold_open.rs` already
  carry — no Range-GET reader exists yet in `strand-core`): the real
  committed graph blob totals **3,152,016 bytes** (node records
  3,136,016 bytes + permutation directory 16,000 bytes) for this
  `n=4,000, dims=128, R=64` graph, fetched wholesale in **3 GETs**
  (pointer, snapshot, segment — invariant 3's own ≤2-RTT-past-the-footer
  accounting, though this family isn't bound by invariant 3, the GET
  count still comes in at the same shape) at **p50 = 4.6ms** against
  MinIO on localhost with no injected network latency (so, like every
  other localhost MinIO figure in this document, a real lower bound, not
  a real-S3 figure). A real, if modest, secondary confirmation that
  warm-caching this family's blob from object storage once is cheap
  relative to the per-query cost measured above.

  Verification: `cargo check --workspace --all-targets`, `cargo clippy
  --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and
  `cargo test --workspace` all clean. Depends on: all four implementation
  slices above. Does not resolve RFC 0014's own remaining Open questions
  (the compressed-code cache, the navigation-graph entry-point
  optimization, `spec/graph-vectors.md`) — those remain real, named,
  unwritten follow-on work; this closes specifically the "no `bench/`
  measurement exists" gap the RFC's own Status line and Napkin math
  named from the start.
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
  M4 entry. **Status: done (2026-08-20)**
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

## Domain scoping (2026-08-20 decision) — logs and code

`CLAUDE.md` §1's new "Target domain" paragraph and `docs/ledger.md`'s matching
decision entry narrow this project's target content domain to telemetry-adjacent
text — application/system logs and source code — without changing the format's own
generality (checked directly against `spec/container.md`, `spec/row-ids.md`,
`spec/manifest.md`, and all fourteen approved-or-implemented RFCs, `docs/ledger.md`'s
own entry). This section is that decision's own promised downstream task list,
decomposing its four named "does change" items into completable work, the same
discipline every other milestone gets in this document.

- **D-1** — A real logs-and-code analyzer profile: identifier splitting
  (`camelCase`/`snake_case`/`kebab-case`), log-line structure (`key=value` pairs,
  timestamps, stack traces), and the harder mixed-analysis problem — prose-shaped
  spans (comments, docstrings) embedded inside code-shaped tokens, needing per-span,
  not just per-blob, analysis policy. Source: `docs/ledger.md`'s scope-narrowing
  entry, its own "precision note" on invariant 6. **This is real format-design
  surface, not a pure-implementation task**: invariant 6 (`CLAUDE.md` §5) commits to
  per-blob pluggable analyzer descriptors, not per-span mixed-analysis policy within
  one blob, so registering a genuinely mixed-content profile may need either (a) an
  amendment to RFC 0004's own descriptor schema, or (b) a document-level convention
  (e.g., splitting a source file into separate code-span and comment-span sub-fields
  at index time, each with its own conventional analyzer descriptor, needing no
  schema change at all) — which of these is the right shape is itself an open design
  question this task must resolve, per `CLAUDE.md` §3's design-then-implementation
  separation, before any conformance vectors are built. Status: open, RFC-sized (an
  amendment to RFC 0004, or a new RFC, depending on which shape D-1's own design pass
  concludes is correct). Depends on: nothing technically, but real log/code corpora
  (D-2) make its own conformance-vector work concrete rather than speculative.
- **D-2** — Source and license-audit real log and source-code corpora to ground D-1's
  conformance vectors and future benchmarks, the same discipline this project already
  applied to MS MARCO (vendored, license-checked) and the RaBitQ/FastLanes reference
  implementations (license-audited via GitHub's license API before adoption). A real
  log corpus (e.g., a public system-log dataset — several exist under varied
  licenses, none yet checked) and a real source-code corpus (e.g., a public,
  Apache-2.0-or-compatible code dataset or a real open-source repository used as a
  fixed snapshot) both need a genuine license audit before vendoring, matching
  `CLAUDE.md` §1's "every dependency must be Apache-2.0-compatible" rule applied to
  benchmark data, not just code dependencies. Status: open, research-sized — no RFC
  needed, but real licensing verification work, not to be skipped or assumed.
  Depends on: nothing; unblocks D-1's real conformance-vector work and D-3's real
  code-embedding grounding.
- **D-3** — Revisit the vector family's embedding conventions against real
  code-embedding models rather than assumed-generic ones. The graph-blob family's own
  real `bench/` measurement (`bench/src/graph_warm_query.rs`, M2-3 above) used
  synthetic uniform-random points precisely because no real, domain-representative
  embedding source was available at the time — a real code-embedding model run over
  D-2's real code corpus would let a follow-up benchmark measure this family's actual
  cost on structured, clustered data instead of a near-worst-case synthetic
  distribution, closing the gap this project's own conversation about that benchmark
  already named directly. Status: open, needs D-2's real corpus first and a real,
  licensable, locally-runnable code-embedding model (not yet identified or verified
  available in this environment — check feasibility before committing to a specific
  model). Depends on: D-2.
- **D-4** — Vendor telemetry-adjacent prior art into `references/` and `docs/lineage.md`,
  the same discipline every other design lineage entry in this project already
  follows (a primary source, fetched and cited, not asserted from memory,
  `CLAUDE.md` §3). NOFireAI/ravel (surveyed conversationally this session, license
  independently confirmed Apache-2.0 via GitHub's license API, `docs/ledger.md`'s own
  entry) is one real candidate — an object-storage-first observability datastore,
  structurally comparable at the storage layer though not itself a search-index
  format. Real, code-search-specific prior art (e.g., Google/Sourcegraph's Zoekt, a
  trigram-based code-search engine — license and technical shape not yet checked in
  this project) is a second real candidate worth checking, since it is directly a
  code-search system rather than an adjacent storage layer. Status: open,
  research-sized, no code dependencies, no other item in this section blocks it —
  a reasonable next single task given its low weight. Depends on: nothing.
- **D-5** — A recency-weighted BM25 scoring profile, `bm25-recency`, motivated by
  the same domain-scoping decision that opened D-1 through D-4 but not one of its
  four originally-named "does change" items itself — a new task this session's own
  design exploration (research → design → independent adversarial critique)
  surfaced, and a real candidate for logs specifically: log relevance genuinely
  decays with age in a way general prose search does not model. The critique found
  the core research sound (the dl/avdl blob gap is real and correctly independent
  of this design; the recency formula is genuinely Elasticsearch's own exponential
  decay function, independently re-verified against Elastic's live docs;
  `family_id = 6` is genuinely unallocated in `spec/container.md` §9) but flagged
  five real problems before the design would be RFC-ready. This entry is the design
  corrected against all five, re-verified against the actual repo rather than
  trusted from the critique's summary.

  **Mechanism.** Register `family_id = 6` ("temporal"), `blob_type_id = 0`
  ("event-timestamp store"): a dense `u64`-milliseconds-per-row-ordinal array,
  `storage_class = raw-mappable`, `tier = cold-fetchable`, `alignment = 8`,
  `merge strategy = concatenate + remap`. The real structural precedent for that
  merge strategy is `spec/vectors.md` §7's flat-vector blob — also a dense,
  row-ordinal-indexed array merged by concatenation and offset remapping, not
  rebuild or rebalance — not the deletion vector the original proposal cited.
  **Fix 1 (field_id):** the proposal claimed `field_id = FIELD_ID_NONE` (row-scoped,
  not field-scoped) usage here was "exactly the precedent `spec/deletion.md`'s
  deletion-vector blob already establishes." Independently re-checked against
  `spec/deletion.md` §2 ("Not a segment, not a container... No footer, no hotcache,
  no blob registry"), RFC 0012 Design §1 ("this 'blob type' is registered for
  identity and citation purposes even though the object itself never lives inside a
  segment's own blob registry"), and `crates/strand-core/src/deletion.rs` directly
  (no `BlobEntry`/`BlobSpec` anywhere in it): this is false. A deletion vector is
  never a blob-registry entry at all. The only real prior `field_id = 0` usage
  anywhere in the actual blob registry is RFC 0001's own toy placeholder, which
  `spec/container.md` §5a itself now states plainly is the only registry entry that
  has used it to date (stale "deletion vector, RFC 0012" reference in that section
  removed by this same pass, `spec/container.md` §5a). This design would be the
  **first real production use** of blob-registry `field_id = 0`, not a repeat of
  established practice — stated as such, not glossed.

  A new scoring profile `bm25-recency` (`spec/scoring-profiles.md`):
  `final_score = bm25_component * decay(event_timestamp_millis, origin_millis,
  scale_millis, decay, offset_millis)`, where `decay(t, origin, scale, decay,
  offset) = exp((ln(decay) / scale) * max(0, |t - origin| - offset))` —
  Elasticsearch's real exponential decay function. Segment-frozen parameters: `k1`,
  `b` (inherited, same defaults and bounds as `bm25`); `scale_millis` (required, no
  default); `decay` (`0 < decay < 1`, default `0.5`); `offset_millis` (default `0`).
  **Fix 4 (parameter validation):** `scale_millis` had no stated lower bound in the
  original proposal — `ln(decay) / scale_millis` divides by zero at `scale_millis =
  0`, the exact class of bound `spec/scoring-profiles.md` §2 already enforces on
  `k1` (`≥ 0, finite`) and `b` (`0 ≤ b ≤ 1`). Add `scale_millis > 0, finite` to the
  parameter table, matching that rigor. No principled *upper* bound is arguable from
  first principles the way the lower bound is: an unusually large `scale_millis`
  just makes decay negligible over any realistic time delta, which is not a
  correctness problem, only a modeling no-op — inventing an arbitrary cap here would
  be exactly the kind of unsourced number `CLAUDE.md` §2 deletes rather than
  softens. The one real MUST is `finite` (rejecting `+inf`/`NaN`, matching `k1`'s
  own `finite` requirement), not a numeric ceiling.

  Query-time, **not** segment-frozen: `origin_millis` — storing "now" in a segment
  descriptor would silently decay every score to zero once the segment ages past
  `scale_millis`, so `spec/scoring-profiles.md` §1 needs one new sentence
  acknowledging a profile's inputs may include caller-supplied, non-descriptor
  values (real, since today's descriptor schema, RFC 0003 §1, states only
  segment-frozen fields).

  **Worked example**, extending RFC 0003's own canonical `bm25` example (4 docs,
  term "whale", `dl = 41`, `tf = 3`, `avdl = 40`, `k1 = 1.2`, `b = 0.75` →
  `idf = 0.847298`, `norm = 1.222500`, `bm25 = 0.601988` — re-derived by hand here,
  matches) with a 5th document sharing the same `tf`/`dl`/`avdl` shape but a
  different `n`: `idf = ln(1.4) = 0.336472`, `norm = 1.222500` (unchanged, `dl`/`avdl`
  held fixed by construction, a toy simplification, not a claim about a real
  multi-segment `avdl` recomputation), `bm25_component = 0.336472 * 3 / 4.222500 =
  0.239057` (recomputed by hand: `3 * 0.336472 = 1.009416`; `1.009416 / 4.2225 =
  0.239057` — confirmed, not copied uncritically). At `scale_millis = 3,600,000`
  (1 hour), `decay = 0.5`, `offset_millis = 0`: doc D (30 min = 1,800,000 ms old)
  gets `decay = exp(0.5 * ln(0.5)) = 0.5^0.5 = 0.707107` → `final = 0.239057 *
  0.707107 = 0.169039`; doc E (2 hr = 7,200,000 ms old) gets `decay = 0.5^2 = 0.25`
  → `final = 0.239057 * 0.25 = 0.059764`. Ratio `0.169039 / 0.059764 = 2.828428 ≈
  2√2` — all four figures re-derived independently here, matching the original
  proposal exactly.

  **Cost, re-verified against the real `bench/results/field-end-to-end-100476.json`
  figures, one error found and corrected.** Storage at the real 100,476-document
  MS MARCO benchmark scale: `100,476 * 8 = 803,808` bytes (≈785 KiB, confirmed:
  `803808 / 1024 = 784.97`), **≈0.77%** of the 100 MB (104,857,600-byte) cold-open
  budget (`803808 / 104857600 = 0.00767`) — matches the original proposal. The
  proposal's comparison figure does not: it cited "the real positions-blob figure of
  4,135,112 bytes (≈19% of it)" against the same 100 MB budget. Recomputed directly
  against the real, committed figure (`bench/results/field-end-to-end-100476.json`,
  `"positions_bytes": 4135112`): `4135112 / 104857600 = 0.03944` — **≈3.9%, not
  ≈19%**. The raw byte figure itself is real and correctly cited (confirmed against
  the committed JSON); the percentage computed from it was not. Corrected here as an
  additional finding beyond the critique's five, per `CLAUDE.md` §2's rule that a
  wrong number is deleted or fixed, not softened. Registry overhead: 42 bytes/segment
  (`spec/container.md` §5) against the measured 16,344-byte hotcache ceiling
  (`docs/ledger.md`'s hotcache-tail-read entry) — `42 / 16344 = 0.00257`, **≈0.26%**,
  confirmed. Zero added round trips: the wholesale-fetch placement rule RFC 0003 §4,
  `spec/postings.md` §8, `spec/term-dictionary.md` §5, and `spec/filter-bitmaps.md`
  §5 already established applies identically to a `raw-mappable`, `cold-fetchable`
  blob addressed from the hotcache's blob registry. `committed_at_millis` (the
  existing snapshot-level timestamp, `spec/manifest.md` line 91: "stamped by the
  proposing writer," one value per commit, not per row) cannot substitute:
  `CLAUDE.md` §6 states a production writer batches commits to control segment
  count, and a single `committed_at_millis` shared by every row in that batch
  would collapse an hour-spanning batch's real event-time spread to one decay
  value against this same worked example's own 1-hour `scale_millis` — the
  batching incentive is `CLAUDE.md` §6's own text; the one-value-per-commit fact
  it operates on is `spec/manifest.md`'s, cited precisely rather than folded into
  a single attribution.

  **Fix 2 (undisclosed dependency on an open RFC).** The profile-precondition rule
  (a segment declaring `bm25-recency` MUST also declare the event-timestamp blob)
  requires a reader to know which scoring profile a field declares *before* checking
  blob presence — but RFC 0003's own Open Questions section defers the
  scoring-profile descriptor's placement inside a segment to the still-open
  R2/postings RFC (`docs/ledger.md` R2; RFC 0003 Open Questions: "deferred to the
  R2/postings RFC, which owns the lexical blob's byte layout"). The original
  proposal's blast-radius section claimed zero interaction with any other open RFC
  and missed this coupling entirely. Named explicitly here, not claimed independent:
  drafting `bm25-recency` as a numbered RFC does not strictly require R2 to land
  first (RFC 0003 itself shipped and was approved with its own descriptor placement
  left open, the exact precedent to follow), but the profile-precondition rule's
  own conformance behavior is genuinely unresolved until R2 lands, and the eventual
  RFC MUST say so rather than assume the precondition mechanism is fully specified.

  **Fix 3 (missing infrastructure).** "Apply decay once per document, not once per
  term" assumes a multi-term, per-document score-summing path. Checked directly
  against `crates/strand-lexical/src/field.rs`: `FieldReader::search_bm25(term:
  &str, doc_lengths: &[u32], profile: &Bm25Profile)` and `search_bm25_row_ids` both
  take one `term: &str`, singular — every caller scores exactly one query term at a
  time today; no multi-term aggregation path exists. A literal implementation
  against the current codebase risks applying the decay factor once per
  `(term, document)` pair instead of once per document, silently squaring the
  recency penalty on any multi-term query. Resolution for this pass: **scope the MVP
  explicitly to single-term queries**, matching what `search_bm25` already supports,
  and name multi-term score aggregation (summing term scores per document before
  applying decay once) as new, separate required design work the eventual RFC must
  do — not an assumed-available primitive, and not silently deferred without a name.

  **Fix 5 (domain-fit scope).** `committed_at_millis`'s inadequacy is argued for
  logs specifically; the original proposal never addressed source-code segments,
  where "recency" would mean commit or last-modified time from the VCS, a
  completely different provenance than log-ingest timing, and possibly a different
  per-file rather than per-batch granularity. Resolution: **`bm25-recency` is scoped
  to log-family segments for this pass.** Whether and how an analogous mechanism
  applies to code segments (a `family_id = 6` blob populated from VCS metadata
  instead of ingest timestamps, or a different profile entirely) is named here as a
  separate, later open question, not silently unaddressed the way the original
  proposal left it.

  Status: **open, RFC-sized, design revised after independent adversarial review** —
  not yet an approved or implemented RFC; a corrected pre-RFC design ready for
  someone to actually draft. Depends on: nothing to begin drafting (following RFC
  0003's own precedent of shipping the profile mechanism ahead of the R2 placement
  decision, Fix 2 above); real conformance-vector and end-to-end work is coupled to
  R2 landing and to D-1's log analyzer profile (a `bm25-recency` conformance corpus
  wants real, D-2-sourced log data with real timestamps, not synthetic ones).

  **Next step**: draft this as a real numbered RFC — `ls rfcs/` shows 0001 through
  0014 already allocated, so the next free number is **0015** — once someone picks
  this up, following `CLAUDE.md` §3's design-then-implementation separation (design
  lands and passes its own adversarial review before any implementation session
  starts building against it).

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
