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
`0`, with **591 distinct states** found (1793 generated, search depth 14).
That state count is the baseline: a future session scaling the model up
should expect it to move, and a change that leaves it identical has probably
not reached the state space it meant to. (Baseline was 561/1487 before
`ReadCurrent`'s `Expired` branch and `ProposeSnapshot`'s failure branch were
added, closing the TLA+ model correspondence gap `docs/ledger.md` recorded;
`ProposeSnapshot`'s new failure path is what actually grew the reachable
state space — a self-loop alone, like `ReadCurrent`'s `Expired` branch,
cannot introduce a new distinct state.) A parse-only check (no model
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
