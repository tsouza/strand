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
/// ranking, via Reciprocal Rank Fusion (see this module's doc comment for
/// the formula and `k`'s provenance). Each element of `rankings` is one
/// ranking: row-IDs in best-first order, exactly as `strand_lexical::
/// field::FieldReader::search_bm25_row_ids` or
/// `strand_vector::query::scan_selected_clusters` (translated to row-IDs)
/// would return them, index `0` being the best match. A row-ID's rank
/// position within a ranking is `1 + its index` — the paper's own
/// 1-based convention — and a row-ID absent from a ranking contributes no
/// term to its score from that ranking, exactly as the paper states.
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
pub fn reciprocal_rank_fusion(rankings: &[&[u64]], k: f64) -> Vec<FusedResult> {
    // row_id -> (score, one Option<rank> slot per input ranking).
    let mut by_row_id: HashMap<u64, (f64, Vec<Option<u32>>)> = HashMap::new();

    for (ranking_idx, ranking) in rankings.iter().enumerate() {
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
        assert!(score1 > score3, "doc 1 must strictly beat doc 3: {score1} vs {score3}");
        assert!(score3 > score2, "doc 3 must strictly beat doc 2: {score3} vs {score2}");

        let by_id: HashMap<u64, &FusedResult> = fused.iter().map(|f| (f.row_id, f)).collect();
        assert_close(by_id[&1].score, score1, "doc 1");
        assert_close(by_id[&2].score, score2, "doc 2");
        assert_close(by_id[&3].score, score3, "doc 3");

        assert_eq!(fused.iter().map(|f| f.row_id).collect::<Vec<_>>(), vec![1, 3, 2]);
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
}
