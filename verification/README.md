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
