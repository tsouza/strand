# RFC 0002: Dual-model verification of the manifest CAS protocol

- **Status:** Approved — passed a second adversarial review after a first review
  found 10 ranked issues (citation errors, one backwards citation, incompleteness,
  a proportionality question). All 10 were fixed in a revision (references
  vendored, the PObserve and MongoDB citations corrected, the drift-classification
  table completed, the action grammar extended to the reader path); the second
  review confirmed each fix against the vendored primary sources and the real
  protocol code rather than trusting the revision's own account, and found three
  further minor issues (a modality contradiction in the Type-II resolution rule, a
  missing `DefiniteFailure` outcome in the reader-path grammar, an unsourced
  "already expert" claim in the effort estimate), each fixed in place. On
  proportionality specifically: the property-based alternative the first review
  called for was built and evaluated (see "Evidence gathered") and caught this
  protocol's known bug class; the user then determined, independent of that
  result, that test-based coverage — however rigorous — cannot substitute for
  exhaustive model-space exploration, because it is bounded by the scenario space
  a human encoded into the generator ("known unknowns"), while a faithful model
  checked exhaustively within its bounds can surface interactions no test author
  conceived of ("unknown unknowns"), and chose the full original scope (TLA+
  model, TLAPS proof, DST harness, dual-tracing cross-validation) over a cheaper
  phased alternative. Implementation (the actual `.tla` spec, the TLAPS proof, and
  the DST harness) may now begin against this RFC.
- **Milestone:** Originally none directly (cross-cutting verification infrastructure,
  gating nothing in M1–M5) — **amended, `docs/milestones.md`'s M3 entry, "Discussion
  — post-approval amendments" below**: the remaining two artifacts (TLAPS proof, DST
  harness) now gate M3's compaction work specifically, since compaction needs its
  own new manifest commit shape and this RFC's own model already covers two others.
- **Invariants exercised:** none changed. This RFC proposes no change to `CLAUDE.md`'s
  invariants, RFC 0001's protocol, or any wire format — it proposes a method for
  gaining confidence that the existing, already-approved protocol and its existing,
  already-implemented Rust code agree with each other.

## Adversarial review findings

An independent review (fresh agent, no access to this RFC's own drafting
context, instructed to fact-check every external citation against primary
sources and to give a real proportionality opinion rather than a
both-sides summary) found, ranked most important first:

1. **The AWS PObserve citation is backwards.** PObserve validates production
   logs against a pre-existing spec *after the fact* — "observe real
   execution, reconcile against spec," in AWS's own words — the opposite of
   what §2 cited it for (evidence that "spec generates traces, drives the
   implementation" is the direction that works in production). This was the
   sole evidence for sequencing Workflow II before Workflow I; the argument
   needs to be rebuilt without it, if it still holds at all.
2. **Disproportionate to the protocol's actual size.** `commit()` is one
   retry loop over four `StoreError` outcomes; `read_snapshot()` is a bounded
   loop over three. No consensus, no quorum, no multi-node state. All three
   bugs found in this protocol so far were caught by targeted mutation
   tests, not anything TLA+-shaped — a fact this RFC's own Motivation
   states without drawing the conclusion. Recommendation: build and run the
   property-based-testing alternative and show it insufficient before this
   RFC's scope is reconsidered, not the reverse.
3. **Citations not vendored**, contrary to CLAUDE.md §3's explicit
   requirement for exactly this situation.
4. **The MongoDB citation is embellished beyond its source.** The real
   anecdote (RaftMongo.tla modeling leader step-down/step-up as one action
   against two real implementation steps) checks out, but "~10 weeks"
   conflates the total project duration with this specific mismatch's
   duration, and "deterministic, 100%-consistent... not probabilistic
   drift" appears nowhere in the source — invented precision layered onto a
   real citation, the specific failure mode CLAUDE.md §3 names by example.
5. **§2's stronger claim overclaims.** "Granularity agreement is a design
   property already established, not a hope" after Workflow II doesn't
   follow: Workflow II proves driven, spec-generated sequences replay
   correctly, not that the real code's spontaneous concurrent trace
   decomposes at the same boundaries — the actual MongoDB failure mode.
6. **Drift-classification table (§3) is incomplete**: no rule for which
   side is authoritative in a Type-II fix, and a fourth drift source is
   missing — the DST harness's simulated fault model itself not matching
   real S3/MinIO/GCS/Azure semantics (RFC 0001 notes GCS/Azure are R5,
   unverified).
7. **§4's action grammar omits the reader side** (`read_snapshot()`'s
   404-refresh/`Expired` handling) despite the RFC's own Motivation scoping
   itself to "the manifest CAS protocol," not the writer path alone.
8. **The effort estimate (§5) may not match the actual proposal**: it
   leans on FMDSE's 1,282-line TLAPS proof as the dominant cost, but nothing
   in Design or Open Questions actually commits to TLAPS.
9. **Sequencing gap against the project's own milestones**: table metadata
   and compaction/retention are M3, not yet implemented; modeling the
   protocol now risks rework, and the reader-side 404-refresh race can't be
   exercised end-to-end without the M3 orphan sweep.
10. Minor: the Raft-vs-Delta-Lake granularity contrast is this RFC's own
    framing, not Vanlightly's; `spacejam/tla-rust` contains only `.tla`
    files, no Rust — "attempted the spec half; no implementation was ever
    begun" is more accurate than "attempted this architecture." The FMDSE
    citation itself (arXiv:2501.08550) checks out closely, including the
    Workflow I/II terminology being the paper's own.

Bottom line from the review: substantially reworked before another pass,
not approved with minor fixes — and the proportionality question should be
resolved with evidence, not deferred again.

### Evidence gathered

The property-based alternative is built:
`crates/strand-core/src/manifest.rs`'s `tests::property` module, a
`proptest` property over randomized sequences of 1–8 concurrent-writer
rounds (solo commit, real rival-wins race, ambiguous-landed, and
ambiguous-not-landed pointer writes), checked against the protocol's real
safety invariants rather than one scenario's hand-computed expected
values. Mutation-tested against both bugs previously found in this
protocol: the row-ID-overlap bug (stale `current` reused across retries)
reproduces as a hang, caught by the property test's timeout; the blind-
ambiguous-retry bug reproduces as a clean assertion failure, with
`proptest`'s shrinker reducing it automatically to its minimal case
(`rounds = [(AmbiguousLanded, 1)]`) — a case no hand-authored test in this
file specifically targeted. This is direct evidence for finding 2 above:
the cheaper alternative catches this protocol's known bug class without
TLA+ or DST. Whether it leaves a real residual gap only a model checker
would find remains open — nothing has yet tried to construct a bug this
property test's invariant set cannot express (e.g., a liveness violation,
which a bounded-round proptest property structurally cannot show).

Per the user's explicit decision (recorded in the Status header above), this
residual-gap question is no longer the gate on whether this RFC proceeds — it
proceeds regardless, because property-based testing and model checking answer
different questions. It remains useful context for scoping *what the TLA+
model needs to check that the property suite already doesn't*, which is part
of what the Open Questions below still need to settle.

### Second adversarial review

A second, independent review (fresh agent, verifying each of the 10 findings
above against the newly vendored `references/` files and the real protocol
code — `rfcs/0001-container-rowid-manifest.md`, `spec/manifest.md`,
`crates/strand-core/src/manifest.rs` — rather than trusting the revision's own
account of itself) confirmed findings 1, 3, 4, 5, 6, 8, 9, and 10 genuinely
fixed, and finding 2 (proportionality) honestly resolved as a recorded policy
decision rather than a smuggled technical claim. It flagged one caveat worth
keeping in view rather than treating as fully closed: TLC's "exhaustive within
bounds" is exhaustive only over the *modeled* action/outcome enumeration,
which — like `proptest`'s `RoundPlan` — is itself human-authored; the
known-unknowns/unknown-unknowns distinction this RFC's Status header draws is
softer than a categorical line, since a spec that omits an action class is
blind to it in exactly the way a generator that omits a fault type is. This
does not change the user's decision, but it means the model's own action
grammar (§4) deserves the same scrutiny for completeness that "How this could
be wrong" already gives the property-based generator.

It also found three new, minor issues, each fixed directly rather than
requiring a further review pass: §3's "Resolving Type-II" paragraph
contradicted its own table (reasoning in "the model forbids" terms about a
drift type defined as "the model permits"), corrected to stay in one
modality throughout; §4's reader-path actions were missing a `DefiniteFailure`
outcome the writer path already had, even though `read_snapshot` has exactly
that outcome (`ReadError::Io`), now added; and §5 attributed "already expert
in the technique" to FMDSE's team, a claim the vendored source does not make,
now removed. Verdict: approve with those fixes, which are reflected in the
body above.

## Summary

The manifest's compare-and-swap commit protocol (RFC 0001 §3, `spec/manifest.md` §2)
is concurrent, retry-based, and safety-critical: a bug in it can silently commit
overlapping row-ID ranges or lose a writer's data. It is currently verified by
hand-written unit tests, mutation tests, and a real-MinIO commit-contention benchmark
(`bench/src/commit_contention.rs`) — all valuable, all necessarily incomplete, because
each exercises a scenario a human thought to write down. This RFC proposes a
**dual-model verification** architecture: a TLA+ model of the protocol, machine-checked
for the safety properties that matter, paired with a deterministic-simulation-testing
(DST) harness that drives the *real* Rust `commit()`/`read_snapshot()` code through the
same state space and cross-checks its behavior against the model via a shared trace
vocabulary. Where the two disagree, the disagreement is classified — real bug, model
too loose, or tracer artifact — and fixed at the layer actually responsible, never by
tuning either side to force agreement.

This is original in its specific niche: no located prior art connects a TLA+ model of
an Iceberg/Delta/Lance-shaped optimistic-concurrency manifest protocol to a
trace-validated Rust implementation. The closest analogues — MongoDB's real-execution
trace validation, AWS's PObserve/P-language production tooling, and the dormant
`spacejam/tla-rust` project — are named throughout as the grounding for this design and
its risks.

## Motivation

CLAUDE.md §3 requires "the how this could be wrong section... must also name which
death from the graveyard it most risks repeating" for every RFC, and requires fuzzing
and round-trip property tests as non-optional. The manifest protocol has round-trip
tests and mutation tests but no fuzzing and no exhaustive state-space exploration —
the closest it has is `bench/src/commit_contention.rs`'s eight real concurrent
writers, which exercises one interleaving per run, not the space of interleavings.
Three bugs already found in this protocol during M0 (the row-ID-overlap race, the
`Io`-vs-`PreconditionFailed` retry bug, the pointer-CAS `Ambiguous` gap this RFC's own
prerequisite fix closed) were each found by a human noticing a specific scenario and
writing a test for it, not by systematic exploration. A model checker explores the
reachable state space of a specified protocol exhaustively (bounded by the model's
finite instantiation); a DST harness explores the *real* code's behavior under
controlled, replayable fault injection. Neither alone closes the gap the other leaves:
TLA+ alone proves a model correct, with no mechanical guarantee the Rust code matches
the model; DST alone explores the real code, with no way to know if the space it
explored was ever exhaustive.

## Non-goals

This RFC does not verify: the container byte format (RFC 0001 §1, orthogonal); S3 or
MinIO's own correctness (assumed, per RFC 0001's existing empirical verification
against real MinIO); GCS/Azure semantics (R5, open, unrelated); or the format's
performance properties (`docs/benchmarks.md`'s domain, not this one). It does not
propose changing the manifest protocol itself — if the verification work surfaces a
protocol defect, the fix is a revision to RFC 0001 or a follow-on RFC, not silently
folded in here. It does not commit to formally verifying every future STRAND
subsystem; this RFC is scoped to the manifest CAS protocol specifically, the one
piece of the format that is concurrent and retry-based in a way that makes exhaustive
human-authored test coverage genuinely hard to trust. It does not model table
metadata, retention policy, compaction, or the orphan sweep — all M3
(`spec/manifest.md` §5), not yet implemented. Modeling ahead of M3 is a deliberate
scoping choice (Open Questions, below, says why), not an oversight; a follow-on
revision is expected once M3 lands.

## Design

### 1. Architecture

Two independently-checkable artifacts, connected by a shared trace vocabulary:

- **The TLA+ model** (`verification/manifest.tla`, new): the commit and read
  protocols, specified at **Delta Lake's action granularity, not Raft's** — a
  deliberate choice, informed by (not copied from) Jack Vanlightly's public Delta
  Lake TLA+ model, whose actions (`StartOperation`, `ReadDataFiles`, `WriteDataFiles`,
  `TryCommitTxn`) match the shape of RFC 0001 §3's steps far more closely than a
  fine-grained, message-passing spec (`raft.tla`, vendored as part of the
  `spacejam/tla-rust` example set, `references/spacejam-tla-rust.md`) would — the
  Raft-vs-Delta-Lake contrast is this RFC's own framing, not Vanlightly's; his post
  makes no such comparison (`references/vanlightly-delta-lake-tla-plus.md`). §4 below
  sketches the action grammar this implies, extended to the reader side.
- **A TLAPS proof of the model's core safety properties** (also new, alongside the
  TLA+ model): per the user's choice of the full original scope, this RFC commits to
  a machine-checked proof of the safety properties listed in "Open questions" below —
  not only a bounded TLC check — following the shape FMDSE's own case study used for
  a comparably-scoped protocol (`references/fmdse-blockchain-conformance-testing.md`).
  TLC's bounded exhaustive check remains valuable in its own right during
  development (fast counterexample discovery at small model instantiations) but is
  not a substitute for the proof once the model stabilizes.
- **The DST harness** (`crates/strand-core` test/bench infrastructure, new): drives
  the real `commit()`/`read_snapshot()` functions against a `ConditionalStore`
  implementation that is deterministic (seeded), controllable (can inject
  `PreconditionFailed`, `Io`, and — per the fix already landed —
  `Ambiguous` outcomes at chosen points), and instrumented to emit the same
  trace vocabulary the TLA+ actions use.
- **The trace vocabulary**: the shared language both sides speak. Each real or
  modeled action (read-current, propose-snapshot, try-advance-pointer with its four
  possible outcomes, resolve-ambiguity) is one trace event, carrying only the fields
  the protocol's safety properties actually depend on (writer identity, the version
  and row-ID range attempted, the outcome) — deliberately not lower-level noise
  (exact byte contents, wall-clock timing, connection pool state). Designing this
  vocabulary correctly is itself real design work (§3 of the earlier discussion this
  RFC follows from), not a mechanical step.

### 2. Sequencing: Workflow II before Workflow I

Following FMDSE's own bidirectional framing (`references/fmdse-blockchain-
conformance-testing.md`), there are two directions this dual-validation can run, and
they are not equally hard — though not for the reason an earlier draft of this RFC
gave:

- **Workflow II — spec drives implementation.** TLC (or a similar model-space walker)
  generates a large set of valid action sequences from the TLA+ spec. The DST harness
  replays each sequence directly against the real Rust code — this action, then this
  one, then this one, with the specified fault injected at the specified point — and
  checks the real code's outcome matches what the spec predicted.
- **Workflow I — implementation drives verification.** The DST harness runs the real
  code under its own exploration (concurrent writers, randomized fault injection,
  many seeds), emits traces from what actually happened, and those traces are checked
  for conformance against the TLA+ spec after the fact.

**The actual argument for building Workflow II first is structural, not a claim
that Workflow I doesn't work in production** (an earlier draft cited AWS's PObserve
for that claim; PObserve is itself a real, working production example of Workflow
I's shape — observe real execution, reconcile against a pre-existing spec after the
fact — which is evidence Workflow I is achievable, not evidence it should go
second; see `references/aws-pobserve-p-language.md` for the correction). The real
asymmetry is about where the discovery problem sits. In Workflow II, this RFC
chooses the action sequence and drives the harness through it directly — there is
no ambiguity about where one action ends and the next begins, because the harness
only knows how to receive commands phrased in the trace vocabulary; a mismatch
surfaces immediately, at the exact action that failed to replay. In Workflow I, the
real code runs on its own and *produces* a trace that must then be independently
shown to decompose into the spec's action grammar at all — a discovery problem, not
a given. MongoDB's documented experience is the concrete cautionary case: ten weeks
of trace-checking effort against real (not simulated) execution did not yield one
successfully validated spec, in part because a leader step-down/step-up sequence
the implementation performed as two separate steps had been modeled as a single
atomic spec action, and repeated attempts to paper over the mismatch with
post-processing never worked (`references/mongodb-conformance-checking.md` — the
exact quotes, corrected from an earlier draft's embellishment of this same
anecdote, which is not documented as "deterministic" or "100%-consistent" in the
source, only as a structural difference no post-processing fixed).

This RFC proposes building Workflow II first, using it to establish — by
construction, since the harness is driven rather than left to decompose its own
spontaneous trace — that the trace vocabulary and the spec's action granularity
actually correspond to the real code's behavior, and only then attempting Workflow
I. This buys confidence that the vocabulary itself is sound before betting
diagnostic effort on discovering whether the real code's *spontaneous* concurrent
trace decomposes the same way — it does not, by itself, guarantee that Workflow I
will succeed once attempted, and "How this could be wrong" below keeps that
distinction explicit rather than claiming more than Workflow II can prove.

### 3. Drift classification

Per the earlier discussion this RFC formalizes: drift between the two sides is a
diagnostic signal, never a target to tune away directly. Every disagreement is
classified before anything is changed:

| drift type | meaning | fix |
| --- | --- | --- |
| Type-I | the Rust code did something the spec forbids | a real bug — fix the Rust code |
| Type-II | the spec permits something the Rust code can't or doesn't do | fix whichever side is not authoritative for that behavior (see below) |
| tracer artifact | neither side is wrong; the trace vocabulary's abstraction boundary is | fix the tracer, not the model or the code |
| fault-model mismatch | the DST harness's simulated `ConditionalStore` failure behavior doesn't match what the real backend actually does | fix the simulated fault model, not the spec or the Rust protocol logic |

**Resolving Type-II: which side is authoritative.** Type-II means the model
*permits* some behavior the Rust code never actually produces. The deciding
question is whether RFC 0001 (or its governing spec chapters) actually intends the
protocol to support that behavior. If it does, the code is incomplete relative to
the design — not the model too loose — and the Rust code is extended to cover it.
If it does not, the model is simply more permissive than the protocol was ever
designed to be, an artifact of how the spec was written rather than a real
requirement, and the *model* is tightened to forbid it. This is a design-intent
question, answered by consulting RFC 0001/`spec/manifest.md`, not answered by the
model or the code alone. (A previous version of this paragraph reasoned about "the
behavior the model forbids," which is Type-I's modality, not Type-II's, and its
second branch described a Rust bug producing forbidden behavior — Type-I again,
not Type-II; corrected here to stay in "the model permits it, the code doesn't do
it" terms throughout.)

**The fourth drift source (fault-model mismatch) is distinct from a tracer
artifact.** A tracer artifact is about vocabulary — the trace vocabulary
mis-describing what happened. A fault-model mismatch is about fidelity — the DST
harness's simulated `ConditionalStore` claiming a failure mode is possible (or
impossible, or shaped a certain way) when the real backend disagrees. RFC 0001
notes GCS/Azure conditional-write semantics are unverified (R5, `docs/ledger.md`),
and this RFC's `StoreError::Ambiguous` classification (`crates/strand-core/src/
s3_store.rs`) is derived from the real AWS SDK's documented `SdkError` variants for
S3 specifically — a DST harness whose simulated fault injection doesn't match a
real backend's actual failure shapes can produce drift that reflects nothing about
either the spec or the Rust protocol logic, only a wrong simulation.

DST's determinism is what makes classification tractable regardless of which of
the four types is in play: a drift instance is a seed, and a seed is replayable,
so a specific disagreement can be bisected to its exact point of divergence at
zero flake cost, the same way a real network's nondeterminism never allows.

### 4. A sketch of the action grammar

Not the spec itself — a sketch showing the granularity this RFC commits to, so the
adversarial review has something concrete to push on before real TLA+ is written.
The first review found this sketch covered only the writer path; it now covers both,
matching `commit()` and `read_snapshot()`/`try_read_current()`/`read_current()` in
`crates/strand-core/src/manifest.rs` and the reader protocol in `spec/manifest.md`
§3.

**Writer path:**

```
ReadCurrent(w)              \* writer w reads _strand/current + the snapshot it names
ProposeSnapshot(w, v)       \* writer w writes a new snapshot object for version v
TryAdvancePointer(w, v)     \* writer w attempts the pointer CAS; outcome ∈
                             \*   {Success, PreconditionFailed, DefiniteFailure, Ambiguous}
ResolveAmbiguity(w, v)      \* on Ambiguous: writer w re-reads the pointer to
                             \*   determine whether its own write landed
```

**Reader path:**

```
ReadPointer(r)               \* reader r issues GET _strand/current; outcome ∈
                              \*   {Found(path), Absent, DefiniteFailure}
ReadSnapshotObject(r, path)  \* reader r issues GET on the snapshot metadata
                              \*   object `path` names; outcome ∈
                              \*   {Found(snapshot), Expired, DefiniteFailure}
RefreshAndRetry(r)           \* on Expired: r re-issues ReadPointer, bounded by
                              \*   READER_REFRESH_RETRY_LIMIT attempts total
RetriesExhausted(r)          \* the bound in RefreshAndRetry is reached without
                              \*   ever landing on a readable snapshot
```

`DefiniteFailure` at either reader action propagates immediately as
`ReadError::Io` (`manifest.rs`'s `read_snapshot`, which does not retry it),
distinct from `Expired`, which continues the bounded refresh loop, and from
`RetriesExhausted`, which is reached only by exhausting that loop on repeated
`Expired` outcomes, never on a `DefiniteFailure`.

Each is one coarse, per-attempt action — matching the real functions' structure
directly, not a decomposition into the individual HTTP requests each one issues.
`ResolveAmbiguity` exists as its own action specifically because of the fix RFC
0001's implementation already landed (`store.rs`'s `StoreError::Ambiguous`,
`manifest.rs`'s pointer-CAS disambiguation) — this RFC's model must cover it, not
the two-outcome CAS RFC 0001 originally described. The reader path has no
`Ambiguous`-shaped outcome of its own: a `get` has no side effect to reconcile, so
`store.rs`'s own `StoreError` doc comment already treats a `get`-side ambiguous
failure the same as a definite one, and the model follows that same collapse
rather than inventing a distinction the real code doesn't make.

### 5. Effort, honestly

FMDSE's own reported cost for a comparably-scoped protocol (`references/fmdse-
blockchain-conformance-testing.md`): a 675-line TLA+ spec, a 1,282-line TLAPS
proof (machine-checking in about two minutes once written), a roughly 2,000-line
custom simulator (1,000 lines core driver, 1,000 lines network/DES abstraction)
against a 2,411-line Go implementation, for **approximately two person-months,
distributed across three engineers** (the source states the effort and
headcount, `references/fmdse-blockchain-conformance-testing.md`; it makes no
claim about the team's prior expertise with the technique, so none is asserted
here). STRAND's
manifest protocol is smaller than FMDSE's consensus-protocol case study — no
quorum, no leader election — but this RFC does not assume the cost scales down
proportionally with protocol size; the TLAPS proof step in particular is not
obviously cheaper just because the model has fewer states, and this RFC has no
independent estimate to offer in its place.

`spacejam/tla-rust` (`references/spacejam-tla-rust.md`) is named here as the
actual practical risk, not the granularity-matching question (a design discipline
§2's sequencing already addresses by construction): confirmed by listing its full
file tree, the repository contains only TLA+/PlusCal source and vendored example
specs — no Rust implementation was ever begun there. It attempted the spec half of
roughly this architecture and stopped; that is this project's nearest precedent
for a stalled effort of this specific shape, not a completed one to learn
implementation lessons from.

## How this could be wrong

**The model's own action grammar is a human enumeration too, not an escape from
that limit.** The second adversarial review's caveat, not fully resolved here:
TLC's exhaustive check is exhaustive only over the actions §4 actually
specifies. An action class nobody thought to model (a fifth writer-side
outcome, a reader-path interaction the current grammar doesn't capture) is
invisible to TLC in exactly the way an unmodeled fault type is invisible to
`proptest`'s generator — the "known unknowns vs. unknown unknowns" framing in
the Status header is directionally right (TLC explores the *combinations* of
modeled actions exhaustively, which a sampled generator does not, and that is
real and valuable) but is not a categorical escape from needing the action
grammar itself to be complete. §4 should get the same adversarial scrutiny for
missing action classes that "Alternatives considered" already gives the
property-based generator's `RoundPlan` enumeration — this has not yet
happened and is worth doing before the model is treated as settled.

**This could simply not get finished.** `spacejam/tla-rust`'s dormancy is this
project's nearest grave for this specific kind of effort — not a format grave from
`docs/lineage.md`, but a formal-methods-tooling grave nonetheless. A multi-week,
two-artifact verification effort that stalls halfway leaves a partially-built TLA+
spec and a partially-instrumented DST harness that verify nothing and cost ongoing
maintenance to keep merely compiling. Mitigation: Workflow II is scoped to be a
complete, useful artifact on its own — "TLC-generated traces replay cleanly against
the real code" is a real, checkable claim even if Workflow I never starts.

**Granularity agreement is a discipline that must be sustained, not a decision made
once.** §2's sequencing avoids MongoDB's specific day-one failure, but every future
change to `commit()` that adds, removes, or reorders a step requires a matching
change to the TLA+ actions and the trace vocabulary, kept in lockstep, for the life of
the protocol. Nothing in this design enforces that automatically; a CI check that
fails when the trace vocabulary's event names drift from the actual function calls
emitting them (a lint, not a proof) is worth scoping as a follow-on, not yet designed
here.

**The proof-checker is itself trusted, unverified software.** TLAPS's own soundness
is assumed, not verified by this RFC — standard for all formal-methods work, worth
stating rather than leaving implicit, since CLAUDE.md's own culture is to name
assumptions rather than bury them.

**This is a real cost for a small protocol, accepted deliberately, not
discovered late.** The manifest protocol is single-writer-per-attempt with bounded
retry, not a distributed consensus protocol — its state space is genuinely smaller
than Raft's or Delta Lake's own multi-table transaction model, and the
property-based alternative (§"Alternatives considered," "Evidence gathered" above)
demonstrably catches this protocol's known bug class at a fraction of the cost §5
states plainly. The first adversarial review treated that as grounds to pause this
RFC pending evidence the cheaper alternative was insufficient. The user has since
made a considered decision to proceed regardless, on grounds this RFC's Status
header states precisely: property-based testing and exhaustive model checking
answer different questions, and a green test suite is not evidence there is
nothing left for the model checker to find. This RFC records that decision rather
than re-litigating it, but the underlying cost is real, and this section exists so
a future reader — including a future session revisiting this RFC — sees the
trade-off was made with eyes open, not glossed over because a green test suite made
it easy to stop asking.

## Alternatives considered

**Property-based testing (proptest/quickcheck-style) instead of TLA+ + DST.**
Not rejected on paper — actually built and evaluated (see "Evidence gathered"
above): `crates/strand-core/src/manifest.rs`'s `tests::property` module, which
caught both of this protocol's known historical bugs under mutation testing, one
via a shrunk minimal counterexample. Far cheaper to build and maintain than a TLA+
model plus a proof plus a dual-tracing harness. What it structurally cannot give,
however many cases it runs: a scenario outside the fault/round vocabulary its
generator was written with (`RoundPlan`'s four variants are an enumeration a human
wrote down, not a state space discovered independent of that enumeration), TLC's
exhaustive-within-bounds exploration of the *specified* protocol, or a
machine-checked proof of the safety properties rather than a statistical increase
in confidence from sampling. This is why the two are pursued together rather than
the cheaper one substituting for the other, per the Status header.

**`loom` for exhaustive interleaving testing.** Rejected as a fit for this protocol:
loom exhaustively explores thread-interleavings of in-process shared-memory code
under the C11 memory model, not the network-level, request/response concurrency this
protocol actually has (writers on different processes, possibly different machines,
racing on an object store's CAS semantics). Loom's model does not capture what this
protocol needs verified.

**TLA+ alone, no DST.** Rejected: proves the model correct, gives no mechanical
signal about whether the Rust implementation matches it — precisely the gap that
motivated this RFC.

**DST alone, no TLA+.** Rejected: explores the real code under fault injection and
concurrency, but with no way to know the exploration was ever exhaustive, or that the
invariants checked are complete — the same gap from the other side.

## Open questions / follow-on RFCs

- The concrete list of safety and liveness properties to prove is not yet fixed.
  A starting set, to be finalized in the approved version of this RFC or a follow-on:
  no two `SegmentRef`s across the full committed segment set have overlapping row-ID
  ranges; a committed snapshot's `version` and `next_row_id` are both strictly
  monotonic across commits; every committed snapshot is reachable by following prior
  versions back to version 0 (no "orphaned" committed state); a writer retrying under
  bounded rival contention eventually commits (liveness, under a fairness assumption
  on the CAS host — needs its own justification, since real object storage gives no
  formal fairness guarantee).
- Tooling choice (TLC vs. Apalache; whether the originally-discussed fork of the TLA+
  Java trace-validation tooling is actually necessary once Workflow II's simpler
  "TLC generates, harness replays" shape is adopted, versus the heavier
  observe-and-reconcile tooling Workflow I would need) is not decided here.
- The CI lint that keeps the trace vocabulary and the real code's emission points in
  lockstep (named in "How this could be wrong" above) is not designed here.
- How the DST harness's simulated `ConditionalStore` fault model will itself be
  validated against real S3/MinIO behavior (and, eventually, GCS/Azure once R5
  resolves) — the "fault-model mismatch" drift source in §3 — is not designed here;
  the existing `crates/strand-core/tests/s3_store.rs` real-MinIO integration tests
  are a starting point but were not written with this purpose in mind.
- This RFC's model covers the manifest protocol's *current* surface: no table
  metadata, no retention policy, no compaction, no orphan sweep — all M3
  (`spec/manifest.md` §5 lists what's not yet implemented). Building the model now,
  ahead of M3, is a deliberate scoping choice, not an oversight: it establishes the
  Workflow II infrastructure and the trace vocabulary against the protocol surface
  that already exists and is already exercised by real tests, rather than waiting on
  work that has no fixed timeline. It will need a follow-on revision once M3's
  deletion and retention semantics land — the reader-side `Expired`/404-refresh
  action in §4 already exists in the current protocol, but the *conditions* under
  which a real deployment triggers it (compaction) don't yet.

## Discussion — post-approval amendments

Per `CLAUDE.md` §3, a design problem revealed after approval is recorded here, in the
RFC, rather than folded silently into the model. §4's action grammar is unmodified
above; this section is the record of what changed and why.

**§4's writer-path actions listed no outcome set for `ReadCurrent` or
`ProposeSnapshot`.** Unlike `TryAdvancePointer` (`{Success, PreconditionFailed,
DefiniteFailure, Ambiguous}`) and the reader path's `ReadPointer`/`ReadSnapshotObject`
(each with an explicit outcome set), §4 as approved gave these two writer-path actions
no outcome set at all — an omission, not a decision that they have exactly one
outcome. The gap was harmless to every invariant this RFC's model checks (a failed
read or propose reaches a terminal state, or loops back to one, that no invariant
observes), but would have entrenched itself into a TLAPS proof built on the
as-approved grammar. `docs/ledger.md` recorded it as a correspondence gap to close
before that phase starts; it is closed here.

`ReadCurrent(w)` gained a third branch: an `Expired` self-transition back to `"Read"`,
grounded in `read_current()`'s real behavior (`crates/strand-core/src/manifest.rs`) —
it loops unboundedly on a 404 race against the snapshot object, unlike the reader
path's bounded refresh, because the writer's real bound is the pointer CAS it is
about to contend on, not this read. `ProposeSnapshot(w)` gained a second branch: a
single collapsed failure outcome covering both `StoreError::Io` and
`StoreError::Ambiguous`, since `commit()`'s snapshot-object write needs no
disambiguation on ambiguity — its path is attempt-unique (the `writer_nonce`), so a
landed-but-unacked write is simply a harmless orphan (`CLAUDE.md` §6), never a source
of doubt about what the writer itself believes happened.

Both additions are grounded and mutation-test-free by design: neither changes an
invariant's truth value on any reachable path (`ReadCurrent`'s branch cannot reach a
new state at all, being a true self-loop; `ProposeSnapshot`'s failure branch reaches
only the pre-existing `"Failed"` terminal state, which `WriterSuccessIsCommitted` and
every other invariant already ignore) — re-running TLC after the change is the
verification that matters here, not a new mutation test. TLC's state count moved from
561 distinct states (1487 generated) to **591 distinct states (1793 generated)**,
depth unchanged at 14, all seven invariants still holding; `verification/README.md`
carries the new baseline. Landed 2026-08-18, task: "Start with the TLA+ gap."

**RFC 0012's own review found a second, separate model correspondence gap**
(`ProposeSnapshot`'s only transition is `Append`, with no shape for revising an
existing entry in place — `commit_deletion_vector`'s real behavior). Closed the same
day, prompted by the user's own recommendation to sequence the model extension
before either remaining artifact: a new `ProposeDeletionVectorCommit(w)` action, a
new `DeleteWriter` CONSTANT (mirroring `DistinguishedWriter`'s established pattern),
and two new invariants (`SegmentCountNeverDecreases`,
`DeletionVectorCommitsOnlyReviseOneEntry`), both confirmed load-bearing by real
mutation tests. TLC's state count moved from 591 (1793 generated, depth 14) to
**5,943 distinct states (22,286 generated), depth 18**, all nine invariants holding.
Full account: RFC 0012's own Discussion section
(`rfcs/0012-deletion-vectors.md`); `verification/README.md` carries the new
baseline.

**Milestone reassignment**, prompted directly by the user asking where this work
sits on the M0–M5 roadmap and then asking for it to be placed there properly, not
left as ungated cross-cutting work indefinitely. The original "does not gate any of
M1–M5" was correct when written — the model covered exactly one commit shape
(`commit`'s append), and nothing downstream depended on proving it further. That
stopped being true once a second commit shape (`commit_deletion_vector`'s
revise-in-place) existed, and will stop being true again, more consequentially,
when compaction adds a **third** (merge multiple source segments into one, under
the same pointer CAS) — M3's own named deliverable. Reasoning for gating M3's
compaction work specifically, rather than continuing to treat verification as
perpetually optional: piling a third unverified commit shape onto a protocol with
zero mechanized proof and zero cross-validation against the real Rust code is
exactly the kind of accumulating, unexamined risk this project's own "verification
rigor sequencing" discipline exists to catch before it compounds, not after value
has already been built on top of it. `docs/milestones.md`'s M3 entry now states
this gate explicitly: a TLAPS proof of the model as it stands (covering `commit`
and `commit_deletion_vector`) and the DST cross-validation harness (Workflow II
first, per §2's own approved sequencing) both land before compaction's own
commit-path design work starts, so that work extends a model already proven to
correspond to the real code rather than stacking a third hopeful, unverified
extension on top. Neither artifact is built yet. Landed 2026-08-19.

**The DST cross-validation harness (Workflow II) is built and run — M3-3
(`docs/roadmap.md`).** This closes one of the two remaining artifacts this
section names above; the TLAPS proof (M3-2) is still open.

**Mechanism, researched live rather than assumed** (§2's own Open Questions
left "how the action sequences are actually obtained" undecided). TLC's real
flag list (`java -cp tla2tools.jar tlc2.TLC -help`, `tla2tools.jar` 2.19,
already cached in this environment) offers no mode that emits an action
*name* per step — `-dump` writes the whole reachable state graph, not one
sequence, and the heavier TLA+ Trace Validation / `TLCTrace` tooling this
RFC's Open Questions flagged as possibly unnecessary is built for Workflow
I's observe-and-reconcile shape (checking a trace *against* a spec after
the fact), not Workflow II's simpler "spec generates, harness replays" one.
The mechanism this harness actually uses is `-simulate num=N,file=PREFIX`:
real random-simulation runs of the spec's own `Init`/`Next` relation, each
written out as a numbered `STATE_1 == ... STATE_k ==` sequence of full
model states (not action labels) in a `.tla`-shaped trace file. Confirmed
empirically before committing to it: a `-workers W` run produces `W ×
num` trace files (`tr_<worker>_<n>`), and a state where every enabled
transition has already run into a terminal process-counter value ends the
trace early (a real deadlock in the simulator's sense, not a bug) — both
behaviors are used, not fought, by the harness below. Since no state
carries an action label, the harness reconstructs which action fired
between two consecutive states by diffing their writer/reader
process-counter variables (`wPc`/`rPc`) — sound here specifically because
`Next` is a disjunction of single-process actions (never two processes
stepping in the same transition), confirmed by asserting exactly this
invariant while parsing every real trace in the runs below, and because
every `(from_pc, to_pc)` pair in §4's action grammar is unique across the
whole model (worked out by hand while designing the classifier, in
`crates/strand-core/src/bin/dst_manifest_harness/replay.rs`'s module doc).
The one action this scheme cannot see at all is `ReadCurrent`'s `Expired`
self-loop (`manifest.tla`'s own comment: "a true self-loop... cannot
introduce a new reachable state"), which is correct, not a gap: it changes
nothing observable, so there is nothing for a real replay to check.

**Where the harness actually lives, and the granularity mismatch it
resolves.** `crates/strand-core/src/bin/dst_manifest_harness/` (`trace.rs`:
the trace-file parser above; `replay.rs`: everything below; `main.rs`: CLI,
TLC invocation, the report), a new `[[bin]]` named `dst-manifest-harness`
in `crates/strand-core/Cargo.toml`. It was deliberately **not** placed
inside `strand-core`'s own `#[cfg(test)]` modules: §4's action grammar
(`ReadCurrent`/`ProposeSnapshot`/`TryAdvancePointer`/`ResolveAmbiguity`) is
individually-firable in the model, but the real `commit()` is one function
running its own internal retry loop over exactly that sequence with no
external pause point — there is no private per-step entry point to drive
"this action, then this one" against from outside. Living in `src/bin/`
means this harness sees exactly the public API (`commit`,
`commit_deletion_vector`, `read_snapshot`, `ConditionalStore`,
`InMemoryStore`) an external consumer would, and resolves the mismatch by
replaying **one writer's whole trajectory as one real `commit()` (or
`commit_deletion_vector()`) call**, with writers replayed in the order
each first reaches its own terminal process-counter value in the trace.
That ordering is load-bearing, not arbitrary — `replay.rs`'s own module doc
proves why: any rival whose landing causes another writer's real
`PreconditionFailed` must itself have reached `Done` strictly earlier in
trace order, so replaying in terminal-order means every such rival has
already landed for real, via its own real `commit()` call, by the time the
writer observing staleness runs. Real staleness therefore emerges from the
real `InMemoryStore`'s own real ETag comparison — never injected — and only
the outcomes a plain store cannot produce on its own (`Io`, `Ambiguous`,
and reader-side `Expired`) are scripted, via a `ConditionalStore` wrapper
built directly from each trajectory's own trace-derived script. Because
this collapses some model-predicted stale-retry cycles into a single real
attempt that simply starts past the point of staleness, comparison is done
at **outcome** level — final `Ok`/`Err`, final version/segments/next_row_id
— matching this section's own phrasing ("the real code's outcome matches
what the spec predicted"), not at exact internal retry-count. What this
ordering cannot reproduce is a rival landing strictly *between* one
writer's own `ReadCurrent` and its own `TryAdvancePointer` within a single
cycle (true sub-call-granularity interleaving) beyond the `Ambiguous`
landed/not-landed axis the model actually needs; `replay.rs`'s module doc
states this scope limit in the same place a reader will look for the
ordering argument, rather than only here.

**DST's own replayability property, checked, not just claimed.** The only
randomness anywhere in this pipeline is TLC's `-seed`; the harness's own
replay logic is a pure function of a parsed trace file. Re-running the
exact command line the harness itself prints (seed 111, `-workers 1`,
`num=100`, `-depth 25`) reproduced byte-identical trace files and an
identical report (300 writer trajectories: 255 matched / 45 skipped / 0
drift; 100 reader trajectories: 100 matched / 0 skipped / 0 drift, twice)
— confirmed directly, not assumed. `-workers` is pinned alongside `-seed`
in the reproducibility contract because TLC splits its RNG stream across
workers.

**Two real bugs, both in the harness itself, caught by its own output
before they could mask a real finding — worth recording exactly because
CLAUDE.md §2 says a wrong number is deleted, not softened, and the same
discipline applies to a wrong *classification*.** First, an off-by-one:
`manifest.tla`'s `rLocal[r].ptrVersion` is `Len(snapshots)` — TLA+'s
1-indexed sequence length — used to index `history`, a 0-indexed Rust
`Vec`; the first version of `replay_reader`'s seeding loop read
`history[ptr_version]` instead of `history[ptr_version - 1]`, which
silently reported "no entry for this version" as a *skip* for every reader
trajectory that actually read a committed snapshot, at a small trace
count. Second, `commit_deletion_vector` replay bailed out early with
"needs an existing real segment" whenever the real store had none yet —
correct when the trajectory's own first action genuinely needs one, wrong
when the trajectory's script fails at `ReadCurrent` before `segment_path`
is ever inspected (`manifest.rs`'s own control flow), which made ~30% of
`DeleteWriter` replays skip needlessly at a 20-trace smoke test. Both were
caught by watching the skip-reason counts move in ways the trace content
didn't justify, both fixed before the run cited below, and neither is a
manifest protocol bug — RFC 0002 §3's fourth category, fault-model
mismatch, is about the *simulated backend*'s fidelity; these were bugs in
the harness's own trace-to-replay translation, a distinct failure mode
worth naming so a future reader does not conflate "the harness had a bug"
with "the protocol had a bug." Neither survived past this same session.

**The run.** Seed 20260819 (the harness's own default, so a bare
invocation reproduces it), `-workers 1`, `num=1000` per worker, `-depth
40`: `java -XX:+UseParallelGC -jar tla2tools.jar -workers 1 -config
verification/manifest.cfg -simulate num=1000,file=... -depth 40 -seed
20260819 verification/manifest.tla`, then `cargo run -p strand-core --bin
dst-manifest-harness -- --seed 20260819` replaying the result. 1,000 trace
files generated and parsed clean, 8,396 total states. **3,000 writer
trajectories replayed: 2,511 matched, 489 skipped (never reached a
terminal process-counter value within this trace's depth — a legitimate,
reported non-outcome, not a failure), 0 drift.** **1,000 reader
trajectories replayed: 1,000 matched, 0 skipped, 0 drift.** Of the
replayed trajectories, 2,335/3,000 writers and 501/1,000 readers required
injecting at least one non-trivial fault outcome (`Io`, `Ambiguous`, or
reader-side `Expired`) to reach their trace-predicted terminal — meaning
most of the corpus actually exercised §4's fault branches, not only the
uncontended happy path. **Zero drift, of either RFC 0002 §3's four types,
across this run.**

**What this does and does not establish, stated with the same care §2's
own text used.** This is real, positive evidence for exactly the thing
Workflow II was built to check first: that the trace vocabulary and the
model's action granularity (§4) actually correspond to the real code's
behavior across a real, non-trivial sample of the state space TLC's random
simulation reached — not a cherry-picked handful of scenarios. It does
**not** prove the TLA+ model is complete (the second adversarial review's
own caveat stands: TLC explores the *modeled* action space exhaustively-
within-bounds, not action classes nobody wrote down), does not prove
liveness (out of scope per the Open Questions), and — per §2's own
argument — does **not** by itself establish that Workflow I will succeed
once attempted: this shows *driven* replay of spec-generated sequences
works, not that the real code's own *spontaneous* concurrent trace
decomposes at the same boundaries, which is MongoDB's specific documented
failure mode and remains a live risk for Workflow I specifically. Workflow
I is unstarted, per §2's own approved sequencing and this task's own
explicit scope.

**Classification, honestly scoped.** The harness reports human-readable
mismatch detail (which writer/reader, which trace file, trace-predicted
outcome vs. real outcome) sufficient for a human to classify any future
drift against §3's four-way table; it does not attempt automatic
classification into Type-I/Type-II/tracer-artifact/fault-model-mismatch —
that judgment, per §3's own text, is a design-intent question answered by
consulting this RFC and `spec/manifest.md`, not a mechanical one. No drift
occurred in the run above, so this capability is documented but untested
against a real instance; the next session that finds real drift should
expect to do the classification by hand and record it here, not expect the
harness to have already done it.

Landed 2026-08-20.
**TLAPS proof of the model's five writer actions — landed 2026-08-20,
`docs/roadmap.md` M3-2.** The first of the two remaining artifacts named
above now has real, mechanically-checked progress, honestly partial. Every
claim below was independently confirmed by running `tlapm` and reading its
own final summary line — never assumed from a `THEOREM`'s presence in the
file alone, per this task's own explicit honesty requirement.

A real toolchain fix was needed before any proof could be attempted: TLAPS
1.5.0 rejects any module extending `manifest.tla` outright, because its
level-checker cannot process `SumCounts`'s `RECURSIVE`-operator definition
(an assertion failure in `e_levels.ml`, "Recursive operator definitions are
not supported"). Fixed by rewriting `SumCounts` to TLA+'s native
recursive-*function* syntax, a semantics-preserving change re-confirmed
against TLC (identical 5,943 states, 22,286 generated, depth 18, before and
after). `verification/README.md`'s new "TLAPS proof" section carries the
full account, including a "lessons for extending this proof" list — several
genuinely reproducible backend-flakiness patterns (existentials spanning a
state transition, repeated large inline subexpressions, `<=`-transitivity
through record-field expressions) that cost real iteration to diagnose and
are worth a future session reading before adding to this file.

`verification/manifest_proofs.tla` (new, `EXTENDS manifest` so TLC's own
checking of the model stays untouched by proof iteration) proves
`IndInv1 == TypeOK /\ WriterSuccessIsCommitted /\ ReaderSeesOnlyCommitted
/\ FnDomains /\ BaseVersionBounded /\ ProposedIsReal` inductive across
`Init` and all five of the model's **writer**-path actions (`ReadCurrent`,
`ProposeSnapshot`, `ProposeDeletionVectorCommit`, `TryAdvancePointer`,
`ResolveAmbiguity` — the actions matching `commit()`'s and
`commit_deletion_vector()`'s real control flow, which is the literal scope
this task named). Three of `IndInv1`'s six conjuncts (`FnDomains`,
`BaseVersionBounded`, `ProposedIsReal`) are not among `manifest.cfg`'s own
seven TLC-checked invariants — each is a fact true by construction that
still needed stating as its own explicit inductive conjunct before TLAPS
could use it, found by trying to type-check an obligation without it and
watching tlapm reject the gap. None weakens or contradicts the seven
TLC-checked invariants.

**Fully proved, confirmed by a clean `tlapm` run reporting
`[INFO]: All 1261 obligations proved.` and exit code 0, reproduced
identically on two separate cache-cleared runs**: `Init1` and one step
theorem per writer action, each of the shape `IndInv1 /\ Action(w) =>
IndInv1'`. Chained together this is a genuine inductive-invariant proof —
not a bounded check — that across any sequence of these five actions,
`TypeOK` never breaks, `WriterSuccessIsCommitted` holds (no writer that
reports success has actually lost its own committed data — the property
this RFC's own Summary names first: "lose a writer's data"), and
`ReaderSeesOnlyCommitted` holds (no reader ever observes a snapshot that
was never really committed).

**Addendum, 2026-08-20 (a false completion claim, caught by independent
review, corrected).** An earlier pass of this same task-level effort
reported 1,247 obligations proved and treated that as settled. An
independent adversarial reviewer re-ran `tlapm` fresh (cleared proof
cache, matching backend versions) against that exact commit and got a
different, real result: one obligation failed
(`ProposeDeletionVectorCommitStep1`'s `<3>eq` step, which proved the
target `EXCEPT` expression equal to a literal record before checking the
literal's membership), reproduced failing on 4 separate cache-cleared
runs. The fix replaces that step with `ExceptSegmentDelVer`, a small
reusable lemma proving the same membership fact directly, field by field,
the same technique `ExceptProposedAt` already used elsewhere in this file
— not a patch to the failing step, a different proof strategy for it.
The corrected 1,261-obligation count above was independently reproduced
twice, fresh, before being written down here; `verification/README.md`'s
"Lessons" section carries the full account, including why the
literal-equality route looked like it worked but did not reliably hold.
Nothing about this correction changes the *scope* named below — the
writer-path proof this section claims was always the right scope, and
remains exactly as scoped; only the obligation count and the reliability
of one internal step were wrong, and both are now independently
verified, not merely re-asserted.

**Explicitly not yet attempted, so partial scope is never mistaken for
complete coverage**: the reader-path actions (`ReadPointer`,
`ReadSnapshotObject`); the `Next`-level case-split composing all seven
actions into one property and the temporal invariance theorem itself
(`Spec => []IndInv1`, needing TLAPS's `PTL` backend) — today's six theorems
are independent per-action facts, not yet assembled into that single
statement; and the model's other six TLC-checked invariants, most notably
`NoOverlappingRowIds` — the invariant most directly answering "no two
writers' CAS-raced commits silently overlap row-ID ranges," arguably the
more central of the two properties this RFC's own Summary names
("silently commit overlapping row-ID ranges or lose a writer's data").
`NoOverlappingRowIds` needs a materially harder inductive strengthening
(proving each snapshot's segments are packed contiguously, by induction
over `SumCounts`'s recursive structure) than anything this pass attempted;
it is not close to done, not merely unstarted busywork. **This paragraph
described the DST cross-validation harness (this RFC's third artifact) as
entirely unstarted when first written; the harness now exists** (Workflow
II, M3-3, above) — the sentence is corrected here rather than left stale
now that both entries live in the same section. `docs/ledger.md` carries
the same accounting for the settled-vs-open ledger's sake, and
`docs/roadmap.md`'s M3-2 entry is updated to reflect partial, not
complete, status — M3's compaction gate (this section, above) still needs
the remainder of this artifact (the reader-path actions, the `Next`-level
composition, and the other six invariants named just above) before it is
satisfied; M3-3's own gate contribution is done.
