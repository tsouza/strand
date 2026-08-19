# Postings

Normative for STRAND v0.1. Defines the postings blob for the lexical family:
per-term doc-ID delta-gaps and term frequencies, bit-packed in 256-value blocks
with a variable-length final block, plus a per-block block-max region for skip
pruning. Approved by RFC 0007 (`rfcs/0007-postings-codec.md`); this chapter states
the settled result — see the RFC for the worked example, alternatives considered,
and the adversarial review. Registered in `spec/container.md` §9: `family_id = 1`
(lexical), `blob_type_id = 2` (postings).

Reference implementation: `crates/strand-lexical/src/postings.rs`. Golden file:
`conformance/postings/toy-postings.bin`, matching this chapter's RFC's worked
example exactly, byte for byte.

## 1. Scope: one postings blob per field's term dictionary

Each term in a field's term dictionary (`spec/term-dictionary.md`) has exactly one
postings region within this blob, addressed by that term's
`TermInfo.postings_offset`/`postings_length` (`spec/term-dictionary.md` §3).
`TermInfo.doc_freq` is the term's post count and is never repeated inside this
blob's own layout.

## 2. The d-gap variant (invariant 11's complete registration)

Values are local ordinals (`0` to `row_id_count - 1`, `spec/row-ids.md` §1), never
the global row-ID directly. The gap sequence is first-order delta: `gap[0] =
ordinal[0] - 0`; `gap[i] = ordinal[i] - ordinal[i-1]` for `i >= 1`. Little-endian
throughout (invariant 11). Full blocks use the `bitpacking` crate's `BitPacker8x`
kernel (`quickwit-oss`, already registered for R2, `docs/ledger.md`), vertical
(interleaved) SIMD layout, 256 values per block.

## 3. Block structure: fixed 256-value blocks, variable-length final block

A term with `n = doc_freq` postings divides into `block_count = ceil(n / 256)`
blocks. Every block except possibly the last covers exactly 256 postings, packed
with `BitPacker8x`'s SIMD kernel at that block's own bit width.

**The final block, when `n` is not an exact multiple of 256 (or `n < 256`), MUST
cover exactly the real remaining count — a writer MUST NOT pad it to 256** — and
MUST be packed with a plain scalar bit-packer: LSB-first, shift/mask, no SIMD, no
forced block width. Padding this block wastes both space and decode time in
proportion to how short the real remainder is, measured directly
(`rfcs/0007-postings-codec.md` Motivation) rather than assumed.

`block_count` is never stored in this blob — a reader already has `doc_freq` from
`TermInfo` before fetching this blob, so `block_count = ceil(doc_freq / 256)` is
computed, not read.

## 4. Layout

| region                | size                                 | notes                                                                 |
| --------------------- | ------------------------------------ | --------------------------------------------------------------------- |
| `block_max`           | `4 * block_count` bytes              | one `u32` per block, §5                                               |
| `gap_widths`          | `block_count` bytes                  | one `u8` per block: bits needed for that block's gaps                 |
| `tf_widths`           | `block_count` bytes                  | one `u8` per block: bits needed for that block's term frequencies     |
| gap stream            | sum of each block's packed gap bytes | full blocks via `BitPacker8x`; final block via the scalar packer, §3  |
| term-frequency stream | sum of each block's packed tf bytes  | same block boundaries as the gap stream, independently width-selected |

A block's packed byte length is `ceil(block_real_len * width / 8)`, where
`block_real_len` is `256` for every block except a shorter final block. Gap
magnitude and term-frequency magnitude are unrelated, so each stream is
independently width-selected per block — forcing a shared width would waste bits
on whichever stream has the smaller range in a given block.

`storage-class: raw-mappable`, `tier: cold-fetchable` (`spec/container.md` §5) —
the blob's own content is already dense/bit-packed; `storage-class` governs
whether a further chunk codec (zstd) wraps it, not whether the content itself is
compact (invariant 10).

## 5. Block-max: per-block maximum doc-ordinal

`block_max[i]` is the real (post-delta) local ordinal of the last posting the
`i`-th block covers. Since blocks are formed from consecutive slices of a sorted
list, `block_max` is monotonically increasing and binary-searchable. This is the
raw-statistics sibling region invariant 4 commits STRAND to for postings: a reader
locates the one block that can contain a target ordinal by binary-searching
`block_max` — untouched compressed bytes — then decodes only that block, instead
of decoding the whole list to find it.

`block_max` is a region within this blob (§4's layout), not a separately
registered blob — see `rfcs/0007-postings-codec.md`'s Alternatives considered for
why, and its own flagged tension with invariant 4's "sibling blob" wording.

## 6. Query resolution

**Full decode**: read `block_max`/`gap_widths`/`tf_widths` once (already resident,
part of the cold-fetchable wave), then decode every block in sequence,
reconstructing real ordinals via a running prefix sum and real term frequencies
directly.

**Skip query** (given a target ordinal): binary-search `block_max` to find the one
candidate block; decode only that block. The starting index into the overall
posting sequence is recoverable without an additional stored prefix-count array,
since every block except possibly the last covers exactly 256 postings.

## 7. Merge semantics (invariant 1)

This blob family's merge strategy is **rebuild**: at compaction, a term's
postings across merging segments are fully decoded, their real ordinals rebased
into the merged segment's local-ordinal space and combined in sorted order, and
re-encoded from scratch per §2–§5. Local-ordinal delta-gap encoding (§2) makes a
cheaper concatenate+remap strategy genuinely non-trivial — gaps are relative
differences between decoded values, not absolute values a uniform shift applies
to — so this chapter declares the conservative, fully-correct strategy rather than
an unproven optimization. See `rfcs/0007-postings-codec.md` §8 for the full
reasoning and the narrower optimization it names as real but undesigned.

## 8. Placement constraint

Identical in spirit to `spec/scoring-profiles.md` §4, `spec/analyzer-descriptors.md`
§6, `spec/term-dictionary.md` §5, and `spec/filter-bitmaps.md` §5: this blob is
part of the cold-fetchable wave invariant 3 already budgets for after the segment
open, adding bytes to that wave's payload but no additional round trip.

## 9. Conformance status

Implemented (`crates/strand-lexical`). `rfcs/0007-postings-codec.md`'s worked
example (a 3-posting, single-block term) is the first `conformance/postings/`
golden vector — real executed bytes, a real round trip, `doc_freq` supplied
externally exactly as a real reader would have it from `TermInfo`, confirmed
byte-exact against `crates/strand-lexical/tests/postings_worked_example.rs`.
Multi-block lists (spanning `BitPacker8x`'s SIMD-packed full blocks and the
variable-length scalar-packed final block together) are property-tested in
`crates/strand-lexical/tests/postings_round_trip.rs`, including the skip query
checked against a plain linear-scan reference implementation.

## 10. Open dependencies

Positions (phrase-query support), term-frequency/document-length block-max bounds
for BM25-scoring pruning, ARM/non-AVX2 validation, and a validated batch-size
range are all explicitly out of this chapter's scope
(`rfcs/0007-postings-codec.md` Non-goals) — real, separate future work, not
silently assumed resolved.
