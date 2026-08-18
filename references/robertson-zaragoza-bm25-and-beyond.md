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

## What this vendoring does not independently re-confirm

The RSJ weight `w_i^RSJ`, in the absence of relevance feedback, "reduces... to a form
of idf" (§3.5, already quoted in the source material read for this vendoring) — but
this pass did not independently re-derive or re-quote the RSJ-to-idf reduction's
exact equation number from the paper's earlier sections (§2.4–3.1, the binary
independence model derivation). The classic Robertson–Sparck-Jones idf form, `log((N
- n + 0.5) / (n + 0.5))`, is used in the scoring-profiles RFC on the strength of its
wide, independent citation elsewhere in the IR literature, not on a byte-exact
equation-number match confirmed from this specific PDF in this pass — flagged
honestly rather than asserted as independently re-verified here.
