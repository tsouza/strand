# Manifest

Normative for STRAND v0.1. Defines the snapshot manifest: the three object
kinds under a table's `_strand/` prefix, the compare-and-swap commit
protocol, the reader protocol, and the safety rules from `CLAUDE.md` §6
that make multi-writer, multi-reader interop actually safe rather than
merely possible. Approved by RFC 0001
(`rfcs/0001-container-rowid-manifest.md`); this chapter states the settled
result — see the RFC for alternatives considered and the adversarial
review, whose protocol finding was the step-1 filename collision. Two
further protocol changes were found during implementation, not by the
review — the pointer-CAS `Io`-vs-`PreconditionFailed` retry bug, and the
definite-vs-ambiguous failure distinction with its disambiguating
follow-up read (§2 step 3) — and are recorded in the RFC's Discussion
section.

Reference implementation: `crates/strand-core/src/manifest.rs`. Backends:
`crates/strand-core/src/store.rs` (the `ConditionalStore` trait and an
in-memory test double), `crates/strand-core/src/s3_store.rs` (S3/MinIO).

## 1. Object kinds

Three object kinds, all under a `_strand/` prefix inside the index's root.
Snapshot metadata and the current pointer are JSON; invariant 11's
byte-determinism pins (endianness, checksum algorithm, codec-variant
registration) govern binary wire structures decoded into a fixed byte
layout, which JSON is not — its determinism concern is the smaller one of
stable, documented key ordering for human diffability, not bit-exactness.
This mirrors Puffin's "opaque typed blobs with a JSON footer" pattern
(`docs/lineage.md`): binary where bytes are read by a codec on the hot
path, JSON where humans and cross-engine tooling read it.

**Table metadata** (`_strand/metadata.json`, written once, immutable
thereafter except for the CAS-host move, the one declared amendment
(`CLAUDE.md` §6)). Unlike the other two object kinds, this object never
participates in the `_strand/current` CAS race described in §2 below — it
has no pointer, no proposed-vs-current distinction, and no retry loop.
Creating it is a single `put_if_absent` to a fixed key
(`table_metadata::write_table_metadata`); an `Ambiguous` outcome is
resolved by a follow-up read that checks whether this attempt's own bytes
are the ones now present, the same disambiguation principle §2's pointer
CAS uses, applied to a plain create instead of a compare-and-swap.

| field          | type              | notes                                                                                                                       |
| -------------- | ----------------- | --------------------------------------------------------------------------------------------------------------------------- |
| format_version | u32               | this object's own format version, distinct from a segment's `format_major`/`format_minor` (`spec/container.md` §1)          |
| cas_host       | `CasHost`         | `{"type": "native", "store": "<name>"}` or `{"type": "catalog", "uri": "<uri>"}` — `CLAUDE.md` §6's "one declared CAS host" |
| retention      | `RetentionPolicy` | below                                                                                                                       |

A `RetentionPolicy`:

| field                   | type | notes                                                                   |
| ----------------------- | ---- | ----------------------------------------------------------------------- |
| min_snapshots_to_keep   | u32? | keep at least this many of the most recent snapshots, regardless of age |
| max_snapshot_age_millis | u64? | keep any snapshot committed within this many milliseconds of "now"      |

At least one of the two `RetentionPolicy` fields MUST be set —
`write_table_metadata` rejects a policy declaring neither, since a table
with no retention floor at all would let a sweep treat every snapshot but
the current one as immediately expired. When both fields are set, a
snapshot is retained if it satisfies *either* one — the union, not the
intersection — resolved in RFC 0001's Discussion section (2026-08-19)
because the original approval left the both-set case unstated: the
deletion-safety rule's cost of under-retaining (real, unrecoverable data
loss for a reader with an expired-but-still-open snapshot) so outweighs
the cost of over-retaining (storage only, a cost `CLAUDE.md` §6 already
accepts elsewhere) that the safer reading wins outright, not as a close
call. This matches Apache Iceberg's own documented behavior for its
equivalent pair of knobs (`history.expire.min-snapshots-to-keep`,
`history.expire.max-snapshot-age-ms`,
`references/iceberg-snapshot-expiration-retention-properties.md`).

The current snapshot (the one `_strand/current` names) is retained
unconditionally, regardless of what the policy alone would otherwise
say — nothing safely lets a policy expire the one snapshot every live
reader is using. `table_metadata::retained_snapshots` implements this
whole rule as a pure function: given a `RetentionPolicy`, a snapshot
list, and a "now" timestamp, it returns exactly the retained subset.
Boundary rule: a snapshot exactly `max_snapshot_age_millis` old is
retained (`age <= max_age`, inclusive).

**Snapshot metadata** (`_strand/snapshots/{version:020}-{writer_nonce}.json`,
immutable, one per *proposed* commit — §3 explains the nonce). Fields:

| field               | type                | notes                                                                                                                                                                        |
| ------------------- | ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| version             | u64                 | this snapshot's version                                                                                                                                                      |
| next_row_id         | u64                 | one past the highest row-ID any referenced segment claims                                                                                                                    |
| segments            | array of SegmentRef | the segment set (below)                                                                                                                                                      |
| index_versions      | per-blob-family map | each blob family's index version (below)                                                                                                                                     |
| committed_at_millis | u64                 | milliseconds since the Unix epoch, stamped by the proposing writer (RFC 0001 Discussion, 2026-08-19, M3-4); not an invariant-11 byte-determinism target, like `writer_nonce` |

A `SegmentRef`:

| field           | type                 | notes                                                                                     |
| --------------- | -------------------- | ----------------------------------------------------------------------------------------- |
| path            | string               | the segment object's key                                                                  |
| row_id_base     | u64                  | must match the segment's own hotcache                                                     |
| row_id_count    | u64                  | must match the segment's own hotcache                                                     |
| byte_length     | u64                  | the segment object's total size                                                           |
| checksum        | u64                  | xxHash3-64 over the segment's on-disk bytes                                               |
| deletion_vector | `DeletionVectorRef?` | absent iff no row in this segment has ever been deleted (`spec/deletion.md` §3, RFC 0012) |

Per the Lance model (`docs/lineage.md`), `index_versions` references each
blob family's index version without embedding that family's internal
structure — index-aware, index-internals-agnostic, as RFC 0001 decided.
*Not yet implemented in the reference implementation* — the field becomes
real when the first blob families with versioned index state land (M1
lexical, M2 vector); its exact key/value shape is pinned then, in those
milestones' RFCs, not here. No blob-family-specific fields belong in
`SegmentRef`; a family's own internal state lives inside the segment
(`spec/container.md`), not the manifest.

**Current pointer** (`_strand/current`): the single object every reader
and writer reads first. Its content is the path (key) of the current
snapshot metadata object — not a bare version number, so a reader or
writer never needs a second lookup to resolve it to a fetchable path.

## 2. Commit protocol

On a store with native conditional writes (S3, confirmed; GCS/Azure
header semantics are R5, open — `docs/ledger.md`):

1. Read `_strand/current` (or, for a table's very first commit, treat it
   as absent). Read that snapshot's `version` and `next_row_id` — or, if
   absent, treat both as `0`. Derive this attempt's version
   (`version + 1`, or `0` if absent), row-ID range
   (`[next_row_id, next_row_id + count)`, or `[0, count)` if absent), and
   a fresh random `writer_nonce`. If the snapshot the pointer names is
   gone (the §3 404 race), the writer re-reads and retries this step;
   unlike the reader path, this retry is unbounded, because the writer's
   real bound is the pointer CAS it is about to contend on.
2. Write the new segment(s), then create the snapshot metadata object at
   `_strand/snapshots/{version:020}-{writer_nonce}.json` with
   `If-None-Match: *`. Because the nonce makes this path unique to this
   attempt, this create is not expected to fail — a writer that
   nonetheless hits a precondition failure here has a bug, not a race,
   and MUST NOT silently retry as if it were one.
3. Advance the current pointer: `PUT _strand/current` with `If-Match:
   <etag last read>` (or `If-None-Match: *` if step 1 found no existing
   pointer), pointing at the object just written in step 2.
   - Success: this writer's commit is now current.
   - `412 Precondition Failed`: another writer landed first. Re-read
     `_strand/current` and the winner's snapshot metadata, recompute (not
     reuse) this attempt's version and row-ID range from that fresh
     state, write a new snapshot object under a new nonce, and retry
     from step 3.
   - A definite backend failure (a well-formed error response from the
     store, or a request that never left the client — not a precondition
     failure): a conforming writer MUST NOT treat this the same as a lost
     CAS and retry indefinitely — that would turn a permanent outage into
     an infinite loop, mistaking it for a rival writer that will
     eventually stop contending. It MUST surface the failure to its
     caller.
   - An ambiguous outcome (a timeout, a dropped connection, a response
     that stopped arriving mid-stream — the request may have reached the
     store and been applied before the failure occurred): a conforming
     writer MUST NOT treat this as either success or failure without
     checking. Because the pointer CAS is atomic on the backend, a plain
     follow-up `GET _strand/current` resolves the ambiguity completely: if
     it now names the path this attempt just wrote, the write landed and
     this commit succeeded (the response was merely lost); otherwise it
     did not land, and the writer proceeds exactly as on `412` — re-read,
     recompute, retry under a new nonce. A writer that instead retries
     blindly on ambiguity risks committing a redundant, wasted extra
     version on top of one that already succeeded.

Every I/O a `build_segments`-style callback performs (writing a segment,
say) MUST be safe to run more than once per logical commit, since a
retried attempt re-invokes it: a segment written to a *fixed* path derived
only from caller-side state (a loop index, a writer ID) will collide with
that same path's first attempt on retry, because the first attempt's
write already landed even though its commit lost the race. Include
something attempt-unique in any such path, the same way the snapshot
path's `writer_nonce` does.

**A second commit path, same protocol, different transform.**
`commit_deletion_vector` (`spec/deletion.md` §4, RFC 0012) uses the
identical CAS retry loop described above — read current state, propose a
new snapshot, race the pointer, recompute and retry on loss — but revises
exactly one existing `SegmentRef`'s `deletion_vector` field instead of
appending a new segment; `next_row_id` is unchanged. This is a distinct
function, not a mode of `commit` itself, precisely so `commit`'s own
row-ID-allocation accounting (`next_row_id + added_row_ids`) never has to
reason about an update-in-place case. `verification/manifest.tla` now
models this revise shape too (`ProposeDeletionVectorCommit`, RFC 0012
Discussion — post-approval amendments), TLC-checked alongside the
original append shape; a DST cross-validation harness covering it is now
built and run (`docs/roadmap.md` M3-3, 0 drift across 3,000 writer and
1,000 reader trajectories), and a TLAPS proof covers it too, partially —
the writer-path inductive invariant, including `ProposeDeletionVectorCommit`
itself, is mechanically proved (`docs/roadmap.md` M3-2, 1,261 obligations),
while the reader-path actions, the `Next`-level temporal-invariance
theorem, and the model's other invariants remain open (RFC 0002's own
remaining scope).

## 3. Reader protocol

1. `GET _strand/current` — the current snapshot's path.
2. `GET` that snapshot metadata object — the segment set and
   `next_row_id`.
3. Open each referenced segment per `spec/container.md` §3, in parallel
   across segments.

A `404` at either step 1 or step 2 means the referenced object was
removed by compaction between this reader's requests — a live race, not
corruption. A reader MUST refresh `_strand/current` and retry the whole
sequence on such a 404, bounded by an implementation-chosen retry limit
(the exact count is a reader parameter this chapter does not pin, for the
same reason the container chapter's speculative tail size is unpinned —
but the bound itself, unlike its value, is not optional). Past the limit,
a reader MUST surface an error rather than loop forever. The reference
implementation's recommended default, measured against real, sustained
MinIO contention rather than guessed
(`bench/src/reader_refresh_contention.rs`,
`rfcs/0001-container-rowid-manifest.md` Discussion, 2026-08-19), is
**`READER_REFRESH_RETRY_LIMIT = 5`**
(`crates/strand-core/src/manifest.rs`) — roughly 5x the worst-case retry
count (1) observed across 691 real reads under sustained concurrent-writer
and compactor contention.

## 4. Safety rules (`CLAUDE.md` §6)

**One declared CAS host.** Table metadata declares where the pointer
lives: native conditional writes on the store, or a named external
catalog. All writers MUST use the declared host. This is a conformance
requirement on writers, not a mechanism this protocol enforces against a
misconfigured writer — no fencing token or cross-host detection is
specified. Enforcement beyond convention is out of scope for v0.1.

**Deletion safety and reader 404-refresh.** A segment file or a snapshot
metadata object MUST NOT be physically deleted while any retained
snapshot (per table metadata's retention policy, §1's `RetentionPolicy`
and `table_metadata::retained_snapshots`) references it. A reader
that gets 404 on an object its snapshot references MUST treat the
snapshot as expired — refresh and retry per §3 — rather than report
corruption.

**Orphan files.** A writer that crashes, or loses the pointer CAS, after
writing segment or snapshot metadata files but before its commit lands
(§2 step 3 never completing) leaves orphans. Orphans are harmless to
correctness — nothing live references them — and cost only storage. They
are removed by listing the prefix, subtracting everything referenced by a
retained snapshot (§1's retention-eligibility rule, implemented in
`table_metadata::retained_snapshots`), and deleting the remainder older
than the retention window. The sweep tool (`strand-tools sweep`,
`crates/strand-tools/src/orphan_sweep.rs`) implements this at M3-5
(`docs/roadmap.md`).

The **retention window** here is a parameter to the sweep itself, not a
`TableMetadata`/`RetentionPolicy` field — a real gap the original rule
left unstated (RFC 0001's Discussion section, M3-5) — since it protects a
different thing than `RetentionPolicy` does. `RetentionPolicy` bounds
which *snapshots* stay eligible for a reader that loaded one a while ago;
the orphan retention window bounds how long a sweep waits before treating
an unreferenced object as safe to remove, the safety margin against a
race with a writer whose commit has not yet landed its pointer update.
Those two concerns have no shared natural horizon — a table's snapshot
policy is reasonably measured in days, an in-flight commit's window in
seconds to minutes — so `strand-tools sweep` takes its own
`--retention-window-secs`, defaulting to Apache Iceberg's own
`remove_orphan_files` default of 3 days
(`references/iceberg-remove-orphan-files-procedure.md`), the same prior
art already cited for `RetentionPolicy`'s own union-combination rule
above. `TableMetadata` itself carries no orphan-specific field.

A listing of `_strand/snapshots/` can hold, at the same version number,
both the one real snapshot object a commit's pointer CAS won and one or
more objects a losing attempt wrote first — nothing in the wire format
distinguishes them. It can also hold an orphan whose version is strictly
**higher** than the real current snapshot's: a writer that crashes after
writing its snapshot object but before its pointer CAS lands leaves an
orphan at exactly `true_current.version + 1` (the version is computed
from the state read at the *start* of the attempt), and nothing advances
the real pointer to catch up if that writer never retries. A conforming
sweep MUST therefore resolve "current" from the real CAS pointer (the
same read every conforming reader performs), never by inferring it as
"the highest version number present in a listing" — that heuristic would
misidentify a crashed writer's high-numbered orphan as current and
protect its files indefinitely, the exact failure this rule exists to
prevent. Any listed snapshot object whose version exceeds the real
current version is provably an orphan (the pointer proves no commit ever
reached it) and MUST NOT be fed to `retained_snapshots`. Every other
listed snapshot object — version less than or equal to the real current
version — is genuinely ambiguous (real-historical-and-superseded, or an
orphan that lost a same-version race) and MAY be fed to
`retained_snapshots` together: the count-based floor dedupes by version
number, so a same-version orphan cannot displace a real snapshot from the
retained set, and at worst is itself also protected for one extra sweep
cycle — over-retention, never under-retention.

**Reader freshness has a price, and it is stated.** A reader's consistency
model is snapshot-at-load. Freshness costs one GET of the
pointer per refresh; this chapter defines no push or notification
mechanism. A "warm query with read-your-writes" costs one pointer round
trip more than a query against a cached snapshot.

**Write amplification is the writer's problem.** Immutable segments mean
a commit per tiny batch produces a segment per tiny batch. This format
defines no WAL and no memtable; a production writer batches on its own
side.

## 5. Conformance status

Verified against real S3-compatible object storage (MinIO, via
`crates/strand-core/tests/s3_store.rs` and `bench/`), not simulated:
conditional-write semantics, the full commit-and-read round trip, the
step-3 recompute-on-retry requirement under genuine concurrent writers
(`bench/src/commit_contention.rs`), the orphan-harmless crash test, and
the reader 404-refresh recovery path. Table metadata
(`_strand/metadata.json`, `table_metadata::write_table_metadata`/
`read_table_metadata`) and retention-policy-driven snapshot-eligibility
(`table_metadata::retained_snapshots`) are implemented and unit-tested
against `InMemoryStore` (M3-4, `docs/roadmap.md`); real-MinIO coverage for
`write_table_metadata`/`read_table_metadata` themselves (as opposed to the
sweep tool built on top of them, below) remains open. The orphan-sweep
tool (`strand-tools sweep`, `crates/strand-tools/src/orphan_sweep.rs`,
M3-5) is implemented and verified against real MinIO — `list`
(`ListableStore`, real `ListObjectsV2` pagination) and `delete_object`
(`DeletableStore`) join `ConditionalStore`/`RangeGetStore` on
`crates/strand-core/src/s3_store.rs`'s `S3Store` — reproducing the same
crashed-writer-orphan pattern `orphaned_writer_crash_is_harmless_to_
readers` established for readers, now for the sweep, plus the
retention-window safety-margin case. `commit`/`read_snapshot`
propagate definite backend failures as typed errors (`CommitError::Io`,
`ReadError::Io`) and resolve ambiguous pointer-write outcomes per §2
(`StoreError::Ambiguous`, `crates/strand-core/src/store.rs`), classified
from the real AWS SDK's `SdkError` variants in
`crates/strand-core/src/s3_store.rs`. GCS/Azure conditional-write
semantics remain unverified (R5, open).
