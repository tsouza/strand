# Apache Iceberg — `remove_orphan_files` Procedure

Vendored excerpt, fetched 2026-08-19 to ground the M3-5 orphan-sweep tool's
resolution of a real spec gap: `spec/manifest.md`'s "Orphan files" rule (and
`CLAUDE.md` §6) says orphans are deleted only when "older than the retention
window" but never says which window — `RetentionPolicy` (`min_snapshots_to_keep`
/ `max_snapshot_age_millis`) is defined only for *snapshot* expiry, not for
raw, possibly-unreferenced objects with no snapshot of their own. Fetched to
check whether a real prior-art system treats an orphan-file sweep's safety
margin as the same knob as snapshot retention, or as its own separate
parameter, rather than assuming the answer from memory (`CLAUDE.md` §3).

**Source:** `raw.githubusercontent.com/apache/iceberg/main/docs/docs/
spark-procedures.md`, the `remove_orphan_files` procedure section. License:
Apache-2.0 (already recorded for this repository in
`references/iceberg-commit-conflict-resolution-and-retry.md`).

## Description

"The procedure removes files that are not referenced in any metadata files
of an Iceberg table and can thus be considered 'orphaned'."

## Key parameters

- `older_than: timestamp` — "Remove orphan files created before this
  timestamp." **Default: 3 days ago.**
- `max_concurrent_deletes: int` — size of the thread pool used for delete
  actions; no thread pool by default.
- Also: `dry_run` (boolean), `location` (string, directory to scan),
  `stream_results` (boolean).

## Relationship to `expire_snapshots`

`expire_snapshots` is the procedure that applies `RetentionPolicy`'s own
equivalent knobs (`history.expire.max-snapshot-age-ms`,
`history.expire.min-snapshots-to-keep` — see
`references/iceberg-snapshot-expiration-retention-properties.md`) to decide
which *snapshots* remain live. `remove_orphan_files` is a **separate
procedure** with its **own separate `older_than` parameter and its own
separate default (3 days)** — it does not read or derive from
`expire_snapshots`'s retention properties at all. Iceberg's own docs state
the distinction directly: "Unlike `expire_snapshots`, which targets old
snapshots and their associated data files, `remove_orphan_files`
specifically identifies and removes files that have become disconnected
from table metadata entirely."

## What this confirms

The orphan-sweep safety window and the snapshot-retention policy are two
different knobs in real prior art, not one knob reused for both purposes —
confirming STRAND's own resolution (`rfcs/0001-container-rowid-manifest.md`
Discussion, M3-5): the orphan sweep's retention window is a parameter
supplied to the sweep itself, not a field read out of `TableMetadata`'s
`RetentionPolicy`.
