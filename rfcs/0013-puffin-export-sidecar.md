# RFC 0013: Puffin export sidecar

- **Status:** Draft. This RFC contains a real, substantive "How this could be
  wrong" section (below) — the same adversarial discipline every other RFC in
  this repository applies — written by the same session that drafted the
  design, not by an independent second pass. Per this task's own instruction
  and `CLAUDE.md` §3's own "not in the same breath" principle, that makes it a
  self-review, not the independent adversarial review Approval requires here:
  the design commits STRAND to a second wire format and a second checksum
  algorithm (Invariant-11 checklist, below) purely for interchange, which is
  exactly the class of decision that benefits from a reader who did not write
  it. This RFC's own worked example is independently checkable by anyone
  (Design §4) precisely so that review does not have to start from scratch.
- **Milestone:** M4 — Interchange + independence (`docs/milestones.md`),
  the "Puffin blob-type packaging RFC" deliverable named there; tracked as
  M4-5 (`docs/roadmap.md`).
- **Spec chapters produced:** none yet. This RFC proposes a new chapter,
  `spec/puffin-export.md`, written only at Approval — every existing spec
  chapter in this repository states an already-settled result
  (`spec/manifest.md`'s own opening line: "this chapter states the settled
  result"), and this RFC is not settled.
- **Invariants exercised:** 2 (`CLAUDE.md` §5 — the deletion-vector blob is
  this RFC's one real translation target), 8 (don't invent encodings — this
  RFC reuses Puffin's own already-registered `deletion-vector-v1` layout
  verbatim, never inventing a STRAND-specific equivalent), 11 (byte
  determinism — see the checklist; this RFC is the first to introduce a
  second checksum algorithm into the repository, scoped only to its own
  output). Invariant 3 (the cold-open round-trip budget) is argued *not* to
  apply (Napkin math, below) — the central scoping question this RFC answers
  is which invariants a Puffin sidecar can satisfy at all, and the answer is
  "a proper subset," stated precisely rather than glossed.

## Summary

`docs/milestones.md`'s M4 entry names one line of scope this RFC exists to
resolve: "Puffin blob-type packaging RFC." Nothing else in that entry, or in
`docs/roadmap.md`'s M4-5 line, says what problem the packaging is meant to
solve — `spec/manifest.md` §1 already borrowed Puffin's *shape* (opaque typed
blobs behind a JSON footer) for STRAND's own manifest, so "packaging" cannot
mean re-adopting a pattern STRAND already has. The real gap is the other
direction: STRAND already has its own numeric blob-type registry
(`spec/container.md` §9, `family_id`/`blob_type_id`), readable only by a
STRAND-aware reader. Puffin is the format Apache Iceberg's own ecosystem
already reads. This RFC asks, and answers, a narrower and more honest
question than "make STRAND Puffin-compatible": **can a STRAND segment export
a sidecar file that a real, existing, Puffin-aware tool — one that has never
heard of STRAND — can open and get something correct out of, without a
STRAND-specific reader?**

The answer this RFC lands on: yes, for exactly one blob family, because
Puffin already has a standard blob type that means almost exactly what
STRAND's deletion vector means, and — a real, checked fact, not an
assumption — STRAND's own on-disk Roaring bitmap bytes for that blob are
reusable **unmodified** inside it (Design §4). For every other blob family,
a Puffin-aware tool gets structural visibility only (it can list, size, and
copy STRAND's blobs) via one STRAND-namespaced opaque blob type, and this RFC
says so plainly rather than oversell it. This is a one-way, on-demand
**export** sidecar — never referenced by `spec/manifest.md`'s own snapshot
metadata, never part of STRAND's own read path — sitting next to CIFF import
as M4's other half of "Interchange": CIFF brings a foreign index in; this RFC
sends one narrow, real piece of a STRAND index out.

## Motivation

`spec/manifest.md` §1 already states the precedent this RFC formalizes: "This
mirrors Puffin's 'opaque typed blobs with a JSON footer' pattern... binary
where bytes are read by a codec on the hot path, JSON where humans and
cross-engine tooling read it." That sentence describes an architectural
resemblance, not an interop claim — nothing in the repository lets an actual
Puffin-aware tool read actual STRAND bytes. M4 is named "Interchange +
independence" and already has one half of that story (the CIFF importer,
ingesting a foreign lexical index); this RFC's job is the missing outbound
half, for the one direction Iceberg's own ecosystem can already receive:
Puffin sidecars.

`docs/lineage.md` credits Puffin honestly, not uncritically: "the container
pattern of opaque typed blobs with a JSON footer — adopted with eyes open:
Puffin's registry has spawned essentially no third-party blob ecosystem
despite Iceberg's gravity. The pattern is a good container, not a
distribution strategy." This RFC takes that sentence as a constraint on its
own ambition, not a reason to skip the milestone item: the motivation here is
deliberately narrow — real interop where Puffin already has a matching
standard type, honest non-interop everywhere else — rather than a claim that
shipping a Puffin sidecar makes STRAND segments broadly portable. Whether
that narrow motivation is still worth the design and maintenance cost is
argued out directly in How this could be wrong, below, not assumed here.

## Non-goals

**Redefining STRAND's own segment container around Puffin.** Design §1 below
walks Puffin's real `BlobMetadata` schema field by field against STRAND's own
invariants and finds it cannot host `spec/container.md`'s registry contract
without silently dropping guarantees invariants 7, 10, and 11 make normative,
plus the field-disambiguation mechanism `spec/container.md` §5a adds on top
of them. A "STRAND segment *is* a Puffin file" container profile is
considered and rejected, not merely undiscussed.

**A Puffin → STRAND importer.** This RFC is one-way, STRAND → Puffin.
Ingesting a foreign index is CIFF's job (M4's other named deliverable); nothing
about reading an arbitrary Puffin file back into a STRAND segment is
addressed here.

**Registering new blob types with the Iceberg project itself.** The two
types Puffin's own spec registers (`apache-datasketches-theta-v1`,
`deletion-vector-v1`) are Iceberg's to extend, through Iceberg's own process
— this RFC does not propose STRAND lobby for new entries there. Where this
RFC needs a type string Puffin has no equivalent for, it mints one
STRAND-namespaced string (Design §5) rather than asking Iceberg to adopt it.

**Changing `spec/manifest.md`'s snapshot metadata format.** A Puffin sidecar
this RFC produces is never referenced by a `SegmentRef` or any other manifest
field. It is a standalone object a caller asks for on demand; the manifest
does not know it exists.

**Chunked or per-block export of large blobs.** Puffin supports exactly two
whole-blob compression codecs (`lz4`, `zstd`) and no internal chunk index —
`spec/container.md`'s chunk-compressed storage class has no Puffin
equivalent. STRAND's large blobs (postings, positions, vector navigation
tiers and posting lists) are addressed only by Design §5's opaque
passthrough, carrying their already-chunk-compressed bytes as an opaque
payload Puffin's own compression machinery never touches — not re-encoded,
not decompressed, not made independently readable by a Puffin-only tool.

**Deletion-vector correctness across compaction or merge.** This RFC exports
one segment's deletion vector as it stands at export time. What a merged
segment's deletion vector should look like is M3 compaction's own settled
question (`rfcs/0012-deletion-vectors.md` Non-goals); this RFC exports
whatever `spec/deletion.md` already produces, unchanged.

## Design

### 1. Why a sidecar, and not a container profile — argued, not assumed

`spec/container.md` §5's blob registry entry carries eleven fields per blob:
`family_id`, `blob_type_id`, `field_id`, `storage_class`, `tier`, `alignment`,
`chunk_codec`, `chunk_codec_level`, `offset`, `length`, `checksum`. Puffin's
`BlobMetadata` (`references/puffin-spec-and-iceberg-rust-implementation.md`)
carries eight: `type`, `fields`, `snapshot-id`, `sequence-number`, `offset`,
`length`, `compression-codec`, `properties`. Lining them up field by field is
the whole argument for why STRAND's segment cannot simply *be* a Puffin file:

`type` is a free-form JSON string with **no collision-avoidance rule of any
kind** — the spec's own "Blob types" section, read in full, says nothing
about how a third party names, namespaces, or registers one. STRAND's
`family_id`/`blob_type_id` pair is a numeric registry, incrementally
allocated one RFC at a time (`spec/container.md` §9), with a reserved
sentinel (`family_id = 0`) and a stated collision consequence for the
*separate* `field_id` hash (§5a) — the opposite discipline invariant 8
("registered codec," "don't invent encodings" read together with a stated
allocation authority) already commits STRAND to. Puffin's `fields` field
means something different again: Iceberg schema column IDs, used "to
compute sketches," not a disambiguator between two blobs of the same type —
STRAND's `field_id` (a hashed field name, resolvable with no catalog lookup,
`spec/container.md` §5a) has no Puffin equivalent at all.

`BlobMetadata` has **no checksum field**, of any kind, on any blob — confirmed
by reading the schema table directly, not inferred from an omission
elsewhere. Invariant 11 requires "every chunk carries a declared checksum,"
and `spec/container.md` §5's registry entry already gives every STRAND blob
one. A Puffin blob's integrity rests entirely on `deletion-vector-v1`'s own
per-blob CRC-32 where that specific type happens to define one (Design §4)
and on nothing at all for every other type, `apache-datasketches-theta-v1`
included.

There is no `storage_class`, `alignment`, or `tier` field. Invariant 7's
`cold-fetchable`/`warm` tier declaration and invariant 10's `chunk-
compressed`/`raw-mappable` storage class are both STRAND-specific navigation
metadata a cold reader depends on to decide *how* to fetch a blob before
fetching it; Puffin's `compression-codec` answers a narrower question
(whole-blob lz4, zstd, or nothing) and has nothing to say about alignment or
raw-mappability at all.

There is no row-ID space. `spec/container.md` §4's hotcache carries
`row_id_base`/`row_id_count` alongside the blob registry; Puffin's
`FileMetadata` has no equivalent concept, because Puffin blobs describe
statistics *about* an Iceberg data file, not an alternative encoding of one.

None of this is a defect in Puffin — it is a narrower format, built for a
narrower job (statistics and small indexes riding alongside a columnar data
file), and it is honest about that scope. It does mean a "STRAND segment is
a Puffin file" container profile would have to either invent extension
fields Puffin's spec does not define (repeating the mistake invariant 8
exists to prevent — inventing an encoding instead of adopting one) or
silently drop invariants 7, 10, and 11's per-blob guarantees, plus §5a's
field-disambiguation mechanism, the moment a segment's real registry entries
got flattened into Puffin's eight fields. Both outcomes are worse than not
doing this. The sidecar option carries none of this risk, because it never
claims to *replace* anything: it is an additional, optional, narrower object
that STRAND's own reader never opens.

### 2. What the sidecar is, structurally

A Puffin export sidecar is one ordinary Puffin v1 file (magic `50 46 41 31`,
one or more blobs, one JSON-footer trailer, exactly as
`references/puffin-spec-and-iceberg-rust-implementation.md` describes),
written by `strand-tools export-puffin` (a new CLI verb; **its
implementation is out of scope for this RFC**, per `CLAUDE.md` §3 — the
deliverable here is the design and the byte-exact worked example a future
implementation session builds against) given one already-committed STRAND
segment and, optionally, its current `DeletionVectorRef`
(`spec/deletion.md` §3). The sidecar is written to a path of the caller's
choosing, outside the `_strand/` manifest prefix `spec/manifest.md` §1
reserves — it is not part of the table at all, in the same sense a
`strand-tools inspect` text report is not part of the table.

### 3. Registration surface this RFC actually touches

This RFC adds nothing to `spec/container.md` §9 (STRAND's own numeric
registry) — a Puffin sidecar is not a STRAND blob and carries no
`family_id`/`blob_type_id` pair of its own. What it registers instead, at
Approval, in the new `spec/puffin-export.md` chapter: which STRAND blob
families map to which Puffin `type` strings (§4 and §5, below), and the
translation rule for each. This is a translation table, not an extension of
invariant 8's codec registry — STRAND's own on-disk bytes for a translated
blob are unchanged by this RFC; only a second, Puffin-shaped wrapper around a
copy of them is new.

### 4. The one real translation: deletion vectors

`spec/deletion.md` §1 already commits to "the standard 32-bit Roaring format,
exactly as `RoaringFormatSpec` defines it... under
`SERIAL_COOKIE_NO_RUNCONTAINER`" for STRAND's own deletion-vector object —
indexed by local ordinal, `0` to `row_id_count - 1`. Puffin's
`deletion-vector-v1` blob type (quoted in full in
`references/puffin-spec-and-iceberg-rust-implementation.md`) is, at its
core, *also* a 32-bit Roaring bitmap in the same portable format, indexed by
"position" — split into a 32-bit key (the position's upper 32 bits) and a
32-bit sub-position (the lower 32 bits), one Roaring bitmap per distinct key.

`spec/deletion.md` §1 already normatively caps every STRAND segment
declaring a deletion vector at `row_id_count <= 2^32`. Every local ordinal
therefore fits in the lower 32 bits alone: the translation needs exactly one
key, `0`, and that key's Roaring bitmap is **STRAND's existing deletion-
vector object bytes, verbatim, with no repacking, no reinterpretation, and no
re-derivation of the bitmap's contents** — the single concrete compatibility
fact this whole RFC turns on, checked in the worked example below by
constructing real bytes, not asserted from the two formats merely sounding
similar. What differs is only the outer framing Puffin's spec adds around
that bitmap: a length-prefixed key/bitmap list, then a big-endian
length+magic+CRC-32 wrapper the spec states in full (quoted above).

The one place the two formats' semantics genuinely diverge, stated rather
than glossed: Puffin's positions are "positions of rows in a file," meaning
an Iceberg *data file* the deletion vector accompanies; STRAND's local
ordinals are positions within a STRAND *segment*. This RFC's translation
maps STRAND's `SegmentRef.path` onto Puffin's required
`referenced-data-file` property — a reasonable reading (a STRAND segment,
like an Iceberg data file, is a single self-contained object with a dense,
zero-based internal row ordering) but one a real Puffin-only consumer might
not expect, since it will not find that path in any Iceberg table metadata
it knows about. This is named again, as a real risk rather than resolved
away, in How this could be wrong.

### 5. Everything else: opaque passthrough, honestly labeled

For every STRAND blob that is not a deletion vector, this RFC registers one
catch-all Puffin blob type, `strand-segment-blob-v1` (a STRAND-namespaced
string, chosen to avoid colliding with any future Iceberg-registered type,
per the complete absence of a namespacing convention noted in Design §1).
Its Puffin `properties` carry three STRAND-specific keys —
`strand-family-id`, `strand-blob-type-id`, `strand-field-id` — each a decimal
string (Puffin's `properties` values are JSON strings only), copied straight
from that blob's `spec/container.md` §5 registry entry. Its blob payload is
that blob's on-disk bytes, unmodified — for a `chunk-compressed` blob, that
means the bytes stay exactly as compressed under STRAND's own chunk framing;
Puffin's `compression-codec` property is omitted (not one of the two codecs
Puffin itself defines), since re-decompressing and re-compressing a large
postings or vector blob under Puffin's whole-blob-only lz4/zstd model would
be real, pointless work this RFC has no reason to require.

A tool that understands only Puffin, not STRAND, gets real, if modest, value
from this: it can list every blob a segment carries, see its declared byte
size, and copy it — all from the footer alone, no STRAND-specific parsing
needed for that much. It gets nothing more: the bytes inside a
`strand-segment-blob-v1` blob are opaque to it, exactly as `docs/lineage.md`'s
own assessment of Puffin predicts for a niche type nobody else has adopted.
This RFC states that limitation here, in the design itself, rather than
letting a reader discover it only in How this could be wrong.

## Worked example

The same deletion vector `rfcs/0012-deletion-vectors.md`'s own worked example
built: local ordinals `{2, 5, 100}` tombstoned, segment `row_id_base = 1000`,
`row_id_count = 200`, `SegmentRef.path = "segments/0000000000000001.strand"`.
STRAND's real, already-computed 22-byte Roaring bitmap for this set (RFC
0012 Worked example, unmodified here): `3a 30 00 00 01 00 00 00 00 00 02 00
10 00 00 00 02 00 05 00 64 00`.

Every byte below is computed, not hand-derived — a small Python script (CRC-32
via `zlib.crc32`, matching the widely-used IEEE/ISO-HDLC CRC-32 variant this
spec's own wording assumes) built the translated blob and the full Puffin
file end to end; any reviewer can reproduce it from this table alone.

**Deletion-vector-v1 blob payload** (46 bytes, this segment's Puffin blob
content):

| field                    | size | value                                    | bytes (as stored)                                        |
| ------------------------ | ---- | ----------------------------------------- | ---------------------------------------------------------- |
| combined_length          | 4    | 38 (magic + vector bytes)                 | `00 00 00 26` (big-endian)                                  |
| magic                    | 4    | `D1 D3 39 64`                              | `D1 D3 39 64`                                               |
| bitmap_count             | 8    | 1                                          | `01 00 00 00 00 00 00 00` (little-endian)                   |
| key[0]                   | 4    | 0                                          | `00 00 00 00` (little-endian)                                |
| bitmap[0]                | 22   | STRAND's own deletion-vector bytes, as-is  | `3A 30 00 00 01 00 00 00 00 00 02 00 10 00 00 00 02 00 05 00 64 00` |
| crc32                    | 4    | `CRC32(magic ‖ bitmap_count ‖ key[0] ‖ bitmap[0])` = `0x85872AAD` | `85 87 2A AD` (big-endian) |

**Footer payload JSON** (this RFC's own canonical field order, compact
form — Puffin itself imposes no key-order requirement; the order below is a
STRAND-authored-file convention, stated so golden files reproduce it exactly,
not a Puffin normative rule):

```
{"blobs":[{"type":"deletion-vector-v1","fields":[],"snapshot-id":-1,"sequence-number":-1,"offset":4,"length":46,"properties":{"referenced-data-file":"segments/0000000000000001.strand","cardinality":"3"}}],"properties":{"created-by":"strand-tools 0.1.0 (rfc-0013-puffin-export)"}}
```

279 bytes, UTF-8, uncompressed (this worked example sets the footer's
compression flag bit to `0`; a real export of many blobs may prefer LZ4
compression, Design's Non-goals notwithstanding for the JSON footer itself,
which Puffin's own compression applies to independently of any blob's own
codec).

**Full Puffin file** (345 bytes total):

| region                | offset | size | value                       |
| ---------------------- | ------ | ---- | ---------------------------- |
| file magic              | 0      | 4    | `50 46 41 31`                  |
| blob 0 (deletion-vector-v1) | 4  | 46   | the 46-byte table above        |
| footer magic             | 50     | 4    | `50 46 41 31`                  |
| footer payload           | 54     | 279  | the JSON above, UTF-8          |
| footer_payload_size      | 333    | 4    | `17 01 00 00` (279, little-endian signed) |
| flags                    | 337    | 4    | `00 00 00 00`                  |
| trailing magic           | 341    | 4    | `50 46 41 31`                  |

A real, complete, byte-exact 345-byte file: `50 46 41 31 00 00 00 26 D1 D3 39
64 01 00 00 00 00 00 00 00 00 00 00 00 3A 30 00 00 01 00 00 00 00 00 02 00 10
00 00 00 02 00 05 00 64 00 85 87 2A AD 50 46 41 31 {279 bytes of the JSON
footer above} 17 01 00 00 00 00 00 00 50 46 41 31`. A future implementation
session pins this exact file as `conformance/puffin/toy-deletion-vector.
puffin`, matching `spec/container.md`'s own golden-file discipline.

Any real Puffin reader — including `apache/iceberg-rust`'s own
`PuffinReader::new(input_file)`, which opens a bare file with no surrounding
Iceberg table (`references/puffin-spec-and-iceberg-rust-implementation.md`)
— given this exact file, reads back `blob.blob_type() ==
"deletion-vector-v1"`, `blob.data()` equal to the 46-byte payload above, and
(for a reader that also implements Puffin's own `deletion-vector-v1`
semantics, not merely the generic blob envelope) the deleted-position set
`{2, 5, 100}` against `referenced-data-file =
"segments/0000000000000001.strand"` — the same three rows `rfcs/0012-
deletion-vectors.md`'s own worked example tombstones, recovered through a
reader that has never heard of STRAND.

## Napkin math (`CLAUDE.md` §7)

**This RFC does not touch STRAND's own cold query path, and says so rather
than filling in the arithmetic pro forma.** A Puffin sidecar is never named
by a `SegmentRef` or by any snapshot metadata field (`spec/manifest.md` §1);
`spec/container.md`'s open protocol (§3) and its two-round-trip budget
(invariant 3) govern *opening a STRAND segment*, and nothing about opening a
segment changes here. The sidecar is produced on demand, by a caller who
already has the segment open, and consumed — if at all — by a tool that is
not STRAND's own reader and is not bound by invariant 3 in the first place
(nothing in this repository can bind an external tool's own round-trip
behavior). The napkin-math rule exists for cold-path *structures*; this RFC
adds an export *artifact*, not a structure any STRAND query depends on.

What is worth sizing honestly, because §7's own discipline is "justified in
GETs and bytes read, not only in CPU" wherever bytes are actually produced:
the sidecar's own size. `rfcs/0012-deletion-vectors.md`'s own napkin math
already bounds a deletion vector's worst case — every local ordinal
tombstoned in a segment at RFC 0010's sizing law (~760,000 rows) — at "well
under 1 MB" (roughly 12 32-bit-key Roaring containers, each bounded at an
8 KB dense-bitmap-container representation). This RFC's translation adds a
fixed, small overhead on top of that per-segment bitmap: 4 bytes
(combined_length) + 4 bytes (magic) + 8 bytes (bitmap_count) + 4 bytes (one
key, since `row_id_count <= 2^32` per `spec/deletion.md` §1 means exactly one
key group regardless of scale) + 4 bytes (CRC-32), plus the Puffin file's own
fixed envelope (magic, footer magic, `footer_payload_size`, flags, second
magic — 20 bytes) and one JSON footer entry (a few hundred bytes, as the
worked example's own 279-byte payload for one blob shows). At worst-case
segment scale this totals roughly 1 MB, a rounding error against any budget
this project states, and irrelevant to the 100 MB cold-open byte budget
specifically, since that budget governs STRAND's own query-time fetches and
this artifact is never fetched at query time.

## Invariant-11 checklist

**Byte-determinism of this RFC's own output, given the same logical input.**
Yes, for the translated `deletion-vector-v1` blob: every field above is a
deterministic function of STRAND's own already-deterministic deletion-vector
bytes (`spec/deletion.md` §1's own `SERIAL_COOKIE_NO_RUNCONTAINER` MUST) plus
fixed constants Puffin's spec pins exactly (the magic sequence, the framing
layout). For the JSON footer: deterministic **given this RFC's own pinned
key order** (stated in the worked example) — Puffin's own spec pins no key
order, so this determinism is a STRAND-authored-writer convention, not a
Puffin requirement, stated as such rather than implied to be normative
upstream.

**Endianness.** Deliberately mixed, and not STRAND's choice: Puffin's own
`deletion-vector-v1` framing is big-endian for `combined_length` and the
CRC-32, little-endian for the inner Roaring-bitmap-list wrapper (`bitmap_
count`, each `key`) — the spec's own stated reason is Delta Lake wire
compatibility, quoted in full in Design §4 and
`references/puffin-spec-and-iceberg-rust-implementation.md`. This is a
foreign format's own byte order, adopted verbatim per invariant 8 ("don't
invent encodings... registered codec"), not a violation of invariant 11's
little-endian rule for STRAND's own wire structures — invariant 11 governs
STRAND's native format; this RFC's output is explicitly not that.

**Checksum.** Puffin's `deletion-vector-v1` blob carries its own CRC-32,
computed and verified per that type's own spec (Design §4) — a **second**
checksum algorithm in this repository, alongside invariant 11's own
xxHash3-64 default, scoped **only** to bytes this RFC's export path produces.
STRAND's own on-disk objects (segments, deletion vectors, manifest objects)
keep using xxHash3-64 exactly as before; nothing here touches that default.

**Codec registration.** This RFC registers no new STRAND-native codec and
extends no `family_id`/`blob_type_id` pair (Design §3). It cites and reuses
Puffin's own already-registered `deletion-vector-v1` byte layout, verbatim,
against the primary source now vendored in `references/`, satisfying
invariant 8 for translated content precisely by inventing nothing.

**No Rust reference implementation of Puffin itself exists to check this
worked example against automatically, at draft time.** `references/
puffin-spec-and-iceberg-rust-implementation.md` records the crates.io search
that found no crate under the `puffin` name implements this file format at
all — the real implementation lives inside `apache/iceberg-rust`'s own
`iceberg` crate, a dependency a future implementation session could add and
cross-check against directly (mirroring RFC 0006's roaring cross-check and
RFC 0011's compiled-C++ cross-check), but this RFC's worked example is
Python-script-computed and hand-verified against the spec's prose, not yet
machine-cross-checked against that crate. Named as a real, open gap
(Open questions, below), not glossed as equivalent to the stronger
verification this project's other worked examples carry.

## How this could be wrong

**Nearest grave: Pilosa, and — sharper than Pilosa — Puffin's own entry in
`docs/lineage.md` is already this exact critique, aimed at the thing this RFC
proposes exporting into.** `docs/lineage.md` states it about Puffin itself:
"Puffin's registry has spawned essentially no third-party blob ecosystem
despite Iceberg's gravity. The pattern is a good container, not a
distribution strategy" — nearly the same sentence the graveyard uses for
Pilosa: "a good structure with a spec is not a distribution strategy." The
real risk this RFC carries is not that the design is technically wrong — the
worked example round-trips real bytes correctly against the real spec — it
is that the design could be **technically sound and still pointless**: a
Puffin export path built to satisfy a milestone checklist line, consumed by
nobody, because the niche Puffin itself occupies within Iceberg's own
ecosystem is already narrow, and STRAND is not Iceberg.

This RFC does not resolve that risk away, but it does have one real, checked
fact to weigh against it rather than assert past it: `apache/iceberg-rust`'s
`PuffinReader::new(input_file: InputFile)` is real, current, Apache-2.0,
official-ASF-project code (1.85 million downloads at fetch time) that opens
a **bare** Puffin file — no Iceberg table, no catalog, no manifest required
(`references/puffin-spec-and-iceberg-rust-implementation.md`). A tool that
already depends on the `iceberg` crate for an unrelated reason can, today,
read a file this RFC's design produces and get `blob_type() ==
"deletion-vector-v1"` and the correct `data()` bytes back, no STRAND-specific
code involved. That is a real, if narrow, consumer that exists independent
of this RFC — not a hypothetical one this RFC invented to justify itself.
Whether any such consumer will ever actually point that reader at a STRAND
sidecar is a question this RFC cannot answer by design alone; it is named
here as the genuinely open bet this RFC makes, not resolved by asserting
confidence past what the evidence supports.

**A second, narrower risk: the `referenced-data-file` semantic mismatch**
(Design §4's own admission). Puffin's `deletion-vector-v1` type was built for
Delta-Lake-style positional deletes against Parquet data files inside a real
Iceberg table; its `referenced-data-file` property is specified to "be equal
to the data file's `location` in table metadata." A STRAND segment path in
that field will not resolve against any Iceberg table metadata a generic
Puffin-and-Iceberg-aware tool might expect to cross-reference it with. A tool
that reads the blob generically (as `apache/iceberg-rust`'s `Blob` API
does — `data()`, `properties()`, no built-in cross-reference) is unaffected;
a tool that specifically tries to resolve `referenced-data-file` against a
real Iceberg catalog will fail to find it, correctly, since no such table
exists. This RFC accepts that gap rather than inventing a fictitious table
to paper over it, and names it here so a future session does not rediscover
it as a surprise.

**A third risk, procedural rather than technical: this RFC's own Status is
Draft, and it says exactly why in its own header** — the design decision to
build a second wire format and a second checksum algorithm purely for
narrow, unproven interop value is precisely the kind of call `CLAUDE.md` §3's
"agent designs, agent implements — but not in the same breath" principle
means to protect against being rubber-stamped by the same pass that drafted
it. Leaving Status as Draft, rather than self-declaring Approved the way
every other RFC in this repository does after its own inline review, is
itself part of this RFC's answer to "how this could be wrong": by construction,
not yet resolved by this session alone.

## Alternatives considered

**A Puffin-shaped container profile for STRAND's primary segment format**
(covered fully in Design §1). Rejected: Puffin's `BlobMetadata` schema has no
slot for `field_id`, `storage_class`, `tier`, `alignment`, or a per-blob
checksum on every blob type, so adopting it as STRAND's own container would
mean silently dropping invariants 7, 10, and 11's guarantees and §5a's
field-disambiguation mechanism, or inventing extension fields Puffin's spec
does not define — the latter repeating exactly the mistake invariant 8
exists to prevent.

**Registering new Puffin blob types for STRAND's lexical and vector blobs
with Iceberg's own community process**, so a Puffin-aware tool could
interpret STRAND's postings or vector codes semantically, not just
opaquely. Rejected for this RFC: that is a real, multi-party proposal this
project does not control the outcome of, is out of scope for a single
session's design work, and would commit Iceberg's own spec to formats
(BitPacker8x-encoded postings, RaBitQ-quantized vector codes) that have no
meaning outside a STRAND-aware reader in the first place — Design §5's
opaque passthrough gets the honestly-available fraction of this benefit
(structural visibility) without that dependency.

**Referencing the Puffin sidecar from `spec/manifest.md`'s `SegmentRef`**,
so every reader would automatically know a sidecar exists for a given
segment. Rejected: this would make the sidecar part of STRAND's own commit
protocol and deletion-safety accounting (`CLAUDE.md` §6) for an artifact no
STRAND reader ever needs to open, adding real protocol surface — a new
optional reference field, a new orphan-sweep case — for zero query-path
benefit. An on-demand, caller-driven export (Design §2) gets the same
outcome for any caller who actually wants one, with no manifest change at
all.

## Open questions / follow-on RFCs

- **Cross-check the worked example against `apache/iceberg-rust`'s real
  `PuffinReader`**, not only against this RFC's own Python-script
  computation — the same discipline RFC 0006 applied to the `roaring` crate
  and RFC 0011 applied to a compiled C++ reimplementation. Requires adding
  the `iceberg` crate as a dev-dependency somewhere this project's own build
  can reach it and confirming Apache-2.0 compatibility formally (its license
  header reads Apache-2.0; a full audit like R3's or R9's has not been done
  here).
- **`fields: []` for a non-columnar blob** — this RFC's own choice (Design
  §4), since Puffin's spec never addresses what a whole-row, non-per-column
  blob's `fields` list should contain. Not contradicted by anything fetched,
  but not confirmed against a second real Puffin writer's convention either;
  worth checking against `apache/iceberg-rust`'s own Puffin *writer* path (a
  different module from the `reader.rs`/`blob.rs` this RFC's grounding
  covers) before implementation.
- **Whether `strand-segment-blob-v1`'s opaque-passthrough design is worth
  building at all**, given How this could be wrong's own honest admission
  that it buys structural visibility only. A future session could narrow
  M4-5's remaining scope to the deletion-vector translation alone (Design
  §4) and drop Design §5 entirely, if implementation experience finds no
  real caller for the opaque form.
- **A real S3-hosted round-trip test**: write a sidecar via the (not yet
  built) `strand-tools export-puffin`, fetch it back over real or MinIO
  object storage, and confirm the same tail-read trick `spec/container.md`
  §3 uses for STRAND's own segments (Puffin's footer is also
  footer-last-with-trailing-magic, so the same speculative-tail-GET pattern
  should work for a Puffin-only reader too) — this RFC states the structural
  resemblance but never measures it, since no implementation exists yet to
  measure.
- **Whether this RFC should also cover `apache-datasketches-theta-v1`** for
  STRAND's own collection statistics (`spec/scoring-profiles.md`'s exact
  term/document counts), translating an exact count into a sketch. Not
  attempted here: STRAND's collection stats are exact, not approximate, and
  downgrading them to a lossy sketch for export has no motivating use case
  named anywhere in this project's own documents. Left open rather than
  built speculatively.
