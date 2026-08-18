# Filter bitmaps

Normative for STRAND v0.1. Defines the value-dictionary FST and filter-bitmap store
blobs for categorical/metadata fields. Approved by RFC 0006
(`rfcs/0006-filter-bitmaps.md`); this chapter states the settled result — see the
RFC for the worked example, alternatives considered, and the adversarial review.
Registered in `spec/container.md` §9: `family_id = 2` ("filter"), `blob_type_id =
0` (value-dictionary FST), `blob_type_id = 1` (filter-bitmap store).

Reference implementation: not yet implemented.

## 1. Scope: one pair per filterable field, low-to-medium cardinality only

A filterable field carries one value-dictionary FST blob and one filter-bitmap
store blob. This design's cost scales with the field's distinct-value count, not
its row count — it is not intended for high-cardinality fields (RFC 0006
Non-goals).

## 2. Value-dictionary FST

Identical in shape to `spec/term-dictionary.md` §2: keys are the field's distinct
values' bytes in unsigned UTF-8 byte order (invariant 11); values are a dense `u64`
ordinal in sorted order. Bytes are the `fst` crate's compiled `Map` format, version
`0.4.7` (`references/roaring-format-spec-and-rust-crate.md`,
`references/tantivy-fst-termdict-and-fst-crate.md`). `storage-class: raw-mappable`,
`tier: cold-fetchable`. A lookup miss is a normal outcome.

## 3. Filter-bitmap store

A fixed-size directory, 12 bytes per value ordinal, at byte offset `ordinal * 12`
from the blob's start, followed by the bitmap data:

| field           | type | notes                                                         |
| --------------- | ---- | ------------------------------------------------------------- |
| `bitmap_offset` | u64  | absolute byte offset of this value's bitmap, within this blob |
| `bitmap_length` | u32  | byte length of this value's serialized Roaring bitmap         |

Little-endian (invariant 11). `storage-class: raw-mappable`, `tier:
cold-fetchable`.

Bitmap bytes are the **standard 32-bit Roaring format**, exactly as
`RoaringFormatSpec` defines it (`references/roaring-format-spec-and-rust-crate.md`)
— cookie header, descriptive header, offset header, container storage. The 64-bit
Roaring extension MUST NOT be used (RFC 0006 Design §3: it lacks the 32-bit form's
universal interoperability). A value's bitmap indexes **local ordinals** (`0` to
`row_id_count - 1`, `spec/row-ids.md` §1), never the global 64-bit row-ID space
directly; a reader resolves the global row-ID via `row_id_base + local_ordinal`.

## 4. Query resolution

Given a filter predicate and the field's resident FST and filter-bitmap-store
blobs: look the value up in the FST (§2); its ordinal gives the directory record's
offset directly (`ordinal * 12`, §3); the record's `bitmap_offset`/`bitmap_length`
locate the bitmap, already resident — no further round trip. Set operations
(union, intersection) run directly on the Roaring container representation, no
decompression step.

## 5. Placement constraint

Identical in spirit to `spec/scoring-profiles.md` §4, `spec/analyzer-
descriptors.md` §6, and `spec/term-dictionary.md` §5: both blobs are part of the
cold-fetchable wave invariant 3 already budgets for after the segment open.

## 6. Conformance status

Not yet implemented. Golden-file status for both blobs is provisional on the
cross-version/cross-platform determinism questions RFC 0006's "How this could be
wrong" names for the `fst` and `roaring` crates respectively.

## 7. Open dependencies

Deletion vectors (invariant 2, M3 scope) are a separate blob and a separate RFC;
that RFC may cite this chapter's Roaring wire-format registration (§3) without
repeating it, since the format registration is general and only this chapter's
FST-plus-directory layout is filter-bitmap-specific.
