# Robertson & Zaragoza — "The Probabilistic Relevance Framework: BM25 and Beyond"

Vendored excerpt. Source: `staff.city.ac.uk/~sbrp622/papers/foundations_bm25_review.pdf`
(the authors' own posted copy of *Foundations and Trends in Information Retrieval*,
Vol. 3, No. 4 (2009), pp. 333–389, DOI 10.1561/1500000019). Read directly as a PDF.
Fetched 2026-08-18.

Cited by: the M1 scoring-profiles RFC (drafted 2026-08-18), `CLAUDE.md` invariant 5.

## The canonical BM25 term-weighting formula (§3.4.4–3.4.5, eq. 3.10–3.15)

The saturation function (eq. 3.10): `tf / (k + tf)` for some `k > 0`.

Document length normalization (eq. 3.12): `B := (1-b) + b * dl/avdl`, `0 ≤ b ≤ 1`,
where `dl` is document length and `avdl` the collection's average document length.
Length-normalized term frequency (eq. 3.13): `tf' = tf / B`.

The full formula, in the paper's own final form (eq. 3.15):

> w_i^BM25(tf) = tf / (k1·((1-b) + b·dl/avdl) + tf) · w_i^RSJ

**No `(k1+1)` factor multiplies the numerator in this, the paper's own canonical
form.** The paper states this explicitly, in a "Variations on BM25" section
immediately following (§3.5.1):

> "A common variant is to add a `(k1 + 1)` component to the numerator of the
> saturation function. This is the same for all terms, and therefore does not affect
> the ranking produced. The reason for including it was to make the final formula
> more compatible with the RSJ weight used on its own. If it is included, then a
> single occurrence of a term would have the same weight in both schemes."

This is the opposite of what an earlier, memory-based draft of the scoring-profiles
RFC assumed (that the "textbook" formula carries `(k1+1)` and real engines deviate
from it) — the primary source's own canonical definition has no such factor; adding
one is an explicitly named variant, not the base case, and the authors state plainly
it cannot change rank order.

## Document length, defined precisely (§3.4.5)

> "We define document length in an obvious way: document length `dl := Σ_{i∈V} tf_i`"

Document length is the sum of term frequencies over the document's vocabulary — a
token count, not a byte count or character count. The paper separately notes (same
section) that in practice "the number of characters in the document, or the number
of words before parsing, or even the number of unique terms" all give "very similar
results," but the formal definition is the token-frequency sum.

## Parameter guidance (§3.5)

> "values such as `0.5 < b < 0.8` and `1.2 < k1 < 2` are reasonably good in many
> circumstances. However, there is also evidence that optimal values do depend on
> other factors (such as the type of documents or queries)."

A "common combination" of `b = 0.5, k1 = 2` is also noted, with the caveat that "many
experiments suggest a somewhat lower value of `k1` and a somewhat higher value of
`b`" than that combination — consistent with Lucene's own defaults, `k1 = 1.2, b =
0.75` (`references/lucene-bm25similarity-and-smallfloat.md`), which sit inside the
paper's own "reasonably good" range.

## The RSJ-to-idf reduction (§3.1, eq. 3.2–3.3) — closed by adversarial review

An earlier version of this file flagged the classic idf formula's exact equation
number as unconfirmed. The RFC 0003 adversarial review (2026-08-18) traced it: §3.1
("The Binary Independence Model") derives the RSJ weight (eq. 3.2), then shows that
setting `R = r_i = 0` (no relevance information available) is equivalent to setting
`P(t_i|rel) = 0.5`, giving eq. 3.3:

> w_i^IDF = log((N − n_i + 0.5) / (n_i + 0.5))

This is an exact match, verbatim, to the classic Robertson–Sparck-Jones idf form the
`bm25` profile uses. The citation is complete as of this update; earlier text in this
file describing it as unconfirmed no longer applies.

## `sum-of-term-weights`'s exact location (§3.4.5, not §2.4)

A document's full score is the sum of term weights over query terms present in the
document — stated by the paper immediately after eq. 3.15, at the end of §3.4.5, not
in §2.4 (which covers general notation and the "removing the zeros" arithmetic trick,
not this specific summation statement). An earlier version of the scoring-profiles
RFC cited §2.4 for this; corrected during adversarial review.
