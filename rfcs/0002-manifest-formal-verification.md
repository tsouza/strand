# RFC 0002: Dual-model verification of the manifest CAS protocol

- **Status:** Not approved. Adversarial review (below) found the proposal
  disproportionate to the protocol's actual size and a load-bearing citation
  (AWS PObserve, §2) backwards relative to what it was cited for. Per the
  review's own recommendation, this RFC is superseded for now by building the
  property-based testing alternative named in "Alternatives considered" and
  determining empirically whether it leaves a real gap only TLA+ + DST would
  close. Do not begin building the TLA+ spec or the DST harness against this
  RFC unless that evidence emerges and a revised version of this RFC passes
  its own adversarial review.

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
- **Milestone:** None directly. Cross-cutting verification infrastructure for the
  manifest CAS protocol RFC 0001 §3 already specifies and `crates/strand-core/src/
  manifest.rs` already implements; does not gate any of M1–M5.
- **Invariants exercised:** none changed. This RFC proposes no change to `CLAUDE.md`'s
  invariants, RFC 0001's protocol, or any wire format — it proposes a method for
  gaining confidence that the existing, already-approved protocol and its existing,
  already-implemented Rust code agree with each other.

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
human-authored test coverage genuinely hard to trust.

## Design

### 1. Architecture

Two independently-checkable artifacts, connected by a shared trace vocabulary:

- **The TLA+ model** (`verification/manifest.tla`, new): the commit and read
  protocols, specified at **Delta Lake's action granularity, not Raft's** — a
  deliberate choice grounded in Jack Vanlightly's public Delta Lake TLA+ model, whose
  actions (`StartOperation`, `ReadDataFiles`, `WriteDataFiles`, `TryCommitTxn`) match
  the shape of RFC 0001 §3's steps far more closely than Raft's fine-grained
  message-passing actions do. §4 below sketches the action grammar this implies.
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

Following FMDSE's (arXiv:2501.08550) own bidirectional framing, there are two
directions this dual-validation can run, and they are not equally hard:

- **Workflow II — spec drives implementation.** TLC (or a similar model-space walker)
  generates a large set of valid action sequences from the TLA+ spec. The DST harness
  replays each sequence directly against the real Rust code — this action, then this
  one, then this one, with the specified fault injected at the specified point — and
  checks the real code's outcome matches what the spec predicted. This is the shape
  AWS's PObserve/P-language tooling uses in production, and it is the direction that
  actually works in practice: the spec's action boundaries are known in advance and
  drive the harness, so there is no discovery problem about where one action ends and
  the next begins.
- **Workflow I — implementation drives verification.** The DST harness runs the real
  code under its own exploration (concurrent writers, randomized fault injection,
  many seeds), emits traces from what actually happened, and those traces are checked
  for conformance against the TLA+ spec after the fact. This is the harder direction:
  it requires the real code's emitted trace granularity to already agree with the
  spec's action granularity, discovered rather than driven — precisely the failure
  mode in MongoDB's real, documented case (a leader step-down/step-up modeled as one
  atomic spec action against an implementation that actually took two separate steps,
  a mismatch that invalidated every trace crossing that boundary, deterministically,
  not probabilistically).

This RFC proposes building Workflow II first, using it to establish that the trace
vocabulary and the spec's action granularity actually correspond to the real code's
behavior, and only then attempting Workflow I — at which point granularity agreement
is a design property already established, not a hope.

### 3. Drift classification

Per the earlier discussion this RFC formalizes: drift between the two sides is a
diagnostic signal, never a target to tune away directly. Every disagreement is
classified before anything is changed:

| drift type | meaning | fix |
| --- | --- | --- |
| Type-I | the Rust code did something the spec forbids | a real bug — fix the Rust code |
| Type-II | the spec permits something the Rust code can't or doesn't do | fix whichever side is not authoritative for that behavior — tighten the spec, or extend the Rust code |
| tracer artifact | neither side is wrong; the trace vocabulary's abstraction boundary is | fix the tracer, not the model or the code |

DST's determinism is what makes classification tractable: a drift instance is a seed,
and a seed is replayable, so a specific disagreement can be bisected to its exact
point of divergence at zero flake cost, the same way a real network's nondeterminism
never allows.

### 4. A sketch of the action grammar

Not the spec itself — a sketch showing the granularity this RFC commits to, so the
adversarial review has something concrete to push on before real TLA+ is written:

```
ReadCurrent(w)              \* writer w reads _strand/current + the snapshot it names
ProposeSnapshot(w, v)       \* writer w writes a new snapshot object for version v
TryAdvancePointer(w, v)     \* writer w attempts the pointer CAS; outcome ∈
                             \*   {Success, PreconditionFailed, DefiniteFailure, Ambiguous}
ResolveAmbiguity(w, v)      \* on Ambiguous: writer w re-reads the pointer to
                             \*   determine whether its own write landed
```

Each is one coarse, per-attempt action — matching `commit()`'s actual structure
(`crates/strand-core/src/manifest.rs`) directly, not a decomposition into the
individual HTTP requests each one issues. `ResolveAmbiguity` exists as its own action
specifically because of the fix RFC 0001's implementation just landed
(`store.rs`'s `StoreError::Ambiguous`, `manifest.rs`'s pointer-CAS disambiguation) —
this RFC's model must cover it, not the two-outcome CAS RFC 0001 originally described.

### 5. Effort, honestly

FMDSE's own reported cost for a comparably-scoped protocol: 675 lines of TLA+, 1,282
lines of TLAPS proof, and a roughly 2,000-line custom simulator, for a team already
expert in the technique — weeks, not days. `spacejam/tla-rust` attempted close to
this architecture and is dormant. This RFC does not treat that precedent as a reason
not to attempt this, but names it as the actual practical risk (§ "How this could be
wrong" below), not the granularity-matching question, which is a design discipline
this RFC's sequencing (§2) already addresses by construction.

## How this could be wrong

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

**This may be disproportionate to the actual risk.** The manifest protocol is
single-writer-per-attempt with bounded retry, not a distributed consensus protocol —
its state space is genuinely smaller than Raft's or Delta Lake's own multi-table
transaction model. A lighter-weight alternative (property-based testing, §"Alternatives
considered") might close most of the same gap at a fraction of the cost. This RFC's
adversarial review should weigh this directly: is the manifest protocol's actual
complexity large enough to justify TLA+ + DST, or would proptest-style randomized
interleaving plus invariant checks (no two committed segments overlap; `next_row_id`
is monotonic; every committed snapshot is reachable from the one before it) catch
the same class of bug at a fraction of the engineering cost this RFC's own §5 states
plainly?

## Alternatives considered

**Property-based testing (proptest/quickcheck-style) instead of TLA+ + DST.**
Not rejected — named here as the live alternative the adversarial review must weigh
against this RFC's proposal, per the risk above. Randomized generation of writer
interleavings and fault injection, checked against hand-stated invariants, is far
cheaper to build and maintain than a TLA+ model plus a proof plus a dual-tracing
harness, and the three bugs already found in this protocol during M0 were each
caught by targeted mutation tests, not anything TLA+-shaped. What it does not give:
TLC's exhaustive (within a bounded model) exploration of the *specified* protocol
independent of any particular Rust implementation, and a machine-checked proof of the
safety properties rather than a statistical increase in confidence from random
sampling.

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
- Whether this effort proceeds at all, given the proportionality question raised
  above, is itself the first thing the adversarial review should settle — not an
  open question to defer past approval.
