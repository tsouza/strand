# Analyzer descriptors

Normative for STRAND v0.1. Defines the analyzer-descriptor schema and per-document
length. Approved by RFC 0004 (`rfcs/0004-analyzer-descriptors.md`); this chapter
states the settled result — see the RFC for the worked example, alternatives
considered, and the adversarial review.

Reference implementation: `crates/strand-lexical/src/analyzer.rs` — the one
descriptor RFC 0004's worked example names (UAX29-word tokenization with
`"word-only"` retention, `"lower"` case folding, `lucene-en-10.5.1` stopwords,
`snowball-porter2-en` stemming), built on the real `unicode-segmentation` and
`rust-stemmers` crates rather than hand-rolled algorithms. The descriptor's own
wire placement (which blob carries these bytes) still lands with the R2 lexical
blob (M1, `docs/ledger.md`). Conformance vectors: `conformance/analyzers/` —
`lucene-en-word-only-01.json` pins RFC 0004's worked example (raw text,
descriptor, expected token stream, expected `dl`) as the first normative vector,
checked in `crates/strand-lexical/tests/analyzer_conformance_vectors.rs`.

## 1. The descriptor

JSON with the following fields, all REQUIRED except where noted:

| field                        | type              | notes                                |
| ----------------------------- | ----------------- | -------------------------------------- |
| `unicode_version`              | string             | e.g. `"17.0.0"`                        |
| `icu_version`                  | string             | versioned independently of CLDR        |
| `cldr_version`                 | string             |                                         |
| `tokenizer_profile`            | object             | §2                                     |
| `stopword_list_id`             | string or `null`   | `null` = no stopword filtering         |
| `stemmer`                      | object or `null`   | `null` = no stemming                   |
| `segmentation_dictionary`      | object or `null`   | MUST be non-null for CJK/Thai/Lao content |
| `counts_overlaps_in_length`    | boolean            | §4                                     |

An index carrying undeclared analysis (a lexical blob with no descriptor, or a
descriptor that omits a field this table marks required) is invalid, per invariant
6.

## 2. `tokenizer_profile`

```
{
  "algorithm": "UAX29-word",
  "unicode_version": <string, matches the descriptor's own unicode_version>,
  "token_retention": "word-only" | "all-segments",
  "case_folding": "none" | "lower" | "full-case-fold",
  "deviations": [ <string> ]
}
```

`deviations` MUST be present even when empty. An empty list is a normative claim —
this tokenizer follows stock UAX #29 at the declared `unicode_version` with no
property overrides — not merely the absence of a claim.

`token_retention: "word-only"` MUST use the following criterion, not an appeal to
any word/non-word classification in UAX #29 itself (the annex defines only
break/no-break positions between characters and has no such classification): a
boundary segment is retained if and only if it contains at least one character with
the Unicode `Alphabetic` property or `General_Category = Number`
(`references/unicode-segmentation-word-filter-criterion.md`).

`case_folding` distinguishes no case transformation (`"none"`), ordinary lowercasing
(`"lower"`), and Unicode's full case-folding algorithm (`"full-case-fold"`, which
normalizes cases simple lowercasing does not — e.g. U+00DF LATIN SMALL LETTER SHARP
S has only a full case-folding mapping, to `"ss"`, no simple one,
`references/unicode-casefolding-sharp-s.md`).

## 3. `stopword_list_id` and `stemmer`

`stopword_list_id` MUST resolve to an exact, versioned word list — specific enough
that any two engines holding the same identifier hold byte-identical lists. This
chapter does not mandate a resolution/registry mechanism.

`stemmer`, when non-null:

```
{ "name": <string>, "version": <string> }
```

`version` MUST anchor to a specific, reproducible point (a released version or
commit hash of the stemmer's own algorithm definition), never an unqualified
"latest."

## 4. Per-document length

Per-document length, for a field, is the count of tokens surviving that field's full
declared analysis chain (tokenization per §2, then stopword removal per §3 if
declared, then stemming per §3 if declared, in that order) — exactly `dl := Σ_{i∈V}
tf_i` over the tokens this chain actually produces as indexed postings, never a raw
pre-analysis count.

`counts_overlaps_in_length` (boolean) states whether tokens sharing a zero position
increment with the preceding token count toward this length. RFC 0003's `bm25`
profile uses whichever value a segment's descriptor declares. RFC 0003's
`lucene-parity` profile additionally REQUIRES `counts_overlaps_in_length = false` —
a segment declaring `true` while claiming `lucene-parity` scoring is non-conforming.

## 5. `segmentation_dictionary`

```
{ "script": <string, a Unicode Script property value>, "identity": <string>, "version": <string> } | null
```

`script` names one Script value (e.g. `"Han"`, `"Thai"`, `"Lao"`, `"Hiragana"`,
`"Katakana"`, `"Hangul"`), not the umbrella term "CJK" — content mixing scripts
needs per-script handling. MUST be non-null when the field's content uses a
dictionary-segmented script (CJK, Thai, Lao). This chapter does not yet pin a
default dictionary (RFC 0004 Non-goals);
a conforming segment states one explicitly regardless.

## 6. Placement constraint

Identical to `spec/scoring-profiles.md` §4: whatever mechanism eventually carries a
descriptor's bytes MUST NOT require a round trip beyond invariant 3's ≤2-RTT open
budget.

## 7. Conformance status

One descriptor implemented and pinned (`crates/strand-lexical/src/analyzer.rs`,
`conformance/analyzers/lucene-en-word-only-01.json`) — the `lucene-en-10.5.1`
English chain RFC 0004's worked example describes. Every other declared
combination this schema permits (other languages, other stopword lists,
`token_retention: "all-segments"`, `case_folding: "full-case-fold"`, dictionary
segmentation) remains unimplemented and unvectored; each is real, separate M1
execution work, not covered by this one vector's presence.
