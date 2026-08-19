---- MODULE manifest ----
(*                                                                         *)
(* Copyright the STRAND authors.                                          *)
(*                                                                         *)
(* Licensed under the Apache License, Version 2.0 (the "License");        *)
(* you may not use this file except in compliance with the License.       *)
(* You may obtain a copy of the License at                                *)
(*                                                                         *)
(*     http://www.apache.org/licenses/LICENSE-2.0                        *)
(*                                                                         *)
(* Unless required by applicable law or agreed to in writing, software    *)
(* distributed under the License is distributed on an "AS IS" BASIS,      *)
(* WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or        *)
(* implied. See the License for the specific language governing           *)
(* permissions and limitations under the License.                        *)
(*                                                                         *)
(* TLA+ model of STRAND's manifest CAS commit and read protocols.         *)
(* Grounds: RFC 0001 Section 3 (rfcs/0001-container-rowid-manifest.md),   *)
(* spec/manifest.md, and RFC 0002's approved action grammar (Section 4,   *)
(* rfcs/0002-manifest-formal-verification.md). Scope: the CURRENT         *)
(* protocol surface only -- no table metadata, retention, compaction, or  *)
(* orphan sweep (all M3, not yet implemented; RFC 0002 Non-goals).        *)
(*                                                                         *)
(* Extended for RFC 0012 (rfcs/0012-deletion-vectors.md,                  *)
(* spec/deletion.md SS4): commit_deletion_vector's revise-in-place commit *)
(* shape -- ProposeSnapshot's Append-only transition was the one gap RFC  *)
(* 0012's own adversarial review found unmodeled (docs/ledger.md). See    *)
(* ProposeDeletionVectorCommit below.                                     *)

EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS
    Writers,             \* finite, nonempty set of writer ids
    Readers,             \* finite set of reader ids (may be empty)
    \* Mirrors the SHAPE of manifest.rs's READER_REFRESH_RETRY_LIMIT -- a
    \* bounded retry count after which the reader gives up -- not its value.
    \* The .cfg sets it to 2; the real Rust constant is 5. The smaller stand-in
    \* keeps the model fast to check, and nothing here depends on the exact
    \* bound, only on its being finite.
    ReaderRetryLimit,
    DistinguishedWriter, \* one member of Writers; see RowIdCounts below
    \* One member of Writers whose commits always take the
    \* commit_deletion_vector shape (ProposeDeletionVectorCommit) instead of
    \* the append shape (ProposeSnapshot) -- the same established pattern
    \* DistinguishedWriter already uses for varying one writer's shape
    \* without a combinatorial per-writer CONSTANTS explosion. Every other
    \* writer, including DistinguishedWriter itself, keeps the append shape.
    DeleteWriter,
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

\* delVer is a bare generation counter standing in for a segment's
\* DeletionVectorRef (spec/deletion.md SS3): 0 means no deletion vector
\* committed yet; incrementing it models commit_deletion_vector's
\* "supersede, don't accumulate" write (spec/deletion.md SS4). The model has
\* no reason to represent actual Roaring-bitmap content -- none of this
\* protocol's safety properties depend on WHICH rows are tombstoned, only on
\* whether a revise-in-place commit can safely interleave, through the
\* shared pointer CAS, with the append-shaped commits every other writer
\* still performs.
SegmentRec == [base: Nat, count: Nat, delVer: Nat]

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
NoProposal == NoProposalVal    \* wLocal[w].proposed before ProposeSnapshot has run
NoResult == NoResultVal        \* rLocal[r].result before a reader finishes
NoCommitsYet == NoCommitsYetVal \* rLocal[r].result when the pointer does not exist

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
        /\ rLocal[r].result = NoResult \/ rLocal[r].result = NoCommitsYet \/ rLocal[r].result \in SnapshotRec

\* RFC 0002 SS4 gives ReadCurrent no explicit outcome set, but the real
\* read_current() propagates a store.get() failure as CommitError::Io
\* (exercised by commit_surfaces_io_errors_from_the_initial_read_instead_of_panicking
\* in manifest.rs) -- so this action needs a DefiniteFailure branch too, found
\* missing by the second adversarial review (see above) and added here.
\*
\* A second, later-found gap in the same spot (docs/ledger.md, "TLA+ model
\* correspondence gap"): the real read_current() also loops unboundedly when
\* try_read_current() returns ReadAttempt::Expired (the pointer named a
\* snapshot object compaction already removed) -- unlike the reader path,
\* whose analogous Expired case is bounded by ReaderRetryLimit. The third
\* branch below models that: the writer's own bound is the pointer CAS it is
\* about to contend on (manifest.rs's doc comment says so explicitly), not
\* this read, so an Expired read here is just a self-transition back to
\* "Read" -- everything unchanged, since nothing was actually learned. Adding
\* a branch that changes nothing is a legitimate TLA+ action (a self-loop
\* edge in the state graph, not a stutter step); it cannot introduce a new
\* reachable state or falsify any invariant here, since every invariant this
\* module checks reads only committed `snapshots` or a writer/reader that has
\* actually reached "Done" -- neither is true of a writer sitting in "Read".
ReadCurrent(w) ==
    /\ wPc[w] = "Read"
    /\ \/ /\ LET nid == IF Len(snapshots) = 0 THEN 0 ELSE snapshots[Len(snapshots)].nextRowId IN
             wLocal' = [wLocal EXCEPT ![w] = [baseVersion |-> Len(snapshots), nextRowId |-> nid, proposed |-> NoProposal]]
          /\ wPc' = [wPc EXCEPT ![w] = "Propose"]
          /\ UNCHANGED <<snapshots, rPc, rLocal>>
       \/ /\ wPc' = [wPc EXCEPT ![w] = "Failed"]
          /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>
       \/ /\ wPc' = [wPc EXCEPT ![w] = "Read"]
          /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>

\* RFC 0002 SS4's action grammar lists no outcome set for ProposeSnapshot -- the
\* same class of gap ReadCurrent had (docs/ledger.md, "TLA+ model correspondence
\* gap"). The real put_if_absent in commit() (manifest.rs:131-139) can return
\* StoreError::Io or StoreError::Ambiguous here, and BOTH map to the same
\* CommitError::Io outcome -- unlike the pointer CAS below, this write's path is
\* attempt-unique (the writer_nonce), so an ambiguous outcome needs no
\* disambiguation: whether it landed or not, nothing will ever reference it under
\* a wrong assumption, and a landed-but-unacked write just leaves a harmless
\* orphan (CLAUDE.md SS6) once this attempt is abandoned. The second branch below
\* is that single collapsed failure outcome, not two separate ones.
ProposeSnapshot(w) ==
    /\ wPc[w] = "Propose"
    /\ w # DeleteWriter
    /\ LET base == wLocal[w].baseVersion
           nid == wLocal[w].nextRowId
           priorSegs == IF base = 0 THEN <<>> ELSE snapshots[base].segments
           newSeg == [base |-> nid, count |-> RowIdCounts[w], delVer |-> 0]
           proposed == [version |-> base, nextRowId |-> nid + RowIdCounts[w], segments |-> Append(priorSegs, newSeg)]
       IN \/ /\ wLocal' = [wLocal EXCEPT ![w].proposed = proposed]
             /\ wPc' = [wPc EXCEPT ![w] = "Advance"]
             /\ UNCHANGED <<snapshots, rPc, rLocal>>
          \/ /\ wPc' = [wPc EXCEPT ![w] = "Failed"]
             /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>

\* commit_deletion_vector's revise-in-place commit shape (RFC 0012,
\* spec/deletion.md SS4): unlike ProposeSnapshot, this NEVER appends a new
\* segment -- it revises one EXISTING segment's deletion-vector reference
\* (here: increments its delVer, SegmentRec's note above), leaving segment
\* count and nextRowId untouched. DeleteWriter always targets the first
\* segment (index 1) of whatever it currently sees; which segment is
\* targeted is irrelevant to the properties this model checks (see
\* DeletionVectorCommitsOnlyReviseOneEntry below) -- what matters is that
\* exactly one entry changes and nothing else does.
\*
\* Requires a segment to exist (Len(priorSegs) >= 1): if this writer's
\* snapshot read finds no segments yet, the action is simply not enabled --
\* the model's stand-in for the real commit_deletion_vector's
\* CommitError::SegmentNotFound, a caller error the retry loop does not
\* absorb (spec/deletion.md SS4), not a race outcome to model as a
\* transition.
ProposeDeletionVectorCommit(w) ==
    /\ wPc[w] = "Propose"
    /\ w = DeleteWriter
    /\ LET base == wLocal[w].baseVersion
           nid == wLocal[w].nextRowId
           priorSegs == IF base = 0 THEN <<>> ELSE snapshots[base].segments
       IN /\ Len(priorSegs) >= 1
          /\ LET revisedSegs == [priorSegs EXCEPT ![1].delVer = @ + 1]
                 proposed == [version |-> base, nextRowId |-> nid, segments |-> revisedSegs]
             IN \/ /\ wLocal' = [wLocal EXCEPT ![w].proposed = proposed]
                   /\ wPc' = [wPc EXCEPT ![w] = "Advance"]
                   /\ UNCHANGED <<snapshots, rPc, rLocal>>
                \/ /\ wPc' = [wPc EXCEPT ![w] = "Failed"]
                   /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>

\* RFC 0002 SS4: outcome in {Success, PreconditionFailed, DefiniteFailure, Ambiguous}.
\* A stale CAS token always fails, never ambiguously succeeds -- RFC 0002 SS3's
\* drift table treats a definite service response, which this is, as distinct
\* from a genuinely ambiguous one.
\*
\* Deliberate narrowing, stated plainly because the code below does not say it:
\* the other three outcomes are modeled ONLY when the CAS is not stale. A stale
\* attempt is modeled as forcing PreconditionFailed, even though the real store
\* could also return Io or Ambiguous on a stale attempt -- the backend can fail,
\* or the ack can be lost, whatever the etag's freshness. This narrowing is
\* safety-neutral for the invariants this model checks, worked through rather
\* than assumed: a stale+Io path only reaches a "Failed" state, which no
\* invariant here observes, and a stale+Ambiguous path would resolve to "not
\* landed" (ResolveAmbiguity's landed branch needs Len(snapshots) = baseVersion,
\* which staleness denies) and loop back to "Read" -- the same successor the
\* plain stale branch already produces. Widening the outcome space here would
\* add states, not reachable invariant violations.
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
\* the ambiguity completely, because the CAS is atomic server-side. The
\* follow-up read can itself fail (manifest.rs lines ~178-182, also
\* CommitError::Io) -- the third branch below, missing from the first draft
\* per the second adversarial review (see above).
\*
\* Deliberate abstraction, stated plainly because the shape below invites the
\* opposite reading: this action models the OUTCOME of the disambiguation
\* (landed or not landed), not the read-only recheck the real function
\* performs. The real StoreError::Ambiguous means the CAS may already have
\* landed on the store, and commit() then merely observes which; here the
\* Append is deferred into the "landed" branch, so the model's
\* ResolveAmbiguity has a write side effect its real counterpart does not, and
\* it decides "landed" by asking whether anyone else has committed since this
\* writer's read rather than whether the committed thing is this writer's own.
\* One real scenario is therefore unreachable in the model: this writer's write
\* actually lands, a rival builds on it, and this writer then retries and
\* commits a duplicate.
\*
\* The reason is state-space finiteness, confirmed empirically rather than
\* assumed. Modeling this more literally -- append nondeterministically at the
\* ambiguous CAS, then resolve by reading -- makes the state space unbounded
\* (>4M states and still growing when the whole-branch review measured it),
\* because a writer can append, fail to observe it, and append again without
\* bound. Bounding it properly (a CONSTRAINT on Len(snapshots), or a per-writer
\* attempt cap) is follow-on work for a later plan, not this model.
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
          /\ rLocal' = [rLocal EXCEPT ![r].result = NoCommitsYet]
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

Next ==
    \/ \E w \in Writers : ReadCurrent(w) \/ ProposeSnapshot(w) \/ ProposeDeletionVectorCommit(w)
                          \/ TryAdvancePointer(w) \/ ResolveAmbiguity(w)
    \/ \E r \in Readers : ReadPointer(r) \/ ReadSnapshotObject(r)

\* Not referenced by manifest.cfg, which drives INIT/NEXT directly -- and so
\* not dead code by accident but by design. It is here as groundwork for the
\* follow-on liveness plan (RFC 0002 Open Questions): a temporal property needs
\* a full Spec with its fairness conditions, not just INIT/NEXT, and writing
\* the Spec formula now keeps the module honest about what it is a spec OF.
Spec == Init /\ [][Next]_<<snapshots, wPc, wLocal, rPc, rLocal>>

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
\* Checked here as <= rather than <, deliberately weaker than that phrasing: a
\* commit that adds zero rows is legal in the real protocol, and <= is the
\* property that survives it. In this model every writer's RowIdCounts is >= 1,
\* so < would hold too -- the weaker form is chosen for the real protocol's
\* sake, not because the model needs it.
\*
\* Not redundant against NextRowIdMatchesSegments/VersionsMatchIndex, which is
\* worth recording because it looks like it should be: those two are checks on
\* a single snapshot (the latest), while this one is the only cross-snapshot
\* ordering check. A mutation that makes each snapshot individually consistent
\* but the sequence disordered is caught here and nowhere else. Confirmed by
\* mutation test: replacing ProposeSnapshot's `proposed` with one built from
\* scratch rather than from the snapshot the writer read --
\*   proposed == [version |-> base, nextRowId |-> RowIdCounts[w],
\*                segments |-> <<[base |-> 0, count |-> RowIdCounts[w]]>>]
\* -- leaves every snapshot self-consistent (one segment, next_row_id equal to
\* its count, version equal to its index) and so passes all six other
\* invariants clean at the full 561 states, while this invariant catches it:
\* w2 (2 rows) committing before w1 (1 row) yields next_row_id 2 then 1.
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

\* RFC 0012 (spec/deletion.md SS4): a commit_deletion_vector commit revises
\* an existing segment in place -- it must never remove a segment, and (by
\* construction, since ProposeDeletionVectorCommit never calls Append) it
\* never adds one either, but this invariant checks the removal half
\* directly rather than trusting the construction: segment count is
\* monotonic non-decreasing across the whole committed history, exactly the
\* same style of cross-snapshot ordering check MonotonicNextRowId already
\* is for row-ID allocation. Before this action existed, every commit grew
\* the segment count by exactly 1, so this held trivially; now that
\* revise-in-place commits exist (which must leave the count unchanged),
\* it is a real, load-bearing check, not a restatement of the old one.
SegmentCountNeverDecreases ==
    \A i, j \in 1..Len(snapshots) : i < j => Len(snapshots[i].segments) <= Len(snapshots[j].segments)

\* commit_deletion_vector's core promise, checked directly rather than only
\* trusted by construction: between two consecutive committed snapshots
\* whose segment COUNT is unchanged -- which, given SegmentCountNeverDecreases
\* and that ProposeSnapshot always grows the count by exactly one segment,
\* can only be a ProposeDeletionVectorCommit commit -- every segment's base
\* and count fields are unchanged (nothing was resized or reassigned), and
\* at most one segment's delVer differs (exactly one entry was revised, not
\* zero and not several). This is the model-level encoding of "revise
\* exactly one entry, nothing else" (spec/deletion.md SS4,
\* "Superseding, not accumulating").
DeletionVectorCommitsOnlyReviseOneEntry ==
    \A i \in 1..Len(snapshots) - 1 :
        LET segsA == snapshots[i].segments
            segsB == snapshots[i + 1].segments
        IN Len(segsA) = Len(segsB) =>
            /\ \A k \in 1..Len(segsA) : segsA[k].base = segsB[k].base /\ segsA[k].count = segsB[k].count
            /\ Cardinality({k \in 1..Len(segsA) : segsA[k].delVer # segsB[k].delVer}) <= 1

\* A reader that finishes with an actual result never reports a snapshot
\* that isn't really in the committed history -- ties the reader model to
\* writer-side ground truth, the property a reader-safety bug would break.
ReaderSeesOnlyCommitted ==
    \A r \in Readers :
        (rPc[r] = "Done" /\ rLocal[r].result # NoCommitsYet) =>
            \E i \in 1..Len(snapshots) : snapshots[i] = rLocal[r].result

\* The writer-side counterpart of ReaderSeesOnlyCommitted, and the one
\* property that makes a lost update visible: a writer that reports success
\* must have actually put its own proposed snapshot into committed history.
\* Without this, nothing in the model reads wPc or wLocal outside TypeOK, so a
\* writer could reach "Done" having committed nothing at all and every other
\* invariant would still hold -- confirmed by mutation test (deleting the
\* Append from TryAdvancePointer's Success branch passes clean without this
\* invariant, and is caught by it). RFC 0002's Summary names exactly this
\* failure ("lose a writer's data") as motivating the model; NoOverlappingRowIds
\* covers the other half of that sentence.
WriterSuccessIsCommitted ==
    \A w \in Writers :
        wPc[w] = "Done" =>
            \E i \in 1..Len(snapshots) : snapshots[i] = wLocal[w].proposed

====
