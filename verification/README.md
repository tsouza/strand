# verification/

A TLA+ model of the manifest CAS commit and read protocols (RFC 0001 §3,
`spec/manifest.md`), covering the action grammar RFC 0002 §4 approved.
Approved by RFC 0002 (`rfcs/0002-manifest-formal-verification.md`); this
directory now holds all three of that RFC's artifacts. The model and its
TLC-checked safety invariants (`manifest.tla`, `manifest.cfg`) exist; a
TLAPS mechanized proof (`manifest_proofs.tla`) exists too, covering all
five writer actions and both reader actions with the precise,
tlapm-confirmed scope stated in "TLAPS proof" below — read that section
before trusting any claim about what is proved; and the DST cross-validation harness
(Workflow II, `docs/roadmap.md` M3-3) exists, lives in
`crates/strand-core/src/bin/dst_manifest_harness/`, and is documented in
its own section near the end of this file.

## Running it

Requires Java 17+ and `tla2tools.jar` (MIT-licensed,
`github.com/tlaplus/tlaplus`). Fetch it if you don't already have a copy:

    curl -LO https://github.com/tlaplus/tlaplus/releases/latest/download/tla2tools.jar

Then, from the repository root:

    java -jar /path/to/tla2tools.jar -workers auto -config verification/manifest.cfg verification/manifest.tla

Expect `Model checking completed. No error has been found.` and exit code
`0`, with **5,943 distinct states** found (22,286 generated, search depth
18). That state count is the baseline: a future session scaling the model up
should expect it to move, and a change that leaves it identical has probably
not reached the state space it meant to. (Baseline was 591/1793, depth 14,
before RFC 0012's `commit_deletion_vector` extension added a third writer —
`DeleteWriter`, alongside the existing `DistinguishedWriter` — and a new
revise-in-place action, `ProposeDeletionVectorCommit`, closing the TLA+
model correspondence gap that RFC's own adversarial review found; before
that, baseline was 561/1487, before `ReadCurrent`'s `Expired` branch and
`ProposeSnapshot`'s failure branch were added, closing an earlier TLA+ model
correspondence gap `docs/ledger.md` recorded.) A parse-only check (no model
checking, just confirms the module is well-formed) is also available:

    java -cp /path/to/tla2tools.jar tla2sany.SANY verification/manifest.tla

## Invariants checked

Seven, all listed in `verification/manifest.cfg`, each carrying its
grounding in a comment beside it in `manifest.tla`: three
(`MonotonicNextRowId`, `VersionsMatchIndex`, `WriterSuccessIsCommitted`)
additionally record their mutation test inline — the others' mutation tests
live in the commit log (`9fcf963`, `d3d3139`), not in comments.

- `TypeOK` — every variable stays in its declared domain.
- `NoOverlappingRowIds` — no two committed segments claim overlapping
  row-ID ranges.
- `MonotonicNextRowId` — `next_row_id` never goes backwards across commits.
- `VersionsMatchIndex` — a snapshot's recorded version equals its position
  in committed history.
- `NextRowIdMatchesSegments` — `next_row_id` equals the summed row counts of
  the committed segments.
- `ReaderSeesOnlyCommitted` — a reader that finishes with a result reports a
  snapshot that is really in committed history.
- `WriterSuccessIsCommitted` — a writer that reports success really did
  commit its own proposed snapshot. This is the one that makes a lost update
  visible; without it nothing outside `TypeOK` reads writer state at all.

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

One abstraction in the writer path is worth knowing before reading the
model, because its shape invites the wrong reading. `ResolveAmbiguity` models
the *outcome* of resolving an ambiguous CAS — landed or not landed — rather
than the read-only recheck the real `commit` performs; the append is deferred
into its "landed" branch. This is deliberate and is about keeping the state
space finite: modeling it literally (append nondeterministically at the
ambiguous CAS, then read to resolve) lets a writer append, fail to observe
it, and append again without bound, which was measured during review at over
4M states and still growing. The cost is that one real scenario — a writer's
write lands, a rival builds on it, and the writer then retries and commits a
duplicate — is unreachable here. Bounding the literal model properly is
follow-on work, not a defect to patch in place. `TryAdvancePointer` carries a
smaller, safety-neutral narrowing of the same kind, documented at the action.

## Model size

`verification/manifest.cfg` uses a small, fast-checking configuration (2
writers, 1 reader, `DistinguishedWriter` claiming 1 row per segment and the
other writer claiming 2, `ReaderRetryLimit` 2). That retry limit is the one
constant whose value visibly diverges from its real counterpart:
`manifest.rs`'s `READER_REFRESH_RETRY_LIMIT` is 5, and the model mirrors the
shape of a bounded retry, not the number — nothing checked here depends on
the exact bound, only on its being finite. The rest is deliberate too, not an
unexamined default: an
adversarial review of this model's design confirmed every guard in this
protocol is a single boolean comparison with no quorum/threshold logic
depending on rival *count* (unlike Raft), and readers never interact with
each other, so a 3rd writer or 2nd reader adds interleaving volume, not new
guard combinations to check. Scaling the model up regardless is a
deliberate follow-on, not done here.

## DST cross-validation harness (Workflow II)

`crates/strand-core/src/bin/dst_manifest_harness/` — a new `dst-manifest-
harness` binary target in `crates/strand-core/Cargo.toml`. Implements RFC
0002 §2's Workflow II (spec drives implementation): TLC generates real
action sequences from `manifest.tla` above, and the harness replays each
directly against the real `commit`/`commit_deletion_vector`/
`read_snapshot` in `crates/strand-core/src/manifest.rs`, checking the real
outcome against what the trace predicted. Workflow I (implementation
drives verification — the real code's own spontaneous trace checked
against the spec after the fact) is explicitly sequenced after Workflow II
succeeds (RFC 0002 §2) and is not built here.

**Running it**:

```
cargo run -p strand-core --bin dst-manifest-harness -- \
  --seed 20260819 --workers 1 --num-per-worker 1000 --depth 40
```

Requires the same `tla2tools.jar` this directory's model-checking section
above does; pass `--jar PATH` if it isn't at `~/.cache/tlaplus/
tla2tools.jar`. `--traces-dir DIR` replays an already-generated directory
instead of invoking TLC again (useful for re-inspecting one run's traces
without regenerating them). The binary prints the exact `java` command
line it runs, so a report is reproducible by copying that line back out.

**Mechanism**: `-simulate num=N,file=PREFIX` (TLC's real random-simulation
trace-file mode, checked live against `tla2tools.jar -help`'s actual flag
list rather than assumed — `-dump` writes the whole reachable graph, not a
sequence; the heavier TLA+ Trace Validation / `TLCTrace` tooling this
project's own RFC 0002 Open Questions once wondered whether it would be
needed is built for Workflow I's observe-and-reconcile shape, not this
one). Each trace is a numbered sequence of full model states, not action
labels, so the harness reconstructs which action fired between two
consecutive states by diffing writer/reader process-counter variables
(`crates/strand-core/src/bin/dst_manifest_harness/replay.rs`'s
`diff_states`) — sound because `Next` never steps two processes in one
transition and every `(from_pc, to_pc)` pair in RFC 0002 §4's grammar is
unique. `ReadCurrent`'s `Expired` self-loop is invisible to this scheme by
construction (it changes no state), which is correct: nothing observable
happened for a real replay to check.

**Replay design**: full rationale, including why writers are replayed in
their own terminal-step order against one shared real `InMemoryStore`
(real staleness then emerges from a real ETag comparison, never injected)
and the named scope limit (sub-cycle rival interleaving beyond the
`Ambiguous` landed/not-landed axis is not reproduced), lives in
`replay.rs`'s own module doc — deliberately not duplicated here.

**The recorded run** (seed 20260819, the harness's own default,
`-workers 1`, 1,000 traces, `-depth 40`): 1,000 trace files parsed clean,
8,396 total states. 3,000 writer trajectories replayed — 2,511 matched,
489 skipped (never reached a terminal pc within this trace's depth,
reported honestly rather than counted as pass or fail), **0 drift**. 1,000
reader trajectories replayed — 1,000 matched, 0 skipped, **0 drift**.
2,335/3,000 writer and 501/1,000 reader trajectories required injecting at
least one non-trivial fault outcome (`Io`/`Ambiguous`/reader-side
`Expired`) to reach their predicted terminal, so most of the corpus
exercised RFC 0002 §4's fault branches, not only the uncontended happy
path. Determinism confirmed directly, not assumed: re-running the same
seed (111, `-workers 1`, `num=100`, `-depth 25`) twice produced
byte-identical trace files and an identical report both times.

Zero drift is real, positive evidence that the trace vocabulary and RFC
0002 §4's action granularity correspond to this real code across a
non-trivial, TLC-generated sample of the state space — not proof the
model itself is complete (an unmodeled action class is invisible to this
scheme in exactly the way an unmodeled fault is invisible to a `proptest`
generator, the second adversarial review's own caveat), not a liveness
check, and not, by itself, evidence Workflow I will succeed once attempted
(RFC 0002 §2's own distinction: driven replay working is not the same
claim as a spontaneous trace decomposing the same way). Full account,
including two real bugs found and fixed in the harness itself during
construction (neither a manifest protocol bug), in RFC 0002's Discussion
section and `docs/roadmap.md`'s M3-3 entry.

Automatic classification of any future drift into RFC 0002 §3's four
types (Type-I / Type-II / tracer artifact / fault-model mismatch) is not
implemented — the harness reports which writer/reader, which trace file,
and the trace-predicted outcome versus the real one, which is enough
detail for a human to classify by hand, per §3's own text that the
classification is a design-intent question, not a mechanical one.

## TLAPS proof

`manifest_proofs.tla` (new `EXTENDS manifest`, kept as its own module so
proof-engineering iteration never touches the TLC-checked `manifest.tla`
itself) is RFC 0002's second artifact: a machine-checked proof, not just a
bounded model check. `docs/roadmap.md`'s M3-2 entry tracks this work.

### Installing TLAPS

TLAPS (`tlapm`) is a separate toolchain from TLC (`tla2tools.jar`) and is
not installed by default:

    curl -sL -o /tmp/tlaps-installer.bin "https://github.com/tlaplus/tlapm/releases/download/202210041448/tlaps-1.5.0-x86_64-linux-gnu-inst.bin"
    chmod +x /tmp/tlaps-installer.bin
    /tmp/tlaps-installer.bin -d ~/tlaps
    export PATH="$HOME/tlaps/bin:$PATH"
    tlapm --version

The installer compiles Isabelle theories and runs a self-test; expect it to
take a minute or two. Version used for every proof in this directory: TLAPS
1.5.0 (bundled Isabelle2011-1, zenon 0.8.4, z3 4.8.9 — the three backend
provers a bare `BY` step tries in sequence).

### Running it

From the repository root, with `tlapm` on `PATH`:

    tlapm -I verification verification/manifest_proofs.tla

`-I verification` puts the directory on tlapm's search path so it finds
`manifest.tla` locally. Expect `[INFO]: All N obligations proved.` for
every `THEOREM`/`LEMMA` and exit code 0. There is no partial-success signal
at the process level — one failed obligation makes the whole run exit
non-zero and print the specific unproved goal — which is why the scope
accounting below is obligation-count-based, taken from tlapm's own final
summary line, never inferred from the file merely containing a theorem.

A real toolchain fix was required before any proof could be attempted:
TLAPS 1.5.0 cannot process **any** module that `EXTENDS manifest` as the
model stood before this work, because tlapm's level-checker (`e_levels.ml`)
asserts on `RECURSIVE`-operator definitions ("Error: Recursive operator
definitions are not supported"), and `manifest.tla` had exactly one,
`SumCounts` (used by `NextRowIdMatchesSegments`). Confirmed by running
`tlapm` against an empty module extending `manifest` and watching it abort
on `manifest.tla`'s own line, before anything in the new file was even
reached. Fixed by rewriting `SumCounts` from the `RECURSIVE`-operator form
to TLA+'s native recursive-*function* syntax
(`SumCounts[segs \in Seq(SegmentRec)] == ...`), which TLAPS handles without
complaint. This is the one change this work made to `manifest.tla` itself,
and it is semantics-preserving, not a model change: both forms are
standard TLA+, and TLC model-checks them identically — re-run after the
change confirmed the exact same 5,943 distinct states (22,286 generated,
depth 18) as the baseline above.

### What is actually proved

Independently confirmed by running the exact `tlapm` invocation above and
reading its own final summary line — never assumed from a `THEOREM`'s mere
presence in the file. As of this file's most recent `tlapm` run:
**`[INFO]: All 2018 obligations proved.`, exit code 0, on two separate
runs producing the identical result: one ordinary run, one with
`--cleanfp` (fingerprint cache erased first, so nothing was reused from
the ordinary run)** — the determinism check this file's own honesty
discipline calls for, not a single run taken on faith (`manifest_proofs.tla`,
2,090 lines: 10 generic reusable lemmas plus 8 theorems). An earlier
version of this file reported 1,247 obligations from a run that did not,
in fact, reproduce: an independent adversarial review re-ran `tlapm` fresh
against that exact commit and got `[ERROR]: 1/1247 obligations failed`
instead, isolating the failure to `ProposeDeletionVectorCommitStep1`'s
`<3>eq` step. The fix (`ExceptSegmentDelVer`, described in "Lessons,"
below) replaced that step entirely rather than patching it, and the
1,261-obligation count that followed was independently reproduced twice
before being written down. The current 2,018-obligation count is the
five-writer-plus-`Init` 1,261 count plus the two reader-action theorems
added in this pass (`ReadPointerStep1`, `ReadSnapshotObjectStep1`) plus
one additional inductive conjunct (`PtrVersionBounded`, below) whose
preservation had to be re-proved across every one of the other six
theorems too.

`IndInv1 == TypeOK /\ WriterSuccessIsCommitted /\ ReaderSeesOnlyCommitted
/\ FnDomains /\ BaseVersionBounded /\ ProposedIsReal /\ PtrVersionBounded`
is proved to be a genuine inductive invariant of **all five writer-path
actions and both reader-path actions** — `ReadCurrent`, `ProposeSnapshot`,
`ProposeDeletionVectorCommit`, `TryAdvancePointer`, `ResolveAmbiguity`
(matching `commit()`'s and `commit_deletion_vector()`'s real control flow),
plus `ReadPointer` and `ReadSnapshotObject` (matching `read_snapshot()`'s
retry loop and its `try_read_current()` helper's real control flow) —
plus `Init`:

- `THEOREM Init1 == Init => IndInv1`
- `THEOREM <Action>Step1 == ASSUME IndInv1, NEW w \in Writers, <Action>(w)
  PROVE IndInv1'`, one such theorem per writer action above.
- `THEOREM <Action>Step1 == ASSUME IndInv1, NEW r \in Readers, <Action>(r)
  PROVE IndInv1'`, one such theorem per reader action above.

Four of `IndInv1`'s seven conjuncts are not part of `manifest.cfg`'s own
seven TLC-checked invariants — they were found necessary *during* the
proof, each the same shape (a fact true by construction that still has to
be stated as its own explicit inductive conjunct before TLAPS can use it,
discovered by trying to type-check an obligation without it and watching
tlapm reject the gap): `FnDomains` (`DOMAIN wLocal = Writers /\
DOMAIN rLocal = Readers` — not implied by `TypeOK` as written, unlike
`wPc`/`rPc`'s domains, which follow from their `\in [Writers -> ...]`
conjuncts), `BaseVersionBounded` (`wLocal[w].baseVersion <= Len(snapshots)`
for every writer — needed to type-check indexing `snapshots` at a writer's
cached base version), `ProposedIsReal` (a writer whose `wPc` has
reached `Advance`/`ResolveAmbiguity`/`Done` always has a real `SnapshotRec`,
not `NoProposal`, staged in `wLocal[w].proposed` — needed to type-check the
`Append` that lands a writer's commit), and `PtrVersionBounded`
(`rPc[r] = "ReadSnap" => rLocal[r].ptrVersion \in 1..Len(snapshots)` for
every reader — the reader-side counterpart of `BaseVersionBounded`, added
in this pass and needed for the same reason: `ReadSnapshotObject`'s Found
branch indexes `snapshots[rLocal[r].ptrVersion]`, and without a bound
relating a reader's cached `ptrVersion` to `snapshots`'s current length,
neither that index nor the existential witness `ReaderSeesOnlyCommitted'`
needs once the reader reaches "Done" type-check). Adding
`PtrVersionBounded` meant re-proving its preservation across all seven
actions, not just the two new reader ones — trivial for `ReadCurrent`,
`ProposeSnapshot`, and `ProposeDeletionVectorCommit` (none of which touch
`rPc`/`rLocal`/`snapshots` at all in any branch), and needing the
`Len(snapshots) <= Len(snapshots')` fact already established for
`BaseVersionBounded'` in `TryAdvancePointerStep1`/`ResolveAmbiguityStep1`.
None of the four contradicts or weakens the seven TLC-checked invariants;
they are additional facts about the same state, proved alongside them.

What this combination actually establishes: for each of the five writer
actions and both reader actions, `IndInv1` holds after the action given it
held before. Chained with `Init1`, this is a real inductive-invariant
proof — not a bounded check — that across any sequence of these seven
actions, `TypeOK` never breaks, a writer that reports success really did
commit its own proposed snapshot (`WriterSuccessIsCommitted` — the
property RFC 0002's own Motivation names as "lose a writer's data"), and a
reader that finishes with a result never reports a snapshot that was never
really committed (`ReaderSeesOnlyCommitted` — now proved for the reader
actions that actually produce that result, not only inherited unchanged
through writer actions that never touch reader state).

### What is explicitly not yet proved

Stated plainly so a `THEOREM` name already in the file is never mistaken
for finished scope:

- **The `Next`-level composition and the temporal invariance theorem**
  (`Spec => []IndInv1` via TLAPS's `PTL` backend, combining `Init1` with
  every action's step lemma into one property that holds at every
  reachable state of an actual run) — not attempted. Today's eight
  theorems are seven independent "one action preserves `IndInv1`" facts,
  not yet assembled into that single statement.
- **The model's other six TLC-checked invariants** — `NoOverlappingRowIds`,
  `MonotonicNextRowId`, `VersionsMatchIndex`, `NextRowIdMatchesSegments`,
  `SegmentCountNeverDecreases`, `DeletionVectorCommitsOnlyReviseOneEntry` —
  have no TLAPS proof at all. `NoOverlappingRowIds` specifically — the
  invariant most directly answering "no two writers' CAS-raced commits
  silently overlap row-ID ranges," arguably the more central of the two
  properties RFC 0002's Motivation names — needs a materially harder
  inductive strengthening (proving segments are packed contiguously,
  by induction over `SumCounts`'s recursive structure) that this effort
  did not attempt. `rfcs/0002-manifest-formal-verification.md`'s
  Discussion section and `docs/ledger.md` carry the full accounting.
- **The DST cross-validation harness** — RFC 0002's third artifact, is
  done (`docs/roadmap.md` M3-3), not part of the TLAPS proof scope this
  section describes; see the top of this file and the "DST
  cross-validation harness" section below.

### Lessons for extending this proof (read before adding a theorem)

Found the hard way, across many `tlapm` runs, and worth stating so the next
session doesn't rediscover them at the same cost:

- **zenon needs domain facts spelled out.** `[f EXCEPT ![x]=v][x]=v`
  (the "own index" EXCEPT-projection) genuinely requires `x \in DOMAIN f`
  present as an explicit hypothesis in scope, cited in the same `BY` — the
  "other index unchanged" identity (`x # y => [f EXCEPT![x]=v][y]=f[y]`)
  needs no such fact. `ExceptSame`/`ExceptOther`/`ExceptType`/
  `ExceptDomain` are the four reusable lemmas this file leans on instead of
  re-deriving this every time.
- **A goal combining a disjunction + `LET`/`IF` + `EXCEPT` reliably
  defeats every backend.** Always extract the literal (LET-inlined,
  non-existential) disjunction as its own named step first, cited via
  `BY DEF <Action>`, then `CASE`-split on each full branch (the branch's
  *entire* conjunction, not just the one conjunct a later step happens to
  need — citing a partial branch is a genuine unprovable gap, confirmed the
  hard way in `WriterSuccessIsCommitted`'s `TryAdvancePointer` case, not
  mere flakiness).
- **Repeating a large subexpression inline, several times, in one
  obligation measurably worsens backend reliability**, independent of the
  logic being simple. `<1>revok`'s inner proof (`ProposeDeletionVectorCommitStep1`)
  reliably defeated every backend — confirmed across five separate `tlapm`
  runs, several with a single z3 or zenon subprocess spinning for 10+
  minutes on one obligation. Naming the repeated `IF...THEN...ELSE...` once
  via a local `<2> DEFINE priorSeg1 == ...` abbreviation (standard TLAPS
  practice) fixed half the problem. The other half needed a real second
  fix, found only after the first "fix" was independently re-tested and
  found not to reliably hold: an earlier version of this file additionally
  proved the target EXCEPT expression equal to a literal record
  (`[base|->.., count|->.., delVer|->..]`) before checking *that* literal's
  set membership — this route *sometimes* discharged but was not reliable
  across fresh, cache-cleared runs with the documented backend versions
  (reproduced failing 2/2 by an independent review). The fix that actually
  holds up under repeated fresh runs sidesteps literal-equality entirely:
  `ExceptSegmentDelVer`, a small reusable lemma proving `[r EXCEPT
  !.delVer = v] \in SegmentRec` field-by-field (`DOMAIN`, then each field,
  via `ExceptSame`/`ExceptOther`/`ExceptDomain`, the same shape
  `ExceptProposedAt` above already uses for `SnapshotRec`'s `proposed`
  field), cited directly at the call site instead of reconstructing a
  literal record. The general lesson: for an EXCEPT-membership goal, prefer
  proving membership directly, field by field, over proving equality to a
  literal record first — the literal-equality route is a plausible-looking
  shortcut that measurably does not hold up under a real, repeated,
  cache-cleared `tlapm` run.
- **Existential goals built by combining "a witness exists in the OLD
  state" with "the old state's contents are preserved into the new
  state"** (the `\E i \in 1..Len(s') : s'[i] = v` shape that recurs in
  `WriterSuccessIsCommitted` and `ReaderSeesOnlyCommitted`) reliably
  defeat a single combined `BY` citation. Use `<n> PICK i \in S : P(i)`
  to name the witness explicitly, then prove the new range membership and
  the new value at that witness as two separate small facts.
- **`<=`-transitivity through a record-field-and-index expression** (e.g.
  `wLocal[w0].baseVersion <= Len(s) <= Len(s')`) reliably fails via a bare
  `BY` even though the identical shape with a plain `NEW x \in Nat`
  hypothesis proves instantly with `OBVIOUS` — confirmed in isolation.
  State the field expression's own `\in Nat` membership as an explicit
  local fact before citing it in the chain, every time, even when it
  "obviously" follows from an already-cited invariant.
- **Orphaned backend worker subprocesses do not always get reaped** when a
  `tlapm` run is killed or completes with failures, and compete for CPU
  with a later run. `pkill -9 -f zenon`, `pkill -9 -f 'z3 -smt2'`,
  `pkill -9 -f isabelle-process`, `pkill -9 -f 'poly -q'` before a fresh
  run is worth doing if a run seems implausibly slow.
- **Proving interval membership (`x \in a..b`) for a PRIMED expression from
  facts stated about the corresponding UNPRIMED expression plus a separate
  equality reliably fails**, even when each fact type-checks alone and the
  identical shape works fine for a bare `<=` goal (the transitivity lesson
  above). Found extending `PtrVersionBounded` (this pass's reader-side
  counterpart of `BaseVersionBounded`) to every theorem whose action leaves
  `rLocal[r]`/`rPc[r]` unchanged for the reader `r` a goal is about:
  `TryAdvancePointerStep1` and `ResolveAmbiguityStep1` (writer actions —
  `rLocal`/`rPc` unchanged for every reader), and the `r0 # r` branch of
  `ReadPointerStep1`/`ReadSnapshotObjectStep1`'s own `PtrVersionBounded'`
  proof (reader actions — `rLocal`/`rPc` unchanged for every *other*
  reader). Citing an equality (e.g. `rLocal'[r].ptrVersion =
  rLocal[r].ptrVersion`) together with `<=` and `>=` facts about the
  unprimed value in one `BY` for a goal like `rLocal'[r].ptrVersion \in
  1..Len(snapshots')` failed for the writer-action pair on the first
  full-module `tlapm` run, after the reader-action pair's analogous branch
  had already needed (and received) the same fix during earlier
  `--toolbox`-scoped iteration — caught before either of the two full,
  cache-cleared confirmation runs this file's own honesty discipline
  requires, not by one of them. The fix, applied in both places: restate
  the `<=` bound, the `>=` bound, and the value's own `\in Nat` membership
  about the PRIMED expression directly (three small additional steps,
  substituting through the equality once each) before the
  interval-membership `QED`, rather than leaving the substitution for the
  backend to perform inside the interval check itself.
