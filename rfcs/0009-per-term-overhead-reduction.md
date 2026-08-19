# RFC 0009: Per-term fixed-overhead reduction (term-info and positions)

- **Status:** Approved. Adversarial review re-derived every arithmetic
  claim and both worked examples from scratch (all reproduced exactly)
  and found 2 Critical, 2 Important, and 2 Minor findings, all fixed.
  Critical: (1) Fix 1 amends `blob_type_id = 3`'s already-shipped,
  already-golden-filed layout in place, with no version discriminator —
  the draft's own Invariant-11 checklist claimed the old 12-byte and new
  8-byte golden files "both remain valid," which is not achievable as
  designed. Fixed by stating plainly that this is a breaking, in-place
  change (Design §1) that retires RFC 0008's original golden file rather
  than supplementing it, adding the rejected symmetric alternative (a new
  `blob_type_id` instead) to Alternatives considered, and naming the real
  cost in How this could be wrong rather than calling the whole fix
  "essentially free." (2) The 100,476-document napkin-math figures
  (vocabulary, positions/term_dict/term_info bytes) had no vendored
  source — `docs/ledger.md` only carries rounded MB figures at that scale,
  not exact bytes, and `bench/src/field_end_to_end.rs` printed but never
  committed its output. Fixed by adding real JSON output to that tool,
  re-running it at both scales, committing `bench/results/
  field-end-to-end-10003.json` and `-100476.json`, and citing them
  precisely — both reproduced the already-stated figures exactly, so no
  number changed, only its provenance. Important: this RFC's per-field
  term-info-shape selection silently assumed fields are distinguishable in
  the segment's blob registry, which RFC 0008's own Non-goals already
  named as unsolved project-wide — fixed by adding this dependency to
  Non-goals and Open questions and softening Design §2's claim to state
  plainly that the mutual-exclusivity rule is correct but currently
  unenforceable by any reader. Three internal citations pointed to the
  wrong RFC 0008 subsection (Design §1 and §8 cited where Design §10 was
  meant) and one to the wrong `spec/term-dictionary.md` section (§4 where
  §3 was meant) — the same citation-drift class prior RFCs' reviews have
  repeatedly caught — all four fixed by re-verifying against RFC 0008's
  and `spec/term-dictionary.md`'s actual current headings. Minor: a claim
  sourced to "the existing spec text" that actually only holds of the real
  implementation (`spec/positions.md` §6 itself uses raw subscript
  notation, not an accessor abstraction) — fixed to cite the code
  directly; and an incomplete self-critique that called fix 1 "essentially
  free" without separating the mechanical change (genuinely free) from the
  format-evolution cost (retiring a golden file, not free) — fixed by
  splitting the claim in How this could be wrong. **Implemented**
  (`crates/strand-lexical/src/positions.rs`,
  `crates/strand-lexical/src/term_dictionary.rs`): both fixes' predicted
  napkin-math figures were confirmed exactly against re-run real MS MARCO
  data — positions shrank from `620,503` to `493,359` bytes at 10,003
  documents and from `4,678,608` to `4,135,112` at 100,476, both matching
  this RFC's own prediction to the byte
  (`bench/results/field-end-to-end-10003.json`, `-100476.json`,
  regenerated post-implementation). RFC 0008's original 12-byte positions
  golden file is retired, replaced by this RFC's 8-byte one, exactly as
  Design §1 stated it would be.
- **Milestone:** M1 — Lexical (`docs/milestones.md`)
- **Spec chapters produced:** amends `spec/term-dictionary.md` §3 (new short
  term-info record, RFC 0005) and `spec/positions.md` §4–§5 (trims
  `postings_block_pos_prefix`, RFC 0008); additively extends
  `spec/container.md` §9 (registers `family_id = 1`, `blob_type_id = 4`,
  "term-info store, no positions")
- **Invariants exercised:** 8, 11 (`CLAUDE.md` §5)

## Summary

Two independent, additive fixes to per-term fixed overhead that real
measurement — a same-corpus, same-token-stream comparison against tantivy,
`docs/ledger.md`'s field-integration entries — found costing STRAND real
bytes for no benefit:

1. **`postings_block_pos_prefix[0]` is a mathematical constant (always `0`
   — nothing precedes a term's first postings block) and is currently
   stored anyway** (`spec/positions.md` §5). Omitting it shrinks every
   term's positions blob by 4 bytes, unconditionally, and shrinks the
   region to zero bytes for the common case (`postings_block_count == 1`,
   the overwhelming majority of terms under Zipf's law). Real, re-derived
   napkin math below: this alone shrinks STRAND's measured positions-blob
   gap against tantivy's real `.pos` from `33.2%` to `5.9%` at 10,003 real
   documents, and from `16.8%` to `3.3%` at 100,476.
2. **`TermInfo`'s 28-byte fixed record (`spec/term-dictionary.md` §3)
   reserves 12 bytes/term for `positions_offset`/`positions_length` even
   for a field that will never carry positions** — real, permanent dead
   weight for that field, not a transitional cost. This RFC registers a
   16-byte short record (`doc_freq` + `postings_offset` + `postings_length`
   only) as an alternative, selected at the field level by which
   `blob_type_id` the field's term-info blob registers as — no new schema
   flag.

Neither fix changes any codec, any d-gap convention, or any merge
semantics. Both are pure byte-layout trims to structures RFC 0005/0008
already approved.

## Motivation

**Fix 1's real, measured payoff.** `docs/ledger.md`'s field-integration
entries record a same-corpus, same-token-stream comparison against real
tantivy (`bench/src/field_end_to_end.rs` vs. `bench/src/tantivy_index.rs`,
both fed identical pre-tokenized text so the comparison isolates format
efficiency), with every STRAND-side figure now committed precisely in
`bench/results/field-end-to-end-10003.json` and
`bench/results/field-end-to-end-100476.json` (added alongside this RFC,
matching the detail level `bench/src/tantivy_index.rs`'s own committed
JSON already had): at 10,003 real documents, STRAND's positions blob
(620,503 bytes, `field-end-to-end-10003.json`'s `positions_bytes`) was
`33.2%` larger than tantivy's real `.pos` (465,942 bytes,
`bench/results/tantivy-index-benchmark-10003.json`); at 100,476 documents,
`16.8%` larger (4,678,608 bytes, `field-end-to-end-100476.json`'s
`positions_bytes`, vs. 4,004,502 bytes,
`bench/results/tantivy-index-benchmark-100476.json`). The
`postings_block_pos_prefix` region (`spec/positions.md` §5) is exactly
`vocabulary_size` entries too large: index `0` is defined as "the total
count of positions preceding postings block `0`'s first document," which
is `0` for every term, by construction, every time — nothing can precede
the first block. `spec/postings.md` §5's `block_max` has no equivalent
redundancy (each block's maximum is real, term-specific data); this one
entry, in this one region, is the only place this project's own postings/
positions design has a provably constant stored value.

**Fix 2's real, measured payoff.** The same comparison found
`term_dict + term_info` `46–48%` larger than tantivy's real term
dictionary at both scales (`docs/ledger.md`). Part of that gap is a
structural difference (a flat fixed-record array vs. an FST-based
dictionary) this RFC does not attempt to close. But part of it is real,
avoidable dead weight: a field that opts out of positions (RFC 0008
Design §10 already anticipates this — "a field that does not need
phrase-query support SHOULD omit its positions blob entirely,"
`spec/positions.md` §8, self-cited from RFC 0008 Design §2's scope)
still pays 12 bytes/term for fields it will never populate, because
`TermInfo`'s only registered shape is the 28-byte record. `crates/
strand-lexical/src/field.rs` does not yet exercise this path — its own doc
comment already names positions-opt-out as deferred, real future work, not
done in that module. This RFC provides the format-level capability that
follow-on work would need; it does not itself change `field.rs`.

## Non-goals

**Shrinking `total_term_freq`'s fixed 4 bytes** (e.g., a variable-length
integer encoding for the common small-value case) is not attempted here.
Every other multi-byte field in the postings/positions blobs (`spec/
postings.md`, `spec/positions.md`) is a fixed-width little-endian integer,
never a variable-length one (invariant 11's byte-determinism registrations
are all fixed-width so far); introducing the first variable-length integer
into this codec family is a real novelty-budget spend (invariant 8) this
RFC does not judge justified by one field's savings alone, named here as a
real, deferred option rather than silently assumed impossible.

**Shrinking `pos_widths`'s minimum 1 byte/term** is not attempted. That
byte is load-bearing — a reader cannot decode even a single scalar-packed
position block without knowing its bit width — and no redundancy exists
there the way it does for `postings_block_pos_prefix[0]`.

**Changing `crates/strand-lexical/src/field.rs` to actually build the
short term-info record for opt-out fields** is not done here. This RFC
registers the capability; wiring a positions-opt-out flag into `build_field`
is real, separate follow-on work, named in Open questions.

**Any change to `TermInfo`'s existing 28-byte record's own byte layout**
(field order, widths) is out of scope — that record is untouched; this RFC
adds a second, shorter record as an alternative, never modifies the first.

**Solving multi-field blob addressing** is not attempted here, and this
RFC's own per-field record-shape selection (Design §2) inherits that gap
rather than resolving it. `spec/container.md` §5's blob registry entry
(`family_id`, `blob_type_id`, `storage_class`, `tier`, `alignment`,
`chunk_codec`, `chunk_codec_level`, `offset`, `length`, `checksum`) carries
no field identifier at all — RFC 0008's own Non-goals already named this
as unsolved, inherited from RFC 0005/0006/0007. This RFC's Design §2
describes a reader locating "a field's term-info blob" by `blob_type_id`
as if fields were already distinguishable in the registry; they are not,
today, for *any* multi-field segment, independent of this RFC. Design §2's
mutual-exclusivity rule (exactly one term-info shape per field) is
therefore a real, correct requirement, but not yet a checkable one — no
mechanism exists by which a reader could detect a nonconforming writer
violating it, until multi-field addressing itself is solved.

## Design

### 1. `postings_block_pos_prefix[0]` omission (amends `spec/positions.md` §4–§5)

**This is a breaking, in-place change to `blob_type_id = 3`'s already-
approved, already-implemented, already-golden-filed layout — not an
additive registration.** STRAND has no format-version discriminator for a
blob's internal layout (only `Footer.format_major`/`format_minor` at the
container level, `spec/container.md` §2, which this RFC does not touch),
so there is no mechanism by which the old 12-byte encoding and this RFC's
new 8-byte encoding could both be current for `blob_type_id = 3` at once.
This RFC's own Fix 2, by contrast, registers a genuinely new
`blob_type_id` specifically so the old and new `TermInfo` shapes can
coexist (§2, below) — Fix 1 does not have that luxury, because
`postings_block_pos_prefix` lives inside the same already-registered
positions blob, and minting a second `blob_type_id` for "positions, but
with the redundant entry trimmed" was considered and rejected (Alternatives
considered) as disproportionate novelty spend for what the reference
implementation can absorb as a direct edit before any external consumer
depends on the old layout — this project has none yet. Adopting this RFC
**retires** RFC 0008's original 12-byte worked example and its golden
file; it does not coexist with the new 8-byte one. This is a real cost,
acceptable only because STRAND is pre-v0.1-freeze and RFC 0008 landed in
the same development arc as this RFC, not because breaking an approved
layout is free in general (see How this could be wrong).

The region's stored length changes from `4 * postings_block_count` bytes to
`4 * (postings_block_count - 1)` bytes: entries for postings blocks
`1..postings_block_count` are stored, in order; index `0`'s value is never
stored and is always `0` by definition. When `postings_block_count == 1`
(the common case — a term whose `doc_freq <= 256`), this region is empty:
zero bytes.

A reader's `postings_block_pos_prefix(i)` becomes: return `0` immediately
if `i == 0`; otherwise read the little-endian `u32` at byte offset
`4 * (i - 1)` within the (now shorter) region. Every other computation in
`spec/positions.md` §6 (targeted lookup, full decode) is unchanged — they
already only ever call `postings_block_pos_prefix(i)` through this
accessor, never index the region's raw bytes directly, confirmed against
the real implementation (`crates/strand-lexical/src/positions.rs`:
`PositionsReader::postings_block_pos_prefix` is the sole accessor,
`decode_position_block` and `decode_all` never touch the region directly)
— `spec/positions.md` §6's own prose uses raw subscript notation
(`postings_block_pos_prefix[lo]`) without naming an accessor abstraction,
so this guarantee is sourced to the code, not the spec text.

This is the entire mechanical fix: one region shrinks by exactly one
entry, and one accessor gains a one-line special case for index `0`. No
other byte in either blob moves.

### 2. Short term-info record (amends `spec/term-dictionary.md` §3; extends `spec/container.md` §9)

Registers `family_id = 1` (lexical, already registered), `blob_type_id = 4`
("term-info store, no positions") alongside the existing `blob_type_id = 1`
("term-info store"). A 16-byte fixed record, little-endian, one per term
ordinal, direct-indexed at `ordinal * 16` — identical mechanics to the
existing 28-byte record's `spec/term-dictionary.md` §3, just a shorter
record:

| field             | type | notes                                   |
| ----------------- | ---- | --------------------------------------- |
| `doc_freq`        | u32  | identical to the 28-byte record's field |
| `postings_offset` | u64  | identical to the 28-byte record's field |
| `postings_length` | u32  | identical to the 28-byte record's field |

**A field registers exactly one term-info blob, `blob_type_id = 1` or
`blob_type_id = 4`, never both, and never neither** (`spec/term-dictionary.md`
§1's existing "exactly one pair per field" scope, extended: exactly one
term-info *shape* per field). **A field whose term-info blob is
`blob_type_id = 4` MUST NOT also register a positions blob
(`blob_type_id = 3`) for that field** — there is no `positions_offset`
field to address it with, so a positions blob would be structurally
unreachable, not merely unused. This is a writer-time decision, made once
per field, based on whether that field will ever need phrase-query support
— the same per-field granularity `spec/positions.md` §1 already uses for
whether the positions blob exists at all, applied one layer up to the
term-info shape that has to agree with it.

A reader locates a field's term-info blob by whichever `blob_type_id`
(`1` or `4`) is present in the segment's blob registry, and uses the
matching record length and field set — the registry entry's own
`blob_type_id` is the shape declaration, not a new flag anywhere else.
**This description assumes a reader can already tell which registry entry
belongs to which field** — true for a single-field segment (this project's
only implemented case, `crates/strand-lexical/src/field.rs`), genuinely
open for a multi-field one (Non-goals, above): this RFC's mutual-exclusivity
rule is correct but currently unenforceable by any reader, for the same
reason RFC 0008's own positions-blob presence signal is.

## Worked examples

Both extend this project's own recurring worked-example term (local
ordinals `5, 12, 47`, term frequencies `2, 1, 3`) for continuity with RFC
0007/0008's own worked examples, computed and verified by executing the
real packer/encoder logic in Python, not hand-derived.

**Positions blob, fix 1.** The identical within-document positions RFC
0008 used (doc `5`: positions `[3, 9]`; doc `12`: `[0]`; doc `47`:
`[1, 4, 10]`) — `total_term_freq = 6`, one postings block
(`postings_block_count = 1`, doc_freq = 3), one position block
(`position_block_count = 1`). Under this RFC:

- `total_term_freq` (4 bytes): `06 00 00 00`.
- `postings_block_pos_prefix` (0 bytes — `postings_block_count - 1 = 0`
  entries): empty.
- `pos_widths` (1 byte, value `3`): `03`.
- Position-delta stream (3 bytes, identical to RFC 0008's own worked
  example): `33 32 03`.

**Full blob, 8 bytes**: `06 00 00 00 03 33 32 03` — 4 bytes shorter than
RFC 0008's own 12-byte worked-example blob, with exactly the
`postings_block_pos_prefix` region's single entry removed and nothing
else changed. (This 8-byte figure coincidentally matches an early,
*incorrect* draft of RFC 0008's own worked example, caught and fixed
during that RFC's ACPR review for an unrelated reason — an omission bug,
not this deliberate trim. The two are unrelated: RFC 0008's approved,
correct worked example is 12 bytes; this RFC's own, different, 8-byte
result is a new, deliberate reduction of that 12-byte figure, verified
independently above, not a reversion to the earlier mistake.)

**Short term-info record, fix 2.** A term with `doc_freq = 3`,
`postings_offset = 0` (first term in its field's postings blob), and
`postings_length = 10` (RFC 0007's own worked-example postings blob is
exactly 10 bytes, reused here for the same continuity):

`doc_freq` (4 bytes): `03 00 00 00`. `postings_offset` (8 bytes):
`00 00 00 00 00 00 00 00`. `postings_length` (4 bytes): `0A 00 00 00`.

**Full record, 16 bytes**: `03 00 00 00 00 00 00 00 00 00 00 00 0A 00 00 00`
— 12 bytes shorter than the equivalent 28-byte record would be for the
same term, with `positions_offset`/`positions_length` simply absent rather
than present-and-zero.

## Napkin math (`CLAUDE.md` §7)

Both fixes are cold-fetchable-wave payload (`spec/term-dictionary.md` §5,
`spec/positions.md` §8) — neither changes round-trip accounting, only
bytes.

**Fix 1, real and immediate**, computed from the now fully-committed
same-corpus comparison (`bench/results/field-end-to-end-10003.json`,
`bench/results/field-end-to-end-100476.json`,
`bench/results/tantivy-index-benchmark-10003.json`,
`bench/results/tantivy-index-benchmark-100476.json`): the fix removes
exactly 4 bytes per term, unconditionally — `vocabulary_size * 4` bytes
off the positions blob, full stop, no distributional assumption required
(unlike RFC 0007/0008's own napkin math, which had to bound an unmeasured
bit-width; this number is exact given only the vocabulary size, which is
real and already measured). At 10,003 documents (vocabulary `31,786`,
`field-end-to-end-10003.json`'s `vocabulary_size`): `31,786 * 4 = 127,144`
bytes saved; positions shrinks from `620,503` to `493,359` bytes — the gap
against tantivy's real `.pos` (`465,942` bytes) narrows from `33.2%` to
`5.9%`. At 100,476 documents (vocabulary `135,874`,
`field-end-to-end-100476.json`'s `vocabulary_size`): `135,874 * 4 =
543,496` bytes saved; positions shrinks from `4,678,608` to `4,135,112`
bytes — the gap against tantivy's real `.pos` (`4,004,502` bytes) narrows
from `16.8%` to `3.3%`. Combined with the already-real postings-codec
parity (`docs/ledger.md`; `postings_bytes` in both `field-end-to-end-*.json`
files), STRAND's **total segment size** gap against tantivy's real total
(`segment_bytes` vs. tantivy's summed `bytes_by_extension`, both committed
files) shrinks from `22.5%` to `16.0%` at 10,003 documents, and from `9.0%`
to `5.1%` at 100,476 — real, computed directly from committed numbers, not
estimated.

**Fix 2, real but not yet exercised.** `12` bytes/term is exact and
unconditional for any field that adopts the short record, but `crates/
strand-lexical/src/field.rs` does not yet build fields that opt out of
positions, so this fix moves no number in the current, committed
comparison. Applied hypothetically to the same two samples' committed
vocabulary sizes, purely to show the magnitude: `31,786 * 12 = 381,432`
bytes at 10,003 documents; `135,874 * 12 = 1,630,488` bytes at 100,476 —
real savings, for a real use case (a field that never needs phrase
queries), once that use case exists in code. This RFC does not claim this
fixes today's measured comparison; it registers the format capability the
eventual fix would use.

## Invariant-11 checklist

- **Endianness:** little-endian throughout — the short term-info record's
  three fields, identical convention to the existing 28-byte record; the
  trimmed `postings_block_pos_prefix` region's remaining entries, unchanged
  from RFC 0008's own registration.
- **Term sort order:** not applicable — neither fix touches term ordering.
- **Chunk codec:** not applicable — both blobs remain `storage-class:
  raw-mappable`, unchanged from RFC 0005/0008.
- **Checksums:** covered by each blob's own registry entry (`spec/
  container.md` §5–§6); no new checksum scope — the short term-info record
  is a new `blob_type_id`, so it gets its own registry entry like any
  other blob, with no special-casing.
- **Codec-variant provenance:** not applicable — neither fix is a codec;
  both are fixed-width record/region trims, fully specified above.
- **Stochastic-transform provenance:** not applicable.
- **Golden files:** both worked examples above are the first golden
  vectors for these two changes, once implemented. **`conformance/
  positions/toy-positions.bin` is replaced, not supplemented**: this RFC's
  8-byte blob supersedes RFC 0008's 12-byte one, since `blob_type_id = 3`
  can only have one current layout (Design §1's breaking-change note,
  above) — the old fixture is retired, not kept alongside the new one.
  `conformance/term-dictionary/` genuinely does gain a second, additive
  fixture (the new 16-byte short record) alongside the existing 28-byte
  one, since Fix 2 registers a distinct `blob_type_id` rather than
  amending the existing one — the two fixes are asymmetric here, and this
  checklist item states that asymmetry rather than treating both as the
  same kind of addition.

## How this could be wrong

**Nearest grave (`docs/lineage.md`): none of the named graves (Indri,
Galago, BitFunnel, the Optane-era formats, Pilosa, CIFF) is really about
per-record byte trimming — the closest lesson is CIFF's own, generalized
one: "a format nobody's production engine is economically forced to read
is a paper artifact." The risk this RFC actually carries is the opposite
kind of failure: spending real engineering and conformance surface chasing
a comparison metric (total bytes vs. tantivy) this project's own mission
(`CLAUDE.md` §1) states is not the actual point — the actual point is
round-trip-bound cold open, already validated separately (`bench/src/
field_cold_open.rs`, `docs/ledger.md`). This RFC is judged worth it anyway
because fix 1's *mechanical* decode-side change is essentially free (one
accessor, one special case for index `0`) and because byte-for-byte parity
with a real, battle-tested engine is still a legitimate, if secondary,
signal of a competent implementation — but "essentially free" describes
only that mechanical change, not fix 1 as a whole: it also retires an
already-shipped golden file and changes an already-approved RFC's own
worked example (Design §1's breaking-change note, above), a real,
if pre-v0.1, cost this section does not fold into "free" alongside the
mechanical part. A future reader should not mistake this RFC for evidence
the project's priorities shifted toward total-size competition.**

**Fix 2's real cost is conformance and test surface, not bytes.** Every
future reader implementation now has two term-info record shapes to
support, not one — real, if modest, doubled surface for invariant 9's
scalar-vs-decode equivalence discipline and for any future clean-room
implementation (M4). This RFC accepts that cost because the underlying
capability (opt-out-of-positions fields) is real and already named as
desirable in RFC 0008 Design §10's own text, not invented here to justify
a format change.

**Fix 1's "always zero" claim depends on `spec/positions.md` §5's own
definition holding exactly as stated** — if a future amendment changed
what block `0` means (e.g., supporting a positions blob that starts
mid-stream for some reason not currently in the format), this RFC's
constant-folding would silently become wrong. No such amendment is
proposed or anticipated, but the dependency is real and named, not
assumed permanently safe by construction alone.

## Alternatives considered

**Registering a new `blob_type_id` for the trimmed positions layout
instead of amending `blob_type_id = 3` in place** — true backward
compatibility, symmetric with Fix 2's own new `blob_type_id = 4`.
Rejected: STRAND has no real external consumers of RFC 0008's positions
blob yet (it landed in this same development arc, `crates/strand-lexical`
is the only implementation), so the coexistence a new ID would buy has no
one to serve today, at the real cost of a second positions-blob shape (a
third overall, alongside the two `TermInfo` shapes) with its own golden
file and property-test surface. If STRAND gains external readers of
`blob_type_id = 3` before this RFC lands, this alternative should be
revisited rather than assumed still unnecessary — named here as a real
condition, not a permanent judgment.

**A variable-length integer encoding for `total_term_freq`** (Non-goals,
above) — real savings for the common small-value case, rejected here as a
larger novelty-budget spend than this RFC's other two fixes, deferred
rather than bundled in.

**Making the short term-info record the *only* record, and moving
`positions_offset`/`positions_length` into the positions blob itself
instead of `TermInfo`** — rejected: this is structurally what RFC 0008 §4
already does for `total_term_freq` (a value not recoverable from `TermInfo`
alone lives inside the positions blob instead), but `positions_offset`/
`positions_length` are needed to *locate* that blob's per-term region in
the first place, so they cannot themselves live inside it without a
chicken-and-egg problem. Keeping both record shapes, selected per field,
is the design that avoids this without inventing an additional index
layer.

**Bit-packing `postings_block_pos_prefix`'s remaining entries at less than
32 bits, mirroring the postings/positions delta streams' own per-block
width selection** — considered and rejected as disproportionate: these
values are already `O(vocabulary_size)` in count for the rare
multi-postings-block terms only, a small fraction of the corpus by
RFC 0007's own "69% lists of length `<= 8`" finding, so the added
decode-path complexity (a third bit-packed region, with its own width
byte) is not judged worth it for a region fix 1 already reduces to zero
bytes for the overwhelming majority of terms.

## Open questions / follow-on RFCs

- Wiring a positions-opt-out flag into `crates/strand-lexical/src/field.rs`
  so fix 2 actually moves a measured number — real, separate follow-on,
  not attempted here (Non-goals).
- A variable-length `total_term_freq` encoding (Non-goals, Alternatives
  considered) — real, deferred, not attempted here.
- Whether the same "index `0` is always `0`" redundancy exists anywhere
  else in the format not yet audited for it — this RFC found one instance
  by direct inspection of `spec/positions.md`, not a systematic sweep of
  every blob family; a broader audit is real, separate, unattempted work.
- Multi-field blob addressing (Non-goals) — inherited as open from RFC
  0005/0006/0007/0008, not solved here; this RFC's own per-field
  term-info-shape rule is unenforceable until it lands.
- Whether a future segment with real external readers of `blob_type_id = 3`
  should change this RFC's Fix 1 from an in-place amendment to a new,
  separately-registered `blob_type_id` (Alternatives considered) — not
  needed today, named as a condition to watch for.

## Discussion — post-approval amendments

Per `CLAUDE.md` §3, corrections revealed after approval are recorded here.

**Multi-field blob addressing (Non-goals, above) is resolved — 2026-08-19,
roadmap item X-1.** This RFC's own Non-goals section states the gap
precisely: `spec/container.md` §5's blob registry entry "carries no field
identifier at all... Design §2's mutual-exclusivity rule (exactly one
term-info shape per field) is therefore a real, correct requirement, but
not yet a checkable one." `spec/container.md` §5 belongs to RFC 0001
(container, row-ID space, manifest), not this one, so the fix — a new
`field_id: u64` field on every `blob_entry`, `spec/container.md` §5a — is
recorded in `rfcs/0001-container-rowid-manifest.md`'s own Discussion
section. This RFC's Non-goals paragraph is left exactly as originally
written, an accurate record of what was true at approval. Design §2's
mutual-exclusivity rule is now genuinely checkable: `crates/strand-
lexical/src/field.rs`'s `FieldReader::open` selects a field's term-info
blob (either shape, `blob_type_id = 1` or `4`) by `field_id` in addition
to `blob_type_id`, so two fields in one segment — one using the full
record, one the short one — no longer collide, and each field's own
"exactly one shape" property is enforced by construction: `to_blob_specs`
emits exactly one term-info blob per field, tagged with that field's own
`field_id`, never both shapes for the same field.
