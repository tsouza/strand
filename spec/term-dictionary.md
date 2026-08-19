# Term dictionary

Normative for STRAND v0.1. Defines the term-dictionary FST and term-info store
blobs for the lexical family. Approved by RFC 0005
(`rfcs/0005-term-dictionary.md`) and extended by RFC 0009
(`rfcs/0009-per-term-overhead-reduction.md` Design §2: a second, 16-byte,
positions-free term-info record shape, additively registered — RFC 0005's
original 28-byte record is untouched); this chapter states the settled
result — see the RFCs for the worked examples, alternatives considered, and
the adversarial reviews. Registered in `spec/container.md` §9: `family_id = 1`
(lexical), `blob_type_id = 0` (term-dictionary FST), `blob_type_id = 1`
(term-info store), `blob_type_id = 4` (term-info store, no positions).

Reference implementation: `crates/strand-lexical/src/term_dictionary.rs`. Golden
files: `conformance/term-dictionary/toy-terms.fst` and
`conformance/term-dictionary/toy-terms.terminfo` for the 28-byte record's
worked example, and `conformance/term-dictionary/short-term-info-worked-example.bin`
for the 16-byte record's, both matching their respective RFC's worked example
exactly, byte for byte. The postings blob `postings_offset`
points into is implemented (`spec/postings.md`, RFC 0007). The positions blob
`positions_offset` points into is also implemented (`spec/positions.md`, RFC
0008/0009).

## 1. Scope: one pair per field

A field with indexed lexical content carries exactly one term-dictionary FST blob
and one term-info store blob. A multi-field index carries one such pair per field.

## 2. Term-dictionary FST

Keys: a field's term bytes, in unsigned UTF-8 byte order (invariant 11). Values: a
dense `u64` ordinal, `0, 1, 2, ...` in insertion (sorted) order.

The blob's bytes are the `fst` crate's own compiled `Map` format
(`references/tantivy-fst-termdict-and-fst-crate.md`), treated as an opaque,
externally-defined structure this chapter does not re-specify. Per invariant 11, the
exact dependency MUST be registered precisely: `fst` crate version `0.4.7` is this
chapter's registration; an implementation MUST reconfirm the exact version it links
against and update this registration if it differs. `storage-class: raw-mappable`,
`tier: cold-fetchable`.

A lookup miss (the queried term is absent from this field in this segment) is a
normal outcome, not an error.

## 3. Term-info store

A flat array of fixed 28-byte records, one per term ordinal, in ordinal order —
ordinal `i`'s record is at byte offset `i * 28`. Little-endian (invariant 11):

| field              | type | notes                                                                    |
| ------------------ | ---- | ------------------------------------------------------------------------ |
| `doc_freq`         | u32  | documents in this segment containing the term (RFC 0003's scoring input) |
| `postings_offset`  | u64  | byte offset **within the postings blob** (not the segment file)          |
| `postings_length`  | u32  | byte length of this term's postings, within the postings blob            |
| `positions_offset` | u64  | byte offset **within the positions blob**                                |
| `positions_length` | u32  | byte length of this term's positions, within the positions blob          |

`storage-class: raw-mappable`, `tier: cold-fetchable`.

### 3a. Short term-info store (RFC 0009, no positions)

A field that will never carry a positions blob (`spec/positions.md` §1's
per-field opt-out) pays 12 bytes/term of permanent dead weight in the
28-byte record above for `positions_offset`/`positions_length` fields it
will never populate. `blob_type_id = 4` registers an alternative: a flat
array of fixed 16-byte records, identical mechanics (ordinal `i`'s record
at byte offset `i * 16`, little-endian), omitting the positions fields
entirely:

| field             | type | notes                                   |
| ----------------- | ---- | --------------------------------------- |
| `doc_freq`        | u32  | identical to the 28-byte record's field |
| `postings_offset` | u64  | identical to the 28-byte record's field |
| `postings_length` | u32  | identical to the 28-byte record's field |

`storage-class: raw-mappable`, `tier: cold-fetchable` — identical
classification to the 28-byte record.

A field registers exactly one term-info blob, `blob_type_id = 1` or
`blob_type_id = 4`, never both, and never neither. A field whose term-info
blob is `blob_type_id = 4` MUST NOT also register a positions blob
(`blob_type_id = 3`) for that field — there is no `positions_offset` field
to address it with, so a positions blob would be structurally unreachable,
not merely unused. This rule is correct but not yet mechanically checkable
by any reader: `spec/container.md` §5's blob registry carries no field
identifier, so multi-field blob addressing — which registry entry belongs
to which field — is unsolved project-wide (`rfcs/0008-positions.md`'s own
Non-goals, `rfcs/0009-per-term-overhead-reduction.md`'s own Non-goals), and
this rule inherits that gap rather than resolving it.

## 4. Query resolution

Given a query term and a field's already-resident FST and term-info blobs: look the
term up in the FST (§2); if found, its ordinal gives the term-info record's offset
directly (`ordinal * 28`, §3, or `ordinal * 16` for the short record, §3a); `doc_freq`
is immediately usable, and the postings/positions offsets (the short record has
no positions offset — a reader already knows from the registered `blob_type_id`
whether to expect one) locate the term's data once those blobs are also resident.
No step after both blobs are fetched costs a further round trip (invariant 3's
one-wave rule).

## 5. Placement constraint

Identical in spirit to `spec/scoring-profiles.md` §4 and `spec/analyzer-
descriptors.md` §6: both blobs are part of the cold-fetchable wave invariant 3
already budgets for after the segment open, adding bytes to that wave's payload but
no additional round trip.

## 6. Conformance status

Implemented (`crates/strand-lexical`), with both the 28-byte and 16-byte
record shapes' worked-example bytes pinned as `conformance/` golden files —
the 28-byte record's confirmed byte-exact against
`crates/strand-lexical/tests/worked_example.rs`, the 16-byte record's
against `crates/strand-lexical/tests/short_term_info_worked_example.rs`,
with property-based round-trip coverage in
`crates/strand-lexical/tests/short_term_info_round_trip.rs`.
Golden-file status is provisional on the cross-version/cross-platform determinism
question RFC 0005's "How this could be wrong" names — confirmed same-version,
same-platform only, by an actual test
(`references/tantivy-fst-termdict-and-fst-crate.md`), not yet across either axis
that matters for independent-implementation conformance.

## 7. Open dependencies

FST size at realistic vocabulary scale is unmeasured (RFC 0005 Open questions); the
cold-open byte budget question this chapter's blobs contribute to cannot be answered
with a real number until M1 benchmark data exists.
