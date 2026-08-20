---- MODULE manifest_proofs ----
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
(* TLAPS mechanized proof of verification/manifest.tla's safety           *)
(* invariants. RFC 0002 (rfcs/0002-manifest-formal-verification.md)'s     *)
(* remaining artifact, tracked as docs/roadmap.md's M3-2. Kept in a       *)
(* separate module (EXTENDS manifest) rather than folded into             *)
(* manifest.tla itself, so that TLC's own model checking of manifest.tla  *)
(* -- the artifact this file's proofs are grounded against -- stays       *)
(* untouched by proof-engineering iteration.                              *)
(*                                                                         *)
(* Status: see verification/README.md and rfcs/0002-manifest-formal-      *)
(* verification.md's Discussion section for the precise, tlapm-confirmed  *)
(* accounting of what is proved, what is partially proved, and what is    *)
(* not yet attempted. Do not trust a THEOREM name alone -- only a         *)
(* THEOREM this file's own `tlapm` run (see README.md for the exact       *)
(* invocation) reports as proved is actually proved.                     *)

EXTENDS Naturals, Sequences, FiniteSets, SequenceTheorems, TLAPS, manifest

vars == <<snapshots, wPc, wLocal, rPc, rLocal>>

(* ----------------------------------------------------------------------- *)
(* Generic EXCEPT facts, proved once and reused everywhere below. TLAPS's  *)
(* backends (zenon, then Isabelle, then SMT/z3) can discharge each of      *)
(* these directly via OBVIOUS, but empirically (confirmed by running       *)
(* tlapm against isolated test cases during this proof's development)     *)
(* cannot reliably re-derive them on demand when they are buried inside a  *)
(* larger goal alongside an action definition's own disjunctions and LETs. *)
(* Stating them as standalone lemmas and citing them by name is what makes *)
(* the per-action proofs below tractable at all.                          *)
(*                                                                         *)
(* The domain hypothesis in ExceptSame is load-bearing, not decoration:    *)
(* `[f EXCEPT ![x]=v][x]=v` requires x \in DOMAIN f under TLA+'s actual    *)
(* EXCEPT semantics ([f EXCEPT ![x]=v] == [y \in DOMAIN f |-> ...]), and   *)
(* tlapm's backends genuinely fail to discharge the fact without it        *)
(* present as an explicit hypothesis -- confirmed the hard way, by first   *)
(* trying without it and watching every backend report "Could not prove."  *)
(* ExceptOther needs no such hypothesis: at y # x, both sides of the       *)
(* equality reduce to the literal same expression f[y], defined or not.    *)
(* ----------------------------------------------------------------------- *)

LEMMA ExceptSame ==
    ASSUME NEW f, NEW x \in DOMAIN f, NEW v
    PROVE  [f EXCEPT ![x] = v][x] = v
OBVIOUS

LEMMA ExceptOther ==
    ASSUME NEW f, NEW x, NEW y, x # y, NEW v
    PROVE  [f EXCEPT ![x] = v][y] = f[y]
OBVIOUS

LEMMA ExceptType ==
    ASSUME NEW D, NEW T, NEW f \in [D -> T], NEW x \in D, NEW v \in T
    PROVE  [f EXCEPT ![x] = v] \in [D -> T]
OBVIOUS

LEMMA ExceptDomain ==
    ASSUME NEW f, NEW x, NEW v
    PROVE  DOMAIN [f EXCEPT ![x] = v] = DOMAIN f
OBVIOUS

(* SnapshotRec-membership fact used repeatedly: an element read out of a   *)
(* Seq(SnapshotRec) is itself a SnapshotRec. Isolated once so every later  *)
(* per-action step just cites SnapshotElt instead of re-deriving           *)
(* ElementOfSeq's instantiation each time.                                *)
(* ProposeSnapshot / ProposeDeletionVectorCommit update wLocal via a        *)
(* nested field-EXCEPT (`[wLocal EXCEPT ![w].proposed = v]`), TLA+ sugar    *)
(* for `[wLocal EXCEPT ![w] = [wLocal[w] EXCEPT !.proposed = v]]`. Same     *)
(* story as the ExceptSame/ExceptOther pair above: each conjunct here is   *)
(* OBVIOUS alone, but the two-step desugaring has to be spelled out        *)
(* explicitly (<1>1 first, then the plain record-field facts, combined at  *)
(* the end) or tlapm's backends fail to connect them.                      *)
LEMMA ExceptProposedAt ==
    ASSUME NEW f, NEW x \in DOMAIN f, NEW v
    PROVE  /\ [f EXCEPT ![x].proposed = v][x].proposed = v
           /\ [f EXCEPT ![x].proposed = v][x].baseVersion = f[x].baseVersion
           /\ [f EXCEPT ![x].proposed = v][x].nextRowId = f[x].nextRowId
<1>1. [f EXCEPT ![x].proposed = v][x] = [f[x] EXCEPT !.proposed = v]
  OBVIOUS
<1>2. [f[x] EXCEPT !.proposed = v].proposed = v
  OBVIOUS
<1>3. [f[x] EXCEPT !.proposed = v].baseVersion = f[x].baseVersion
  OBVIOUS
<1>4. [f[x] EXCEPT !.proposed = v].nextRowId = f[x].nextRowId
  OBVIOUS
<1>5. QED
  BY <1>1, <1>2, <1>3, <1>4

(* rLocal-field EXCEPT-projection facts, the reader-side counterparts of      *)
(* ExceptProposedAt above, needed once the reader actions (ReadPointer,       *)
(* ReadSnapshotObject) are in scope: both update rLocal via a nested          *)
(* field-EXCEPT (`[rLocal EXCEPT ![r].result = v]`, `.ptrVersion`, `.retries`)*)
(* -- the same two-step-desugaring shape ExceptProposedAt already handles for *)
(* wLocal's `proposed` field. One lemma per field an action in manifest.tla   *)
(* actually assigns: `result` (ReadPointer's Found/NoCommitsYet branches,     *)
(* ReadSnapshotObject's Found branch), `ptrVersion` (ReadPointer's Found      *)
(* branch), `retries` (ReadSnapshotObject's retry branch).                   *)
LEMMA ExceptResultAt ==
    ASSUME NEW f, NEW x \in DOMAIN f, NEW v
    PROVE  /\ [f EXCEPT ![x].result = v][x].result = v
           /\ [f EXCEPT ![x].result = v][x].retries = f[x].retries
           /\ [f EXCEPT ![x].result = v][x].ptrVersion = f[x].ptrVersion
<1>1. [f EXCEPT ![x].result = v][x] = [f[x] EXCEPT !.result = v]
  OBVIOUS
<1>2. [f[x] EXCEPT !.result = v].result = v
  OBVIOUS
<1>3. [f[x] EXCEPT !.result = v].retries = f[x].retries
  OBVIOUS
<1>4. [f[x] EXCEPT !.result = v].ptrVersion = f[x].ptrVersion
  OBVIOUS
<1>5. QED
  BY <1>1, <1>2, <1>3, <1>4

LEMMA ExceptPtrVersionAt ==
    ASSUME NEW f, NEW x \in DOMAIN f, NEW v
    PROVE  /\ [f EXCEPT ![x].ptrVersion = v][x].ptrVersion = v
           /\ [f EXCEPT ![x].ptrVersion = v][x].retries = f[x].retries
           /\ [f EXCEPT ![x].ptrVersion = v][x].result = f[x].result
<1>1. [f EXCEPT ![x].ptrVersion = v][x] = [f[x] EXCEPT !.ptrVersion = v]
  OBVIOUS
<1>2. [f[x] EXCEPT !.ptrVersion = v].ptrVersion = v
  OBVIOUS
<1>3. [f[x] EXCEPT !.ptrVersion = v].retries = f[x].retries
  OBVIOUS
<1>4. [f[x] EXCEPT !.ptrVersion = v].result = f[x].result
  OBVIOUS
<1>5. QED
  BY <1>1, <1>2, <1>3, <1>4

LEMMA ExceptRetriesAt ==
    ASSUME NEW f, NEW x \in DOMAIN f, NEW v
    PROVE  /\ [f EXCEPT ![x].retries = v][x].retries = v
           /\ [f EXCEPT ![x].retries = v][x].ptrVersion = f[x].ptrVersion
           /\ [f EXCEPT ![x].retries = v][x].result = f[x].result
<1>1. [f EXCEPT ![x].retries = v][x] = [f[x] EXCEPT !.retries = v]
  OBVIOUS
<1>2. [f[x] EXCEPT !.retries = v].retries = v
  OBVIOUS
<1>3. [f[x] EXCEPT !.retries = v].ptrVersion = f[x].ptrVersion
  OBVIOUS
<1>4. [f[x] EXCEPT !.retries = v].result = f[x].result
  OBVIOUS
<1>5. QED
  BY <1>1, <1>2, <1>3, <1>4

(* SegmentRec-EXCEPT membership fact for ProposeDeletionVectorCommit's        *)
(* revoke step: incrementing an already-typed SegmentRec's delVer field       *)
(* stays inside SegmentRec. This is the same shape as ExceptProposedAt above  *)
(* -- field-by-field facts about the EXCEPT expression, proved directly via   *)
(* ExceptSame/ExceptOther/ExceptDomain and combined once at the end -- rather *)
(* than first proving the EXCEPT expression EQUALS a literal record          *)
(* ([base|->.., count|->.., delVer|->..]) and then checking that literal's    *)
(* membership. An earlier version of this proof took the literal-equality     *)
(* route (`<3>eq`, citing only `DEF SegmentRec`) and that step did not        *)
(* reliably discharge with the documented backend versions (TLAPS 1.5.0,      *)
(* zenon 0.8.4, Isabelle2011-1, z3 4.8.9): reproduced 2/2 on a genuinely       *)
(* fresh, cache-cleared run (a single z3 subprocess spinning past 20 minutes  *)
(* on that one obligation), matching this file's own "Lessons" section        *)
(* below, which already flagged this exact step as the hardest in the file.   *)
(* This lemma sidesteps the literal-equality goal entirely -- the same fix    *)
(* direction ExceptProposedAt already uses for SnapshotRec's `proposed`       *)
(* field -- and discharges reliably in its place (see README.md's "TLAPS     *)
(* proof" section for the current, personally-reverified obligation count).   *)
LEMMA ExceptSegmentDelVer ==
    ASSUME NEW r \in SegmentRec, NEW v \in Nat
    PROVE  [r EXCEPT !.delVer = v] \in SegmentRec
<1>dom. DOMAIN r = {"base", "count", "delVer"}
  BY DEF SegmentRec
<1>domdv. "delVer" \in DOMAIN r
  BY <1>dom
<1>domE. DOMAIN [r EXCEPT !.delVer = v] = {"base", "count", "delVer"}
  BY <1>dom, ExceptDomain
<1>baseE. [r EXCEPT !.delVer = v].base = r.base
  BY ExceptOther
<1>countE. [r EXCEPT !.delVer = v].count = r.count
  BY ExceptOther
<1>delVerE. [r EXCEPT !.delVer = v].delVer = v
  BY <1>domdv, ExceptSame
<1>fields. r.base \in Nat /\ r.count \in Nat
  BY DEF SegmentRec
<1>qed. QED
  BY <1>domE, <1>baseE, <1>countE, <1>delVerE, <1>fields DEF SegmentRec

LEMMA SnapshotElt ==
    ASSUME NEW seq \in Seq(SnapshotRec), NEW k \in 1..Len(seq)
    PROVE  seq[k] \in SnapshotRec
BY ElementOfSeq

-----------------------------------------------------------------------------
(* Stage 1: the two "lost update" / "phantom snapshot" safety properties   *)
(* RFC 0002's own Motivation names directly ("silently commit overlapping  *)
(* row-ID ranges or lose a writer's data") -- the writer half              *)
(* (WriterSuccessIsCommitted) and the reader half (ReaderSeesOnlyCommitted)*)
(* of "nothing here lies about what got committed," carried alongside      *)
(* TypeOK (needed as a hypothesis by both, and proved inductive in its own *)
(* right since it is the domain of discourse every other invariant in      *)
(* verification/manifest.tla depends on).                                 *)
-----------------------------------------------------------------------------

(* FnDomains is not implied by TypeOK as written: TypeOK's wLocal/rLocal    *)
(* conjuncts are the pointwise `\A w \in Writers : wLocal[w]. ...` shape,   *)
(* which does not entail `DOMAIN wLocal = Writers` in TLA+ (unlike wPc/rPc, *)
(* whose TypeOK conjuncts are literal `\in [Writers -> ...]` membership,    *)
(* which DOES carry that domain fact by definition of the `[S -> T]`       *)
(* operator). Discovered the hard way: an early version of this file tried *)
(* `BY DEF IndInv1, TypeOK` to derive `DOMAIN wLocal = Writers` and tlapm   *)
(* rejected it outright, which is what forced separating this out as its   *)
(* own inductive conjunct, established directly at Init (wLocal is built   *)
(* there as `[w \in Writers |-> ...]`) and preserved by ExceptDomain        *)
(* (EXCEPT never changes a function's domain) rather than by TypeOK.       *)
FnDomains == DOMAIN wLocal = Writers /\ DOMAIN rLocal = Readers

(* Needed so ProposeSnapshot/ProposeDeletionVectorCommit can type-check     *)
(* `snapshots[wLocal[w].baseVersion]` when baseVersion > 0: without an      *)
(* upper bound relating a writer's cached baseVersion to the CURRENT        *)
(* length of `snapshots`, nothing says that index is even in bounds. True   *)
(* by construction (ReadCurrent's only write to baseVersion sets it to the  *)
(* CURRENT Len(snapshots); thereafter snapshots only grows, never shrinks,  *)
(* and no other action touches a writer's baseVersion field at all), but    *)
(* "true by construction" still has to be said as an explicit inductive     *)
(* conjunct for TLAPS to use it -- found necessary the same way FnDomains   *)
(* was, by first trying to type-check ProposeSnapshot's `proposed` record   *)
(* without it and watching tlapm reject the missing-bound obligation.       *)
BaseVersionBounded == \A w \in Writers : wLocal[w].baseVersion <= Len(snapshots)

(* Needed so TryAdvancePointer/ResolveAmbiguity can type-check the Append   *)
(* that lands a writer's own `wLocal[w].proposed` into `snapshots`:         *)
(* `TypeOK` alone only says that field is `NoProposal` OR a `SnapshotRec`,  *)
(* which is not enough to know it is safe to append at the specific moment *)
(* the pointer CAS succeeds. It is always a real `SnapshotRec` by that      *)
(* point (ProposeSnapshot/ProposeDeletionVectorCommit only ever advance a  *)
(* writer to "Advance" in the same step that sets its `.proposed` field to *)
(* a real record), but -- same story as FnDomains and BaseVersionBounded -- *)
(* that has to be said as its own inductive conjunct before TLAPS can use  *)
(* it; found necessary the same way, by first trying to type-check the     *)
(* Append in TryAdvancePointerStep1 without it.                            *)
ProposedIsReal == \A w \in Writers :
    wPc[w] \in {"Advance", "ResolveAmbiguity", "Done"} => wLocal[w].proposed \in SnapshotRec

(* The reader-side counterpart of BaseVersionBounded above, discovered the     *)
(* same way and necessary for the same reason: ReadSnapshotObjectStep1's       *)
(* Found branch indexes `snapshots[rLocal[r].ptrVersion]`, and for that read   *)
(* result to type-check -- and for ReaderSeesOnlyCommitted' to have an actual  *)
(* witness once the reader reaches "Done" -- something has to say that a       *)
(* reader parked at "ReadSnap" holds a ptrVersion that is a valid index into   *)
(* the CURRENT snapshots sequence. True by construction (ReadPointer's only    *)
(* write to ptrVersion sets it to the CURRENT Len(snapshots), guarded by       *)
(* Len(snapshots) > 0 in the very same branch that advances rPc[r] to          *)
(* "ReadSnap"; thereafter snapshots only grows, never shrinks, and no other    *)
(* action -- reader or writer -- touches a reader's ptrVersion field at all),  *)
(* but "true by construction" still has to be stated as its own explicit      *)
(* inductive conjunct before TLAPS can use it. Found necessary while writing   *)
(* ReadSnapshotObjectStep1's ReaderSeesOnlyCommitted' obligation without it    *)
(* and watching the existential witness step fail to type-check the index --  *)
(* the same discovery shape FnDomains/BaseVersionBounded/ProposedIsReal above  *)
(* were each found by, now on the reader side. IndInv1 therefore has seven     *)
(* conjuncts as of this addition, not six; ReadPointerStep1 and                *)
(* ReadSnapshotObjectStep1 below are the first (and, so far, only) theorems    *)
(* that need it, and every earlier theorem's proof gets one small additional  *)
(* step showing this conjunct is preserved too (trivial in each case, since    *)
(* no writer action touches rPc or rLocal, and snapshots only grows).         *)
PtrVersionBounded == \A r \in Readers :
    rPc[r] = "ReadSnap" => rLocal[r].ptrVersion \in 1..Len(snapshots)

IndInv1 == TypeOK /\ WriterSuccessIsCommitted /\ ReaderSeesOnlyCommitted /\ FnDomains
           /\ BaseVersionBounded /\ ProposedIsReal /\ PtrVersionBounded

THEOREM Init1 == Init => IndInv1
<1>1. Init => TypeOK
  BY DEF Init, TypeOK
<1>2. Init => WriterSuccessIsCommitted
  BY DEF Init, WriterSuccessIsCommitted
<1>3. Init => ReaderSeesOnlyCommitted
  BY DEF Init, ReaderSeesOnlyCommitted
<1>4. Init => FnDomains
  BY DEF Init, FnDomains
<1>5. Init => BaseVersionBounded
  BY DEF Init, BaseVersionBounded
<1>6. Init => ProposedIsReal
  BY DEF Init, ProposedIsReal
<1>7. Init => PtrVersionBounded
  BY DEF Init, PtrVersionBounded
<1>8. QED
  BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7 DEF IndInv1

(* ------------------------------------------------------------------- *)
(* Per-action step lemmas. Each shows IndInv1 preserved across one      *)
(* action, for an arbitrary acting writer/reader. `Next`'s own proof    *)
(* below is then just a case split over its disjuncts, citing these.    *)
(* ------------------------------------------------------------------- *)

THEOREM ReadCurrentStep1 ==
    ASSUME IndInv1, NEW w \in Writers, ReadCurrent(w)
    PROVE  IndInv1'
<1>dom. DOMAIN wLocal = Writers
  BY DEF IndInv1, FnDomains
<1>wldisj. \/ wLocal' = [wLocal EXCEPT ![w] =
                          [baseVersion |-> Len(snapshots),
                           nextRowId |-> IF Len(snapshots) = 0 THEN 0 ELSE snapshots[Len(snapshots)].nextRowId,
                           proposed |-> NoProposal]]
           \/ wLocal' = wLocal
  BY DEF ReadCurrent
<1>wpcdisj. \/ wPc' = [wPc EXCEPT ![w] = "Propose"]
            \/ wPc' = [wPc EXCEPT ![w] = "Failed"]
            \/ wPc' = [wPc EXCEPT ![w] = "Read"]
  BY DEF ReadCurrent
<1>unch. snapshots' = snapshots /\ rPc' = rPc /\ rLocal' = rLocal
  BY DEF ReadCurrent
<1>wlatw. \/ /\ wLocal'[w].baseVersion \in Nat
             /\ wLocal'[w].nextRowId \in Nat
             /\ wLocal'[w].proposed = NoProposal
          \/ wLocal'[w] = wLocal[w]
  <2>1. CASE wLocal' = wLocal
    BY <2>1
  <2>2. CASE wLocal' = [wLocal EXCEPT ![w] =
                         [baseVersion |-> Len(snapshots),
                          nextRowId |-> IF Len(snapshots) = 0 THEN 0 ELSE snapshots[Len(snapshots)].nextRowId,
                          proposed |-> NoProposal]]
    <3>a. wLocal'[w] = [baseVersion |-> Len(snapshots),
                         nextRowId |-> IF Len(snapshots) = 0 THEN 0 ELSE snapshots[Len(snapshots)].nextRowId,
                         proposed |-> NoProposal]
      BY <2>2, <1>dom, ExceptSame
    <3>b. Len(snapshots) \in Nat
      BY DEF IndInv1, TypeOK
    <3>c. snapshots = <<>> \/ snapshots[Len(snapshots)].nextRowId \in Nat
      <4>1. snapshots \in Seq(SnapshotRec)
        BY DEF IndInv1, TypeOK
      <4>2. snapshots # <<>> => Len(snapshots) \in 1..Len(snapshots)
        BY <4>1, LenProperties
      <4>3. snapshots # <<>> => snapshots[Len(snapshots)] \in SnapshotRec
        BY <4>1, <4>2, SnapshotElt
      <4>4. QED
        BY <4>3 DEF SnapshotRec
    <3>d. QED
      BY <3>a, <3>b, <3>c
  <2>3. QED
    BY <2>1, <2>2, <1>wldisj
<1>wpcatw. wPc'[w] \in {"Read", "Propose", "Advance", "ResolveAmbiguity", "Done", "Failed"} /\ wPc'[w] # "Done"
  <2>dom. DOMAIN wPc = Writers
    BY DEF IndInv1, TypeOK
  <2>rng. wPc \in [Writers -> {"Read", "Propose", "Advance", "ResolveAmbiguity", "Done", "Failed"}]
    BY DEF IndInv1, TypeOK
  <2>1. CASE wPc' = [wPc EXCEPT ![w] = "Propose"]
    BY <2>1, <2>dom, ExceptSame
  <2>2. CASE wPc' = [wPc EXCEPT ![w] = "Failed"]
    BY <2>2, <2>dom, ExceptSame
  <2>3. CASE wPc' = [wPc EXCEPT ![w] = "Read"]
    BY <2>3, <2>dom, ExceptSame
  <2>4. QED
    BY <2>1, <2>2, <2>3, <1>wpcdisj
<1>1. TypeOK'
  <2>a. snapshots' \in Seq(SnapshotRec)
    BY <1>unch DEF IndInv1, TypeOK
  <2>b. wPc' \in [Writers -> {"Read", "Propose", "Advance", "ResolveAmbiguity", "Done", "Failed"}]
    <3>rng. wPc \in [Writers -> {"Read", "Propose", "Advance", "ResolveAmbiguity", "Done", "Failed"}]
      BY DEF IndInv1, TypeOK
    <3>1. CASE wPc' = [wPc EXCEPT ![w] = "Propose"]
      BY <3>1, <3>rng, ExceptType
    <3>2. CASE wPc' = [wPc EXCEPT ![w] = "Failed"]
      BY <3>2, <3>rng, ExceptType
    <3>3. CASE wPc' = [wPc EXCEPT ![w] = "Read"]
      BY <3>3, <3>rng, ExceptType
    <3>4. QED
      BY <3>1, <3>2, <3>3, <1>wpcdisj
  <2>c. \A w0 \in Writers : wLocal'[w0].baseVersion \in Nat /\ wLocal'[w0].nextRowId \in Nat
             /\ (wLocal'[w0].proposed = NoProposal \/ wLocal'[w0].proposed \in SnapshotRec)
    <3> SUFFICES ASSUME NEW w0 \in Writers
                 PROVE  wLocal'[w0].baseVersion \in Nat /\ wLocal'[w0].nextRowId \in Nat
                        /\ (wLocal'[w0].proposed = NoProposal \/ wLocal'[w0].proposed \in SnapshotRec)
      OBVIOUS
    <3>1. CASE w0 = w
      BY <3>1, <1>wlatw DEF IndInv1, TypeOK
    <3>2. CASE w0 # w
      <4>a. wLocal'[w0] = wLocal[w0]
        BY <3>2, <1>wldisj, <1>dom, ExceptOther
      <4>b. QED
        BY <4>a DEF IndInv1, TypeOK
    <3>3. QED
      BY <3>1, <3>2
  <2>d. rPc' \in [Readers -> {"ReadPtr", "ReadSnap", "Done", "Failed_RetriesExhausted", "Failed_DefiniteFailure"}]
    BY <1>unch DEF IndInv1, TypeOK
  <2>e. \A r0 \in Readers : rLocal'[r0].retries \in Nat /\ rLocal'[r0].ptrVersion \in Nat
             /\ (rLocal'[r0].result = NoResult \/ rLocal'[r0].result = NoCommitsYet \/ rLocal'[r0].result \in SnapshotRec)
    BY <1>unch DEF IndInv1, TypeOK
  <2>f. QED
    BY <2>a, <2>b, <2>c, <2>d, <2>e DEF TypeOK
<1>2. WriterSuccessIsCommitted'
  <2> SUFFICES ASSUME NEW w0 \in Writers, wPc'[w0] = "Done"
               PROVE  \E i \in 1..Len(snapshots') : snapshots'[i] = wLocal'[w0].proposed
    BY DEF WriterSuccessIsCommitted
  <2>1. CASE w0 = w
    BY <2>1, <1>wpcatw
  <2>2. CASE w0 # w
    <3>a. wPc[w0] = "Done"
      <4>dom. DOMAIN wPc = Writers
        BY DEF IndInv1, TypeOK
      <4>b. wPc'[w0] = wPc[w0]
        <5>1. CASE wPc' = [wPc EXCEPT ![w] = "Propose"]
          BY <5>1, <2>2, ExceptOther
        <5>2. CASE wPc' = [wPc EXCEPT ![w] = "Failed"]
          BY <5>2, <2>2, ExceptOther
        <5>3. CASE wPc' = [wPc EXCEPT ![w] = "Read"]
          BY <5>3, <2>2, ExceptOther
        <5>4. QED
          BY <5>1, <5>2, <5>3, <1>wpcdisj
      <4>c. QED
        BY <4>b
    <3>b. \E i \in 1..Len(snapshots) : snapshots[i] = wLocal[w0].proposed
      BY <3>a DEF IndInv1, WriterSuccessIsCommitted
    <3>c. wLocal'[w0] = wLocal[w0]
      BY <2>2, <1>wldisj, <1>dom, ExceptOther
    <3>d. QED
      BY <3>b, <3>c, <1>unch
  <2>3. QED
    BY <2>1, <2>2
<1>3. ReaderSeesOnlyCommitted'
  BY <1>unch DEF IndInv1, ReaderSeesOnlyCommitted
<1>4. FnDomains'
  <2>a. DOMAIN wLocal' = Writers
    <3>1. CASE wLocal' = [wLocal EXCEPT ![w] =
                           [baseVersion |-> Len(snapshots),
                            nextRowId |-> IF Len(snapshots) = 0 THEN 0 ELSE snapshots[Len(snapshots)].nextRowId,
                            proposed |-> NoProposal]]
      BY <3>1, <1>dom, ExceptDomain
    <3>2. CASE wLocal' = wLocal
      BY <3>2, <1>dom
    <3>3. QED
      BY <3>1, <3>2, <1>wldisj
  <2>b. DOMAIN rLocal' = Readers
    BY <1>unch DEF IndInv1, FnDomains
  <2>c. QED
    BY <2>a, <2>b DEF FnDomains
<1>5. BaseVersionBounded'
  <2> SUFFICES ASSUME NEW w0 \in Writers PROVE wLocal'[w0].baseVersion <= Len(snapshots')
    BY DEF BaseVersionBounded
  <2>1. CASE w0 = w
    <3>1. CASE wLocal' = [wLocal EXCEPT ![w] =
                           [baseVersion |-> Len(snapshots),
                            nextRowId |-> IF Len(snapshots) = 0 THEN 0 ELSE snapshots[Len(snapshots)].nextRowId,
                            proposed |-> NoProposal]]
      <4>a. wLocal'[w] = [baseVersion |-> Len(snapshots),
                           nextRowId |-> IF Len(snapshots) = 0 THEN 0 ELSE snapshots[Len(snapshots)].nextRowId,
                           proposed |-> NoProposal]
        BY <3>1, <1>dom, ExceptSame
      <4>b. QED
        BY <4>a, <2>1, <1>unch
    <3>2. CASE wLocal' = wLocal
      BY <3>2, <2>1, <1>unch DEF IndInv1, BaseVersionBounded
    <3>3. QED
      BY <3>1, <3>2, <1>wldisj
  <2>2. CASE w0 # w
    <3>a. wLocal'[w0] = wLocal[w0]
      BY <2>2, <1>wldisj, <1>dom, ExceptOther
    <3>b. QED
      BY <3>a, <1>unch DEF IndInv1, BaseVersionBounded
  <2>3. QED
    BY <2>1, <2>2
<1>6. ProposedIsReal'
  <2> SUFFICES ASSUME NEW w0 \in Writers, wPc'[w0] \in {"Advance", "ResolveAmbiguity", "Done"}
               PROVE  wLocal'[w0].proposed \in SnapshotRec
    BY DEF ProposedIsReal
  <2>1. CASE w0 = w
    <3>domp. DOMAIN wPc = Writers
      BY DEF IndInv1, TypeOK
    <3>a. wPc'[w] \in {"Propose", "Failed", "Read"}
      <4>1. CASE wPc' = [wPc EXCEPT ![w] = "Propose"]
        BY <4>1, <3>domp, ExceptSame
      <4>2. CASE wPc' = [wPc EXCEPT ![w] = "Failed"]
        BY <4>2, <3>domp, ExceptSame
      <4>3. CASE wPc' = [wPc EXCEPT ![w] = "Read"]
        BY <4>3, <3>domp, ExceptSame
      <4>4. QED
        BY <4>1, <4>2, <4>3, <1>wpcdisj
    <3>b. QED
      BY <3>a, <2>1
  <2>2. CASE w0 # w
    <3>a. wPc'[w0] = wPc[w0]
      <4>1. CASE wPc' = [wPc EXCEPT ![w] = "Propose"]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE wPc' = [wPc EXCEPT ![w] = "Failed"]
        BY <4>2, <2>2, ExceptOther
      <4>3. CASE wPc' = [wPc EXCEPT ![w] = "Read"]
        BY <4>3, <2>2, ExceptOther
      <4>4. QED
        BY <4>1, <4>2, <4>3, <1>wpcdisj
    <3>b. wLocal'[w0] = wLocal[w0]
      BY <2>2, <1>wldisj, <1>dom, ExceptOther
    <3>c. QED
      BY <3>a, <3>b DEF IndInv1, ProposedIsReal
  <2>3. QED
    BY <2>1, <2>2
<1>7. PtrVersionBounded'
  BY <1>unch DEF IndInv1, PtrVersionBounded
<1>8. QED
  BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7 DEF IndInv1

THEOREM ProposeSnapshotStep1 ==
    ASSUME IndInv1, NEW w \in Writers, ProposeSnapshot(w)
    PROVE  IndInv1'
<1>rc. RowIdCounts[w] \in Nat \ {0}
  BY DEF RowIdCounts
<1>domw. DOMAIN wLocal = Writers
  BY DEF IndInv1, FnDomains
<1>disj. \/ /\ wLocal' = [wLocal EXCEPT ![w].proposed =
                          [version |-> wLocal[w].baseVersion,
                           nextRowId |-> wLocal[w].nextRowId + RowIdCounts[w],
                           segments |-> Append(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments,
                                                [base |-> wLocal[w].nextRowId, count |-> RowIdCounts[w], delVer |-> 0])]]
            /\ wPc' = [wPc EXCEPT ![w] = "Advance"]
            /\ UNCHANGED <<snapshots, rPc, rLocal>>
         \/ /\ wPc' = [wPc EXCEPT ![w] = "Failed"]
            /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>
  BY DEF ProposeSnapshot
<1>unchsnap. snapshots' = snapshots /\ rPc' = rPc /\ rLocal' = rLocal
  BY <1>disj
<1>priorsegs. wLocal[w].baseVersion # 0 => snapshots[wLocal[w].baseVersion].segments \in Seq(SegmentRec)
  <2>1. wLocal[w].baseVersion \in Nat
    BY DEF IndInv1, TypeOK
  <2>2. wLocal[w].baseVersion <= Len(snapshots)
    BY DEF IndInv1, BaseVersionBounded
  <2>3. wLocal[w].baseVersion # 0 => wLocal[w].baseVersion \in 1..Len(snapshots)
    BY <2>1, <2>2
  <2>4. snapshots \in Seq(SnapshotRec)
    BY DEF IndInv1, TypeOK
  <2>5. wLocal[w].baseVersion # 0 => snapshots[wLocal[w].baseVersion] \in SnapshotRec
    BY <2>3, <2>4, SnapshotElt
  <2>6. QED
    BY <2>5 DEF SnapshotRec
<1>newrecok. wLocal[w].baseVersion # 0 \/ TRUE =>
    [version |-> wLocal[w].baseVersion,
     nextRowId |-> wLocal[w].nextRowId + RowIdCounts[w],
     segments |-> Append(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments,
                          [base |-> wLocal[w].nextRowId, count |-> RowIdCounts[w], delVer |-> 0])] \in SnapshotRec
  <2>a. wLocal[w].baseVersion \in Nat /\ wLocal[w].nextRowId \in Nat
    BY DEF IndInv1, TypeOK
  <2>b. (IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments) \in Seq(SegmentRec)
    <3>1. CASE wLocal[w].baseVersion = 0
      <4>a. (IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments) = <<>>
        BY <3>1
      <4>b. <<>> \in Seq(SegmentRec)
        OBVIOUS
      <4>c. QED
        BY <4>a, <4>b
    <3>2. CASE wLocal[w].baseVersion # 0
      BY <3>2, <1>priorsegs
    <3>3. QED
      BY <3>1, <3>2
  <2>c. [base |-> wLocal[w].nextRowId, count |-> RowIdCounts[w], delVer |-> 0] \in SegmentRec
    BY <2>a, <1>rc DEF SegmentRec
  <2>d. Append(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments,
               [base |-> wLocal[w].nextRowId, count |-> RowIdCounts[w], delVer |-> 0]) \in Seq(SegmentRec)
    BY <2>b, <2>c, AppendProperties
  <2>e. QED
    BY <2>a, <1>rc, <2>d DEF SnapshotRec
<1>wlatw. \/ /\ wLocal'[w].baseVersion \in Nat
             /\ wLocal'[w].nextRowId \in Nat
             /\ wLocal'[w].proposed \in SnapshotRec
          \/ wLocal'[w] = wLocal[w]
  <2>1. CASE wLocal' = [wLocal EXCEPT ![w].proposed =
                         [version |-> wLocal[w].baseVersion,
                          nextRowId |-> wLocal[w].nextRowId + RowIdCounts[w],
                          segments |-> Append(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments,
                                               [base |-> wLocal[w].nextRowId, count |-> RowIdCounts[w], delVer |-> 0])]]
    <3>a. wLocal'[w].proposed =
            [version |-> wLocal[w].baseVersion,
             nextRowId |-> wLocal[w].nextRowId + RowIdCounts[w],
             segments |-> Append(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments,
                                  [base |-> wLocal[w].nextRowId, count |-> RowIdCounts[w], delVer |-> 0])]
      BY <2>1, <1>domw, ExceptProposedAt
    <3>b. wLocal'[w].baseVersion = wLocal[w].baseVersion /\ wLocal'[w].nextRowId = wLocal[w].nextRowId
      BY <2>1, <1>domw, ExceptProposedAt
    <3>c. wLocal[w].baseVersion \in Nat /\ wLocal[w].nextRowId \in Nat
      BY DEF IndInv1, TypeOK
    <3>d. QED
      BY <3>a, <3>b, <3>c, <1>newrecok
  <2>2. CASE wLocal' = wLocal
    BY <2>2
  <2>3. QED
    BY <2>1, <2>2, <1>disj
<1>wpcdisj. \/ wPc' = [wPc EXCEPT ![w] = "Advance"]
            \/ wPc' = [wPc EXCEPT ![w] = "Failed"]
  BY <1>disj
<1>domp. DOMAIN wPc = Writers
  BY DEF IndInv1, TypeOK
<1>wpcatw. wPc'[w] \in {"Read", "Propose", "Advance", "ResolveAmbiguity", "Done", "Failed"} /\ wPc'[w] # "Done"
  <2>rng. wPc \in [Writers -> {"Read", "Propose", "Advance", "ResolveAmbiguity", "Done", "Failed"}]
    BY DEF IndInv1, TypeOK
  <2>1. CASE wPc' = [wPc EXCEPT ![w] = "Advance"]
    BY <2>1, <1>domp, ExceptSame
  <2>2. CASE wPc' = [wPc EXCEPT ![w] = "Failed"]
    BY <2>2, <1>domp, ExceptSame
  <2>3. QED
    BY <2>1, <2>2, <1>wpcdisj
<1>1. TypeOK'
  <2>a. snapshots' \in Seq(SnapshotRec)
    BY <1>unchsnap DEF IndInv1, TypeOK
  <2>b. wPc' \in [Writers -> {"Read", "Propose", "Advance", "ResolveAmbiguity", "Done", "Failed"}]
    <3>rng. wPc \in [Writers -> {"Read", "Propose", "Advance", "ResolveAmbiguity", "Done", "Failed"}]
      BY DEF IndInv1, TypeOK
    <3>1. CASE wPc' = [wPc EXCEPT ![w] = "Advance"]
      BY <3>1, <3>rng, ExceptType
    <3>2. CASE wPc' = [wPc EXCEPT ![w] = "Failed"]
      BY <3>2, <3>rng, ExceptType
    <3>3. QED
      BY <3>1, <3>2, <1>wpcdisj
  <2>c. \A w0 \in Writers : wLocal'[w0].baseVersion \in Nat /\ wLocal'[w0].nextRowId \in Nat
             /\ (wLocal'[w0].proposed = NoProposal \/ wLocal'[w0].proposed \in SnapshotRec)
    <3> SUFFICES ASSUME NEW w0 \in Writers
                 PROVE  wLocal'[w0].baseVersion \in Nat /\ wLocal'[w0].nextRowId \in Nat
                        /\ (wLocal'[w0].proposed = NoProposal \/ wLocal'[w0].proposed \in SnapshotRec)
      OBVIOUS
    <3>1. CASE w0 = w
      BY <3>1, <1>wlatw DEF IndInv1, TypeOK
    <3>2. CASE w0 # w
      <4>a. wLocal'[w0] = wLocal[w0]
        <5>1. CASE wLocal' = [wLocal EXCEPT ![w].proposed =
                               [version |-> wLocal[w].baseVersion,
                                nextRowId |-> wLocal[w].nextRowId + RowIdCounts[w],
                                segments |-> Append(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments,
                                                     [base |-> wLocal[w].nextRowId, count |-> RowIdCounts[w], delVer |-> 0])]]
          BY <5>1, <3>2, ExceptOther
        <5>2. CASE wLocal' = wLocal
          BY <5>2
        <5>3. QED
          BY <5>1, <5>2, <1>disj
      <4>b. QED
        BY <4>a DEF IndInv1, TypeOK
    <3>3. QED
      BY <3>1, <3>2
  <2>d. rPc' \in [Readers -> {"ReadPtr", "ReadSnap", "Done", "Failed_RetriesExhausted", "Failed_DefiniteFailure"}]
    BY <1>unchsnap DEF IndInv1, TypeOK
  <2>e. \A r0 \in Readers : rLocal'[r0].retries \in Nat /\ rLocal'[r0].ptrVersion \in Nat
             /\ (rLocal'[r0].result = NoResult \/ rLocal'[r0].result = NoCommitsYet \/ rLocal'[r0].result \in SnapshotRec)
    BY <1>unchsnap DEF IndInv1, TypeOK
  <2>f. QED
    BY <2>a, <2>b, <2>c, <2>d, <2>e DEF TypeOK
<1>2. WriterSuccessIsCommitted'
  <2> SUFFICES ASSUME NEW w0 \in Writers, wPc'[w0] = "Done"
               PROVE  \E i \in 1..Len(snapshots') : snapshots'[i] = wLocal'[w0].proposed
    BY DEF WriterSuccessIsCommitted
  <2>1. CASE w0 = w
    BY <2>1, <1>wpcatw
  <2>2. CASE w0 # w
    <3>a. wPc'[w0] = wPc[w0]
      <4>1. CASE wPc' = [wPc EXCEPT ![w] = "Advance"]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE wPc' = [wPc EXCEPT ![w] = "Failed"]
        BY <4>2, <2>2, ExceptOther
      <4>3. QED
        BY <4>1, <4>2, <1>wpcdisj
    <3>b. wPc[w0] = "Done"
      BY <3>a, <2>2
    <3>c. \E i \in 1..Len(snapshots) : snapshots[i] = wLocal[w0].proposed
      BY <3>b DEF IndInv1, WriterSuccessIsCommitted
    <3>d. wLocal'[w0] = wLocal[w0]
      <4>1. CASE wLocal' = [wLocal EXCEPT ![w].proposed =
                             [version |-> wLocal[w].baseVersion,
                              nextRowId |-> wLocal[w].nextRowId + RowIdCounts[w],
                              segments |-> Append(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments,
                                                   [base |-> wLocal[w].nextRowId, count |-> RowIdCounts[w], delVer |-> 0])]]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE wLocal' = wLocal
        BY <4>2
      <4>3. QED
        BY <4>1, <4>2, <1>disj
    <3>e. QED
      BY <3>c, <3>d, <1>unchsnap
  <2>3. QED
    BY <2>1, <2>2
<1>3. ReaderSeesOnlyCommitted'
  BY <1>unchsnap DEF IndInv1, ReaderSeesOnlyCommitted
<1>4. FnDomains'
  <2>a. DOMAIN wLocal' = Writers
    <3>1. CASE wLocal' = [wLocal EXCEPT ![w].proposed =
                           [version |-> wLocal[w].baseVersion,
                            nextRowId |-> wLocal[w].nextRowId + RowIdCounts[w],
                            segments |-> Append(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments,
                                                 [base |-> wLocal[w].nextRowId, count |-> RowIdCounts[w], delVer |-> 0])]]
      BY <3>1, <1>domw, ExceptDomain
    <3>2. CASE wLocal' = wLocal
      BY <3>2, <1>domw
    <3>3. QED
      BY <3>1, <3>2, <1>disj
  <2>b. DOMAIN rLocal' = Readers
    BY <1>unchsnap DEF IndInv1, FnDomains
  <2>c. QED
    BY <2>a, <2>b DEF FnDomains
<1>5. BaseVersionBounded'
  <2> SUFFICES ASSUME NEW w0 \in Writers PROVE wLocal'[w0].baseVersion <= Len(snapshots')
    BY DEF BaseVersionBounded
  <2>1. CASE w0 = w
    <3>a. wLocal'[w].baseVersion = wLocal[w].baseVersion
      <4>1. CASE wLocal' = [wLocal EXCEPT ![w].proposed =
                             [version |-> wLocal[w].baseVersion,
                              nextRowId |-> wLocal[w].nextRowId + RowIdCounts[w],
                              segments |-> Append(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments,
                                                   [base |-> wLocal[w].nextRowId, count |-> RowIdCounts[w], delVer |-> 0])]]
        BY <4>1, <1>domw, ExceptProposedAt
      <4>2. CASE wLocal' = wLocal
        BY <4>2
      <4>3. QED
        BY <4>1, <4>2, <1>disj
    <3>b. QED
      BY <3>a, <2>1, <1>unchsnap DEF IndInv1, BaseVersionBounded
  <2>2. CASE w0 # w
    <3>a. wLocal'[w0] = wLocal[w0]
      <4>1. CASE wLocal' = [wLocal EXCEPT ![w].proposed =
                             [version |-> wLocal[w].baseVersion,
                              nextRowId |-> wLocal[w].nextRowId + RowIdCounts[w],
                              segments |-> Append(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments,
                                                   [base |-> wLocal[w].nextRowId, count |-> RowIdCounts[w], delVer |-> 0])]]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE wLocal' = wLocal
        BY <4>2
      <4>3. QED
        BY <4>1, <4>2, <1>disj
    <3>b. QED
      BY <3>a, <1>unchsnap DEF IndInv1, BaseVersionBounded
  <2>3. QED
    BY <2>1, <2>2
<1>6. ProposedIsReal'
  <2> SUFFICES ASSUME NEW w0 \in Writers, wPc'[w0] \in {"Advance", "ResolveAmbiguity", "Done"}
               PROVE  wLocal'[w0].proposed \in SnapshotRec
    BY DEF ProposedIsReal
  <2>1. CASE w0 = w
    <3>1. CASE /\ wLocal' = [wLocal EXCEPT ![w].proposed =
                              [version |-> wLocal[w].baseVersion,
                               nextRowId |-> wLocal[w].nextRowId + RowIdCounts[w],
                               segments |-> Append(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments,
                                                    [base |-> wLocal[w].nextRowId, count |-> RowIdCounts[w], delVer |-> 0])]]
               /\ wPc' = [wPc EXCEPT ![w] = "Advance"]
               /\ UNCHANGED <<snapshots, rPc, rLocal>>
      <4>a. wLocal'[w].proposed =
              [version |-> wLocal[w].baseVersion,
               nextRowId |-> wLocal[w].nextRowId + RowIdCounts[w],
               segments |-> Append(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments,
                                    [base |-> wLocal[w].nextRowId, count |-> RowIdCounts[w], delVer |-> 0])]
        BY <3>1, <1>domw, ExceptProposedAt
      <4>b. QED
        BY <2>1, <4>a, <1>newrecok
    <3>2. CASE wPc' = [wPc EXCEPT ![w] = "Failed"] /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>
      BY <3>2, <2>1, <1>domp, ExceptSame
    <3>3. QED
      BY <3>1, <3>2, <1>disj
  <2>2. CASE w0 # w
    <3>a. wPc'[w0] = wPc[w0]
      <4>1. CASE wPc' = [wPc EXCEPT ![w] = "Advance"]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE wPc' = [wPc EXCEPT ![w] = "Failed"]
        BY <4>2, <2>2, ExceptOther
      <4>3. QED
        BY <4>1, <4>2, <1>wpcdisj
    <3>b. wLocal'[w0] = wLocal[w0]
      <4>1. CASE wLocal' = [wLocal EXCEPT ![w].proposed =
                             [version |-> wLocal[w].baseVersion,
                              nextRowId |-> wLocal[w].nextRowId + RowIdCounts[w],
                              segments |-> Append(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments,
                                                   [base |-> wLocal[w].nextRowId, count |-> RowIdCounts[w], delVer |-> 0])]]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE wLocal' = wLocal
        BY <4>2
      <4>3. QED
        BY <4>1, <4>2, <1>disj
    <3>c. QED
      BY <3>a, <3>b DEF IndInv1, ProposedIsReal
  <2>3. QED
    BY <2>1, <2>2
<1>7. PtrVersionBounded'
  BY <1>unchsnap DEF IndInv1, PtrVersionBounded
<1>8. QED
  BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7 DEF IndInv1

THEOREM ProposeDeletionVectorCommitStep1 ==
    ASSUME IndInv1, NEW w \in Writers, ProposeDeletionVectorCommit(w)
    PROVE  IndInv1'
<1>domw. DOMAIN wLocal = Writers
  BY DEF IndInv1, FnDomains
<1>lenps. wLocal[w].baseVersion # 0 /\
          Len(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments) >= 1
  BY DEF ProposeDeletionVectorCommit
<1>priorsegs. (IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments) \in Seq(SegmentRec)
  <2>1. wLocal[w].baseVersion \in Nat
    BY DEF IndInv1, TypeOK
  <2>2. wLocal[w].baseVersion <= Len(snapshots)
    BY DEF IndInv1, BaseVersionBounded
  <2>3. wLocal[w].baseVersion \in 1..Len(snapshots)
    BY <2>1, <2>2, <1>lenps
  <2>4. snapshots \in Seq(SnapshotRec)
    BY DEF IndInv1, TypeOK
  <2>5. snapshots[wLocal[w].baseVersion] \in SnapshotRec
    BY <2>3, <2>4, SnapshotElt
  <2>6. QED
    BY <2>5, <1>lenps DEF SnapshotRec
<1>revok. [(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments)
             EXCEPT ![1].delVer = @ + 1] \in Seq(SegmentRec)
  (* Named locally via `<2> DEFINE` rather than repeating the IF-expression *)
  (* inline (as the surrounding steps in this file otherwise do): this is   *)
  (* the one step in the whole file where the repeated inline expression    *)
  (* reliably defeated every backend on the raw EXCEPT-of-unknown-record    *)
  (* membership check, taking 10+ minutes on a single z3/zenon subprocess   *)
  (* or failing outright, across three separate `tlapm` runs -- confirmed   *)
  (* the hard way. Shrinking the term via a local abbreviation (standard    *)
  (* TLAPS practice for exactly this situation) resolves the abbreviation   *)
  (* half of the problem; <2>d below then discharges the per-field         *)
  (* membership check via `ExceptSegmentDelVer` -- see that lemma's own     *)
  (* comment for why the literal-record-equality route this step used      *)
  (* to take was replaced.                                                 *)
  <2> DEFINE priorSeg1 == (IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments)[1]
  <2>a. 1 \in 1..Len(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments)
    BY <1>lenps
  <2>b. priorSeg1 \in SegmentRec
    BY <1>priorsegs, <2>a, SnapshotElt
  <2>c. priorSeg1.delVer \in Nat
    BY <2>b DEF SegmentRec
  <2>d. [priorSeg1 EXCEPT !.delVer = priorSeg1.delVer + 1] \in SegmentRec
    BY <2>b, <2>c, ExceptSegmentDelVer
  <2>e. [(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments) EXCEPT ![1].delVer = @ + 1]
        = [(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments) EXCEPT ![1] =
             [priorSeg1 EXCEPT !.delVer = priorSeg1.delVer + 1]]
    OBVIOUS
  <2>f. QED
    BY <1>priorsegs, <2>a, <2>d, <2>e, ExceptSeq
<1>newrecok. [version |-> wLocal[w].baseVersion,
              nextRowId |-> wLocal[w].nextRowId,
              segments |-> [(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments)
                              EXCEPT ![1].delVer = @ + 1]] \in SnapshotRec
  <2>a. wLocal[w].baseVersion \in Nat /\ wLocal[w].nextRowId \in Nat
    BY DEF IndInv1, TypeOK
  <2>b. QED
    BY <2>a, <1>revok DEF SnapshotRec
<1>disj. \/ /\ wLocal' = [wLocal EXCEPT ![w].proposed =
                          [version |-> wLocal[w].baseVersion,
                           nextRowId |-> wLocal[w].nextRowId,
                           segments |-> [(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments)
                                           EXCEPT ![1].delVer = @ + 1]]]
            /\ wPc' = [wPc EXCEPT ![w] = "Advance"]
            /\ UNCHANGED <<snapshots, rPc, rLocal>>
         \/ /\ wPc' = [wPc EXCEPT ![w] = "Failed"]
            /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>
  BY DEF ProposeDeletionVectorCommit
<1>unchsnap. snapshots' = snapshots /\ rPc' = rPc /\ rLocal' = rLocal
  BY <1>disj
<1>wlatw. \/ /\ wLocal'[w].baseVersion \in Nat
             /\ wLocal'[w].nextRowId \in Nat
             /\ wLocal'[w].proposed \in SnapshotRec
          \/ wLocal'[w] = wLocal[w]
  <2>1. CASE wLocal' = [wLocal EXCEPT ![w].proposed =
                         [version |-> wLocal[w].baseVersion,
                          nextRowId |-> wLocal[w].nextRowId,
                          segments |-> [(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments)
                                          EXCEPT ![1].delVer = @ + 1]]]
    <3>a. wLocal'[w].proposed =
            [version |-> wLocal[w].baseVersion,
             nextRowId |-> wLocal[w].nextRowId,
             segments |-> [(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments)
                             EXCEPT ![1].delVer = @ + 1]]
      BY <2>1, <1>domw, ExceptProposedAt
    <3>b. wLocal'[w].baseVersion = wLocal[w].baseVersion /\ wLocal'[w].nextRowId = wLocal[w].nextRowId
      BY <2>1, <1>domw, ExceptProposedAt
    <3>c. wLocal[w].baseVersion \in Nat /\ wLocal[w].nextRowId \in Nat
      BY DEF IndInv1, TypeOK
    <3>d. QED
      BY <3>a, <3>b, <3>c, <1>newrecok
  <2>2. CASE wLocal' = wLocal
    BY <2>2
  <2>3. QED
    BY <2>1, <2>2, <1>disj
<1>wpcdisj. \/ wPc' = [wPc EXCEPT ![w] = "Advance"]
            \/ wPc' = [wPc EXCEPT ![w] = "Failed"]
  BY <1>disj
<1>domp. DOMAIN wPc = Writers
  BY DEF IndInv1, TypeOK
<1>wpcatw. wPc'[w] \in {"Read", "Propose", "Advance", "ResolveAmbiguity", "Done", "Failed"} /\ wPc'[w] # "Done"
  <2>rng. wPc \in [Writers -> {"Read", "Propose", "Advance", "ResolveAmbiguity", "Done", "Failed"}]
    BY DEF IndInv1, TypeOK
  <2>1. CASE wPc' = [wPc EXCEPT ![w] = "Advance"]
    BY <2>1, <1>domp, ExceptSame
  <2>2. CASE wPc' = [wPc EXCEPT ![w] = "Failed"]
    BY <2>2, <1>domp, ExceptSame
  <2>3. QED
    BY <2>1, <2>2, <1>wpcdisj
<1>1. TypeOK'
  <2>a. snapshots' \in Seq(SnapshotRec)
    BY <1>unchsnap DEF IndInv1, TypeOK
  <2>b. wPc' \in [Writers -> {"Read", "Propose", "Advance", "ResolveAmbiguity", "Done", "Failed"}]
    <3>rng. wPc \in [Writers -> {"Read", "Propose", "Advance", "ResolveAmbiguity", "Done", "Failed"}]
      BY DEF IndInv1, TypeOK
    <3>1. CASE wPc' = [wPc EXCEPT ![w] = "Advance"]
      BY <3>1, <3>rng, ExceptType
    <3>2. CASE wPc' = [wPc EXCEPT ![w] = "Failed"]
      BY <3>2, <3>rng, ExceptType
    <3>3. QED
      BY <3>1, <3>2, <1>wpcdisj
  <2>c. \A w0 \in Writers : wLocal'[w0].baseVersion \in Nat /\ wLocal'[w0].nextRowId \in Nat
             /\ (wLocal'[w0].proposed = NoProposal \/ wLocal'[w0].proposed \in SnapshotRec)
    <3> SUFFICES ASSUME NEW w0 \in Writers
                 PROVE  wLocal'[w0].baseVersion \in Nat /\ wLocal'[w0].nextRowId \in Nat
                        /\ (wLocal'[w0].proposed = NoProposal \/ wLocal'[w0].proposed \in SnapshotRec)
      OBVIOUS
    <3>1. CASE w0 = w
      BY <3>1, <1>wlatw DEF IndInv1, TypeOK
    <3>2. CASE w0 # w
      <4>a. wLocal'[w0] = wLocal[w0]
        <5>1. CASE wLocal' = [wLocal EXCEPT ![w].proposed =
                               [version |-> wLocal[w].baseVersion,
                                nextRowId |-> wLocal[w].nextRowId,
                                segments |-> [(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments)
                                                EXCEPT ![1].delVer = @ + 1]]]
          BY <5>1, <3>2, ExceptOther
        <5>2. CASE wLocal' = wLocal
          BY <5>2
        <5>3. QED
          BY <5>1, <5>2, <1>disj
      <4>b. QED
        BY <4>a DEF IndInv1, TypeOK
    <3>3. QED
      BY <3>1, <3>2
  <2>d. rPc' \in [Readers -> {"ReadPtr", "ReadSnap", "Done", "Failed_RetriesExhausted", "Failed_DefiniteFailure"}]
    BY <1>unchsnap DEF IndInv1, TypeOK
  <2>e. \A r0 \in Readers : rLocal'[r0].retries \in Nat /\ rLocal'[r0].ptrVersion \in Nat
             /\ (rLocal'[r0].result = NoResult \/ rLocal'[r0].result = NoCommitsYet \/ rLocal'[r0].result \in SnapshotRec)
    BY <1>unchsnap DEF IndInv1, TypeOK
  <2>f. QED
    BY <2>a, <2>b, <2>c, <2>d, <2>e DEF TypeOK
<1>2. WriterSuccessIsCommitted'
  <2> SUFFICES ASSUME NEW w0 \in Writers, wPc'[w0] = "Done"
               PROVE  \E i \in 1..Len(snapshots') : snapshots'[i] = wLocal'[w0].proposed
    BY DEF WriterSuccessIsCommitted
  <2>1. CASE w0 = w
    BY <2>1, <1>wpcatw
  <2>2. CASE w0 # w
    <3>a. wPc'[w0] = wPc[w0]
      <4>1. CASE wPc' = [wPc EXCEPT ![w] = "Advance"]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE wPc' = [wPc EXCEPT ![w] = "Failed"]
        BY <4>2, <2>2, ExceptOther
      <4>3. QED
        BY <4>1, <4>2, <1>wpcdisj
    <3>b. wPc[w0] = "Done"
      BY <3>a, <2>2
    <3>c. \E i \in 1..Len(snapshots) : snapshots[i] = wLocal[w0].proposed
      BY <3>b DEF IndInv1, WriterSuccessIsCommitted
    <3>d. wLocal'[w0] = wLocal[w0]
      <4>1. CASE wLocal' = [wLocal EXCEPT ![w].proposed =
                             [version |-> wLocal[w].baseVersion,
                              nextRowId |-> wLocal[w].nextRowId,
                              segments |-> [(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments)
                                              EXCEPT ![1].delVer = @ + 1]]]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE wLocal' = wLocal
        BY <4>2
      <4>3. QED
        BY <4>1, <4>2, <1>disj
    <3>e. QED
      BY <3>c, <3>d, <1>unchsnap
  <2>3. QED
    BY <2>1, <2>2
<1>3. ReaderSeesOnlyCommitted'
  BY <1>unchsnap DEF IndInv1, ReaderSeesOnlyCommitted
<1>4. FnDomains'
  <2>a. DOMAIN wLocal' = Writers
    <3>1. CASE wLocal' = [wLocal EXCEPT ![w].proposed =
                           [version |-> wLocal[w].baseVersion,
                            nextRowId |-> wLocal[w].nextRowId,
                            segments |-> [(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments)
                                            EXCEPT ![1].delVer = @ + 1]]]
      BY <3>1, <1>domw, ExceptDomain
    <3>2. CASE wLocal' = wLocal
      BY <3>2, <1>domw
    <3>3. QED
      BY <3>1, <3>2, <1>disj
  <2>b. DOMAIN rLocal' = Readers
    BY <1>unchsnap DEF IndInv1, FnDomains
  <2>c. QED
    BY <2>a, <2>b DEF FnDomains
<1>5. BaseVersionBounded'
  <2> SUFFICES ASSUME NEW w0 \in Writers PROVE wLocal'[w0].baseVersion <= Len(snapshots')
    BY DEF BaseVersionBounded
  <2>1. CASE w0 = w
    <3>a. wLocal'[w].baseVersion = wLocal[w].baseVersion
      <4>1. CASE wLocal' = [wLocal EXCEPT ![w].proposed =
                             [version |-> wLocal[w].baseVersion,
                              nextRowId |-> wLocal[w].nextRowId,
                              segments |-> [(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments)
                                              EXCEPT ![1].delVer = @ + 1]]]
        BY <4>1, <1>domw, ExceptProposedAt
      <4>2. CASE wLocal' = wLocal
        BY <4>2
      <4>3. QED
        BY <4>1, <4>2, <1>disj
    <3>b. QED
      BY <3>a, <2>1, <1>unchsnap DEF IndInv1, BaseVersionBounded
  <2>2. CASE w0 # w
    <3>a. wLocal'[w0] = wLocal[w0]
      <4>1. CASE wLocal' = [wLocal EXCEPT ![w].proposed =
                             [version |-> wLocal[w].baseVersion,
                              nextRowId |-> wLocal[w].nextRowId,
                              segments |-> [(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments)
                                              EXCEPT ![1].delVer = @ + 1]]]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE wLocal' = wLocal
        BY <4>2
      <4>3. QED
        BY <4>1, <4>2, <1>disj
    <3>b. QED
      BY <3>a, <1>unchsnap DEF IndInv1, BaseVersionBounded
  <2>3. QED
    BY <2>1, <2>2
<1>6. ProposedIsReal'
  <2> SUFFICES ASSUME NEW w0 \in Writers, wPc'[w0] \in {"Advance", "ResolveAmbiguity", "Done"}
               PROVE  wLocal'[w0].proposed \in SnapshotRec
    BY DEF ProposedIsReal
  <2>1. CASE w0 = w
    <3>1. CASE /\ wLocal' = [wLocal EXCEPT ![w].proposed =
                              [version |-> wLocal[w].baseVersion,
                               nextRowId |-> wLocal[w].nextRowId,
                               segments |-> [(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments)
                                               EXCEPT ![1].delVer = @ + 1]]]
               /\ wPc' = [wPc EXCEPT ![w] = "Advance"]
               /\ UNCHANGED <<snapshots, rPc, rLocal>>
      <4>a. wLocal'[w].proposed =
              [version |-> wLocal[w].baseVersion,
               nextRowId |-> wLocal[w].nextRowId,
               segments |-> [(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments)
                               EXCEPT ![1].delVer = @ + 1]]
        BY <3>1, <1>domw, ExceptProposedAt
      <4>b. QED
        BY <2>1, <4>a, <1>newrecok
    <3>2. CASE wPc' = [wPc EXCEPT ![w] = "Failed"] /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>
      BY <3>2, <2>1, <1>domp, ExceptSame
    <3>3. QED
      BY <3>1, <3>2, <1>disj
  <2>2. CASE w0 # w
    <3>a. wPc'[w0] = wPc[w0]
      <4>1. CASE wPc' = [wPc EXCEPT ![w] = "Advance"]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE wPc' = [wPc EXCEPT ![w] = "Failed"]
        BY <4>2, <2>2, ExceptOther
      <4>3. QED
        BY <4>1, <4>2, <1>wpcdisj
    <3>b. wLocal'[w0] = wLocal[w0]
      <4>1. CASE wLocal' = [wLocal EXCEPT ![w].proposed =
                             [version |-> wLocal[w].baseVersion,
                              nextRowId |-> wLocal[w].nextRowId,
                              segments |-> [(IF wLocal[w].baseVersion = 0 THEN <<>> ELSE snapshots[wLocal[w].baseVersion].segments)
                                              EXCEPT ![1].delVer = @ + 1]]]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE wLocal' = wLocal
        BY <4>2
      <4>3. QED
        BY <4>1, <4>2, <1>disj
    <3>c. QED
      BY <3>a, <3>b DEF IndInv1, ProposedIsReal
  <2>3. QED
    BY <2>1, <2>2
<1>7. PtrVersionBounded'
  BY <1>unchsnap DEF IndInv1, PtrVersionBounded
<1>8. QED
  BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7 DEF IndInv1

THEOREM TryAdvancePointerStep1 ==
    ASSUME IndInv1, NEW w \in Writers, TryAdvancePointer(w)
    PROVE  IndInv1'
<1>disj. \/ /\ Len(snapshots) # wLocal[w].baseVersion
            /\ wPc' = [wPc EXCEPT ![w] = "Read"]
            /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>
         \/ /\ Len(snapshots) = wLocal[w].baseVersion
            /\ snapshots' = Append(snapshots, wLocal[w].proposed)
            /\ wPc' = [wPc EXCEPT ![w] = "Done"]
            /\ UNCHANGED <<wLocal, rPc, rLocal>>
         \/ /\ Len(snapshots) = wLocal[w].baseVersion
            /\ wPc' = [wPc EXCEPT ![w] = "Failed"]
            /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>
         \/ /\ Len(snapshots) = wLocal[w].baseVersion
            /\ wPc' = [wPc EXCEPT ![w] = "ResolveAmbiguity"]
            /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>
  BY DEF TryAdvancePointer
<1>wlunch. wLocal' = wLocal /\ rPc' = rPc /\ rLocal' = rLocal
  BY <1>disj
<1>domw. DOMAIN wLocal = Writers
  BY DEF IndInv1, FnDomains
<1>domp. DOMAIN wPc = Writers
  BY DEF IndInv1, TypeOK
<1>propw. wLocal[w].proposed \in SnapshotRec
  BY DEF IndInv1, ProposedIsReal, TryAdvancePointer
<1>snaplen. Len(snapshots) <= Len(snapshots')
  <2>1. CASE snapshots' = Append(snapshots, wLocal[w].proposed)
    <3>a. snapshots \in Seq(SnapshotRec)
      BY DEF IndInv1, TypeOK
    <3>b. QED
      BY <2>1, <3>a, <1>propw, AppendProperties
  <2>2. CASE snapshots' = snapshots
    BY <2>2
  <2>3. QED
    BY <2>1, <2>2, <1>disj
<1>snappres. \A i \in 1..Len(snapshots) : snapshots'[i] = snapshots[i]
  <2>1. CASE snapshots' = Append(snapshots, wLocal[w].proposed)
    <3>a. snapshots \in Seq(SnapshotRec)
      BY DEF IndInv1, TypeOK
    <3>b. QED
      BY <2>1, <3>a, <1>propw, AppendProperties
  <2>2. CASE snapshots' = snapshots
    BY <2>2
  <2>3. QED
    BY <2>1, <2>2, <1>disj
<1>wpcdisj. \/ wPc' = [wPc EXCEPT ![w] = "Read"]
            \/ wPc' = [wPc EXCEPT ![w] = "Done"]
            \/ wPc' = [wPc EXCEPT ![w] = "Failed"]
            \/ wPc' = [wPc EXCEPT ![w] = "ResolveAmbiguity"]
  BY <1>disj
<1>1. TypeOK'
  <2>a. snapshots' \in Seq(SnapshotRec)
    <3>1. CASE snapshots' = Append(snapshots, wLocal[w].proposed)
      <4>a. snapshots \in Seq(SnapshotRec)
        BY DEF IndInv1, TypeOK
      <4>b. QED
        BY <3>1, <4>a, <1>propw, AppendProperties
    <3>2. CASE snapshots' = snapshots
      BY <3>2 DEF IndInv1, TypeOK
    <3>3. QED
      BY <3>1, <3>2, <1>disj
  <2>b. wPc' \in [Writers -> {"Read", "Propose", "Advance", "ResolveAmbiguity", "Done", "Failed"}]
    <3>1. CASE wPc' = [wPc EXCEPT ![w] = "Read"]
      BY <3>1, <1>domp, ExceptType DEF IndInv1, TypeOK
    <3>2. CASE wPc' = [wPc EXCEPT ![w] = "Done"]
      BY <3>2, <1>domp, ExceptType DEF IndInv1, TypeOK
    <3>3. CASE wPc' = [wPc EXCEPT ![w] = "Failed"]
      BY <3>3, <1>domp, ExceptType DEF IndInv1, TypeOK
    <3>4. CASE wPc' = [wPc EXCEPT ![w] = "ResolveAmbiguity"]
      BY <3>4, <1>domp, ExceptType DEF IndInv1, TypeOK
    <3>5. QED
      BY <3>1, <3>2, <3>3, <3>4, <1>wpcdisj
  <2>c. \A w0 \in Writers : wLocal'[w0].baseVersion \in Nat /\ wLocal'[w0].nextRowId \in Nat
             /\ (wLocal'[w0].proposed = NoProposal \/ wLocal'[w0].proposed \in SnapshotRec)
    BY <1>wlunch DEF IndInv1, TypeOK
  <2>d. rPc' \in [Readers -> {"ReadPtr", "ReadSnap", "Done", "Failed_RetriesExhausted", "Failed_DefiniteFailure"}]
    BY <1>wlunch DEF IndInv1, TypeOK
  <2>e. \A r0 \in Readers : rLocal'[r0].retries \in Nat /\ rLocal'[r0].ptrVersion \in Nat
             /\ (rLocal'[r0].result = NoResult \/ rLocal'[r0].result = NoCommitsYet \/ rLocal'[r0].result \in SnapshotRec)
    BY <1>wlunch DEF IndInv1, TypeOK
  <2>f. QED
    BY <2>a, <2>b, <2>c, <2>d, <2>e DEF TypeOK
<1>2. WriterSuccessIsCommitted'
  <2> SUFFICES ASSUME NEW w0 \in Writers, wPc'[w0] = "Done"
               PROVE  \E i \in 1..Len(snapshots') : snapshots'[i] = wLocal'[w0].proposed
    BY DEF WriterSuccessIsCommitted
  <2>1. CASE w0 = w
    <3>a. snapshots' = Append(snapshots, wLocal[w].proposed)
      (* <1>disj's branches are full conjunctions; a CASE naming only the   *)
      (* wPc' conjunct of the "Done" branch (as an earlier draft of this    *)
      (* proof did) does not carry that branch's sibling snapshots' fact    *)
      (* along with it -- found the hard way, confirmed genuinely unprovable*)
      (* rather than merely flaky. Matching the branch's FULL statement in  *)
      (* each CASE (as done throughout the rest of this file) fixes it.    *)
      <4>1. CASE Len(snapshots) # wLocal[w].baseVersion /\ wPc' = [wPc EXCEPT ![w] = "Read"]
                 /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>
        BY <4>1, <2>1, <1>domp, ExceptSame
      <4>2. CASE Len(snapshots) = wLocal[w].baseVersion /\ snapshots' = Append(snapshots, wLocal[w].proposed)
                 /\ wPc' = [wPc EXCEPT ![w] = "Done"] /\ UNCHANGED <<wLocal, rPc, rLocal>>
        BY <4>2
      <4>3. CASE Len(snapshots) = wLocal[w].baseVersion /\ wPc' = [wPc EXCEPT ![w] = "Failed"]
                 /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>
        BY <4>3, <2>1, <1>domp, ExceptSame
      <4>4. CASE Len(snapshots) = wLocal[w].baseVersion /\ wPc' = [wPc EXCEPT ![w] = "ResolveAmbiguity"]
                 /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>
        BY <4>4, <2>1, <1>domp, ExceptSame
      <4>5. QED
        BY <4>1, <4>2, <4>3, <4>4, <1>disj
    <3>b. snapshots \in Seq(SnapshotRec)
      BY DEF IndInv1, TypeOK
    (* Split into two atomic facts (membership, then the value at that      *)
    (* index) rather than one conjunction, and prove the goal's existential *)
    (* witness explicitly -- the combined single-step form here reliably    *)
    (* defeated every backend across two separate `tlapm` runs even though  *)
    (* each half is easy alone; same lesson as <1>revok elsewhere in this   *)
    (* file, that backends need existential witnesses spelled out, not left *)
    (* to be inferred from a bundle of hypotheses.                         *)
    <3>c1. Len(snapshots) + 1 \in 1..Len(snapshots')
      BY <3>a, <3>b, <1>propw, AppendProperties
    <3>c2. snapshots'[Len(snapshots) + 1] = wLocal[w].proposed
      BY <3>a, <3>b, <1>propw, AppendProperties
    <3>d. wLocal'[w0].proposed = wLocal[w].proposed
      BY <2>1, <1>wlunch
    <3>e. QED
      BY <3>c1, <3>c2, <3>d
  <2>2. CASE w0 # w
    <3>a. wPc'[w0] = wPc[w0]
      <4>1. CASE wPc' = [wPc EXCEPT ![w] = "Read"]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE wPc' = [wPc EXCEPT ![w] = "Done"]
        BY <4>2, <2>2, ExceptOther
      <4>3. CASE wPc' = [wPc EXCEPT ![w] = "Failed"]
        BY <4>3, <2>2, ExceptOther
      <4>4. CASE wPc' = [wPc EXCEPT ![w] = "ResolveAmbiguity"]
        BY <4>4, <2>2, ExceptOther
      <4>5. QED
        BY <4>1, <4>2, <4>3, <4>4, <1>wpcdisj
    <3>b. wPc[w0] = "Done"
      BY <3>a, <2>2
    <3>c. \E i \in 1..Len(snapshots) : snapshots[i] = wLocal[w0].proposed
      BY <3>b DEF IndInv1, WriterSuccessIsCommitted
    <3>d. wLocal'[w0] = wLocal[w0]
      BY <1>wlunch
    <3>e. QED
      <4> PICK i \in 1..Len(snapshots) : snapshots[i] = wLocal[w0].proposed
        BY <3>c
      <4>i1. i \in 1..Len(snapshots')
        BY <1>snaplen
      <4>i2. snapshots'[i] = wLocal'[w0].proposed
        BY <1>snappres, <3>d
      <4>qed. QED
        BY <4>i1, <4>i2
  <2>3. QED
    BY <2>1, <2>2
<1>3. ReaderSeesOnlyCommitted'
  <2> SUFFICES ASSUME NEW r \in Readers, rPc'[r] = "Done", rLocal'[r].result # NoCommitsYet
               PROVE  \E i \in 1..Len(snapshots') : snapshots'[i] = rLocal'[r].result
    BY DEF ReaderSeesOnlyCommitted
  <2>a. rPc[r] = "Done" /\ rLocal[r].result # NoCommitsYet
    BY <1>wlunch
  <2>b. \E i \in 1..Len(snapshots) : snapshots[i] = rLocal[r].result
    BY <2>a DEF IndInv1, ReaderSeesOnlyCommitted
  (* Same lesson as WriterSuccessIsCommitted's w0#w case above: the         *)
  (* combined `BY <2>b, <1>snappres, <1>wlunch` reliably failed to close    *)
  (* the goal's existential on its own; PICK-ing the witness out of <2>b    *)
  (* explicitly and re-proving membership/value at that witness fixes it.  *)
  <2>c. QED
    <3> PICK i \in 1..Len(snapshots) : snapshots[i] = rLocal[r].result
      BY <2>b
    <3>i1. i \in 1..Len(snapshots')
      BY <1>snaplen
    <3>i2. snapshots'[i] = rLocal'[r].result
      BY <1>snappres, <1>wlunch
    <3>qed. QED
      BY <3>i1, <3>i2
<1>4. FnDomains'
  <2>a. DOMAIN wLocal' = Writers
    BY <1>wlunch, <1>domw
  <2>b. DOMAIN rLocal' = Readers
    BY <1>wlunch DEF IndInv1, FnDomains
  <2>c. QED
    BY <2>a, <2>b DEF FnDomains
<1>5. BaseVersionBounded'
  <2> SUFFICES ASSUME NEW w0 \in Writers PROVE wLocal'[w0].baseVersion <= Len(snapshots')
    BY DEF BaseVersionBounded
  <2>a. wLocal'[w0].baseVersion = wLocal[w0].baseVersion
    BY <1>wlunch
  <2>b. wLocal[w0].baseVersion <= Len(snapshots)
    BY DEF IndInv1, BaseVersionBounded
  (* The plain `BY <2>b, <1>snaplen` combination (transitivity of <=)       *)
  (* reliably failed here even though the same shape of fact proves         *)
  (* trivially with OBVIOUS when `wLocal[w0].baseVersion \in Nat` is an     *)
  (* explicit hypothesis -- confirmed in isolation. Without it, tlapm's     *)
  (* backends apparently will not commit to <= transitivity axioms for an   *)
  (* untyped term, even though `<2>b`'s own `<=` establishes the same fact  *)
  (* implicitly (BaseVersionBounded's domain is Nat by TypeOK).            *)
  <2>bnat. wLocal[w0].baseVersion \in Nat
    BY DEF IndInv1, TypeOK
  <2>c. wLocal[w0].baseVersion <= Len(snapshots')
    BY <2>b, <2>bnat, <1>snaplen
  <2>d. QED
    BY <2>a, <2>c
<1>6. ProposedIsReal'
  <2> SUFFICES ASSUME NEW w0 \in Writers, wPc'[w0] \in {"Advance", "ResolveAmbiguity", "Done"}
               PROVE  wLocal'[w0].proposed \in SnapshotRec
    BY DEF ProposedIsReal
  <2>1. CASE w0 = w
    BY <2>1, <1>wlunch, <1>propw
  <2>2. CASE w0 # w
    <3>a. wPc'[w0] = wPc[w0]
      <4>1. CASE wPc' = [wPc EXCEPT ![w] = "Read"]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE wPc' = [wPc EXCEPT ![w] = "Done"]
        BY <4>2, <2>2, ExceptOther
      <4>3. CASE wPc' = [wPc EXCEPT ![w] = "Failed"]
        BY <4>3, <2>2, ExceptOther
      <4>4. CASE wPc' = [wPc EXCEPT ![w] = "ResolveAmbiguity"]
        BY <4>4, <2>2, ExceptOther
      <4>5. QED
        BY <4>1, <4>2, <4>3, <4>4, <1>wpcdisj
    <3>b. wLocal'[w0] = wLocal[w0]
      BY <1>wlunch
    <3>c. QED
      BY <3>a, <3>b DEF IndInv1, ProposedIsReal
  <2>3. QED
    BY <2>1, <2>2
<1>7. PtrVersionBounded'
  <2> SUFFICES ASSUME NEW r \in Readers, rPc'[r] = "ReadSnap"
               PROVE  rLocal'[r].ptrVersion \in 1..Len(snapshots')
    BY DEF PtrVersionBounded
  <2>rpc. rPc[r] = "ReadSnap"
    BY <1>wlunch
  <2>rl. rLocal'[r].ptrVersion = rLocal[r].ptrVersion
    BY <1>wlunch
  <2>old. rLocal[r].ptrVersion \in 1..Len(snapshots)
    BY <2>rpc DEF IndInv1, PtrVersionBounded
  <2>nat. rLocal[r].ptrVersion \in Nat
    BY <2>old
  <2>le. rLocal[r].ptrVersion <= Len(snapshots')
    BY <2>old, <2>nat, <1>snaplen
  <2>ge. rLocal[r].ptrVersion >= 1
    BY <2>old
  <2>lep. rLocal'[r].ptrVersion <= Len(snapshots')
    BY <2>rl, <2>le
  <2>gep. rLocal'[r].ptrVersion >= 1
    BY <2>rl, <2>ge
  <2>natp. rLocal'[r].ptrVersion \in Nat
    BY <2>rl, <2>nat
  <2>qed. QED
    BY <2>lep, <2>gep, <2>natp
<1>8. QED
  BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7 DEF IndInv1

THEOREM ResolveAmbiguityStep1 ==
    ASSUME IndInv1, NEW w \in Writers, ResolveAmbiguity(w)
    PROVE  IndInv1'
<1>disj. \/ /\ Len(snapshots) = wLocal[w].baseVersion
            /\ snapshots' = Append(snapshots, wLocal[w].proposed)
            /\ wPc' = [wPc EXCEPT ![w] = "Done"]
            /\ UNCHANGED <<wLocal, rPc, rLocal>>
         \/ /\ wPc' = [wPc EXCEPT ![w] = "Read"]
            /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>
         \/ /\ wPc' = [wPc EXCEPT ![w] = "Failed"]
            /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>
  BY DEF ResolveAmbiguity
<1>wlunch. wLocal' = wLocal /\ rPc' = rPc /\ rLocal' = rLocal
  BY <1>disj
<1>domw. DOMAIN wLocal = Writers
  BY DEF IndInv1, FnDomains
<1>domp. DOMAIN wPc = Writers
  BY DEF IndInv1, TypeOK
<1>propw. wLocal[w].proposed \in SnapshotRec
  BY DEF IndInv1, ProposedIsReal, ResolveAmbiguity
<1>snaplen. Len(snapshots) <= Len(snapshots')
  <2>1. CASE snapshots' = Append(snapshots, wLocal[w].proposed)
    <3>a. snapshots \in Seq(SnapshotRec)
      BY DEF IndInv1, TypeOK
    <3>b. QED
      BY <2>1, <3>a, <1>propw, AppendProperties
  <2>2. CASE snapshots' = snapshots
    BY <2>2
  <2>3. QED
    BY <2>1, <2>2, <1>disj
<1>snappres. \A i \in 1..Len(snapshots) : snapshots'[i] = snapshots[i]
  <2>1. CASE snapshots' = Append(snapshots, wLocal[w].proposed)
    <3>a. snapshots \in Seq(SnapshotRec)
      BY DEF IndInv1, TypeOK
    <3>b. QED
      BY <2>1, <3>a, <1>propw, AppendProperties
  <2>2. CASE snapshots' = snapshots
    BY <2>2
  <2>3. QED
    BY <2>1, <2>2, <1>disj
<1>wpcdisj. \/ wPc' = [wPc EXCEPT ![w] = "Done"]
            \/ wPc' = [wPc EXCEPT ![w] = "Read"]
            \/ wPc' = [wPc EXCEPT ![w] = "Failed"]
  BY <1>disj
<1>1. TypeOK'
  <2>a. snapshots' \in Seq(SnapshotRec)
    <3>1. CASE snapshots' = Append(snapshots, wLocal[w].proposed)
      <4>a. snapshots \in Seq(SnapshotRec)
        BY DEF IndInv1, TypeOK
      <4>b. QED
        BY <3>1, <4>a, <1>propw, AppendProperties
    <3>2. CASE snapshots' = snapshots
      BY <3>2 DEF IndInv1, TypeOK
    <3>3. QED
      BY <3>1, <3>2, <1>disj
  <2>b. wPc' \in [Writers -> {"Read", "Propose", "Advance", "ResolveAmbiguity", "Done", "Failed"}]
    <3>1. CASE wPc' = [wPc EXCEPT ![w] = "Done"]
      BY <3>1, <1>domp, ExceptType DEF IndInv1, TypeOK
    <3>2. CASE wPc' = [wPc EXCEPT ![w] = "Read"]
      BY <3>2, <1>domp, ExceptType DEF IndInv1, TypeOK
    <3>3. CASE wPc' = [wPc EXCEPT ![w] = "Failed"]
      BY <3>3, <1>domp, ExceptType DEF IndInv1, TypeOK
    <3>4. QED
      BY <3>1, <3>2, <3>3, <1>wpcdisj
  <2>c. \A w0 \in Writers : wLocal'[w0].baseVersion \in Nat /\ wLocal'[w0].nextRowId \in Nat
             /\ (wLocal'[w0].proposed = NoProposal \/ wLocal'[w0].proposed \in SnapshotRec)
    BY <1>wlunch DEF IndInv1, TypeOK
  <2>d. rPc' \in [Readers -> {"ReadPtr", "ReadSnap", "Done", "Failed_RetriesExhausted", "Failed_DefiniteFailure"}]
    BY <1>wlunch DEF IndInv1, TypeOK
  <2>e. \A r0 \in Readers : rLocal'[r0].retries \in Nat /\ rLocal'[r0].ptrVersion \in Nat
             /\ (rLocal'[r0].result = NoResult \/ rLocal'[r0].result = NoCommitsYet \/ rLocal'[r0].result \in SnapshotRec)
    BY <1>wlunch DEF IndInv1, TypeOK
  <2>f. QED
    BY <2>a, <2>b, <2>c, <2>d, <2>e DEF TypeOK
<1>2. WriterSuccessIsCommitted'
  <2> SUFFICES ASSUME NEW w0 \in Writers, wPc'[w0] = "Done"
               PROVE  \E i \in 1..Len(snapshots') : snapshots'[i] = wLocal'[w0].proposed
    BY DEF WriterSuccessIsCommitted
  <2>1. CASE w0 = w
    <3>a. snapshots' = Append(snapshots, wLocal[w].proposed)
      <4>1. CASE Len(snapshots) = wLocal[w].baseVersion /\ snapshots' = Append(snapshots, wLocal[w].proposed)
                 /\ wPc' = [wPc EXCEPT ![w] = "Done"] /\ UNCHANGED <<wLocal, rPc, rLocal>>
        BY <4>1
      <4>2. CASE wPc' = [wPc EXCEPT ![w] = "Read"] /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>
        BY <4>2, <2>1, <1>domp, ExceptSame
      <4>3. CASE wPc' = [wPc EXCEPT ![w] = "Failed"] /\ UNCHANGED <<snapshots, wLocal, rPc, rLocal>>
        BY <4>3, <2>1, <1>domp, ExceptSame
      <4>4. QED
        BY <4>1, <4>2, <4>3, <1>disj
    <3>b. snapshots \in Seq(SnapshotRec)
      BY DEF IndInv1, TypeOK
    <3>c1. Len(snapshots) + 1 \in 1..Len(snapshots')
      BY <3>a, <3>b, <1>propw, AppendProperties
    <3>c2. snapshots'[Len(snapshots) + 1] = wLocal[w].proposed
      BY <3>a, <3>b, <1>propw, AppendProperties
    <3>d. wLocal'[w0].proposed = wLocal[w].proposed
      BY <2>1, <1>wlunch
    <3>e. QED
      BY <3>c1, <3>c2, <3>d
  <2>2. CASE w0 # w
    <3>a. wPc'[w0] = wPc[w0]
      <4>1. CASE wPc' = [wPc EXCEPT ![w] = "Done"]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE wPc' = [wPc EXCEPT ![w] = "Read"]
        BY <4>2, <2>2, ExceptOther
      <4>3. CASE wPc' = [wPc EXCEPT ![w] = "Failed"]
        BY <4>3, <2>2, ExceptOther
      <4>4. QED
        BY <4>1, <4>2, <4>3, <1>wpcdisj
    <3>b. wPc[w0] = "Done"
      BY <3>a, <2>2
    <3>c. \E i \in 1..Len(snapshots) : snapshots[i] = wLocal[w0].proposed
      BY <3>b DEF IndInv1, WriterSuccessIsCommitted
    <3>d. wLocal'[w0] = wLocal[w0]
      BY <1>wlunch
    <3>e. QED
      <4> PICK i \in 1..Len(snapshots) : snapshots[i] = wLocal[w0].proposed
        BY <3>c
      <4>i1. i \in 1..Len(snapshots')
        BY <1>snaplen
      <4>i2. snapshots'[i] = wLocal'[w0].proposed
        BY <1>snappres, <3>d
      <4>qed. QED
        BY <4>i1, <4>i2
  <2>3. QED
    BY <2>1, <2>2
<1>3. ReaderSeesOnlyCommitted'
  <2> SUFFICES ASSUME NEW r \in Readers, rPc'[r] = "Done", rLocal'[r].result # NoCommitsYet
               PROVE  \E i \in 1..Len(snapshots') : snapshots'[i] = rLocal'[r].result
    BY DEF ReaderSeesOnlyCommitted
  <2>a. rPc[r] = "Done" /\ rLocal[r].result # NoCommitsYet
    BY <1>wlunch
  <2>b. \E i \in 1..Len(snapshots) : snapshots[i] = rLocal[r].result
    BY <2>a DEF IndInv1, ReaderSeesOnlyCommitted
  <2>c. QED
    <3> PICK i \in 1..Len(snapshots) : snapshots[i] = rLocal[r].result
      BY <2>b
    <3>i1. i \in 1..Len(snapshots')
      BY <1>snaplen
    <3>i2. snapshots'[i] = rLocal'[r].result
      BY <1>snappres, <1>wlunch
    <3>qed. QED
      BY <3>i1, <3>i2
<1>4. FnDomains'
  <2>a. DOMAIN wLocal' = Writers
    BY <1>wlunch, <1>domw
  <2>b. DOMAIN rLocal' = Readers
    BY <1>wlunch DEF IndInv1, FnDomains
  <2>c. QED
    BY <2>a, <2>b DEF FnDomains
<1>5. BaseVersionBounded'
  <2> SUFFICES ASSUME NEW w0 \in Writers PROVE wLocal'[w0].baseVersion <= Len(snapshots')
    BY DEF BaseVersionBounded
  <2>a. wLocal'[w0].baseVersion = wLocal[w0].baseVersion
    BY <1>wlunch
  <2>b. wLocal[w0].baseVersion <= Len(snapshots)
    BY DEF IndInv1, BaseVersionBounded
  <2>bnat. wLocal[w0].baseVersion \in Nat
    BY DEF IndInv1, TypeOK
  <2>c. wLocal[w0].baseVersion <= Len(snapshots')
    BY <2>b, <2>bnat, <1>snaplen
  <2>d. QED
    BY <2>a, <2>c
<1>6. ProposedIsReal'
  <2> SUFFICES ASSUME NEW w0 \in Writers, wPc'[w0] \in {"Advance", "ResolveAmbiguity", "Done"}
               PROVE  wLocal'[w0].proposed \in SnapshotRec
    BY DEF ProposedIsReal
  <2>1. CASE w0 = w
    BY <2>1, <1>wlunch, <1>propw
  <2>2. CASE w0 # w
    <3>a. wPc'[w0] = wPc[w0]
      <4>1. CASE wPc' = [wPc EXCEPT ![w] = "Done"]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE wPc' = [wPc EXCEPT ![w] = "Read"]
        BY <4>2, <2>2, ExceptOther
      <4>3. CASE wPc' = [wPc EXCEPT ![w] = "Failed"]
        BY <4>3, <2>2, ExceptOther
      <4>4. QED
        BY <4>1, <4>2, <4>3, <1>wpcdisj
    <3>b. wLocal'[w0] = wLocal[w0]
      BY <1>wlunch
    <3>c. QED
      BY <3>a, <3>b DEF IndInv1, ProposedIsReal
  <2>3. QED
    BY <2>1, <2>2
<1>7. PtrVersionBounded'
  <2> SUFFICES ASSUME NEW r \in Readers, rPc'[r] = "ReadSnap"
               PROVE  rLocal'[r].ptrVersion \in 1..Len(snapshots')
    BY DEF PtrVersionBounded
  <2>rpc. rPc[r] = "ReadSnap"
    BY <1>wlunch
  <2>rl. rLocal'[r].ptrVersion = rLocal[r].ptrVersion
    BY <1>wlunch
  <2>old. rLocal[r].ptrVersion \in 1..Len(snapshots)
    BY <2>rpc DEF IndInv1, PtrVersionBounded
  <2>nat. rLocal[r].ptrVersion \in Nat
    BY <2>old
  <2>le. rLocal[r].ptrVersion <= Len(snapshots')
    BY <2>old, <2>nat, <1>snaplen
  <2>ge. rLocal[r].ptrVersion >= 1
    BY <2>old
  <2>lep. rLocal'[r].ptrVersion <= Len(snapshots')
    BY <2>rl, <2>le
  <2>gep. rLocal'[r].ptrVersion >= 1
    BY <2>rl, <2>ge
  <2>natp. rLocal'[r].ptrVersion \in Nat
    BY <2>rl, <2>nat
  <2>qed. QED
    BY <2>lep, <2>gep, <2>natp
<1>8. QED
  BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7 DEF IndInv1

THEOREM ReadPointerStep1 ==
    ASSUME IndInv1, NEW r \in Readers, ReadPointer(r)
    PROVE  IndInv1'
<1>domr. DOMAIN rLocal = Readers
  BY DEF IndInv1, FnDomains
<1>domp. DOMAIN rPc = Readers
  BY DEF IndInv1, TypeOK
<1>disj. \/ /\ Len(snapshots) = 0
            /\ rPc' = [rPc EXCEPT ![r] = "Done"]
            /\ rLocal' = [rLocal EXCEPT ![r].result = NoCommitsYet]
            /\ UNCHANGED <<snapshots, wPc, wLocal>>
         \/ /\ Len(snapshots) > 0
            /\ rLocal' = [rLocal EXCEPT ![r].ptrVersion = Len(snapshots)]
            /\ rPc' = [rPc EXCEPT ![r] = "ReadSnap"]
            /\ UNCHANGED <<snapshots, wPc, wLocal>>
         \/ /\ rPc' = [rPc EXCEPT ![r] = "Failed_DefiniteFailure"]
            /\ UNCHANGED <<snapshots, wPc, wLocal, rLocal>>
  BY DEF ReadPointer
<1>unch. snapshots' = snapshots /\ wPc' = wPc /\ wLocal' = wLocal
  BY <1>disj
<1>rpcdisj. \/ rPc' = [rPc EXCEPT ![r] = "Done"]
            \/ rPc' = [rPc EXCEPT ![r] = "ReadSnap"]
            \/ rPc' = [rPc EXCEPT ![r] = "Failed_DefiniteFailure"]
  BY <1>disj
<1>rlatr. \/ /\ rLocal'[r].retries \in Nat
             /\ rLocal'[r].ptrVersion \in Nat
             /\ (rLocal'[r].result = NoResult \/ rLocal'[r].result = NoCommitsYet \/ rLocal'[r].result \in SnapshotRec)
          \/ rLocal'[r] = rLocal[r]
  <2>1. CASE rLocal' = [rLocal EXCEPT ![r].result = NoCommitsYet]
    <3>a. rLocal'[r].result = NoCommitsYet
      BY <2>1, <1>domr, ExceptResultAt
    <3>b. rLocal'[r].retries = rLocal[r].retries /\ rLocal'[r].ptrVersion = rLocal[r].ptrVersion
      BY <2>1, <1>domr, ExceptResultAt
    <3>c. rLocal[r].retries \in Nat /\ rLocal[r].ptrVersion \in Nat
      BY DEF IndInv1, TypeOK
    <3>d. QED
      BY <3>a, <3>b, <3>c
  <2>2. CASE rLocal' = [rLocal EXCEPT ![r].ptrVersion = Len(snapshots)]
    <3>a. rLocal'[r].ptrVersion = Len(snapshots)
      BY <2>2, <1>domr, ExceptPtrVersionAt
    <3>b. rLocal'[r].retries = rLocal[r].retries /\ rLocal'[r].result = rLocal[r].result
      BY <2>2, <1>domr, ExceptPtrVersionAt
    <3>c. Len(snapshots) \in Nat
      BY DEF IndInv1, TypeOK
    <3>d. rLocal[r].retries \in Nat
      BY DEF IndInv1, TypeOK
    <3>e. rLocal[r].result = NoResult \/ rLocal[r].result = NoCommitsYet \/ rLocal[r].result \in SnapshotRec
      BY DEF IndInv1, TypeOK
    <3>f. QED
      BY <3>a, <3>b, <3>c, <3>d, <3>e
  <2>3. CASE rLocal' = rLocal
    BY <2>3
  <2>4. QED
    BY <2>1, <2>2, <2>3, <1>disj
<1>1. TypeOK'
  <2>a. snapshots' \in Seq(SnapshotRec)
    BY <1>unch DEF IndInv1, TypeOK
  <2>b. wPc' \in [Writers -> {"Read", "Propose", "Advance", "ResolveAmbiguity", "Done", "Failed"}]
    BY <1>unch DEF IndInv1, TypeOK
  <2>c. \A w0 \in Writers : wLocal'[w0].baseVersion \in Nat /\ wLocal'[w0].nextRowId \in Nat
             /\ (wLocal'[w0].proposed = NoProposal \/ wLocal'[w0].proposed \in SnapshotRec)
    BY <1>unch DEF IndInv1, TypeOK
  <2>d. rPc' \in [Readers -> {"ReadPtr", "ReadSnap", "Done", "Failed_RetriesExhausted", "Failed_DefiniteFailure"}]
    <3>rng. rPc \in [Readers -> {"ReadPtr", "ReadSnap", "Done", "Failed_RetriesExhausted", "Failed_DefiniteFailure"}]
      BY DEF IndInv1, TypeOK
    <3>1. CASE rPc' = [rPc EXCEPT ![r] = "Done"]
      BY <3>1, <3>rng, ExceptType
    <3>2. CASE rPc' = [rPc EXCEPT ![r] = "ReadSnap"]
      BY <3>2, <3>rng, ExceptType
    <3>3. CASE rPc' = [rPc EXCEPT ![r] = "Failed_DefiniteFailure"]
      BY <3>3, <3>rng, ExceptType
    <3>4. QED
      BY <3>1, <3>2, <3>3, <1>rpcdisj
  <2>e. \A r0 \in Readers : rLocal'[r0].retries \in Nat /\ rLocal'[r0].ptrVersion \in Nat
             /\ (rLocal'[r0].result = NoResult \/ rLocal'[r0].result = NoCommitsYet \/ rLocal'[r0].result \in SnapshotRec)
    <3> SUFFICES ASSUME NEW r0 \in Readers
                 PROVE  rLocal'[r0].retries \in Nat /\ rLocal'[r0].ptrVersion \in Nat
                        /\ (rLocal'[r0].result = NoResult \/ rLocal'[r0].result = NoCommitsYet \/ rLocal'[r0].result \in SnapshotRec)
      OBVIOUS
    <3>1. CASE r0 = r
      BY <3>1, <1>rlatr DEF IndInv1, TypeOK
    <3>2. CASE r0 # r
      <4>a. rLocal'[r0] = rLocal[r0]
        <5>1. CASE rLocal' = [rLocal EXCEPT ![r].result = NoCommitsYet]
          BY <5>1, <3>2, ExceptOther
        <5>2. CASE rLocal' = [rLocal EXCEPT ![r].ptrVersion = Len(snapshots)]
          BY <5>2, <3>2, ExceptOther
        <5>3. CASE rLocal' = rLocal
          BY <5>3
        <5>4. QED
          BY <5>1, <5>2, <5>3, <1>disj
      <4>b. QED
        BY <4>a DEF IndInv1, TypeOK
    <3>3. QED
      BY <3>1, <3>2
  <2>f. QED
    BY <2>a, <2>b, <2>c, <2>d, <2>e DEF TypeOK
<1>2. WriterSuccessIsCommitted'
  BY <1>unch DEF IndInv1, WriterSuccessIsCommitted
<1>3. ReaderSeesOnlyCommitted'
  <2> SUFFICES ASSUME NEW r0 \in Readers, rPc'[r0] = "Done", rLocal'[r0].result # NoCommitsYet
               PROVE  \E i \in 1..Len(snapshots') : snapshots'[i] = rLocal'[r0].result
    BY DEF ReaderSeesOnlyCommitted
  <2>1. CASE r0 = r
    <3>1. CASE /\ Len(snapshots) = 0
               /\ rPc' = [rPc EXCEPT ![r] = "Done"]
               /\ rLocal' = [rLocal EXCEPT ![r].result = NoCommitsYet]
               /\ UNCHANGED <<snapshots, wPc, wLocal>>
      <4>a. rLocal'[r].result = NoCommitsYet
        BY <3>1, <1>domr, ExceptResultAt
      <4>b. QED
        BY <2>1, <4>a
    <3>2. CASE /\ Len(snapshots) > 0
               /\ rLocal' = [rLocal EXCEPT ![r].ptrVersion = Len(snapshots)]
               /\ rPc' = [rPc EXCEPT ![r] = "ReadSnap"]
               /\ UNCHANGED <<snapshots, wPc, wLocal>>
      <4>a. rPc'[r] = "ReadSnap"
        BY <3>2, <1>domp, ExceptSame
      <4>b. QED
        BY <2>1, <4>a
    <3>3. CASE /\ rPc' = [rPc EXCEPT ![r] = "Failed_DefiniteFailure"]
               /\ UNCHANGED <<snapshots, wPc, wLocal, rLocal>>
      <4>a. rPc'[r] = "Failed_DefiniteFailure"
        BY <3>3, <1>domp, ExceptSame
      <4>b. QED
        BY <2>1, <4>a
    <3>4. QED
      BY <3>1, <3>2, <3>3, <1>disj
  <2>2. CASE r0 # r
    <3>a. rPc'[r0] = rPc[r0]
      <4>1. CASE rPc' = [rPc EXCEPT ![r] = "Done"]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE rPc' = [rPc EXCEPT ![r] = "ReadSnap"]
        BY <4>2, <2>2, ExceptOther
      <4>3. CASE rPc' = [rPc EXCEPT ![r] = "Failed_DefiniteFailure"]
        BY <4>3, <2>2, ExceptOther
      <4>4. QED
        BY <4>1, <4>2, <4>3, <1>rpcdisj
    <3>b. rLocal'[r0] = rLocal[r0]
      <4>1. CASE rLocal' = [rLocal EXCEPT ![r].result = NoCommitsYet]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE rLocal' = [rLocal EXCEPT ![r].ptrVersion = Len(snapshots)]
        BY <4>2, <2>2, ExceptOther
      <4>3. CASE rLocal' = rLocal
        BY <4>3
      <4>4. QED
        BY <4>1, <4>2, <4>3, <1>disj
    <3>c. rPc[r0] = "Done" /\ rLocal[r0].result # NoCommitsYet
      BY <2>2, <3>a, <3>b
    <3>d. \E i \in 1..Len(snapshots) : snapshots[i] = rLocal[r0].result
      BY <3>c DEF IndInv1, ReaderSeesOnlyCommitted
    <3>e. QED
      <4> PICK i \in 1..Len(snapshots) : snapshots[i] = rLocal[r0].result
        BY <3>d
      <4>i1. i \in 1..Len(snapshots')
        BY <1>unch
      <4>i2. snapshots'[i] = rLocal'[r0].result
        BY <1>unch, <3>b
      <4>qed. QED
        BY <4>i1, <4>i2
  <2>3. QED
    BY <2>1, <2>2
<1>4. FnDomains'
  <2>a. DOMAIN wLocal' = Writers
    BY <1>unch DEF IndInv1, FnDomains
  <2>b. DOMAIN rLocal' = Readers
    <3>1. CASE rLocal' = [rLocal EXCEPT ![r].result = NoCommitsYet]
      BY <3>1, <1>domr, ExceptDomain
    <3>2. CASE rLocal' = [rLocal EXCEPT ![r].ptrVersion = Len(snapshots)]
      BY <3>2, <1>domr, ExceptDomain
    <3>3. CASE rLocal' = rLocal
      BY <3>3, <1>domr
    <3>4. QED
      BY <3>1, <3>2, <3>3, <1>disj
  <2>c. QED
    BY <2>a, <2>b DEF FnDomains
<1>5. BaseVersionBounded'
  BY <1>unch DEF IndInv1, BaseVersionBounded
<1>6. ProposedIsReal'
  BY <1>unch DEF IndInv1, ProposedIsReal
<1>7. PtrVersionBounded'
  <2> SUFFICES ASSUME NEW r0 \in Readers, rPc'[r0] = "ReadSnap"
               PROVE  rLocal'[r0].ptrVersion \in 1..Len(snapshots')
    BY DEF PtrVersionBounded
  <2>1. CASE r0 = r
    <3>1. CASE /\ Len(snapshots) = 0
               /\ rPc' = [rPc EXCEPT ![r] = "Done"]
               /\ rLocal' = [rLocal EXCEPT ![r].result = NoCommitsYet]
               /\ UNCHANGED <<snapshots, wPc, wLocal>>
      <4>a. rPc'[r] = "Done"
        BY <3>1, <1>domp, ExceptSame
      <4>b. QED
        BY <2>1, <4>a
    <3>2. CASE /\ Len(snapshots) > 0
               /\ rLocal' = [rLocal EXCEPT ![r].ptrVersion = Len(snapshots)]
               /\ rPc' = [rPc EXCEPT ![r] = "ReadSnap"]
               /\ UNCHANGED <<snapshots, wPc, wLocal>>
      <4>a. rLocal'[r].ptrVersion = Len(snapshots)
        BY <3>2, <1>domr, ExceptPtrVersionAt
      <4>b. snapshots' = snapshots
        BY <3>2
      <4>c. Len(snapshots) \in Nat
        BY DEF IndInv1, TypeOK
      <4>d. QED
        BY <2>1, <3>2, <4>a, <4>b, <4>c
    <3>3. CASE /\ rPc' = [rPc EXCEPT ![r] = "Failed_DefiniteFailure"]
               /\ UNCHANGED <<snapshots, wPc, wLocal, rLocal>>
      <4>a. rPc'[r] = "Failed_DefiniteFailure"
        BY <3>3, <1>domp, ExceptSame
      <4>b. QED
        BY <2>1, <4>a
    <3>4. QED
      BY <3>1, <3>2, <3>3, <1>disj
  <2>2. CASE r0 # r
    <3>a. rPc'[r0] = rPc[r0]
      <4>1. CASE rPc' = [rPc EXCEPT ![r] = "Done"]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE rPc' = [rPc EXCEPT ![r] = "ReadSnap"]
        BY <4>2, <2>2, ExceptOther
      <4>3. CASE rPc' = [rPc EXCEPT ![r] = "Failed_DefiniteFailure"]
        BY <4>3, <2>2, ExceptOther
      <4>4. QED
        BY <4>1, <4>2, <4>3, <1>rpcdisj
    <3>b. rLocal'[r0] = rLocal[r0]
      <4>1. CASE rLocal' = [rLocal EXCEPT ![r].result = NoCommitsYet]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE rLocal' = [rLocal EXCEPT ![r].ptrVersion = Len(snapshots)]
        BY <4>2, <2>2, ExceptOther
      <4>3. CASE rLocal' = rLocal
        BY <4>3
      <4>4. QED
        BY <4>1, <4>2, <4>3, <1>disj
    <3>c. rPc[r0] = "ReadSnap"
      BY <2>2, <3>a
    <3>d. rLocal[r0].ptrVersion \in 1..Len(snapshots)
      BY <3>c DEF IndInv1, PtrVersionBounded
    <3>dnat. rLocal[r0].ptrVersion \in Nat
      BY <3>d
    <3>e. Len(snapshots) <= Len(snapshots')
      BY <1>unch
    <3>f. rLocal[r0].ptrVersion <= Len(snapshots')
      BY <3>d, <3>dnat, <3>e
    <3>g. rLocal[r0].ptrVersion >= 1
      BY <3>d
    <3>bp. rLocal'[r0].ptrVersion = rLocal[r0].ptrVersion
      BY <3>b
    <3>fp. rLocal'[r0].ptrVersion <= Len(snapshots')
      BY <3>bp, <3>f
    <3>gp. rLocal'[r0].ptrVersion >= 1
      BY <3>bp, <3>g
    <3>pnat. rLocal'[r0].ptrVersion \in Nat
      BY <3>bp, <3>dnat
    <3>h. QED
      BY <3>fp, <3>gp, <3>pnat
  <2>3. QED
    BY <2>1, <2>2
<1>8. QED
  BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7 DEF IndInv1

THEOREM ReadSnapshotObjectStep1 ==
    ASSUME IndInv1, NEW r \in Readers, ReadSnapshotObject(r)
    PROVE  IndInv1'
<1>domr. DOMAIN rLocal = Readers
  BY DEF IndInv1, FnDomains
<1>domp. DOMAIN rPc = Readers
  BY DEF IndInv1, TypeOK
<1>disj. \/ /\ rPc' = [rPc EXCEPT ![r] = "Done"]
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
  BY DEF ReadSnapshotObject
<1>unch. snapshots' = snapshots /\ wPc' = wPc /\ wLocal' = wLocal
  BY <1>disj
<1>rpcdisj. \/ rPc' = [rPc EXCEPT ![r] = "Done"]
            \/ rPc' = [rPc EXCEPT ![r] = "ReadPtr"]
            \/ rPc' = [rPc EXCEPT ![r] = "Failed_RetriesExhausted"]
            \/ rPc' = [rPc EXCEPT ![r] = "Failed_DefiniteFailure"]
  BY <1>disj
<1>rinsnap. rLocal[r].ptrVersion \in 1..Len(snapshots)
  BY DEF IndInv1, PtrVersionBounded, ReadSnapshotObject
<1>resultok. snapshots[rLocal[r].ptrVersion] \in SnapshotRec
  <2>a. snapshots \in Seq(SnapshotRec)
    BY DEF IndInv1, TypeOK
  <2>b. QED
    BY <2>a, <1>rinsnap, SnapshotElt
<1>rlatr. \/ /\ rLocal'[r].retries \in Nat
             /\ rLocal'[r].ptrVersion \in Nat
             /\ (rLocal'[r].result = NoResult \/ rLocal'[r].result = NoCommitsYet \/ rLocal'[r].result \in SnapshotRec)
          \/ rLocal'[r] = rLocal[r]
  <2>1. CASE rLocal' = [rLocal EXCEPT ![r].result = snapshots[rLocal[r].ptrVersion]]
    <3>a. rLocal'[r].result = snapshots[rLocal[r].ptrVersion]
      BY <2>1, <1>domr, ExceptResultAt
    <3>b. rLocal'[r].retries = rLocal[r].retries /\ rLocal'[r].ptrVersion = rLocal[r].ptrVersion
      BY <2>1, <1>domr, ExceptResultAt
    <3>c. rLocal[r].retries \in Nat /\ rLocal[r].ptrVersion \in Nat
      BY DEF IndInv1, TypeOK
    <3>d. QED
      BY <3>a, <3>b, <3>c, <1>resultok
  <2>2. CASE rLocal' = [rLocal EXCEPT ![r].retries = @ + 1]
    <3>a. rLocal'[r].retries = rLocal[r].retries + 1
      BY <2>2, <1>domr, ExceptRetriesAt
    <3>b. rLocal'[r].ptrVersion = rLocal[r].ptrVersion /\ rLocal'[r].result = rLocal[r].result
      BY <2>2, <1>domr, ExceptRetriesAt
    <3>c. rLocal[r].retries \in Nat
      BY DEF IndInv1, TypeOK
    <3>d. rLocal[r].ptrVersion \in Nat
      BY DEF IndInv1, TypeOK
    <3>e. rLocal[r].result = NoResult \/ rLocal[r].result = NoCommitsYet \/ rLocal[r].result \in SnapshotRec
      BY DEF IndInv1, TypeOK
    <3>f. rLocal[r].retries + 1 \in Nat
      BY <3>c
    <3>g. QED
      BY <3>a, <3>b, <3>f, <3>d, <3>e
  <2>3. CASE rLocal' = rLocal
    BY <2>3
  <2>4. QED
    BY <2>1, <2>2, <2>3, <1>disj
<1>1. TypeOK'
  <2>a. snapshots' \in Seq(SnapshotRec)
    BY <1>unch DEF IndInv1, TypeOK
  <2>b. wPc' \in [Writers -> {"Read", "Propose", "Advance", "ResolveAmbiguity", "Done", "Failed"}]
    BY <1>unch DEF IndInv1, TypeOK
  <2>c. \A w0 \in Writers : wLocal'[w0].baseVersion \in Nat /\ wLocal'[w0].nextRowId \in Nat
             /\ (wLocal'[w0].proposed = NoProposal \/ wLocal'[w0].proposed \in SnapshotRec)
    BY <1>unch DEF IndInv1, TypeOK
  <2>d. rPc' \in [Readers -> {"ReadPtr", "ReadSnap", "Done", "Failed_RetriesExhausted", "Failed_DefiniteFailure"}]
    <3>rng. rPc \in [Readers -> {"ReadPtr", "ReadSnap", "Done", "Failed_RetriesExhausted", "Failed_DefiniteFailure"}]
      BY DEF IndInv1, TypeOK
    <3>1. CASE rPc' = [rPc EXCEPT ![r] = "Done"]
      BY <3>1, <3>rng, ExceptType
    <3>2. CASE rPc' = [rPc EXCEPT ![r] = "ReadPtr"]
      BY <3>2, <3>rng, ExceptType
    <3>3. CASE rPc' = [rPc EXCEPT ![r] = "Failed_RetriesExhausted"]
      BY <3>3, <3>rng, ExceptType
    <3>4. CASE rPc' = [rPc EXCEPT ![r] = "Failed_DefiniteFailure"]
      BY <3>4, <3>rng, ExceptType
    <3>5. QED
      BY <3>1, <3>2, <3>3, <3>4, <1>rpcdisj
  <2>e. \A r0 \in Readers : rLocal'[r0].retries \in Nat /\ rLocal'[r0].ptrVersion \in Nat
             /\ (rLocal'[r0].result = NoResult \/ rLocal'[r0].result = NoCommitsYet \/ rLocal'[r0].result \in SnapshotRec)
    <3> SUFFICES ASSUME NEW r0 \in Readers
                 PROVE  rLocal'[r0].retries \in Nat /\ rLocal'[r0].ptrVersion \in Nat
                        /\ (rLocal'[r0].result = NoResult \/ rLocal'[r0].result = NoCommitsYet \/ rLocal'[r0].result \in SnapshotRec)
      OBVIOUS
    <3>1. CASE r0 = r
      BY <3>1, <1>rlatr DEF IndInv1, TypeOK
    <3>2. CASE r0 # r
      <4>a. rLocal'[r0] = rLocal[r0]
        <5>1. CASE rLocal' = [rLocal EXCEPT ![r].result = snapshots[rLocal[r].ptrVersion]]
          BY <5>1, <3>2, ExceptOther
        <5>2. CASE rLocal' = [rLocal EXCEPT ![r].retries = @ + 1]
          BY <5>2, <3>2, ExceptOther
        <5>3. CASE rLocal' = rLocal
          BY <5>3
        <5>4. QED
          BY <5>1, <5>2, <5>3, <1>disj
      <4>b. QED
        BY <4>a DEF IndInv1, TypeOK
    <3>3. QED
      BY <3>1, <3>2
  <2>f. QED
    BY <2>a, <2>b, <2>c, <2>d, <2>e DEF TypeOK
<1>2. WriterSuccessIsCommitted'
  BY <1>unch DEF IndInv1, WriterSuccessIsCommitted
<1>3. ReaderSeesOnlyCommitted'
  <2> SUFFICES ASSUME NEW r0 \in Readers, rPc'[r0] = "Done", rLocal'[r0].result # NoCommitsYet
               PROVE  \E i \in 1..Len(snapshots') : snapshots'[i] = rLocal'[r0].result
    BY DEF ReaderSeesOnlyCommitted
  <2>1. CASE r0 = r
    <3>1. CASE /\ rPc' = [rPc EXCEPT ![r] = "Done"]
               /\ rLocal' = [rLocal EXCEPT ![r].result = snapshots[rLocal[r].ptrVersion]]
               /\ UNCHANGED <<snapshots, wPc, wLocal>>
      <4>a. rLocal'[r].result = snapshots[rLocal[r].ptrVersion]
        BY <3>1, <1>domr, ExceptResultAt
      <4>b. rLocal[r].ptrVersion \in 1..Len(snapshots')
        BY <1>rinsnap, <1>unch
      <4>c. snapshots'[rLocal[r].ptrVersion] = snapshots[rLocal[r].ptrVersion]
        BY <1>unch
      <4>d. snapshots'[rLocal[r].ptrVersion] = rLocal'[r0].result
        BY <4>a, <4>c, <2>1
      <4>e. QED
        BY <4>b, <4>d
    <3>2. CASE /\ rLocal[r].retries < ReaderRetryLimit
               /\ rPc' = [rPc EXCEPT ![r] = "ReadPtr"]
               /\ rLocal' = [rLocal EXCEPT ![r].retries = @ + 1]
               /\ UNCHANGED <<snapshots, wPc, wLocal>>
      <4>a. rPc'[r] = "ReadPtr"
        BY <3>2, <1>domp, ExceptSame
      <4>b. QED
        BY <2>1, <4>a
    <3>3. CASE /\ rLocal[r].retries >= ReaderRetryLimit
               /\ rPc' = [rPc EXCEPT ![r] = "Failed_RetriesExhausted"]
               /\ UNCHANGED <<snapshots, wPc, wLocal, rLocal>>
      <4>a. rPc'[r] = "Failed_RetriesExhausted"
        BY <3>3, <1>domp, ExceptSame
      <4>b. QED
        BY <2>1, <4>a
    <3>4. CASE /\ rPc' = [rPc EXCEPT ![r] = "Failed_DefiniteFailure"]
               /\ UNCHANGED <<snapshots, wPc, wLocal, rLocal>>
      <4>a. rPc'[r] = "Failed_DefiniteFailure"
        BY <3>4, <1>domp, ExceptSame
      <4>b. QED
        BY <2>1, <4>a
    <3>5. QED
      BY <3>1, <3>2, <3>3, <3>4, <1>disj
  <2>2. CASE r0 # r
    <3>a. rPc'[r0] = rPc[r0]
      <4>1. CASE rPc' = [rPc EXCEPT ![r] = "Done"]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE rPc' = [rPc EXCEPT ![r] = "ReadPtr"]
        BY <4>2, <2>2, ExceptOther
      <4>3. CASE rPc' = [rPc EXCEPT ![r] = "Failed_RetriesExhausted"]
        BY <4>3, <2>2, ExceptOther
      <4>4. CASE rPc' = [rPc EXCEPT ![r] = "Failed_DefiniteFailure"]
        BY <4>4, <2>2, ExceptOther
      <4>5. QED
        BY <4>1, <4>2, <4>3, <4>4, <1>rpcdisj
    <3>b. rLocal'[r0] = rLocal[r0]
      <4>1. CASE rLocal' = [rLocal EXCEPT ![r].result = snapshots[rLocal[r].ptrVersion]]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE rLocal' = [rLocal EXCEPT ![r].retries = @ + 1]
        BY <4>2, <2>2, ExceptOther
      <4>3. CASE rLocal' = rLocal
        BY <4>3
      <4>4. QED
        BY <4>1, <4>2, <4>3, <1>disj
    <3>c. rPc[r0] = "Done" /\ rLocal[r0].result # NoCommitsYet
      BY <2>2, <3>a, <3>b
    <3>d. \E i \in 1..Len(snapshots) : snapshots[i] = rLocal[r0].result
      BY <3>c DEF IndInv1, ReaderSeesOnlyCommitted
    <3>e. QED
      <4> PICK i \in 1..Len(snapshots) : snapshots[i] = rLocal[r0].result
        BY <3>d
      <4>i1. i \in 1..Len(snapshots')
        BY <1>unch
      <4>i2. snapshots'[i] = rLocal'[r0].result
        BY <1>unch, <3>b
      <4>qed. QED
        BY <4>i1, <4>i2
  <2>3. QED
    BY <2>1, <2>2
<1>4. FnDomains'
  <2>a. DOMAIN wLocal' = Writers
    BY <1>unch DEF IndInv1, FnDomains
  <2>b. DOMAIN rLocal' = Readers
    <3>1. CASE rLocal' = [rLocal EXCEPT ![r].result = snapshots[rLocal[r].ptrVersion]]
      BY <3>1, <1>domr, ExceptDomain
    <3>2. CASE rLocal' = [rLocal EXCEPT ![r].retries = @ + 1]
      BY <3>2, <1>domr, ExceptDomain
    <3>3. CASE rLocal' = rLocal
      BY <3>3, <1>domr
    <3>4. QED
      BY <3>1, <3>2, <3>3, <1>disj
  <2>c. QED
    BY <2>a, <2>b DEF FnDomains
<1>5. BaseVersionBounded'
  BY <1>unch DEF IndInv1, BaseVersionBounded
<1>6. ProposedIsReal'
  BY <1>unch DEF IndInv1, ProposedIsReal
<1>7. PtrVersionBounded'
  <2> SUFFICES ASSUME NEW r0 \in Readers, rPc'[r0] = "ReadSnap"
               PROVE  rLocal'[r0].ptrVersion \in 1..Len(snapshots')
    BY DEF PtrVersionBounded
  <2>1. CASE r0 = r
    <3>1. CASE /\ rPc' = [rPc EXCEPT ![r] = "Done"]
               /\ rLocal' = [rLocal EXCEPT ![r].result = snapshots[rLocal[r].ptrVersion]]
               /\ UNCHANGED <<snapshots, wPc, wLocal>>
      <4>a. rPc'[r] = "Done"
        BY <3>1, <1>domp, ExceptSame
      <4>b. QED
        BY <2>1, <4>a
    <3>2. CASE /\ rLocal[r].retries < ReaderRetryLimit
               /\ rPc' = [rPc EXCEPT ![r] = "ReadPtr"]
               /\ rLocal' = [rLocal EXCEPT ![r].retries = @ + 1]
               /\ UNCHANGED <<snapshots, wPc, wLocal>>
      <4>a. rPc'[r] = "ReadPtr"
        BY <3>2, <1>domp, ExceptSame
      <4>b. QED
        BY <2>1, <4>a
    <3>3. CASE /\ rLocal[r].retries >= ReaderRetryLimit
               /\ rPc' = [rPc EXCEPT ![r] = "Failed_RetriesExhausted"]
               /\ UNCHANGED <<snapshots, wPc, wLocal, rLocal>>
      <4>a. rPc'[r] = "Failed_RetriesExhausted"
        BY <3>3, <1>domp, ExceptSame
      <4>b. QED
        BY <2>1, <4>a
    <3>4. CASE /\ rPc' = [rPc EXCEPT ![r] = "Failed_DefiniteFailure"]
               /\ UNCHANGED <<snapshots, wPc, wLocal, rLocal>>
      <4>a. rPc'[r] = "Failed_DefiniteFailure"
        BY <3>4, <1>domp, ExceptSame
      <4>b. QED
        BY <2>1, <4>a
    <3>5. QED
      BY <3>1, <3>2, <3>3, <3>4, <1>disj
  <2>2. CASE r0 # r
    <3>a. rPc'[r0] = rPc[r0]
      <4>1. CASE rPc' = [rPc EXCEPT ![r] = "Done"]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE rPc' = [rPc EXCEPT ![r] = "ReadPtr"]
        BY <4>2, <2>2, ExceptOther
      <4>3. CASE rPc' = [rPc EXCEPT ![r] = "Failed_RetriesExhausted"]
        BY <4>3, <2>2, ExceptOther
      <4>4. CASE rPc' = [rPc EXCEPT ![r] = "Failed_DefiniteFailure"]
        BY <4>4, <2>2, ExceptOther
      <4>5. QED
        BY <4>1, <4>2, <4>3, <4>4, <1>rpcdisj
    <3>b. rLocal'[r0] = rLocal[r0]
      <4>1. CASE rLocal' = [rLocal EXCEPT ![r].result = snapshots[rLocal[r].ptrVersion]]
        BY <4>1, <2>2, ExceptOther
      <4>2. CASE rLocal' = [rLocal EXCEPT ![r].retries = @ + 1]
        BY <4>2, <2>2, ExceptOther
      <4>3. CASE rLocal' = rLocal
        BY <4>3
      <4>4. QED
        BY <4>1, <4>2, <4>3, <1>disj
    <3>c. rPc[r0] = "ReadSnap"
      BY <2>2, <3>a
    <3>d. rLocal[r0].ptrVersion \in 1..Len(snapshots)
      BY <3>c DEF IndInv1, PtrVersionBounded
    <3>dnat. rLocal[r0].ptrVersion \in Nat
      BY <3>d
    <3>e. Len(snapshots) <= Len(snapshots')
      BY <1>unch
    <3>f. rLocal[r0].ptrVersion <= Len(snapshots')
      BY <3>d, <3>dnat, <3>e
    <3>g. rLocal[r0].ptrVersion >= 1
      BY <3>d
    <3>bp. rLocal'[r0].ptrVersion = rLocal[r0].ptrVersion
      BY <3>b
    <3>fp. rLocal'[r0].ptrVersion <= Len(snapshots')
      BY <3>bp, <3>f
    <3>gp. rLocal'[r0].ptrVersion >= 1
      BY <3>bp, <3>g
    <3>pnat. rLocal'[r0].ptrVersion \in Nat
      BY <3>bp, <3>dnat
    <3>h. QED
      BY <3>fp, <3>gp, <3>pnat
  <2>3. QED
    BY <2>1, <2>2
<1>8. QED
  BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7 DEF IndInv1

=============================================================================
