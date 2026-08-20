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
  2026-08-20; reader-path actions added 2026-08-20; `Next`-level
  composition and temporal invariance theorem added 2026-08-20).**
  `verification/manifest_proofs.tla` (2,156 lines) proves `IndInv1`
  (`TypeOK` plus six safety/typing properties, one of them —
  `PtrVersionBounded` — added in this latest pass) inductive across `Init`
  and **all five writer-path actions plus both reader-path actions**
  (`ReadCurrent`, `ProposeSnapshot`, `ProposeDeletionVectorCommit`,
  `TryAdvancePointer`, `ResolveAmbiguity` — matching `commit()`'s and
  `commit_deletion_vector()`'s real control flow — plus `ReadPointer` and
  `ReadSnapshotObject`, matching `read_snapshot()`'s retry loop and its
  `try_read_current()` helper), confirmed by a clean `tlapm` run reporting
  `[INFO]: All 2018 obligations proved.`, exit code 0, reproduced
  identically on two separate runs (one ordinary, one with `--cleanfp`,
  fingerprint cache erased first). A real toolchain fix (TLAPS 1.5.0
  cannot process a `RECURSIVE`-operator definition; `SumCounts` rewritten
  to a recursive function, semantics-preserving, re-confirmed against TLC)
  was required first, before any of this work. **A first pass of this task
  reported 1,247 obligations from a run that did not, in fact,
  reproduce** — caught by an independent adversarial review that re-ran
  `tlapm` fresh and found one real failing obligation; fixed with a
  different proof strategy for that one step (`ExceptSegmentDelVer`,
  field-by-field `EXCEPT`-membership rather than a literal-record-equality
  detour that looked like it worked but was not reliable), independently
  re-verified twice before that entry was corrected to 1,261 — full
  account in `verification/README.md`'s "Lessons" section and RFC 0002's
  own Discussion addendum. **This latest pass** added the two reader-path
  step lemmas named above; doing so required extending `IndInv1` with a
  new seventh conjunct, `PtrVersionBounded` (the reader-side counterpart
  of the writer-side `BaseVersionBounded` conjunct the first pass already
  needed), and re-proving its preservation across the five writer
  theorems too, not only the two new reader ones — the 2,018 total
  reflects all of that, not just the two new theorems in isolation. A
  genuinely new failure mode surfaced and was fixed during this pass:
  proving `x' \in a..b` for a primed expression from `<=`/`>=` facts about
  the corresponding unprimed expression plus a separate equality reliably
  failed even though the identical shape works for a bare `<=` goal —
  full account in `verification/README.md`'s "Lessons" section, new
  bullet. **This latest pass** added the two remaining theorems named in
  `verification/README.md`'s own "What is explicitly not yet proved"
  section as of the prior pass: `NextStep1` (`ASSUME IndInv1, [Next]_vars
  PROVE IndInv1'`, assembling the eight per-action step lemmas into one
  fact about `manifest.tla`'s actual `Next`, confirmed by reading its real
  definition rather than assumed) and `TemporalInvariance` (`Spec =>
  []IndInv1`, `manifest.tla`'s actual `Spec == Init /\ [][Next]_vars`
  lifted to hold at every reachable state of an actual run via TLAPS's
  `PTL` backend — this file's first use of `PTL`). Confirmed by a clean
  `tlapm` run reporting `[INFO]: All 2073 obligations proved.`, exit code
  0, reproduced identically on two separate fresh runs (one ordinary, one
  with `--cleanfp`). A genuinely new `PTL`-specific failure mode surfaced
  and was fixed during this pass: the temporal `QED` step failed on its
  first full-module run because the step lemma's own `vars` operator was
  not unfolded alongside `Spec` at the point `PTL` was cited, so the two
  `[Next]_<...>` action formulas it needed to line up were not
  syntactically identical; fixed by adding `vars` to the same `DEF`
  clause — full account in `verification/README.md`'s "Lessons" section,
  new bullet. `manifest.tla` itself needed no changes for this pass, as
  expected. **Still not done**: the model's other six TLC-checked
  invariants, most notably `NoOverlappingRowIds` (needs a materially
  harder segment-packing inductive strengthening not yet attempted). Full
  accounting: `verification/README.md`'s "TLAPS proof" section, RFC
  0002's Discussion section, `docs/ledger.md`. M3-2 is not yet complete,
  and the compaction gate below still needs the remainder of this item
  (the one item named as not yet done, immediately above) plus M3-3
  (below, done).
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
  separation, before any conformance vectors are built. Status: open, RFC-sized —
  **the concrete design and the open question's resolution are D-7, below**
  (design produced, independently adversarially reviewed, and corrected against
  every finding). Depends on: nothing technically, but real log/code corpora
  (D-2) make its own conformance-vector work concrete rather than speculative.
- **D-2** — Source and license-audit real log and source-code corpora to ground D-1's
  conformance vectors and future benchmarks, the same discipline this project already
  applied to MS MARCO (fetched for `bench/`, license-checked, never committed — see
  below) and the RaBitQ/FastLanes reference implementations (license-audited via
  GitHub's license API before adoption). **Research pass completed 2026-08-20**,
  live-verified against primary sources rather than remembered, per `CLAUDE.md` §3.

  **Logs: LogHub (`github.com/logpai/loghub`) researched, real licensing
  complication confirmed, not merely suspected.** GitHub's own license API
  classifies the repository `license.key: "other"` / `spdx_id: "NOASSERTION"` — not
  a recognized OSI license. The repository's actual `LICENSE` file (fetched
  byte-for-byte, 2026-08-20) reads: "The datasets are freely available for research
  or academic work, subject to the following condition: For any usage or
  distribution of the loghub datasets, please refer to the loghub repository URL...
  and cite the following loghub paper... The above license notice shall be included
  in all copies of the datasets." This is a research/academic-use grant with a
  mandatory attribution condition — not a permissive redistribution license, and not
  Apache-2.0-compatible under any reading. The dataset's own Zenodo archival record
  (`zenodo.org/records/8196385`, the exact host every download link in LogHub's
  README points to) genuinely complicates rather than clarifies this: its structured
  metadata field reports `license.id: "cc-by-4.0"`, but the human-readable
  description on that same record restates the LICENSE file's more restrictive
  "research or academic work" text verbatim, condition and all. These two signals
  from the same publisher conflict — CC-BY-4.0 would permit commercial
  redistribution with attribution, the description's own text does not — and this
  pass does not resolve that conflict in LogHub's favor; it is reported as genuinely
  ambiguous, per this task's own instruction not to pick the convenient reading.
  Checked per this task's specific instruction not to assume the aggregator's
  top-level license covers every inner dataset uniformly: the per-dataset READMEs
  for `Apache/`, `HDFS/` (all three of `HDFS_v1`, `HDFS_v2`, `HDFS_v3_TraceBench`),
  and `OpenSSH/` were each fetched directly — none carries a dataset-specific
  license override; each states only "the raw logs are available for downloading at
  github.com/logpai/loghub" and repeats the same citation requirement. **No
  individual dataset inside LogHub is verifiably cleaner than the aggregator's own
  restrictive grant** — the task's hoped-for "name that specific one explicitly"
  outcome does not hold; stated honestly rather than softened. The successor
  `logpai/loghub-2.0` (ISSTA'24, the paper LogHub's own README also asks to be
  cited) carries the identical GitHub API classification (`"other"` /
  `"NOASSERTION"`); its own README was not individually re-fetched dataset-by-dataset
  the way the original LogHub's was, so that repo's per-dataset situation is assumed
  structurally identical, not independently re-verified to the same depth.

  **Real alternative researched and confirmed, as this task's own fallback
  instruction anticipated.** `github.com/mingrammer/flog` — a real, actively
  maintained (1,329 stars, last pushed 2025-06-05) fake-log generator — is
  MIT-licensed, confirmed both via GitHub's license API (`license.key: "mit"`) and
  by fetching its `LICENSE` file byte-for-byte (standard MIT text, copyright
  mingrammer 2018). MIT is already an accepted-compatible license in this project
  (the same class of check already applied to tantivy and FAISS, `CLAUDE.md` §1).
  `flog` generates genuinely real log *formats*, not an invented one: Apache Common
  Log Format, Apache Combined, Apache error log, RFC 3164 and RFC 5424 syslog, and
  JSON — covering D-1's stated needs directly (real timestamps, real
  key=value-shaped structure in JSON mode, real syslog framing). Because a run of
  `flog` is *our own* generated output from an MIT-licensed tool, not a
  redistribution of anyone else's copyrighted log content, its output carries zero
  third-party redistribution risk and can be committed directly into
  `conformance/analyzers/` — unlike anything sourced from LogHub.

  **Recommendation, split by where the data lands, not a single verdict.** LogHub
  is real, current (pushed as recently as today per its GitHub API `pushed_at`),
  and directly on-domain (Hadoop, Spark, HDFS, Android, OpenSSH, Apache — real
  system and application logs, unmodified per its own README's "NOT sanitized,
  anonymized or modified" claim) — genuinely useful for **bench-only grounding**:
  fetched at `bench/` build time into a gitignored location exactly like MS MARCO's
  existing `/bench/data` (never committed to this Apache-2.0 repository, so its
  research/academic-use restriction never actually collides with the repo's own
  license), cited per its attribution requirement in the bench source itself. Named
  candidates for that use, chosen for domain fit and moderate size: `Apache` (error
  log, 56,481 lines, 4.90 MiB), `OpenSSH` (655,146 lines, 70.02 MiB), and `HDFS_v1`
  (labeled with a real anomaly ground truth, 11,175,629 lines, 1.47 GiB) — all three
  figures read directly from LogHub's own README table, 2026-08-20. LogHub is
  **not** recommended for anything landing in `conformance/`, since those files are
  committed into this Apache-2.0-licensed repository and redistributed to every
  downstream user, which LogHub's own LICENSE does not clearly permit. For that
  committed-conformance-vector use, **`flog`'s own generated output is the
  recommendation** — deterministic, license-clean, and already covering the real
  formats D-1 needs.

  **Code: Rust standard library (`rust-lang/rust`, `library/core` + `library/std` +
  `library/alloc`) is the recommendation, verified live rather than assumed.**
  GitHub's license API reports the whole repository as `apache-2.0`; the repo's own
  `COPYRIGHT` file (fetched directly) states the real, more precise claim: "The
  Rust Project is dual-licensed under Apache 2.0 and MIT terms... at your option,"
  with the caveat "Except as otherwise noted." That caveat was chased down rather
  than glossed over: the project's own machine-checked license ledger,
  `REUSE.toml` (fetched at tag `1.97.1`, the newest stable numeric release tag as of
  this pass), sets a blanket `SPDX-License-Identifier = "MIT OR Apache-2.0"` over
  `library/**` (among other paths), confirming the dual-license claim for the
  subtree as a whole — but with exactly two documented per-file exceptions inside
  that same subtree: `library/core/src/unicode/unicode_data.rs`
  (`SPDX-License-Identifier = "Unicode-3.0"`, Unicode, Inc.'s own generated
  character-property-table license — a separately permissive license, but not
  itself Apache-2.0) and `library/std/src/sys/sync/mutex/fuchsia.rs`
  (`SPDX-License-Identifier = "BSD-2-Clause AND (MIT OR Apache-2.0)"` — compatible
  by combination, not solely Apache-2.0). A real vendoring pass MUST exclude or
  separately attribute these two named files rather than assume the blanket
  `library/**` claim covers them; this is exactly the "no exceptions, verify
  carefully" discipline `CLAUDE.md` §1 already applies to RaBitQ and FastLanes,
  applied here to a fixed source snapshot instead of a code dependency. Real,
  live-measured scale at tag `1.97.1` (`git/trees` API, recursive, restricted to
  `library/core/`, `library/std/`, `library/alloc/`, blobs only): **1,097 files,
  13,977,908 bytes (≈13.3 MiB)** — real numbers from this pass, not estimated.
  **Reasoning for the recommendation**: dual MIT/Apache-2.0 (strictly cleaner than
  either MIT-only or the code corpora below), large enough for real conformance
  work without needing a scale-shrinking sample, idiomatic production Rust with
  heavy real doc-comment density (directly useful for D-1's own "prose-shaped spans
  embedded inside code-shaped tokens" problem statement, since `std`'s doc comments
  are exactly that shape at scale), and a fixed, citable tag rather than a moving
  branch.

  A second real candidate, already in this project's own dependency graph rather
  than an outside pick, is named but not recommended as the primary choice:
  `quickwit-oss/tantivy` — confirmed MIT via GitHub's license API, already a direct
  `strand-tools` dependency (`tantivy = "0.26.1"`, `crates/strand-tools/Cargo.toml`)
  ahead of M4's tantivy-fork work. It is a real, idiomatic Rust codebase this
  project already builds against, so a fixed snapshot would double as grounding
  code this project's own tooling already touches — a strong second option,
  particularly if future work wants code that resembles a search engine's own
  source. Not the primary pick because it is MIT-only (compatible, not dual) and
  smaller than the standard library.

  Two aggregator-style code datasets were checked live and ruled out for a first
  vendored subset, for the same class of reason LogHub was: **CodeSearchNet**
  (`github/CodeSearchNet`) aggregates code across many repositories, each keeping
  its own source repository's license; the project's own construction only removed
  repositories with no license or a license that didn't "explicitly permit
  redistribution" — it does not filter down to Apache-2.0 or even a small permissive
  set, so it reproduces the exact aggregator-license trap this task warned about,
  confirmed via the dataset's own repository and Hugging Face dataset card rather
  than assumed. **The Stack v2** (BigCode) does apply real upstream license
  filtering — repositories are included only if they carry a license the Blue Oak
  Council list or ScanCode's "Permissive"/"Public Domain" categories recognize —
  but BigCode's own documentation states plainly this filtering is repository-level
  and heuristic, not a file-level guarantee, and admits multiple permissive
  licenses (MIT, BSD, Apache-2.0, others), not uniformly Apache-2.0; an independent
  paper, "Cracks in the Stack: Hidden Vulnerabilities and Licensing Risks in LLM
  Pre-Training Datasets" (arXiv:2501.02628, found via live search, not cited from
  memory), documents real licensing risk in exactly this class of dataset,
  corroborating rather than just asserting the concern. At 67.5 TB full scale it is
  also wildly disproportionate to what a first conformance/benchmark pass needs. The
  Stack v2 is not ruled out permanently — it is a real fallback if a broader
  multi-repository, multi-language corpus is ever needed past one fixed-snapshot
  repository — but it is not this pass's recommendation.

  **Concrete next step (not this task's own job — this task is research and
  recommendation only, per its own scope):** (1) for the log corpus, download
  `Apache.tar.gz`, `SSH.tar.gz` (OpenSSH), and `HDFS_v1.zip` from LogHub's Zenodo
  record (`zenodo.org/records/8196385`) into a gitignored `bench/data`-style
  location, with the LICENSE attribution string reproduced in the fetching bench
  source, mirroring `bench/src/msmarco_index.rs`'s existing pattern exactly; (2) run
  `flog` (pinned version, pinned seed/flags for determinism) to generate the actual
  committed `conformance/analyzers/` golden files for log-shaped analysis; (3) for
  the code corpus, `git clone --branch 1.97.1 --depth 1
  https://github.com/rust-lang/rust`, sparse-checkout `library/core`,
  `library/std`, `library/alloc`, and carry `REUSE.toml`'s two named exceptions as
  explicit per-file attribution notes rather than folding them into a blanket
  Apache-2.0 claim. None of this — the actual download, vendoring, or
  `references/`-style write-up — is done by this pass; this entry is the research
  and recommendation only, matching this item's own original scope.

  Status: **research complete, recommendation made 2026-08-20** — LogHub (bench-only,
  named datasets above) plus `flog` (MIT, for committed conformance vectors) for
  logs; Rust standard library `library/core`+`library/std`+`library/alloc` at tag
  `1.97.1` (dual MIT/Apache-2.0, two named per-file exceptions) for code. Actual
  vendoring (the concrete next step above) remains open, unblocked, and is real,
  disk-space-bearing follow-on work for a later task, not this one. Depends on:
  nothing; unblocks D-1's real conformance-vector work and D-3's real
  code-embedding grounding.
- **D-3** — Revisit the vector family's embedding conventions against real
  code-embedding models rather than assumed-generic ones. The graph-blob family's own
  real `bench/` measurement (`bench/src/graph_warm_query.rs`, M2-3 above) used
  synthetic uniform-random points precisely because no real, domain-representative
  embedding source was available at the time — a real code-embedding model run over
  D-2's real code corpus would let a follow-up benchmark measure this family's actual
  cost on structured, clustered data instead of a near-worst-case synthetic
  distribution, closing the gap this project's own conversation about that benchmark
  already named directly. Status: **closed by D-8**, below — a real, licensable,
  locally-runnable code-embedding model was found, D-2's real corpus was embedded
  with it, and the graph-blob benchmark was re-run against the result. Depends on:
  D-2.
- **D-4** — Vendor telemetry-adjacent and code-search-adjacent prior art into
  `references/` and `docs/lineage.md`, the same discipline every other design
  lineage entry in this project already follows (a primary source, fetched and
  cited, not asserted from memory, `CLAUDE.md` §3). **Status: done, 2026-08-20.**
  Two candidates were vendored, both re-fetched live and independently re-verified
  in the session that closed this item rather than trusted from any earlier
  conversational summary.

  **NOFireAI/ravel** — `references/nofireai-ravel-storage-architecture.md`.
  License independently re-confirmed Apache-2.0 via GitHub's license API (matching
  the method already used for RaBitQ and FastLanes). Real README, `docs/catalog-
  and-mvcc.md`, `docs/consistency-model.md`, and `docs/object-store-contract.md`
  fetched and quoted; the specific technical claims from prior conversational
  survey (a `prefix_list_crossover_requests` default of 720 hour-buckets, a
  `fold_reconcile_window_hours` default of 26, "sealed hours" and the "seal lemma"
  terminology) were all independently re-verified — the numeric defaults confirmed
  directly against real, currently-committed Rust source
  (`crates/ravel-catalog/src/config.rs`, lines 85 and 124). One correction found
  and recorded: the prior summary attributed both defaults to `crates/ravel-
  catalog/src/fold.rs`; live re-verification found the fields are actually declared
  in `config.rs` and the `prefix_list_crossover_requests` switch itself is consumed
  in `crates/ravel-catalog/src/catalog.rs` (line 976) — `fold.rs` only references
  the config values, it doesn't define or switch on them. `docs/lineage.md` gained
  a real entry positioned among the living object-storage-first systems (Iceberg,
  Lance, turbopuffer), explicitly not the graveyard, stating what STRAND shares
  (immutable segments, a single CAS'd current pointer, the same commit-protocol
  shape) and does not share (ravel is a full telemetry datastore with its own query
  engines; STRAND is a storage format only) with it.

  **Zoekt** (`sourcegraph/zoekt`) — `references/zoekt-code-search-engine.md`.
  License independently confirmed Apache-2.0 (both the GitHub API and the actual
  `LICENSE` file text), including the real fork history (Sourcegraph's copy has
  been the maintained source since 2017, forked from `google/zoekt`, itself
  Apache-2.0 — no relicensing found in the fetched material). Real README and
  `doc/design.md`/`doc/ctags.md`/`doc/faq.md` fetched and quoted. Granularity
  finding: Zoekt indexes at **file granularity** — a shard's data is file content,
  filenames, and posting lists over both; symbol positions come from an external,
  sandboxed `ctags` invocation used only as a ranking signal, never as a persisted,
  addressable identity across a re-index. This **independently confirms**
  `docs/ledger.md`'s existing "code row-IDs stay file-granular" settled entry and
  its characterization of Zoekt as recomputing symbol positions wholesale with no
  persisted cross-reindex identity — no correction was needed to that entry; a
  short confirming addendum citing the new reference was added to it instead.
  `docs/lineage.md` gained a Zoekt entry as code-search-specific prior art,
  distinct from ravel's storage-layer comparison.

  No third candidate was added: neither fetch surfaced a clearly better-fitting
  system than these two, and `CLAUDE.md` §2's economy rule argues against padding
  this entry with weaker candidates just to have more of them.
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

- **D-6** — Optional per-segment range-pruning summary stats, resolving
  `docs/ledger.md`'s R10 question ("should the manifest carry optional
  per-segment summary metadata... so a reader can prune segments before
  opening them") for the one concrete, generic case the logs domain motivates
  directly: an ordered field — a log ingest timestamp, most concretely — whose
  per-segment `[min, max]` range lets a reader skip opening segments a query's
  range predicate cannot match. Like D-5, this is this session's own design
  exploration (research → design → independent adversarial critique)
  surfacing a real candidate, not one of the domain-scoping decision's four
  originally-named items. The critique found the GET/byte arithmetic case
  genuinely strong at measured scale but flagged real problems before the
  design would be RFC-ready. This entry is the design corrected against all
  of them, re-verified here directly against the actual repo — `spec/
  manifest.md`, `spec/container.md` §5a, `rfcs/0001-container-rowid-
  manifest.md`'s real Discussion section, `docs/ledger.md`'s R10 entry, and
  `docs/lineage.md`'s Pilosa entry — rather than trusted from the critique's
  own summary.

  **Mechanism.** `TableMetadata` (`spec/manifest.md` §1, written once via a
  single `put_if_absent`, immutable thereafter except the CAS-host move)
  gains one new optional field, `range_prunable_fields: array of
  RangePrunableField`, default empty:

  | field             | type  | notes                                                        |
  | ----------------- | ----- | -------------------------------------------------------------- |
  | manifest_field_id | u64   | assigned sequentially from 1 at table-creation time; 0 reserved, unused |
  | field_name        | string | the declared field's name, raw UTF-8, no normalization        |
  | value_type        | string | `"i64"` only, this pass                                       |

  `SegmentRef` (`spec/manifest.md` §1) gains one new optional field,
  `summary_stats: array of FieldRangeStat`, default empty:

  | field             | type | notes                                       |
  | ----------------- | ---- | -------------------------------------------- |
  | manifest_field_id | u64  | keys into `TableMetadata.range_prunable_fields` |
  | min               | i64  | minimum value of the declared field over this segment's rows |
  | max               | i64  | maximum value of the declared field over this segment's rows |

  At commit, a writer that has declared a field range-prunable computes that
  field's min/max over the segment being appended (already touching the data
  during indexing) and attaches the resulting `FieldRangeStat` to the
  proposed `SegmentRef`. Reader protocol (`spec/manifest.md` §3) gains one
  new step, inserted before today's step 1, done at most once per reader
  session: if the reader intends to exploit range pruning, `GET
  _strand/metadata.json` once and cache `range_prunable_fields` — safe to
  cache indefinitely, since this list is part of the write-once object and
  this pass defines no amendment path for it (an explicit, named limitation,
  unlike the CAS-host move, which the spec does define an amendment path
  for) — and resolve the query's declared field name to its
  `manifest_field_id`. A new step 2.5 is inserted between today's step 2
  (`GET` snapshot metadata) and step 3 (open segments): filter `segments` to
  those whose `summary_stats` contains an entry for the resolved
  `manifest_field_id` whose `[min, max]` intersects the query's range
  predicate; a segment with no matching `summary_stats` entry — an older
  segment predating the feature, or one whose writer didn't compute stats for
  this field — is conservatively kept. No new round trip is added to the
  cold-open sequence itself: the snapshot metadata `GET` a reader already
  makes at step 2 just carries more bytes, and the one new
  `_strand/metadata.json` fetch is a one-time, cacheable session cost, not a
  per-query one — a real, precise correction to the original design's
  unqualified "no new round trip" claim, which didn't name this one-time
  fetch at all.

  **Worked example.** Four daily segments (`seg-2026-08-17` through
  `seg-2026-08-20`, 3,200 rows each), `event_timestamp_millis` declared
  range-prunable (`manifest_field_id = 1`). Query: `timestamp BETWEEN
  2026-08-19T06:00 AND 2026-08-19T18:00`. Today: all 4 segments opened, 6
  cold GETs (`2 + segment_count`). Proposed: only `seg-2026-08-19`
  intersects, 3 cold GETs (`2 + 1`). Stats overhead added to the snapshot
  metadata `GET`: 4 segments × 24 bytes/`FieldRangeStat` = **96 bytes** (one
  declared field; `FieldRangeStat` is `manifest_field_id: u64` + `min: i64` +
  `max: i64` = 8 + 8 + 8 = 24 bytes — the original design's "16 bytes" figure
  silently dropped `manifest_field_id`/`field_id` from the count, corrected
  here).

  Napkin math at measured scale (128 segments, 4 relevant), re-derived here
  against the real M3-7 baseline (`bench/results/multi-segment-query-
  partial.json`, read directly: `"segment_count": 128, "cold_get_count":
  130, "cold_bytes_fetched": 1291500`). GETs: today 130 (`2 + 128`); proposed
  6 (`2 + 4`) — a reduction of 124 GETs, **95.4%** (`124 / 130 = 0.9538`,
  unaffected by the byte-size correction). Bytes: average per-segment
  open cost is `1,291,500 / 128 = 10,089.84` bytes; 4 relevant segments cost
  `4 * 10,089.84 ≈ 40,359.4` bytes to open. Stats overhead at this scale,
  corrected: `128 * 24 = 3,072` bytes (not the original design's `128 * 16 =
  2,048`) added to every snapshot `GET` regardless of how many segments a
  given query matches — a real, small, always-paid cost, negligible against
  the 1.29 MB baseline but not zero, and it scales linearly with the number
  of declared range-prunable fields. Total proposed bytes: `3,072 + 40,359 ≈
  43,431`; reduction from the 1,291,500-byte baseline is `(1,291,500 -
  43,431) / 1,291,500 = 0.96636`, **≈96.6%** (the original design's
  undercounted math gave ≈96.7% off a smaller, wrong overhead figure — the
  qualitative conclusion is unchanged, the number is corrected). Latency is
  explicitly **not** estimated, for the same reason `docs/roadmap.md`'s own
  M3-7 entry above gives: GET-count and latency did not scale linearly in
  that real run (16→128 segments: 7.2x the GETs, 9.6x the p50 latency), so a
  real re-measurement is owed, not invented, once this mechanism is actually
  built.

  **Fix 1 (byte-math error).** Corrected throughout above:
  `FieldRangeStat` is 24 bytes, not 16 — the original design's field-by-field
  breakdown counted only `min`/`max` and silently dropped the identifier
  field from its own stated three-field struct. Every downstream figure
  (worked example, at-scale bytes, at-scale percentage) is recomputed from
  the corrected size, not just re-stated with a caveat.

  **Fix 2 (precedent count).** The original design claimed SegmentRef's
  schema had "already been amended post-approval through this RFC's
  Discussion section four times," cited as evidence of low invasiveness.
  Checked directly against `rfcs/0001-container-rowid-manifest.md`'s real
  Discussion section (the only place post-approval amendments are recorded):
  four items are named there — Task X-1, M3-4, M3-5, and Task X-2 — and none
  of the four amends `SegmentRef`. X-1 (2026-08-19) adds `field_id` to
  `blob_entry`, a `spec/container.md` §5a segment-registry structure, not the
  manifest at all. M3-4 (2026-08-19) adds `committed_at_millis` to
  `SnapshotMetadata` — the wrapper object that contains `segments: array of
  SegmentRef`, not `SegmentRef` itself — and separately resolves how
  `RetentionPolicy`'s two fields combine (the union reading), a **semantics**
  clarification, not a new field: the RFC's own text states plainly
  "`TableMetadata`/`RetentionPolicy` are unmodified" by that resolution,
  correcting even the critique's own looser framing of this item as a
  "resolution... landed on RetentionPolicy." M3-5 makes no wire-format
  change at all — the retention window becomes a `sweep_orphans` call
  parameter, explicitly not a `TableMetadata` field, per the RFC's own text
  quoted above. Task X-2 closes three open questions (tail-read size,
  404-refresh retry bound, suffix-range support) with no schema change of any
  kind. The real count: `SegmentRef` has been amended exactly **once**, by
  RFC 0012 (`spec/manifest.md` §1, `rfcs/0012-deletion-vectors.md` Design §1
  and Discussion), adding the optional `deletion_vector` field. This design
  would be the second. "Low-to-moderate invasiveness" is still a fair
  characterization of amending a manifest object that has already absorbed
  one optional, additive field cleanly — but on one real precedent, not four
  fabricated ones, and RFC 0001 itself gains no new precedent from this
  entry's correction; the accurate citation for the real precedent is RFC
  0012, not RFC 0001.

  **Fix 3 (index-internals-agnostic tension, resolved).** The original
  design keyed `summary_stats` by the segment-internal `field_id` — the
  `xxHash3-64` hash of a field's declared name, defined in `spec/
  container.md` §5a for exactly one purpose: letting a reader that already
  knows a field's name locate that field's blobs *within one segment's own
  blob registry* without a name-to-ID lookup. Checked directly against
  `spec/manifest.md` §1's own normative text — "No blob-family-specific
  fields belong in `SegmentRef`; a family's own internal state lives inside
  the segment... not the manifest" — and `docs/ledger.md`'s R10 entry, which
  states any summary mechanism "must stay index-internals-agnostic per
  `CLAUDE.md` §6": reusing container-internal `field_id` at the manifest
  layer means the manifest's own schema becomes interpretable only by a
  reader that also understands `spec/container.md` §5a's hashing scheme — a
  new cross-chapter coupling the manifest has never had before. `field_id`
  is not itself a blob-family-specific structure (it is used the same way
  across every family), so the letter of the "no blob-family-specific
  fields" rule is arguably satisfied either way; but the *spirit* — the
  manifest needing no knowledge of what a segment's own footer defines,
  matching `index_versions`'s existing, deliberately coarse and opaque
  precedent for referencing family internals — is not. This design resolves
  the tension by **not** reusing container `field_id` at all:
  `manifest_field_id` is a distinct, manifest-native identifier, declared
  once in `TableMetadata` (above) and never derived from, or requiring
  knowledge of, any segment's internal hashing scheme. This is the option
  the critique leaned toward, and it is the one actually adopted here — a
  real design decision, not left argued-but-unsettled. It has a second,
  independent point in its favor beyond closing the internals-agnostic
  question: because `TableMetadata` is written exactly once via a single
  `put_if_absent` (`spec/manifest.md` §1), `manifest_field_id` assignment
  happens exactly once, ever, per table — there is no multi-writer
  coordination race to reason about the way there would be if field
  declarations lived in `SnapshotMetadata` (rewritten fresh on every commit,
  `spec/manifest.md` §1), which would need a new append-only-across-commits
  invariant this design avoids needing at all.

  **Fix 4 (RFC 0004 misattribution, corrected).** The original design cited
  RFC 0004/invariant 6 as covering "the 'declare field X as range-prunable'
  plumbing." Checked directly against `rfcs/0004-analyzer-descriptors.md`
  and `CLAUDE.md` §5 invariant 6: both are specifically about text-analysis
  conformance for lexical blobs (tokenizer profile, stemmer, UAX #29
  deviations, segmentation-dictionary identity) — a timestamp field's
  range-prunability has nothing to do with analyzer descriptors, and the
  citation is removed rather than repeated. There is currently no settled,
  per-field, non-lexical metadata mechanism anywhere in this project;
  `TableMetadata.range_prunable_fields` and `SegmentRef.summary_stats`
  (above) are real new design surface an eventual RFC must specify from
  scratch, closer in spirit to RFC 0003's small, typed, declared-once
  scoring-profile-parameter mechanism than to RFC 0004's per-blob descriptor
  mechanism, but neither is a drop-in precedent to lean on.

  **Fix 5 (grave, corrected).** The original design named BitFunnel
  (`docs/lineage.md`: "a hardware-profile bet, published with strong
  numbers, adopted by nobody") as its nearest grave, arguing this mechanism
  is distinguishable as "additive/optional/generic rather than
  structural/all-or-nothing." Checked against `docs/lineage.md` directly:
  that argument is true but the comparison was never sharp — `summary_stats`
  bakes in no hardware assumption whatsoever (a plain `i64` min/max pair), so
  BitFunnel's specific failure mode never really applied here. The nearest
  real grave is Pilosa: "a good structure with a spec is not a distribution
  strategy" (`docs/lineage.md`). This mechanism's entire payoff is
  contingent on writer discipline the format cannot enforce — time-clustered
  commits, or more generally, commits where a segment's declared field
  actually has a narrow range rather than one spanning most of the table's
  history. A writer that batches indiscriminately (interleaving old and new
  timestamps into every segment) produces segments whose `[min, max]` each
  span nearly the whole table; every query's predicate then intersects
  nearly every segment's range, and the mechanism degrades silently to
  today's baseline (open everything) with **no reader-visible signal that it
  happened** — a query just gets no faster, with nothing in the manifest or
  the response telling an operator why. This is exactly Pilosa's shape: a
  well-specified, structurally sound mechanism whose real-world payoff is
  entirely a deployment-discipline question the spec text cannot answer, and
  can silently fail to answer, in production. Naming this honestly doesn't
  kill the design — daily- or hourly-batched log ingestion (D-5's own
  motivating assumption) is a genuinely common, plausible writer pattern —
  but an eventual RFC needs a real "how this could be wrong" treatment built
  around this specific risk, and probably a reader-facing diagnostic (for
  example, `strand-tools inspect` reporting each declared field's
  per-segment range width relative to the table's overall range, so an
  operator can see degradation directly instead of inferring it from query
  latency) as part of its own scope, not an afterthought.

  **Blast radius, corrected.** Invariant 1 (`CLAUDE.md` §5) is untouched:
  `summary_stats` is not a registered blob family, so it needs no declared
  row-ID merge strategy; its own composition rule under a future segment
  merge is simply min-of-mins/max-of-maxes over the two input segments'
  already-computed `FieldRangeStat` entries, requiring no data rescan — a
  real, welcome property for whenever M3-1 compaction lands, though M3-1
  remains unbuilt (`docs/roadmap.md`, confirmed still open) and this
  composition claim is therefore unexercised, not measured. Invariant 3 (the
  one-wave rule) is untouched: pruning happens entirely before any
  segment-open request. Invariant 4 (`CLAUDE.md` §5) is **not** a literal
  generalization the way the original design claimed ("generalized one level
  up... from its current postings-block-granularity scope") — invariant 4 is
  specifically scoped to codec-independent, scoring-independent postings
  pruning (block-max bounds). `summary_stats` shares its spirit — raw
  statistics, stored as sibling metadata beside the structure they describe,
  never a precomputed score — but operates over an arbitrary declared
  `i64`-orderable field at segment granularity in the manifest, a different
  layer invariant 4 never claimed to cover; stated here as spirit-consistent,
  not as an instance of an already-settled invariant, correcting the
  original design's overstatement. `verification/manifest.tla`'s
  `SegmentRec` abstraction likely needs no change: `summary_stats` is
  written once per segment at initial commit (the `commit` append path)
  and never revised via `commit_deletion_vector`'s revise path, the same
  shape as `SegmentRef`'s other already-modeled write-once fields
  (`row_id_base`, `row_id_count`, `checksum`) — following the same reasoning
  RFC 0001's own M3-4 entry used to justify `committed_at_millis` needing no
  TLA+ model change ("`SnapshotRec` already abstracts away `path`,
  `checksum`, and `byte_length` as content its safety properties don't
  depend on"). This is a real, low-cost recheck still owed, not yet done —
  named honestly as unstarted work, not claimed complete. RFC 0012 (the real
  `SegmentRef` precedent, Fix 2) is the right target for a `SegmentRef`
  amendment RFC to cite, not RFC 0001.

  Nearest live prior art for the future-escalation direction: NOFireAI/
  ravel's snapshot resolution switches from a per-bucket LIST loop to a
  prefix-scan strategy once a query window crosses
  `prefix_list_crossover_requests` (default 720 suffix buckets) — now a
  real, vendored citation (`references/nofireai-ravel-storage-
  architecture.md`, `docs/roadmap.md` D-4, **done**, independently
  re-verified against `crates/ravel-catalog/src/config.rs` line 124), not
  the unvendored conversational claim the original design rested on. This is
  a real, if structurally different, precedent for "switch strategy once a
  measured count threshold is crossed," named here as a future escalation
  path if segment counts grow past whatever crossover a future STRAND
  benchmark finds — not a present dependency this design relies on, since
  this pass scopes itself to the tens-to-hundreds segment-count range the
  real M3-7 measurement above actually covers.

  Status: **open, RFC-sized, design revised after independent adversarial
  review** — not yet an approved or implemented RFC; a corrected pre-RFC
  design ready for someone to actually draft. Depends on: nothing to begin
  drafting; real validation of the compaction-composition claim above is
  coupled to M3-1 landing, and a real multi-segment pruning benchmark (an
  actual measured latency figure, not the explicitly-not-estimated one
  above) is coupled to D-1/D-2's real log corpora with real timestamps and
  to M3-7's still-open full, post-compaction version.

  **Next step**: draft this as a real numbered RFC — `ls rfcs/` shows 0001
  through 0014 already allocated on disk, and `docs/roadmap.md`'s own D-5
  entry above claims **0015** for the `bm25-recency` RFC (not yet drafted,
  so not yet on disk, but spoken for) — so the next free number for this
  design is **0016**, once someone picks this up, following `CLAUDE.md` §3's
  design-then-implementation separation.

- **D-7** — The concrete logs-and-code analyzer profile D-1 itself asks for:
  identifier splitting, log-line structure, and a resolution of D-1's own
  named open question (RFC 0004 schema change vs. a document-level
  field-splitting convention for mixed code/comment content). Like D-5 and
  D-6, this is this session's own design exploration (research → design →
  independent adversarial critique) — the critique found the structural
  argument for the span-question resolution sound and every non-`§6.1`
  citation (tree-sitter's real node types, Lucene's real `WordDelimiterIterator.
  isBreak()`, Elasticsearch's real `word_delimiter_graph` docs, tantivy's real
  `text_options.rs`, `flog`'s real format strings, the `manifest.rs` excerpt,
  RFC 0004's real schema, `field_id` mechanics) accurate, but found three
  Critical citation-hygiene defects inside the design's own primary worked
  example (§6.1) — a fabricated quote, a false "checked against the real
  list" claim, and unverified stemmer output presented as verified — plus two
  Important framing gaps and one grave-selection finding. This entry is the
  design corrected against all nine findings, each re-verified here directly
  (live re-fetches, direct file reads, and one live computation against the
  authoritative Snowball implementation) rather than trusted from either
  prior agent's summary, per `CLAUDE.md` §3.

  **Resolving D-1's own open question: option (b), a document-level
  field-splitting convention, not an invariant-6 or per-document-length
  schema change.** Mixed code+comment content is split into separate,
  independently-named, independently-analyzed sub-fields at index time —
  `content.code` and `content.comment`, or any other pair of names — using
  tree-sitter's real, live-confirmed structural node types (`block_comment`,
  `line_comment`, `doc_comment`, `inner_doc_comment_marker`,
  `outer_doc_comment_marker`, confirmed live against
  `tree-sitter/tree-sitter-rust`'s `node-types.json`) to decide the byte
  ranges. This needs no amendment to invariant 6's text and no amendment to
  RFC 0004's per-document-length definition (`dl := Σ_{i∈V} tf_i`, written
  singular-per-field): `spec/container.md` §5a's `field_id` mechanism (a
  `u64`, `xxHash3-64` over any raw-UTF-8 field name, no registry, no
  writer coordination) already makes an arbitrary number of independently
  analyzed fields free today. Two engines this project's own lineage already
  draws from confirm the pattern rather than inventing it: Lucene's real
  `PerFieldAnalyzerWrapper` and tantivy's real `TextFieldIndexing::
  set_tokenizer` (`Cow<'static, str>`, one tokenizer per field) both dispatch
  analysis strictly by field name — neither engine has ever built per-span
  analyzer mixing within one field. Zoekt is prior art for *caring* about the
  code/comment distinction (`doc/design.md`'s "Ranking" section lists
  "tokenizer ranking: does a match fall comment or string literal?" among its
  named ranking signals) but not for *how* to structure an index around it —
  correcting the original design's overclaim, the vendored excerpt
  (`references/zoekt-code-search-engine.md`) states this signal exists as a
  named ranking option; it does not state whether it is computed at index
  time or at query time, and no stronger claim than that should be made from
  it (Minor finding, below). What (b) does not resolve for free: the two
  concrete tokenizer algorithms D-1 itself asks for genuinely cannot be
  expressed in RFC 0004's current `tokenizer_profile` shape, whose `algorithm`
  field has exactly one defined value, `"UAX29-word"`. Registering
  `code-identifier-v1` and `log-line-v1` as new `algorithm` values is a real,
  contained RFC 0004 Design §2 amendment, specified below.

  **Why the Han-script dictionary-segmentation amendment is not a
  counter-precedent for folding these into `UAX29-word` plus `deviations`
  (Fix, addressing the critique's first Important finding).** RFC 0004's own
  Discussion section (`rfcs/0004-analyzer-descriptors.md`, "2026-08-19 —
  CJK/Thai/Lao default `segmentation_dictionary` resolved") already
  accommodated a genuinely different tokenization *mechanism* — dictionary
  lookup via `icu_segmenter::WordSegmenter::new_dictionary`, not rule-based
  UAX #29 boundary-finding — without a new `algorithm` value, folding it into
  `algorithm: "UAX29-word"` plus a `deviations` entry and the companion
  `segmentation_dictionary` field; the real, committed vector
  (`conformance/analyzers/icu4x-dictionary-zh-01.json`, read directly) confirms
  this exactly: `"algorithm": "UAX29-word"`, `"deviations": ["Dictionary-based
  word-boundary segmentation for Han script content via icu_segmenter's
  WordSegmenter::new_dictionary, rather than plain UAX #29 rule-based
  boundaries."]`. The real, load-bearing distinction the original design
  never checked this against: the Han case still answers the same question
  `UAX29-word` exists to answer — *where are the word boundaries* — just by a
  different mechanism (dictionary lookup instead of rule-based break
  iteration) over the same kind of content (natural-language text in a
  script UAX #29's own rules handle poorly). `code-identifier-v1` and
  `log-line-v1` answer a categorically different question: not "where are the
  word boundaries in this prose," but "how do I parse this syntax" —
  delimiter- and pattern-based structural extraction (`_`/`-` splits,
  digit/case transitions, `key=value` recognition, timestamp-pattern
  extraction) over content that is not natural-language text at all.
  Stretching `deviations` — a field RFC 0004's own normative text scopes
  explicitly to "describing departures from stock UAX #29 behavior
  specifically," not a general parameter bag — to carry an entirely different
  algorithm family's parameters (`split_on_delimiters`, `timestamp_pattern`,
  `kv_pair_detection`) would be the kind of change invariant 6's own text
  does not commit to, not a same-shape extension of the Han precedent. The
  Han case is real, relevant, and now checked directly rather than missed;
  it does not change the conclusion, but the design is honest about having
  looked.

  **`code-identifier-v1`** — a new `tokenizer_profile.algorithm` value for a
  field carrying source-code-shaped content:

  | field | type | notes |
  | --- | --- | --- |
  | `algorithm` | string | `"code-identifier-v1"` |
  | `case_folding` | `"none"` \| `"lower"` \| `"full-case-fold"` | same three-value enum as `UAX29-word`'s field, reused |
  | `split_on_delimiters` | array of string | typically `["_", "-"]` |
  | `split_on_digit_boundary` | bool | reuses Elasticsearch's real, quoted `split_on_numerics` semantics: `j2se` → `j`, `2`, `se` |
  | `split_on_case_change` | bool | reuses Elasticsearch's real, quoted `split_on_case_change` semantics: `camelCase` → `camel`, `Case` |
  | `acronym_run_boundary` | bool | STRAND's own stated deviation from stock Lucene/ES default, below |
  | `preserve_original` | bool | reuses Elasticsearch's real, quoted `preserve_original` semantics |

  `stopword_list_id` and `stemmer` MUST be `null` when `tokenizer_profile.
  algorithm = "code-identifier-v1"`: Porter-family stemming collapses
  `get`/`gets`/`getting`, correct for English prose and wrong for code
  identifiers, where `get_user` and `getting_user` name different real
  symbols. Pipeline, in order: delimiter split (discarding the delimiter);
  digit-boundary split if enabled; case-transition split if enabled, with the
  `acronym_run_boundary` deviation (below) as an explicit, selectable
  addition rather than a silent default change; uniform case folding; and,
  if `preserve_original`, one additional whole-identifier token at the first
  sub-token's position — the first real, concrete producer of
  same-position-overlapping tokens under RFC 0004's own
  `counts_overlaps_in_length` mechanism, which existed in the approved RFC
  only as an anticipated hook until now. Fetched and traced live from
  Lucene's real `WordDelimiterIterator.isBreak()`
  (`apache/lucene@main`): stock Lucene/Elasticsearch's real, current default
  does not split an acronym run from a following capitalized word at all —
  every `UPPER→UPPER` transition is a same-type non-break and the one
  `UPPER→lower` transition hits the method's own `"UPPER->letter: Don't
  split"` branch. `acronym_run_boundary: true` is STRAND's one genuinely new
  rule, not a literature reuse: given a maximal uppercase run of length
  *N* ≥ 2 immediately followed by lowercase letters, insert a boundary after
  the run's (*N*−1)th character.

  Traces: `getUserById` → `get`, `User`, `By`, `Id`. `user2Name` (letter↔digit
  transitions) → `user`, `2`, `Name`. A hand-constructed acronym example,
  `HTTPServer` (`acronym_run_boundary: true`) → `HTTP`, `Server`; the same
  input under `acronym_run_boundary: false` reproduces stock Lucene/ES
  exactly — `HTTPServer` stays unsplit. **This specific example is honestly
  labeled, not silently kept: `HTTPServer` is not idiomatic Rust.** The real
  Rust API Guidelines (`rust-lang.github.io/api-guidelines/naming.html`,
  re-fetched live here), quoted exactly: "In UpperCamelCase, acronyms and
  contractions of compound words count as one word: use `Uuid` rather than
  `UUID`, `Usize` rather than `USize` or `Stdin` rather than `StdIn`." Genuine
  multi-letter uppercase acronym runs essentially don't occur in idiomatic
  Rust identifiers — the exact corpus class (idiomatic Rust) D-2 recommends
  and this design's own worked example draws from. The real grounded
  evidence is a direct grep of this repository's own non-comment Rust source,
  re-run here: `ETag` (`crates/strand-core/src/store.rs:25`) → `E`, `Tag`;
  `KMeansResult` (`crates/strand-vector/src/kmeans.rs:68`) → `K`, `Means`,
  `Result`; `UInt32Array` (an `arrow` crate import, `crates/strand-datafusion/
  src/lexical_table.rs`, not this project's own naming choice, so weaker
  evidence than the other rows, but still a real trace) → `U`, `Int`, `32`,
  `Array`; `LBracket`/`RBracket`/`LParen`/`RParen`
  (`crates/strand-core/src/bin/dst_manifest_harness/trace.rs`) → `L`/`R` +
  word, in every case; `BTreeMap` (std, imported throughout) → `B`, `Tree`,
  `Map`. All five trace correctly under the stated rule — real evidence the
  original design never ran. **The rule's honest failure mode, also named
  explicitly rather than left implicit: `SQLite` traces to `SQ` + `Lite`, not
  `SQL` + `ite` or any sensible split**, because the rule's boundary is fixed
  at the run's (*N*−1)th character regardless of where the following text
  would suggest a human reader splits it — the same class of named,
  selectable-alternative limitation `acronym_run_boundary: false` already
  states for the stock-Lucene mode, now stated for the `true` mode too.

  **`log-line-v1`** — a second new `tokenizer_profile.algorithm` value:

  | field | type | notes |
  | --- | --- | --- |
  | `algorithm` | string | `"log-line-v1"` |
  | `timestamp_pattern` | string or `null` | Go-time-layout-shaped, matching `flog`'s real constants |
  | `timestamp_field_name` | string or `null` | sub-field the extracted epoch-millis value is written to |
  | `kv_pair_detection` | bool | recognizes an ASCII `=` bounded by non-whitespace, non-`=` runs; emits the bare key, bare value, and one composite `key=value` token at the value's position |
  | `line_join` | `"none"` \| `"newline-continuation"` | schema hook only; stack-trace frame grammar is out of scope (below) |
  | `url_path_split` | bool | delimiter-splits `/`, `.`, `?`, `&`, `=`-bounded spans |

  Grounded against `flog`'s real, live-fetched `log.go`/`time.go`/`random.go`
  (`mingrammer/flog@master`): the real Apache-combined and JSON format
  strings, the four real time layouts (`"02/Jan/2006:15:04:05 -0700"` for
  Apache, `"2006-01-02T15:04:05.000Z"` for RFC5424, etc.), and the honest,
  verified finding that none of `flog`'s six format constants contains a
  `key=value` shape or an embedded newline — `flog` grounds the timestamp and
  URL-path halves of this design but not the `key=value` or stack-trace
  halves.

  **Fix — §2.7's fabricated logfmt quote, corrected.** The prior design
  attributed a formal delimiter/escaping rule ("Keys and values are
  delimited by an equals sign. Values containing spaces are enclosed in
  quotation marks.") to `brandur.org/logfmt` in quotation marks. Re-fetched
  live here, twice (rendered fetch and raw HTML with tags stripped, full
  text read start to end): **that sentence appears nowhere on the page.**
  The article states no formal delimiter or escaping rule at all. What it
  does say, quoted exactly and confirmed present: "Each line consists of a
  single level of key/value pairs which are densely packed together compared
  to other well-known structured formats like JSON," alongside the real
  Heroku router log-line example, `at=info method=GET path=/
  host=mutelight.org fwd="124.133.52.161" dyno=web.2 connect=4ms
  service=8ms status=200 bytes=1653`, itself directly, observably quoting a
  value (`fwd="…"`) — a real instance of quoting behavior visible in the
  example, not a formally stated rule the source gives. The claim of a
  formal escaping rule is dropped rather than replaced with an invented
  citation, the same honest-gap pattern this design already uses for
  `flog`'s own no-`key=value`/no-newline finding.

  **Fix — §6.1's worked example, both stopword and stemmer claims corrected.**
  The prior design traced *"An expired read here is retried unboundedly."*
  (from the real `crates/strand-core/src/manifest.rs:87-108` comment, quoted
  correctly) through stopword removal and stemming, claiming `an`/`is`/`here`
  are dropped as stopwords "checked against `references/
  lucene-english-stopwords.md`'s real list." Re-read that file directly here:
  its real, vendored 33-word list (Lucene `EnglishAnalyzer`'s default,
  `releases/lucene/10.5.1`) is `a, an, and, are, as, at, be, but, by, for, if,
  in, into, is, it, no, not, of, on, or, such, that, the, their, then, there,
  these, they, this, to, was, will, with`. **"here" is not in it** — "there"
  and "their" are, a different, easily-confused pair. Under the real list,
  "here" survives stopword removal and proceeds to stemming and indexing.
  The stemmer claim (`retried` → `retri`, `unboundedly` → `unbound`) had the
  opposite problem: presented with the same confidence as the checked
  stopword claim, but the vendored `references/
  snowball-porter2-english-stemmer.md` test-vector table has exactly ten
  entries, neither word among them — the stems were plausible predictions
  from memory of Porter2's behavior, not checked data, the exact failure
  that file's own text says vendoring exists to prevent. Both are corrected
  here with real, live verification rather than left as a flagged gap: the
  full 42,649-entry Snowball vocabulary (`snowballstem/snowball-data`,
  `english/voc.txt`/`output.txt`, fetched live) confirms neither "retried"
  nor "unboundedly" is one of the vocabulary's own entries either, so a
  second, independent method was used — the official `snowballstemmer`
  PyPI package (home page `github.com/snowballstem/snowball`, the same
  authoritative implementation this project's reference file already cites),
  installed and run live in this session, first cross-checked against all
  nine non-trivial entries of the vendored test-vector table (`whales→whale`,
  `swimming→swim`, `quickly→quick`, `running→run`, `runs→run`, etc.) — every
  one matched exactly — then run against the two words in question:
  `retried → retri`, `unboundedly → unbound`. These are now real,
  live-verified stems from the authoritative implementation, not estimates.
  The corrected trace of the full sentence: `an` and `is` are real stopwords
  and are dropped; `here` is not, survives, and stems to `here` (no suffix
  rule applies — the same package run above confirms this directly);
  `expired` stems to `expir` (also confirmed live); `retried` stems to
  `retri`; `unboundedly` stems to `unbound`. The corrected token set for
  this sentence includes `expired`, `read`, `here`, `retri`, `unbound` — one
  more indexed, stemmed token (`here`) than the original, uncorrected trace
  claimed.

  **Fix — `spec/analyzer-descriptors.md` named as real editing surface.**
  The prior design's Open Questions named only `rfcs/
  0004-analyzer-descriptors.md`'s Design §2 as needing amendment. Read `spec/
  analyzer-descriptors.md` directly here: its §2 duplicates RFC 0004's
  `tokenizer_profile` schema verbatim (the chapter's own header states "this
  chapter states the settled result"), and its §7 ("Conformance status")
  enumerates exactly which descriptor/algorithm combinations are implemented
  and vectored — both sections a `code-identifier-v1`/`log-line-v1`
  amendment must update. This matches the pattern every one of RFC 0004's
  three prior post-approval Discussion-section amendments already followed
  (each closed by updating both the RFC's own Discussion section and the
  corresponding `spec/analyzer-descriptors.md` section) — an established,
  checkable in-repo precedent the prior design never named.

  **Fix — grave selection: CIFF, not Pilosa.** The prior design named Pilosa
  ("a good structure with a spec is not a distribution strategy,"
  `docs/lineage.md`, read directly here) as the nearest grave for the risk
  that nothing in the format can verify a writer's claimed code/comment split
  is honest — reaching for it by analogy to D-6's own reuse of the same
  grave for a different mechanism, without checking a closer, already-
  established alternative. Read `docs/lineage.md`'s real Pilosa gloss
  directly: it names a production-adoption and ecosystem-gravity failure,
  not a self-reported-provenance-verifiability one — a real mismatch, one
  step further stretched than D-6's own already-once-stretched reuse of it.
  RFC 0004's own "How this could be wrong" section, read directly, already
  names a closer grave for this exact shape of gap: CIFF, "no analyzer
  metadata" — "CIFF has no mechanism at all for a consumer to know how the
  producer tokenized, so cross-engine reuse silently corrupts results,"
  and, on the risk that RFC 0004's own descriptor schema could still fall
  into the same grave, "a descriptor schema loose enough to be satisfied
  trivially... would be 'analyzer metadata' in name only, reproducing CIFF's
  gap under a different field name." This is precisely the D-1 risk,
  restated one layer up: invariant 6's conformance-vector mechanism can prove
  a `code-identifier-v1` descriptor tokenizes its own declared input
  correctly; nothing in the format can prove that declared input was
  genuinely the code-only half of some real source file rather than the
  whole file with comments left in — chain-correctness, never input-
  provenance correctness, the same gap CIFF left and RFC 0004 was written to
  close for the analysis-chain half but not, it turns out, for this new
  span-provenance half. CIFF is adopted as the corrected nearest grave; a
  future RFC amendment needs its own "how this could be wrong" treatment
  built around it, and probably a `strand-tools inspect`-level diagnostic
  (comparing a code-span field's byte length against the comment-span
  field's, or spot-checking that the code-span field contains no
  `///`-marker-adjacent text) as real scope, the same remedy D-6's own Fix 5
  named for its own, structurally identical risk.

  **Fix — the query-side score-fusion gap, named as an explicit out-of-scope
  caveat (matching D-5's own Fix 3 pattern).** Row-ID identity, deletion
  vectors, and BM25 scoring are all unaffected by splitting one document into
  N sub-fields: row-ID assignment is per-row, entirely orthogonal to field
  count (`spec/row-ids.md` §1–§2), deletion vectors tombstone row-IDs, not
  fields, and `CLAUDE.md` explicitly excludes query-time fusion logic from
  the format's own scope, so STRAND itself has no combiner to break. The
  real, previously unnamed gap: a consumer that wants one ranked result per
  file — not one per sub-field — needs some convention for fusing
  `content.code`'s and `content.comment`'s separate per-field BM25 scores
  into a single per-row-ID ranking. The existing reciprocal-rank-fusion
  mechanism (invariant 1, M3-6, `crates/strand-core/src/fusion.rs`'s real
  `reciprocal_rank_fusion(rankings: &[&[u64]], k: f64)`, confirmed directly
  here) is a plausible mechanism for this — it already fuses independent
  rankings that share a row-ID space, which `content.code` and
  `content.comment` do by construction — but nothing in this design wires it
  for this specific use, and this document does not claim otherwise. Named
  here as explicit out-of-scope follow-on work, the same shape D-5's own
  Fix 3 named for multi-term score aggregation.

  **Costs, stated honestly.** Every mixed-content source file pays roughly
  2x the per-field blob overhead (term dictionary, term-info store,
  postings, positions, block-max sibling) a single-analyzer field would.
  For a structured log line with nine JSON keys, this is a **fixed
  per-field-per-segment multiplier, not a cost that scales with document
  count** — correcting the prior design's looser phrasing: a segment with
  one document carrying nine keys and a segment with a million documents
  sharing the same nine keys both produce exactly nine fields' worth of
  blobs, because blobs are keyed by `(family_id, blob_type_id, field_id)`
  once per segment, not once per document. Tree-sitter's real Rust binding
  compiles a small generated C parser per grammar via `cc` — a new,
  non-pure-Rust build dependency, accepted rather than avoided (unlike the
  ICU4X path RFC 0004's own Discussion section chose specifically to dodge a
  C dependency), since no pure-Rust incremental parser has comparable
  language coverage. License audit is real but partial: tree-sitter core
  plus the Rust, Python, and JavaScript grammars were checked live
  (`gh api`, `license.spdx_id: "MIT"` for all four) — every additional
  language needs its own independent check before adoption, mirroring RFC
  0004's own per-candidate CJK discipline. `acronym_run_boundary` is the one
  genuinely new piece of literature (the rest of `code-identifier-v1`'s
  splitting mechanics directly reuse Elasticsearch's real, quoted
  `word_delimiter_graph` semantics unmodified) — a real design decision,
  named as such, with its `SQLite` failure mode now stated explicitly
  (above) rather than left for a future review to find.

  **How this could be wrong.** Nearest grave: CIFF (above, corrected from
  Pilosa) — a well-specified conformance-vector mechanism that proves chain-
  correctness but cannot prove a writer's declared code/comment split was
  honest. A second, narrower risk, unchanged from the prior design:
  `acronym_run_boundary` is new normative surface with no conformance vector
  of its own yet — RFC 0004's two existing vectors (`lucene-en-word-only-01.
  json`, `icu4x-dictionary-zh-01.json`) are English prose and Chinese
  dictionary segmentation; nothing tests `code-identifier-v1` or
  `log-line-v1` today. The §6.1-style worked examples above (now corrected)
  are the intended seed of the first two, not a substitute for building
  them.

  Status: **open, RFC-sized, design revised after independent adversarial
  review** — not yet an approved or implemented RFC; a corrected pre-RFC
  design ready for someone to actually draft. Depends on: nothing to begin
  drafting; real conformance-vector work is coupled to D-2's real corpora
  (`flog`, the Rust stdlib) and to the tree-sitter grammar-version
  provenance-pinning mechanism, left open here the same way RFC 0004's own
  stemmer commit-hash pinning was left open for a later session.

  **Next step**: this amends RFC 0004 in place — **no new RFC number is
  needed**. `ls rfcs/` shows 0001 through 0014 already allocated on disk;
  `docs/roadmap.md`'s own D-5 and D-6 entries above claim 0015 and 0016
  respectively for RFCs that register genuinely new blob families or amend
  the manifest's own schema (a different layer, whose one real prior
  amendment, RFC 0012, was itself a new numbered RFC rather than an in-place
  amendment — Fix 2, D-6, above). `tokenizer_profile` is different: it is
  RFC 0004's own owned schema, and RFC 0004 has already amended it once,
  in place, through its Discussion section (the `segmentation_dictionary`
  default, 2026-08-19) rather than through a new RFC — the direct precedent
  this design follows rather than departs from. The real editing surface,
  once someone picks this up: `rfcs/0004-analyzer-descriptors.md` Design §2
  (widening `tokenizer_profile` into a discriminated union keyed by
  `algorithm`, plus a new dated Discussion-section entry recording the
  amendment) and `spec/analyzer-descriptors.md` §2 and §7 (schema and
  conformance-status update, per the newly-named editing-surface fix above)
  — both updated in the same session, matching every one of RFC 0004's prior
  amendments, before any conformance vectors are built.

- **D-8** — D-3, executed: a real code-embedding model was found, licensed,
  and run locally; D-2's Rust stdlib corpus was chunked and embedded with it;
  the graph-blob benchmark was re-run against the result and compared
  directly to `bench/results/graph-warm-query.json`'s synthetic-data run.
  **Feasibility research, live-checked rather than assumed.** Three real
  candidates were checked against Hugging Face's own model API (license
  field, real file sizes, real siblings list), not against remembered
  model-card text, per `CLAUDE.md` §3: `microsoft/codebert-base` was
  rejected — its model API response carries no license field at all (not
  merely an unfamiliar one), a real gap this pass would not paper over with
  an assumed Apache-2.0/MIT reading, and it also ships no ONNX export, so
  using it would require building a separate pooling head on top of a bare
  encoder; `Salesforce/codet5p-110m-embedding` was rejected on runtime
  grounds, not license — it is real BSD-3-Clause (compatible), but its
  `config.json` names a `custom_code` architecture with no ONNX export,
  so loading it at all requires `trust_remote_code=True` through the full
  `transformers`+PyTorch stack, the heavy install path this task asked to
  avoid where a lighter one exists. `jinaai/jina-embeddings-v2-base-code`
  was the real, live-confirmed winner: Hugging Face's model API reports
  `license: apache-2.0` directly (no combination-license reasoning needed,
  the cleanest possible match to `CLAUDE.md` §1's zero-exceptions bar), a
  real `onnx/model_quantized.onnx` export already exists in the repo
  (161,895,621 bytes — squarely in the "tens-to-low-hundreds of MB" range
  this task asked for, not the multi-GB range), and its model card states
  real code-specific training: pretrained on the `github-code` dataset then
  fine-tuned on "more than 150 million... coding question answer and
  docstring source code pairs" across "English and 30 widely used
  programming languages," Rust named explicitly among them. Real output
  dimensionality, confirmed by inspecting the ONNX graph's own output
  tensor shape directly (`onnxruntime.InferenceSession.get_outputs()`), is
  **768**, not the original synthetic benchmark's `dims=128` — used as-is
  here rather than truncated or padded, since either transform would
  discard or fabricate signal the model was never trained to tolerate.

  **The minimal-footprint runtime path this task asked to check for is
  real and was used**: `onnxruntime` (23.1 MB wheel) plus the pure-Rust-
  backed `tokenizers` package (3.3 MB wheel) — no PyTorch, no `transformers`,
  no `trust_remote_code` — because the model's own published ONNX export
  already bakes its custom ALiBi-attention architecture into a standard
  graph with two plain inputs (`input_ids`, `attention_mask`) and one
  output (`last_hidden_state`, shape `[batch, seq, 768]`), confirmed by
  loading the graph and printing its real input/output signature before
  committing to this path. Pooling is attention-mask-weighted mean pooling
  over `last_hidden_state` (the same convention the model's own
  `1_Pooling/config.json` and its published Transformers.js example use),
  followed by L2 normalization — a deliberate choice, not the model's own
  forced default, made because this model's intended similarity metric is
  cosine similarity on unit vectors, and `crate::vamana`'s squared-Euclidean
  distance is exactly monotonic with cosine similarity on unit vectors
  (`‖a−b‖² = 2 − 2·cos(a,b)`), so the two pair up correctly without any
  further metric-conversion work. Before spending compute on the full
  corpus, this pipeline's real signal was sanity-checked on four hand-
  written Rust snippets: `checked_add` vs. `checked_sub` (structurally
  near-identical) scored cosine similarity 0.756, against 0.10–0.35 for
  every cross-category pair (`string_split`, `vec_push`) — real, confirmed
  cluster structure, not an assumption, before the corpus-scale run began.

  **Real corpus and chunking, per D-2's own recommendation.** The identical
  source D-2 named — `rust-lang/rust`, `library/core`+`library/std`+
  `library/alloc` at tag `1.97.1` — was fetched with a shallow, blob-filtered,
  sparse-checkout clone (`git clone --filter=blob:none` + `sparse-checkout`),
  landing 1,064 real `.rs` files in ~17 MB (`.git` a further 13 MB), excluding
  D-2's own two named per-file license exceptions
  (`library/core/src/unicode/unicode_data.rs`,
  `library/std/src/sys/sync/mutex/fuchsia.rs`) from this pass exactly the
  same way D-2 itself flagged them, even though this data never leaves the
  local machine. Chunking method, stated plainly per this task's own
  requirement (not `syn`, not tree-sitter — a regex-plus-brace-counting
  heuristic, fast to get mostly right rather than slow to get exactly
  right, judged acceptable for a one-off benchmark-input generation pass
  and not committed spec tooling): find a `fn` signature (with optional
  `pub`/`async`/`unsafe`/`extern`/`const` modifiers and leading attributes)
  up to its opening `{`, then walk forward counting braces to the matching
  close, keeping the whole span. This is naive about string/char/comment
  literals containing braces, so a small, unquantified fraction of chunks
  may be mis-bounded — stated honestly rather than silently assumed
  correct. 17,262 raw chunks were extracted this way; 14,773 remained after
  exact-text dedup (macro-expanded impls across primitive integer types
  produce some identical bodies); a 4,000-chunk build sample (matching the
  original benchmark's own `n=4,000`) and a disjoint 300-chunk query sample
  were drawn with fixed, distinct seeds (`20260820`/`20260821`) from that
  pool, length-filtered to 40–4,000 characters (mean 352.8) to drop
  one-line trivial getters and pathological outliers. The query set is real
  held-out code embeddings from the same corpus, not fresh random vectors —
  a random vector in ambient 768-d space would sit off this model's real
  data manifold, which would be a *less* representative query workload than
  reusing the corpus, not a neutral baseline.

  **The re-run benchmark**: `bench/src/graph_warm_query_real_embeddings.rs`
  (`graph-warm-query-real-embeddings` in `bench/Cargo.toml`), a new binary
  alongside `graph_warm_query.rs` rather than a rewrite of it — the same
  pattern `cold_open.rs`/`cold_open_injected_latency.rs` already establish
  for "same measurement, different real regime, separate file, separate
  results JSON." Construction parameters held identical to the original run
  for a fair A/B (`R=64`, construction `L=100`, `alpha=1.2`, query-time
  `L ∈ {32, 100}`, `k=10`); `dims=768` differs honestly, for the real reason
  above. Local NVMe read latency was re-measured anyway (for a
  self-contained report) rather than reused, and landed within noise of the
  original run (p50 57.5μs vs. 56.2μs) — confirming, not contradicting, that
  this component measures the storage device, not the data distribution, as
  the D-3 task itself expected.

  **The real comparative numbers**
  (`bench/results/graph-warm-query-real-embeddings.json` against
  `bench/results/graph-warm-query.json`, both real, both committed):

  | metric (n=4,000, R=64) | synthetic, dims=128 | real embeddings, dims=768 |
  |---|---|---|
  | L=32 mean hops/query (min–max) | 33.45 (32–37) | 33.9 (32–38) |
  | L=32 mean fetches/query | 2,032.9 | 1,416.3 |
  | L=32 fetches as % of pessimistic hops×R bound | 95.0% | 65.2% |
  | L=100 mean hops/query (min–max) | 100.80 (100–103) | 101.5 (100–104) |
  | L=100 mean fetches/query | 5,761.1 | 3,512.4 |
  | L=100 fetches as % of pessimistic hops×R bound | 89.3% | 54.1% |
  | Starling OR(G), BNF permutation | 0.0795 | 0.1769 |
  | Starling OR(G), unshuffled | 0.0156 | 0.0121 |
  | est. query latency (local p50), L=32/L=100 | 114.3ms / 324.0ms | 81.4ms / 201.9ms |
  | `build_vamana` wall time | 183,468ms | 416,827ms |
  | graph blob bytes (node records + directory) | 3,152,016 | 13,392,016 |
  | S3 whole-blob open | 3 GETs, p50 4.6ms | 3 GETs, p50 12.6ms |

  **What this does and does not confirm, stated precisely rather than
  picking the flattering half.** It does **not** confirm the hoped-for
  "real clustered data converges in fewer hops" framing: hop count stayed
  pinned almost exactly to the query-time `L` ceiling in both regimes —
  every sweep's minimum hop count equals `L` itself, in both the synthetic
  and the real run. `GreedySearch`'s termination is driven by when its
  `L`-sized candidate list stops improving, and that saturation happened at
  essentially the same rate regardless of whether the underlying data was
  uniform-random or genuinely clustered — a real, honest correction to this
  entry's own opening hypothesis, not softened. It **does** confirm a real,
  distinct effect of cluster structure, visible in a different metric:
  **fetch count and hop-to-hop redundancy**. Real embeddings needed 30–39%
  fewer total record fetches per query at both `L` values, and the
  "fraction of the pessimistic `hops × R` bound" — how much of the
  worst-case fan-out each hop's real neighbor-overlap saves — dropped from
  ~90–95% (synthetic, i.e. barely better than the pessimistic bound at all)
  to ~54–65% (real). This is the real signature of cluster structure this
  benchmark was built to find: a clustered embedding space's neighbor sets
  overlap substantially more between successive greedy-search hops than a
  uniform-random space's do, so a fixed hop budget touches fewer distinct
  records — even though the hop budget itself is not shortened by
  clustering. The Starling `OR(G)` locality metric corroborates this
  independently (not derived from the fetch-count numbers at all): BNF's
  own block-locality score more than doubled, 0.0795 → 0.1769, meaning BNF
  reordering finds meaningfully better block-local neighbor structure on
  real clustered data than on synthetic uniform-random data. Estimated
  query latency (fetches × the real local-NVMe p50, essentially identical
  across both runs) fell correspondingly, 29–38% lower with real data — but
  remains firmly in the same regime RFC 0014 Design §5 already predicted
  and `graph-warm-query.json` already measured: 81–202ms per query, still
  one to two orders of magnitude above DiskANN's own cited `<3ms`, because
  this v0.1 graph-blob format still has no compressed-code cache to avoid a
  full-precision fetch per visited node — real data narrows that gap, real
  data does not close it. `build_vamana` wall time grew 2.27x (183.5s →
  416.8s) for a 6x increase in `dims`, a real, sub-linear-in-dims scaling
  measurement offered without a forced explanation for the exact ratio
  (plausibly memory-bandwidth rather than pure-compute bound at this scale,
  not independently verified here). Graph blob bytes grew 4.25x for the
  same 6x `dims` increase, the expected sub-linear-in-dims result of a
  fixed per-record overhead (row ID, neighbor list) that does not scale
  with vector width. The S3 GET count stayed exactly 3 in both runs,
  confirming the wire-format shape is unaffected by embedding source or
  dimensionality, as invariant 3 requires.

  **Named limitation, stated explicitly per this task's own instruction:**
  one model's (`jina-embeddings-v2-base-code`) embedding space on one
  corpus (4,000 heuristically-extracted Rust stdlib function chunks) is not
  a universal claim about "code embeddings" in general — a different model,
  a different language mix, a different chunking granularity (whole files,
  logical blocks smaller than a function, or docstring-only spans), or a
  corpus with different real cluster density could show different
  fetch-count/redundancy numbers. The query workload here is also
  same-distribution held-out code chunks, not an independently-sourced
  query shape a real deployment would see (for example, natural-language
  code-search questions rather than function bodies) — real query-side
  embeddings for a natural-language-to-code search task are a real
  follow-up this entry does not attempt. The heuristic, non-parser chunker
  (above) is a second, separately-named source of imprecision in exactly
  what "a code embedding" means in this run.

  Real artifacts from this pass: `bench/src/graph_warm_query_real_embeddings.rs`,
  `bench/results/graph-warm-query-real-embeddings.json`, and (gitignored,
  per `/bench/data`, never committed — the same pattern `msmarco_index.rs`
  already establishes) `bench/data/d3-jina-code-v2/` (the ONNX model, tokenizer,
  and the two generated embedding `.bin`/`.meta.json` pairs) and
  `bench/data/d3-rust-stdlib-1.97.1/` (the sparse Rust stdlib checkout).
  `cargo check --workspace --all-targets` and `cargo clippy -p strand-bench
  --bin graph-warm-query-real-embeddings --all-targets -- -D warnings` both
  clean. Status: **closed** — D-3's own open question (real vs. synthetic
  convergence behavior) has a real, measured, honestly-mixed answer: hop
  count is governed by `L` regardless of data distribution; fetch count and
  graph locality are real, measurably better on clustered data, by 30–65%
  depending on the metric, which is a genuine, partial confirmation of the
  original "near-worst-case synthetic" caveat, not a full one. Depends on:
  D-2, D-3.

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
