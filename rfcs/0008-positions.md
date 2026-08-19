# RFC 0008: Positions

- **Status:** Approved. Adversarial review re-fetched every live source cited
  (Lucene99PostingsFormat javadoc, both tantivy pages), re-derived the worked
  example and every napkin-math figure independently, and checked internal
  section citations against the RFC's own actual heading numbers. Found 3
  Critical, 2 Important, and 2 Minor findings, all fixed. Critical: (1)/(2)
  the worked example's "Full blob" line and Design §2's "minimum blob size"
  claim both silently dropped the `postings_block_pos_prefix` region from
  the byte count — an identical 4-byte omission in two places, raising
  confidence it was a real systematic slip rather than one typo. Fixed:
  the worked example is now 12 bytes (`06 00 00 00 00 00 00 00 03 33 32
  03`, re-verified by independently re-executing the packer), and the
  stated minimum blob size is now 9 bytes, with its citation corrected from
  a nonexistent "§4" to "§7" (Layout). (3) The Napkin math section's
  combined MB totals didn't reproduce from its own stated per-component
  figures (understated by ~2% on the tighter bound, overstated by ~4.3% on
  the conservative bound) — fixed by recomputing every component exactly
  (delta stream, `pos_widths`, `postings_block_pos_prefix`, `total_term_freq`
  overhead) and restating both the positions-alone range (≈19.28–≈27.39 MB)
  and the combined-with-RFC-0007 range (≈92.48–≈100.59 MB) precisely, with
  the downstream figures in "How this could be wrong" and "Alternatives
  considered" updated to match — the corrected conservative bound still
  exceeds the 100 MB budget, so the RFC's central conclusion holds, arguably
  sharpened, not undermined. Important: two internal citations pointed to
  `spec/postings.md` §7 (Merge semantics) for the skip query, which actually
  lives in §6 — the exact citation-drift class RFC 0007's own review caught
  — fixed in both places; and a claim that `spec/postings.md` §6 names the
  `block_max` u32-range assumption was wrong (that argument lives only in
  `rfcs/0007-postings-codec.md` Design §6, not the settled spec chapter) —
  fixed to cite the RFC directly rather than a spec citation that doesn't
  carry the argument. Invariant 4 was listed in the header but never
  substantively engaged — fixed by adding a paragraph to Design §8 explaining
  that this blob carries no independent pruning bound by design (a phrase
  query always resolves its target document via postings' own `block_max`
  first), rather than dropping the invariant from scope. Minor: a
  mislabeled blob-type-id cross-reference ("RFC 0007's `postings_offset` at
  `2`", conflating a `TermInfo` field name with the registered blob name)
  and a `pos_widths` upper-bound off-by-one (494,427 → 494,428), both fixed.
  The review independently re-verified every quoted sentence in
  `references/lucene99-and-tantivy-position-formats.md` against the live
  source pages, the worked example's multi-block targeted-lookup path via a
  hand-constructed synthetic case, and every RFC-0007-machinery-reuse claim
  against the actual `crates/strand-lexical/src/postings.rs` source, finding
  no further issues.
- **Milestone:** M1 — Lexical (`docs/milestones.md`)
- **Spec chapters produced:** `spec/positions.md`; additively extends
  `spec/container.md` §9 (registers `family_id = 1` "lexical",
  `blob_type_id = 3` "positions")
- **Invariants exercised:** 1, 3, 4, 8, 9, 10, 11 (`CLAUDE.md` §5)

## Summary

Registers STRAND's positions blob: per-term, within-document token-position
delta-gaps, closing the gap RFC 0007 explicitly deferred ("This RFC does not
register positions") and the gap `spec/term-dictionary.md` §3's `TermInfo`
record has reserved `positions_offset`/`positions_length` fields for since
RFC 0005, unused until now. The encoding reuses RFC 0007's already-approved,
already-implemented codec shape wholesale — `BitPacker8x` SIMD-packed
256-value blocks, a scalar-packed variable-length final block, a per-block
bit-width array stored apart from the packed bytes — applied to a
differently-shaped stream: positions reset to zero at every document
boundary (Lucene's `PositionDelta` convention), and the total count a term's
position blocks must cover is `totalTermFreq` (the sum of that term's
per-document term frequencies), not `doc_freq`. Because that total isn't
recoverable without decoding, this RFC stores it as a small leading field
inside the positions blob itself, rather than growing `TermInfo`'s
already-implemented, already-golden-filed 28-byte record. A new per-postings-
block prefix-count region (`postings_block_pos_prefix`) bridges postings-
block space to position-stream space, so a reader that has already located a
target document via `spec/postings.md` §6's skip query can find that
document's position run without decoding any positions block it doesn't need
— the same problem Lucene's `.pos`/`.pay` skip file-pointers solve for a
round-trip-bound reader, solved here for STRAND's fully-resident-after-open
model, where the cost avoided is CPU-bound block decode, not a round trip.

## Motivation

**The milestone gap.** `CLAUDE.md` §1 lists "postings, positions, term
stats, block-max bounds" as what the lexical family delivers; §10's M1 entry
names "BP128 postings + positions + FST term dictionary + block-max sibling
blob + Roaring filter bitmaps" as one deliverable. RFC 0007 (postings), RFC
0005 (term dictionary), and RFC 0006 (filter bitmaps) are all Approved and
implemented; positions is the one M1 lexical-family blob still undefined.
Without it, STRAND cannot answer a phrase query ("machine learning", not
just "machine" OR "learning") — a real, table-stakes lexical-search
capability, not a nice-to-have.

**Real prior-art grounding, fetched and vendored, not recalled.** Both
Lucene99 (`.pos`/`.pay`) and tantivy (`.pos`) were fetched directly for this
RFC (`references/lucene99-and-tantivy-position-formats.md`), per `CLAUDE.md`
§3's rule against implementing against a remembered spec. Both use the same
shape STRAND's own RFC 0007 already registers for postings — fixed-size
SIMD-packed blocks, a per-block bit-width array kept apart from the packed
bytes, a variable-length tail — which is why this RFC reuses that shape
rather than inventing a third. Both also confirm a structural fact this
RFC's design depends on: position blocks are sized from `totalTermFreq`
(Lucene: "`PackedPosBlockNum = floor(totalTermFreq/PackedBlockSize)`"), not
from `doc_freq` — position-stream block boundaries and postings-stream block
boundaries are independent counts, not the same partition seen twice.

**Two real prior-art choices this RFC deliberately does not follow, named
precisely.** (1) Lucene uses 128-value blocks for `.pos` (same as `.doc`);
this RFC registers 256-value blocks instead, reusing `BitPacker8x` — already
registered, already implemented, already fuzz-tested for postings — rather
than adding a second bit-packer dependency (`BitPacker4x`, a different,
incompatible format) for one block family. (2) tantivy's own documented
approach locates a target document's positions by sequentially accumulating
term-frequency counts across every preceding document in the postings
iteration ("As we iterate through the docset, we advance the position
reader by the number of term frequencies of the current document") — no
per-block skip index into the position stream at all. This is a real,
working design for tantivy's warm, memory-mapped access pattern, but it is
exactly the dependent, unbounded-decode shape invariant 3 rules out for a
cold segment open. This RFC's `postings_block_pos_prefix` region exists
specifically so STRAND does not inherit tantivy's sequential-accumulation
cost.

## Non-goals

**Payloads and per-position offsets** (byte start/end spans, arbitrary
per-position metadata) are not registered here. Lucene supports both but
physically separates them from bare positions into `.pay` specifically so a
query needing only positions doesn't pay for data it doesn't need
(`references/lucene99-and-tantivy-position-formats.md`) — this RFC follows
that separation by simply not building the payload/offset side at all until
a real use case demands it (invariant 8's minimal-novelty rule), not by
interleaving it and pretending it's free.

**Proximity/sloppy-phrase scoring** (edit-distance-tolerant phrase matching,
term-proximity as a ranking signal) is not designed here. This RFC defines
where positions live and how a reader locates them; how a query layer scores
proximity is explicitly out of the format's scope per `CLAUDE.md` §1
("Query-time fusion logic, ranking models... do not belong in the spec").

**A field-level schema flag for "does this field carry positions"** is not
registered. A field's positions blob either exists (with real, non-zero
`positions_length` on every term with occurrences) or it doesn't — the
registry entry's own presence or absence is the signal, matching how
`spec/container.md` §5 already models every blob as an entry in a flat,
unenumerated registry list rather than a fixed schema with presence bits.
No new invariant-11 registration surface is spent here.

**ARM/non-AVX2 validation** is inherited, not re-opened. This RFC's block
codec is the identical `BitPacker8x` registration RFC 0007 already named as
genuinely unmitigated on ARM (`rfcs/0007-postings-codec.md`'s own "How this
could be wrong"); this RFC does not re-measure or re-argue that gap, only
notes it applies equally here.

**A validated batch-size range** (invariant 9) remains open, as RFC 0007
left it.

**Multi-field blob addressing** — how a segment's flat blob registry
disambiguates two fields' positions blobs sharing the same `blob_type_id` —
is not solved here. Neither RFC 0005, 0006, nor 0007 solved this for the
term-dictionary, filter-bitmap, or postings blobs either; this RFC inherits
whatever mechanism eventually resolves it for those, not a new one.

## Design

### 1. Blob registration

`family_id = 1` (lexical, already registered by RFC 0005), `blob_type_id = 3`
("positions", the next free ID after RFC 0007's `postings` blob at `2`),
`storage-class: raw-mappable`, `tier: cold-fetchable` — identical
classification to postings (`spec/postings.md` §4): already dense/bit-packed
content, `storage-class` governs only whether a further chunk codec wraps
it.

### 2. Scope: one positions blob per field, addressed via already-reserved `TermInfo` fields

Each term in a field's term dictionary has at most one positions region,
addressed by that term's `TermInfo.positions_offset`/`positions_length`
(`spec/term-dictionary.md` §3 — reserved since RFC 0005, unused until this
RFC). **`positions_length == 0` means this field carries no stored
positions for this term** — a real, always-nonzero blob is written for every
term with at least one occurrence (§7 below shows the minimum size is 9
bytes: `total_term_freq`(4) + one `postings_block_pos_prefix` entry(4) +
one `pos_widths` entry(1) + a possibly-empty stream, for the smallest case
of `doc_freq = total_term_freq = 1`), so zero is unambiguous. A writer
either stores real positions for
every term in a field or omits them for every term in that field; there is
no per-term opt-out within a field that does support positions.

### 3. The d-gap variant (invariant 11's required complete registration)

Values are **within-document token positions** — `0`-based indices into a
document's token stream after the field's declared analyzer chain
(`spec/analyzer-descriptors.md`), the same notion of position invariant 6
already requires an analyzer descriptor to pin. The delta sequence resets
at every document boundary: for a document contributing `tf` positions to
this term, `delta[0] = position[0] - 0`, `delta[i] = position[i] -
position[i-1]` for `i >= 1` — identical in shape to `spec/postings.md` §2's
own delta-from-zero convention, just reset once per document instead of
once per whole term (Lucene's own `PositionDelta` semantics, quoted in
`references/lucene99-and-tantivy-position-formats.md`). Deltas from
different documents are concatenated directly, in postings order, with no
separator: a decoder that already knows each document's `tf` (from the
postings blob, already resident) knows exactly how many consecutive deltas
belong to each document without needing an explicit boundary marker.
Little-endian throughout, `BitPacker8x`'s vertical SIMD layout for full
blocks — the identical registration RFC 0007 §3 already made, not repeated
by reference alone.

### 4. `total_term_freq`: a leading field inside the blob, not a `TermInfo` growth

Unlike `doc_freq` (already in `TermInfo`, free to a reader before this blob
is fetched), the total position count a term's blocks must cover —
`totalTermFreq`, the sum of that term's per-document term frequencies — is
not recoverable without decoding the postings blob's entire term-frequency
stream. Storing it in `TermInfo` would grow RFC 0005's already-implemented,
already-golden-filed 28-byte fixed record (`spec/term-dictionary.md` §3),
breaking byte-determinism for every already-shipped conformance vector.
This RFC instead stores it as the first 4 bytes of the term's own positions
region: `total_term_freq: u32`, little-endian. `position_block_count =
ceil(total_term_freq / 256)` follows directly, the same
computed-not-stored pattern `spec/postings.md` §3 already uses for
`block_count`. `total_term_freq` inherits the same realistic-range
assumption `rfcs/0007-postings-codec.md` Design §6 already named for
`block_max` (`u32` against an unbounded `row_id_count: u64`) — a term
occurring more than 2^32 times in one segment would overflow this field, a
real, named, inherited gap, not a new one. That assumption is argued in RFC
0007's own text, not restated in the settled `spec/postings.md` chapter
itself; this RFC inherits the argument, not a spec citation that doesn't
carry it.

### 5. Block structure: fixed 256-value blocks, variable-length final block

Identical mechanics to `spec/postings.md` §3, applied to the position-delta
stream instead of the doc-ID-gap stream: every block except possibly the
last covers exactly 256 deltas, packed with `BitPacker8x`'s SIMD kernel at
that block's own bit width; the final block, when `total_term_freq` is not
an exact multiple of 256, covers exactly the real remainder and is packed
with the identical scalar bit-packer RFC 0007 already implements
(`crates/strand-lexical/src/postings.rs`'s `scalar_pack`/`scalar_unpack`) —
this RFC's implementation reuses those functions directly rather than
duplicating them.

### 6. `postings_block_pos_prefix`: the bridge from postings-block space to position-stream space

One `u32` per **postings** block (count = `ceil(doc_freq / 256)`, `doc_freq`
already known from `TermInfo`, external to this blob) — `postings_block_pos_prefix[i]`
is the total count of positions (summed `tf` across every document) that
precede postings block `i`'s first document. This is the region that lets a
reader who has located a target document's postings block `lo` (via
`spec/postings.md` §6's skip query, which already decodes block `lo` to
verify the match) find that document's position-stream start index in O(1)
plus a sum over only the documents in block `lo` that precede the target —
never requiring a decode of any postings block before `lo`, and never
requiring a decode of any position block before the target's own. Entries
are `u32`, inheriting the same `total_term_freq` range assumption (§4).

### 7. Layout

| region                       | size                                    | notes                                                                    |
| ----------------------------- | ---------------------------------------- | ------------------------------------------------------------------------- |
| `total_term_freq`             | 4 bytes                                  | §4; little-endian `u32`                                                   |
| `postings_block_pos_prefix`   | `4 * postings_block_count` bytes         | §6; `postings_block_count = ceil(doc_freq / 256)`, `doc_freq` external    |
| `pos_widths`                  | `position_block_count` bytes             | one `u8` per position block; `position_block_count = ceil(total_term_freq / 256)` |
| position-delta stream          | sum of each block's packed bytes          | full blocks via `BitPacker8x`; final block via the scalar packer, §5      |

A block's packed byte length is `ceil(block_real_len * width / 8)`, where
`block_real_len` is `256` for every position block except a shorter final
block — identical arithmetic to `spec/postings.md` §4.

### 8. Query resolution

**Targeted lookup** (phrase query resolving one candidate document, the
common case): given a target document already located via
`spec/postings.md` §6's skip query — which yields postings block index
`lo`, that document's within-block position, and its `tf` — a reader: reads
`total_term_freq` and `postings_block_pos_prefix[lo]` (both already-resident,
O(1) reads); sums the `tf` of every document in block `lo` strictly before
the target (already decoded during the postings skip, no new decode); adds
that sum to `postings_block_pos_prefix[lo]` to get `start_index`, the
target document's first position's index in this term's overall position
stream; locates `start_block = start_index / 256` and, since a document's
run may straddle a block boundary, `end_block = (start_index + tf - 1) /
256`; computes `start_block`'s byte offset by summing `pos_widths[0..start_block]`'s
packed lengths (the identical `stream_offset` pattern `crates/strand-
lexical/src/postings.rs` already implements); decodes blocks
`start_block..=end_block` and slices to the `tf` deltas beginning at
`start_index`; reconstructs absolute in-document positions via a running
sum that starts at `0` exactly at that slice's first delta (§3's
reset-per-document convention means no additional boundary detection is
needed — the slice's first delta already encodes "distance from position
0").

**Full decode** (every position for every document a term occurs in — used
to build a full phrase-scan structure, not the common per-query path):
walk the position-delta stream sequentially, block by block, while walking
the postings blob's already-decoded term-frequency array in parallel,
resetting the running sum to `0` each time the parallel walk crosses a
document boundary.

**Why this blob carries no independent pruning bound (invariant 4).** This
blob has no `block_max`-equivalent region of its own, unlike postings. That
is a deliberate consequence of the Targeted-lookup algorithm above, not an
oversight: a phrase query never skips *within* the position stream on its
own — it always arrives at a specific target document via `spec/postings.md`
§6's skip query first, which already applies postings' own `block_max`
pruning before this blob is touched at all. There is no query shape this
blob needs to serve where a reader has a target position (rather than a
target document) and needs to prune position blocks without already
knowing which document it's looking for. Invariant 4's pruning-metadata
commitment is therefore satisfied for phrase queries by the postings
blob's existing `block_max` region (`spec/postings.md` §5), inherited
rather than duplicated here — this RFC registers `postings_block_pos_prefix`
as an offset bridge, not a second pruning structure, and the two should not
be conflated.

### 9. Merge semantics (invariant 1)

**Rebuild**, inherited directly from `spec/postings.md` §7 rather than
independently argued: a segment merge that combines multiple segments'
postings for a term already re-decodes and re-encodes that term's entire
postings list from scratch (RFC 0007's own declared strategy), and this
blob's `postings_block_pos_prefix` region is defined entirely in terms of
that rebuilt postings blob's own block boundaries — there is no
concatenate-and-remap option here even in principle, because the bridge
region's values are a direct function of the postings blob's block
structure, which itself gets rebuilt. Positions therefore rebuild whenever
postings do, as a structural consequence, not a separately chosen policy.

### 10. Placement constraint, and why this blob's size matters more than most

Identical in spirit to `spec/postings.md` §8: part of the cold-fetchable
wave invariant 3 already budgets for after the segment open, adding bytes
but no additional round trip. Unlike most cold-fetchable additions,
though, this one's byte contribution is large enough to matter on its own
— the Napkin math below measures it directly, and a field that does not
need phrase-query support SHOULD omit its positions blob entirely (§2) to
avoid paying that cost for every query, not only phrase queries, since the
whole wave's bytes are fetched regardless of what a specific query needs.

## Worked example

The identical three-posting term RFC 0007's own worked example used — local
ordinals `5, 12, 47`, term frequencies `2, 1, 3` — extended with concrete
within-document token positions, chosen by hand and verified by executing
both the packer and the design's own query-resolution algorithm in Python
(not hand-derived arithmetic):

- Document at ordinal `5` (`tf = 2`): term occurs at token positions `3`
  and `9`. Deltas: `3 - 0 = 3`, `9 - 3 = 6`.
- Document at ordinal `12` (`tf = 1`): term occurs at token position `0`.
  Delta: `0 - 0 = 0`.
- Document at ordinal `47` (`tf = 3`): term occurs at token positions `1`,
  `4`, `10`. Deltas: `1 - 0 = 1`, `4 - 1 = 3`, `10 - 4 = 6`.

Flattened delta stream, in postings order: `3, 6, 0, 1, 3, 6` — 6 values,
matching `total_term_freq = 2 + 1 + 3 = 6`. Well under 256, so
`position_block_count = 1` (the variable-length final block only, same as
RFC 0007's own worked example). This term's postings also fit in one block
(`postings_block_count = 1`, RFC 0007's worked example), so
`postings_block_pos_prefix` has exactly one entry: `0` (no positions
precede the term's only postings block).

Maximum delta value is `6` (`0b110`), needing 3 bits.

**`total_term_freq`** (4 bytes, little-endian `u32` `6`): `06 00 00 00`.

**`postings_block_pos_prefix`** (4 bytes, one `u32` entry, value `0`):
`00 00 00 00`.

**`pos_widths`** (1 byte, one entry, value `3`): `03`.

**Position-delta stream** (packing `3, 6, 0, 1, 3, 6` at width 3,
LSB-first, via the identical `scalar_pack` RFC 0007 already implements —
executed, not hand-derived): `33 32 03`.

**Full blob, 12 bytes** (`total_term_freq` + `postings_block_pos_prefix` +
`pos_widths` + stream, §7's layout order):
`06 00 00 00 00 00 00 00 03 33 32 03`.

Resolving a targeted lookup for the document at ordinal `47` (postings
block `lo = 0`, that block's decode already yields `tf = 3` for this
document and reveals it is the third and last document in the block):
`postings_block_pos_prefix[0] = 0`; sum of `tf` for documents before it in
block `0` is `2 + 1 = 3` (ordinals `5` and `12`); `start_index = 0 + 3 =
3`; `start_block = 3 / 256 = 0`, `end_block = (3 + 3 - 1) / 256 = 0` — a
single block covers the whole run. Decoding `pos_widths[0] = 3` at the
computed offset and slicing to the 3 deltas beginning at index `3` within
the decoded block (`[3, 6, 0, 1, 3, 6][3..6] = [1, 3, 6]`) and running a
prefix sum from `0` recovers `1, 4, 10` — exactly the document-47
positions this example started from, confirmed by executing the full
resolution algorithm, not asserted.

## Napkin math (`CLAUDE.md` §7)

Positions are cold-fetchable-wave payload, identical in round-trip
accounting to postings and term-info (RFC 0007's own napkin math, which
this RFC does not repeat) — what changes is bytes, and this RFC's own
Placement-constraint section (§10) already flags that the change is large.

**The real, measured input.** This session extended `bench/src/
msmarco_index.rs` (already the source of RFC 0007's own real MS MARCO
numbers) to also sum `total_term_occurrences` — the sum of every posting's
term frequency, i.e. exactly the total count of position deltas a positions
blob would store, across the sample — and to track document length.
Re-run against the same cached 520,108-passage (5.88% of 8,841,823), real
Tevatron/msmarco-passage-corpus sample RFC 0007 used
(`bench/results/msmarco-real-postings-sample.json`, regenerated this
session): `total_term_occurrences = 20,752,140` across `413,364` distinct
terms and `15,680,663` postings; mean document length (post-analysis, i.e.
after stopword removal) `≈ 39.9` tokens; maximum sampled document length
`407` tokens.

**A stated bound, not a real per-block measurement.** This session did not
extend the sample to track actual within-document token positions (only
per-document term counts), so no real position-delta bit-width histogram
exists yet — unlike RFC 0007's own gap/tf widths, which came from real
decile samples. This RFC's byte estimate is therefore a **bound**, stated
as such, using two bit-width assumptions bracketing the real answer: (1) a
conservative per-block width bound using this sample's single longest
document (`407` tokens, `9` bits, `⌈log2(407)⌉`) as if every block needed
it — almost certainly a large overestimate, since within-document
positions of the same term are usually far closer together than the
document's full length; (2) a tighter bound using the sample's *mean*
document length (`≈ 40` tokens, `6` bits) as a per-block proxy, still an
estimate, not a measurement. Real per-block widths, once measured (Open
questions), will very likely land below even the tighter bound, the same
direction RFC 0007's own real measurement corrected an earlier padding-
inflated estimate.

**The arithmetic**, computed exactly, not rounded until the final figures:
delta-stream bytes are `total_term_occurrences * width / 8` —
`20,752,140 * 6 / 8 = 15,564,105` bytes (`≈ 15.56 MB`, tighter bound) to
`20,752,140 * 9 / 8 = 23,346,157.5` bytes (`≈ 23.35 MB`, conservative
bound). `pos_widths` bytes: `position_block_count` summed across all terms
is bounded between `413,364` (`≈ 0.41 MB`; if nearly every term's
`total_term_freq` stays under 256, contributing exactly one partial block
each — the likely case, given RFC 0007's own finding that 69% of this same
corpus's postings lists have length `<= 8`) and `413,364 +
⌈20,752,140/256⌉ = 413,364 + 81,064 = 494,428` (`≈ 0.49 MB`) in the opposite
extreme. `postings_block_pos_prefix` bytes: bounded the same way using
`total_postings = 15,680,663` in place of `total_term_occurrences`, times 4
bytes per entry: `413,364 * 4 = 1,653,456` bytes (`≈ 1.65 MB`) to
`(413,364 + ⌈15,680,663/256⌉) * 4 = (413,364 + 61,253) * 4 = 1,898,468`
bytes (`≈ 1.90 MB`). `total_term_freq`'s own 4 bytes per term:
`413,364 * 4 = 1,653,456` bytes (`≈ 1.65 MB`, identical in both bounds — it
does not depend on either bit-width assumption).

**Combined: `15,564,105 + 413,364 + 1,653,456 + 1,653,456 = 19,284,381`
bytes (≈ 19.28 MB, tighter bound) to `23,346,157.5 + 494,428 + 1,898,468 +
1,653,456 = 27,392,509.5` bytes (≈ 27.39 MB, conservative bound), for one
field's positions blob alone, on the same 5.88%-of-corpus sample.** Added
to RFC 0007's own real ~73.2 MB figure for that sample's
postings-plus-term-info: **≈ 92.48 MB to ≈ 100.59 MB combined — 92% to
just past the entire 100 MB cold-open budget, for one field, on well under
6% of the corpus.** This is the real, concrete consequence §10 names in the
abstract: positions are not a small addition to the postings budget, they
are comparable in size to postings-plus-term-info themselves, and the
conservative bound alone exceeds the budget outright, on this sample, for
this one field, before accounting for any other field or any vector blob
in the same segment. This is not this RFC softening the number — the
tighter, more realistic bound still
consumes the large majority of the remaining headroom RFC 0007's own
napkin math left. The practical conclusion this RFC draws is the one §10
already states as a design point, not an afterthought: **fields that do not
need phrase-query support should not carry a positions blob**, and R1's
segment-sizing work (`docs/ledger.md`) needs this number, not just the
vector blob's and postings blob's, when it turns to real segment-size
limits.

## Invariant-11 checklist

- **Endianness:** little-endian throughout — `total_term_freq` and
  `postings_block_pos_prefix` (both `u32`), and both bit-packed streams
  (full-block `BitPacker8x` vertical SIMD, final-block scalar LSB-first),
  identical convention to `spec/postings.md`'s own checklist item.
- **Term sort order:** not applicable — positions within this blob follow
  postings order, inherited, not independently sorted.
- **Chunk codec:** not applicable — `storage-class: raw-mappable`.
- **Checksums:** covered by this blob's own registry entry
  (`spec/container.md` §5–§6); no new checksum scope.
- **Codec-variant provenance:** identical registration to `spec/postings.md`
  §3/§5 — `BitPacker8x` (`bitpacking` crate, `quickwit-oss`), vertical SIMD
  layout, 256-value blocks, plus the scalar final-block packer — applied
  here to a within-document-reset delta stream instead of a whole-term
  delta stream, the one real difference from postings' own registration.
- **Stochastic-transform provenance:** not applicable.
- **Golden files:** the worked example above is the first
  `conformance/positions/` vector, once implemented — real bytes, a real
  round trip, a real targeted-lookup resolution, `doc_freq` and the
  postings blob's own bytes supplied externally exactly as a real reader
  would have them.

## How this could be wrong

**Nearest grave (`docs/lineage.md`): CIFF.** "CIFF is a well-made exchange
format no engine runs operationally: conversion required, **no
positions**, no pruning bounds, no analyzer metadata, lossy doc lengths.
Every gap is a MUST here." This RFC exists specifically to close the exact
gap CIFF is named for in this project's own lineage document — a
lightweight interchange format that never became a real index format
partly *because* it stopped at postings. The risk this RFC actually
carries is not "positions are unimportant" (the graveyard entry says the
opposite) but that this RFC's own design — reusing RFC 0007's codec
wholesale, bridging via `postings_block_pos_prefix` — could still leave
STRAND in CIFF's position if the byte cost (Napkin math, above) makes
positions impractical to actually ship as a default rather than an
opt-in, or if a real implementation reveals the targeted-lookup algorithm
(§8) is more expensive in practice than this worked example's single-block
case suggests once documents with `tf` spanning multiple position blocks
are common.

**The `total_term_freq`-as-blob-header design (§4) is untested against a
concrete implementation.** RFC 0007's equivalent design decisions
(`block_count` computed from external `doc_freq`) were validated by real,
executing Rust code before this RFC's own approval process began; this
RFC's parallel decision — computing `position_block_count` from a value
read from *inside* the blob itself, a genuinely different shape (self-
describing rather than externally-driven) — has not yet been implemented or
tested. The worked example above executes the arithmetic in Python, which
catches encoding-logic errors but not Rust-specific implementation risk
(e.g., a reader that tries to read `pos_widths` before establishing
`position_block_count`, or an off-by-one in the block-count formula).

**The Napkin math's bit-width bound is honestly a bound, not a
measurement, and the gap between its two bracketing values (≈19.28 MB vs.
≈27.39 MB, a ≈1.42× spread) is itself evidence real per-block widths matter a
lot here** — RFC 0007's own real measurement corrected an earlier estimate
by roughly 4.5×, in the direction of *smaller*; if the same correction
applies here, the real number could land well under even this RFC's
tighter bound, or (less likely, since within-document term recurrence is
generally denser than whole-document length) it could land closer to the
conservative bound if some fields have unusually bursty term repetition.
Either way, this RFC's central "positions are expensive" conclusion does
not depend on resolving that gap precisely — even the tighter bound alone
consumes the large majority of RFC 0007's remaining budget headroom.

## Alternatives considered

**Growing `TermInfo` with a `total_term_freq` field instead of a blob-
internal header (§4).** Rejected: `spec/term-dictionary.md`'s 28-byte
record is already implemented and golden-file-pinned
(`conformance/term-dictionary/`); adding a field would break byte-
determinism for every existing conformance vector and require a coordinated
regeneration this RFC has no need to force. A blob-internal header costs 4
extra bytes per term (identical to what a `TermInfo` field would have
cost) without touching an already-shipped chapter.

**128-value blocks, matching Lucene and tantivy exactly, instead of
reusing `BitPacker8x`'s 256.** Rejected: would require adding
`BitPacker4x` (a distinct, incompatible packing format,
`references/quickwit-bitpacking-crate.md`) as a second bit-packer
dependency for one blob family, spending novelty budget invariant 8
reserves for real, justified cases. `BitPacker4x` does have a real ARM
NEON path RFC 0007's registered `BitPacker8x` lacks — a real point in its
favor this RFC does not dismiss — but adopting it here, for positions
only, while postings stays on `BitPacker8x`, would mean two different
block sizes and two different bit-packers live side by side in the same
blob family for no reason grounded in this RFC's own measurements. If a
future ARM-validation RFC (RFC 0007's own Open questions) concludes
`BitPacker4x` is the right registered default generally, it should change
postings and positions together, not this RFC alone.

**Adopting tantivy's sequential-accumulation lookup instead of
`postings_block_pos_prefix`.** Rejected in the Motivation section above:
real for tantivy's warm access pattern, a dependent-decode shape invariant
3 rules out for STRAND's cold-open model.

**Storing `postings_block_pos_prefix` as deltas between consecutive
entries instead of raw cumulative values.** Considered and rejected as
premature optimization: raw `u32` cumulative values are directly usable
without a prefix-sum decode step at lookup time, at a cost of up to 4
bytes per postings block instead of a smaller delta-encoded value — the
Napkin math above already shows this region is a small fraction (roughly
1.65–1.90 MB of 19.28–27.39 MB total) of this blob's own footprint, so this
is not where the byte budget is actually being spent.

## Open questions / follow-on RFCs

- A real per-block position-delta bit-width measurement (Napkin math,
  How this could be wrong) — extending `bench/src/msmarco_index.rs` to
  track actual within-document token positions, not just per-document
  counts, the same real-vs-synthetic correction RFC 0007 already made for
  doc-ID gaps and term frequencies.
- Whether the `postings_block_pos_prefix` bridge, once implemented, adds
  meaningful decode-path complexity or bugs beyond what the worked
  example's single-block case exercises (How this could be wrong) —
  answerable only by real implementation and property testing, matching
  RFC 0007's own precedent of implementing before treating a design as
  settled.
- ARM/non-AVX2 validation (Non-goals) — inherited from RFC 0007, not
  independently re-opened here.
- Whether a field-level convention beyond "the blob is present or absent"
  (Non-goals) is ever needed — not motivated by any real requirement this
  session found.
- Multi-field blob addressing (Non-goals) — inherited as open from RFC
  0005/0006/0007, not solved here.
- Proximity/sloppy-phrase scoring (Non-goals) — real, separate future
  work at the query-layer, not the format layer.
