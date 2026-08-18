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
thereafter except for declared amendments): format version, the declared
CAS host, minimum snapshot retention. *Not yet implemented in the
reference implementation* — table-metadata-driven retention is M3 scope
(compaction); this chapter states the shape now so no future session
invents a different one.

**Snapshot metadata** (`_strand/snapshots/{version:020}-{writer_nonce}.json`,
immutable, one per *proposed* commit — §3 explains the nonce). Fields:

| field          | type                | notes                                                     |
| -------------- | ------------------- | --------------------------------------------------------- |
| version        | u64                 | this snapshot's version                                   |
| next_row_id    | u64                 | one past the highest row-ID any referenced segment claims |
| segments       | array of SegmentRef | the segment set (below)                                   |
| index_versions | per-blob-family map | each blob family's index version (below)                  |

A `SegmentRef`:

| field        | type   | notes                                       |
| ------------ | ------ | ------------------------------------------- |
| path         | string | the segment object's key                    |
| row_id_base  | u64    | must match the segment's own hotcache       |
| row_id_count | u64    | must match the segment's own hotcache       |
| byte_length  | u64    | the segment object's total size             |
| checksum     | u64    | xxHash3-64 over the segment's on-disk bytes |

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
a reader MUST surface an error rather than loop forever.

## 4. Safety rules (`CLAUDE.md` §6)

**One declared CAS host.** Table metadata declares where the pointer
lives: native conditional writes on the store, or a named external
catalog. All writers MUST use the declared host. This is a conformance
requirement on writers, not a mechanism this protocol enforces against a
misconfigured writer — no fencing token or cross-host detection is
specified. Enforcement beyond convention is out of scope for v0.1.

**Deletion safety and reader 404-refresh.** A segment file or a snapshot
metadata object MUST NOT be physically deleted while any retained
snapshot (per table metadata's retention policy) references it. A reader
that gets 404 on an object its snapshot references MUST treat the
snapshot as expired — refresh and retry per §3 — rather than report
corruption.

**Orphan files.** A writer that crashes, or loses the pointer CAS, after
writing segment or snapshot metadata files but before its commit lands
(§2 step 3 never completing) leaves orphans. Orphans are harmless to
correctness — nothing live references them — and cost only storage. They
are removed by listing the prefix, subtracting everything referenced by a
retained snapshot, and deleting the remainder older than the retention
window. The sweep tool lands at M3; this rule is stated now so that tool
has nothing to invent.

**Reader freshness has a price, and it is stated.** A reader's consistency
model is snapshot-at-load. Freshness costs one conditional GET of the
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
the reader 404-refresh recovery path. Not yet implemented: table metadata
itself (`_strand/metadata.json`), retention-policy-driven snapshot
expiry, and the orphan-sweep tool — all M3 scope. `commit`/`read_snapshot`
propagate definite backend failures as typed errors (`CommitError::Io`,
`ReadError::Io`) and resolve ambiguous pointer-write outcomes per §2
(`StoreError::Ambiguous`, `crates/strand-core/src/store.rs`), classified
from the real AWS SDK's `SdkError` variants in
`crates/strand-core/src/s3_store.rs`. GCS/Azure conditional-write
semantics remain unverified (R5, open).
