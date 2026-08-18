# R2 follow-on: a methodology for the BP128/Elias-Fano hybrid question

This is a research plan, not a result. It does not commit STRAND to any codec
decision and does not require an RFC — R2's own ledger entry already establishes
that BP128 is the postings-codec default under active grounding, and this document
investigates a narrower, separate question raised alongside that grounding: whether
a single codec could combine the block-based PFOR/BP128 family's raw full-scan
decode speed with Elias-Fano's compressed-domain searchability, since no published
construction achieving both has been found (PISA offers both as separate per-index
choices, not one fused codec). On compression the measured direction is the
reverse of what an earlier draft of this document assumed: partitioned Elias-Fano
is the *smaller* of the two on the reference collections
(`references/ottaviano-venturini-partitioned-elias-fano.md`), so full-scan decode
speed, not size, is the axis a hybrid would need to win back. If a later session runs any
phase and finds something load-bearing for R2's actual codec choice, that finding
gets its own ledger entry and, if it changes the registered codec, its own RFC —
this document is scoped to the investigation, not its outcome.

**Phase 0 — attempted 2026-08-18, inconclusive on its strongest candidate for a
genuine access-barrier reason, not a design or measurement failure.** Before
searching, the bar was fixed as instructed (Step 1): the same yardstick Phase 3
Stage 1 states explicitly — full-scan decode throughput within 15% of BP128-class
speed, and compressed-domain search at least 25% faster than decode-then-search,
both required simultaneously. A real, structurally on-point candidate was found —
LICO (Zhu et al., SIGMOD/PACMMOD 2026, `references/lico-simd-learned-inverted-index-appendix.md`):
a learned piecewise-linear-model codec with both a genuine compressed-domain
`NextGeq` operation (binary search over the model's own encoded segments, no full
decode) and SIMD decode/intersection paths, confirmed by reading the actual source
code, not a description. Its precise numbers against the stated bar could not be
checked: the paper's Experiments section sits behind a Cloudflare bot-challenge
that blocked every automated fetch attempt despite Unpaywall reporting the paper
CC-BY (DBLP's own record for the same DOI contradicts this, reporting "closed" —
neither claim was resolved, both are stated), no arXiv or author-mirrored copy
exists, and this session's hardware lacks the AVX-512 the reference implementation
hard-requires to build or run at all. Full detail, including exactly which access
routes were tried and ruled out, is in the vendored reference file. Per Step 4's
verdict-mapping, this is neither "candidate passes" nor "candidate found but
fails" — it is a genuine third outcome the original mapping did not anticipate: a
real candidate the plan cannot currently rule in or out. **Standing, not yet
decided:** whether to keep pursuing LICO specifically (a licensed/paywall-cleared
copy, or borrowed AVX-512 hardware) versus treating Phase 0 as inconclusive and
proceeding to the Track A/B fork on that basis. This document does not decide that
on its own — see `docs/ledger.md` R9 for the standing status.

The plan was produced by generating four independent methodologies from different
angles (structural/theoretical feasibility, adaptive composition of the two existing
codecs, cross-domain literature archaeology, empirical prototyping), adversarially
critiquing all four from three lenses (rigor, proportionality, discovery-power), and
synthesizing one phased, gated plan — then passing that synthesis through one more
independent adversarial review before treating it as finished. That review found and
fixed a real defect (an undecidable branch in the gate meant to resolve the plan's
central cost-vs-trustworthiness tension); the fixes are inlined below, not deferred.

---

**Scope note.** This document is a plan for finding out. It does not propose, sketch,
or evaluate any specific codec or encoding design at any point below. Phase 2B does
evaluate one narrow composition question — whether choosing per list or per block
between the two existing, unmodified codecs can approach an oracle ceiling — which is
selection among already-registered codecs, never a new encoding; its surviving
output, a validated composition scheme, is what Phase 3 tests.

## Phase Sequence and Rationale

Phases are cost-ordered where a real dependency exists. Phase 0 gates everything.
After Phase 0, the investigation forks into two *independent* tracks that answer
different questions and do not block each other: a structural/theoretical track
(Phase 2A) and a practical/empirical track (Phase 1 → possibly 2B → possibly 3).
Either, both, or neither may run depending on what Phase 0 finds and what appetite
exists for each track's cost.

### Phase 0 — Literature and Terminology Filter

**What it does.** Establishes whether a named prior construction already claims to
fuse BP128-class decode speed with EF-class compressed-domain searchability, against
a precise definition, not a description.

**Steps.**
1. Write down, before searching, the exact operational properties a construction
   must have to count as "fusing both": decode throughput within a stated factor of
   BP128's measured numbers, and compressed-domain operations (rank/select/skip/
   intersect) within a stated factor of EF's — both defined against the benchmark
   family Phase 3 later uses, so the definition and the eventual test are the same
   yardstick.
2. Search IR venues, succinct-data-structure literature (wavelet trees, PGM-index,
   sdsl-lite, RRR, learned indexes — named here as search targets, not as candidate
   designs), and adjacent fields (columnar-DB compression, genomics indexing).
3. Check any candidate found against the Step 1 properties with real numbers, not a
   description.
4. Verdict-mapping, fixed before starting: candidate passes → stop, question
   answered; candidate found but fails → logged as a negative data point, proceed;
   nothing found → weak evidence of absence only, explicitly logged as such, proceed.

**GO/NO-GO.** Proceed to the fork below unless Step 3 finds a construction clearing
the bar, in which case the investigation stops with an answer.

**Cost.** Lowest of all phases — a few focused days, no specialist training or
infrastructure required.

### Track A: Structural/Theoretical (Phase 2A, conditional)

**Trigger.** Only if Phase 0 is genuinely inconclusive *and* a structural answer to
"can such a thing exist at all" is wanted, independent of whether anyone pursues
Track B below.

**What it does.** Pursues formal impossibility or feasibility arguments (reduction
arguments against known predecessor-structure results; cell-probe lower bounds on
the redundancy compressed-domain query support requires vs. the zero-redundancy
budget dense fixed-width packing allows).

**Explicit flag.** Requires specialist theoretical-CS expertise (cell-probe model
literacy, information-theoretic lower-bound technique) not assumable on a generalist
team — budget sourcing it externally as part of this phase's real cost, not as a
free add-on. If more than one independent formal line is attempted, check explicitly
that they don't share an unstated assumption (e.g., all modeling postings as
adversarial rather than empirically skewed) before treating agreement between them
as independent confirmation.

**Output and feedback.** A clean impossibility result stops the whole investigation
with a real answer, regardless of what Track B is doing. A clean existence sketch,
or an inconclusive result, does not block Track B — it hands over whatever
structural insight was gained (e.g., which specific property is the hard one) as
context Track B should use to sharpen its own pilot signal search (Phase 1) or
candidate evaluation (Phase 3), without waiting on it.

**Cost.** High — specialist time, no guaranteed answer, run only if the appetite for
this cost exists independent of Track B's outcome.

### Track B: Practical/Empirical

#### Phase 1 — Cheap Separability Pilot

**What it does.** Tests, on a small sample, whether BP128-vs-EF winner is
predictable from cheap, precomputable signals — before committing to anything
resembling whole-corpus dual encoding.

**Steps.**
1. Sample a few thousand postings lists (not a full corpus) spanning a range of
   list length, skew, and gap-distribution shape.
2. Encode each with both existing codec implementations (no new engineering);
   measure size and decode/skip cost.
3. Fit the simplest candidate signal (a single statistic or threshold rule) on a
   training split; evaluate it on a held-out split. A signal only counts as
   "learnable structure" if it holds up out-of-sample — in-sample-only separation is
   explicitly treated as "no signal found," not as a third, ambiguous outcome. (This
   replaces an earlier three-way outcome list that left one branch — "strong
   separation, unclear if generalizable" — without a mapped decision; the ACPR pass
   (Adversarial Critic Pass Review — the final independent critique pass this
   document went through before being treated as finished, per the intro) caught
   this as the pipeline's central undecidable gate, and this fix closes it.)
4. Before treating a failed Step 3 as final, run one cheap extension: check a single
   pairwise-interaction term (e.g., length × skew) in addition to the univariate
   signals already tried. This exists specifically because Phase 1's method is
   deliberately weak — it is calibrated to catch only strong, simple signal, and a
   real opportunity that depends on a nonlinear interaction between features would
   otherwise be invisible to it and wrongly read as "no opportunity exists." This
   step does not turn Phase 1 into a full learned-chooser exercise; it is one
   additional, cheap check, not an open-ended search.

**GO/NO-GO.** Out-of-sample signal found (including via the Step 4 extension) →
proceed to Phase 2B. No signal found even with Step 4 → stop; log explicitly that
this is a necessary-but-not-sufficient screen (it cannot rule out an opportunity
only visible to a full learned chooser over many correlated features) rather than
implying "no opportunity exists" — a real, if smaller, residual chance of a false
negative here is acknowledged, not hidden, and is the price paid for not building
Phase 2B's full apparatus speculatively.

**Cost.** Low — days, script-only, no harness or production infrastructure.

#### Phase 2B — Full Oracle-Based Composition Study (conditional on Phase 1's GO)

**What it does.** Whole-corpus dual encoding (BP128 and EF) across a realistic,
deliberately heterogeneous corpus, measuring the actual achievable composition
ceiling and whether a chooser (heuristic or learned) can approach it — with three
named downside studies run as required deliverables, not folded into a vague
"uniformity check":

- **SIMD-batch uniformity loss.** Measure full-list sequential decode throughput
  under realistic per-block or per-list codec alternation, not each codec's
  isolated per-block speed — the interesting failure (branch/pipeline cost from
  alternation) only shows up at the full-list level.
- **Chooser error cost, not just accuracy.** For every misclassified unit, measure
  how bad the miss is (mildly suboptimal vs. actively worse than either pure
  baseline on some metric) — a highly accurate chooser with catastrophic rare
  errors is a worse result than a less accurate one with gentle failure modes, and
  aggregate accuracy alone hides this.
- **Engineering/verification surface cost.** Count, concretely, the added test
  surface (fuzzing two decode paths instead of one), the merge-semantics question
  (what happens when concatenating segments whose lists chose different codecs, per
  invariant 1's per-family merge semantics), and the added registry/metadata
  entries — report this as a stated cost figure to weigh against the measured gain,
  not an afterthought.

**Budget and off-ramp.** Stated up front as substantially larger than the pilot
(multi-week, harness-building scale). Intermediate checkpoint: on a growing corpus
subset, if the achievable ceiling is already collapsing toward one codec's baseline,
stop before finishing the full corpus.

**GO/NO-GO.** A genuine, corpus-scale ceiling gain that survives all three downside
studies → Phase 3. A collapsed, marginal, or downside-dominated result → stop, with
the specific dominating downside named in the record.

**Cost.** Highest single-phase cost in Track B — comparable to building a real
prototype system; never entered without Phase 1's cheap gate clearing first.

#### Phase 3 — Empirical Validation Against Realistic Data

**Trigger.** Only for whatever specific artifact survived Track A (a hybrid design
this plan does not itself produce, evaluated elsewhere) or Phase 2B (a validated
composition scheme) — never run speculatively against nothing.

**What it does.** Reuses the exact test collections the cited baseline literature
used (for direct numeric comparability), with Stage 0's cost-model calibration
running first against cheap/public numbers or a small freely-available proxy
collection — expensive licensed corpora (e.g. ClueWeb09-class collections) are
deferred until a prototype has already cleared the proxy-data stage, so the
expensive asset is never needed to reach the cheap kill-switch.

**Gates, stated explicitly here rather than by reference** (the specific thresholds
are inlined so this document is self-contained and the numbers can't quietly soften
in a future re-telling):

- *Stage 0 → Stage 1:* proceed only if a cost model, calibrated against real
  measured baseline numbers, predicts — in at least one clearly bounded regime —
  either a ≥2× compressed-domain-search advantage over decode-then-search, or
  closure of partitioned EF's full-scan decode deficit against block-based
  PFOR-family codecs (the 7–17% OR-query edge Ottaviano & Venturini measured for
  block-based indexes) while keeping most of EF's measured advantages — its
  compression edge (OptPFD is 11.6–12.3% *larger* than partitioned EF on
  ClueWeb09/Gov2) and its 14–40% AND-query speed edge
  (`references/ottaviano-venturini-partitioned-elias-fano.md`; an earlier
  version of this gate stated the compression direction backwards, as an EF
  "disadvantage" — the vendored paper says the opposite, and the gate now
  reflects it). A predicted win under roughly 20% in the best-case regime does
  not justify writing code.
- *Stage 1 → Stage 2:* proceed only if a minimal scalar prototype clears all three,
  simultaneously, reproducibly across a cross-collection check (a second collection
  with a different skew profile — sign reversal means the result is
  collection-specific, not real) and a full length/skew regime sweep:
  - selective (AND/WAND) query latency within the range the cited literature
    already reports for EF-vs-BP128-family, on at least two collections;
  - bulk sequential decode throughput no more than 15% slower than the measured
    BP128 scalar baseline, on the same hardware and optimization tier;
  - compressed-domain search at least 25% faster than decode-then-search on the
    same prototype's own decode path, measured same-binary/same-process/
    interleaved, reported as a ratio.

  Any one of the three failing means "not a general win" — stop, name which axis
  failed, do not proceed hoping a later SIMD pass fixes it.

Stage 2, for completeness: the SIMD-tier implementation of whichever artifact
cleared Stage 1, measured under the same three criteria on the same collections.
Its exit is the investigation's final verdict: all three still holding at the
SIMD tier means the question is answered affirmatively and any codec-registration
consequence goes to its own RFC per the intro; any criterion failing means stop,
with the failing axis named — a Stage 1 pass that Stage 2 cannot reproduce is
recorded as a scalar-only result, not a general win.

**Cost.** Cheap at Stage 0, rising only once earned by clearing the cheap gate.

## What This Plan Does Not Do

It does not propose, sketch, or evaluate any specific hybrid codec design or encoding
scheme at any phase. Naming existing structures
(PGM-index, wavelet trees, sdsl-lite, RRR) in Phase 0 is search-target specification,
not design-sketching — nothing above states or implies how such a structure would
combine with BP128 or EF. Every phase produces evidence and a stop/continue verdict;
Phase 2B alone may additionally hand a validated composition scheme to Phase 3, per
the scope note.

## The Resolved Tension, Precisely

Discovery-power correctly found that Phase 2B's oracle design (encode everything
both ways, measure the real winner) is the only mechanism among the original four
proposals that establishes a true ceiling on the opportunity, making its conclusions
hard to dismiss as artifacts. Proportionality correctly found that committing to
that oracle unconditionally — with no budget and no off-ramp — was the single
biggest risk of runaway scope in the whole investigation. The fix is not to average
these two judgments: Phase 2B is preserved exactly as strong as originally designed,
including its full downside-hunting rigor, but it is never entered without Phase 1's
cheap, explicitly-bounded, honestly-caveated pilot clearing first — and that pilot's
own limits (it can produce a false negative on opportunities only visible to a full
learned chooser) are stated plainly rather than hidden, so a "stop" verdict at Phase
1 is understood as "no simple opportunity found," never oversold as "no opportunity
exists."
