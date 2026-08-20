# verification/

A TLA+ model of the manifest CAS commit and read protocols (RFC 0001 §3,
`spec/manifest.md`), covering the action grammar RFC 0002 §4 approved.
Approved by RFC 0002 (`rfcs/0002-manifest-formal-verification.md`); this
directory is the first of that RFC's three artifacts (TLA+ model, TLAPS
proof, DST harness). The model and its TLC-checked safety invariants exist
and are documented below; the DST cross-validation harness (Workflow II,
`docs/roadmap.md` M3-3) also now exists, lives in
`crates/strand-core/src/bin/dst_manifest_harness/`, and is documented in
its own section near the end of this file. The TLAPS proof (M3-2) does
not exist yet.

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
