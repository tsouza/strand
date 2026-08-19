# Apache Iceberg — Snapshot Expiration Retention Properties

Vendored excerpt, fetched 2026-08-19 to ground `docs/roadmap.md` M3-4's
count-vs-duration retention-combination decision (`rfcs/0001-container-rowid-
manifest.md` Discussion) against Iceberg's own documented, equivalent knobs,
rather than asserting the match from memory.

**Source:** `raw.githubusercontent.com/apache/iceberg/main/docs/docs/
configuration.md` (table properties) and `raw.githubusercontent.com/apache/
iceberg/main/docs/docs/spark-procedures.md` (the `expire_snapshots` procedure
that applies them). License: Apache-2.0 (already recorded for this repository
in `references/iceberg-commit-conflict-resolution-and-retry.md`).

## Table properties (`configuration.md`, "Table behavior properties")

| property | default | description |
| --- | --- | --- |
| `history.expire.max-snapshot-age-ms` | `432000000` (5 days) | Default max age of snapshots to keep on the table and all of its branches while expiring snapshots |
| `history.expire.min-snapshots-to-keep` | `1` | Default min number of snapshots to keep on the table and all of its branches while expiring snapshots |
| `history.expire.max-ref-age-ms` | `Long.MAX_VALUE` (forever) | For snapshot references except the `main` branch, default max age of snapshot references to keep while expiring snapshots. The `main` branch never expires. |

## The `expire_snapshots` procedure (`spark-procedures.md`)

> "`older_than` — Timestamp before which snapshots will be removed (Default: 5
> days ago)"
>
> "`retain_last` — Number of ancestor snapshots to preserve **regardless of**
> `older_than` (defaults to 1)"
>
> "If `older_than` and `retain_last` are omitted, the table's [expiration
> properties](configuration.md#table-behavior-properties) will be used."

## What this confirms

"Regardless of" is Iceberg's own wording for how its count-based knob
(`retain_last` / `min-snapshots-to-keep`) interacts with its age-based knob
(`older_than` / `max-snapshot-age-ms`): the last N snapshots are preserved
**even if** they are older than the age cutoff would otherwise allow — a
union, not an intersection, of what each knob alone would retain. This is the
real, documented precedent STRAND's own retention-eligibility decision (keep
a snapshot if it satisfies *either* the count or the duration bound) matches,
not an assumed analogy.
