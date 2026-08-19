# Term dictionary

Normative for STRAND v0.1. Defines the term-dictionary FST and term-info store
blobs for the lexical family. Approved by RFC 0005
(`rfcs/0005-term-dictionary.md`); this chapter states the settled result — see the
RFC for the worked example, alternatives considered, and the adversarial review.
Registered in `spec/container.md` §9: `family_id = 1` (lexical), `blob_type_id = 0`
(term-dictionary FST), `blob_type_id = 1` (term-info store).

Reference implementation: `crates/strand-lexical/src/term_dictionary.rs`. Golden
files: `conformance/term-dictionary/toy-terms.fst` and
`conformance/term-dictionary/toy-terms.terminfo`, matching this chapter's RFC's
worked example exactly, byte for byte. The postings blob `postings_offset`
points into is implemented (`spec/postings.md`, RFC 0007). The positions blob
`positions_offset` points into is also implemented (`spec/positions.md`, RFC
0008).

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

## 4. Query resolution

Given a query term and a field's already-resident FST and term-info blobs: look the
term up in the FST (§2); if found, its ordinal gives the term-info record's offset
directly (`ordinal * 28`, §3); `doc_freq` is immediately usable, and the postings/
positions offsets locate the term's data once those blobs are also resident. No step
after both blobs are fetched costs a further round trip (invariant 3's one-wave
rule).

## 5. Placement constraint

Identical in spirit to `spec/scoring-profiles.md` §4 and `spec/analyzer-
descriptors.md` §6: both blobs are part of the cold-fetchable wave invariant 3
already budgets for after the segment open, adding bytes to that wave's payload but
no additional round trip.

## 6. Conformance status

Implemented (`crates/strand-lexical`), with both blobs' worked-example bytes pinned
as `conformance/` golden files and confirmed byte-exact against `crates/strand-lexical/tests/worked_example.rs`.
Golden-file status is provisional on the cross-version/cross-platform determinism
question RFC 0005's "How this could be wrong" names — confirmed same-version,
same-platform only, by an actual test
(`references/tantivy-fst-termdict-and-fst-crate.md`), not yet across either axis
that matters for independent-implementation conformance.

## 7. Open dependencies

FST size at realistic vocabulary scale is unmeasured (RFC 0005 Open questions); the
cold-open byte budget question this chapter's blobs contribute to cannot be answered
with a real number until M1 benchmark data exists.
