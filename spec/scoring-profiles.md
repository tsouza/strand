# Scoring profiles

Normative for STRAND v0.1. Defines the scoring-profile descriptor and the two
profiles this chapter registers: `bm25` and `lucene-parity`. Approved by RFC 0003
(`rfcs/0003-scoring-profiles.md`); this chapter states the settled result — see the
RFC for alternatives considered, the worked example, and the adversarial review.

Reference implementation: `crates/strand-core/src/scoring.rs` — the `bm25` and
`lucene-parity` scoring formulas, including `SmallFloat.intToByte4`/`byte4ToInt`
ported directly from the vendored Lucene 10.5.1 source
(`references/lucene-bm25similarity-and-smallfloat.md`), tested against this
chapter's RFC's worked example to 1e-6. The descriptor's own wire placement (which
blob carries these bytes) still lands with the R2 lexical blob (M1,
`docs/ledger.md`).

## 1. The descriptor

A scoring-profile descriptor is JSON with two fields:

| field        | type   | notes                                                            |
| ------------ | ------ | ----------------------------------------------------------------- |
| `profile_id` | string | `"bm25"` or `"lucene-parity"` at v0.1; other identifiers are non-conforming |
| `parameters` | object | profile-specific, §2 and §3                                       |

Where the descriptor's bytes physically live inside a segment is not pinned by this
chapter — that is the R2/postings RFC's decision, bound by the constraint in §4.

## 2. The `bm25` profile

`parameters`: `k1` (float, `k1 ≥ 0`, finite, default `1.2`), `b` (float, `0 ≤ b ≤ 1`,
default `0.75`).

`N`, `n`, and `avdl` below are **per-segment statistics**, computed over the
segment's own documents only, never aggregated across a multi-segment index — the
same per-segment scope invariant 3 and the manifest layer already hold to
(`CLAUDE.md` §5, §7; `docs/ledger.md` R10). A multi-segment comparison against an
engine that aggregates statistics globally (Lucene's `IndexSearcher` ordinarily
does) is out of this format's scope; RFC 0003 states the M1 benchmark implication.

For a query term with document frequency `n` in a field-level collection of `N`
documents carrying that field, and a document with term frequency `tf` and field
length `dl` in a collection whose average field length is `avdl`:

```
idf(n, N)                 = ln((N - n + 0.5) / (n + 0.5))
norm(dl, avdl)             = k1 * ((1 - b) + b * dl / avdl)
score(tf, n, N, dl, avdl)  = idf(n, N) * tf / (tf + norm(dl, avdl))
```

A document's score for a query is the sum of `score(...)` over the query's terms
present in that document. This is Robertson & Zaragoza's own canonical BM25 form; it
carries no `(k1 + 1)` numerator factor, since that factor is a documented variant
that does not affect ranking (RFC 0003 explains why it is deliberately omitted).

Negative `idf` (for a term with `n > N/2`) is intentional. A conforming
implementation MUST NOT clip a negative `idf` value or a negative term-score
contribution to zero.

## 3. The `lucene-parity` profile

`parameters`: same `k1`/`b`, same defaults. Two formula differences from `bm25`,
both MUST be implemented exactly as stated, not approximated:

**idf.** `idf_lucene(n, N) = ln(1 + (N - n + 0.5) / (n + 0.5))`. Note the `+1` inside
the logarithm, absent from `bm25`'s idf.

**Document-length quantization.** `norm` MUST be computed from a quantized length,
not the stored lossless `dl`:

```
dl_lucene = byte4_decode(byte4_encode(dl))
norm_lucene(dl, avdl) = k1 * ((1 - b) + b * dl_lucene / avdl)
score_lucene(tf, n, N, dl, avdl) = idf_lucene(n, N) * tf / (tf + norm_lucene(dl, avdl))
```

`byte4_encode`/`byte4_decode` MUST implement Lucene 10.5.1's
`SmallFloat.intToByte4`/`byte4ToInt` exactly (`references/lucene-bm25similarity-and-
smallfloat.md` carries the source to implement against). This is an integer-valued,
4-significant-bit encoding, not the classic `byte315` float encoding — document
lengths `0`–`23` tokens MUST encode exactly (no information loss); lengths `24` and
above enter a lossy floating encoding a conforming implementation MUST reproduce
bit-for-bit, not approximate, since a conformance harness comparing scores would
otherwise silently diverge on the majority of real documents.

## 4. Placement constraint

Whatever mechanism eventually carries a scoring-profile descriptor's bytes (decided
by the R2/postings RFC) MUST NOT require a round trip beyond invariant 3's ≤2-RTT
open budget (`CLAUDE.md` §5). The descriptor is small — a profile identifier string
plus at most two floats — and MUST arrive as part of whatever the open protocol
already fetches wholesale.

## 5. Score parity (invariant 5)

Engine B evaluating a segment's declared profile MUST match engine A's score within
a stated floating-point tolerance. This chapter pins that tolerance at **relative
error ≤ 1e-5**, a starting point for the M1 Lucene-parity benchmark to confirm or
tighten (`docs/milestones.md`), not an empirically validated figure. For
`lucene-parity` specifically, parity means a harness computing Lucene's quantized
norm from STRAND's own lossless length and comparing like with like — never a demand
that STRAND's stored length itself be lossy.

## 6. Conformance status

Implemented (`crates/strand-core/src/scoring.rs`). `references/lucene-bm25similarity-and-smallfloat.md`
and `references/robertson-zaragoza-bm25-and-beyond.md` carry the primary sources
this chapter is grounded against. RFC 0003's worked example is a test in that
module (`worked_example_bm25_profile`, `worked_example_lucene_parity_profile`),
checked to 1e-6 rather than a `conformance/` binary golden file — the descriptor
itself is JSON metadata (§1), not a fixed binary layout, so there is no wire-byte
golden file to pin the way `conformance/term-dictionary/` and
`conformance/filter-bitmaps/` pin theirs; a numeric worked-example test is the
right-shaped conformance check here.

## 7. Per-document length — resolved by `spec/analyzer-descriptors.md`

Per-document length — which tokens count toward `dl`, at which point in the analysis
chain, per field — is defined in `spec/analyzer-descriptors.md` §4 (RFC 0004,
`rfcs/0004-analyzer-descriptors.md`), not restated here. `lucene-parity` requires
`counts_overlaps_in_length = false` on the field's analyzer descriptor
(`spec/analyzer-descriptors.md` §4) — matching Lucene's own `discountOverlaps =
true` default, which excludes zero-position-increment tokens from the counted
length.
