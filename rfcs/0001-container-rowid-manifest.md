# RFC 0001: Container format, row-ID space, and manifest

- **Status:** Implemented — passed adversarial review (invariant compliance,
  byte-arithmetic, protocol/concurrency, and citation-accuracy passes); every
  blocking finding fixed and grounded against fetched primary sources. Residual
  non-blocking items are tracked in "Open questions / follow-on RFCs" below.
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

A segment is one object in object storage, laid out data-first, metadata-last. A
reader opening a segment through the manifest protocol (§3) already knows its exact
byte length before the first GET — the snapshot metadata records each segment's
`byte_length` — so the footer read can be an ordinary, explicitly-bounded range
request rather than depending on suffix-range support that isn't confirmed for every
target store; see the open protocol below.

```
[ 0 .......................... data region: one or more blob regions, back to back ]
[ .......................... hotcache region: row-ID range + blob registry ]
[ .......... footer trailer: fixed 40 bytes, always the file's last 40 bytes ]
```

**Footer trailer (fixed 40 bytes, little-endian per invariant 11):**

| offset | size | field           | value                                       |
| ------ | ---- | --------------- | ------------------------------------------- |
| 0      | 4    | magic           | ASCII `STRD`                                |
| 4      | 2    | format_major    | u16                                         |
| 6      | 2    | format_minor    | u16                                         |
| 8      | 8    | hotcache_offset | u64, byte offset from file start            |
| 16     | 8    | hotcache_length | u64                                         |
| 24     | 1    | checksum_algo   | u8, `1` = xxHash3-64 (invariant 11 default) |
| 25     | 7    | reserved        | zero                                        |
| 32     | 8    | footer_checksum | u64, checksum_algo over bytes [0, 32)       |

**Open protocol (invariant 3's ≤2-RTT budget).** Because the reader already has
`byte_length` from the snapshot metadata, the first GET is an ordinary range request
with an explicit end, `Range: bytes={byte_length-N}-{byte_length-1}`, for a
speculative tail size `N` — a **reader-side tuning parameter, not a format
constant**, so no vendor- or deployment-specific number is baked into wire bytes (the
Optane lesson, `docs/lineage.md`). A reader MUST clamp N to byte_length — the
request's first byte position is max(0, byte_length − N) — since a first position
below zero is not a valid byte range (RFC 9110 §14.1.2 defines that clamping only
for the suffix form this protocol does not use). This deliberately avoids relying on HTTP
suffix-range syntax (`bytes=-N`, no explicit end): RFC 9110 §14.1.2 fully defines the
suffix-range form as standard HTTP (`references/rfc9110-range-requests.txt`, vendored
and read directly, not assumed), so the *protocol* is not in question — what is
unconfirmed is whether S3 and MinIO's *server-side* implementations honor that form.
AWS's own `GetObject` "Range" parameter documentation demonstrates only the
explicit-end form in its examples and points readers to RFC 9110 §14.2 "Range" — which
obsoletes RFC 7233 — for the header's semantics, but an absent example is not evidence
either way. An explicit-end range has no such gap, since it is the form AWS's own
documentation exercises directly. The last 40 bytes of the response are the
footer trailer. Because the hotcache always ends immediately before the footer, at
`byte_length - 40`, a single check — `hotcache_length + 40 <= N` — is sufficient to
guarantee the *entire* hotcache (not merely its start) landed inside the fetched
window: **one RTT, the common case**. If that check fails, the reader issues one more
range GET for `[hotcache_offset, byte_length - 40)` — the **second and last RTT** the
invariant allows. Either way, by the time the open completes, the blob registry and
the row-ID range are fully resident, and invariant 3's one-wave rule holds for
everything that follows: no further offset lookup ever costs a round trip.

A tool that opens a segment directly, outside the manifest protocol (`strand-tools
inspect` given a bare path, say), does not have `byte_length` for free and needs a
`HEAD` request or independently confirmed suffix-range support to get it. That path
is out of scope for invariant 3's budget, which binds the query-serving path only,
where the manifest is always read first.

**Hotcache region** (the navigation tier fetched wholesale at open):

| field                  | type                                | notes       |
| ---------------------- | ----------------------------------- | ----------- |
| row_id_base            | u64                                 |             |
| row_id_count           | u64                                 |             |
| blob_count             | u32                                 |             |
| blob_entry[blob_count] | struct, repeated `blob_count` times | table below |

**`blob_entry` fields:**

| field             | type | notes                                                |
| ----------------- | ---- | ---------------------------------------------------- |
| family_id         | u16  | registry-assigned: lexical, vector, ...              |
| blob_type_id      | u16  | registered codec ID within the family                |
| storage_class     | u8   | 0 = chunk-compressed, 1 = raw-mappable; invariant 10 |
| tier              | u8   | 0 = n/a, 1 = cold-fetchable, 2 = warm; invariant 7   |
| alignment         | u16  | power-of-two; raw-mappable blobs only                |
| chunk_codec       | u8   | 0 = none, 1 = zstd; invariant 11 default             |
| chunk_codec_level | u8   |                                                      |
| offset            | u64  | byte offset within the segment file                  |
| length            | u64  |                                                      |
| checksum          | u64  | checksum_algo over the blob's on-disk bytes          |

Per invariant 10, a `chunk-compressed` blob's internal chunk offset table (chunk
lengths, per-chunk checksums, the mapping from chunk index to byte range) is part of
that blob's own region, not the container footer — the container only needs to know
where the blob starts and ends; a specific blob family's chunk index is that family's
spec chapter's concern (M1/M2). A `raw-mappable` blob has no internal chunk table at
all: its bytes are addressed directly at the declared `alignment`.

The registry entry's `checksum` field is scoped differently depending on
`storage_class`, and this distinction matters for invariant 11. For a `raw-mappable`
blob, on-disk bytes *are* the uncompressed content, so `checksum` is fully
deterministic across conformant implementations and participates in byte-for-byte
golden-file comparison like every other hotcache field. For a `chunk-compressed`
blob, on-disk bytes are compressed, and invariant 11 already states that compressed
chunk bytes "may vary across compressor versions and are verified by checksum and
round-trip, not byte-comparison" — the same exception applies one level up here:
the registry entry's `checksum` *value* is excluded from byte-exact golden-file
comparison for chunk-compressed blobs (verified instead by recomputing it against the
actual stored bytes and confirming the recomputed value matches, which catches
corruption without demanding two different zstd builds produce identical compressed
output). What invariant 11 pins is that the field is present, little-endian, and
computed with the declared `checksum_algo` — not a specific value.

### 2. Row-ID space

Each segment declares a contiguous row-ID range `[row_id_base, row_id_base +
row_id_count)` in its hotcache, assigned by the writer at build time. Within the
segment, local ordinal `i` (for `i` in `[0, row_id_count)`) maps to row-ID
`row_id_base + i`; every blob family that stores per-row data dense-indexed by local
ordinal (a flat vector blob, a lexical doc-length array) uses this same mapping, so no
family needs its own ID table.

Global uniqueness across all segments in one index is a manifest-level property, not a
container-level one: a writer *proposes* a range read from the current snapshot's
`next_row_id` cursor (§3 below), but that proposal is only real once its commit wins
the pointer CAS — a writer that loses the race re-reads the winner's `next_row_id` and
recomputes its range before retrying, so two writers never end up holding the same
range as of any snapshot a reader can actually see.

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
  except for the CAS-host move, the one declared amendment (`CLAUDE.md` §6)): format
  version, the declared CAS host
  (`{"type": "native", "store": "s3"}` or `{"type": "catalog", "uri": "..."}` —
  `CLAUDE.md` §6's "one declared CAS host" rule), minimum snapshot retention (a count,
  a duration, or both).
- **Snapshot metadata** (`_strand/snapshots/{version:020}-{writer_nonce}.json`,
  immutable, one per *proposed* commit — see the filename rationale below): the
  `version` (u64) this snapshot represents, so a writer can compute the next version
  as `version + 1` from the snapshot's own content rather than parsing it out of a
  path; a `next_row_id` cursor (u64, one past the highest row-ID any referenced
  segment claims — an O(1) read for the next writer, instead of scanning every
  segment's range); the segment set, each entry giving the segment's path, its
  row-ID range, `byte_length`, and checksum; and, per the Lance model cited in
  `docs/lineage.md`, a reference to each blob family's index version *without*
  embedding that family's internal structure (index-aware, index-internals-agnostic).
- **Current pointer** (`_strand/current`): the single object every reader and writer
  reads first. Its content is the path (key) of the current snapshot metadata object.

These are JSON, not the container's binary format — invariant 11's byte-determinism
pins (endianness, checksum algorithm, codec-variant registration) govern *wire
structures a reader decodes into a fixed byte layout*; a JSON manifest is decoded by a
JSON parser, and its determinism problem is a different, smaller one (stable key
ordering is not required for correctness, only for human diffability, which JSON
gives for free when keys are written in a fixed, documented order). This mirrors
Puffin's "opaque typed blobs with a JSON footer" pattern (`docs/lineage.md`): binary
where bytes are read by a codec on the hot path, JSON where humans and cross-engine
tooling read it.

**Why the snapshot filename carries a `writer_nonce`.** Apache Iceberg's own
optimistic-concurrency shape is the model here, verified against its spec directly:
"Writers create table metadata files optimistically, assuming that the current
version will not be changed before the writer's commit... If the snapshot on which an
update is based is no longer current, the writer must retry the update based on the
new current version" (`apache/iceberg`, `format/spec.md`). That retry is only cheap
if *proposing* a new metadata object never itself collides between two writers — and
a bare `{version:020}.json` path can collide, because two writers reading the same
`current` independently compute the same next version number and race to create the
identical path, a case §"How this could be wrong" below discusses. Appending a random
`writer_nonce` (for example, 8 bytes of random hex) to the snapshot filename removes
that collision entirely: every writer's proposed object has a distinct path, so the
`If-None-Match: *` create in step 1 always succeeds, and the *only* place writers
actually contend is the pointer CAS in step 2 — matching Iceberg's documented shape,
where conflict is detected and resolved at the pointer swap, not at file creation.

**Commit protocol**, on a store with native conditional writes (S3, confirmed;
GCS/Azure header semantics are R5, open — see below):

1. Read `_strand/current` (a snapshot path), then `GET` that snapshot's `version`
   and `next_row_id` — or, for a table's very first commit, treat both as absent.
   Derive this attempt's version number (`version + 1`, or `0` if absent), row-ID
   range (`[next_row_id, next_row_id + count)`, or `[0, count)` if absent), and a
   fresh random `writer_nonce`.
2. Write the new segment(s), then create the snapshot metadata object at
   `_strand/snapshots/{version:020}-{writer_nonce}.json` with `If-None-Match: *`.
   Because the nonce makes this path unique to this attempt, this create does not
   contend with other writers and is not expected to fail.
3. Advance the current pointer: `PUT _strand/current` with `If-Match: <etag last
   read>` (or `If-None-Match: *` if step 1 found no existing pointer), pointing at the
   object just written in step 2. Success means this writer's commit is now current.
   A `412 Precondition Failed` means another writer landed first: this writer
   re-reads `_strand/current` and the winner's snapshot metadata, re-derives its
   version number and row-ID range from *that* fresh state (its previously proposed
   range may now overlap the winner's and MUST be recomputed, not reused — this is
   the "rerun the conflict check" step Iceberg's spec describes), writes a new
   snapshot object under a new nonce, and retries from step 3. The orphaned snapshot
   object from the losing attempt is harmless per the orphan-file rule (`CLAUDE.md`
   §6) and is swept later, at M3.

**Reader protocol:**

1. `GET _strand/current` — the current snapshot's path.
2. `GET` that snapshot metadata object — the segment set and `next_row_id`.
3. Open each referenced segment per §1, in parallel across segments.

A `404` at either step means the referenced object was removed by compaction between
this reader's two GETs (`CLAUDE.md` §6's deletion-safety rule bounds *when* that can happen, not
*whether* a reader can lose this particular race). Per the safety rules below, the
reader treats this as an expired snapshot: refresh `_strand/current` and retry the
whole sequence, capped at a small, implementation-chosen retry limit — the exact
count is a reader parameter this RFC does not pin, for the same reason `N` in §1 is
unpinned, but the requirement itself is not optional: an unbounded retry loop is not
a conforming reader, and past the limit a reader MUST surface an error rather than
loop forever.

**Safety rules** (`CLAUDE.md` §6, restated here as this RFC's obligations): a segment
file *and* a snapshot metadata object are never physically deleted while any retained
snapshot (per the table metadata's retention policy) references them; a reader that
gets 404 on either — not only a segment file — refreshes and retries, bounded, as
just described, rather than reporting corruption; orphaned segment and snapshot
objects — left behind by a writer that crashed or lost the pointer CAS — are swept by
listing the prefix and subtracting everything referenced by a retained snapshot (tool
lands at M3; the rule is normative now so the M3 tool has nothing to invent). The
"one declared CAS host" rule (`CLAUDE.md` §6) is, as written, a conformance
requirement on writers, not a mechanism this protocol enforces against a
misconfigured or malicious writer — no fencing token or cross-host detection is
specified. This is not unique to STRAND: Iceberg's own model relies on writers using
the catalog it was configured with, and nothing in that model cryptographically stops
a writer from bypassing it either. Enforcement beyond convention is out of scope here
and not a v0.1 problem this RFC solves.

## Worked example

A toy segment holding two rows (row-IDs 1000 and 1001) and one raw-mappable blob
storing two little-endian `u32` values, `42` and `43`, 8-byte aligned.

**Data region** (file offset 0, 8 bytes):

```
2A 00 00 00  2B 00 00 00
```

**Hotcache region** (offset 8, 54 bytes):

| field        | type | value | bytes (little-endian)     |
| ------------ | ---- | ----- | ------------------------- |
| row_id_base  | u64  | 1000  | `E8 03 00 00 00 00 00 00` |
| row_id_count | u64  | 2     | `02 00 00 00 00 00 00 00` |
| blob_count   | u32  | 1     | `01 00 00 00`             |

`blob_entry[0]`:

| field             | type | value                               | bytes (little-endian)                    |
| ----------------- | ---- | ----------------------------------- | ---------------------------------------- |
| family_id         | u16  | 0                                   | `00 00`                                  |
| blob_type_id      | u16  | 0                                   | `00 00`                                  |
| storage_class     | u8   | 1 (raw-mappable)                    | `01`                                     |
| tier              | u8   | 0 (n/a)                             | `00`                                     |
| alignment         | u16  | 8                                   | `08 00`                                  |
| chunk_codec       | u8   | 0 (none)                            | `00`                                     |
| chunk_codec_level | u8   | 0                                   | `00`                                     |
| offset            | u64  | 0                                   | `00 00 00 00 00 00 00 00`                |
| length            | u64  | 8                                   | `08 00 00 00 00 00 00 00`                |
| checksum          | u64  | xxHash3-64(data region bytes above) | computed by the reference implementation |

The `checksum` and, below, `footer_checksum` bytes are left as "computed by the
reference implementation" rather than hand-derived: consistent with §2, no number
ships without actually being computed from a source, and hand-arithmetic on a
non-trivial hash is not a source.

**Footer trailer** (offset 62, 40 bytes):

| field           | value                   | bytes                                    |
| --------------- | ----------------------- | ---------------------------------------- |
| magic           | "STRD"                  | `53 54 52 44`                            |
| format_major    | 0                       | `00 00`                                  |
| format_minor    | 1                       | `01 00`                                  |
| hotcache_offset | 8                       | `08 00 00 00 00 00 00 00`                |
| hotcache_length | 54                      | `36 00 00 00 00 00 00 00`                |
| checksum_algo   | 1 (xxHash3-64)          | `01`                                     |
| reserved        | 0 (7 bytes)             | `00 00 00 00 00 00 00`                   |
| footer_checksum | xxHash3-64(bytes[0,32)) | computed by the reference implementation |

Total file size: 102 bytes. A reader whose speculative window is larger than the file
clamps it to byte_length per §1's open protocol and gets the whole 102-byte file in
one GET — the common case, one RTT.
A production segment's hotcache will not fit an arbitrarily small speculative window
once blob and chunk counts grow; that failure mode is addressed in "How this could be
wrong" below.

## Napkin math (`CLAUDE.md` §7)

End-to-end cold path, from the pointer read, using the pinned ~100ms per-round-trip
planning figure:

| step                         | round trips                     | notes                          |
| ---------------------------- | ------------------------------- | ------------------------------ |
| `GET _strand/current`        | 1                               | pointer read                   |
| `GET` snapshot metadata      | 1                               | segment set, O(segments) bytes |
| open each referenced segment | ≤2, in parallel across segments | invariant 3                    |

Wall time: the two manifest GETs are sequential (the snapshot path depends on the
pointer's content) — 2 × ~100ms — then segment opens run in parallel across however
many segments the snapshot references, so their contribution to wall time is one
segment-open's worth (≤2 × ~100ms), not N segment-opens' worth. Total: **~300–400ms**
structured cold-path wall time, independent of segment count — this is STRAND's own
unmeasured napkin-math estimate, not yet an M0-measured number.

`docs/benchmarks.md` gives two turbopuffer figures, and it is explicit about which
one this estimate should be judged against: **874ms p50 for a truly cold namespace**,
measured, and the smaller **"often as little as ~400ms"** figure, which is turbopuffer's
own *structured*-path, first-principles budget, not a measured p50 — `docs/benchmarks.md`
states plainly that "the 874ms true-cold figure — not the ~400ms structured-path
figure — is the number our cold-open story actually competes with, and the more
beatable of the two." Comparing STRAND's estimate against 874ms measured: this
estimate is well inside it, which is the honest headline. Comparing it against the
~400ms figure instead would be comparing two unmeasured budgets, and even then not a
clean one: turbopuffer's structured path is described (`docs/benchmarks.md`) as
"metadata, then filter/centroid indexes + WAL tail, then clusters" — three
components — while this RFC's accounting has two manifest GETs and one segment-open
wave, without an enumerated WAL-tail-equivalent step, so the step counts are not
shown to correspond. The ~400ms figure is context, not the comparison this RFC rests
its claim on.

Bytes: the pointer object is a few dozen bytes; snapshot metadata is O(segments) —
unmeasured at this stage, flagged for M0 benchmark data rather than asserted; each
segment's hotcache is small (34 bytes per `blob_entry`, per the field table in §1,
plus 20 bytes of fixed row-ID/count header) and is **not** the same budget as the
100 MB cold-open byte budget (`CLAUDE.md` §7) — that
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
  region, not duplicated here. The container-level `blob_entry.checksum` *value* is
  golden-file-comparable only for `raw-mappable` blobs; for `chunk-compressed` blobs
  it is verified by recomputation, per the design note in §1 above and invariant 11's
  own compressed-bytes exception.
- **Codec-variant provenance:** not applicable to container-level structures; the
  chunk compression codec (zstd, with level) is fully named per blob, satisfying the
  "complete registration" requirement for *this* layer's only codec use.
- **Stochastic-transform provenance:** not applicable — no stochastic transform exists
  at this layer (RaBitQ rotation is M2).
- **Golden files:** this RFC's worked example, once implemented, becomes the first
  `conformance/` golden file: uncompressed hotcache and footer bytes, byte-for-byte —
  with the one exception just noted, a chunk-compressed blob's registry checksum,
  which conformance verifies by recomputation rather than literal byte match.

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

**Row-ID range reservation is a coordination point.** Even with per-attempt nonces
removing the file-creation collision, global uniqueness is still enforced only by the
pointer CAS — a writer's range is only real once its commit wins — so many concurrent
writers targeting the same table still serialize on the pointer for range visibility.
This is a real throughput cost this RFC surfaces but does not solve — consistent with
`CLAUDE.md` §6's "write amplification is the writer's problem," a high-throughput
writer can reserve a larger range per commit to amortize contention, an engineering
choice the format doesn't need to make for it.

**A losing writer must actually recompute, not just retry.** The commit protocol
requires a writer that loses the pointer CAS to re-derive its row-ID range and
version number from the winner's fresh state before retrying, not merely resend its
original proposal under a new nonce. This is correct as specified, but it is also
exactly the kind of step an implementation can silently get wrong under load (retry
the write, forget to refresh the range) and produce duplicate row-IDs that no test
catches until two segments' data collides at query time. The round-trip property
tests and the manifest commit-contention benchmark (`docs/milestones.md`, M0) are
where this must be exercised, deliberately, with concurrent writers whose proposed
ranges are forced to collide.

**The reader-retry bound is unpinned, deliberately, and that's a real gap until it
isn't.** §3 requires readers to bound their 404-refresh retries but does not say by
how much, for the same reason the speculative tail size `N` is a reader parameter and
not a format constant — but an unpinned bound is not itself a benchmark result, and a
badly chosen one (too low, spurious failures under legitimate heavy compaction; too
high, slow error surfacing) is a real deployment problem this RFC defers rather than
solves. The crash tests in `docs/milestones.md`'s M0 list ("reader on expired
snapshot → 404-refresh path") should produce a recommended default, not just prove
the path exists.

**Adopting Iceberg's shape, not copying its bytes.** The commit protocol's shape —
optimistic metadata creation, a CAS-guarded pointer, refresh-and-retry on conflict —
is grounded against Iceberg's own spec text, not memory: "Writers create table
metadata files optimistically... If the snapshot on which an update is based is no
longer current, the writer must retry the update based on the new current version"
(`apache/iceberg`, `format/spec.md`). It is not, however, a byte-for-byte or even
protocol-for-protocol copy — Iceberg's reference deployments commonly commit through
an external catalog (Hive, Glue, a REST catalog) rather than a bare conditional PUT on
a fixed object path, and this RFC's specific mechanics (the `writer_nonce`-suffixed
filename, the `next_row_id` cursor) are STRAND's own, not verified against any
Iceberg source. The nearest grave for a genuinely *bespoke* protocol is Indri/Galago
(`docs/lineage.md`): a well-specified format nobody's engine is pressured to keep
implementing. Adopting the same conceptual shape as a widely-implemented one is the
mitigation; this RFC does not claim more fidelity to Iceberg than that.

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

**Bare `{version:020}.json` snapshot filenames without a per-writer nonce.** Rejected:
two writers reading the same `current` pointer independently compute the same next
version number, so a bare version-number path collides under genuine concurrency —
not the "version-number collision, not a race" an earlier draft of this RFC claimed.
Appending a random `writer_nonce` removes the collision at the cost of a filename
that isn't purely a sort key; `next_row_id` in the snapshot body, not the filename,
is what a reader actually needs to interpret order.

**A suffix range GET (`bytes=-N`) instead of an explicit-end range.** Rejected on
verification, not on principle: a suffix range would save the reader from needing
`byte_length` up front, and RFC 9110 §14.1.2 confirms the HTTP protocol fully defines
this form (`references/rfc9110-range-requests.txt`) — but whether S3 and MinIO's
server-side implementations honor it was not confirmed either way; AWS's `GetObject`
"Range" documentation demonstrates only ordinary explicit-range requests in its
examples. Since the manifest already hands the reader `byte_length` for free before it
opens any segment, there was no reason to depend on the unconfirmed server behavior.

## Open questions / follow-on RFCs

- ~~Hotcache size ceiling and the default speculative tail-read size — needs M0 MinIO
  benchmark data before either is stated as more than provisional.~~ **Resolved
  2026-08-19** — see Discussion below (`bench/src/hotcache_tail_read.rs`).
- ~~The reader 404-refresh retry bound — this RFC requires one to exist but does not
  pin a number; M0's crash tests should produce a recommended default.~~ **Resolved
  2026-08-19** — see Discussion below (`bench/src/reader_refresh_contention.rs`).
- GCS/Azure conditional-write header semantics and the external-catalog fallback
  protocol (R5) — follow-on RFC once a non-S3 target or catalog is in scope.
- ~~Whether S3 (or another target store) actually *implements* the suffix-range form
  RFC 9110 §14.1.2 defines (the protocol question is settled; the server-support
  question is not) is worth confirming empirically — this RFC no longer depends on
  the answer, but a confirmed "yes" would let `strand-tools inspect` open a bare
  segment file in one RTT without a `HEAD` first.~~ **Resolved for MinIO, 2026-08-19**
  — see Discussion below (`crates/strand-core/tests/s3_store.rs`'s
  `suffix_range_get_is_honored_by_minio`); real S3 itself remains unconfirmed (no
  AWS account credentials available to test against), so the MinIO-only finding
  should not be over-read as "S3 too."
- Whether the manifest should eventually carry optional per-segment summary metadata
  for cross-segment pruning is R10 and stays explicitly out of this RFC's scope.

## Discussion — post-approval amendments

Per `CLAUDE.md` §3, design problems revealed by implementation are recorded here,
in the RFC, rather than folded silently into spec text. Three such changes landed
after this RFC was approved. The Design sections above are unmodified; this section
is the record of what changed and why, and `spec/manifest.md`/`spec/container.md`
cite it.

**Definite vs. ambiguous store failures, and the pointer-CAS disambiguation.** The
commit protocol as approved (Design §3, step 3) enumerated two CAS outcomes:
success, and `412 Precondition Failed`. Implementation revealed a third: the store
abstraction conflated "the backend definitely failed" with "the request timed out
and its outcome is unknown." These demand different handling — a pointer CAS in the
second state may already have landed server-side, and a writer that retries blindly
after a landed-but-unacknowledged CAS commits a redundant duplicate version on top
of its own success. The fix added a distinct `Ambiguous` outcome to the store
trait's error type and a disambiguating follow-up `GET` of the pointer in
`commit()`: because the CAS is atomic on the backend, one read fully resolves the
ambiguity — the pointer either names this attempt's own snapshot path (landed;
return success) or it does not (proceed exactly as on `412`). Landed in commit
`1879b52`, test-first, with a mutation test confirming the naive blind-retry
version produces the predicted duplicate commit. `spec/manifest.md` §2 was updated
in the same commit and is the normative statement of the amended protocol.

**Provenance of the `Io`-vs-`PreconditionFailed` retry fix.** An earlier version of
`spec/manifest.md` attributed this fix to this RFC's pre-approval adversarial
review. That was wrong. The review's genuine protocol finding was the step-1
snapshot-filename collision (resolved by the `writer_nonce`, Design §3). The
retry bug — `commit()`'s CAS loop treating a permanent backend failure the same as
a lost race and retrying forever — was found during M0 implementation by writing a
failing test against the unfixed code (the `a0994b7`-era error-propagation work),
not by the review. Both provenances are stated here so the record is honest about
which safeguards caught which defects.

**Raw-mappable blob alignment: a normative obligation the Design section never
stated.** Design §1's `blob_entry` table declares `alignment` ("power-of-two;
raw-mappable blobs only") but never states that a writer must actually place the
blob there — a gap an ACPR convergence pass found (2026-08-18): `spec/container.md`
§5 declared the field with no matching writer obligation, and
`crates/strand-core/src/segment.rs`'s `SegmentBuilder` placed every blob back-to-back
regardless of its declared alignment, silently under-delivering on the field's own
stated purpose. Fixed test-first: `spec/container.md` §5 now states the MUST (padding
to the next multiple of `alignment`, zero-filled — pinned for invariant 11
byte-determinism, since padding content is otherwise unread by any conforming reader
but still affects byte-for-byte golden-file comparison between implementations), and
`SegmentBuilder::build` pads accordingly, verified by two new tests
(`build_pads_a_raw_mappable_blob_up_to_its_declared_alignment`,
`build_does_not_pad_chunk_compressed_blobs`) plus the full workspace suite and
`cargo clippy -- -D warnings`, both clean. The existing `toy-segment.bin` golden file
needed no regeneration: its one blob sits at offset 0, trivially aligned regardless of
padding logic.

**Task X-2: the three remaining Open Questions, closed against real measurement.**
`docs/roadmap.md`'s X-2 named all three of this RFC's own remaining unresolved
items — the speculative tail-read size `N` and hotcache ceiling, the reader
404-refresh retry bound, and the suffix-range server-support question — as owed,
not just flagged. All three are closed here against real data, not guessed, per
`CLAUDE.md` §2's rule that a number without a source is deleted rather than
softened.

*Speculative tail-read size `N` and the hotcache-size ceiling.* Measured with
`bench/src/hotcache_tail_read.rs`: real segments built across a sweep of blob
counts (1, 12, 50, 100, 250, 500, 1000 — 12 is today's actual maximum for one
field spanning every currently-registered family, `spec/container.md` §9; the
rest stand in for the multi-field growth X-1, `docs/roadmap.md`, will eventually
allow, since the container format itself places no cap on blob count), each
committed to real MinIO, then opened via this RFC's own two-phase protocol —
`S3Store::get_tail_range` for the tail read, a real `Footer`/`Hotcache` decode, and a
conditional second range GET — across a sweep of candidate `N` values (512
B–16 KB). The measured transition from one RTT to two tracks the arithmetic
exactly (`hotcache_length + 40 <= N`, RFC 0001 §1's own check): at `N = 512`–
`1024` bytes only `blob_count <= 12` stays at one RTT; by `N = 4096` bytes,
segments up to 100 blob entries (hotcache 3,420 bytes) still open in one RTT;
by `N = 16384` bytes, 250 blob entries (hotcache 8,520 bytes) opens in one RTT,
while 500 and 1,000 blob entries (17,020 and 34,020 bytes) still need the
second GET at every tested `N`. Real per-open latency was also measured at each
point (`bench/results/hotcache-tail-read.json`); the two-RTT cases were
consistently slower, as expected, though the local-MinIO, no-injected-latency
measurement (the same standing caveat every `bench/` benchmark in this
repository carries) understates the real gap a network-latency-bearing
deployment would see.

**`N = 16384` bytes (16 KiB)** is the recommended reader default, carried as
`strand_core`'s recommended-not-mandated tuning parameter (consistent with §1's
"reader-side tuning parameter, not a format constant"). The corresponding
hotcache-size ceiling this implies — `N - 40` bytes of row-ID header plus blob
registry before an open silently degrades from one RTT to two — is **16,344
bytes, roughly 480 blob entries** (`(16344 - 20) / 34`). That is comfortably
above today's real 12-blob-entry maximum (428 bytes) — nearly 40x headroom —
and covers the 250-blob-entry stress point in this sweep, though not the
500- or 1,000-blob-entry synthetic points; those remain honestly two-RTT
until X-1's actual multi-field design lands and its real blob-count
implications can be measured rather than guessed. 16 KiB was chosen over the
smaller candidates precisely because the measured per-open latency showed no
meaningful marginal cost from a larger speculative window at these sizes (the
`blob_count = 1` case, for example, was not measurably slower at `N = 16384`
than at `N = 512`) — so there is no real reason to pick a tighter ceiling and
trade away headroom for it. Full trial data:
`bench/results/hotcache-tail-read.json`.

*Reader 404-refresh retry bound.* Measured with
`bench/src/reader_refresh_contention.rs`: 4 concurrent writers committing
back-to-back against real MinIO with no artificial delay (60 total commits), a
compactor deleting each snapshot the instant a newer one becomes current (the
tightest race window the deletion-safety rule, `CLAUDE.md` §6, allows), and 4
concurrent readers hammering `read_snapshot` throughout — recovering each real
call's internal retry count from `CountingStore`'s GET count (2 GETs per
internal attempt once a table has any commits). Across **691 real reads**
sampled, only **1 read needed a single internal retry**; the other 690 needed
zero, and none exhausted the bound. `manifest::READER_REFRESH_RETRY_LIMIT`
therefore stays at **5** — already roughly 5x the observed worst case, so
measurement confirms this provisional value rather than changing it — and is
now `pub` so this benchmark and any caller reasoning about the bound reference
the real constant rather than a duplicated literal. Full data:
`bench/results/reader-refresh-contention.json`. This benchmark runs against
MinIO on localhost with no injected network round-trip latency, like every
other `bench/` cold-path measurement (`docs/ledger.md` R1) — the same caveat
this RFC's own napkin math (above) already carries, so the observed
race-window width is a lower bound on what a real, latency-bearing production
deployment would see, not an upper one; a real deployment's wider race window
is a reason the 5x headroom matters, not a reason to distrust the
measurement.

*Suffix-range server support.* AWS's `GetObject` API reference
(`references/aws-s3-getobject-range-parameter.md`, fetched 2026-08-19) was
re-checked directly rather than from memory: its only concrete `Range` example
uses the explicit-end form (`bytes=0-9`), and its prose points a reader at RFC
9110 §14.2 "Range" generally, neither confirming nor ruling out the suffix
form — confirming this RFC's original framing exactly, an absent example is not
evidence either way. The server-support question itself was then closed
empirically, for MinIO: `crates/strand-core/tests/s3_store.rs`'s
`suffix_range_get_is_honored_by_minio` issues a raw `Range: bytes=-10` request
(RFC 9110 §14.1.2's suffix-length form) against a real MinIO instance and
confirms MinIO both serves the correct trailing 10 bytes and reports the
resolved absolute range in `Content-Range` (`bytes 16-25/26`), rather than
rejecting the request or silently returning the whole object. **MinIO honors
the suffix-range form.** Real S3 itself remains untested — this session has no
AWS account credentials — so this finding should be read precisely as stated:
MinIO's server-side implementation supports RFC 9110 §14.1.2's suffix form;
whether S3 itself does remains open. This RFC's open protocol (§1) is
unaffected either way, exactly as designed: it was written specifically to not
depend on the answer, using the explicit-end form because the manifest already
hands the reader `byte_length` for free. The practical payoff named in the
original Open Questions entry — letting `strand-tools inspect` open a bare
segment file in one RTT via a suffix range instead of a `HEAD` first — is now
confirmed safe for a MinIO target specifically, still open for a real-S3
target.

Verification: `cargo test --workspace` and `cargo clippy --workspace
--all-targets -- -D warnings`, both clean, alongside the new tests and
benchmarks themselves.

**M3-4: table metadata and retention-eligibility — `committed_at_millis`
added to `SnapshotMetadata`, and the both-fields-set retention reading
resolved (2026-08-19).** `docs/roadmap.md`'s M3-4 implements
`_strand/metadata.json` (Design §3's table-metadata object, until now
specified but not built — `spec/manifest.md` §1 said so explicitly) and
the retention-eligibility function `CLAUDE.md` §6's deletion-safety rule
depends on: "compaction may only physically delete files unreferenced by
every retained snapshot." Two real, if narrow, gaps surfaced during that
work, neither a new commit action on the CAS protocol — the M3-4 task's
own scope boundary, respected here — so `verification/manifest.tla` needs
no change for either.

*Gap 1: nothing in the manifest carried wall-clock time.* A
duration-based retention policy ("keep snapshots committed within the
last N days") needs to compare each snapshot's age against now, but
`SnapshotMetadata` (Design §3) had no timestamp field at all — an
omission the original Design section simply never named, since nothing
before M3 needed one. `SnapshotMetadata` gained `committed_at_millis:
u64`, stamped by the proposing writer (`manifest::now_millis`,
milliseconds since the Unix epoch) immediately before each snapshot
object is written, in both `commit` and `commit_deletion_vector`. This is
additive only: it changes neither CAS action's shape (write a snapshot
object, race the pointer) — `verification/manifest.tla`'s `SnapshotRec`
already abstracts away `path`, `checksum`, and `byte_length` as content
its safety properties don't depend on, and `committed_at_millis` joins
that same category. It is also, deliberately, not subject to invariant
11's byte-determinism pins: like `writer_nonce`, it is real wall-clock
time, not part of the logical input two independent implementations
converging on the same index must agree on, so it is never a golden-file
comparison target — stated explicitly on the field itself
(`crates/strand-core/src/manifest.rs`) so a future session doesn't assume
otherwise.

*Gap 2: the spec names two retention knobs but never says how they
combine.* `spec/manifest.md` §1 states a table declares "minimum snapshot
retention (a count, a duration, or both)" but stops there — silent on
what "both" means when a snapshot satisfies one criterion but not the
other. Per `CLAUDE.md` §3, this is exactly the kind of implementation-
revealed design gap that gets resolved here, adversarially, rather than
picked silently inside `table_metadata.rs`. Two readings are both
internally consistent: the *intersection* (retain only what both
criteria agree to keep) is the more storage-frugal reading; the *union*
(retain what either criterion alone would keep) is the safer one. The
two are not a close call once weighed against what a wrong answer costs:
the deletion-safety rule this whole mechanism exists to serve is
asymmetric — under-retaining risks physically deleting a file a live
snapshot still references, real and unrecoverable for any reader that
has that snapshot open, while over-retaining only costs storage, a cost
`CLAUDE.md` §6 already accepts elsewhere ("write amplification is the
writer's problem"). `table_metadata::retained_snapshots` therefore
implements the union, and additionally treats the current snapshot (the
highest `version` in the list) as always retained regardless of either
policy field — a floor the spec text doesn't state in so many words but
that "retained" cannot coherently mean without: nothing safely lets a
pathological policy (for example, `max_snapshot_age_millis: Some(0)`)
mark the one snapshot every live reader is using as expired. Both
resolutions match Apache Iceberg's own documented behavior for its
equivalent pair of knobs (`history.expire.min-snapshots-to-keep` and
`history.expire.max-snapshot-age-ms`) — the same prior art Design §3
already cites for this protocol's optimistic-concurrency shape — so this
is a return to precedent already in the RFC, not a new invention.
`spec/manifest.md` §1 now states both resolutions normatively.

Verification: `cargo test --workspace` and `cargo clippy --workspace
--all-targets -- -D warnings`, both clean, alongside
`crates/strand-core/src/table_metadata.rs`'s own new tests (write/read
round-trip against a real `ConditionalStore`, the exact RFC 0001 JSON
shape for both `CasHost` variants, and `retained_snapshots` exercised
against a snapshot within the duration window, one outside it, the
inclusive boundary and one millisecond past it, the count-only floor, the
union case, and the always-retain-current floor) and
`crates/strand-core/src/manifest.rs`'s updated `SnapshotMetadata`
round-trip test.
