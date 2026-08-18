# RFC 0006: Filter bitmaps

- **Status:** Approved. Adversarial review found 4 Important findings, all fixed:
  a broken §2→§3 self-citation; a quote misattributed to `CLAUDE.md` M3 that is
  actually from `docs/milestones.md` M3 (now correctly attributed); an
  understated Roaring determinism risk — the `roaring` crate's `insert_range` can
  select a run-container store that serializes under a different cookie than
  array/bitmap containers for the same logical bitmap, a same-version/same-
  platform divergence, not merely cross-platform — closed with a normative MUST
  (`spec/filter-bitmaps.md` §3: always serialize without run containers, under
  `SERIAL_COOKIE_NO_RUNCONTAINER`), independently confirmed against the `roaring`
  0.11.5 source (`Store::Run`/`IntervalStore` in `container.rs`); and an unsourced
  "ordinals fit comfortably" claim, replaced by a normative cap
  (`row_id_count <= 2^32`) with the arithmetic that grounds it. Three Minor
  findings also fixed: an invariant-8 overreach in Alternatives considered
  (invariant 8 governs encodings, not container composition — reworded to cite
  precedent instead), an ambiguous "absolute...within this blob" offset
  description (reworded to match RFC 0005's blob-relative wording, with the
  `blob_entry.offset` reminder), and a systemic observation (no M1 RFC yet tracks
  cumulative cold-fetchable-wave bytes against the 100MB budget — noted, not
  fixed here, out of this RFC's scope). Every byte-level worked-example claim and
  all markdown tables were independently reproduced and confirmed exact by the
  review.
- **Milestone:** M1 — Lexical (`docs/milestones.md`)
- **Spec chapters produced:** `spec/filter-bitmaps.md`; additively extends
  `spec/container.md` §9 (registers `family_id = 2`, "filter")
- **Invariants exercised:** 3, 8, 10, 11 (`CLAUDE.md` §5)

## Summary

Defines STRAND's filter-bitmap blob pair for categorical/metadata fields: a value
dictionary (an FST mapping distinct field values to a dense ordinal, reusing RFC
0005's design directly) paired with a filter-bitmap store — a small fixed-size
directory plus one Roaring bitmap per distinct value, each bitmap marking which
local ordinals in the segment carry that value. Both the FST reuse and the Roaring
wire format itself are invariant 8 in action: nothing here is a new encoding.
Bitmaps use the standard 32-bit Roaring format, indexed by local ordinal, never the
less-interoperable 64-bit extension. The worked example is real: an actual FST and
two actual Roaring bitmaps, compiled by their real crates, byte-decoded field by
field.

## Motivation

`docs/data-structures.md` already settles the wire format: "Roaring bitmap
serialization — the interoperable wire format used by Lucene, ClickHouse, Doris,
and Druid. One of the few places where reusing an exact wire format, not just a
technique, buys real interoperability." `docs/milestones.md` lists "Roaring filter
bitmaps" as a gating M1 deliverable, and `docs/benchmarks.md`'s own hybrid-fusion
benchmark design assumes it exists: "a Roaring metadata filter swept across
selectivities (0.1%, 1%, 10%, 100%)." Neither source designs the actual blob that
makes a filter queryable — a distinct value has to resolve to *its* bitmap somehow,
and that resolution step is this RFC's job, the same gap RFC 0005 closed for terms
resolving to postings.

## Non-goals

**High-cardinality filter fields** (free-text tags, unique identifiers, anything
with a distinct-value count approaching the row count) are out of scope. This
design's cost scales with the number of *distinct values*, not the number of rows —
each distinct value gets its own dictionary entry and its own bitmap — which is
exactly right for low-to-medium-cardinality categorical fields (the
`docs/benchmarks.md` selectivity sweep's own use case) and exactly wrong for
high-cardinality ones. This RFC does not attempt to serve both; a future RFC
targeting high-cardinality filtering is separate work, not a variant of this one.

**Deletion vectors** (invariant 2) also use Roaring, but are M3 scope
(`docs/milestones.md` M3: "Deletion vectors; compaction implementing the per-family
merge semantics of invariant 1") — a different blob, a different lifecycle (updated
by compaction, not written once at segment build time), and a different RFC, not
designed here. This RFC's wire-format registration (§3 below) is written generally
enough that a future deletion-vector RFC can cite it without repeating the spec
citation, but this RFC does not itself define deletion vectors.

**Multi-value fields** (a document carrying more than one value for the same
filterable field, e.g. multiple tags) are not addressed. This RFC's dictionary-and-
bitmap design happens to support them for free — a local ordinal simply appears in
more than one value's bitmap — but this RFC does not state that as a requirement or
test it; a future session should confirm it explicitly before relying on it.

**The 64-bit Roaring extension** is deliberately not used (Design, below); this is
a decision, not an oversight, grounded in the format spec's own statement that the
64-bit form lacks the 32-bit form's universal interoperability
(`references/roaring-format-spec-and-rust-crate.md`).

**Where the two blobs physically live inside a segment** is deferred to the R2 RFC,
identically to RFC 0005's and RFC 0003's own deferred placement — bound by the same
zero-added-round-trip constraint (Napkin math, below).

## Design

### 1. Two blobs, one pair per filterable field

A filterable field carries a **value-dictionary FST** (`family_id = 2`,
`blob_type_id = 0`) and a **filter-bitmap store** (`family_id = 2`, `blob_type_id =
1`), registered in `spec/container.md` §9. A multi-field index carries one such
pair per filterable field, the same per-field scoping RFC 0005 already establishes
for term dictionaries.

Both blobs declare `storage-class: raw-mappable`, `tier: cold-fetchable` — fetched
wholesale, part of the same cold-fetchable wave RFC 0003/0004/0005 already place
their own blobs in, adding no round trip beyond invariant 3's existing budget for
that wave.

### 2. The value-dictionary FST

Identical in shape to RFC 0005 §2: keys are the field's distinct values' bytes, in
unsigned UTF-8 byte order (invariant 11); values are a dense `u64` ordinal in
sorted-insertion order. The blob's bytes are the `fst` crate's own compiled `Map`
format (registered dependency, version `0.4.7`, matching RFC 0005's own
registration — the same dependency, cited once per RFC per this project's existing
convention, not re-litigated here). A lookup miss (the queried value doesn't occur
in this field in this segment) is a normal outcome.

### 3. The filter-bitmap store

A small fixed-size **directory**, one 12-byte record per value ordinal, followed by
the **bitmap data** itself. Ordinal `i`'s directory record sits at byte offset `i *
12` from the blob's start, directly computable — the same direct-indexing pattern
RFC 0005 §3 uses for term-info records, at a smaller record size since a filter
bitmap needs no `doc_freq` or positions field:

| field           | type | notes                                                                      |
| --------------- | ---- | -------------------------------------------------------------------------- |
| `bitmap_offset` | u64  | byte offset **within the filter-bitmap store blob** (not the segment file) |
| `bitmap_length` | u32  | byte length of this value's serialized Roaring bitmap                      |

`12 = 8 (u64) + 4 (u32)`, little-endian (invariant 11). Like RFC 0005's
`postings_offset`/`positions_offset` (`rfcs/0005-term-dictionary.md` §3),
`bitmap_offset` is relative to this blob's own start, not the segment file's start —
a reader adds `spec/container.md` §5's `blob_entry.offset` to get the segment-file
position.

The bitmap bytes themselves are the standard 32-bit Roaring format, exactly as
specified by `RoaringFormatSpec` (`references/roaring-format-spec-and-rust-crate.md`)
— cookie header, descriptive header, offset header, container storage, all as the
spec defines, registered here as an external dependency the same way RFC 0005
registers the `fst` crate's format: a bare "Roaring" is not a sufficiently precise
registration on its own (the format has multiple cookie variants and, separately, an
explicitly non-universal 64-bit extension); this RFC pins the standard 32-bit form,
`SERIAL_COOKIE`/`SERIAL_COOKIE_NO_RUNCONTAINER` as specified, no vendor-specific
deviation.

**Writers MUST NOT emit run containers.** `RoaringFormatSpec` itself documents that
the same logical bitmap can serialize to two different byte sequences depending on
whether run containers are used, and names its own mitigation: always use
`SERIAL_COOKIE_NO_RUNCONTAINER`. This is not a theoretical risk for STRAND's own
default crate: `roaring` (`references/roaring-format-spec-and-rust-crate.md`)
exposes a `Store::Run` container, reachable through range-based insertion APIs
(`insert_range`), that serializes under the run-container cookie with a different
byte layout than the array/bitmap containers the same logical set would otherwise
produce. Two conformant writers of the identical logical bitmap — one inserting
ordinals individually, another using a range API for a contiguous span — can
therefore diverge in wire bytes on the same crate version and the same platform,
not only across versions or platforms. `spec/filter-bitmaps.md` §3 makes the
spec's own recommended mitigation normative: a MUST to serialize every bitmap
without run containers, always under `SERIAL_COOKIE_NO_RUNCONTAINER`.

**Bitmaps index local ordinals, never global row-IDs.** A value's bitmap marks
which of the segment's local ordinals (`0` to `row_id_count - 1`, `spec/row-ids.md`
§1) carry that value — never the 64-bit global row-ID space directly. A reader
recovers the global row-ID via `row_id_base + local_ordinal`, the same arithmetic
every STRAND reader already performs (`spec/row-ids.md` §1). This is a deliberate
choice, not an oversight, and it comes with a normative bound rather than an
assumption: `spec/filter-bitmaps.md` §3 requires `row_id_count <= 2^32`
(4,294,967,296) for any segment declaring this blob family — exactly the range the
standard 32-bit Roaring form can index (`0` to `2^32 - 1`). `spec/container.md` §4
leaves `row_id_count` an unbounded `u64`, so nothing else in the format enforces
this; a segment that violated the cap without this check would silently produce
ordinals the bitmap format cannot represent. Given that bound, this design never
needs the 64-bit extension the format spec itself flags as non-universally
interoperable (`references/roaring-format-spec-and-rust-crate.md`) — sidestepping
that interoperability gap entirely rather than working around it.

### 4. Query resolution

Given a filter predicate (field, value) and the field's already-resident FST and
filter-bitmap-store blobs: look the value up in the FST (§2); if found, its ordinal
gives the directory record's offset directly (`ordinal * 12`, §3); that record's
`bitmap_offset`/`bitmap_length` locate the value's Roaring bitmap, already resident
in the same blob — no further round trip. The bitmap itself directly answers
membership (`contains(local_ordinal)`) or supports set operations (union,
intersection) against other bitmaps entirely within the compressed representation,
with no decompression step
(`references/roaring-bitmaps-container-operations.md`, already vendored during an
earlier grounding pass) — the property that makes Roaring the right choice for this
job in the first place, not merely a convenient one.

## Worked example

A toy 6-document segment (local ordinals `0`–`5`), one filterable field with two
distinct values: `"blue"` (ordinals `0, 3, 4`) and `"red"` (ordinals `1, 2, 5`).
Built with the actual `fst` and `roaring` crates, not hand-derived
(`references/roaring-format-spec-and-rust-crate.md`).

**Value-dictionary FST, 53 bytes:**

```
03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
00 10 82 D3 CF 00 10 92 C2 01 00 01 05 72 62 11
02 02 00 00 00 00 00 00 00 20 00 00 00 00 00 00
00 48 B2 51 42
```

Confirmed against the same real build: `map.get("blue") = Some(0)`,
`map.get("red") = Some(1)` — `"blue"` sorts before `"red"` in UTF-8 byte order, so
it gets the lower ordinal.

**Filter-bitmap store, 68 bytes total.** Directory (24 bytes, 2 records × 12
bytes):

| ordinal | value  | `bitmap_offset` | `bitmap_length` | directory bytes (little-endian)       |
| ------- | ------ | --------------- | --------------- | ------------------------------------- |
| 0       | `blue` | 24              | 22              | `18 00 00 00 00 00 00 00 16 00 00 00` |
| 1       | `red`  | 46              | 22              | `2E 00 00 00 00 00 00 00 16 00 00 00` |

Followed by the two real, compiled Roaring bitmaps back to back (each 22 bytes,
decoded field-by-field in `references/roaring-format-spec-and-rust-crate.md`):

`blue` bitmap (local ordinals `{0, 3, 4}`), at byte offset 24:

```
3A 30 00 00 01 00 00 00 00 00 02 00 10 00 00 00 00 00 03 00 04 00
```

`red` bitmap (local ordinals `{1, 2, 5}`), at byte offset 46:

```
3A 30 00 00 01 00 00 00 00 00 02 00 10 00 00 00 01 00 02 00 05 00
```

`24 (directory) + 22 (blue) + 22 (red) = 68` bytes total, matching the blob's actual
compiled length exactly. Resolving the filter predicate `field = "red"`: FST lookup
returns ordinal `1`; the directory record at byte offset `1 * 12 = 12` gives
`bitmap_offset = 46, bitmap_length = 22`; the bytes at `[46, 68)` are the `red`
bitmap, immediately usable for membership or set operations with no further fetch
or decompression.

## Napkin math (`CLAUDE.md` §7)

Same conclusion as RFC 0003/0004/0005's own napkin math, for the same reason:
these two blobs are cold-fetchable-wave metadata, not per-query cold-path
structures requiring their own round trip. The binding constraint on whichever RFC
places their bytes is the same one already stated three times: zero added round
trips beyond invariant 3's existing budget for that wave. Total size for realistic
low-to-medium-cardinality fields is small by construction (this design's whole
premise, Non-goals above) — a field with, say, a few hundred distinct values costs
a few hundred small bitmaps plus a proportionally tiny FST, nowhere near the scale
where RFC 0005's own FST-size-at-scale open question would apply, since that
question is about vocabulary size (potentially millions of terms), not filter-value
cardinality (bounded by this RFC's own stated scope to stay small).

## Invariant-11 checklist

- **Endianness:** little-endian, pinned for the directory's fixed-size fields; the
  FST blob and the Roaring bitmap bytes are each externally defined formats
  (`fst` crate, `RoaringFormatSpec`) — both already little-endian by their own
  specifications, confirmed rather than assumed
  (`references/roaring-format-spec-and-rust-crate.md`).
- **Term sort order:** unsigned UTF-8 byte order for dictionary values, identical
  reasoning to RFC 0005 §2.
- **Chunk codec:** not applicable — both blobs raw-mappable, no chunking.
- **Checksums:** covered by each blob's own registry entry (`spec/container.md`
  §5, §6); no new checksum scope.
- **Codec-variant provenance:** two registrations this RFC makes precisely — the
  `fst` crate, version `0.4.7` (matching RFC 0005's own registration exactly), and
  the Roaring format, standard 32-bit form specifically, **run containers
  excluded by a normative MUST** (`spec/filter-bitmaps.md` §3): every bitmap is
  serialized under `SERIAL_COOKIE_NO_RUNCONTAINER` as `RoaringFormatSpec` defines
  it, never `SERIAL_COOKIE`, and never the 64-bit extension.
- **Stochastic-transform provenance:** not applicable.
- **Golden files:** the worked example's real 53-byte FST and real 68-byte
  filter-bitmap store become the first `conformance/` golden files for this blob
  family once implemented. The `fst` crate half carries the same
  cross-version/cross-platform determinism caveat RFC 0005 already names. The
  `roaring` crate half's more serious risk — same-version, same-platform
  divergence from run-container promotion (Design §3 above) — is closed
  normatively by the no-run-containers MUST, not merely narrowed; what remains
  open for the `roaring` half is the same, narrower cross-version/cross-platform
  question already named for `fst` (same-process determinism confirmed here;
  cross-platform/cross-version not independently tested in this pass,
  `references/roaring-format-spec-and-rust-crate.md`).

## How this could be wrong

**Nearest grave (`docs/lineage.md`): Pilosa** — "a good structure with a spec is
not a distribution strategy." This is not a generic citation: Pilosa *is*, roughly,
a standalone product built around exactly this RFC's central idea — categorical
fields indexed as one bitmap per distinct value. Pilosa's technology worked; it
died from lack of an adoption path, not a design flaw. The direct, structural risk
for this RFC is the same one, not a distant cousin of it: a filter-bitmap blob
design nobody's real engine reads is precisely a Pilosa outcome in miniature. This
RFC's actual mitigation is not technical superiority over Pilosa's design — it's
positional: STRAND is not trying to be a standalone bitmap-index product the way
Pilosa was (`CLAUDE.md` §1 already states this project is a format, not an engine,
and explicitly warns against drifting toward "building engine features"); this
design's payoff is that any engine already reading STRAND segments gets filter
bitmaps as one more registered blob type, at the marginal cost of implementing one
more well-specified, externally-defined wire format it likely already has a reader
for (Lucene, Druid, ClickHouse, Doris all already read standard Roaring,
`docs/data-structures.md`) — adoption is riding on the format's adoption, not
requiring its own.

**High-cardinality misuse is a real, not hypothetical, way to reintroduce the
Pilosa risk from a different angle.** If a future implementation reaches for this
blob family for a high-cardinality field despite Non-goals explicitly excluding
that case, the resulting design (millions of tiny bitmaps, a huge FST) would
perform badly enough that "STRAND's filter bitmaps don't work" becomes the
takeaway, when the actual lesson is "this RFC was never designed for that case."
Stating the scope boundary precisely (Non-goals, above) is this RFC's defense
against that misreading, not a claim the boundary enforces itself.

**Run-container promotion is a same-version, same-platform byte-determinism risk,
not merely a cross-platform one.** `RoaringFormatSpec` documents that a single
logical bitmap has two possible serializations depending on run-container use, and
the `roaring` crate's own range-insertion API can select the run-container store for
an ordinary, non-adversarial write pattern (a contiguous span of local ordinals) —
meaning the risk this RFC's determinism story has to close is not "two different
crate versions might diverge," it is "two calls into the identical crate build,
differing only in which insertion API the writer happened to use, might diverge."
Design §3 and `spec/filter-bitmaps.md` §3 close this with a normative MUST
(`SERIAL_COOKIE_NO_RUNCONTAINER`, always), following the format spec's own stated
mitigation rather than inventing one. Naming this precisely matters because
invariant 11's byte-determinism promise is not satisfied by "usually produces the
same bytes" — a golden-file comparison that passed only because a reference
implementation happened to always use one insertion API would be silently fragile.

**The 64-bit-extension-avoidance choice trades one interoperability gap for a
narrower one.** Avoiding the 64-bit Roaring extension sidesteps a real, spec-
documented interoperability problem (`references/roaring-format-spec-and-rust-
crate.md`), but it does so by committing every filter bitmap to local-ordinal
addressing specifically — meaning any future consumer of this blob outside the
per-segment reading model this format assumes (e.g., a tool that wants a filter
bitmap over *global* row-IDs directly, without resolving through `row_id_base`
first) cannot use these bytes without that resolution step. This is judged an
acceptable, minor cost against the 64-bit extension's real non-interoperability,
not a cost-free choice.

## Alternatives considered

**The 64-bit Roaring extension**, indexed directly by global row-ID, avoiding the
`row_id_base` resolution step entirely. Rejected: the format spec's own text states
this extension is not universally interoperable across even the reference Java
implementations (`references/roaring-format-spec-and-rust-crate.md`) — adopting it
would mean STRAND's filter bitmaps are less portable than they need to be, for a
convenience (skip one addition) this format's own per-segment, local-ordinal-native
design (invariant 1, `spec/row-ids.md`) doesn't actually need.

**A single combined FST-plus-bitmaps blob** instead of two separate blob
registrations. Rejected for consistency with RFC 0005's own two-blob precedent
(dictionary, then value-indexed data): invariant 8 governs *encodings* (postings
compression, adjacency layouts, quantization) and does not itself forbid a combined
blob at the container level, so the real reason is precedent and registry
simplicity, not an invariant-8 violation — reusing RFC 0005's already-working
two-blob pattern rather than inventing a second container-composition convention
this project does not yet need.

**Storing bitmaps `chunk-compressed`** instead of `raw-mappable`. Rejected:
Roaring's own container format is already compact and directly operable without a
decompression step (`references/roaring-bitmaps-container-operations.md`) — an
additional compression layer on top would force decompression before any
operation could run, destroying exactly the property that makes Roaring worth
using here, the identical reasoning RFC 0005 §Alternatives already applies to the
term-info store.

## Open questions / follow-on RFCs

- Multi-value-field support is not tested or required by this RFC (Non-goals,
  above); a future session should confirm it explicitly.
- The two blobs' exact placement inside a segment is deferred to the R2 RFC,
  identically to RFC 0003/0004/0005's own deferred placement questions.
- Cross-platform/cross-version determinism for both the `fst` crate half (already
  named open by RFC 0005) and the `roaring` crate half needs an actual test before
  either half's golden-file status is fully, not provisionally, satisfied. The
  more serious same-version/same-platform risk on the `roaring` half —
  run-container promotion diverging bytes within one build — is already closed
  normatively by Design §3's no-run-containers MUST; what remains open here is the
  narrower cross-version/cross-platform question, symmetric with the `fst` half.
- Deletion vectors (invariant 2, M3 scope) will need their own RFC; that RFC can
  cite this one's Roaring wire-format registration (§3 above) rather than
  re-deriving it, since the format choice itself is general, only this RFC's
  specific blob layout (the FST-plus-directory pattern) is filter-bitmap-specific.
- High-cardinality filter fields (Non-goals, above) are explicitly out of scope
  and unaddressed; a future RFC would need a structurally different design, not an
  extension of this one.
