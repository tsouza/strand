# Bruch, Gai & Ingber (2023) — An Analysis of Fusion Functions for Hybrid Retrieval

Vendored excerpt, not the full paper. Source: Sebastian Bruch, Siyu Gai,
Amir Ingber, "An Analysis of Fusion Functions for Hybrid Retrieval," ACM
Transactions on Information Systems (TOIS), Vol. 42, Article 20 (2023),
DOI `10.1145/3596512`. arXiv preprint `2210.11934` (submitted 2022-10-21,
revised 2023-05-04), fetched 2026-08-20 from `arxiv.org/abs/2210.11934`
and `ar5iv.labs.arxiv.org/html/2210.11934`. Vendored because `CLAUDE.md`
§3 forbids implementing against a remembered spec, and because an earlier
draft of this idea's own design document already misattributed this
paper's evidence once (see the "what this paper does not show" section
below) — the primary source is fetched and cited directly here rather
than trusted from memory or from that earlier draft, and
`crates/strand-core/src/fusion.rs` cites this file directly.

Abstract (in full): "We study hybrid search in text retrieval where
lexical and semantic search are fused together with the intuition that
the two are complementary in how they model relevance. In particular, we
examine fusion by a convex combination (CC) of lexical and semantic
scores, as well as the Reciprocal Rank Fusion (RRF) method, and identify
their advantages and potential pitfalls. Contrary to existing studies, we
find RRF to be sensitive to its parameters; that the learning of a CC
fusion is generally agnostic to the choice of score normalization; that
CC outperforms RRF in in-domain and out-of-domain settings; and finally,
that CC is sample efficient, requiring only a small set of training
examples to tune its only parameter to a target domain."

### Equation 8: RRF with one constant per input ranking

For the two-system case the paper actually works with (one lexical
ranking, one semantic/vector ranking), Equation 8 rewrites the base RRF
formula with an independent constant per ranking rather than one shared
`k`:

> f_RRF(q, d) = 1 / (η_Lex + π_Lex(q, d)) + 1 / (η_Sem + π_Sem(q, d))

`π_Lex(q, d)` and `π_Sem(q, d)` are document `d`'s rank positions in the
lexical and semantic rankings for query `q` (the same `r(d)` notation
Cormack/Clarke/Büttcher use, see
`references/cormack-clarke-buettcher-2009-reciprocal-rank-fusion.md`);
`η_Lex` and `η_Sem` are independent tunable constants, in place of the
one shared `k` the base formula uses for every ranking. The paper writes
Equation 8 for exactly two rankings, since that is the hybrid lexical/
semantic setting the whole paper studies — it is not itself stated as an
N-ranking sum with one `η_i` per ranking `i`. STRAND's own
`reciprocal_rank_fusion_asymmetric` generalizes it to an arbitrary number
of rankings the same mechanical way `crates/strand-core/src/fusion.rs`'s
existing `reciprocal_rank_fusion` already generalizes the base one-`k`
formula beyond Cormack/Clarke/Büttcher's own worked pairwise examples:
by summing one `1/(η_i + rank_i(d))` term per input ranking, falling back
to Equation 8 exactly in the two-ranking case. That extension is this
project's own mechanical step, not a claim the paper itself makes for
more than two rankings.

### Table 3: the real asymmetric-RRF grid-search results

Grid-searching `(η_Lex, η_Sem)` per dataset, Table 3 reports (NDCG@1000):

| Dataset (regime)             | RRF baseline `(60, 60)` | RRF tuned                       | CC / TM2C2 (`α = 0.8`) |
|-------------------------------|--------------------------|----------------------------------|-------------------------|
| MS MARCO (in-domain)          | 0.425                    | 0.451 at `(η_Lex, η_Sem) = (10, 4)` | 0.454                   |
| HotpotQA (out-of-domain)      | 0.675                    | 0.693 at `(η_Lex, η_Sem) = (5, 5)`  | 0.699                   |

Two things follow directly from this table, and both matter for how
STRAND cites this paper:

1. **Tuned asymmetric RRF does beat symmetric `k = 60` RRF on both
   datasets shown** (0.451 > 0.425 on MS MARCO; 0.693 > 0.675 on
   HotpotQA) — asymmetric weighting is a real, measured improvement over
   the symmetric baseline when the per-ranking constants are grid-searched
   per dataset.
2. **Tuned asymmetric RRF still does not reach what the paper's own
   recommended alternative (CC / TM2C2, a convex combination of
   normalized scores) achieves on the same datasets** (0.451 < 0.454 on
   MS MARCO; 0.693 < 0.699 on HotpotQA) — even the best asymmetric RRF
   this paper measures underperforms the paper's own recommended,
   different, non-RRF fusion function.

### The domain-reversal finding — the real, usable reason to expose a per-ranking constant

The paper's own stated finding, and the reason this matters for a format
that cannot pick a default for every caller's corpus: "On MS MARCO, an
in-domain dataset, NDCG improves when η_Lex > η_Sem, while the opposite
effect can be seen for HotpotQA, an out-of-domain dataset" — the
grid-searched optimum is `(10, 4)` (lexical constant larger) on MS MARCO
but `(5, 5)` on HotpotQA, and the paper frames this as part of a broader
sensitivity finding: "while tuning a parametric RRF does lead to gains on
in-domain datasets, the tuned function does not generalize well to
out-of-domain datasets." No single `(η_Lex, η_Sem)` pair — including
equal weights, i.e. ordinary symmetric RRF — is shown to be safe across
both regimes in this data; which side should get the larger constant
depends on the corpus and evaluation setting, not on the formula alone.

### What this paper does *not* show, stated because an earlier draft got it wrong

An earlier draft of the design work behind this primitive cited this
paper's NDCG@1000 deltas (0.454 vs. 0.425) as evidence *for* asymmetric
RRF specifically. That is wrong: 0.454 is the CC/TM2C2 alternative's
score, compared against symmetric RRF's 0.425 — not asymmetric RRF's own
score, which Table 3 gives separately as 0.451, still below CC/TM2C2's
0.454. The paper's own recommendation is CC/TM2C2, not asymmetric RRF
("the convex combination formulation is theoretically sound, empirically
effective, sample-efficient, and robust to domain shift") — CC/TM2C2 is
out of scope for this primitive and is not implemented here; only the
narrower, honest finding (no single shared `k` is universally safe) is
what this project draws from this paper. The paper does not supply, and
this project does not claim, any default `(η_Lex, η_Sem)` (or `η_i` for
more than two rankings) safe to bake in for an arbitrary caller's corpus.
