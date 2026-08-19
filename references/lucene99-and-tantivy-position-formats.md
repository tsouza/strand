# Lucene99 `.pos`/`.pay` and tantivy `.pos` — Prior Art for Position Storage

Vendored excerpts (fetched via `WebFetch`, 2026-08-19), grounding RFC 0008
(`rfcs/0008-positions.md`). Both are cited elsewhere in this repository already —
Lucene's postings/scoring format for RFC 0003 and RFC 0007
(`references/lucene-bm25similarity-and-smallfloat.md`), and the `bitpacking` crate
tantivy also depends on for RFC 0007
(`references/quickwit-bitpacking-crate.md`) — so this file adds only the
position-specific layout detail neither prior vendoring covered. Apache-2.0
(Lucene) and MIT (tantivy docs/source), both already-accepted licenses in this
project (`CLAUDE.md` §1).

---

## Lucene99PostingsFormat: `.pos` and `.pay`

Source: `https://lucene.apache.org/core/9_9_1/core/org/apache/lucene/codecs/lucene99/Lucene99PostingsFormat.html`.

### Why positions are split from payloads/offsets into a separate file

> "When encoded as a packed block, position data is separated out as .pos,
> while payloads and offsets are encoded in .pay (payload metadata will also
> be stored directly in .pay)."
>
> "Payloads and offsets are stored together. With this strategy, the majority
> of payload and offset data will be outside .pos file. So for queries that
> require only position data, running on a full index with payloads and
> offsets, this reduces disk pre-fetches."

This is the concrete precedent for RFC 0008 declaring payloads and offsets
explicitly out of scope: even Lucene, which supports both, physically
separates them from bare positions specifically so a phrase query touching
only positions doesn't pay for data it doesn't need.

### `.pos` file grammar (as published)

    PosFile(.pos) --> Header, <TermPositions> TermCount, Footer
    Header --> IndexHeader
    TermPositions --> <PackedPosDeltaBlock> PackedPosBlockNum, VIntBlock?
    VIntBlock --> <PositionDelta[, PayloadLength?], PayloadData?, OffsetDelta?, OffsetLength?>PosVIntCount
    PackedPosDeltaBlock --> PackedInts
    PositionDelta, OffsetDelta, OffsetLength --> VInt
    PayloadData --> byte[PayLength]
    Footer --> CodecFooter

Key field semantics, quoted:

- **PackedPosBlockNum**: "the number of packed blocks for current term's
  positions, payloads or offsets. In particular, PackedPosBlockNum =
  floor(totalTermFreq/PackedBlockSize)."
- **PosVIntCount**: "the number of positions encoded as VInt format. In
  particular, PosVIntCount = totalTermFreq - PackedPosBlockNum*PackedBlockSize."
- **PositionDelta**: "if payloads are disabled for the term's field, the
  difference between the position of the current occurrence in the document
  and the previous occurrence (or zero, if this is the first occurrence in
  this document)."

Two facts pinned directly by this grammar, load-bearing for RFC 0008: (1)
block counts are computed from **`totalTermFreq`** (the term's total
occurrence count across every document it appears in), not from `doc_freq`
— positions blocks and postings blocks are independently sized because they
count different things; (2) `PositionDelta` resets to a value relative to
zero at the start of each document, exactly the same "delta-gap, reset per
run" shape STRAND's own postings blob (`spec/postings.md` §2) already uses
for doc-ID gaps, just reset per-document instead of once per term.

### Block size: 128, fixed, and shared with `.doc`

> "the block size (i.e. number of integers inside block) is fixed
> (currently 128)."
>
> "This value should always be a multiple of 64, currently fixed as 128 as
> a tradeoff."

Lucene uses the same 128-integer block size across `.doc`, `.pos`, and
`.pay`. RFC 0008 registers 256 instead (`BitPacker8x`'s fixed block length,
already registered for postings by RFC 0007) — a deliberate divergence from
this precedent, reusing STRAND's own already-implemented codec rather than
introducing a second block size and a second bit-packer dependency. See RFC
0008's Design section for the tradeoff this reuse makes.

### Skip data: per-block file-pointer skips avoid decoding preceding blocks

> "each skip entry points to the beginning of each block" ... "PosFPSkip and
> PayFPSkip record the file offsets of related block in .pos and .pay,
> respectively." ... "DocSkip records the document number of every
> PackedBlockSizeth document number in the postings (i.e. last document
> number in each packed block)."

Lucene's skip list carries a **file-pointer** into `.pos` at each postings
skip entry, so a reader jumping to a target document via `.doc`'s skip list
gets the matching `.pos` byte offset for free — no separate positions-side
scan is needed. This is a *round-trip-motivated* design (avoiding a second
disk seek pattern); RFC 0008's own bridge between postings blocks and
position-stream offsets (`postings_block_pos_prefix`) solves the same
problem for STRAND's fully-resident-after-open model, where the cost being
avoided is CPU-bound block decode, not a round trip (`CLAUDE.md` invariant 3
already resolves the round-trip question — the whole positions blob is
cold-fetchable).

---

## tantivy: `.pos` file and phrase-query position lookup

Sources: `https://docs.rs/tantivy/latest/tantivy/positions/index.html` (position
file format) and
`https://github.com/quickwit-oss/tantivy/blob/main/ARCHITECTURE.md` (how phrase
queries locate a document's positions).

### Block size and encoding — 128-delta SIMD blocks, VInt tail

> "tantivy relies on simd bitpacking to encode the positions delta in blocks
> of 128 deltas."
>
> "Because we rarely have a multiple of 128, the final block encodes the
> remaining values with variable int encoding."
>
> "The skip widths encoded separately makes it easy and fast to rapidly
> skip over n positions."

Formal grammar as published:

    Positions := NumBitPackedBlocks BitPackedPositionBlock^(P/128) BitPackedPositionsDeltaBitWidth VIntPosDeltas?
    NumBitPackedBlocks := P / 128 encoded as variable byte integer
    BitPackedPositionBlock := bit width encoded block of 128 positions delta
    BitPackedPositionsDeltaBitWidth := (BitWidth: u8)^NumBitPackedBlocks
    VIntPosDeltas := VIntPosDelta^(P % 128)

This is structurally the same shape STRAND's own postings blob already
registered in RFC 0007: full SIMD-packed blocks of a fixed size, a per-block
bit-width byte array stored separately from the packed data (enabling
byte-offset computation without decoding), and a variable-length tail for
the remainder. RFC 0008 inherits this shape directly rather than inventing
a new one, differing from tantivy only in block size (256 vs. 128, per the
Lucene section above) and in reusing STRAND's own already-implemented
scalar tail-packer instead of VInt.

### How phrase queries locate a document's positions — sequential, not skip-indexed

> "The [TermInfo] gives an offset (expressed in position this time) in this
> file."
>
> "As we iterate through the docset, we advance the position reader by the
> number of term frequencies of the current document."

Unlike Lucene's file-pointer skip entries, tantivy's own architecture
description states the reader locates a target document's positions by
**sequentially accumulating** term-frequency counts across every preceding
document in the postings iteration — no per-block skip index into the
position stream. This is tantivy's own documented tradeoff, not a
misreading: it is acceptable for tantivy's warm, memory-mapped access
pattern (advancing through a docset is cheap there) but is exactly the
"dependent pointer chasing" shape invariant 3 rules out for a cold, remote
read. RFC 0008 does **not** adopt tantivy's sequential-accumulation
approach for this reason; it is named here specifically as the design this
project rejects, and why.
