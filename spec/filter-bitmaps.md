# Filter bitmaps

Normative for STRAND v0.1. Defines the value-dictionary FST and filter-bitmap store
blobs for categorical/metadata fields. Approved by RFC 0006
(`rfcs/0006-filter-bitmaps.md`); this chapter states the settled result — see the
RFC for the worked example, alternatives considered, and the adversarial review.
Registered in `spec/container.md` §9: `family_id = 2` ("filter"), `blob_type_id =
0` (value-dictionary FST), `blob_type_id = 1` (filter-bitmap store).

Reference implementation: `crates/strand-lexical/src/filter_bitmaps.rs`. Golden
files: `conformance/filter-bitmaps/toy-values.fst` and
`conformance/filter-bitmaps/toy-bitmap-store.bin`, matching RFC 0006's worked
example exactly, byte for byte.

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

| field           | type | notes                                                                          |
| --------------- | ---- | ------------------------------------------------------------------------------ |
| `bitmap_offset` | u64  | byte offset **within this blob** (not the segment file) of this value's bitmap |
| `bitmap_length` | u32  | byte length of this value's serialized Roaring bitmap                          |

Little-endian (invariant 11). `storage-class: raw-mappable`, `tier:
cold-fetchable`. A reader adds `spec/container.md` §5's `blob_entry.offset` to
`bitmap_offset` to get the segment-file byte position.

Bitmap bytes are the **standard 32-bit Roaring format**, exactly as
`RoaringFormatSpec` defines it (`references/roaring-format-spec-and-rust-crate.md`)
— cookie header, descriptive header, offset header, container storage. The 64-bit
Roaring extension MUST NOT be used (RFC 0006 Design §3: it lacks the 32-bit form's
universal interoperability). A value's bitmap indexes **local ordinals** (`0` to
`row_id_count - 1`, `spec/row-ids.md` §1), never the global 64-bit row-ID space
directly; a reader resolves the global row-ID via `row_id_base + local_ordinal`. A
segment declaring this blob family MUST satisfy `row_id_count <= 2^32`
(4,294,967,296) — the exact cardinality the standard 32-bit Roaring form can index
(values `0` to `2^32 - 1`). This is a normative cap, not an assumption:
`spec/container.md` §4 leaves `row_id_count` an unbounded `u64`, so a writer of an
oversized segment that skipped this check would silently produce a filter-bitmap
blob whose ordinals cannot address every row.

A writer MUST NOT emit run containers: every bitmap MUST be serialized with the
`SERIAL_COOKIE_NO_RUNCONTAINER` cookie. This is not a stylistic default — the
`roaring` crate itself provides a `Store::Run` container variant, selectable by
range-based insertion APIs (`insert_range`), that serializes under the *other*
cookie, `SERIAL_COOKIE`, with a structurally different byte layout for the same
logical set. Two conformant writers building the identical logical bitmap — one
inserting ordinals one at a time, another using a range-insertion API for a
contiguous span — can therefore produce different bytes on the same crate version
and the same platform, not merely across platforms or versions. This is exactly the
non-isomorphism `RoaringFormatSpec` itself warns about
(`references/roaring-format-spec-and-rust-crate.md`), and its own recommended
mitigation is this chapter's MUST rule: always serialize without run containers,
under `SERIAL_COOKIE_NO_RUNCONTAINER`, regardless of which container type an
implementation's in-memory representation happens to have chosen. A reader MUST
still accept `SERIAL_COOKIE` (run-container) input on read, for interop with
bitmaps produced outside a conforming STRAND writer, since decoding is unambiguous
either way — only the writer side is constrained.

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

Implemented (`crates/strand-lexical`), with both blobs' worked-example bytes pinned
as `conformance/` golden files and confirmed byte-exact against
`crates/strand-lexical/tests/filter_bitmaps_worked_example.rs`. The no-run-
containers MUST (§3 above) is mechanically checked, not merely asserted: `crates/
strand-lexical/tests/filter_bitmaps_round_trip.rs` builds the identical logical
bitmap through two different `roaring` insertion APIs (one that stays array/
bitmap, one that can select a run container) and asserts the serialized bytes are
identical. Golden-file status for the value-dictionary FST is provisional on the
cross-version/cross-platform `fst`-crate determinism question RFC 0005 and RFC
0006's "How this could be wrong" both name. For the filter-bitmap store, the
same-version/same-platform risk (run-container promotion) is closed normatively by
this chapter's MUST rule and confirmed by the test above; what remains open is the
narrower cross-version/cross-platform question already named for the `fst` half.

## 7. Open dependencies

Deletion vectors (invariant 2, M3 scope) are a separate blob and a separate RFC;
that RFC may cite this chapter's Roaring wire-format registration (§3) without
repeating it, since the format registration is general and only this chapter's
FST-plus-directory layout is filter-bitmap-specific.
