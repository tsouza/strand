# RFC 0007: Postings codec

- **Status:** Approved. Adversarial review found 3 Critical, 2 Important, and 3
  Minor findings, all fixed. Critical: (1) two Motivation-section citations
  attributed real findings to `docs/ledger.md`'s R9 entry when they actually live
  in R2's own bullet (the exact broken-citation pattern already recurring across
  RFCs 0004–0006) — fixed by re-checking the ledger's actual bullet boundaries and
  correcting both. (2) The "How this could be wrong" section's own risk mitigation
  — that the registered `bitpacking` crate's NEON path covers ARM — was checked
  against the crate's actual current source (fetched and read directly, not
  trusted from the vendored excerpt) and found false: `BitPacker8x`'s
  `InstructionSet` enum has no NEON variant at all; NEON exists only in the
  different, incompatible `BitPacker4x` format. Fixed everywhere the claim
  appeared (Non-goals, How this could be wrong, Alternatives considered, Open
  questions) — the ARM gap is now stated as genuinely unmitigated, not
  softened. (3) A "14.2–19.17B values/sec" range spliced together a synthetic
  average and a real-MS-MARCO maximum from two different measurements never
  combined in the ledger's own prose — fixed by citing both figures separately
  and precisely. Important: invariant 1 was claimed in the header but never
  discussed — fixed by adding a real Design §8 declaring postings' merge strategy
  as `rebuild` (delta-gap encoding relative to local ordinals makes a naive
  concatenate+remap non-trivial, named honestly rather than asserted safe), now
  also in `spec/postings.md` §7. The Napkin-math section lacked the absolute
  byte-budget arithmetic `CLAUDE.md` §7 requires — fixed by adding it: postings +
  term-info for this session's real ~520K-passage sample total ~73.2 MB, 73% of
  the 100 MB budget, on under 6% of the full corpus. Minor: an overstated
  "identical convention" claim between the two decode paths, a `block_max: u32`
  vs. unbounded `row_id_count: u64` overflow risk (named, matching RFC 0006's
  precedent), and a garbled worked-example bit-packing sentence, all fixed. A
  self-introduced citation error was also caught and fixed while applying the
  invariant-1 fix (§2/§3 should have been §3/§4 given this RFC's actual
  subsection numbering) — the same citation-drift class the review's own findings
  warned about, recurring even while fixing them, caught by re-verifying rather
  than trusting the fix.
- **Milestone:** M1 — Lexical (`docs/milestones.md`)
- **Spec chapters produced:** `spec/postings.md`; additively extends
  `spec/container.md` §9 (registers `family_id = 1` "lexical", `blob_type_id = 2`
  "postings")
- **Invariants exercised:** 1, 4, 8, 9, 10, 11 (`CLAUDE.md` §5)

## Summary

Registers STRAND's postings blob: per-term doc-ID delta-gaps and term frequencies,
bit-packed SIMD-BP128-style in 256-value blocks (`bitpacking`'s `BitPacker8x`,
vertical layout, already grounded for R2 in `docs/ledger.md`), with one required
deviation from naive fixed-block encoding — **the final block of a list is
variable-length, packed at its own real size, never padded to 256** — closing a
real, measured problem this session's R9 investigation found: naive padding wastes
enormous space and CPU on short lists, which dominate real corpora by count
(Zipf's law; `docs/research/r2-hybrid-codec-methodology.md` Phase 1/2B). A
per-block maximum-doc-ordinal region accompanies the packed data for skip pruning
(invariant 4), real-measured this session to cut skip cost roughly 7× on lists long
enough to span more than one block. `BitPacker8x` is confirmed as the default
codec family over FastPFOR and FastLanes with real decode-throughput and
compression measurements on both synthetic and real MS MARCO data — not asserted
from the literature alone.

This RFC does **not** register positions (phrase-query support), term-frequency or
document-length block-max bounds for BM25-scoring pruning, or a validated
batch-size range — each is real, separate, un-grounded-by-this-session work, named
precisely in Non-goals and Open questions rather than glossed over.

## Motivation

Three separate strands of real measurement from this session converge on this
RFC, none of them literature claims taken on faith:

**The codec-family choice.** `docs/ledger.md`'s R9 entry records real,
executed measurements (`bench/src/codec_decode_throughput.rs`,
`bench/results/codec-decode-throughput.json`), on two distinct datasets that this
RFC does not blend into one range: on synthetic uniform-random data (10 bit
widths, averaged), `BitPacker8x` (256-value blocks, AVX2) decodes at ~14.2B
values/sec, `FastLanes` (1024-value blocks, portable) at 74–77% of that on this
hardware — never faster at any individual width measured — and `FastPFOR`
(adaptive, exception-based) 5.9× slower. On real MS MARCO delta-gaps and term
frequencies specifically, `BitPacker8x` decodes at ~15.6B and ~19.17B values/sec
respectively (`bench/results/codec-decode-throughput.json`'s
`real_msmarco_d_gaps`/`real_msmarco_term_frequencies` results), with `FastPFOR`
7.0–8.3× slower on the same real data. FastPFOR's real compression advantage,
measured against that same real data (not a synthetic worst case), is 4.26× and
5.2× respectively — real, but far narrower than a synthetic 95/5 skew's 17×
estimate, and not judged worth its decode-speed cost. `BitPacker8x` is the
default this RFC registers for exactly this reason: fastest measured decode on
both synthetic and real data, and no measured compression case strong enough to
justify the alternative's cost.

**The padding problem.** While investigating a separate question — whether STRAND
should adopt a compressed-domain-searchable codec (Elias-Fano) alongside or
instead of BP128 — this session's Phase 1/2B pilot
(`bench/src/hybrid_codec_pilot.rs`) found that naive fixed-256-block encoding
inflates both size and decode cost enormously on short lists: a real MS MARCO
sample of 4,016 postings lists is 69% lists of length ≤8 (`docs/ledger.md` R2
entry), and a `BitPacker8x` block padded from (say) 3 real values to 256 costs as
much to store and decode as if it held 256. That pilot's `bp128_variable_bench` —
a minimal scalar bit-packer for the trailing partial block, no padding, no forced
256-wide decode — is the direct ancestor of this RFC's variable-length-final-block
requirement, and its correctness and the magnitude of the fix are both
real-measured, not asserted: mean decode cost on short lists dropped from
~117–137ns (padded) to ~89–100ns (variable) — a real range across reruns, recorded
precisely in `docs/ledger.md`'s R2 entry, not just this session's own scrollback —
and, more consequentially, reported *size* dropped from a padding-inflated figure
that had made `BitPacker8x` look larger than Elias-Fano on this corpus
(~672–673 bytes/list mean, also `docs/ledger.md` R2) to its true, fairly-measured
figure (~148–150 bytes/list mean), a result stable across five reruns once two real
measurement bugs were found and fixed in the same investigation (`docs/ledger.md`
R2 entry has the full account — the hybrid-codec investigation's findings live
inside R2's own bullet, not R9's, since it was raised alongside R2's own
grounding). That investigation's own conclusion — no adaptive
BP128/EF hybrid is justified, `BitPacker8x` wins on size and mostly on decode once
fairly measured — is exactly why this RFC registers a single default codec, not a
per-list choice; but the padding bug it caught along the way is this RFC's direct
motivation and is fixed here regardless of that conclusion.

**Block-max.** `docs/ledger.md`'s R2 entry also records a real block-max
implementation and measurement, from the same investigation: a per-block maximum
real doc-ordinal, binary-searchable since blocks are formed from a sorted list, cuts
skip cost from ~2,551–2,676ns (decode-then-scan) to ~276–352ns on the 177-of-4,016
lists spanning more than one block — a genuine ~7× improvement, though it still
trails Elias-Fano's native skip by roughly 7–8× on those same lists. Invariant 4
already commits STRAND to raw-statistics block-max bounds as a sibling structure;
this RFC is where that commitment becomes concrete for postings, at least for the
one bound this session actually measured (per-block maximum doc-ordinal).

## Non-goals

**Positions** (phrase-query support) are not registered here. `spec/term-dictionary.md`
§3's `TermInfo` record already reserves `positions_offset`/`positions_length` fields
for a separate positions blob; this RFC does not define that blob's layout, and no
session measurement touched position lists at all.

**Term-frequency and document-length block-max bounds for BM25-scoring pruning**
(WAND/BlockMax-WAND-style, `references/ding-suel-block-max-shallow-pointers.md`) are
not registered here. This RFC's block-max region carries exactly one bound — per-block
maximum doc-ordinal, for doc-ID skip pruning — because that is the only bound this
session measured. Scoring-aware pruning bounds are real, separate, future work tying
into RFC 0003's scoring-profile inputs, not a variant of this RFC.

**ARM and non-AVX2 hardware validation** remain open. Every throughput number this
RFC cites was measured on one x86 AVX2 machine (`docs/ledger.md` R9's own stated
caveat, repeated here because it bears directly on this RFC's central claim — see
How this could be wrong, below). The `bitpacking` crate's `BitPacker4x` variant
has a real aarch64 NEON path (`references/quickwit-bitpacking-crate.md`) — but
`BitPacker8x`, the format this RFC actually registers, has no ARM SIMD path at
all in the crate's current source (confirmed by fetching and reading
`bitpacker8x.rs` directly during this RFC's review), only a scalar fallback. ARM
performance for this specific codec is genuinely unmeasured and unmitigated by
anything already shipped, not just unmeasured with a ready fallback waiting.

**A validated batch-size range** (invariant 9) is not settled here. `CLAUDE.md`
invariant 9 cites turbopuffer's validated 512-value reference point; this RFC's own
block size (256, `BitPacker8x`-native) is a distinct concern from the reader API's
iteration batch size, and no session measurement connects the two.

**Registering FastPFOR or FastLanes as available alternative codecs** (invariant 8
permits a registry of named codecs; this RFC's own measurement shows real,
corpus-dependent cases where FastPFOR's compression edge could matter for a
specific field) is not done here — this RFC registers one default, `BitPacker8x`,
and leaves a second registered alternative to a future RFC if a real use case
justifies the added registry and conformance surface.

## Design

### 1. Blob registration

`family_id = 1` (lexical, already registered by RFC 0005), `blob_type_id = 2`
("postings"), `storage-class: raw-mappable`, `tier: cold-fetchable` — the same
classification RFC 0005's term-info store and RFC 0006's filter-bitmap store both
use: the blob's own content is already dense/bit-packed, and `storage-class` is
about whether a *further* chunk codec (zstd) wraps it, not whether the content
itself is compact (invariant 10).

### 2. Scope: one postings blob per field's term dictionary

Each term in a field's term dictionary (RFC 0005) has exactly one postings blob,
addressed by that term's `TermInfo.postings_offset`/`postings_length`
(`spec/term-dictionary.md` §3) within this shared blob. `TermInfo.doc_freq` is the
term's post count — already known to a reader before this blob is fetched, so
nothing in this blob's own layout repeats it (see §4).

### 3. The d-gap variant (invariant 11's required complete registration)

Values are **local ordinals** (`0` to `row_id_count - 1`, `spec/row-ids.md` §1),
identical in scope to RFC 0006's local-ordinal convention, never the 64-bit global
row-ID directly. The gap sequence is first-order delta: `gap[0] = ordinal[0] - 0`,
`gap[i] = ordinal[i] - ordinal[i-1]` for `i >= 1`. This is the plain, standard
scheme — no skip-list interleaving, no frame-of-reference offset beyond the
implicit zero start. A bare "BP128" or "delta encoding" is not a registration per
invariant 11; this is the complete one: little-endian throughout, first-order
delta from zero, `BitPacker8x`'s vertical (interleaved) SIMD layout for full
blocks (`references/lemire-boytsov-simd-bp128.md`, already the registered choice
for R2).

### 4. Block structure: fixed 256-value blocks, variable-length final block

A list of `n = doc_freq` postings is divided into `block_count = ceil(n / 256)`
blocks. Every block except possibly the last covers exactly 256 postings and is
packed with `BitPacker8x`'s SIMD kernel at that block's own bit width. **The final
block, when `n` is not an exact multiple of 256 (or `n < 256`), covers exactly the
real remaining count — never padded — and is packed with a plain scalar bit-packer
(shift/mask, no SIMD, no forced block width)**, real-tested this session
(`bp128_variable_bench`, `bench/src/hybrid_codec_pilot.rs`). `block_count` itself
is never stored in this blob — a reader already has `doc_freq` from `TermInfo`
before fetching this blob, so `block_count = ceil(doc_freq / 256)` is computed, not
read.

### 5. Two parallel streams: doc-ID gaps and term frequencies

Each block's doc-ID gaps and term frequencies are independently width-selected —
gap magnitude and term-frequency magnitude are unrelated, so forcing one shared
width would waste bits on whichever stream has the smaller range in a given block.
Layout, in order:

| region           | size                                 | notes                                                             |
| ---------------- | ------------------------------------ | ----------------------------------------------------------------- |
| `block_max`      | `4 * block_count` bytes              | one `u32` per block, §6                                           |
| `gap_widths`     | `block_count` bytes                  | one `u8` per block, bits needed for that block's gaps             |
| `tf_widths`      | `block_count` bytes                  | one `u8` per block, bits needed for that block's term frequencies |
| gap stream       | sum of each block's packed gap bytes | full blocks via `BitPacker8x`; final block via the scalar packer  |
| term-freq stream | sum of each block's packed tf bytes  | same block boundaries as the gap stream, own widths               |

All multi-byte fields little-endian (invariant 11). A block's packed byte length is
`ceil(block_real_len * width / 8)` where `block_real_len` is 256 for every block
except a shorter final block.

### 6. Block-max: per-block maximum doc-ordinal, for skip pruning

`block_max[i]` is the real (post-delta) local ordinal of the last posting the
`i`-th block covers — monotonically increasing across blocks, since blocks are
formed from consecutive slices of a sorted list, so the whole array is
binary-searchable. This is the raw-statistics sibling region invariant 4 commits
STRAND to: a reader locates the one block that can contain a target ordinal by
binary-searching `block_max` — untouched compressed bytes — then decodes only that
block. `block_max` lives beside the codec's packed bytes, never inside them
(invariant 4's own wording), but as a **region within this same blob**, not a
separately registered blob — see Alternatives considered for why.

`block_max` entries are `u32`, matching local ordinals' own realistic range
(`spec/filter-bitmaps.md` §3 already registers the identical `row_id_count <=
2^32` cap, for the same reason: standard 32-bit Roaring indexing). `spec/container.md` §4 itself leaves
`row_id_count` an unbounded `u64` with no cap of its own — a writer of a segment
whose `row_id_count` exceeded `2^32` would silently overflow `block_max` here
exactly as RFC 0006 already named for its own local-ordinal use. This RFC does
not repeat RFC 0006's normative cap as a new rule; a future spec chapter should
state the `row_id_count <= 2^32` bound once, generally, rather than have every
blob family that indexes local ordinals as `u32` restate it individually — named
here as a real gap, not silently assumed safe.

### 7. Query resolution

Given a query term and its already-resident postings blob (located via `TermInfo`,
`spec/term-dictionary.md` §4): full decode reads `block_max`/`gap_widths`/
`tf_widths` once, then decodes every block in sequence, reconstructing real
ordinals via a running prefix sum and real term frequencies directly. A skip query
for a target ordinal binary-searches `block_max` to find the one candidate block,
decodes only that block (and, for the ordinal reconstruction, needs the prefix sum
up to that block's start — computable from `block_max[i-1]` plus knowledge that no
block skip changes the *count* of prior postings, only their values, so a reader
tracking cumulative counts alongside `block_max` recovers the starting index in
O(1); this RFC does not additionally store a separate prefix-count array,
deferring that as a possible future optimization, not a correctness requirement,
since the count is recoverable by summing `block_real_len` for prior blocks, all of
which are 256 except possibly one).

### 8. Merge semantics (invariant 1)

Invariant 1 requires every blob family to declare its merge strategy, and RFC
0005/0006 both deliberately excluded invariant 1 from their own scope — a
term-info store or a filter-bitmap store doesn't obviously need one until a real
compaction touches it. Postings do need one declared now, because this blob is
exactly what invariant 1 is worried about, and this RFC's own §3/§4
local-ordinal delta-gap encoding makes the natural-looking answer — cheap
concatenate+remap, `spec/row-ids.md`'s own canonical example for posting lists —
genuinely non-trivial: a segment merge that rebases local ordinals cannot simply
rewrite a constant offset into an opaque byte stream, because gaps are relative
differences between consecutive *decoded* values, not absolute values a fixed
shift applies to uniformly at every position — only the first gap of an appended
segment's stream would need adjusting to account for the new predecessor value,
but that single touched value can force a wider bit width for the block it falls
in, which can cascade into repacking that whole block (never more than one block
per merged term, but not zero-cost either).

This RFC declares postings' merge strategy as **rebuild**: at compaction, a
term's postings across merging segments are fully decoded, their real ordinals
rebased and merged in sorted order, and re-encoded from scratch per §3–§6 (the
d-gap variant, block structure, the two parallel streams, and a recomputed
`block_max`). This
is the conservative, safe answer — full invariant-1 compliance, real cost stated
plainly rather than hidden — not a claim that concatenate+remap is impossible.
The narrower optimization sketched above (touch only the boundary gap and,
rarely, its block) is real and plausible, but designing and validating it is M3
compaction work (`docs/milestones.md`), not something this M1 RFC decides by
assertion. Naming "rebuild" now, honestly costed, is what invariant 1 requires of
an M1 RFC that registers a real postings blob; picking a cheaper strategy without
proving it correct would not be.

## Worked example

A term with 3 postings — local ordinals `5, 12, 47` — and term frequencies
`2, 1, 3`. Well under 256, so `block_count = 1`, exercising exactly the
variable-length final block this RFC exists to require. Computed with real
executed Python bit arithmetic mirroring the scalar packer already validated in
`bench/src/hybrid_codec_pilot.rs` (`scalar_pack`/`scalar_unpack`), not hand-derived.

Gaps: `5 - 0 = 5`, `12 - 5 = 7`, `47 - 12 = 35`. Maximum gap `35` needs 6 bits
(`0b100011`). Term frequencies `2, 1, 3`; maximum `3` needs 2 bits (`0b11`).

**`block_max`** (4 bytes): the list's one block covers up to ordinal `47`:
`2F 00 00 00`.

**`gap_widths`** (1 byte): `06`. **`tf_widths`** (1 byte): `02`.

**Gap stream** (3 bytes, LSB-first bit-packing at 6 bits/value): packing
`5, 7, 35` at width 6 gives `C5 31 02` — verified by executing the packer and
independently by hand. Each value contributes its 6 bits starting at the next
free bit position, LSB of the value at the lower bit position: gap `5`
(`0b000101`) occupies bits `0–5`; gap `7` (`0b000111`) occupies bits `6–11`,
straddling the byte-0/byte-1 boundary; gap `35` (`0b100011`) occupies bits
`12–17`, straddling byte 1/byte 2. Byte 0 (bits `0–7`) holds all 6 bits of `5`
plus the low 2 bits of `7`: `(5) | (7 << 6)` truncated to 8 bits = `0xC5`. Byte 1
(bits `8–15`) holds the remaining 4 bits of `7` plus the low 4 bits of `35`:
`0x31`. Byte 2 (bits `16–17`) holds `35`'s remaining 2 bits: `0x02`. The full
3-byte sequence round-trips to `5, 7, 35` exactly via `scalar_unpack`.

**Term-frequency stream** (1 byte, 2 bits/value): packing `2, 1, 3` at width 2
gives `36`.

**Full blob, 10 bytes**: `2F 00 00 00 06 02 C5 31 02 36`.

Resolving this blob given external `doc_freq = 3` (from `TermInfo`, no round trip):
`block_count = ceil(3/256) = 1`; read `block_max[0] = 47`, `gap_widths[0] = 6`,
`tf_widths[0] = 2`; gap stream is 3 bytes (`ceil(3*6/8) = 3`) at offset `6`; term-
frequency stream is 1 byte (`ceil(3*2/8) = 1`) at offset `9`. Decoding both streams
and reconstructing the running prefix sum recovers `(5, tf=2), (12, tf=1), (47,
tf=3)` exactly — confirmed by executing the round trip, not asserted.

## Napkin math (`CLAUDE.md` §7)

Postings blobs are the cold-fetchable-wave payload RFC 0005/0006 already place
their own blobs in — this RFC adds no new round trip, only bytes to that wave.
What it changes is how many bytes: the variable-length final block directly
reduces the byte count contributed by every list shorter than a block boundary
(the majority, by count, of any real vocabulary under Zipf's law). This session's
own measurement gives a real number for the specific corpus sampled: mean
postings-blob size per list dropped from a padding-inflated ~672–673 bytes to a
fairly-measured ~148–150 bytes, a real ~4.5× reduction on this corpus — not a
universal constant (a corpus with longer average lists would see a smaller
relative padding tax), but real and directionally significant.

**The absolute arithmetic `CLAUDE.md` §7 requires** (an RFC without it is
incomplete): the real, measured vocabulary on this session's ~520,108-passage MS
MARCO sample (5.88% of the full 8,841,823-passage corpus) is 413,364 distinct
terms. At the fairly-measured ~149 bytes/list mean, one field's postings blobs
alone total `413,364 * 149 ≈ 61.6 MB`. Adding RFC 0005's term-info store
(28 bytes/term, `spec/term-dictionary.md` §3): `413,364 * 28 ≈ 11.6 MB`. Combined,
**~73.2 MB — 73% of the 100 MB cold-open byte budget — for one field, on a
sample covering under 6% of the corpus this project's own bake-off target names**.
This is not a comfortable margin, and this RFC does not pretend otherwise: it
directly confirms `CLAUDE.md` §7's own segment-count-amplification framing —
real segments covering this corpus at any useful scale must be sized well below
"the whole corpus in one segment," which the format already expects and reports
honestly (§7's own "segment count is reported, never hidden" rule) rather than
this RFC discovering it as a surprise. A precise full-corpus extrapolation is not
attempted here — vocabulary growth is sub-linear in corpus size (Heaps' law), so
a naive ~17× linear scaling of the 413,364-term figure would overstate the true
number — but the 73% figure on a 5.88% sample is real, measured, and exactly the
kind of number this project's R1 sizing-law work (`docs/ledger.md`) should use
when it turns to lexical segment sizing, not just the vector blob's own
already-established ~1M-vectors-per-segment rule.

## Invariant-11 checklist

- **Endianness:** little-endian throughout — `block_max` (`u32`), and both the
  full-block (`BitPacker8x`'s own vertical/interleaved SIMD layout) and
  final-block (the scalar packer's plain sequential LSB-first layout) bit-packed
  streams. These are two genuinely different packing conventions, not one
  "identical" convention applied twice — see How this could be wrong for the
  real, stated cost of maintaining two decode paths — but both are LSB-first at
  the bit level and both little-endian at the byte level, which is the specific
  claim this checklist item pins.
- **Term sort order:** not applicable at this layer (postings are ordered by
  ascending local ordinal, inherited from the segment's row-ID assignment, not a
  sort this blob itself performs).
- **Chunk codec:** not applicable — `storage-class: raw-mappable`, no chunk
  wrapper.
- **Checksums:** covered by this blob's own registry entry (`spec/container.md`
  §5, §6); no new checksum scope.
- **Codec-variant provenance:** this RFC's own precise registration —
  `BitPacker8x` (the `bitpacking` crate, `quickwit-oss`, already registered for R2
  per `docs/ledger.md`), vertical SIMD layout, 256-value blocks, first-order delta
  from zero — plus the variable-length final block's own scalar packer, specified
  completely in §4–§5 above (shift/mask, LSB-first, `ceil(n*width/8)` bytes), not
  merely named.
- **Stochastic-transform provenance:** not applicable — nothing here is
  stochastic.
- **Golden files:** the worked example above is the first `conformance/postings/`
  vector once implemented — real bytes, a real round trip, `doc_freq` supplied
  externally as any real reader would have it.

## How this could be wrong

**Nearest grave (`docs/lineage.md`): BitFunnel** — "a hardware-profile bet,
published with strong numbers, adopted by nobody." This RFC's central numbers —
`BitPacker8x` decoding ~14.2B/sec on synthetic data and ~15.6–19.17B/sec on real
MS MARCO data, decisively faster than every alternative measured — come from
exactly one x86 AVX2 machine, the same caveat `docs/ledger.md`'s R9 entry has
stated honestly since the numbers were first gathered rather than discovered
late. If `BitPacker8x`'s AVX2-specific advantage doesn't generalize — and the
FastLanes literature's own pitch is explicitly that portable, auto-vectorizing
layouts exist *because* hand-tuned SIMD intrinsics don't generalize across ISAs —
this RFC's confident default could be a narrow, single-hardware-profile bet
dressed up as a settled measurement. **Checked directly, not assumed, and the
result sharpens this risk rather than closing it**: an earlier version of this
RFC claimed the registered `bitpacking` crate's NEON path mitigates the ARM gap.
That claim was wrong. The crate's own current source
(`bitpacker8x.rs`, fetched and read directly) defines `BitPacker8x`'s
`InstructionSet` as exactly `{ #[cfg(target_arch = "x86_64")] AVX2, Scalar }` —
no NEON variant anywhere. NEON exists only in the same crate's `BitPacker4x`
(`bitpacker4x.rs`), a different, incompatible 128-value-block format this RFC
does not register. On ARM, the codec this RFC actually pins falls back to plain
scalar decode — no SIMD acceleration at all, not a slower-but-real alternative
path. This RFC's registration is therefore genuinely, not just apparently,
locked into x86 for its performance case; the ARM gap is real and unmitigated
by anything already shipped in the registered crate, and closing it (measuring
`BitPacker4x`'s NEON path, or `FastLanes`'s portable auto-vectorization, on real
ARM hardware) is necessary before this RFC's numbers can be called settled
anywhere but x86 — named here precisely, not glossed over, exactly to avoid
BitFunnel's mistake of publishing strong numbers from one profile as if they
settle the question everywhere.

**The variable-length final block trades a small amount of decode-path complexity
for a real, measured space and time win — worth stating precisely, not just
assumed a clean win.** A reader now has two decode paths per postings blob (a SIMD
path for full blocks, a scalar path for a possible final partial block) instead of
one, real added conformance and fuzzing surface (invariant 9's own scalar-vs-SIMD
equivalence testing requirement now has two scalar references to keep in sync,
not one). This is judged worth it because the alternative — padding — was shown
this session to cost roughly 4.5× the bytes on this corpus's real, short-list-
dominated shape, not a marginal difference.

**Block-max as a region within this blob, not its own registered blob, is a
specific reading of invariant 4's "sibling blob" wording that a future RFC could
reasonably contest** — see Alternatives considered.

## Alternatives considered

**Block-max as its own separately registered blob** (`family_id = 1`, a new
`blob_type_id`), matching invariant 4's literal "sibling blob" phrasing more
closely. Rejected here: a separate blob would need its own offset/length pair
somewhere per term, which means growing `TermInfo`'s already-tight 28-byte record
(`spec/term-dictionary.md` §3) by another 12 bytes (an `offset`/`length` pair) —
a real, concrete cost this session's own earlier analysis of a different
per-list-metadata question already flagged directly (`docs/research/
r2-hybrid-codec-methodology.md`'s engineering-surface-cost discussion). Keeping
`block_max` as a fixed-position region within the postings blob itself — always
first, always addressable via the same `postings_offset` `TermInfo` already
carries — avoids that cost while still keeping `block_max` structurally separate
from the codec's own packed bytes (never interleaved with them, never touched to
decide whether to skip a block), which is this RFC's reading of what invariant 4's
"sibling, never inside a codec's private structures" language is actually
protecting against: block-max data being unreadable without decompressing the
very bytes it exists to let a reader avoid touching. A registered separate blob
would satisfy the letter of "sibling blob" more literally; this RFC's reading
trades that literalness for avoiding `TermInfo` growth, and names the tension for
review rather than picking silently.

**FastPFOR as the default instead of `BitPacker8x`.** Rejected: this session's own
real measurement (Motivation, above) shows FastPFOR 5.9–8.3× slower to decode for
a real compression advantage of only 4.26–5.2× on real MS MARCO data — a real
trade, but one this RFC judges not worth making for the *default*, matching the
same conclusion the separate EF investigation reached for a different codec pair
via the same reasoning (`docs/research/r2-hybrid-codec-methodology.md`'s Phase 2B
checkpoint). FastPFOR remains a plausible future *registered alternative* for a
specific skewed-workload field, not ruled out categorically, just not chosen as
the one default this RFC registers.

**FastLanes instead of `BitPacker8x`.** Rejected on this hardware specifically:
measured 74–77% of `BitPacker8x`'s throughput, never faster at any bit width
tested. This rejection is honestly narrower than an earlier version of this RFC
claimed: FastLanes' real advantage — portability across ISAs via
auto-vectorization, without hand-tuned per-ISA intrinsics — is exactly the
property that matters most on hardware `BitPacker8x` doesn't have a tuned path
for, and `BitPacker8x` genuinely has no such path (How this could be wrong,
above — its own crate ships NEON only for the incompatible `BitPacker4x`
format). This RFC still registers `BitPacker8x` because the measured x86 win is
real and large, but does not claim — as an earlier draft incorrectly did — that
this choice is ARM-safe by virtue of the crate's own portability story. The
FastLanes-vs-`BitPacker8x` question for ARM specifically remains genuinely open,
named in Open questions rather than resolved by an inaccurate mitigation claim.

**Padding the final block instead of variable-length encoding.** Rejected: the
motivating, real-measured cost (Motivation, above) is exactly what this RFC exists
to close.

## Open questions / follow-on RFCs

- ARM/non-AVX2 validation (Non-goals, above) — genuinely open, not just
  unmeasured: `BitPacker8x` has no NEON path at all (How this could be wrong,
  above), so this needs either measuring `BitPacker4x`'s real NEON path (a
  different, incompatible block format) or `FastLanes`'s portable
  auto-vectorization on real ARM hardware before this RFC's default can be
  called settled anywhere but x86.
- Positions (phrase-query support, Non-goals) — a separate blob, a separate RFC.
- Term-frequency and document-length block-max bounds for BM25-scoring pruning
  (Non-goals) — real, separate future work tying into RFC 0003's scoring-profile
  inputs.
- Validated batch-size range (invariant 9, Non-goals) — turbopuffer's 512-value
  reference point is cited context, not a validated STRAND number.
- Whether FastPFOR should be registered as a second, optional codec for specific
  skewed-workload fields (Alternatives considered) — plausible, not attempted
  here.
- The block-max-as-region-not-separate-blob reading of invariant 4 (How this could
  be wrong) is this RFC's own judgment call, flagged precisely for review rather
  than treated as obviously correct.

## Discussion — post-approval amendments

Per `CLAUDE.md` §3, design problems (or, here, measurement problems) revealed
after approval are recorded here rather than silently rewritten into the
Napkin math section above, which is left unmodified as the historical record
of what was approved and why.

**The ~149-bytes/list mean-size projection was a real ~2.09× overestimate,
corrected 2026-08-19 against a real tantivy index on the same corpus.**
This RFC's own Napkin math used `bench/src/hybrid_codec_pilot.rs`'s
stratified 4,016-list sample (~149.1 bytes/list, `docs/ledger.md` R2) and
linearly extrapolated it across the full 413,364-term vocabulary to get
`413,364 * 149 ≈ 61.6 MB`. That extrapolation is now known wrong. Building
a real tantivy index over the identical corpus sample and token stream
(`bench/src/tantivy_index.rs`, every document fed as a `PreTokenizedString`
built from this project's own analyzer output, so tantivy's tokenizer is
never invoked — `bench/results/tantivy-index-benchmark.json`) gave a real
postings-file size of `29,504,002` bytes (`≈ 29.50 MB`). Suspecting the
extrapolation rather than the codec, `bench/src/msmarco_index.rs` was
extended to call `strand_lexical::postings::build_postings` — the actual
shipped implementation, not a projection — across every one of the real
413,364 terms and sum real bytes: `29,489,488` bytes (`≈ 29.49 MB`),
essentially identical to tantivy's real number (a `0.05%` difference,
`bench/results/msmarco-real-postings-sample.json`'s `stats.real_postings_bytes`).
The stratified sample's mean was real and correctly measured *on its own
4,016-list sample*; it simply did not generalize to the full, more
Zipf-skewed vocabulary via naive linear extrapolation. **The codec choice
this RFC makes is not merely unaffected by this correction — it is
strengthened by it**: `BitPacker8x` with a variable-length final block,
measured for real across the full vocabulary, matches a real, battle-tested
production engine's own postings size on the same corpus almost exactly.
Only the *estimate* was wrong, not the design. `rfcs/0008-positions.md`'s
own combined cold-open budget figure (`~92.5–100.6 MB`, built on this RFC's
now-corrected `~61.6 MB`) needs the same correction; see that RFC's own
Discussion section.
