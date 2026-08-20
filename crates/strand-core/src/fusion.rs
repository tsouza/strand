// Copyright the STRAND authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Reciprocal Rank Fusion (RRF): the query-time glue that combines two
//! already-ranked, already-row-ID-resolved result lists — one from a
//! lexical field, one from a vector field, or any other number of ranked
//! lists over the same row-ID space (invariant 1) — into a single fused
//! ranking. `CLAUDE.md` §1 states plainly that "query-time fusion logic...
//! does not belong in the spec"; this module is exactly that: crate-level
//! glue, not a spec chapter, and it is deliberately independent of both
//! `strand-lexical` and `strand-vector` — it knows nothing about `TermInfo`
//! or `Candidate`, only about `u64` row-IDs and their rank positions,
//! which is all the fusion contract (invariant 1's row-ID space) actually
//! requires two blob families to agree on.
//!
//! The formula and the `k = 60` constant are taken from the paper that
//! defines RRF, not from memory (`CLAUDE.md` §3): G. V. Cormack, C. L. A.
//! Clarke, S. Büttcher, "Reciprocal Rank Fusion outperforms Condorcet and
//! individual Rank Learning Methods," SIGIR'09, vendored in
//! `references/cormack-clarke-buettcher-2009-reciprocal-rank-fusion.md`.
//! That paper's own formula (§1): given a set `D` of documents and a set
//! of rankings `R`, each a permutation on `1..|D|`,
//!
//! `RRFscore(d) = Σ_{r ∈ R} 1 / (k + r(d))`
//!
//! where `r(d)` is `d`'s **1-based** rank position within ranking `r`
//! (the paper's own "permutation on 1..|D|"), and a document absent from
//! a given ranking simply contributes no term for that ranking to the
//! sum. `k = 60` is the paper's own constant, "fixed during a pilot
//! investigation and not altered during subsequent validation" — the
//! vendored reference's own "k = 60, and why it's not 'critical'" section
//! records that the pilot sweep's actual MAP peak was at `k = 80`, with
//! `k = 60` close behind on a plateau from roughly `k = 30` to `k = 100`;
//! `DEFAULT_RRF_K` here is that same fixed, not-independently-re-tuned
//! constant, not a value this project claims to have found optimal for
//! its own workload.
//!
//! `reciprocal_rank_fusion` uses one shared `k` for every input ranking.
//! `reciprocal_rank_fusion_asymmetric` generalizes this to one constant
//! per ranking, grounded in a second real, vendored source: Sebastian
//! Bruch, Siyu Gai, Amir Ingber, "An Analysis of Fusion Functions for
//! Hybrid Retrieval," ACM Transactions on Information Systems (TOIS),
//! Vol. 42, Article 20 (2023), arXiv:2210.11934, vendored in
//! `references/bruch-gai-ingber-2023-fusion-functions-hybrid-retrieval.md`.
//! That paper's Equation 8 rewrites RRF for its two-ranking (lexical,
//! semantic) setting as `f(q,d) = 1/(eta_Lex + rank_Lex(q,d)) +
//! 1/(eta_Sem + rank_Sem(q,d))` — one constant per ranking instead of
//! one shared `k`; `reciprocal_rank_fusion_asymmetric` extends this to
//! an arbitrary number of rankings the same mechanical way
//! `reciprocal_rank_fusion` already extends the base one-`k` formula
//! beyond the original paper's own pairwise examples, summing one
//! `1/(k_i + rank_i(d))` term per input ranking.
//!
//! This primitive is exposed for a real, narrow, honestly-stated reason,
//! not because asymmetric weighting is shown to be generally better: the
//! vendored reference's own Table 3 grid search finds the *optimal*
//! per-ranking constants reverse which side is larger between an
//! in-domain and an out-of-domain evaluation setting (`(eta_Lex, eta_Sem)
//! = (10, 4)` on MS MARCO vs. `(5, 5)` on HotpotQA) — so no single
//! default asymmetric weighting, including one tuned for this project's
//! own workload, is safe to bake in as a new default. A caller who wants
//! to tune per-ranking constants for their own corpus can; this module
//! does not choose the constants for them. (An earlier draft of this
//! idea's design work misread that same table's NDCG@1000 deltas as
//! evidence asymmetric RRF beats symmetric RRF — the vendored
//! reference's own "what this paper does not show" section corrects
//! that: those deltas are the paper's *recommended*, different, non-RRF
//! alternative compared against symmetric RRF, and the paper's own best
//! grid-searched asymmetric RRF result still trails that alternative.)

use std::collections::{HashMap, HashSet};

/// RRF's own constant, `k = 60`
/// (`references/cormack-clarke-buettcher-2009-reciprocal-rank-fusion.md`
/// §1, Table 1) — see this module's doc comment for what "fixed" means
/// here: not independently tuned for STRAND's own workload, carried over
/// from the paper's own pilot investigation.
pub const DEFAULT_RRF_K: f64 = 60.0;

/// One row-ID's fused RRF score, together with which rankings it survived
/// from and its rank position in each — kept for callers that want to
/// explain a fused result, not just consume it. `ranks` is parallel to
/// the `rankings` slice `reciprocal_rank_fusion` was called with:
/// `ranks[i]` is this row-ID's `Some(1-based rank)` in `rankings[i]`, or
/// `None` if it did not appear there at all.
#[derive(Debug, Clone, PartialEq)]
pub struct FusedResult {
    pub row_id: u64,
    pub score: f64,
    pub ranks: Vec<Option<u32>>,
}

/// Fuses any number of already-ranked, best-first row-ID lists into one
/// ranking, via Reciprocal Rank Fusion with one constant per ranking (see
/// this module's doc comment for the generalized formula and its
/// provenance — Bruch/Gai/Ingber's Equation 8, extended here from their
/// two-ranking case to an arbitrary number of rankings). Each element of
/// `rankings` is one ranking: row-IDs in best-first order, exactly as
/// `strand_lexical::field::FieldReader::search_bm25_row_ids` or
/// `strand_vector::query::scan_selected_clusters` (translated to row-IDs)
/// would return them, index `0` being the best match. A row-ID's rank
/// position within a ranking is `1 + its index` — the paper's own
/// 1-based convention — and a row-ID absent from a ranking contributes no
/// term to its score from that ranking, exactly as the paper states.
///
/// `ks[i]` is the constant used for `rankings[i]`. `ks.len()` MUST equal
/// `rankings.len()` — one constant per ranking, not a shared default for
/// the rest — and this is checked with a real `assert_eq!`, not a silent
/// zip-truncation: silently dropping or ignoring rankings because the two
/// slices disagree in length is a real correctness footgun (a caller who
/// resizes one list without the other should get a loud panic, not a
/// quietly wrong fused ranking).
///
/// A ranking listing the same row-ID more than once is a caller error
/// this function does not itself detect (every real producer in this
/// project — `search_bm25`'s per-term postings, `scan_selected_clusters`'
/// own row-ID deduplication — already yields each row-ID at most once
/// per ranking, so this is not a real path, only a documented
/// precondition): the row-ID's *first* occurrence's rank is the one
/// counted; a later duplicate is ignored rather than double-counted,
/// so one ranking can never contribute more than one term to a given
/// row-ID's score.
///
/// Returns every row-ID that appeared in at least one ranking, sorted by
/// descending `score`. Ties (identical `score`, possible when two row-IDs
/// happen to land on the same rank positions across the same rankings)
/// are broken by ascending row-ID, so the result is fully deterministic
/// regardless of hash-map iteration order — this is fusion output, not a
/// wire structure invariant 11 governs, but determinism is still worth
/// having for a reproducible test and a reproducible query result.
pub fn reciprocal_rank_fusion_asymmetric(rankings: &[&[u64]], ks: &[f64]) -> Vec<FusedResult> {
    assert_eq!(
        ks.len(),
        rankings.len(),
        "reciprocal_rank_fusion_asymmetric: ks.len() ({}) must equal rankings.len() ({}) \
         — one constant per ranking, not a default for the rest",
        ks.len(),
        rankings.len()
    );

    // row_id -> (score, one Option<rank> slot per input ranking).
    let mut by_row_id: HashMap<u64, (f64, Vec<Option<u32>>)> = HashMap::new();

    for (ranking_idx, ranking) in rankings.iter().enumerate() {
        let k = ks[ranking_idx];
        let mut seen_in_this_ranking: HashSet<u64> = HashSet::new();
        for (i, &row_id) in ranking.iter().enumerate() {
            if !seen_in_this_ranking.insert(row_id) {
                // Duplicate within one ranking: first occurrence already
                // counted, per this function's own documented precondition.
                continue;
            }
            let rank = (i + 1) as u32; // 1-based, per the paper's own convention.
            let entry = by_row_id
                .entry(row_id)
                .or_insert_with(|| (0.0, vec![None; rankings.len()]));
            entry.0 += 1.0 / (k + rank as f64);
            entry.1[ranking_idx] = Some(rank);
        }
    }

    let mut fused: Vec<FusedResult> = by_row_id
        .into_iter()
        .map(|(row_id, (score, ranks))| FusedResult {
            row_id,
            score,
            ranks,
        })
        .collect();
    fused.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.row_id.cmp(&b.row_id)));
    fused
}

/// `reciprocal_rank_fusion_asymmetric` with the same shared constant `k`
/// for every ranking — Cormack/Clarke/Büttcher's original, symmetric RRF
/// formula (see this module's doc comment). A thin wrapper: builds
/// `rankings.len()` copies of `k` and delegates.
pub fn reciprocal_rank_fusion(rankings: &[&[u64]], k: f64) -> Vec<FusedResult> {
    let ks = vec![k; rankings.len()];
    reciprocal_rank_fusion_asymmetric(rankings, &ks)
}

/// `reciprocal_rank_fusion` with `k = DEFAULT_RRF_K` — the ordinary entry
/// point for a caller with no reason to deviate from the paper's own
/// fixed constant.
pub fn fuse(rankings: &[&[u64]]) -> Vec<FusedResult> {
    reciprocal_rank_fusion(rankings, DEFAULT_RRF_K)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64, label: &str) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "{label}: expected {expected}, got {actual}"
        );
    }

    /// A document ranked first in one ranking and absent from a second
    /// scores exactly `1/(k+1)` — a direct instance of the paper's own
    /// formula with `|R| = 1` contributing term.
    #[test]
    fn a_single_ranking_first_place_matches_the_formula_by_hand() {
        let ranking_a: &[u64] = &[42, 7, 9];
        let ranking_b: &[u64] = &[];
        let fused = reciprocal_rank_fusion(&[ranking_a, ranking_b], 60.0);
        let row42 = fused.iter().find(|f| f.row_id == 42).unwrap();
        assert_close(row42.score, 1.0 / 61.0, "row 42's score");
        assert_eq!(row42.ranks, vec![Some(1), None]);
    }

    /// A document ranked first in both of two rankings sums both
    /// contributions: `1/(k+1) + 1/(k+1) = 2/(k+1)`.
    #[test]
    fn a_document_ranked_first_in_two_rankings_sums_both_terms() {
        let ranking_a: &[u64] = &[100];
        let ranking_b: &[u64] = &[100];
        let fused = reciprocal_rank_fusion(&[ranking_a, ranking_b], 60.0);
        assert_eq!(fused.len(), 1);
        assert_close(fused[0].score, 2.0 / 61.0, "row 100's score");
        assert_eq!(fused[0].ranks, vec![Some(1), Some(1)]);
    }

    /// A fully hand-computed three-document, two-ranking example, `k =
    /// 60`: ranking A = [1, 2, 3], ranking B = [3, 1, 2]. Doc 1: rank 1 in
    /// A (1/61), rank 2 in B (1/62). Doc 2: rank 2 in A (1/62), rank 3 in
    /// B (1/63). Doc 3: rank 3 in A (1/63), rank 1 in B (1/61). This is a
    /// cyclic permutation, not a symmetric swap, so the three scores are
    /// in fact all distinct, not two of them tied: doc 1 sums the two
    /// largest terms (1/61 + 1/62), doc 3 sums the largest and the
    /// smallest (1/61 + 1/63), and doc 2 sums the two smallest (1/62 +
    /// 1/63) — so doc 1 > doc 3 > doc 2, confirmed both by direct
    /// arithmetic below and by an independent decimal check
    /// (`1/61+1/62 ≈ 0.0325225`, `1/61+1/63 ≈ 0.0322665`, `1/62+1/63 ≈
    /// 0.0320020`) before this test trusted an earlier, wrong claim that
    /// doc 1 and doc 3 tie by "symmetry" — they don't, and this comment
    /// is corrected rather than the wrong claim merely dropped, per
    /// `CLAUDE.md` §2's "a number without a vendored source is deleted,
    /// not softened" spirit applied to a wrong derivation as much as a
    /// wrong figure.
    #[test]
    fn a_hand_computed_three_document_two_ranking_example() {
        let ranking_a: &[u64] = &[1, 2, 3];
        let ranking_b: &[u64] = &[3, 1, 2];
        let fused = reciprocal_rank_fusion(&[ranking_a, ranking_b], 60.0);

        let score1 = 1.0 / 61.0 + 1.0 / 62.0;
        let score2 = 1.0 / 62.0 + 1.0 / 63.0;
        let score3 = 1.0 / 63.0 + 1.0 / 61.0;
        assert!(
            score1 > score3,
            "doc 1 must strictly beat doc 3: {score1} vs {score3}"
        );
        assert!(
            score3 > score2,
            "doc 3 must strictly beat doc 2: {score3} vs {score2}"
        );

        let by_id: HashMap<u64, &FusedResult> = fused.iter().map(|f| (f.row_id, f)).collect();
        assert_close(by_id[&1].score, score1, "doc 1");
        assert_close(by_id[&2].score, score2, "doc 2");
        assert_close(by_id[&3].score, score3, "doc 3");

        assert_eq!(
            fused.iter().map(|f| f.row_id).collect::<Vec<_>>(),
            vec![1, 3, 2]
        );
    }

    #[test]
    fn empty_rankings_produce_an_empty_fusion() {
        let empty: &[u64] = &[];
        assert!(reciprocal_rank_fusion(&[empty, empty], 60.0).is_empty());
        assert!(reciprocal_rank_fusion(&[], 60.0).is_empty());
    }

    /// A row-ID appearing twice in the same ranking counts once, at its
    /// first (better) rank — this function's own documented precondition
    /// exercised directly, not left unverified.
    #[test]
    fn a_duplicate_within_one_ranking_counts_only_its_first_occurrence() {
        let ranking: &[u64] = &[5, 5, 9];
        let fused = reciprocal_rank_fusion(&[ranking], 60.0);
        let row5 = fused.iter().find(|f| f.row_id == 5).unwrap();
        assert_close(row5.score, 1.0 / 61.0, "row 5 counted once, at rank 1");
        assert_eq!(row5.ranks, vec![Some(1)]);
    }

    #[test]
    fn fuse_uses_the_papers_default_k_of_60() {
        let ranking: &[u64] = &[1];
        let via_fuse = fuse(&[ranking]);
        let via_explicit = reciprocal_rank_fusion(&[ranking], DEFAULT_RRF_K);
        assert_eq!(via_fuse, via_explicit);
        assert_close(DEFAULT_RRF_K, 60.0, "DEFAULT_RRF_K");
    }

    /// Within a single ranking (no other ranking contributing), score is
    /// strictly decreasing in rank position — `1/(k+r)` is a strictly
    /// decreasing function of `r` for any fixed `k > 0`, so a longer
    /// single ranking must come back in exactly the order it went in,
    /// never reordered or tied. General, not example-specific: exercised
    /// over a run of twenty distinct row-IDs.
    #[test]
    fn a_single_rankings_own_order_is_always_preserved_and_strictly_decreasing() {
        let ranking: Vec<u64> = (100..120).collect();
        let ranking_ref: &[u64] = &ranking;
        let fused = fuse(&[ranking_ref]);
        assert_eq!(fused.iter().map(|f| f.row_id).collect::<Vec<_>>(), ranking);
        for w in fused.windows(2) {
            assert!(
                w[0].score > w[1].score,
                "score must strictly decrease along the ranking: {w:?}"
            );
        }
    }

    /// Result order does not depend on `HashMap` iteration order: running
    /// fusion many times over the same input always yields the same
    /// output order (the sort's own tie-break makes this true even when
    /// scores collide).
    #[test]
    fn fusion_output_order_is_deterministic_across_repeated_runs() {
        let ranking_a: &[u64] = &[10, 20, 30, 40, 50];
        let ranking_b: &[u64] = &[50, 40, 30, 20, 10];
        let first = fuse(&[ranking_a, ranking_b]);
        for _ in 0..20 {
            assert_eq!(fuse(&[ranking_a, ranking_b]), first);
        }
    }

    /// `reciprocal_rank_fusion(rankings, k)` must be exactly
    /// `reciprocal_rank_fusion_asymmetric(rankings, &[k; rankings.len()])`
    /// — the refactor that made the symmetric function a thin wrapper is
    /// behavior-preserving, checked directly rather than only inferred
    /// from the other, unchanged tests in this module still passing.
    #[test]
    fn reciprocal_rank_fusion_delegates_to_the_asymmetric_function_with_equal_constants() {
        let ranking_a: &[u64] = &[1, 2, 3];
        let ranking_b: &[u64] = &[3, 1, 2];
        let via_symmetric = reciprocal_rank_fusion(&[ranking_a, ranking_b], 42.0);
        let via_asymmetric =
            reciprocal_rank_fusion_asymmetric(&[ranking_a, ranking_b], &[42.0, 42.0]);
        assert_eq!(via_symmetric, via_asymmetric);
    }

    /// `ks.len()` disagreeing with `rankings.len()` is a real correctness
    /// footgun (this module's doc comment on
    /// `reciprocal_rank_fusion_asymmetric`) and must panic loudly rather
    /// than silently truncate to the shorter length.
    #[test]
    #[should_panic(expected = "ks.len()")]
    fn reciprocal_rank_fusion_asymmetric_panics_when_ks_length_does_not_match_rankings_length() {
        let ranking_a: &[u64] = &[1, 2];
        let ranking_b: &[u64] = &[3, 4];
        reciprocal_rank_fusion_asymmetric(&[ranking_a, ranking_b], &[60.0]);
    }

    /// The worked example from `crates/strand-core/tests/
    /// hybrid_rrf_end_to_end.rs`'s own six-row hybrid scenario, reused
    /// here as literal row-ID rankings (that test independently asserts
    /// these are the real row-IDs `search_bm25_row_ids` and the real
    /// reranked ANN scan actually produce for its fixture, with
    /// `row_id_base = 0`): the lexical ranking is `[0, 1]` (only rows 0
    /// and 1 ever match the term `"widget"`, row 0 first for its shorter
    /// document length) and the vector ranking is `[3, 2, 0, 4, 1, 5]`
    /// (exact L2 order from an all-zero query, nearest first).
    ///
    /// At the paper's own default, `k = 60` for both rankings, that
    /// test's own hand computation already establishes the fused order
    /// `[0, 1, 3, 2, 4, 5]`: row 0 and row 1 — each mediocre on the
    /// vector side alone — outrank row 3, the vector ranking's actual
    /// nearest neighbor, because they also match the lexical ranking.
    ///
    /// This test picks one illustrative asymmetric weighting, `(k_lex,
    /// k_vec) = (60, 5)` — a smaller constant weights the vector
    /// ranking's top positions more heavily, per Equation 8 (this
    /// module's doc comment; `references/bruch-gai-ingber-2023-fusion-
    /// functions-hybrid-retrieval.md`) — labeled "Variant B" only to
    /// distinguish it from the untried "Variant A" this exploration also
    /// considered, and stated plainly as illustrative, not a recommended
    /// default (this module's doc comment explains why no default is
    /// shipped). Under this weighting, row 3 — a genuine cross-vocabulary
    /// match: the vector ranking's best result, with no lexical match at
    /// all — overtakes both row 0 and row 1, which beat it under today's
    /// symmetric `k = 60`. Every score below is asserted exactly against
    /// the formula applied by hand, independent of the implementation.
    #[test]
    fn asymmetric_rrf_variant_b_lets_the_vector_only_match_overtake_lexical_leaning_rows() {
        let lexical: &[u64] = &[0, 1];
        let vector: &[u64] = &[3, 2, 0, 4, 1, 5];
        let rankings: [&[u64]; 2] = [lexical, vector];

        // Sanity: reproduce hybrid_rrf_end_to_end.rs's own symmetric
        // k=60 result first, so the "overtake" below is measured against
        // a real, independently-established baseline, not assumed.
        let symmetric = reciprocal_rank_fusion(&rankings, 60.0);
        assert_eq!(
            symmetric.iter().map(|f| f.row_id).collect::<Vec<_>>(),
            vec![0, 1, 3, 2, 4, 5],
            "symmetric k=60 baseline must match hybrid_rrf_end_to_end.rs's own hand computation"
        );

        let asymmetric = reciprocal_rank_fusion_asymmetric(&rankings, &[60.0, 5.0]);

        let expected_scores: Vec<(u64, f64)> = vec![
            (3, 1.0 / 6.0),               // vector rank 1 only, k_vec=5
            (2, 1.0 / 7.0),               // vector rank 2 only, k_vec=5
            (0, 1.0 / 61.0 + 1.0 / 8.0),  // lexical rank 1 (k=60) + vector rank 3 (k=5)
            (1, 1.0 / 62.0 + 1.0 / 10.0), // lexical rank 2 (k=60) + vector rank 5 (k=5)
            (4, 1.0 / 9.0),               // vector rank 4 only, k_vec=5
            (5, 1.0 / 11.0),              // vector rank 6 only, k_vec=5
        ];
        for w in expected_scores.windows(2) {
            assert!(
                w[0].1 > w[1].1,
                "expected_scores must be strictly descending: {expected_scores:?}"
            );
        }
        assert_eq!(
            asymmetric.iter().map(|f| f.row_id).collect::<Vec<_>>(),
            expected_scores
                .iter()
                .map(|&(id, _)| id)
                .collect::<Vec<_>>(),
            "asymmetric (k_lex=60, k_vec=5) order must match the hand-computed expectation: {asymmetric:?}"
        );
        for (entry, &(expected_id, expected_score)) in asymmetric.iter().zip(expected_scores.iter())
        {
            assert_eq!(entry.row_id, expected_id);
            assert_close(
                entry.score,
                expected_score,
                &format!("row {expected_id}'s asymmetric score"),
            );
        }

        // The real thesis assertion: row 3 (best vector-only match, no
        // lexical match at all) now overtakes both row 0 and row 1,
        // which beat it under symmetric k=60 above.
        let position_of = |row_id: u64| asymmetric.iter().position(|f| f.row_id == row_id).unwrap();
        assert!(
            position_of(3) < position_of(0),
            "row 3 must overtake row 0 under asymmetric weighting: {asymmetric:?}"
        );
        assert!(
            position_of(3) < position_of(1),
            "row 3 must overtake row 1 under asymmetric weighting: {asymmetric:?}"
        );
    }
}
