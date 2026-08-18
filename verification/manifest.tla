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
