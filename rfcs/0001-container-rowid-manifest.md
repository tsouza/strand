# RFC 0001: Container format, row-ID space, and manifest

- **Status:** Draft
- **Milestone:** M0 — Container + manifest (`docs/milestones.md`)
- **Spec chapters produced:** `spec/container.md`, `spec/row-ids.md`, `spec/manifest.md`
- **Invariants exercised:** 1, 2, 3, 8, 10, 11 (`CLAUDE.md` §5); the manifest safety
  rules (`CLAUDE.md` §6)

## Summary

Defines the three pieces M0 gates on: the segment container's byte layout (footer,
hotcache, blob registry, the chunk/block split), the 64-bit row-ID space and how a
blob family's declared merge strategy relates to it, and the snapshot manifest's
compare-and-swap commit protocol and safety rules. Nothing else in the format —
lexical blobs, vector blobs, scoring profiles — can be specified until these three are
settled, because every later blob type is registered *in* this container and
addressed *through* this row-ID space.

## Motivation

`CLAUDE.md` §1 states the mission: chunk-shaped access, a small bounded number of
independent fetches, never dependent pointer-chasing. That promise is the container's
job to keep. Invariant 3 pins the number: opening a segment MUST cost at most two
round trips before query planning can begin, and after the open, every byte range a
cold query may need must already be addressable — the one-wave rule. Invariant 1
makes the row-ID space the format's fusion contract: a lexical blob and a vector blob
in the same segment must be able to say "this posting and this vector describe the
same row" without a translation layer. Invariant 8 says don't invent encodings for
this layer either — the design here borrows tantivy/Quickwit's footer-first hotcache
(`docs/lineage.md`) and Iceberg/Puffin's atomic-pointer-swap manifest with a
JSON-footer container pattern (`docs/lineage.md`), rather than inventing a new
technique for either.

## Non-goals

Lexical blob internals (postings, FST term dictionary, block-max) and vector blob
internals (RaBitQ codes, cluster navigation tier) are M1/M2 and out of scope here; this
RFC only defines how any blob — whatever family it belongs to — is registered,
addressed, and fetched. Scoring profiles and analyzer descriptors are M1. Cross-segment
pruning (R10) is explicitly open research, not a v0.1 feature, and this RFC does not
attempt it — the manifest here carries nothing that prunes segments at query time,
consistent with `docs/ledger.md`. The orphan-sweep *tool* is M3; this RFC states the
rule the tool will enforce (`CLAUDE.md` §6) but does not implement it.

## Design

### 1. Segment container layout

A segment is one object in object storage, laid out data-first, metadata-last, so a
reader never needs to know the file's total structure before it starts reading — only
its size, which object storage headers give for free:

```
[ 0 .......................... data region: one or more blob regions, back to back ]
[ .......................... hotcache region: row-ID range + blob registry ]
[ .......... footer trailer: fixed 40 bytes, always the file's last 40 bytes ]
```

**Footer trailer (fixed 40 bytes, little-endian per invariant 11):**

| offset | size | field | value |
|---|---|---|---|
| 0 | 4 | magic | ASCII `STRD` |
| 4 | 2 | format_major | u16 |
| 6 | 2 | format_minor | u16 |
| 8 | 8 | hotcache_offset | u64, byte offset from file start |
| 16 | 8 | hotcache_length | u64 |
| 24 | 1 | checksum_algo | u8, `1` = xxHash3-64 (invariant 11 default) |
| 25 | 7 | reserved | zero |
| 32 | 8 | footer_checksum | u64, checksum_algo over bytes [0, 32) |

**Open protocol (invariant 3's ≤2-RTT budget).** A reader issues an HTTP suffix range
request, `Range: bytes=-N`, for a speculative tail size `N` — a **reader-side tuning
parameter, not a format constant**, so no vendor- or deployment-specific number is
baked into wire bytes (the Optane lesson, `docs/lineage.md`). Suffix ranges are
standard HTTP (RFC 7233 §2.1) and every major object store honors them, so this costs
one GET without a prior HEAD for file size. The last 40 bytes of the response are the
footer trailer. If `hotcache_offset` falls within the fetched window, the hotcache is
already in hand — **one RTT, the common case**. If not, the reader issues one more
range GET for `[hotcache_offset, hotcache_offset + hotcache_length)` — the **second
and last RTT** the invariant allows. Either way, by the time the open completes, the
blob registry and the row-ID range are fully resident, and invariant 3's one-wave rule
holds for everything that follows: no further offset lookup ever costs a round trip.

**Hotcache region** (the navigation tier fetched wholesale at open):

```
row_id_base:   u64
row_id_count:  u64
blob_count:    u32
blob_entry[blob_count]:
  family_id:          u16   (registry-assigned: lexical, vector, ...)
  blob_type_id:        u16   (registered codec ID within the family)
  storage_class:        u8    (0 = chunk-compressed, 1 = raw-mappable; invariant 10)
  tier:                  u8    (0 = n/a, 1 = cold-fetchable, 2 = warm; invariant 7)
  alignment:              u16   (power-of-two; raw-mappable blobs only)
  chunk_codec:             u8    (0 = none, 1 = zstd; invariant 11 default)
  chunk_codec_level:        u8
  offset:                    u64   (byte offset within the segment file)
  length:                     u64
  checksum:                    u64   (checksum_algo over the blob's on-disk bytes)
```

Per invariant 10, a `chunk-compressed` blob's internal chunk offset table (chunk
lengths, per-chunk checksums, the mapping from chunk index to byte range) is part of
that blob's own region, not the container footer — the container only needs to know
where the blob starts and ends; a specific blob family's chunk index is that family's
spec chapter's concern (M1/M2). A `raw-mappable` blob has no internal chunk table at
all: its bytes are addressed directly at the declared `alignment`.

### 2. Row-ID space

Each segment declares a contiguous row-ID range `[row_id_base, row_id_base +
row_id_count)` in its hotcache, assigned by the writer at build time. Within the
segment, local ordinal `i` (for `i` in `[0, row_id_count)`) maps to row-ID
`row_id_base + i`; every blob family that stores per-row data dense-indexed by local
ordinal (a flat vector blob, a lexical doc-length array) uses this same mapping, so no
family needs its own ID table.

Global uniqueness across all segments in one index is a manifest-level property, not a
container-level one: a writer reserves its range as part of a manifest commit (§3
below), and the CAS commit protocol makes range reservation atomic — two writers
racing the pointer cannot both claim the same range, because only one wins the CAS.

**What "stable" buys, concretely**, resolving invariant 1's per-family merge
strategies against this scheme:

- **Concatenate + remap** (IVF/SPANN posting lists): a merge concatenates the source
  segments' posting lists into a new segment's storage, and *remaps* each entry's
  internal position (its offset into the new segment's dense arrays) — but the row-ID
  values the entries reference are copied through unchanged. This is exactly the
  saving stable row-IDs buy: a merge rewrites *pointers*, never *identities*.
- **Rebuild** (graph indexes): the merged segment's graph is built from scratch over
  the union of surviving row-IDs; row-IDs are inputs to the rebuild, not preserved
  structure.
- **Rebalance** (centroid layers): row-IDs move between clusters as centroids shift,
  but the row-ID values themselves are unchanged — only which posting list currently
  contains a given row-ID.

A deletion vector (Roaring, invariant 2) marks row-IDs, not local ordinals, as
tombstoned, so it survives a merge that remaps local ordinals without needing its own
remap step.

### 3. Manifest and CAS commit protocol

Three object kinds, all under a `_strand/` prefix inside the index's root:

- **Table metadata** (`_strand/metadata.json`, written once, immutable thereafter
  except for the declared amendments below): format version, the declared CAS host
  (`{"type": "native", "store": "s3"}` or `{"type": "catalog", "uri": "..."}` —
  `CLAUDE.md` §6's "one declared CAS host" rule), minimum snapshot retention (a count,
  a duration, or both).
- **Snapshot metadata** (`_strand/snapshots/{version:020}.json`, immutable, one per
  committed version): the segment set, each entry giving the segment's path, its
  row-ID range, byte length, and checksum; and, per the Lance model cited in
  `docs/lineage.md`, a reference to each blob family's index version *without*
  embedding that family's internal structure (index-aware, index-internals-agnostic).
- **Current pointer** (`_strand/current`): the single object every reader and writer
  reads first. Its content is the current snapshot's version number.

These are JSON, not the container's binary format — invariant 11's byte-determinism
pins (endianness, checksum algorithm, codec-variant registration) govern *wire
structures a reader decodes into a fixed byte layout*; a JSON manifest is decoded by a
JSON parser, and its determinism problem is a different, smaller one (stable key
ordering is not required for correctness, only for human diffability, which JSON
gives for free when keys are written in a fixed, documented order). This mirrors
Puffin's "opaque typed blobs with a JSON footer" pattern (`docs/lineage.md`): binary
where bytes are read by a codec on the hot path, JSON where humans and cross-engine
tooling read it.

**Commit protocol**, on a store with native conditional writes (S3, confirmed;
GCS/Azure header semantics are R5, open — see below):

1. Create the new snapshot metadata object at its versioned path with
   `If-None-Match: *` (fails only if that exact version was already written, which
   would mean a version-number collision, not a race — versions are chosen by reading
   the current pointer first).
2. Advance the current pointer: `PUT _strand/current` with `If-Match: <etag last
   read>`. Success means this writer's commit is now current. A `412 Precondition
   Failed` means another writer landed first; this writer re-reads `_strand/current`,
   re-derives a new version number and (if it reserved a row-ID range) a fresh range
   past the winner's, and retries from step 1. The orphaned snapshot object from the
   losing attempt is harmless per the orphan-file rule (`CLAUDE.md` §6) and is swept
   later, at M3.
3. For the very first commit against a fresh table, `_strand/current` does not exist
   yet: the writer uses `If-None-Match: *` on the create, and a losing writer detects
   this the same way — a failed precondition — and retries by reading the (now
   existing) pointer.

**Reader protocol:**

1. `GET _strand/current` — the version number.
2. `GET _strand/snapshots/{version}.json` — the segment set.
3. Open each referenced segment per §1, in parallel across segments.

**Safety rules** (`CLAUDE.md` §6, restated here as this RFC's obligations): a segment
file is never physically deleted while any retained snapshot (per the table metadata's
retention policy) references it; a reader that gets 404 on a segment its snapshot
references re-fetches `_strand/current` and retries rather than reporting corruption;
orphaned segment and snapshot objects — left behind by a writer that crashed or lost a
CAS race — are swept by listing the prefix and subtracting everything referenced by a
retained snapshot (tool lands at M3; the rule is normative now so the M3 tool has
nothing to invent).

## Worked example

A toy segment holding two rows (row-IDs 1000 and 1001) and one raw-mappable blob
storing two little-endian `u32` values, `42` and `43`, 8-byte aligned.

**Data region** (file offset 0, 8 bytes):

```
2A 00 00 00  2B 00 00 00
```

**Hotcache region** (offset 8, 54 bytes):

```
row_id_base   (u64) = 1000        → E8 03 00 00 00 00 00 00
row_id_count  (u64) = 2           → 02 00 00 00 00 00 00 00
blob_count    (u32) = 1           → 01 00 00 00

blob_entry[0]:
  family_id          (u16) = 0    → 00 00
  blob_type_id        (u16) = 0    → 00 00
  storage_class          (u8)  = 1    → 01            (raw-mappable)
  tier                     (u8)  = 0    → 00            (n/a)
  alignment                 (u16) = 8    → 08 00
  chunk_codec                 (u8)  = 0    → 00            (none)
  chunk_codec_level             (u8)  = 0    → 00
  offset                          (u64) = 0    → 00 00 00 00 00 00 00 00
  length                            (u64) = 8    → 08 00 00 00 00 00 00 00
  checksum                            (u64) = xxHash3-64(data region bytes above)
                                                    → computed by the reference
                                                       implementation; not hand-derived
                                                       here, consistent with §2: no
                                                       number ships without being
                                                       actually computed from a source,
                                                       and hand-arithmetic on a
                                                       non-trivial hash is not a source.
```

**Footer trailer** (offset 62, 40 bytes):

```
magic            = "STRD"        → 53 54 52 44
format_major     = 0             → 00 00
format_minor     = 1             → 01 00
hotcache_offset  = 8              → 08 00 00 00 00 00 00 00
hotcache_length  = 54             → 36 00 00 00 00 00 00 00
checksum_algo    = 1 (xxHash3-64) → 01
reserved (7 bytes, zero)         → 00 00 00 00 00 00 00
footer_checksum  = xxHash3-64(bytes[0,32)) → computed by the reference implementation
```

Total file size: 102 bytes. A reader requesting the last 4096 bytes (any speculative
size larger than the file) gets the whole file in one GET — the common case, one RTT.
A production segment's hotcache will not fit an arbitrarily small speculative window
once blob and chunk counts grow; that failure mode is addressed in "How this could be
wrong" below.

## Napkin math (`CLAUDE.md` §7)

End-to-end cold path, from the pointer read, using the pinned ~100ms per-round-trip
planning figure:

| step | round trips | notes |
|---|---|---|
| `GET _strand/current` | 1 | pointer read |
| `GET` snapshot metadata | 1 | segment set, O(segments) bytes |
| open each referenced segment | ≤2, in parallel across segments | invariant 3 |

Wall time: the two manifest GETs are sequential (the snapshot path depends on the
pointer's content) — 2 × ~100ms — then segment opens run in parallel across however
many segments the snapshot references, so their contribution to wall time is one
segment-open's worth (≤2 × ~100ms), not N segment-opens' worth. Total: **~300–400ms**
structured cold-path wall time, independent of segment count. This lands in the same
range as turbopuffer's stated "often as little as ~400ms" structured cold path
(`docs/benchmarks.md`), which is the right comparison — both figures count from the
pointer/metadata trip, per `CLAUDE.md` §7's rule that arithmetic starting after
metadata is in hand is an engine's accounting, not a format's.

Bytes: the pointer object is a few dozen bytes; snapshot metadata is O(segments) —
unmeasured at this stage, flagged for M0 benchmark data rather than asserted; each
segment's hotcache is small (tens to low hundreds of bytes per blob entry) and is
**not** the same budget as the 100 MB cold-open byte budget (`CLAUDE.md` §7) — that
budget bounds a `tier: cold-fetchable` vector blob's navigation-tier-plus-codes
payload once such a blob is registered (M2), not the container's own footer/hotcache
metadata, which this RFC expects to stay in the low kilobytes for realistic segments
and will confirm or correct against M0 measurement.

## Invariant-11 checklist

- **Endianness:** little-endian, pinned for the footer trailer and hotcache region.
- **Term sort order:** not applicable — no term dictionary exists at this layer (M1).
- **Chunk codec:** declared per blob in the registry entry (`chunk_codec`,
  `chunk_codec_level`); default zstd, `chunk_codec = 0` (none) permitted for
  raw-mappable blobs, which by definition carry no chunk codec.
- **Checksums:** xxHash3-64 default (`checksum_algo` field), applied to the footer
  trailer and to each blob's on-disk bytes at the container level; a chunk-compressed
  blob's own per-chunk checksums (invariant 11's "every chunk carries a declared
  checksum over its uncompressed content") are that blob family's concern, inside its
  region, not duplicated here.
- **Codec-variant provenance:** not applicable to container-level structures; the
  chunk compression codec (zstd, with level) is fully named per blob, satisfying the
  "complete registration" requirement for *this* layer's only codec use.
- **Stochastic-transform provenance:** not applicable — no stochastic transform exists
  at this layer (RaBitQ rotation is M2).
- **Golden files:** this RFC's worked example, once implemented, becomes the first
  `conformance/` golden file: uncompressed hotcache and footer bytes, byte-for-byte,
  per invariant 11's rule that golden files pin uncompressed structures.

## How this could be wrong

**Footer/hotcache size growth breaks the ≤2-RTT budget.** The worked example is 102
bytes; a real segment with many blob families and, later, per-family internal chunk
indices sitting just inside their own blob regions (not the hotcache) should keep the
hotcache itself small — but nothing in this design *caps* hotcache size, and if a
future blob family's registry entry grows (say, per-blob summary statistics get added
under R10's cross-segment pruning question), the hotcache could grow past any fixed
speculative-read window, silently degrading every open from one RTT to two, or, if the
hotcache itself exceeds the second GET's assumptions, breaking the budget outright.
Nearest grave: the Optane-era formats (`docs/lineage.md`) — baking a fixed size into
wire bytes would hardcode today's assumptions about "small." This RFC avoids that by
keeping the speculative tail size a reader parameter, not a format constant, but does
not yet state a target hotcache size ceiling; that number should come from M0
benchmark data, not be guessed here, per §2.

**Row-ID range reservation is a coordination point.** Because global uniqueness is
enforced by the CAS commit (a writer's range is only real once its commit wins), many
concurrent writers targeting the same table serialize on the pointer for range
reservation, not just for visibility. This is a real throughput cost this RFC
surfaces but does not solve — consistent with `CLAUDE.md` §6's "write amplification is
the writer's problem," a high-throughput writer can reserve a larger range per commit
to amortize contention, an engineering choice the format doesn't need to make for it.

**Inventing manifest semantics nobody else runs.** The commit protocol here is close
to a direct copy of Iceberg's, deliberately — the nearest grave for a *bespoke*
protocol is Indri/Galago (`docs/lineage.md`): a well-specified format that dies with
no engine pressured to keep implementing it. Departing from Iceberg's shape without a
strong reason would reopen that risk; this RFC does not depart from it.

**GCS/Azure are unverified.** This RFC's commit protocol is written and reasoned about
against S3's confirmed conditional-write semantics (If-None-Match GA August 2024,
If-Match ETag CAS November 2024, per `docs/research/README.md` R5). GCS generation-match
and Azure ETag conditionals are long-standing but their exact header semantics are not
yet confirmed against primary sources — R5, open. This RFC's external-catalog fallback
(declared via `cas_host.type = "catalog"`) is the escape hatch for stores without
native conditional writes, but its own protocol is not detailed here and needs its own
follow-on RFC once a catalog implementation is in scope.

## Alternatives considered

**Single-file container instead of one object per segment plus a separate manifest.**
Rejected: a segment must be independently fetchable without touching unrelated blob
families (invariant 3), and a single growing file per index would make the CAS commit
protocol either impossible (can't atomically swap part of a file) or require the same
manifest-over-immutable-files structure anyway, at higher blast radius per write.

**Row-ID as a content hash instead of an assigned sequential range.** Rejected: every
family that dense-indexes per-row data by local ordinal (flat vectors, doc-length
arrays) needs an efficient row-ID-to-storage-position mapping; a sparse hash-based ID
space would require the same range-table indirection this design already has, with no
offsetting benefit for this format's access patterns, and would give up the cheap
`local_ordinal = row_id - row_id_base` arithmetic this design gets for free.

**A WAL or memtable-backed manifest for lower write latency.** Rejected outright by
`CLAUDE.md` §6: "the format ships no WAL and no memtable; a production writer batches
on its own side." Not reconsidered here.

## Open questions / follow-on RFCs

- Hotcache size ceiling and the default speculative tail-read size — needs M0 MinIO
  benchmark data before either is stated as more than provisional.
- GCS/Azure conditional-write header semantics and the external-catalog fallback
  protocol (R5) — follow-on RFC once a non-S3 target or catalog is in scope.
- Whether the manifest should eventually carry optional per-segment summary metadata
  for cross-segment pruning is R10 and stays explicitly out of this RFC's scope.
