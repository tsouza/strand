# Cormack, Clarke & Büttcher (2009) — Reciprocal Rank Fusion

Vendored excerpt, not the full paper. Source: G. V. Cormack, C. L. A.
Clarke, S. Büttcher, "Reciprocal Rank Fusion outperforms Condorcet and
individual Rank Learning Methods," SIGIR'09, July 19–23, 2009, Boston,
Massachusetts, USA. ACM 978-1-60558-483-6/09/07. Fetched 2026-08-20 from
the paper's own institutional host,
`http://cormack.uwaterloo.ca/cormacksigir09-rrf.pdf` (redirected there
from `https://plg.uwaterloo.ca/~gvcormac/cormacksigir09-rrf.pdf`, the
author's own faculty page — both hosts are the authors', not a
third-party mirror). Vendored because `CLAUDE.md` §3 forbids implementing
against a remembered spec, and RRF's formula and its `k = 60` constant
are exactly the kind of widely-repeated-from-memory fact that rule warns
against — the source is fetched and cited here rather than trusted from
memory, and `crates/strand-core/src/fusion.rs` cites this file directly.

---

### The formula, in the paper's own words (§1, "Reciprocal Rank Fusion")

> "RRF simply sorts the documents according to a naive scoring formula.
> Given a set D of documents to be ranked and a set of rankings R, each a
> permutation on 1..|D|, we compute
>
> RRFscore(d ∈ D) = Σ_{r∈R} 1 / (k + r(d)),
>
> where k = 60 was fixed during a pilot investigation and not altered
> during subsequent validation. Our intuition in choosing this formula
> derived from fact that while highly-ranked documents are more
> important, the importance of lower-ranked documents does not vanish as
> it would were, say, an exponential function used. The constant k
> mitigates the impact of high rankings by outlier systems."

`r(d)` is document `d`'s rank position within ranking `r` — a permutation
on `1..|D|`, i.e. **1-based**: the top-ranked document in any input
ranking has `r(d) = 1`, not `0`. A document absent from a given ranking
`r` simply contributes no term for that `r` to the sum (§1 defines `R` as
"a set of rankings," each covering however much of `D` it ranks; the
paper's own worked fusions combine rankings of unequal, overlapping
document sets from independent TREC/Wumpus systems). The final score is
the sum of `1/(k + r(d))` across every ranking `d` appears in; documents
are then sorted by descending `RRFscore`.

### `k = 60`, and why it's not "critical" (§1, Table 1)

The paper's own pilot swept `k ∈ {0, 10, 20, ..., 100, 500}` against MAP
on TREC topics 351–400 (30 fused system rankings) and reports (Table 1):

| k    | 0     | 10    | 20    | 30    | 40    | 50    | 60    | 70    | 80    | 90    | 100   | 500   |
|------|-------|-------|-------|-------|-------|-------|-------|-------|-------|-------|-------|-------|
| MAP  | .2072 | .2123 | .2134 | .2139 | .2138 | .2144 | .2145 | .2146 | .2147 | .2145 | .2142 | .2098 |

The text around the table: "The results of the first [pilot experiment],
shown in table 1, indicated that k = 60 was near-optimal, but that the
choice was not critical." Peak MAP in this sweep is actually at `k = 80`
(.2147), with `k = 60` close behind (.2145) — the paper's own stated
conclusion is that the whole plateau from roughly `k = 30` to `k = 100`
performs about equally well, and `k = 60` was simply the value carried
forward into every subsequent experiment ("fixed during a pilot
investigation and not altered during subsequent validation"), not a value
independently re-derived as optimal for each dataset. This project uses
`k = 60` as `fusion.rs`'s default for the same reason the paper gives —
a fixed, not-dataset-tuned constant — and states that reasoning here
rather than presenting `60` as a uniquely optimal number.

### What this paper does *not* claim, stated so it isn't assumed

RRF combines **rank positions only** — it is defined entirely in terms of
`r(d)`, never the underlying rankers' own scores (this is the paper's own
contrast with CombMNZ, §1: "RRF... combines ranks without regard to the
arbitrary scores returned by particular ranking methods"). It carries no
per-ranking weighting term and no normalization by `|D|` or by how many
rankings a document appears in — the formula above is the entire
mechanism. It is validated on TREC ad hoc/robust collections and the
LETOR 3 learning-to-rank benchmark (§1, Tables 2–3), not on a lexical/
vector hybrid-search workload specifically; this project cites it for the
formula and constant, not for a domain-specific effectiveness claim about
STRAND's own BM25-vs-ANN fusion.
