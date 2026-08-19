# Positions

Normative for STRAND v0.1. Defines the positions blob for the lexical family:
per-term, within-document token-position delta-gaps, enabling phrase queries.
Approved by RFC 0008 (`rfcs/0008-positions.md`) and amended by RFC 0009
(`rfcs/0009-per-term-overhead-reduction.md` Design §1: `postings_block_pos_prefix`
omits its always-`0` index-`0` entry — a breaking, in-place change to this
blob's layout, superseding RFC 0008's original one rather than coexisting with
it); this chapter states the current, settled result — see the RFCs for the
worked examples, alternatives considered, and the adversarial reviews.
Registered in `spec/container.md` §9: `family_id = 1` (lexical),
`blob_type_id = 3` (positions).

Reference implementation: `crates/strand-lexical/src/positions.rs`, reusing
`postings.rs`'s `scalar_pack`/`scalar_unpack`/`block_count_for`/
`block_real_len` directly (RFC 0008's own stated implementation plan).
Golden file: `conformance/positions/toy-positions.bin` (8 bytes, RFC 0009's
worked example), matching this chapter's current layout exactly, byte for
byte.

## 1. Scope: one positions blob per field, addressed via already-reserved `TermInfo` fields

Each term in a field's term dictionary (`spec/term-dictionary.md`) has at most
one positions region, addressed by that term's `TermInfo.positions_offset`/
`positions_length` (`spec/term-dictionary.md` §3 — reserved since RFC 0005,
unused until this chapter). `positions_length == 0` means this field carries no
stored positions for this term; a real, always-nonzero blob (minimum 5 bytes,
§4 below) is written for every term with at least one occurrence in a field
that does support positions, so zero is unambiguous. A writer either stores
real positions for every term in a field or omits them for every term in that
field — there is no per-term opt-out within a field that does support
positions, and no separate schema flag: the blob's presence or absence is the
signal.

## 2. The d-gap variant (invariant 11's complete registration)

Values are within-document token positions: `0`-based indices into a
document's token stream after the field's declared analyzer chain
(`spec/analyzer-descriptors.md`). The delta sequence resets at every document
boundary: for a document contributing `tf` positions to this term, `delta[0] =
position[0] - 0`, `delta[i] = position[i] - position[i-1]` for `i >= 1` —
identical in shape to `spec/postings.md` §2's delta-from-zero convention,
reset once per document instead of once per whole term. Deltas from different
documents are concatenated directly in postings order, with no separator: a
decoder that already knows each document's `tf` (from the postings blob,
already resident) knows exactly how many consecutive deltas belong to each
document without an explicit boundary marker. Little-endian throughout,
`BitPacker8x`'s vertical SIMD layout for full blocks (`spec/postings.md` §2's
identical registration).

## 3. Block structure: fixed 256-value blocks, variable-length final block

Identical mechanics to `spec/postings.md` §3, applied to the position-delta
stream instead of the doc-ID-gap stream. `total_term_freq` (§4) determines the
count this stream covers: `position_block_count = ceil(total_term_freq /
256)`. Every block except possibly the last covers exactly 256 deltas, packed
with `BitPacker8x`'s SIMD kernel at that block's own bit width; the final
block, when `total_term_freq` is not an exact multiple of 256, covers exactly
the real remainder and is packed with the same scalar bit-packer
`spec/postings.md` §3 registers (LSB-first, shift/mask, no SIMD).

## 4. Layout

| region                      | size                                   | notes                                                                                   |
| --------------------------- | -------------------------------------- | --------------------------------------------------------------------------------------- |
| `total_term_freq`           | 4 bytes                                | little-endian `u32`; sum of this term's per-document term frequencies                   |
| `postings_block_pos_prefix` | `4 * (postings_block_count - 1)` bytes | one `u32` per postings block *except block 0* (always `0`, never stored — RFC 0009), §5 |
| `pos_widths`                | `position_block_count` bytes           | one `u8` per position block: bits needed for that block's position deltas               |
| position-delta stream       | sum of each block's packed bytes       | full blocks via `BitPacker8x`; final block via the scalar packer, §3                    |

`total_term_freq` is not stored in `TermInfo` (`spec/term-dictionary.md` §3):
unlike `doc_freq`, it is not recoverable without decoding the postings blob's
entire term-frequency stream, so storing it here — as the first 4 bytes of
this blob, rather than growing `TermInfo`'s already-implemented, already-
golden-filed 28-byte record — avoids breaking byte-determinism for every
existing term-dictionary conformance vector. `total_term_freq` and
`postings_block_pos_prefix` entries are both `u32`, inheriting the same
realistic-range assumption `rfcs/0007-postings-codec.md` Design §6 named for
`block_max` (`u32` against an unbounded `row_id_count: u64`): a term occurring
more than 2^32 times in one segment would overflow these fields, a real,
named, inherited gap.

The minimum positions blob is 5 bytes: `total_term_freq`(4) + zero
`postings_block_pos_prefix` entries (`postings_block_count = 1`, so
`postings_block_count - 1 = 0`) + one `pos_widths` entry(1) + a
possibly-empty stream, for the smallest case of `doc_freq = total_term_freq =
1`.

## 5. `postings_block_pos_prefix`: the bridge from postings-block space to position-stream space

`postings_block_pos_prefix[i]` is the total count of positions (summed `tf`
across every document) that precede postings block `i`'s first document.
`postings_block_pos_prefix[0]` is always `0` — nothing precedes the first
postings block, by construction, for every term — and per RFC 0009 Design §1
is never stored: the region holds only entries for blocks `1..postings_block_count`,
so a reader's `postings_block_pos_prefix(i)` accessor returns `0` immediately
for `i == 0` and otherwise reads the little-endian `u32` at byte offset
`4 * (i - 1)` within this (shorter) region. This is the region that lets a
reader who has located a target document's postings block `lo` (via
`spec/postings.md` §6's skip query, which already decodes block `lo` to
verify the match) find that document's position-stream start index in O(1)
plus a sum over only the documents in block `lo` that precede the target —
never requiring a decode of any postings block before `lo`, and never
requiring a decode of any position block before the target's own.

## 6. Query resolution

**Targeted lookup** (phrase query resolving one candidate document, the
common case): given a target document already located via `spec/postings.md`
§6's skip query — which yields postings block index `lo`, that document's
within-block position, and its `tf` — a reader: reads `total_term_freq` and
`postings_block_pos_prefix[lo]` (both already-resident, O(1) reads); sums the
`tf` of every document in block `lo` strictly before the target (already
decoded during the postings skip, no new decode); adds that sum to
`postings_block_pos_prefix[lo]` to get `start_index`, the target document's
first position's index in this term's overall position stream; locates
`start_block = start_index / 256` and, since a document's run may straddle a
block boundary, `end_block = (start_index + tf - 1) / 256`; computes
`start_block`'s byte offset by summing `pos_widths[0..start_block]`'s packed
lengths (the identical `stream_offset` pattern `spec/postings.md`'s reference
implementation uses); decodes blocks `start_block..=end_block` and slices to
the `tf` deltas beginning at `start_index`; reconstructs absolute
in-document positions via a running sum that starts at `0` exactly at that
slice's first delta (§2's reset-per-document convention means no additional
boundary detection is needed).

**Full decode** (every position for every document a term occurs in): walk
the position-delta stream sequentially, block by block, while walking the
postings blob's already-decoded term-frequency array in parallel, resetting
the running sum to `0` each time the parallel walk crosses a document
boundary.

This blob carries no independent pruning bound (invariant 4): a phrase query
never skips within the position stream on its own — it always arrives at a
specific target document via postings' own `block_max` (`spec/postings.md`
§5) first. `postings_block_pos_prefix` is an offset bridge, not a second
pruning structure.

## 7. Merge semantics (invariant 1)

**Rebuild**, inherited directly from `spec/postings.md` §7: a segment merge
that combines multiple segments' postings for a term already re-decodes and
re-encodes that term's entire postings list from scratch, and this blob's
`postings_block_pos_prefix` region is defined entirely in terms of that
rebuilt postings blob's own block boundaries. Positions rebuild whenever
postings do, as a structural consequence, not a separately chosen policy.

## 8. Placement constraint

Part of the cold-fetchable wave invariant 3 already budgets for after the
segment open, adding bytes but no additional round trip
(`spec/postings.md` §8's identical framing). Unlike most cold-fetchable
additions, this one's byte contribution is large enough to matter on its own
(RFC 0008's own napkin math measured it at roughly comparable size to
postings-plus-term-info combined, on a real MS MARCO sample) — a field that
does not need phrase-query support SHOULD omit its positions blob entirely
(§1) rather than pay that cost on every cold open.

## 9. Conformance status

Implemented (`crates/strand-lexical`), current layout per RFC 0009's
amendment. RFC 0009's own worked example (the same 3-posting term RFC 0007/
0008 used, extended with within-document positions) is the current
`conformance/positions/toy-positions.bin` golden vector (8 bytes, superseding
RFC 0008's original 12-byte one) — real executed bytes, confirmed byte-exact
against `crates/strand-lexical/tests/positions_worked_example.rs`, including
targeted-lookup resolution for all three documents. Multi-block lists
(postings-block and position-block boundaries stressed independently, since
they're different counts derived from `doc_freq` and `total_term_freq`
respectively) are property-tested in
`crates/strand-lexical/tests/positions_round_trip.rs`, including targeted
lookups checked directly against the original input (not against this
blob's own full-decode path, for a genuinely independent check) — this
property coverage confirms both RFC 0008's original design and RFC 0009's
index-`0` omission together, since the same tests exercise both.
Real-corpus validation: re-running `bench/src/field_end_to_end.rs` against
the same MS MARCO samples used to motivate RFC 0009 confirmed its predicted
byte counts exactly (`bench/results/field-end-to-end-10003.json`,
`-100476.json`).

## 10. Open dependencies

A real per-block position-delta bit-width measurement (RFC 0008's own Napkin
math used a stated bound, not a measurement); ARM/non-AVX2 validation
(inherited from RFC 0007, unresolved); proximity/sloppy-phrase scoring
(query-layer concern, out of the format's scope per `CLAUDE.md` §1) — all real,
separate future work, not silently assumed resolved.
