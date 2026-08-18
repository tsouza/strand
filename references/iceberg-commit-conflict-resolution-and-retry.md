# Apache Iceberg — Commit Conflict Resolution and Retry

Vendored excerpt, not the full spec. Source: `apache/iceberg`,
`format/spec.md`, sections "Optimistic Concurrency" (added 2026-08-18 in
a second fetch — the whole-project convergence audit found RFC 0001
quoting this section's sentences while only the two sections below had
been vendored), "Commit Conflict Resolution and Retry", and its
"Metastore Tables" subsection. Both fetches 2026-08-18 from
`https://raw.githubusercontent.com/apache/iceberg/main/format/spec.md`.
License: Apache-2.0 (verified byte-level against the `apache/iceberg`
repository's `LICENSE` file, matching the license claim already recorded
in `docs/research/README.md` R11 for the same repository's code). Cited
by `rfcs/0001-container-rowid-manifest.md` and `spec/manifest.md` for
STRAND's manifest commit protocol, which adopts the same conceptual
shape: optimistic metadata creation, a CAS-guarded pointer, refresh-and-
retry on conflict, and — confirmed by this excerpt, not assumed — the
same version-plus-random-component snapshot filename pattern.

---

### Optimistic Concurrency

An atomic swap of one table metadata file for another provides the basis
for serializable isolation. Readers use the snapshot that was current
when they load the table metadata and are not affected by changes until
they refresh and pick up a new metadata location.

Writers create table metadata files optimistically, assuming that the
current version will not be changed before the writer's commit. Once a
writer has created an update, it commits by swapping the table's
metadata file pointer from the base version to the new version.

If the snapshot on which an update is based is no longer current, the
writer must retry the update based on the new current version. Some
operations support retry by re-applying metadata changes and committing,
under well-defined conditions. For example, a change that rewrites files
can be applied to a new table snapshot if all of the rewritten files are
still in the table.

### Commit Conflict Resolution and Retry

When two commits happen at the same time and are based on the same
version, only one commit will succeed. In most cases, the failed commit
can be applied to the new current version of table metadata and retried.
Updates verify the conditions under which they can be applied to a new
version and retry if those conditions are met.

- Append operations have no requirements and can always be applied.
- Replace operations must verify that the files that will be deleted are
  still in the table. Examples of replace operations include format
  changes (replace an Avro file with a Parquet file) and compactions
  (several files are replaced with a single file that contains the same
  rows).
- Delete operations must verify that specific files to delete are still
  in the table. Delete operations based on expressions can always be
  applied (e.g., where timestamp < X).
- Table schema updates and partition spec changes must validate that the
  schema has not changed between the base version and the current
  version.

#### Metastore Tables

The atomic swap needed to commit new versions of table metadata can be
implemented by storing a pointer in a metastore or database that is
updated with a check-and-put operation. The check-and-put validates that
the version of the table that a write is based on is still current and
then makes the new metadata from the write the current version.

Each version of table metadata is stored in a metadata folder under the
table's base location using a naming scheme that includes a version and
UUID: `<V>-<random-uuid>.metadata.json`. To commit a new metadata
version, `V+1`, the writer performs the following steps:

1. Create a new table metadata file based on the current metadata.
2. Write the new table metadata to a unique file:
   `<V+1>-<random-uuid>.metadata.json`.
3. Request that the metastore swap the table's metadata pointer from the
   location of `V` to the location of `V+1`.

(Steps 4 onward, the failure/retry branch, and the deprecated File System
Tables scheme's `v<V>.metadata.json` rename-based variant are omitted
from this excerpt as not load-bearing for STRAND's S3-native design,
which uses conditional writes rather than a metastore or a filesystem
rename.)
