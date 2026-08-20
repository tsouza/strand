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

IndInv1 == TypeOK /\ WriterSuccessIsCommitted /\ ReaderSeesOnlyCommitted /\ FnDomains /\ BaseVersionBounded

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
<1>6. QED
  BY <1>1, <1>2, <1>3, <1>4, <1>5 DEF IndInv1

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
<1>6. QED
  BY <1>1, <1>2, <1>3, <1>4, <1>5 DEF IndInv1

=============================================================================
