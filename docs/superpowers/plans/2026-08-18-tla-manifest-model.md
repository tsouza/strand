# TLA+ Model of the Manifest CAS Protocol — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `verification/manifest.tla`, a TLA+ model of the manifest CAS
commit and read protocols (RFC 0001 §3, `spec/manifest.md`), covering exactly
the action grammar RFC 0002 §4 approved, checked incrementally with TLC as
each piece lands.

**Architecture:** One TLA+ module. Writers and readers are each modeled as a
small local state machine (a `pc`-style phase variable plus per-process
working state) acting on one shared piece of global state: `snapshots`, a
sequence of committed snapshot records that only ever grows by `Append` —
mirroring the real protocol's immutable, monotonically-versioned commit
history. Every action corresponds one-to-one to an action in RFC 0002 §4's
grammar and to a specific function or branch in
`crates/strand-core/src/manifest.rs`. Each task adds one slice and ends with
a real TLC (or SANY) run against it — this is TLA+ authoring's equivalent of
red/green: define the piece, then verify with the tool that it behaves as
specified, including, where the deliverable is a safety invariant,
deliberately breaking the model to confirm TLC actually catches the break
(this project's established mutation-testing discipline, applied here).

**Tech Stack:** TLA+, checked with TLC (the bundled model checker in
`tla2tools.jar`) and SANY (the bundled parser, invoked standalone for a
parse-only check). No TLAPS proof and no DST harness in this plan — RFC 0002
§1 treats those as separate artifacts, and this plan is scoped to the first
of the three, per the explicit request to scope the TLA+ model on its own
before the others.

## Adversarial review record

This plan's modeling decisions (not just its TLC/SANY command syntax, which
was independently tool-verified from the start) went through a second
adversarial review before execution, since RFC 0002 §4 only sketches action
names and outcome sets — the concrete state representation, guard logic, and
invariant formalization below is new work the review actually checked. Five
concerns were raised going in; the review's findings and this plan's
response:

1. **`ReadCurrent(w)` and `ResolveAmbiguity(w)` were missing `DefiniteFailure`
   branches** that the real code has — `read_current()`'s `store.get()` calls
   and `ResolveAmbiguity`'s own follow-up `get` (`manifest.rs` lines ~178–182)
   both propagate `StoreError::Io` as `CommitError::Io`, exercised by
   `commit_surfaces_io_errors_from_the_initial_read_instead_of_panicking`.
   Only `TryAdvancePointer` could reach `"Failed"` in the first draft. This
   is exactly the "the approved grammar doesn't actually fit once you try to
   formalize it" condition this plan's Global Constraints already said to
   treat seriously — fixed below (both actions now have a `DefiniteFailure`
   branch to `"Failed"`).
2. **Reader failures were collapsed into one indistinguishable terminal
   state.** `ReadPointer`'s `DefiniteFailure`, `ReadSnapshotObject`'s
   retries-exhausted branch, and `ReadSnapshotObject`'s own
   `DefiniteFailure` branch all went to the same `"Failed"` state, even
   though RFC 0002 §4 names `RetriesExhausted` and `DefiniteFailure` as
   distinct outcomes and the real `ReadError` enum has two distinct
   variants. No property depending on *why* a reader failed was statable.
   Fixed below: split into `"Failed_RetriesExhausted"` and
   `"Failed_DefiniteFailure"`.
3. **`RowIdCounts` fixed at 1 for every writer masked an entire bug class** —
   any mutation that hardcodes `+1` instead of `+RowIdCounts[w]`, or
   `count |-> 1` instead of `count |-> RowIdCounts[w]`, is numerically
   identical to correct code and undetectable. Fixed below: a new
   `DistinguishedWriter` CONSTANT lets `RowIdCounts` vary (1 for the
   distinguished writer, 2 for every other), without needing the
   function-literal `.cfg` syntax already confirmed broken.
4. **`VersionsMatchIndex` and `MonotonicNextRowId` were never mutation-tested**
   the way `NoOverlappingRowIds` was. Confirmed during this review's
   fix-verification pass: removing `TryAdvancePointer`'s staleness guard
   entirely violates `VersionsMatchIndex` (a stale writer can then append a
   `.version` value that collides with what a rival already committed) —
   Task 3 now includes this as a second mutation test.
5. **A safety property the existing Rust proptest already checks
   (`next_row_id` equals the sum of every segment's `row_id_count`,
   `crates/strand-core/src/manifest.rs`'s `tests::property` module) was
   missing from the TLA+ invariant set** — meaning the model checked *less*
   than the cheaper alternative it's supposed to complement. Fixed below: a
   new `NextRowIdMatchesSegments` invariant, added in Task 3, mutation-tested
   by dropping `RowIdCounts[w]` from the `nextRowId` computation.

The review also confirmed four things did **not** need fixing:
`NoOverlappingRowIds` checking only the latest snapshot's segments is a
sound inductive argument, not a gap (every append only ever extends the true
immediate predecessor, so each entry's non-overlap is validated the moment
it was newest and never mutates afterward — now stated explicitly in the
model's comments, not left implicit); the 2-writer/1-reader model size is
defensible because every guard in this protocol is a single boolean
comparison with no quorum/threshold logic depending on rival *count* (unlike
Raft), and readers don't interact with each other at all, so a 3rd writer or
2nd reader adds interleaving volume, not new guard combinations; `Expired`
modeled as an unconditional environment fault matches the real code's own
worst-case test and is correctly flagged as needing a structural (not
parameter) rework once M3 lands; and `ResolveAmbiguity`'s landed-guard
correctly matches a real, accepted quirk in `manifest.rs` (a rival's commit
landing between an ambiguous write and its resolution forces "not landed,"
matching the real path-equality check, and any resulting duplicate-on-retry
is documented as the caller's idempotency responsibility, out of the
manifest protocol's own contract).

Every fix below was re-verified against the real `tla2tools.jar` before
being written into this plan: the corrected model checks clean (561 distinct
states, up from 466, since the new failure branches and varying row counts
both add reachable states), and all four mutation tests (two original, two
new) are confirmed to trigger the invariant they're supposed to.

## Global Constraints

- **RFC 0002 is Approved** (`rfcs/0002-manifest-formal-verification.md`,
  commit `01ca54f`). This plan implements its §4 action grammar exactly —
  writer path (`ReadCurrent`, `ProposeSnapshot`, `TryAdvancePointer` with
  outcomes `{Success, PreconditionFailed, DefiniteFailure, Ambiguous}`,
  `ResolveAmbiguity`) and reader path (`ReadPointer` with outcomes
  `{Found, Absent, DefiniteFailure}`, `ReadSnapshotObject` with outcomes
  `{Found, Expired, DefiniteFailure}`, `RefreshAndRetry`,
  `RetriesExhausted`). Do not invent a different grammar; if this plan's
  execution finds the approved grammar doesn't actually fit once you try to
  formalize it, stop and revise RFC 0002 through its own adversarial-review
  process (CLAUDE.md §3) rather than silently drifting from it here.
- **File location:** `verification/manifest.tla` — the path RFC 0002 §1
  already commits to. This plan also creates `verification/manifest.cfg`
  (the TLC configuration) and `verification/README.md` (how to run it).
  `verification/` is a new top-level directory; nothing else currently lives
  there.
- **Scope boundary:** this plan covers the CURRENT protocol surface only —
  no table metadata, retention policy, compaction, or orphan sweep (all M3,
  not yet implemented — RFC 0002 Non-goals and Open Questions both call this
  out explicitly, including that the reader-side `Expired` outcome exists in
  the model as an environment-injected fault, not as something derived from
  real deletion, since nothing in the current protocol ever deletes an
  object). Liveness properties (a writer retrying under bounded contention
  eventually commits) are explicitly OUT of this plan's scope — RFC 0002
  Open Questions lists liveness as needing its own fairness-assumption
  justification, a different and harder TLA+ skill (temporal properties,
  `WF_`/`SF_` fairness operators, more expensive TLC checking) from the
  safety-only model this plan builds. A follow-on plan should scope liveness
  once this one lands.
- **Tooling:** Java 17+ (confirmed present: OpenJDK 17.0.19) and
  `tla2tools.jar` (TLC 2.19, MIT-licensed, `tlaplus/tlaplus` on GitHub). A
  copy is already cached at `/home/thiago/.cache/tlaplus/tla2tools.jar` on
  this machine; Task 6 documents how to fetch it fresh
  (`https://github.com/tlaplus/tlaplus/releases/latest/download/tla2tools.jar`,
  confirmed resolving with a live `curl -IL` check during this plan's
  authoring). The jar is NOT vendored into the repository — it is a
  multi-megabyte third-party binary, unlike the small text excerpts
  `references/` holds elsewhere in this project.
- **Every TLC/SANY command in this plan has been run against the real local
  tooling during planning**, not written from memory, including after the
  adversarial-review fixes above: SANY parse-only invocation, `.cfg`
  `CONSTANTS`/`CHECK_DEADLOCK FALSE` syntax, TLC's exit codes (`0` on a
  clean pass, `12` on an invariant violation), and all four mutation tests
  were verified against the final, corrected model — not just an earlier
  draft of it.
- **Model size:** keep constants small enough that every TLC run in this
  plan finishes in well under a minute (2 writers, 1 reader, small row-ID
  counts, a reader retry limit of 2). Scaling the model up (more writers,
  larger bounds) is explicitly a follow-on, not a task here — the goal of
  this plan is a correct, TLC-checked model at a size that keeps the
  iteration loop fast, not a stress-tested one. The adversarial review
  above gives the specific reason this size is adequate for the safety
  properties this model checks, not just an unexamined default.

---

### Task 1: Module skeleton — constants, variables, `Init`, `TypeOK`

**Files:**
- Create: `verification/manifest.tla`

**Interfaces:**
- Produces: the `CONSTANTS` (`Writers`, `Readers`, `ReaderRetryLimit`,
  `DistinguishedWriter`, `NoProposalVal`, `NoResultVal`, `NoCommitsYetVal`),
  the `RowIdCounts` operator, and the `VARIABLES` (`snapshots`, `wPc`,
  `wLocal`, `rPc`, `rLocal`) every later task's actions and invariants
  reference by these exact names. Every `.cfg` from Task 2 onward must
  assign the three sentinel constants (`NoProposalVal = NoProposalVal`, and
  so on — a model value assigned to itself is TLA+'s standard idiom for "a
  unique symbol with no internal structure") and `DistinguishedWriter` (to
  one of `Writers`' members) even before a task's own constants are needed,
  since `TypeOK` already references the sentinels starting here and
  `RowIdCounts` already references `DistinguishedWriter`.

- [ ] **Step 1: Write the module skeleton**

```tla
---- MODULE manifest ----
(* TLA+ model of STRAND's manifest CAS commit and read protocols.         *)
(* Grounds: RFC 0001 Section 3 (rfcs/0001-container-rowid-manifest.md),   *)
(* spec/manifest.md, and RFC 0002's approved action grammar (Section 4,   *)
(* rfcs/0002-manifest-formal-verification.md). Scope: the CURRENT         *)
(* protocol surface only -- no table metadata, retention, compaction, or  *)
(* orphan sweep (all M3, not yet implemented; RFC 0002 Non-goals).        *)

EXTENDS Naturals, Sequences

CONSTANTS
    Writers,             \* finite, nonempty set of writer ids
    Readers,             \* finite set of reader ids (may be empty)
    ReaderRetryLimit,    \* mirrors manifest.rs's READER_REFRESH_RETRY_LIMIT
    DistinguishedWriter, \* one member of Writers; see RowIdCounts below
    NoProposalVal,       \* sentinel model value, see note below
    NoResultVal,         \* sentinel model value, see note below
    NoCommitsYetVal      \* sentinel model value, see note below

VARIABLES
    snapshots,  \* Seq(SnapshotRec); Len(snapshots) = current committed version count
    wPc,        \* [Writers -> {"Read","Propose","Advance","ResolveAmbiguity","Done","Failed"}]
    wLocal,     \* [Writers -> [baseVersion: Nat, nextRowId: Nat, proposed: ...]]
    rPc,        \* [Readers -> {"ReadPtr","ReadSnap","Done","Failed_RetriesExhausted","Failed_DefiniteFailure"}]
    rLocal      \* [Readers -> [retries: Nat, ptrVersion: Nat, result: ...]]

\* DistinguishedWriter claims 1 row per segment; every other writer claims 2.
\* Not a CONSTANT function: TLA+ .cfg files cannot express a function literal
\* (`:>`/`@@`) directly in a CONSTANTS block (confirmed against the real
\* tla2tools.jar while planning -- it fails with tlc2.tool.ConfigFileException).
\* Varying the count (as opposed to fixing it at 1 for everyone) matters: a
\* mutation that hardcodes +1 instead of +RowIdCounts[w] is numerically
\* identical to correct code when every writer's count is 1, and would be
\* undetectable -- confirmed by the second adversarial review (see above).
RowIdCounts == [w \in Writers |-> IF w = DistinguishedWriter THEN 1 ELSE 2]

SegmentRec == [base: Nat, count: Nat]

SnapshotRec == [version: Nat, nextRowId: Nat, segments: Seq(SegmentRec)]

\* Sentinel "not set yet" markers, declared as CONSTANTS (TLA+ model values,
\* assigned to themselves in the .cfg, e.g. `NoProposalVal = NoProposalVal`)
\* rather than plain strings like "none". Confirmed against the real
\* tla2tools.jar while planning: comparing a STRING sentinel to a RECORD
\* value (wLocal[w].proposed is sometimes a SnapshotRec, sometimes "not set
\* yet") makes TLC fail with "TLC was unable to fingerprint... Attempted to
\* check equality of record ... with non-record" -- a real TLC limitation on
\* mixed string/record equality, not a hypothetical one. Model values don't
\* have this problem.
NoProposal == NoProposalVal  \* wLocal[w].proposed before ProposeSnapshot has run
NoResult == NoResultVal      \* rLocal[r].result before a reader finishes

Init ==
    /\ snapshots = <<>>
    /\ wPc = [w \in Writers |-> "Read"]
    /\ wLocal = [w \in Writers |-> [baseVersion |-> 0, nextRowId |-> 0, proposed |-> NoProposal]]
    /\ rPc = [r \in Readers |-> "ReadPtr"]
    /\ rLocal = [r \in Readers |-> [retries |-> 0, ptrVersion |-> 0, result |-> NoResult]]

TypeOK ==
    /\ snapshots \in Seq(SnapshotRec)
    /\ wPc \in [Writers -> {"Read", "Propose", "Advance", "ResolveAmbiguity", "Done", "Failed"}]
    /\ \A w \in Writers :
        /\ wLocal[w].baseVersion \in Nat
        /\ wLocal[w].nextRowId \in Nat
        /\ wLocal[w].proposed = NoProposal \/ wLocal[w].proposed \in SnapshotRec
    /\ rPc \in [Readers -> {"ReadPtr", "ReadSnap", "Done", "Failed_RetriesExhausted", "Failed_DefiniteFailure"}]
    /\ \A r \in Readers :
        /\ rLocal[r].retries \in Nat
        /\ rLocal[r].ptrVersion \in Nat
        /\ rLocal[r].result = NoResult \/ rLocal[r].result = NoCommitsYetVal \/ rLocal[r].result \in SnapshotRec

====
```

- [ ] **Step 2: Verify it parses**

Run:
```
java -cp /home/thiago/.cache/tlaplus/tla2tools.jar tla2sany.SANY verification/manifest.tla
```
Expected: output ending in `Semantic processing of module manifest` with no
`***Parse Error***` or `***Semantic Error***` lines. (Verified during
planning against an equivalent scratch module — this exact invocation form
works against the local `tla2tools.jar`.) There is no TLC run yet in this
task: `Init`/`TypeOK` alone have no `Next` relation to check state
transitions against, so a parse-only check is the right — not a
placeholder — verification for this step.

- [ ] **Step 3: Commit**

```bash
git add verification/manifest.tla
git commit -m "verification: TLA+ manifest model skeleton (constants, variables, Init, TypeOK)"
```

---

### Task 2: Writer path actions and a first TLC run

**Files:**
- Modify: `verification/manifest.tla`
- Create: `verification/manifest.cfg`

**Interfaces:**
- Consumes: `Init`, `TypeOK`, all `CONSTANTS`/`VARIABLES` from Task 1.
- Produces: `ReadCurrent(w)`, `ProposeSnapshot(w)`, `TryAdvancePointer(w)`,
  `ResolveAmbiguity(w)`, and `Next`/`Spec`, which Task 3's invariants and
  Task 4's reader actions both extend.

- [ ] **Step 1: Add the four writer actions and `Next`/`Spec`**

Insert before the closing `====`:

```tla
\* RFC 0002 SS4 gives ReadCurrent no explicit outcome set, but the real
\* read_current() propagates a store.get() failure as CommitError::Io
\* (exercised by commit_surfaces_io_errors_from_the_initial_read_instead_of_panicking
\* in manifest.rs) -- so this action needs a DefiniteFailure branch too, found
\* missing by the second adversarial review (see above) and added here.
ReadCurrent(w) ==
    /\ wPc[w] = "Read"
    /\ \/ /\ LET nid == IF Len(snapshots) = 0 THEN 0 ELSE snapshots[Len(snapshots)].nextRowId IN
             wLocal' = [wLocal EXCEPT ![w] = [baseVersion |-> Len(snapshots), nextRowId |-> nid, proposed |-> NoProposal]]
          /\ wPc' = [wPc EXCEPT ![w] = "Propose"]
          /\ UNCHANGED <<snapshots, rPc, rLocal>>
       \/ /\ wPc' = [wPc EXCEPT ![w] = "Failed"]
          /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>

ProposeSnapshot(w) ==
    /\ wPc[w] = "Propose"
    /\ LET base == wLocal[w].baseVersion
           nid == wLocal[w].nextRowId
           priorSegs == IF base = 0 THEN <<>> ELSE snapshots[base].segments
           newSeg == [base |-> nid, count |-> RowIdCounts[w]]
           proposed == [version |-> base, nextRowId |-> nid + RowIdCounts[w], segments |-> Append(priorSegs, newSeg)]
       IN wLocal' = [wLocal EXCEPT ![w].proposed = proposed]
    /\ wPc' = [wPc EXCEPT ![w] = "Advance"]
    /\ UNCHANGED <<snapshots, rPc, rLocal>>

\* RFC 0002 SS4: outcome in {Success, PreconditionFailed, DefiniteFailure, Ambiguous}.
\* PreconditionFailed is forced (a stale CAS token always fails, never ambiguously
\* succeeds -- RFC 0002 SS3's drift table treats a definite service response, which
\* this is, as distinct from a genuinely ambiguous one). The other three outcomes
\* are a real writer's environment-injected choice, independent of staleness.
TryAdvancePointer(w) ==
    /\ wPc[w] = "Advance"
    /\ LET stale == Len(snapshots) # wLocal[w].baseVersion IN
       IF stale THEN
           /\ wPc' = [wPc EXCEPT ![w] = "Read"]
           /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>
       ELSE
           \/ /\ snapshots' = Append(snapshots, wLocal[w].proposed)
              /\ wPc' = [wPc EXCEPT ![w] = "Done"]
              /\ UNCHANGED <<wLocal, rPc, rLocal>>
           \/ /\ wPc' = [wPc EXCEPT ![w] = "Failed"]
              /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>
           \/ /\ wPc' = [wPc EXCEPT ![w] = "ResolveAmbiguity"]
              /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>

\* manifest.rs's real disambiguation: a follow-up read of the pointer resolves
\* the ambiguity completely, because the CAS is atomic server-side. "Landed" is
\* only still possible if no rival has committed since this writer's read. The
\* follow-up read can itself fail (manifest.rs lines ~178-182, also
\* CommitError::Io) -- the third branch below, missing from the first draft
\* per the second adversarial review (see above).
ResolveAmbiguity(w) ==
    /\ wPc[w] = "ResolveAmbiguity"
    /\ \/ /\ Len(snapshots) = wLocal[w].baseVersion
          /\ snapshots' = Append(snapshots, wLocal[w].proposed)
          /\ wPc' = [wPc EXCEPT ![w] = "Done"]
          /\ UNCHANGED <<wLocal, rPc, rLocal>>
       \/ /\ wPc' = [wPc EXCEPT ![w] = "Read"]
          /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>
       \/ /\ wPc' = [wPc EXCEPT ![w] = "Failed"]
          /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>

Next == \E w \in Writers : ReadCurrent(w) \/ ProposeSnapshot(w) \/ TryAdvancePointer(w) \/ ResolveAmbiguity(w)

Spec == Init /\ [][Next]_<<snapshots, wPc, wLocal, rPc, rLocal>>
```

- [ ] **Step 2: Write the TLC config**

```
CONSTANTS
    Writers = {w1, w2}
    Readers = {}
    ReaderRetryLimit = 2
    DistinguishedWriter = w1
    NoProposalVal = NoProposalVal
    NoResultVal = NoResultVal
    NoCommitsYetVal = NoCommitsYetVal
INIT Init
NEXT Next
INVARIANT TypeOK
CHECK_DEADLOCK FALSE
```

Save as `verification/manifest.cfg`. `CHECK_DEADLOCK FALSE` is required
because every writer reaching `"Done"` or `"Failed"` with nothing left
enabled is the CORRECT terminal state for a finite model, not a bug — TLC's
default deadlock check would otherwise flag that expected termination as an
error. (Verified during planning: `CHECK_DEADLOCK FALSE` is a real, working
`.cfg` directive against this exact `tla2tools.jar`, confirmed with a
scratch model.)

- [ ] **Step 3: Run TLC**

```
java -jar /home/thiago/.cache/tlaplus/tla2tools.jar -workers auto -config verification/manifest.cfg verification/manifest.tla
```

Expected: `Model checking completed. No error has been found.`, exit code
`0` — confirmed during planning against this exact model and config,
reconstructed directly from this plan's own text and run fresh (not carried
over from an earlier draft): 75 distinct states (writer-only, 2 writers,
`TypeOK` only). If `TypeOK` fails, the printed counterexample trace names
the exact state and action where a
field went out of its declared shape — fix the action definition, not the
invariant, and rerun.

- [ ] **Step 4: Commit**

```bash
git add verification/manifest.tla verification/manifest.cfg
git commit -m "verification: writer-path actions (ReadCurrent, ProposeSnapshot, TryAdvancePointer, ResolveAmbiguity), TypeOK checked by TLC"
```

---

### Task 3: Writer-side safety invariants, checked and mutation-tested

**Files:**
- Modify: `verification/manifest.tla`

**Interfaces:**
- Consumes: `snapshots` (Task 1), `Next`/`Spec` (Task 2).
- Produces: `NoOverlappingRowIds`, `MonotonicNextRowId`, `VersionsMatchIndex`,
  `NextRowIdMatchesSegments` — the first three are the safety properties RFC
  0002 Open Questions lists as the starting set; the fourth was added by the
  second adversarial review (see above) because the existing Rust proptest
  (`crates/strand-core/src/manifest.rs`'s `tests::property` module) already
  checks it and the TLA+ model shouldn't check less than the cheaper
  alternative it's meant to complement. Later plans (TLAPS proof) will prove
  all four, not just TLC-check them.

- [ ] **Step 1: Add the four invariants**

```tla
\* RFC 0002 Open Questions: "no two SegmentRefs across the full committed
\* segment set have overlapping row-ID ranges." Only the latest snapshot's
\* segment list needs checking, not a simplification that risks missing an
\* earlier overlap: ProposeSnapshot only ever extends the immediate
\* predecessor (Append(snapshots[base].segments, newSeg) where base is the
\* CURRENT length), so every earlier snapshot's segment list is a strict
\* prefix of every later one by construction, and TLC checks invariants at
\* every reachable state -- meaning each entry's own non-overlap was already
\* validated the moment it was newest, and nothing after that point can
\* mutate it. Confirmed sound, not just assumed, by the second adversarial
\* review (see above).
NoOverlappingRowIds ==
    snapshots = <<>> \/
    LET segs == snapshots[Len(snapshots)].segments IN
    \A i, j \in 1..Len(segs) :
        i # j =>
            \/ segs[i].base + segs[i].count <= segs[j].base
            \/ segs[j].base + segs[j].count <= segs[i].base

\* RFC 0002 Open Questions: "next_row_id [is] strictly monotonic across commits."
MonotonicNextRowId ==
    \A i, j \in 1..Len(snapshots) : i < j => snapshots[i].nextRowId <= snapshots[j].nextRowId

\* RFC 0002 Open Questions: "every committed snapshot is reachable by
\* following prior versions back to version 0" -- restated as: the sequence
\* position and the recorded version number never diverge. Not vacuous
\* despite holding by construction under the CORRECT action definitions: it
\* holds only because both append sites (TryAdvancePointer's Success branch,
\* ResolveAmbiguity's landed branch) gate on Len(snapshots) = wLocal[w].baseVersion
\* before appending a record whose own .version field equals that same
\* baseVersion. Break either guard and this invariant catches it -- confirmed
\* by mutation test, Step 3 below.
VersionsMatchIndex ==
    \A i \in 1..Len(snapshots) : snapshots[i].version = i - 1

\* Matches the existing Rust proptest's own invariant (manifest.rs's
\* tests::property module: `next_row_id` equals the summed `row_id_count`
\* across every committed segment) -- added by the second adversarial
\* review because the TLA+ model previously checked less than the cheaper
\* alternative it's supposed to complement.
RECURSIVE SumCounts(_)
SumCounts(segs) == IF segs = <<>> THEN 0 ELSE segs[1].count + SumCounts(Tail(segs))

NextRowIdMatchesSegments ==
    snapshots = <<>> \/
    snapshots[Len(snapshots)].nextRowId = SumCounts(snapshots[Len(snapshots)].segments)
```

- [ ] **Step 2: Add them to the config and run TLC**

Extend `verification/manifest.cfg`:

```
INVARIANT NoOverlappingRowIds
INVARIANT MonotonicNextRowId
INVARIANT VersionsMatchIndex
INVARIANT NextRowIdMatchesSegments
```

Run the same TLC command as Task 2, Step 3. Expected: `Model checking
completed. No error has been found.`, exit code `0` — confirmed during
planning: the same 75 states as Task 2 (adding invariants doesn't change
the reachable state space, only what gets checked against it), all four new
invariants holding across every one of them.

- [ ] **Step 3: Mutation-test `NoOverlappingRowIds`**

This project's established discipline (`crates/strand-core/src/manifest.rs`'s
test suite, `rfcs/0002-manifest-formal-verification.md`'s own citation of it)
is to confirm a check actually catches the bug it claims to, not just that it
passes. Temporarily break `ProposeSnapshot`'s segment construction to reuse
`base |-> 0` unconditionally instead of `nid` (reintroducing the historical
row-ID-overlap bug at the model level):

```tla
newSeg == [base |-> 0, count |-> RowIdCounts[w]]  \* MUTATION: ignores nid
```

Run TLC again. Expected: `Error: Invariant NoOverlappingRowIds is violated.`
with a printed counterexample trace, exit code `12`. Then revert the
mutation exactly back to `newSeg == [base |-> nid, count |-> RowIdCounts[w]]`
and rerun to confirm it passes clean again (exit code `0`) before moving on.

- [ ] **Step 4: Mutation-test `VersionsMatchIndex`**

Temporarily remove `TryAdvancePointer`'s staleness guard, so it always
attempts to advance regardless of whether a rival has already committed
(this exact mutation was verified during planning to trigger the failure
below, not assumed to):

```tla
TryAdvancePointer(w) ==
    /\ wPc[w] = "Advance"
    /\ \/ /\ snapshots' = Append(snapshots, wLocal[w].proposed)  \* MUTATION: no staleness guard
          /\ wPc' = [wPc EXCEPT ![w] = "Done"]
          /\ UNCHANGED <<wLocal, rPc, rLocal>>
       \/ /\ wPc' = [wPc EXCEPT ![w] = "Failed"]
          /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>
       \/ /\ wPc' = [wPc EXCEPT ![w] = "ResolveAmbiguity"]
          /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>
```

Run TLC again. Expected: `Error: Invariant VersionsMatchIndex is violated.`,
exit code `12`. Then revert exactly back to the guarded version from Step 1
above (with the `LET stale == ... IN IF stale THEN ... ELSE ...` structure)
and rerun to confirm a clean pass (exit code `0`) before moving on.

- [ ] **Step 5: Mutation-test `NextRowIdMatchesSegments`**

Temporarily drop `RowIdCounts[w]` from `ProposeSnapshot`'s `nextRowId`
computation:

```tla
proposed == [version |-> base, nextRowId |-> nid, segments |-> Append(priorSegs, newSeg)]  \* MUTATION
```

Run TLC again. Expected: `Error: Invariant NextRowIdMatchesSegments is
violated.`, exit code `12`. Revert exactly back to
`nextRowId |-> nid + RowIdCounts[w]` and rerun to confirm a clean pass (exit
code `0`) before moving on.

- [ ] **Step 6: Commit**

```bash
git add verification/manifest.tla verification/manifest.cfg
git commit -m "verification: writer-side safety invariants (no overlapping row-IDs, monotonic next_row_id, version/index agreement, next_row_id/segment-sum agreement), all four mutation-tested"
```

---

### Task 4: Reader path actions

**Files:**
- Modify: `verification/manifest.tla`, `verification/manifest.cfg`

**Interfaces:**
- Consumes: `snapshots`, `Next` (extended, not replaced), `ReaderRetryLimit`.
- Produces: `ReadPointer(r)`, `ReadSnapshotObject(r)`.

- [ ] **Step 1: Add the two reader actions and extend `Next`**

```tla
\* RFC 0002 SS4: outcome in {Found, Absent, DefiniteFailure}. Absent and Found
\* are mutually exclusive, guarded by Len(snapshots); DefiniteFailure is an
\* environment-injected possibility independent of either. Terminates at
\* "Failed_DefiniteFailure" (not a bare "Failed"): the real ReadError enum
\* has two distinct variants (Io, RetriesExhausted), and a first draft of
\* this model collapsed both into one indistinguishable terminal state --
\* found by the second adversarial review (see above) and fixed here and in
\* ReadSnapshotObject below.
ReadPointer(r) ==
    /\ rPc[r] = "ReadPtr"
    /\ \/ /\ Len(snapshots) = 0
          /\ rPc' = [rPc EXCEPT ![r] = "Done"]
          /\ rLocal' = [rLocal EXCEPT ![r].result = NoCommitsYetVal]
          /\ UNCHANGED <<snapshots, wPc, wLocal>>
       \/ /\ Len(snapshots) > 0
          /\ rLocal' = [rLocal EXCEPT ![r].ptrVersion = Len(snapshots)]
          /\ rPc' = [rPc EXCEPT ![r] = "ReadSnap"]
          /\ UNCHANGED <<snapshots, wPc, wLocal>>
       \/ /\ rPc' = [rPc EXCEPT ![r] = "Failed_DefiniteFailure"]
          /\ UNCHANGED <<snapshots, wPc, wLocal, rLocal>>

\* RFC 0002 SS4: outcome in {Found, Expired, DefiniteFailure}. Nothing in this
\* model ever deletes a snapshot (compaction is M3, out of scope -- RFC 0002
\* Non-goals), so Expired is modeled as a pure environment-injected fault, the
\* same way DefiniteFailure is -- never derived from real deletion state, per
\* RFC 0002's own explanation of why this action exists ahead of M3. Confirmed
\* by the second adversarial review as an acceptable stand-in for now, with
\* the explicit caveat that M3 will need a structural rework here (Expired
\* conditioned on real retention state), not a parameter tweak.
ReadSnapshotObject(r) ==
    /\ rPc[r] = "ReadSnap"
    /\ \/ /\ rPc' = [rPc EXCEPT ![r] = "Done"]
          /\ rLocal' = [rLocal EXCEPT ![r].result = snapshots[rLocal[r].ptrVersion]]
          /\ UNCHANGED <<snapshots, wPc, wLocal>>
       \/ /\ rLocal[r].retries < ReaderRetryLimit
          /\ rPc' = [rPc EXCEPT ![r] = "ReadPtr"]
          /\ rLocal' = [rLocal EXCEPT ![r].retries = @ + 1]
          /\ UNCHANGED <<snapshots, wPc, wLocal>>
       \/ /\ rLocal[r].retries >= ReaderRetryLimit
          /\ rPc' = [rPc EXCEPT ![r] = "Failed_RetriesExhausted"]
          /\ UNCHANGED <<snapshots, wPc, wLocal, rLocal>>
       \/ /\ rPc' = [rPc EXCEPT ![r] = "Failed_DefiniteFailure"]
          /\ UNCHANGED <<snapshots, wPc, wLocal, rLocal>>
```

Replace `Task 2`'s `Next` definition with:

```tla
Next ==
    \/ \E w \in Writers : ReadCurrent(w) \/ ProposeSnapshot(w) \/ TryAdvancePointer(w) \/ ResolveAmbiguity(w)
    \/ \E r \in Readers : ReadPointer(r) \/ ReadSnapshotObject(r)
```

(`Spec`'s text is unchanged — it already references `Next` by name.)

- [ ] **Step 2: Update the config to include a reader and run TLC**

Change `verification/manifest.cfg`'s `Readers = {}` to `Readers = {r1}`.
Run the same TLC command as before. Expected: `Model checking completed. No
error has been found.` (all invariants from Tasks 2–3 plus `TypeOK` still
hold with a reader present), exit code `0` — confirmed during planning: 561
distinct states once the reader's actions are added to `Next` (state
*count* is exact and reproducible across reruns; TLC's separately-reported
"search depth" turned out, while verifying this plan, to vary with
definition order in the file even for a semantically identical model, so
this plan doesn't assert a specific depth number anywhere).

- [ ] **Step 3: Commit**

```bash
git add verification/manifest.tla verification/manifest.cfg
git commit -m "verification: reader-path actions (ReadPointer, ReadSnapshotObject), one reader added to the checked model"
```

---

### Task 5: Reader-side safety invariant, checked and mutation-tested

**Files:**
- Modify: `verification/manifest.tla`, `verification/manifest.cfg`

**Interfaces:**
- Consumes: `snapshots`, `rPc`, `rLocal` (Tasks 1, 4).
- Produces: `ReaderSeesOnlyCommitted`.

- [ ] **Step 1: Add the invariant**

```tla
\* A reader that finishes with an actual result never reports a snapshot
\* that isn't really in the committed history -- ties the reader model to
\* writer-side ground truth, the property a reader-safety bug would break.
ReaderSeesOnlyCommitted ==
    \A r \in Readers :
        (rPc[r] = "Done" /\ rLocal[r].result # NoCommitsYetVal) =>
            \E i \in 1..Len(snapshots) : snapshots[i] = rLocal[r].result
```

- [ ] **Step 2: Add it to the config and run TLC**

Extend `verification/manifest.cfg`:

```
INVARIANT ReaderSeesOnlyCommitted
```

Run the same TLC command as before. Expected: `Model checking completed. No
error has been found.`, exit code `0` — confirmed during planning: the same
561 states as Task 4, `ReaderSeesOnlyCommitted` holding across every one.

- [ ] **Step 3: Mutation-test it**

Temporarily change `ReadSnapshotObject`'s `Found` branch to report a
fabricated result instead of the real one:

```tla
/\ rLocal' = [rLocal EXCEPT ![r].result = [version |-> 999, nextRowId |-> 0, segments |-> <<>>]]  \* MUTATION
```

Run TLC again. Expected: `Error: Invariant ReaderSeesOnlyCommitted is
violated.`, exit code `12`. Revert the mutation back to
`snapshots[rLocal[r].ptrVersion]` and rerun to confirm a clean pass (exit
code `0`) before moving on.

- [ ] **Step 4: Commit**

```bash
git add verification/manifest.tla verification/manifest.cfg
git commit -m "verification: reader-side safety invariant (a reader only ever reports an actually-committed snapshot), mutation-tested"
```

---

### Task 6: `verification/README.md` and a final full-model run

**Files:**
- Create: `verification/README.md`

**Interfaces:**
- Consumes: nothing new — this task documents Tasks 1–5's finished artifact.
- Produces: nothing later tasks in a follow-on plan (TLAPS, DST) consume by
  name, but it is the reference future sessions read before touching
  `verification/` again, matching this project's documentation culture
  (every subsystem gets a "how to run this" note).

- [ ] **Step 1: Write the README**

```markdown
# verification/

A TLA+ model of the manifest CAS commit and read protocols (RFC 0001 §3,
`spec/manifest.md`), covering the action grammar RFC 0002 §4 approved.
Approved by RFC 0002 (`rfcs/0002-manifest-formal-verification.md`); this
directory is the first of that RFC's three artifacts (TLA+ model, TLAPS
proof, DST harness) — only the model and its TLC-checked safety invariants
exist so far. No TLAPS proof and no DST cross-validation harness yet.

## Running it

Requires Java 17+ and `tla2tools.jar` (MIT-licensed,
`github.com/tlaplus/tlaplus`). Fetch it if you don't already have a copy:

    curl -LO https://github.com/tlaplus/tlaplus/releases/latest/download/tla2tools.jar

Then, from the repository root:

    java -jar /path/to/tla2tools.jar -workers auto -config verification/manifest.cfg verification/manifest.tla

Expect `Model checking completed. No error has been found.` and exit code
`0`. A parse-only check (no model checking, just confirms the module is
well-formed) is also available:

    java -cp /path/to/tla2tools.jar tla2sany.SANY verification/manifest.tla

## Scope

Models the CURRENT protocol surface only: no table metadata, retention
policy, compaction, or orphan sweep (all M3, not yet implemented). The
reader-side `Expired` outcome exists in the model as an environment-injected
fault, not derived from real deletion — nothing in the current protocol
deletes an object yet, matching RFC 0002's own explanation for why this
action is modeled ahead of M3; this will need a structural rework, not a
parameter tweak, once M3 lands. Liveness (a writer retrying under bounded
contention eventually commits) is explicitly out of scope — a follow-on plan
covers it; see RFC 0002's Open Questions.

## Model size

`verification/manifest.cfg` uses a small, fast-checking configuration (2
writers, 1 reader, `DistinguishedWriter` claiming 1 row per segment and the
other writer claiming 2). This is deliberate, not an unexamined default: an
adversarial review of this model's design confirmed every guard in this
protocol is a single boolean comparison with no quorum/threshold logic
depending on rival *count* (unlike Raft), and readers never interact with
each other, so a 3rd writer or 2nd reader adds interleaving volume, not new
guard combinations to check. Scaling the model up regardless is a
deliberate follow-on, not done here.
```

- [ ] **Step 2: Run the full model one more time end to end**

```
java -jar /home/thiago/.cache/tlaplus/tla2tools.jar -workers auto -config verification/manifest.cfg verification/manifest.tla
```

Expected: `Model checking completed. No error has been found.`, all six
invariants (`TypeOK`, `NoOverlappingRowIds`, `MonotonicNextRowId`,
`VersionsMatchIndex`, `NextRowIdMatchesSegments`, `ReaderSeesOnlyCommitted`)
checked, exit code `0`. This exact model (2 writers, 1 reader, the config
above) was run against the real tooling while writing this plan — verified
directly against the model as reconstructed from this plan's own text, not
carried over from an earlier draft — and found 561 distinct states, all
invariants holding, in well under a second, confirmed stable across repeated
reruns of the identical file. TLC's separately-reported "search depth"
statistic turned out to vary with definition order even for a semantically
identical model (discovered while cross-checking two differently-ordered
versions of this same model during planning), so it is not asserted here as
a reproducible number — the state count is. Note the actual reported count
in the commit message (Step 3) regardless — it is useful context for anyone
later deciding whether to scale the model
up.

- [ ] **Step 3: Commit**

```bash
git add verification/README.md
git commit -m "verification: document how to run the TLA+ model (verification/README.md)"
```

---

## Self-review notes

**Spec coverage:** every action in RFC 0002 §4's grammar (writer:
`ReadCurrent`, `ProposeSnapshot`, `TryAdvancePointer` with all four
outcomes, `ResolveAmbiguity`; reader: `ReadPointer`, `ReadSnapshotObject`
with all three/three outcomes, the retry-vs-exhausted split) has a task, and
every action's outcome set matches what the real `manifest.rs` code
actually does, including the `DefiniteFailure` paths a first draft of this
plan missed on `ReadCurrent` and `ResolveAmbiguity` (fixed per the
adversarial review record above). Every safety property RFC 0002's Open
Questions names as the starting set (no overlapping row-IDs, monotonic
`next_row_id`, reachability from genesis) has an invariant, plus one more
(`NextRowIdMatchesSegments`) the review found was missing relative to the
existing Rust proptest; liveness is explicitly and deliberately excluded,
not forgotten (Global Constraints, above).

**Not in this plan, by design:** TLAPS proof of these invariants (RFC 0002
§1 commits to one; it is a separate, later plan), the DST harness and trace
vocabulary that would let the real Rust code be checked against this model
(RFC 0002 §1/§2, also later), and scaling the model's constants beyond a
fast-iterating starting size — the review confirmed 2 writers/1 reader is
adequate for this protocol's actual guard structure, not merely convenient.

**Known risk this plan does not resolve:** RFC 0002's second adversarial
review (of the RFC itself) flagged that the model's own action grammar is a
human enumeration in the same way a property-test generator's fault
vocabulary is — TLC's exhaustive check is only exhaustive over what §4
actually specifies. This plan builds exactly the approved grammar, now
verified against the real code's actual failure paths by a second,
independent adversarial review of the modeling decisions themselves (see
above) — closing the specific gaps that review found, not a general
guarantee no gap remains.
