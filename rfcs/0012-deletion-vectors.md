# RFC 0012: Deletion vectors (invariant 2 general machinery)

- **Status:** Approved. Adversarial review independently re-derived the
  worked example (wrote and ran its own program against `roaring` 0.11.5,
  matching this RFC's claimed 22 bytes digit-for-digit), re-read
  `verification/manifest.tla` directly (confirming the TLA+-gap claim is
  precise, not overstated), and read the real current
  `crates/strand-core/src/manifest.rs`/`segment.rs` rather than trusting
  this RFC's paraphrase. Found 1 Critical, 3 Important, 1 Minor, all
  fixed. Critical: `commit_deletion_vector`'s first-draft closure signature
  (`build_deletion_vector: impl Fn() -> DeletionVectorRef`, no parameter)
  could not satisfy this same RFC's own stated race-safety requirement —
  reading current state fresh on every CAS retry — since it had no way to
  receive that state; implementing the RFC exactly as first drafted would
  have reproduced the lost-update race the RFC claimed to prevent. Fixed
  by widening the signature to `impl Fn(&SegmentRef) -> DeletionVectorRef`,
  mirroring `commit`'s own `next_row_id` parameter, and rewriting the
  race-safety paragraph to show the corrected signature actually closes
  the race (Design §3). Important (3): a checksum-verification precedent
  this RFC cited ("the same discipline `segment::open` already applies")
  does not exist anywhere in the codebase — no `segment::open` function
  exists yet, and `SegmentRef.checksum` is written but never read back;
  fixed by stating this RFC's `deletion::read` is the first code to
  actually verify a checksum on read, with a named error variant on
  mismatch (Design §4); the "segment not found" error path had no named
  variant — fixed with `CommitError::SegmentNotFound(String)` (Design §3);
  `spec/row-ids.md` §3's "marks row-IDs, not local ordinals... survives a
  merge... without needing a remap" reads in real tension with this RFC's
  local-ordinal wire encoding with no reconciling text — fixed with an
  explicit paragraph distinguishing logical row-ID identity (stable) from
  the per-segment bitmap encoding (rebuilt at merge, not translated)
  (Design §1). Minor (1): `segment_path` lookup uniqueness was assumed but
  never stated — fixed inline in the corrected Design §3 text, citing
  `segment::write_segment`'s own collision-panic guarantee.
- **Milestone:** Named as M3 scope in `docs/milestones.md` ("Deletion
  vectors" is M3's first listed deliverable); pulled forward here because
  RFC 0010's own Non-goals named "deletion-vector integration" as the
  single largest remaining item blocking the vector family's query
  resolution (`spec/vectors.md` §6 step 4, already normative prose with no
  mechanism behind it). This RFC registers only the general mechanism and
  the vector family's own integration point — compaction-time physical
  removal, retention policy, and the orphan sweep remain M3 scope
  (Non-goals).
- **Spec chapters produced:** a new `spec/deletion.md`; additively extends
  `spec/container.md` §9 (registers `family_id = 4` "deletion",
  `blob_type_id = 0` "deletion vector") and `spec/manifest.md` (adds
  `SegmentRef.deletion_vector`, a new commit path). Updates `spec/
  vectors.md` §6 step 4 from unimplemented normative prose to a concrete
  mechanism.
- **Invariants exercised:** 1, 2, 3, 6 (`CLAUDE.md` §6), 10, 11

## Summary

Invariant 2 (`CLAUDE.md` §5) states plainly: "Deletes are deletion-vector
blobs (Roaring); updates are delete + reinsert." Nothing in the codebase
implements this yet — not a general blob format, not a manifest slot, not
a reader integration. Every family that references "the segment's deletion
vector" (`spec/vectors.md` §6 step 4; `spec/row-ids.md` §3) has been citing
a mechanism that does not exist. `spec/filter-bitmaps.md` §7 anticipated
this precisely: "Deletion vectors (invariant 2, M3 scope) are a separate
blob and a separate RFC; that RFC may cite this chapter's Roaring
wire-format registration (§3) without repeating it."

The central design fact this RFC turns on: **a segment is one immutable
object** (`spec/container.md` §1). A deletion vector must be revisable —
new deletes accumulate over the segment's lifetime — so it cannot live
inside the segment's own container bytes without violating invariant 2's
"no in-place mutation, ever." It must be its own object, referenced from
the manifest (which already has an established, TLA+-modeled pattern for
revisable, CAS-committed state), not from the segment's own hotcache blob
registry.

This RFC therefore registers: a standalone deletion-vector object format
(a bare, standard 32-bit Roaring bitmap, no container framing); an
optional `deletion_vector` reference on `spec/manifest.md`'s `SegmentRef`;
a new manifest commit path that updates that reference without allocating
new row-IDs or appending a new segment; and the read-side integration that
makes `spec/vectors.md` §6 step 4 real. A genuine, load-bearing
verification gap is surfaced and named, not glossed: `verification/
manifest.tla`'s `ProposeSnapshot` action models a segment as `[base:
Nat, count: Nat]` and only ever *appends* a new one — it has no
transition shape for revising an existing entry's fields in place. This
RFC's new commit path is therefore unmodeled territory, stated as Non-goal
/ open question, not silently assumed covered by RFC 0002's existing
Approval.

## Design

### 1. The deletion-vector object (`spec/deletion.md`, new chapter)

**Not a segment, not a container.** No footer, no hotcache, no blob
registry — `spec/container.md`'s machinery exists to make a large,
multi-blob object cheaply openable in two round trips; a deletion vector
is small (bounded by one segment's row count) and single-purpose. The
object's entire byte content is the serialized bitmap, nothing else.

**Format**: the standard 32-bit Roaring format, exactly as
`RoaringFormatSpec` defines it (`references/roaring-format-spec-and-rust-
crate.md`), under `SERIAL_COOKIE_NO_RUNCONTAINER` — citing `spec/filter-
bitmaps.md` §3's identical MUST rule by reference rather than repeating
its reasoning (per that chapter's own §7 invitation). The 64-bit Roaring
extension MUST NOT be used, for the same interoperability reason.

**Indexing convention**: identical to `spec/filter-bitmaps.md` §3 — the
bitmap indexes **local ordinals** (`0` to `row_id_count - 1`, `spec/
row-ids.md` §1), not the global 64-bit row-ID space directly. A reader
resolves membership via `row_id - row_id_base`. The same normative cap
applies and is restated here rather than left to cross-chapter inference:
a segment declaring a deletion vector MUST satisfy `row_id_count <=
2^32`.

**Reconciling this with `spec/row-ids.md` §3's "marks row-IDs, not local
ordinals," explicitly, not left for a reader to puzzle out.** That
sentence is about *logical* identity, not wire encoding: "this is why it
survives a merge that remaps local ordinals without needing a remap step
of its own" describes the row-ID space's own stability property, the
thing invariant 1 buys generally. This RFC's wire bytes are still
local-ordinal-indexed, exactly like `spec/filter-bitmaps.md`'s, and that
is *not* in tension with row-ids.md once the two claims are kept
separate: **which rows are logically dead** is row-ID-identified and
genuinely stable across a segment's lifetime; **the bitmap that records
it** is a per-segment artifact, valid only against that segment's own
fixed `row_id_base`/`row_id_count` (never reinterpreted against a
different segment's local-ordinal space, since segments are immutable and
a segment's own base never changes). At merge time (M3, Non-goals, below)
a compacted segment gets a **freshly built** deletion vector — the old
per-segment bitmaps are not translated or unioned, they're superseded
along with the segments they described, using the surviving row-IDs'
identities (not their old local ordinals) to populate the new segment's
own, differently-indexed bitmap. Row-ids.md §3's stability claim is about
that row-ID identity surviving the merge, not about any one segment's
bitmap bytes surviving unchanged — this RFC's Non-goals section already
says as much for the merge case; this paragraph makes the general
reconciliation explicit rather than leaving it to be inferred from that
one Non-goals bullet.

**Registration** (`spec/container.md` §9): `family_id = 4` ("deletion"),
`blob_type_id = 0` ("deletion vector"). This "blob type" is registered for
identity and citation purposes even though the object itself never lives
inside a segment's own blob registry — the registry table is where every
`family_id`/`blob_type_id` pair any RFC has assigned is recorded, and this
RFC assigns one.

**Presence is optional, not padded.** A segment with no deletes has no
deletion-vector object and no manifest reference to one — not an empty
bitmap written for uniformity. This mirrors `spec/vectors.md` §1's
already-established pattern (the flat-vector blob is present only when
reranking is enabled).

### 2. Manifest extension (`spec/manifest.md`)

`SegmentRef` gains one new optional field:

| field             | type                        | notes                                                            |
| ----------------- | --------------------------- | ----------------------------------------------------------------- |
| `deletion_vector`  | `DeletionVectorRef?`        | absent iff no row in this segment has ever been deleted           |

A `DeletionVectorRef`:

| field         | type   | notes                                                        |
| ------------- | ------ | -------------------------------------------------------------- |
| `path`        | string | the deletion-vector object's key                               |
| `byte_length` | u64    | the object's total size                                        |
| `checksum`    | u64    | xxHash3-64 over the object's bytes (invariant 11's default)     |

Deliberately shaped like `SegmentRef`'s own `path`/`byte_length`/
`checksum` fields, for the same reason: a reader fetches by path and
verifies by checksum, with no further indirection.

**Deletion-safety interaction.** `CLAUDE.md` §6's rule — "a file MUST NOT
be physically deleted while any retained snapshot references it" —
applies to a deletion-vector object exactly as it does to a segment
object: an old `DeletionVectorRef` still named by a retained (not yet
expired) snapshot keeps its object alive under the same orphan-sweep
accounting `spec/manifest.md`'s existing rule already states. No new rule
is needed; this is the existing rule applied to a new object kind, stated
here so it is not silently assumed.

### 3. Commit protocol: a new, narrower write path

`manifest::commit`'s existing shape — `build_segments: impl Fn(u64) ->
Vec<SegmentRef>`, called fresh on every CAS retry, always *appending* to
the current segment set — does not fit a deletion: a delete touches an
*existing* `SegmentRef`'s `deletion_vector` field, adds no new row-IDs,
and appends no new segment. Reusing `commit` by having callers pre-compute
a modified segment list themselves and pass it through some general
"replace the whole segment list" hook was considered and rejected
(Alternatives considered) as strictly more dangerous than a narrow,
purpose-built path.

**`commit_deletion_vector`** (`crates/strand-core/src/manifest.rs`):

```rust
pub fn commit_deletion_vector<S: ConditionalStore>(
    store: &S,
    segment_path: &str,
    build_deletion_vector: impl Fn(&SegmentRef) -> DeletionVectorRef,
) -> Result<SnapshotMetadata, CommitError>
```

`build_deletion_vector` is called fresh on every CAS retry, receiving the
*current* `SegmentRef` for `segment_path` as found in that retry's
freshly-read snapshot state — carrying `row_id_base`, `row_id_count`, and
the segment's current `deletion_vector: Option<DeletionVectorRef>` in one
parameter, the same role `commit`'s own `build_segments: impl Fn(u64) ->
Vec<SegmentRef>` gives `next_row_id`. This is not incidental plumbing: it
is what makes the race-safety property below (Superseding, not
accumulating) actually implementable, not merely asserted — a
no-argument closure (an earlier draft of this RFC proposed one) cannot
read current state on each retry and therefore cannot satisfy that
property at all, a real, load-bearing signature requirement, not a
stylistic choice. The closure is responsible for writing the actual
bitmap object, under an attempt-unique path, before returning the
`DeletionVectorRef` to it — `store` is captured from the enclosing scope,
the same way a real `build_segments` closure captures `store` to call
`segment::write_segment` internally.

The function performs the identical CAS retry loop `commit` already uses
(read current state, propose a new snapshot, race the pointer, recompute
and retry on loss) with one substitution: the new snapshot's `segments`
list is the current list with exactly one entry — the one at
`segment_path`, assumed unique within `current.segments` (true today:
`segment::write_segment` panics on a colliding path per its own doc
comment, so no two live `SegmentRef`s ever share a `path`) — with its
`deletion_vector` field replaced, `next_row_id` unchanged, no entry
appended or removed. If `segment_path` names no segment in the current
snapshot (the segment does not exist, or was already compacted away), the
loop returns a new error variant, `CommitError::SegmentNotFound(String)`
(carrying `segment_path`), added alongside `CommitError`'s existing
`Io(String)` variant — not silently no-op'd, and not conflated with a
backend I/O failure.

**Superseding, not accumulating, and how the corrected signature actually
closes the race.** Each call replaces the segment's prior
`deletion_vector` reference wholesale — the new object's bitmap is the
*complete* set of that segment's tombstoned local ordinals, not a delta
to be unioned with a predecessor at read time. A caller adding one more
deleted row supplies a `build_deletion_vector` closure that, given the
`&SegmentRef` the retry loop hands it, reads that `SegmentRef`'s current
`deletion_vector` (if any — one `GET` against the object store, via the
captured `store`), decodes it, unions in the new tombstone, and writes
that union as the new object under a fresh, attempt-unique path,
returning the new `DeletionVectorRef`. Because this whole read-union-write
sequence re-runs from the closure on *every* CAS retry — exactly like
`build_segments`'s own re-invocation discipline — two concurrent
`commit_deletion_vector` calls against the same segment cannot silently
lose a tombstone: whichever attempt loses the pointer CAS re-reads the
now-current `SegmentRef` (carrying the winner's `deletion_vector`) on its
next iteration and re-unions against it, rather than clobbering the
winner's write with a stale one computed from pre-race state.

### 4. Read-side integration

**General mechanism** (`crates/strand-core/src/deletion.rs`, new): a
`DeletionVector` type wrapping a decoded `roaring::RoaringBitmap`, with
`is_deleted(&self, row_id: u64, row_id_base: u64) -> bool` (translating to
a local ordinal internally, per §1's convention) and a `read` function
fetching and decoding a `DeletionVectorRef`'s object bytes.

**A real correction, found during this RFC's own adversarial review**: an
earlier draft claimed this checksum verification would match "the same
discipline `segment::open` already applies to a `SegmentRef`'s checksum"
— false. `segment::write_segment` computes and stores a `SegmentRef.
checksum` (`segment.rs`), but nothing in the current codebase reads it
back or verifies it; there is no `segment::open` function at all yet.
This RFC's `deletion::read` is therefore the *first* code in this
codebase to actually verify a stored checksum on read, not a precedent
follower: it computes `XxHash3_64::oneshot` over the fetched bytes and
compares against `DeletionVectorRef.checksum`; on mismatch it returns a
new `DeletionError::ChecksumMismatch { declared: u64, computed: u64 }`
rather than silently decoding bytes that may be corrupt. Whether
`SegmentRef.checksum` itself should gain the same on-read verification is
real, separate, pre-existing scope this RFC does not expand into.

**Fetch timing, resolved (closing RFC 0010's own named-open question,
"How a reader obtains and applies the deletion vector's own bytes,"
Non-goals).** A `SegmentRef` with a present `deletion_vector` costs one
additional GET — fetched in the *same parallel wave* as that segment's
own container open (invariant 3's "issued as one parallel wave" already
covers a *set* of independent fetches, and this GET has no dependency on
the segment's own footer/hotcache bytes, so it adds no sequential
latency, only one more GET and up to the deletion vector's own byte size
to the open accounting). This is stated as this family's own answer, not
inherited silently: the deletion vector is **not** part of the segment's
own two-round-trip open budget (invariant 3's bound is specifically about
a *segment's* footer-then-hotcache sequence) — it is a manifest-level
reference resolved in the same wave as, but independent of, the segment
open, the same way the pointer and snapshot GETs precede segment opens
without being *part of* any one segment's open.

**Vector family integration** (`spec/vectors.md` §6 step 4, now
concrete): `crates/strand-vector/src/query.rs` gains `filter_deleted`, a
small, separate function — not folded into `scan_selected_clusters` —
matching the spec's own step 3/step 4 boundary: step 3 (`scan_selected_
clusters`) is unaware of deletion vectors, exactly as it is today; step 4
is a caller-composed filter over its output, applied only when a
`DeletionVector` is present. `filter_deleted(candidates: Vec<Candidate>,
row_id_base: u64, deletion_vector: &DeletionVector) -> Vec<Candidate>`
retains every candidate whose row-id is not tombstoned. `strand-vector`
gains no `roaring` dependency of its own — `DeletionVector` is a
`strand-core` type, matching the layering invariant 2's own framing
implies ("the deletion-vector blob itself belongs to invariant 2's
general machinery, not this family," RFC 0010 Non-goals).

## Worked example

A real deletion-vector object: local ordinals `{2, 5, 100}` tombstoned in
a segment with `row_id_base = 1000`, `row_id_count = 200`.

Building it with the real `roaring` crate 0.11.5 (`RoaringBitmap`
populated via `.insert(2)`, `.insert(5)`, `.insert(100)`,
`.serialize_into` — the crate's default output is already
`SERIAL_COOKIE_NO_RUNCONTAINER`-framed for a plain array-container
bitmap, matching §1's requirement with no extra step) — real, executed
output, not hand-derived:

`22` bytes total: `3a 30 00 00 01 00 00 00 00 00 02 00 10 00 00 00 02 00
05 00 64 00`. Structurally: bytes `0..4` are the `SERIAL_COOKIE_NO_
RUNCONTAINER` cookie header (`0x0000303a` little-endian — `12346`,
`RoaringFormatSpec`'s registered no-run-container cookie value); bytes
`4..8` a one-container count field; bytes `8..12` that one container's
key (`0`) and cardinality-minus-one (`2`, i.e. cardinality `3`); bytes
`12..16` the offset header; the final 6 bytes the array container's three
u16 values `02 00`, `05 00`, `64 00` — `2`, `5`, `100`, confirming the
bitmap round-trips the exact logical set this worked example started
from.

The corresponding `DeletionVectorRef`: `path =
"_strand/deletions/{segment_id}-{nonce}.bin"`, `byte_length = 22`,
`checksum = XxHash3_64::oneshot(&bytes)` (a real 64-bit value, computed at
write time, not hand-traced here — same discipline `segment::
write_segment` already uses for its own checksum field).

A query for row-id `1002` (`local_ordinal = 1002 - 1000 = 2`) against this
deletion vector: `is_deleted` decodes the bitmap, checks `contains(2)`,
returns `true` — row-id `1002` is filtered from any candidate set before
reranking, per `spec/vectors.md` §6 step 4.

## Napkin math (`CLAUDE.md` §7)

**GET count**: `+1` per segment with a present deletion vector, issued in
the same parallel wave as that segment's own open (no added sequential
latency, per Design §4). A cold query against `N` segments, `k` of which
have live deletions, costs `N` segment-open GET-sets plus `k` deletion-
vector GETs, all in one wave — not `N + k` sequential round trips.

**Bytes**: bounded by the segment's own `row_id_count`. At the worst
realistic case — every local ordinal tombstoned in a segment sized at
RFC 0010's own sizing law (~760,000 768d vectors) — the Roaring bitmap
degrades to its dense-array-container worst case, well under 1 MB
(760,000 ordinals in 32-bit containers is bounded by roughly
760,000/65,536 ≈ 12 containers, each at most an 8 KB bitmap-container
representation per `RoaringFormatSpec`'s own container-selection rule —
Roaring switches a container from array to bitmap representation past
4,096 elements specifically to bound this cost, `references/roaring-
bitmaps-container-operations.md`). This is a rounding error against the
100 MB cold-open byte budget (§7) even in the pathological all-deleted
case, and the realistic case (a small fraction of rows deleted between
compactions) is far smaller — no napkin-math risk to the budget this RFC
needs to flag as a real constraint.

## Alternatives considered

**Embed the deletion vector inside the segment's own container, as a
fifth blob type in the vector/lexical families.** Rejected outright:
segments are immutable objects (`spec/container.md` §1); a deletion vector
that lived inside one would require rewriting the *entire* segment on
every single delete — real, absurd write amplification the project's own
"write amplification is the writer's problem" framing (`spec/manifest.md`
§4 safety rules) does not excuse to this degree, and it would collapse
the deliberate distinction between "delete" (cheap, frequent) and
"compact" (expensive, batched, deferred) invariant 2 exists to preserve.

**One Roaring64 bitmap over the global row-ID space, shared across all
segments, instead of one 32-bit bitmap per segment.** Rejected: `spec/
filter-bitmaps.md` §3 already made this exact call for filter bitmaps and
this RFC follows it for consistency and for the same reason — the 64-bit
extension "lacks the 32-bit form's universal interoperability." A
per-segment, local-ordinal-indexed bitmap also composes naturally with
compaction (a compacted segment gets a fresh deletion vector built from
scratch against its own new local-ordinal space, never needing to
translate old global-row-id tombstones into a shared structure).

**Reuse `commit`'s existing `build_segments` closure by having it return
a full replacement segment list (append zero new segments, mutate one in
place) instead of adding `commit_deletion_vector`.** Rejected: `commit`'s
`added_row_ids` accounting (`current.next_row_id + added_row_ids`) is
computed by summing the *returned* segments' `row_id_count` — repurposing
it for an update-in-place call would either double-count an already-
counted segment's rows (if the caller re-returns it) or require a second,
parallel "which segments are actually new" signal bolted onto the same
closure, silently coupling two different operations (append vs. revise)
through one function signature. A separate, narrower function with its
own, simpler invariant ("exactly one existing entry's `deletion_vector`
field changes, nothing else") is safer to reason about and to eventually
verify formally (How this could be wrong, below) than a single
do-everything commit path.

## How this could be wrong

**Nearest grave: none of `docs/lineage.md`'s graveyard is about a missing
delete mechanism specifically** — the closer parallel is Iceberg/Delta's
own well-trodden position-delete-file pattern, which this RFC's design
(a small, standalone, superseding object referenced from the manifest,
distinct from the immutable data file) directly mirrors, for the same
reason those formats converged on it: mutable metadata needs a place to
live that isn't the immutable data file. The risk this RFC actually
carries is not a graveyard repeat but a **real, load-bearing formal-
verification gap, found by inspection, not assumed away**:
`verification/manifest.tla`'s `ProposeSnapshot` action models a segment as
`[base: Nat, count: Nat]` and its only state transition is `Append(
priorSegs, newSeg)` — there is no modeled transition for revising an
existing entry's fields while leaving the segment count and row-ID
allocation untouched. RFC 0002's Approval covers the *original* commit
protocol's action grammar; `commit_deletion_vector` is new, real,
unmodeled territory. This RFC does not extend the TLA+ model (Non-goals)
— it names the gap precisely enough that a future formal-verification
session does not need to rediscover it, matching this project's own
"verification rigor sequencing" discipline (tests are non-negotiable but
never substitute for formal methods for exactly this class of concurrent-
protocol question).

**A second, narrower risk**: `commit_deletion_vector`'s "superseding, not
accumulating" design (Design §3) means a caller MUST read the current
deletion vector before writing a new one, to avoid a lost-update race
where two concurrent deletes each read no prior tombstones and each write
a single-row bitmap, with the CAS retry loop only protecting the
*pointer* advance, not the read-modify-write of the bitmap's *content*
itself. This is not a bug this RFC's protocol introduces silently — it is
the same class of concern `commit`'s own `build_segments` closure already
has (`build_segments` is called fresh on every retry specifically so it
sees the current state each time) — but it is worth stating explicitly:
`commit_deletion_vector`'s `build_deletion_vector` closure MUST read the
current `deletion_vector` (if any) from the state `commit_deletion_
vector`'s own retry loop hands it, not from a value captured once outside
the retry loop, or concurrent deletes against the same segment can lose
tombstones. The implementation's own test suite must exercise this
directly (concurrent `commit_deletion_vector` calls against the same
segment), not merely trust the loop shape.

## Non-goals

- **Compaction-time physical removal of tombstoned rows.** M3 scope
  (`docs/milestones.md`); this RFC's deletion vectors are read-time
  filters only, never triggering deletion of the underlying segment data.
- **Retention-policy-driven expiry of old `DeletionVectorRef` objects.**
  Governed by the same not-yet-implemented table-metadata retention this
  RFC's Design §2 already notes is an existing, inherited gap (`spec/
  manifest.md` §1's "table metadata... not yet implemented").
- **The orphan-sweep tool's handling of superseded deletion-vector
  objects.** Falls out of the existing orphan rule (Design §2) but the
  sweep tool itself remains M3, unbuilt.
- ~~Extending `verification/manifest.tla` to model `commit_deletion_
  vector`'s new transition shape~~ — done, see Discussion below.
- **A TLAPS mechanized proof and a DST cross-validation harness covering
  the extended model** (RFC 0002's own remaining two artifacts). The TLA+
  model itself is extended and TLC-checked (Discussion, below); the proof
  and the trace-replay harness are real, separate, unstarted work.
- **Per-family deletion-vector caching or batching** (e.g. amortizing the
  extra GET across multiple queries against the same snapshot). A real,
  separate performance question once a real caller exists to measure it
  against.
- **Compaction-time deletion-vector merge semantics** — when segments
  merge (invariant 1), what happens to their respective deletion vectors
  is M3 compaction's own question (a merged segment's fresh local-ordinal
  space needs a fresh deletion vector built from the union of surviving,
  non-tombstoned rows) — named here so M3's own RFC does not need to
  rediscover that this question exists, not answered here.

## Discussion — post-approval amendments

Per `CLAUDE.md` §3, a design problem revealed after approval is recorded here
rather than folded silently into the model. This RFC's own "How this could be
wrong" named a genuine, unmodeled gap in `verification/manifest.tla`:
`ProposeSnapshot`'s only transition is `Append`, with no shape for revising an
existing entry in place, the exact thing `commit_deletion_vector` needs. Closed
here, the same session, prompted by the user's own recommendation to sequence
this before the DST harness or a TLAPS proof — sinking effort into either
against a model already known to be incomplete would mean redoing that work
once the model caught up.

**`SegmentRec` gained a `delVer: Nat` field** — a bare generation counter
standing in for a segment's `DeletionVectorRef` (`spec/deletion.md` §3): `0`
means no deletion vector committed yet, incrementing it models
`commit_deletion_vector`'s "supersede, don't accumulate" write (`spec/
deletion.md` §4). The model has no reason to represent actual Roaring-bitmap
content — none of this protocol's safety properties depend on *which* rows are
tombstoned, only on whether a revise-in-place commit can safely interleave,
through the shared pointer CAS, with the append-shaped commits every other
writer still performs.

**A new action, `ProposeDeletionVectorCommit(w)`**, guarded by a new CONSTANT
`DeleteWriter` (one member of `Writers`, the same established pattern
`DistinguishedWriter` already uses for varying one writer's shape without a
combinatorial per-writer `CONSTANTS` explosion): it revises the first segment's
`delVer` in place, leaving `nextRowId` and every segment's `base`/`count`
untouched, and appends nothing. `ProposeSnapshot` itself gained one line — `w #
DeleteWriter` — so the two shapes are mutually exclusive per writer, matching
the real code's two distinct top-level functions (`commit` vs.
`commit_deletion_vector`) sharing one CAS mechanic
(`propose_snapshot`/`TryAdvancePointer`/`ResolveAmbiguity`, all left completely
unchanged — they already operate generically on `wLocal[w].proposed`,
regardless of which action produced it).

**A real config mistake caught before it shipped, not after**: the first
version of this change pinned `DeleteWriter` to `w2` in the existing 2-writer
config (`Writers = {w1, w2}`), which silently *removed* coverage rather than
only adding it — with `w2` restricted to the revise shape, only `w1` remained
append-capable, eliminating the append-vs-append racing
(`commit_recomputes_row_id_range_when_a_rival_commits_first`'s own scenario
class) this model existed to check in the first place. Fixed by adding a third
writer (`Writers = {w1, w2, w3}`, `DeleteWriter = w3`) so the original
2-append-writer scenario is preserved exactly, with the new revise-shaped
writer layered on top of it, not swapped in for part of it.

**Two new invariants, both confirmed load-bearing by mutation test, not
assumed to hold merely by construction** — the same discipline every other
invariant in this file follows:

- `SegmentCountNeverDecreases`: segment count is monotonic non-decreasing
  across committed history. Before this action existed, every commit grew the
  count by exactly one segment, so this held trivially; a mutation that
  silently drops a segment while independently keeping `next_row_id`'s
  arithmetic consistent (folding the dropped segment's row count into a
  survivor, so the pre-existing `NextRowIdMatchesSegments` invariant stays
  satisfied) is caught by this invariant and *no other* — confirmed by
  running that exact mutation through TLC and observing the counterexample.
- `DeletionVectorCommitsOnlyReviseOneEntry`: between two consecutive
  snapshots whose segment count is unchanged (which, given the invariant
  above and that `ProposeSnapshot` always grows the count by exactly one, can
  only be a revise-shaped commit), every segment's `base`/`count` must be
  identical and at most one segment's `delVer` may differ. A mutation that
  revises *every* segment's `delVer` at once (instead of just the targeted
  one) passes every pre-existing invariant, including `NextRowIdMatchesSegments`,
  clean — and is caught by this invariant alone, confirmed the same way.

TLC re-verified clean: **5,943 distinct states (22,286 generated), depth 18**
(up from 591/1,793, depth 14), all nine invariants — the original seven plus
these two — holding. `verification/README.md` carries the new baseline. What
RFC 0002's own remaining scope still owes: a TLAPS proof and a DST
cross-validation harness, both against this now-extended model, neither
started here (Non-goals, above).
