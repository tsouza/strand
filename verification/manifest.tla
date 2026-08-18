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

Next ==
    \/ \E w \in Writers : ReadCurrent(w) \/ ProposeSnapshot(w) \/ TryAdvancePointer(w) \/ ResolveAmbiguity(w)
    \/ \E r \in Readers : ReadPointer(r) \/ ReadSnapshotObject(r)

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

\* A reader that finishes with an actual result never reports a snapshot
\* that isn't really in the committed history -- ties the reader model to
\* writer-side ground truth, the property a reader-safety bug would break.
ReaderSeesOnlyCommitted ==
    \A r \in Readers :
        (rPc[r] = "Done" /\ rLocal[r].result # NoCommitsYetVal) =>
            \E i \in 1..Len(snapshots) : snapshots[i] = rLocal[r].result

====
