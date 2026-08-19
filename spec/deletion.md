# Deletion vectors

Normative for STRAND v0.1. Defines the deletion-vector object — invariant 2's
general "no in-place mutation, deletes are deletion-vector blobs" machinery
(`CLAUDE.md` §5) — and its manifest-level reference and commit path. Approved
by RFC 0012 (`rfcs/0012-deletion-vectors.md`); this chapter states the settled
result — see the RFC for the worked example, alternatives considered, and the
adversarial review. Registered in `spec/container.md` §9: `family_id = 4`
("deletion"), `blob_type_id = 0` (deletion vector). Additively extends
`spec/manifest.md`'s `SegmentRef`.

Reference implementation: `crates/strand-core/src/deletion.rs` (object
format), `crates/strand-core/src/manifest.rs` (`SegmentRef.deletion_vector`,
`commit_deletion_vector`). Golden files: none yet;
`conformance/deletion/toy-deletion-vector.bin` is due once implemented.

## 1. Scope

A segment with no deleted rows has no deletion-vector object and no manifest
reference to one. A segment with at least one deleted row has exactly one
deletion-vector object, referenced by that segment's `SegmentRef` (§3).
Compaction-time physical removal of tombstoned rows, retention-policy-driven
expiry of superseded deletion-vector objects, and the orphan-sweep tool's
handling of them are M3 scope (`docs/milestones.md`), out of scope here.

## 2. The deletion-vector object

**Not a segment, not a container.** No footer, no hotcache, no blob registry
(`spec/container.md`'s machinery is for large, multi-blob, cold-openable
objects; a deletion vector is small, single-purpose, and frequently
superseded). The object's entire byte content is the serialized bitmap,
nothing else.

**Format**: the standard 32-bit Roaring format, exactly as `RoaringFormatSpec`
defines it (`references/roaring-format-spec-and-rust-crate.md`), under
`SERIAL_COOKIE_NO_RUNCONTAINER` — the identical MUST rule `spec/filter-
bitmaps.md` §3 already states (a writer MUST NOT emit run containers,
regardless of which container type its in-memory representation happens to
have chosen; a reader MUST still accept `SERIAL_COOKIE` run-container input,
for interop with bitmaps produced outside a conforming STRAND writer). The
64-bit Roaring extension MUST NOT be used, for the same interoperability
reason `spec/filter-bitmaps.md` §3 states.

**Indexing convention**: identical to `spec/filter-bitmaps.md` §3 — the
bitmap indexes local ordinals (`0` to `row_id_count - 1`, `spec/row-ids.md`
§1) of the one segment it belongs to, never the global 64-bit row-ID space
directly. A reader resolves membership via `local_ordinal = row_id -
row_id_base`. A segment declaring a deletion-vector object MUST satisfy
`row_id_count <= 2^32` — the exact cardinality the standard 32-bit Roaring
form can index, the same normative cap `spec/filter-bitmaps.md` §3 places on
its own blob family.

**This is a per-segment artifact, not a portable row-ID-space bitmap.** A
deletion vector's bytes are only ever interpreted against the one segment's
own fixed `row_id_base`/`row_id_count` — never reinterpreted against a
different segment's local-ordinal space, and never merged byte-for-byte with
another segment's deletion vector. `spec/row-ids.md` §3's "a deletion vector
marks row-IDs, not local ordinals, as tombstoned... this is why it survives a
merge... without needing a remap step of its own" is a claim about **logical
identity** — which rows are dead is stable, row-ID-identified information —
not about any one segment's bitmap bytes surviving a merge unchanged. At
merge time (M3), a compacted segment gets a freshly built deletion vector
from the union of its source segments' surviving (non-tombstoned) row-IDs,
re-encoded against the new segment's own local-ordinal space; old per-segment
bitmaps are superseded along with the segments they described, not
translated (§5).

## 3. Manifest reference (`spec/manifest.md`)

`SegmentRef` gains one new optional field:

| field             | type              | notes                                                    |
| ------------------ | ----------------- | ---------------------------------------------------------- |
| `deletion_vector`  | `DeletionVectorRef?` | absent iff no row in this segment has ever been deleted |

A `DeletionVectorRef`:

| field         | type   | notes                                                    |
| ------------- | ------ | ----------------------------------------------------------- |
| `path`        | string | the deletion-vector object's key                            |
| `byte_length` | u64    | the object's total size                                     |
| `checksum`    | u64    | xxHash3-64 over the object's bytes (invariant 11's default)  |

Shaped like `SegmentRef`'s own `path`/`byte_length`/`checksum` fields
deliberately: a reader fetches by path and verifies by checksum, with no
further indirection.

**Deletion-safety interaction**: `CLAUDE.md` §6's rule — a file MUST NOT be
physically deleted while any retained snapshot references it — applies to a
deletion-vector object exactly as it does to a segment object. A
`DeletionVectorRef` still named by a retained snapshot keeps its object alive
under the existing orphan-sweep accounting (`spec/manifest.md`); no new rule
is needed.

## 4. Commit path

`commit_deletion_vector` (`crates/strand-core/src/manifest.rs`):

```rust
pub fn commit_deletion_vector<S: ConditionalStore>(
    store: &S,
    segment_path: &str,
    build_deletion_vector: impl Fn(&SegmentRef) -> DeletionVectorRef,
) -> Result<SnapshotMetadata, CommitError>
```

Performs the same CAS retry loop `commit` uses (read current state, propose a
new snapshot, race the pointer, recompute and retry on loss), substituting
exactly one entry — the `SegmentRef` at `segment_path` (assumed unique within
the current segment set, per `segment::write_segment`'s own collision-panic
guarantee) — with its `deletion_vector` field replaced; `next_row_id`
unchanged; no segment appended or removed. `build_deletion_vector` is called
fresh on every retry, receiving that retry's current `SegmentRef` (row_id
range, and any existing `deletion_vector`) — the same role `commit`'s
`build_segments` gives `next_row_id`. This is what makes concurrent deletes
against the same segment safe: whichever attempt loses the pointer CAS
re-reads the winner's current state on its next iteration and recomputes
against it, rather than clobbering the winner's write with a stale union.
If `segment_path` names no segment in the current snapshot, `commit_deletion_
vector` returns `CommitError::SegmentNotFound(String)`.

**Superseding, not accumulating.** Each call's `build_deletion_vector`
closure MUST write the *complete* set of that segment's tombstoned local
ordinals — reading the segment's current deletion vector (if any), unioning
in the new tombstone(s), and writing that union as a fresh object under an
attempt-unique path — never a delta meant to be unioned with a predecessor
at read time.

## 5. Read-side integration

A reader resolves a segment's live deletion state by: if `SegmentRef.
deletion_vector` is present, fetch that object (one GET, checksum-verified
against `DeletionVectorRef.checksum` — a mismatch is a `DeletionError::
ChecksumMismatch` rather than silently decoding possibly-corrupt bytes),
decode it as a Roaring bitmap, and test `row_id - row_id_base` for
membership. This GET is issued in the same parallel wave as the segment's
own container open — it has no dependency on that segment's own
footer/hotcache bytes — so it costs one more GET and up to the deletion
vector's own byte size, not one more sequential round trip (invariant 3's
"≤2 round trips before query planning" bound is specifically about a
segment's own footer-then-hotcache sequence; a deletion-vector fetch is
manifest-level per-segment state resolved alongside, not part of, that
budget, the same way the pointer and snapshot GETs precede segment opens
without being part of any one segment's own open).

Each blob family's own query-resolution chapter states where in its own
steps this filter applies. `spec/vectors.md` §6 step 4 applies it to the
deduplicated candidate set the vector family's own scan (step 3) produces,
before reranking.

## 6. Conformance status

Implemented (`crates/strand-core/src/deletion.rs`, `crates/strand-core/src/
manifest.rs`, `crates/strand-vector/src/query.rs`'s `filter_deleted`). The
worked example's bytes are confirmed byte-for-byte against the real
`roaring` crate output (`crates/strand-core/src/deletion.rs`'s own
`round_trips_a_real_bitmap_through_build_and_decode` test). The
concurrent-delete race safety §4 describes is exercised directly, not just
argued: `crates/strand-core/src/manifest.rs`'s
`commit_deletion_vector_recomputes_against_a_concurrent_rivals_write_
without_losing_a_tombstone` injects a rival `commit_deletion_vector` call
mid-retry and confirms both writers' tombstones survive. `crates/
strand-vector/tests/deletion_end_to_end.rs` proves the full chain end to
end: a real segment committed through the real manifest CAS protocol, a
real deletion vector committed against it, and a real vector-family query
excluding the tombstoned row and promoting the runner-up. No golden files
yet — `conformance/deletion/toy-deletion-vector.bin` remains due.
