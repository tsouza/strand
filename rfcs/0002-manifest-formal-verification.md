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
- **Milestone:** None directly. Cross-cutting verification infrastructure for the
  manifest CAS protocol RFC 0001 §3 already specifies and `crates/strand-core/src/
  manifest.rs` already implements; does not gate any of M1–M5.
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
