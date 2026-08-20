# Puffin export sidecar

Normative for STRAND v0.1. Defines the one-way, on-demand export of an
already-built STRAND segment — and, optionally, that segment's current
deletion vector — into a standalone Puffin v1 file
(`references/puffin-spec-and-iceberg-rust-implementation.md`) a Puffin-aware
tool with no STRAND-specific code can open. Approved by RFC 0013
(`rfcs/0013-puffin-export-sidecar.md`); this chapter states the settled
result — see the RFC for the container-profile alternative it rejects, the
napkin math, and the adversarial review.

Reference implementation: `crates/strand-tools/src/puffin_export.rs`, driven
by `strand-tools export-puffin`. Golden file:
`conformance/puffin/toy-deletion-vector.puffin`, matching this chapter's
worked example (§7) exactly, byte for byte.

## 1. Scope

A Puffin sidecar is produced on demand, given one already-built STRAND
segment file and, optionally, the raw bytes of that segment's current
deletion-vector object (`spec/deletion.md` §2). It is written to a path of
the caller's choosing, outside the `_strand/` manifest prefix
(`spec/manifest.md` §1). It is never referenced by any snapshot metadata,
never read by any STRAND reader, and never produced as a side effect of a
commit.

A sidecar carries no pointer back to the STRAND snapshot it was exported
from. Once the source segment is later compacted, superseded by a fresher
deletion vector, or removed by the orphan sweep (`CLAUDE.md` §6), an
already-exported sidecar goes silently stale, with no notification and no
mechanism for a holder to detect the mismatch. Sidecar freshness is the
exporting caller's own problem, on whatever re-export cadence its use case
requires — the same way `CLAUDE.md` §6 states write amplification is the
writer's problem, not the format's.

This chapter defines two translations, and only two: STRAND's
deletion-vector object into Puffin's own registered `deletion-vector-v1`
blob type (§3), and every other STRAND blob, unmodified, into one
catch-all STRAND-namespaced Puffin type, `strand-segment-blob-v1` (§4).
Nothing here changes `spec/container.md`'s or `spec/manifest.md`'s own
formats; a sidecar's byte layout follows Puffin's rules, not STRAND's,
wherever the two disagree (§6).

## 2. File structure

An exported sidecar is one ordinary Puffin v1 file: file magic, one or more
blob regions back to back, then a footer (footer magic, a UTF-8 JSON
payload, `footer_payload_size`, `flags`, a second footer magic), exactly as
`references/puffin-spec-and-iceberg-rust-implementation.md` describes
Puffin's own layout. This chapter adds no framing of its own; §7's worked
example shows every byte of a real, complete file.

A conforming writer MUST place the translated deletion-vector-v1 blob (§3),
if the export includes one, before any `strand-segment-blob-v1` blob (§4),
and MUST order the remaining blobs in the same order the source segment's
own blob registry (`spec/container.md` §5) lists them. This ordering is
deterministic — two conforming exports of the same segment and deletion
vector produce byte-identical blob ordering — stated here because Puffin's
own spec imposes no blob-ordering requirement of its own.

The footer payload is always written uncompressed (`flags` byte 0 bit 0 is
`0`). A conforming writer does not use Puffin's optional LZ4 footer
compression; a conforming reader is not required to reject a compressed
footer produced elsewhere.

## 3. Deletion-vector translation

`spec/deletion.md` §2's deletion-vector object is, byte for byte, a
standard 32-bit Roaring bitmap under `SERIAL_COOKIE_NO_RUNCONTAINER`,
indexed by local ordinal. Puffin's own `deletion-vector-v1` blob type
(`references/puffin-spec-and-iceberg-rust-implementation.md`) is the
identical bitmap format, split by a 32-bit key. `spec/deletion.md` §2
already caps every deletion-vector-declaring segment at
`row_id_count <= 2^32`, so every local ordinal fits in the lower 32 bits
alone: this translation always emits exactly one key, `0`, whose Roaring
bitmap is the segment's existing deletion-vector object bytes,
**unmodified** — no repacking, no reinterpretation, no re-derivation.

A conforming writer MUST assemble the translated blob exactly as follows:

| field | size (bytes) | value | byte order |
| --- | --- | --- | --- |
| combined_length | 4 | length of (magic ‖ bitmap_count ‖ key[0] ‖ bitmap[0]) | big-endian |
| magic | 4 | `D1 D3 39 64` | fixed |
| bitmap_count | 8 | 1 | little-endian |
| key[0] | 4 | 0 | little-endian |
| bitmap[0] | variable | the segment's own deletion-vector object bytes, verbatim | as stored |
| crc32 | 4 | CRC-32 (IEEE/ISO-HDLC) of `magic ‖ bitmap_count ‖ key[0] ‖ bitmap[0]` | big-endian |

This mixed endianness is Puffin's own — big-endian framing fields for Delta
Lake wire compatibility, little-endian inside the Roaring-bitmap-list
wrapper — adopted verbatim rather than STRAND inventing an equivalent
(invariant 8's "don't invent encodings," read together with "registered
codec").

The blob's footer metadata entry carries `type: "deletion-vector-v1"`,
`fields: []` (this chapter's own convention for a non-columnar blob —
Puffin's spec states no rule either way), `snapshot-id: -1` and
`sequence-number: -1` (Puffin v1's own required value when no Iceberg
snapshot exists), and two required properties: `referenced-data-file`, set
to the exporting caller's declared `SegmentRef.path` for the source segment
(a reasonable but imperfect reading of a field Puffin's spec defines
against an Iceberg data file's table-metadata location, not a STRAND
segment — `rfcs/0013-puffin-export-sidecar.md` Design §4 names the
mismatch honestly), and `cardinality`, the decimal-string count of
tombstoned rows the bitmap carries. `compression-codec` is omitted,
matching `deletion-vector-v1`'s own MUST-omit rule.

## 4. Opaque blob passthrough

Every STRAND blob in the source segment's blob registry
(`spec/container.md` §5) that is not a deletion vector is exported,
unmodified, as one Puffin blob of type `strand-segment-blob-v1` — a
STRAND-namespaced type string this chapter mints because Puffin's own spec
defines no third-party registration or namespacing convention for the
`type` field (`rfcs/0013-puffin-export-sidecar.md` Design §1). The blob's
payload is that registry entry's on-disk bytes, byte for byte — for a
`chunk-compressed` blob (invariant 10), still compressed under STRAND's own
chunk framing. This chapter never re-encodes, decompresses, or otherwise
interprets those bytes.

Its footer metadata entry carries `type: "strand-segment-blob-v1"`,
`fields: []`, `snapshot-id: -1`, `sequence-number: -1`, and four
properties, copied straight from the blob's own registry entry, every value
a JSON string (Puffin's `properties` values are strings only):

| property | value |
| --- | --- |
| `strand-family-id` | the registry entry's `family_id`, decimal |
| `strand-blob-type-id` | the registry entry's `blob_type_id`, decimal |
| `strand-field-id` | the registry entry's `field_id`, decimal |
| `strand-checksum` | the registry entry's `checksum` (xxHash3-64, invariant 11's default), lowercase hex |

`compression-codec` is omitted: Puffin registers only whole-blob `lz4`/
`zstd`, and a `chunk-compressed` blob's bytes are already compressed under
a codec Puffin does not recognize, so re-declaring a codec Puffin cannot
decode would be actively misleading. A Puffin-only tool reading this blob
type gets structural visibility — list, size, copy — and nothing more: the
payload is opaque to it, and this chapter does not claim otherwise
(`rfcs/0013-puffin-export-sidecar.md` Design §5).

## 5. Footer payload

The footer's `FileMetadata` JSON object carries `blobs` (the translated and
passthrough blob metadata entries, §3 and §4, in the order §2 states) and
`properties`, a single `created-by` key naming the writer
(`"strand-tools <version> (rfc-0013-puffin-export)"`). Puffin's own spec
pins no key order for either object; this chapter pins one so two
conforming writers given the same segment and deletion vector produce
byte-identical footer JSON — invariant 11's byte-determinism discipline,
extended here to a JSON structure by an explicit STRAND-authored
convention rather than any rule Puffin itself imposes. Top level: `blobs`,
then `properties`. Per blob entry: `type`, `fields`, `snapshot-id`,
`sequence-number`, `offset`, `length`, `properties`. Within a blob's own
`properties` object: the order given in §3's and §4's tables. The JSON is
written compact, with no inserted whitespace.

## 6. What this chapter is not

This chapter defines an export format only. It does not define a
Puffin-to-STRAND import path (RFC 0013 Non-goals), does not change
`spec/manifest.md`'s `SegmentRef` or any snapshot metadata field, and does
not give a Puffin sidecar any role in STRAND's own read or open path —
`spec/container.md` §3's round-trip budget (invariant 3) is unaffected, per
RFC 0013's own napkin math. It does not attempt chunked or per-block export
of a large blob: §4's passthrough always carries a blob's complete on-disk
bytes as one opaque payload, never split by Puffin's compression or chunk
machinery (Puffin has none of its own). Detecting a stale sidecar is left
to a future RFC, if one is ever written (RFC 0013 Non-goals, "Sidecar
staleness and invalidation").

## 7. Worked example

The same deletion vector `rfcs/0012-deletion-vectors.md`'s own worked
example built: local ordinals `{2, 5, 100}` tombstoned, segment
`row_id_base = 1000`, `row_id_count = 200`,
`SegmentRef.path = "segments/0000000000000001.strand"`. STRAND's real,
already-computed 22-byte Roaring bitmap for this set (RFC 0012 worked
example, unmodified here): `3a 30 00 00 01 00 00 00 00 00 02 00 10 00 00 00
02 00 05 00 64 00`.

**deletion-vector-v1 blob payload** (46 bytes):

| field | size | value | bytes (as stored) |
| --- | --- | --- | --- |
| combined_length | 4 | 38 (magic + vector bytes) | `00 00 00 26` (big-endian) |
| magic | 4 | `D1 D3 39 64` | `D1 D3 39 64` |
| bitmap_count | 8 | 1 | `01 00 00 00 00 00 00 00` (little-endian) |
| key[0] | 4 | 0 | `00 00 00 00` (little-endian) |
| bitmap[0] | 22 | STRAND's own deletion-vector bytes, as-is | `3A 30 00 00 01 00 00 00 00 00 02 00 10 00 00 00 02 00 05 00 64 00` |
| crc32 | 4 | `CRC32(magic ‖ bitmap_count ‖ key[0] ‖ bitmap[0])` = `0x85872AAD` | `85 87 2A AD` (big-endian) |

**Footer payload JSON** (this chapter's own canonical field order, §5,
compact form, 279 bytes, UTF-8, uncompressed):

```
{"blobs":[{"type":"deletion-vector-v1","fields":[],"snapshot-id":-1,"sequence-number":-1,"offset":4,"length":46,"properties":{"referenced-data-file":"segments/0000000000000001.strand","cardinality":"3"}}],"properties":{"created-by":"strand-tools 0.1.0 (rfc-0013-puffin-export)"}}
```

**Full Puffin file** (345 bytes total):

| region | offset | size | value |
| --- | --- | --- | --- |
| file magic | 0 | 4 | `50 46 41 31` |
| blob 0 (deletion-vector-v1) | 4 | 46 | the 46-byte table above |
| footer magic | 50 | 4 | `50 46 41 31` |
| footer payload | 54 | 279 | the JSON above, UTF-8 |
| footer_payload_size | 333 | 4 | `17 01 00 00` (279, little-endian signed) |
| flags | 337 | 4 | `00 00 00 00` |
| trailing magic | 341 | 4 | `50 46 41 31` |

Any real Puffin reader — including `apache/iceberg-rust`'s own
`PuffinReader::new(input_file)`, which opens a bare file with no
surrounding Iceberg table
(`references/puffin-spec-and-iceberg-rust-implementation.md`) — given this
exact file, reads back `blob.blob_type() == "deletion-vector-v1"`,
`blob.data()` equal to the 46-byte payload above, and (for a reader that
also implements Puffin's own `deletion-vector-v1` semantics) the
deleted-position set `{2, 5, 100}` against
`referenced-data-file = "segments/0000000000000001.strand"` — the same
three rows `rfcs/0012-deletion-vectors.md`'s own worked example
tombstones, recovered through a reader that has never heard of STRAND.

## 8. Conformance status

Implemented (`crates/strand-tools/src/puffin_export.rs`,
`strand-tools export-puffin`). §7's worked example is pinned as
`conformance/puffin/toy-deletion-vector.puffin` and reproduced byte-for-byte
by `puffin_export`'s own conformance test, built independently (a
standalone Python script, not this crate's own code) as a second
construction path landing on the identical 345 bytes. Cross-checking output
against a real Puffin reader (`apache/iceberg-rust`'s `PuffinReader`)
remains open — RFC 0013's own Open questions.
