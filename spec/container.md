# Container format

Normative for STRAND v0.1. Defines a segment's on-disk byte layout: the
footer trailer, the hotcache region (row-ID range and blob registry), and
the open protocol a conforming reader MUST use to fetch them within
invariant 3's round-trip budget. Approved by RFC 0001
(`rfcs/0001-container-rowid-manifest.md`); this chapter states the settled
result, not the design discussion — see the RFC for alternatives considered
and the adversarial review.

Reference implementation: `crates/strand-core/src/container.rs`. Golden
file: `conformance/container/toy-segment.bin`, matching the worked example
in this chapter exactly, byte for byte.

## 1. Segment layout

A segment is one object in object storage, laid out data-first,
metadata-last:

```
[ data region: one or more blob regions, back to back ]
[ hotcache region: row-ID range + blob registry ]
[ footer trailer: fixed 40 bytes, always the file's last 40 bytes ]
```

All multi-byte integers in this chapter's structures are little-endian, per
invariant 11.

## 2. Footer trailer

Fixed 40 bytes, always the last 40 bytes of the segment file.

| offset | size | field           | value                                                    |
| ------ | ---- | --------------- | -------------------------------------------------------- |
| 0      | 4    | magic           | ASCII `STRD`                                             |
| 4      | 2    | format_major    | u16                                                      |
| 6      | 2    | format_minor    | u16                                                      |
| 8      | 8    | hotcache_offset | u64, byte offset from file start                         |
| 16     | 8    | hotcache_length | u64                                                      |
| 24     | 1    | checksum_algo   | u8; `1` = xxHash3-64 (the only registered value in v0.1) |
| 25     | 7    | reserved        | MUST be zero                                             |
| 32     | 8    | footer_checksum | u64, checksum_algo over bytes [0, 32)                    |

A reader MUST reject a footer whose `magic` is not `STRD`. A reader MUST
recompute `footer_checksum` over bytes `[0, 32)` using the algorithm named
by `checksum_algo` and reject the footer on mismatch. A writer MUST set
`reserved` to zero; a reader MUST NOT reject a footer for nonzero
`reserved` bytes (reserved for future format versions) but MUST NOT
interpret them.

## 3. Open protocol (invariant 3)

A reader that reached this segment through the manifest (`spec/manifest.md`)
already has the segment's exact `byte_length` from the snapshot metadata
before issuing any request against the segment itself.

1. Issue an ordinary, explicit-end range request,
   `Range: bytes={byte_length-N}-{byte_length-1}`, for a speculative tail
   size `N`. `N` is a reader-side tuning parameter, not a format constant —
   no vendor- or deployment-specific number appears in wire bytes. A reader
   MUST clamp N to byte_length — the request's first byte position is
   max(0, byte_length − N) — since a first position below zero is not a
   valid byte range (RFC 9110 §14.1.2 defines that clamping only for the
   suffix form this protocol does not use). This is
   an ordinary range (not an HTTP suffix range, `bytes=-N` — a form RFC 9110
   §14.1.2 fully defines, `references/rfc9110-range-requests.txt`, but whose
   *server-side* support is confirmed for no target store here): AWS's own
   `GetObject` "Range" documentation demonstrates only the explicit-end form
   in its examples, and this chapter does not depend on suffix-range support
   being confirmed for every target store.
2. Parse the last 40 bytes of the response as the footer trailer (§2).
3. Check `hotcache_length + 40 <= N`. Because the hotcache always ends
   immediately before the footer, at `byte_length - 40`, this single check
   guarantees the *entire* hotcache — not merely its start — landed inside
   the window fetched in step 1. If it holds, the hotcache is already in
   hand: **one round trip**, the common case.
4. If the check in step 3 fails, issue one more range GET for
   `[hotcache_offset, byte_length - 40)`. This is the **second and last**
   round trip invariant 3 allows for the open.

After the open, invariant 3's one-wave rule applies: every byte range a
cold query may need is addressable from the footer, hotcache, or blob
registry already fetched, with no further offset lookup costing a round
trip.

A tool that opens a segment directly, without going through the manifest
(for example, `strand-tools inspect` given a bare file path), does not have
`byte_length` for free and MUST obtain it some other way (a `HEAD`
request, or a suffix range if the target store's support for one has been
separately confirmed). That path is not bound by invariant 3, which
applies to the query-serving path only.

## 4. Hotcache region

The navigation tier fetched wholesale at open: the segment's row-ID range
(`spec/row-ids.md`) and its blob registry.

| field                  | type                                | notes |
| ---------------------- | ----------------------------------- | ----- |
| row_id_base            | u64                                 |       |
| row_id_count           | u64                                 |       |
| blob_count             | u32                                 |       |
| blob_entry[blob_count] | struct, repeated `blob_count` times | §5    |

## 5. Blob registry entry

One `blob_entry`, fixed 34 bytes, per blob in the segment.

| field             | type | notes                                                     |
| ----------------- | ---- | --------------------------------------------------------- |
| family_id         | u16  | registry-assigned blob family (lexical, vector, ...)      |
| blob_type_id      | u16  | registered codec ID within the family                     |
| storage_class     | u8   | `0` = chunk-compressed, `1` = raw-mappable (invariant 10) |
| tier              | u8   | `0` = n/a, `1` = cold-fetchable, `2` = warm (invariant 7) |
| alignment         | u16  | power-of-two byte alignment; raw-mappable blobs only      |
| chunk_codec       | u8   | `0` = none, `1` = zstd (invariant 11 default)             |
| chunk_codec_level | u8   | compressor level; meaningful only when chunk_codec ≠ 0    |
| offset            | u64  | byte offset of the blob's data, within the segment file   |
| length            | u64  | byte length of the blob's on-disk data                    |
| checksum          | u64  | checksum_algo over the blob's on-disk bytes (§6)          |

A `chunk-compressed` blob's internal chunk offset table (chunk lengths,
per-chunk checksums, the mapping from chunk index to byte range) is part of
that blob's own region, not this registry — the registry entry only says
where the blob starts and ends. A specific blob family's chunk index
format is that family's own spec chapter's concern. A `raw-mappable` blob
has no internal chunk table: its bytes are addressed directly at the
declared `alignment`. A writer MUST place a raw-mappable blob at an
`offset` that is a multiple of its declared `alignment`; any padding bytes
this introduces before the blob's data MUST be zero, so that two
conformant writers given the same logical input produce byte-identical
segments (invariant 11) — padding content is otherwise unread by any
conforming reader, which always seeks to `offset` and reads exactly
`length` bytes, but an unpinned padding value would still break
byte-for-byte golden-file comparison between implementations.

## 6. Byte-determinism scope of the registry checksum (invariant 11)

The `checksum` field's *value* is scoped differently by `storage_class`:

- For a `raw-mappable` blob, on-disk bytes are the uncompressed content, so
  `checksum` is fully deterministic across conformant implementations and
  is golden-file-comparable byte-for-byte, like every other hotcache field.
- For a `chunk-compressed` blob, on-disk bytes are compressed, and
  invariant 11 already states that compressed chunk bytes may vary across
  compressor versions and are verified by checksum and round-trip, not
  byte-comparison. The same exception applies here: a conformance check
  MUST recompute `checksum` against the blob's actual stored bytes and
  compare the recomputed value, not assert a fixed byte sequence.

What invariant 11 pins in both cases is that the field is present,
little-endian, and computed with the algorithm named by the footer's
`checksum_algo` — not, for chunk-compressed blobs, a specific value.

## 7. Worked example

A toy segment holding two rows (row-IDs 1000 and 1001) and one
raw-mappable blob storing two little-endian `u32` values, `42` and `43`,
8-byte aligned. Reproduced by `crates/strand-core/src/segment.rs`'s
`SegmentBuilder` and pinned as `conformance/container/toy-segment.bin`.

**Data region** (file offset 0, 8 bytes):

| bytes                     |
| ------------------------- |
| `2A 00 00 00 2B 00 00 00` |

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

**Footer trailer** (offset 62, 40 bytes):

| field           | value                   | bytes                                    |
| --------------- | ----------------------- | ---------------------------------------- |
| magic           | `STRD`                  | `53 54 52 44`                            |
| format_major    | 0                       | `00 00`                                  |
| format_minor    | 1                       | `01 00`                                  |
| hotcache_offset | 8                       | `08 00 00 00 00 00 00 00`                |
| hotcache_length | 54                      | `36 00 00 00 00 00 00 00`                |
| checksum_algo   | 1 (xxHash3-64)          | `01`                                     |
| reserved        | 0 (7 bytes)             | `00 00 00 00 00 00 00`                   |
| footer_checksum | xxHash3-64(bytes[0,32)) | computed by the reference implementation |

Total file size: 102 bytes.

## 8. Open questions

- Hotcache size ceiling and the default speculative tail-read size `N` are
  not pinned by this chapter, pending M0 benchmark data (`bench/results/`).
- GCS/Azure conditional-write and range-request header semantics are R5,
  open (`docs/ledger.md`); this chapter's open protocol is written and
  verified against S3/MinIO only.

## 9. Blob-type registry

`CLAUDE.md` §1 lists "the blob-type registry" among what belongs in the format.
This section is that registry: every `family_id`/`blob_type_id` pair (§5) any
approved RFC has assigned, so future RFCs allocate new IDs without collision.
Populated incrementally, one RFC at a time — this section started empty at RFC
0001's approval and gains its first real entries with RFC 0005.

`family_id = 0` is reserved: RFC 0001's own worked example (§7 above) uses it for
an anonymous placeholder blob with no real family meaning, and no RFC may assign it
a real family.

| `family_id` | family    | `blob_type_id` | blob type          | registered by |
| ------------ | --------- | ---------------- | ------------------- | -------------- |
| 1            | lexical   | 0                | term-dictionary FST | RFC 0005 (`rfcs/0005-term-dictionary.md`) |
| 1            | lexical   | 1                | term-info store     | RFC 0005 (`rfcs/0005-term-dictionary.md`) |
