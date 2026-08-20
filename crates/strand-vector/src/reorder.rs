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

//! Starling's (Wang et al., SIGMOD 2024) block-shuffling node-order
//! permutation algorithms, transcribed against the real, vendored
//! pseudocode in `references/starling-sigmod2024.md`, not reconstructed
//! from memory (`CLAUDE.md` §3). This is pure, in-memory, testable Rust
//! operating on `crate::vamana::Graph` — task 1 of this implementation
//! sequence's output shape — with no wire format and no I/O, the same
//! discipline `crate::vamana`'s own module documentation established as
//! this crate's precedent. Wire layout (`rfcs/0014-graph-blob-family.md`
//! Design §§2–3's node-record and permutation-directory blobs) and the
//! query path over that layout are deliberately out of scope here —
//! separate, later tasks in this same sequence.
//!
//! **What is implemented.** The overlap-ratio locality metric `OR(G)`
//! (§4.1, Definition 1 and Eq. 5), BNP (Algorithm I, Block Neighbor
//! Padding), and BNF (Algorithm II, Block Neighbor Frequency — RFC 0014's
//! registered default, `shuffle_algorithm = 2`), the latter transcribed
//! directly from the reference file's own verbatim Algorithm 1 pseudocode.
//!
//! **What is not: BNS (Algorithm III, Block Neighbor Swap), and why,
//! stated plainly rather than approximated.** `references/
//! starling-sigmod2024.md`'s own BNS section is prose, not pseudocode: "a
//! pair of neighboring vertices in different blocks with low overlap
//! ratio" names no concrete selection rule (low overlap ratio of what,
//! measured how, chosen among how many candidate pairs), no iteration
//! order, and no termination condition beyond the stated complexity class
//! `O(β · o³ · ε · |V|)` — a formula that itself implies a specific
//! candidate-enumeration shape (cubic in average out-degree `o`) the
//! vendored text never spells out. Guessing a mechanism that merely
//! produces the right complexity class and a Lemma-4.2-consistent
//! monotonic `OR(G)` would be inventing an algorithm and presenting it as
//! Starling's, exactly what this task's own brief and `CLAUDE.md` §3
//! forbid. BNP and BNF are this RFC's load-bearing pieces — the simple
//! baseline and the registered default, respectively — so this is a
//! genuine, documented gap, not a shortfall against the task's real
//! requirement.
//!
//! **Three documented interpretation choices for real ambiguities in the
//! vendored BNF pseudocode, named rather than silently resolved.**
//! **First**, tie-breaking: "`x ← block ID with the most neighbors in H`"
//! does not state a tie-break rule when two or more block IDs in `H` share
//! the same neighbor frequency. This module breaks ties toward the lower
//! block ID, for the same deterministic-given-the-same-input reason
//! `crate::vamana::robust_prune`'s own module documentation gives for its
//! `BTreeSet`-based tie-break (ascending-index order, not a randomized
//! hasher's iteration order). **Second**, the fallback branch: "`if H = ∅
//! then add u to an empty block in B`" reads literally as "a block with
//! zero occupancy," but no invariant in the algorithm's own text guarantees
//! a fully empty block still exists whenever this branch fires — only that
//! `ρ · ε ≥ |V|` guarantees *some* block still has spare room. Requiring a
//! literally empty block risks a fallback that cannot always succeed; this
//! module instead falls back to the first block (ascending ID) with any
//! spare capacity, which is always available under that same `ρ · ε ≥ |V|`
//! guarantee and keeps every call to [`bnf`] total (never panicking on a
//! graph this module itself can otherwise place).
//!
//! **Third, found empirically, not anticipated going in: a single BNF
//! iteration can *decrease* `OR(G)` relative to its own BNP starting
//! layout, and this is real, not a transcription bug.** The paper's own
//! text says so directly: "BNF's efficiency is contingent on the number of
//! iterations and does not ensure the convergence of `OR(G)`" — unlike BNS,
//! which Lemma 4.2 proves monotonically non-decreasing. Running this
//! module's own literal transcription of Algorithm 1 against a real
//! `crate::vamana::build_vamana` graph (`tests::
//! bnf_beats_bnp_beats_naive_on_a_real_vamana_graph`'s own scale, `n=500`,
//! `block_size=16`) reproduced exactly this: the first iteration alone drops
//! `OR(G)` from BNP's `0.1401` to `0.0841`, and the pseudocode's own literal
//! last line ("return new layout of G") would return that worse layout
//! whenever the configured stopping rule halts there. This is a real gap
//! the vendored text leaves open (it names the *possibility* of
//! non-convergence but never says what a caller should do about it), not a
//! mechanical transcription step, so [`bnf`] closes it with a documented,
//! low-cost engineering choice: track the best `OR(G)` layout seen across
//! BNP's own starting point and every BNF iteration, and return that one
//! rather than unconditionally the last. This makes `bnf(...)`'s own result
//! a guaranteed lower bound of `bnp(...)`'s on every input, not a
//! probabilistic "usually better" — every intermediate per-iteration
//! transition still follows Algorithm 1's own rule exactly and unmodified;
//! only the final return value departs from a literal reading of the
//! pseudocode's own last line. (A second hypothesis was tested and set
//! aside before adopting this fix: visiting vertices in block-major rather
//! than ascending-ID order within each iteration — unspecified by
//! Algorithm 1's own "forall u ∈ V do" — also improved the measured result
//! in exploratory testing, but best-`OR(G)` tracking is the simpler fix
//! that is provably safe on every input rather than empirically better on
//! the cases tried, so it is the one shipped here.)

use crate::vamana::Graph;
use std::collections::{HashMap, HashSet};

/// A node-order permutation: a dense mapping from **logical node index**
/// (`0..n`, `crate::vamana::Graph`'s own point-array-index convention) to
/// **physical slot index** (also `0..n`) — the same job
/// `rfcs/0014-graph-blob-family.md` Design §3's node-order permutation
/// directory blob eventually persists (`physical_slot =
/// permutation[local_ordinal]`), computed here as pure in-memory data, not
/// yet serialized to that blob's wire layout (a later task).
///
/// Every conforming permutation returned by this module's own functions is
/// a genuine bijection over `0..n`: every logical index maps to exactly one
/// physical slot, and every physical slot is used exactly once. Tested
/// explicitly (this module's own `tests::bijection` group) because the
/// eventual wire-format blob's own correctness depends on it completely.
pub type Permutation = Vec<usize>;

/// Starling's overlap-ratio locality metric `OR(G)` (§4.1, Definition 1 and
/// Eq. 5, `references/starling-sigmod2024.md`): the mean, over every node
/// `u`, of `OR(u) = |B(u) ∩ N(u)| / (|B(u)| − 1)` when `u`'s physical block
/// `B(u)` holds more than one vertex, else `0` — where `N(u)` is `u`'s
/// out-neighbor set (`graph[u]`, this crate's own adjacency convention,
/// matching the paper's own per-vertex "neighbor-ID list" that Definition 1
/// itself sizes `B(u)` and `N(u)` against) and `B(u)` is the set of nodes
/// sharing `u`'s physical block under `permutation` — Definition 1's own
/// block-level layout: `block_size` (`ε` in the paper) contiguous physical
/// slots per block, `⌈n / block_size⌉` blocks total. A layout with optimal
/// data locality has `OR(G) = 1` (verbatim, §4.1); the paper's own measured
/// naive/ID-contiguous baseline is `OR(G) ≈ 0` (§4.1, confirmed again in
/// §3.1's "up to 94% of data read from the disk is wasted").
///
/// # Panics
///
/// Panics if `block_size == 0`, or if `permutation.len() != graph.len()`.
pub fn overlap_ratio(graph: &Graph, permutation: &[usize], block_size: usize) -> f64 {
    assert!(block_size > 0, "block_size must be nonzero");
    let n = graph.len();
    assert_eq!(
        permutation.len(),
        n,
        "permutation must cover every node in graph"
    );
    if n == 0 {
        return 0.0;
    }

    let num_blocks = n.div_ceil(block_size);
    let mut block_members: Vec<Vec<usize>> = vec![Vec::new(); num_blocks];
    for (node, &slot) in permutation.iter().enumerate() {
        block_members[slot / block_size].push(node);
    }
    let block_sets: Vec<HashSet<usize>> = block_members
        .iter()
        .map(|members| members.iter().copied().collect())
        .collect();

    let mut total = 0.0f64;
    for node in 0..n {
        let block = permutation[node] / block_size;
        let block_len = block_members[block].len();
        if block_len <= 1 {
            continue; // OR(u) = 0 by Definition 1's own |B(u)| > 1 guard.
        }
        let overlap = graph[node]
            .iter()
            .filter(|neighbor| block_sets[block].contains(neighbor))
            .count();
        total += overlap as f64 / (block_len - 1) as f64;
    }
    total / n as f64
}

/// Flattens a block-contents layout (block index → member node indices, in
/// within-block placement order) into a dense [`Permutation`]: physical
/// slot `k` is occupied by the `k`-th node encountered scanning blocks in
/// ascending block-index order, and within each block, in that block's own
/// stored (insertion) order.
///
/// # Panics
///
/// Panics (via `debug_assert`) if `blocks`' total member count isn't
/// exactly `n` — every node placed exactly once — which every constructor
/// in this module (`bnp`, `bnf`) guarantees before calling this.
fn permutation_from_blocks(blocks: &[Vec<usize>], n: usize) -> Permutation {
    let mut permutation = vec![usize::MAX; n];
    let mut slot = 0usize;
    for block in blocks {
        for &node in block {
            permutation[node] = slot;
            slot += 1;
        }
    }
    debug_assert_eq!(slot, n, "every node must be placed in exactly one block");
    debug_assert!(
        permutation.iter().all(|&s| s != usize::MAX),
        "every node must have been assigned a slot"
    );
    permutation
}

/// Starling's Algorithm I, Block Neighbor Padding (BNP,
/// `references/starling-sigmod2024.md` §4.1): "fills blocks one at a time
/// in ascending vertex-ID order: assign the next unassigned vertex and (as
/// many of) its neighbors as fit to the current block; open a new block
/// once full." `O(|V|)` — a single pass (bounded per-node work by the
/// graph's own degree bound, so `O(|V| + |E|)` in full, linear in this
/// family's fixed-out-degree graphs). The reference file itself gives this
/// algorithm as prose, not pseudocode (unlike BNF's own Algorithm 1
/// transcription below); this is a direct, literal implementation of that
/// prose with no invented mechanism.
///
/// Registered in `rfcs/0014-graph-blob-family.md` Design §4 as
/// `shuffle_algorithm = 1`: "the weakest locality gain" of the three, "a
/// single ID-order pass with no iterative refinement" — also BNF's own
/// required starting layout, below.
///
/// # Panics
///
/// Panics if `block_size == 0`.
pub fn bnp(graph: &Graph, block_size: usize) -> Permutation {
    assert!(block_size > 0, "block_size must be nonzero");
    let n = graph.len();

    let mut assigned = vec![false; n];
    let mut blocks: Vec<Vec<usize>> = vec![Vec::with_capacity(block_size)];

    for u in 0..n {
        if assigned[u] {
            continue;
        }
        if blocks.last().expect("blocks always has >= 1 block").len() == block_size {
            blocks.push(Vec::with_capacity(block_size));
        }
        blocks.last_mut().expect("just ensured room").push(u);
        assigned[u] = true;

        for &neighbor in &graph[u] {
            if assigned[neighbor] {
                continue;
            }
            if blocks.last().expect("current block exists").len() == block_size {
                break; // Current block full; the next unassigned vertex in
                // the outer loop opens a new one -- "open a new block once
                // full."
            }
            blocks.last_mut().expect("just checked room").push(neighbor);
            assigned[neighbor] = true;
        }
    }

    permutation_from_blocks(&blocks, n)
}

/// [`bnf`]'s own tuning inputs — Starling's `β` (max iterations) and `τ`
/// (`OR(G)`-gain early-stop threshold), Algorithm 1's own two named inputs
/// beyond the block-level layout itself (`references/starling-
/// sigmod2024.md` §4.1). Construction-time writer tuning inputs, out of
/// scope for wire-format normativity per `rfcs/0014-graph-blob-family.md`
/// Non-goals, the same discipline that RFC applies to Vamana's own `R`,
/// `L`, `α`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BnfConfig {
    /// Starling's `ε`: nodes per physical block.
    pub block_size: usize,
    /// Starling's `β`: the maximum number of shuffling iterations. At least
    /// one iteration always runs regardless of this value (the paper's own
    /// "while iterations ≤ β" runs its body before any check can fire).
    pub beta: usize,
    /// Starling's `τ`: the loop stops early once one iteration's `OR(G)`
    /// gain over the previous iteration falls below this threshold.
    pub tau: f64,
}

/// Starling's Algorithm II, Block Neighbor Frequency (BNF,
/// `references/starling-sigmod2024.md` §4.1, Algorithm 1's own pseudocode,
/// transcribed directly): starting from [`bnp`]'s own block-level layout,
/// each iteration clears every block and reassigns every vertex `u` (in
/// ascending ID order) to whichever *not-yet-full* block currently holds
/// the most of `u`'s neighbors under the **previous** iteration's mapping
/// `D` — falling back to the first block with spare capacity if every
/// candidate block `u`'s neighbors touch is already full (or `u` has no
/// neighbors at all). Repeats until `config.beta` iterations have run or
/// one iteration's `OR(G)` gain over the last falls below `config.tau`.
/// `O(β · o · |V|)`, linear in average out-degree `o` — the paper's own
/// stated reason for recommending BNF as "an adept balance between
/// efficiency and effectiveness" against BNP (weaker) and BNS (cubic in
/// `o`, not implemented here — see this module's own top-level
/// documentation).
///
/// Registered in `rfcs/0014-graph-blob-family.md` Design §4 as
/// `shuffle_algorithm = 2`, v0.1's **default**.
///
/// See this module's top-level documentation for the three real, documented
/// interpretation choices this transcription makes for the vendored
/// pseudocode's own real ambiguities and gaps (tie-break order; the "empty
/// block" fallback's exact reading; returning the best `OR(G)` layout seen
/// rather than unconditionally the last, since a single iteration can
/// measurably decrease `OR(G)`, a real finding, not a bug).
///
/// # Panics
///
/// Panics if `config.block_size == 0`.
pub fn bnf(graph: &Graph, config: &BnfConfig) -> Permutation {
    assert!(config.block_size > 0, "block_size must be nonzero");
    let n = graph.len();
    let block_size = config.block_size;
    let rho = n.div_ceil(block_size.max(1)).max(1);

    // Algorithm 1's own required input: "block-level layout from BNP."
    let initial_permutation = bnp(graph, block_size);
    let mut d: Vec<usize> = initial_permutation
        .iter()
        .map(|&slot| slot / block_size)
        .collect();
    let mut prev_or = overlap_ratio(graph, &initial_permutation, block_size);
    let mut last_blocks = blocks_from_assignment(&d, rho, n);

    // Third documented interpretation choice -- see this module's own
    // top-level documentation for the full account of why a single BNF
    // iteration can decrease OR(G) and why this function tracks the best
    // layout seen rather than unconditionally returning the last one.
    let mut best_blocks = last_blocks.clone();
    let mut best_or = prev_or;

    let iterations = config.beta.max(1);
    for _ in 0..iterations {
        let new_blocks = bnf_one_iteration(graph, &d, rho, block_size);
        for (b, members) in new_blocks.iter().enumerate() {
            for &u in members {
                d[u] = b;
            }
        }
        last_blocks = new_blocks;

        let permutation = permutation_from_blocks(&last_blocks, n);
        let or = overlap_ratio(graph, &permutation, block_size);
        if or > best_or {
            best_or = or;
            best_blocks = last_blocks.clone();
        }
        let gain = or - prev_or;
        prev_or = or;
        if gain < config.tau {
            break;
        }
    }

    permutation_from_blocks(&best_blocks, n)
}

/// One raw transition of Starling's Algorithm 1 (BNF): given the *previous*
/// iteration's block assignment `d` (node → block ID), produces the next
/// iteration's block contents by reassigning every node in ascending ID
/// order to whichever not-yet-full block currently holds the most of its
/// neighbors under `d` — exactly Algorithm 1's own per-iteration body, and
/// nothing else. Extracted out of [`bnf`] itself so this literal mechanism
/// is independently hand-checkable (this module's own
/// `hand_check_bnf_one_iteration_matches_algorithm_1s_own_mechanism` test)
/// separately from [`bnf`]'s own best-seen-tracking wrapper, which may
/// choose to discard this iteration's result in favor of an earlier,
/// better-scoring one (see [`bnf`]'s own documentation for why).
fn bnf_one_iteration(graph: &Graph, d: &[usize], rho: usize, block_size: usize) -> Vec<Vec<usize>> {
    let mut new_blocks: Vec<Vec<usize>> = vec![Vec::new(); rho];

    for (u, neighbors) in graph.iter().enumerate() {
        // H <- union of {D(a)} for a in N(u) -- Algorithm 1's own line,
        // computed here together with each candidate block's neighbor
        // frequency (the count Algorithm 1's own next line, "x <- block
        // ID with the most neighbors in H," picks the max of). The
        // frequency count is fixed for this pass of `u` -- it depends
        // only on the previous iteration's `d`, unaffected by
        // `new_blocks` filling up during the inner selection below.
        let mut freq: HashMap<usize, usize> = HashMap::new();
        for &neighbor in neighbors {
            *freq.entry(d[neighbor]).or_insert(0) += 1;
        }
        let mut candidates: Vec<usize> = freq.keys().copied().collect();
        // Descending frequency; ties toward the lower block ID
        // (documented interpretation choice, top-level doc).
        candidates.sort_by(|&a, &b| freq[&b].cmp(&freq[&a]).then(a.cmp(&b)));

        let mut placed = false;
        for x in candidates {
            if new_blocks[x].len() < block_size {
                new_blocks[x].push(u);
                placed = true;
                break;
            }
            // else: H = H \ {x} -- x is exhausted (full); the next
            // iteration of this loop tries the next-highest-frequency
            // candidate, exactly Algorithm 1's own inner while loop.
        }

        if !placed {
            // H = ∅ (no neighbors, or every candidate block full):
            // "add u to an empty block in B" -- read here as the first
            // block (ascending ID) with spare room, see top-level doc.
            for block in new_blocks.iter_mut() {
                if block.len() < block_size {
                    block.push(u);
                    placed = true;
                    break;
                }
            }
        }

        assert!(
            placed,
            "rho * block_size >= n guarantees room for every node"
        );
    }

    new_blocks
}

/// Reconstructs block contents (block ID → member nodes, ascending node-ID
/// order) from a flat node→block-ID assignment — used only to seed
/// [`bnf`]'s own `last_blocks` from [`bnp`]'s initial mapping before its
/// first iteration's `new_blocks` overwrites it.
fn blocks_from_assignment(d: &[usize], rho: usize, n: usize) -> Vec<Vec<usize>> {
    let mut blocks: Vec<Vec<usize>> = vec![Vec::new(); rho];
    for u in 0..n {
        blocks[d[u]].push(u);
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vamana::{VamanaConfig, build_vamana};
    use proptest::prelude::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn assert_is_bijection(permutation: &[usize], n: usize) {
        assert_eq!(permutation.len(), n, "permutation must cover every node");
        let mut seen = vec![false; n];
        for &slot in permutation {
            assert!(slot < n, "slot {slot} out of range for n={n}");
            assert!(!seen[slot], "slot {slot} used more than once");
            seen[slot] = true;
        }
        assert!(
            seen.iter().all(|&s| s),
            "every physical slot must be used exactly once"
        );
    }

    // ---- Bijection: the correctness property the eventual wire-format --
    // ---- permutation-directory blob depends on completely -------------

    #[test]
    fn bnp_produces_a_bijection_on_the_rfcs_worked_example() {
        let graph: Graph = vec![vec![1, 2], vec![0, 3], vec![3, 0], vec![1, 4], vec![3]];
        let permutation = bnp(&graph, 2);
        assert_is_bijection(&permutation, 5);
    }

    #[test]
    fn bnf_produces_a_bijection_on_the_rfcs_worked_example() {
        let graph: Graph = vec![vec![1, 2], vec![0, 3], vec![3, 0], vec![1, 4], vec![3]];
        let config = BnfConfig {
            block_size: 2,
            beta: 10,
            tau: 0.0,
        };
        let permutation = bnf(&graph, &config);
        assert_is_bijection(&permutation, 5);
    }

    #[test]
    fn bnp_and_bnf_handle_a_node_with_no_neighbors_at_all() {
        // Node 3 is isolated -- exercises bnf's "H = ∅ from the start"
        // fallback branch directly, not only the "H exhausted" branch.
        let graph: Graph = vec![vec![1], vec![0, 2], vec![1], vec![]];
        let bnp_perm = bnp(&graph, 2);
        assert_is_bijection(&bnp_perm, 4);

        let config = BnfConfig {
            block_size: 2,
            beta: 5,
            tau: 0.0,
        };
        let bnf_perm = bnf(&graph, &config);
        assert_is_bijection(&bnf_perm, 4);
    }

    #[test]
    fn bnp_and_bnf_handle_the_empty_graph() {
        let graph: Graph = vec![];
        assert_eq!(bnp(&graph, 4), Vec::<usize>::new());
        let config = BnfConfig {
            block_size: 4,
            beta: 3,
            tau: 0.0,
        };
        assert_eq!(bnf(&graph, &config), Vec::<usize>::new());
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// Bijection holds across real Vamana-constructed graphs (task 1's
        /// own output shape) at varied scale, degree bound, and block size
        /// -- not just the hand-picked examples above.
        #[test]
        fn bnp_and_bnf_are_always_bijections_over_real_vamana_graphs(
            n in 5usize..60,
            dims in 2usize..6,
            r in 2usize..8,
            block_size in 1usize..10,
            seed in any::<u64>(),
        ) {
            let mut rng = StdRng::seed_from_u64(seed);
            let points: Vec<f32> = (0..n * dims)
                .map(|_| rng.random_range(-10.0f32..10.0))
                .collect();
            let config = VamanaConfig {
                r,
                l_param: (r * 2).max(1),
                alpha: 1.2,
            };
            let result = build_vamana(&points, n, dims, &config, &mut rng);

            let bnp_perm = bnp(&result.graph, block_size);
            assert_is_bijection(&bnp_perm, n);

            let bnf_config = BnfConfig { block_size, beta: 5, tau: 0.0 };
            let bnf_perm = bnf(&result.graph, &bnf_config);
            assert_is_bijection(&bnf_perm, n);
        }

        /// OR(G) is always within Definition 1's own stated range [0, 1]
        /// ("a graph layout with optimal data locality has OR(G) = 1") --
        /// a real sanity property of the metric itself, checked over the
        /// same real-graph population as the bijection property above.
        #[test]
        fn overlap_ratio_is_always_within_zero_and_one(
            n in 5usize..60,
            dims in 2usize..6,
            r in 2usize..8,
            block_size in 1usize..10,
            seed in any::<u64>(),
        ) {
            let mut rng = StdRng::seed_from_u64(seed);
            let points: Vec<f32> = (0..n * dims)
                .map(|_| rng.random_range(-10.0f32..10.0))
                .collect();
            let config = VamanaConfig {
                r,
                l_param: (r * 2).max(1),
                alpha: 1.2,
            };
            let result = build_vamana(&points, n, dims, &config, &mut rng);

            let identity: Permutation = (0..n).collect();
            let naive_or = overlap_ratio(&result.graph, &identity, block_size);
            prop_assert!((0.0..=1.0).contains(&naive_or));

            let bnp_perm = bnp(&result.graph, block_size);
            let bnp_or = overlap_ratio(&result.graph, &bnp_perm, block_size);
            prop_assert!((0.0..=1.0).contains(&bnp_or));

            let bnf_config = BnfConfig { block_size, beta: 5, tau: 0.0 };
            let bnf_perm = bnf(&result.graph, &bnf_config);
            let bnf_or = overlap_ratio(&result.graph, &bnf_perm, block_size);
            prop_assert!((0.0..=1.0).contains(&bnf_or));
        }
    }

    // ---- Hand-checkable mechanism proofs -------------------------------
    //
    // A 6-node graph, chosen (not the RFC's own 5-node worked example, which
    // turns out to coincide with ID order for both naive and BNP on this
    // exact tiny topology -- see the module-level test below for that
    // example's own, different value) specifically so naive/ID-order
    // placement is provably bad and BNP is provably optimal, both by hand.
    // Two disjoint 2-cycles, deliberately interleaved by ID: 0<->3, 1<->4,
    // 2<->5.

    fn interleaved_pairs_graph() -> Graph {
        vec![
            vec![3], // 0 -> 3
            vec![4], // 1 -> 4
            vec![5], // 2 -> 5
            vec![0], // 3 -> 0
            vec![1], // 4 -> 1
            vec![2], // 5 -> 2
        ]
    }

    #[test]
    fn hand_check_naive_id_order_has_zero_locality_on_interleaved_pairs() {
        // Naive/ID-order blocks of size 2: {0,1}, {2,3}, {4,5}. Every
        // node's sole neighbor lands in a *different* block (0's neighbor
        // 3 is in block {2,3}, not 0's own block {0,1}; symmetric for all
        // six nodes) -- OR(u) = 0 for every u, by hand.
        let graph = interleaved_pairs_graph();
        let identity: Permutation = (0..6).collect();
        let or = overlap_ratio(&graph, &identity, 2);
        assert!((or - 0.0).abs() < 1e-12, "expected exactly 0.0, got {or}");
    }

    #[test]
    fn hand_check_bnp_reaches_perfect_locality_on_interleaved_pairs() {
        // BNP's own ascending-ID sweep: u=0 unassigned -> new block, push
        // 0; 0's only neighbor is 3, unassigned and room remains -> push 3
        // (block {0,3} full). u=1 unassigned -> new block, push 1; 1's
        // only neighbor is 4 -> push 4 (block {1,4} full). u=2 -> new
        // block {2,5}, by the same reasoning. u=3,4,5 already assigned,
        // skipped. Final blocks: [0,3],[1,4],[2,5] -- every node's sole
        // neighbor is now in its own block: OR(u) = 1/(2-1) = 1.0 for
        // every u, by hand.
        let graph = interleaved_pairs_graph();
        let permutation = bnp(&graph, 2);
        assert_eq!(
            permutation,
            vec![0, 2, 4, 1, 3, 5],
            "expected blocks [0,3],[1,4],[2,5] in that order, giving slots \
             0->0, 3->1, 1->2, 4->3, 2->4, 5->5"
        );
        let or = overlap_ratio(&graph, &permutation, 2);
        assert!((or - 1.0).abs() < 1e-12, "expected exactly 1.0, got {or}");
    }

    #[test]
    fn hand_check_bnp_matches_the_rfcs_own_worked_example_topology() {
        // RFC 0014's own 5-node worked example (A=0,B=1,C=2,D=3,E=4):
        // A->{B,C}, B->{A,D}, C->{D,A}, D->{B,E}, E->{D}. Hand trace of
        // BNP at block_size=2: u=0(A) unassigned -> block0=[A]; A's
        // neighbors [B,C] -- B unassigned, room -> block0=[A,B] (full);
        // C: block0 full -> stop. u=1(B) assigned, skip. u=2(C)
        // unassigned, block0 full -> new block1=[C]; C's neighbors [D,A]
        // -- D unassigned, room -> block1=[C,D] (full); A assigned,
        // skip. u=3(D) assigned, skip. u=4(E) unassigned, block1 full ->
        // new block2=[E]; E's neighbor [D] assigned, skip. Final blocks:
        // [A,B],[C,D],[E] -- identical to ID order for this specific
        // topology (a real, honestly-noted coincidence of this tiny
        // example, not true in general -- see the interleaved-pairs
        // example above for a case where it very much is not).
        let graph: Graph = vec![vec![1, 2], vec![0, 3], vec![3, 0], vec![1, 4], vec![3]];
        let permutation = bnp(&graph, 2);
        assert_eq!(permutation, vec![0, 1, 2, 3, 4]);
        let or = overlap_ratio(&graph, &permutation, 2);
        // OR(A)=1 (B in block0), OR(B)=1 (A in block0), OR(C)=1 (D in
        // block1), OR(D)=0 (neither B nor E in block1), OR(E)=0 (singleton
        // block) -- sum 3, /5 = 0.6, by hand.
        assert!((or - 0.6).abs() < 1e-9, "expected 0.6, got {or}");
    }

    #[test]
    fn hand_check_bnf_one_iteration_matches_algorithm_1s_own_mechanism() {
        // Same 5-node RFC graph and BNP starting layout as the test above
        // (D: A,B -> block0; C,D -> block1; E -> block2), one BNF
        // iteration traced by hand against Algorithm 1's own pseudocode:
        //
        // u=0(A): neighbors {B,C}, D(B)=0, D(C)=1 -> freq {0:1, 1:1},
        //   tie -> try block0 (lower id) first: empty, room -> place A in
        //   block0. new_blocks: block0=[A].
        // u=1(B): neighbors {A,D}, D(A)=0 (OLD D, unchanged this pass),
        //   D(D)=1 -> freq {0:1, 1:1}, tie -> block0 first: room (len 1)
        //   -> place B. block0=[A,B] (now full).
        // u=2(C): neighbors {D,A}, D(D)=1, D(A)=0 -> freq {1:1, 0:1},
        //   tie -> block0 first: full -> try block1: empty, room -> place
        //   C. block1=[C].
        // u=3(D): neighbors {B,E}, D(B)=0 (OLD D), D(E)=2 -> freq {0:1,
        //   2:1}, tie -> block0 first: full -> try block2: empty, room ->
        //   place D. block2=[D].
        // u=4(E): neighbors {D}, D(D)=1 (OLD D) -> freq {1:1} -> try
        //   block1: room (len 1) -> place E. block1=[C,E] (full).
        //
        // Final blocks after iteration 1: [A,B],[C,E],[D] -- a real,
        // honest case where a single BNF iteration *decreases* OR(G)
        // relative to BNP's own 0.6 (see the assertion below), matching
        // the paper's own explicit caveat that "BNF's efficiency is
        // contingent on the number of iterations and does not ensure the
        // convergence of OR(G)" (unlike BNS, which Lemma 4.2 proves
        // monotonic) -- not a bug in this transcription. This test checks
        // that raw one-iteration mechanism directly via `bnf_one_iteration`
        // (not `bnf` itself, which -- correctly, see this module's own
        // top-level documentation -- discards this particular iteration's
        // result in favor of BNP's own better-scoring starting layout).
        let graph: Graph = vec![vec![1, 2], vec![0, 3], vec![3, 0], vec![1, 4], vec![3]];
        // BNP's own starting layout for this graph at block_size=2 (matches
        // `hand_check_bnp_matches_the_rfcs_own_worked_example_topology`
        // above exactly): blocks [A,B],[C,D],[E] -> d = [0,0,1,1,2].
        let d = vec![0, 0, 1, 1, 2];
        let new_blocks = bnf_one_iteration(&graph, &d, 3, 2);
        let permutation = permutation_from_blocks(&new_blocks, 5);
        assert_eq!(
            permutation,
            vec![0, 1, 2, 4, 3],
            "expected blocks [A,B],[C,E],[D] giving slots A=0,B=1,C=2,E=3,D=4"
        );
        let or = overlap_ratio(&graph, &permutation, 2);
        // OR(A)=1 (B in block0), OR(B)=1 (A in block0), OR(C)=0 (neither D
        // nor A in block1={C,E}), OR(D)=0 (singleton block2), OR(E)=0
        // (neither D in block1) -- sum 2, /5 = 0.4, by hand.
        assert!((or - 0.4).abs() < 1e-9, "expected 0.4, got {or}");

        // And bnf() itself, called with the exact same config, correctly
        // returns BNP's own better-scoring 0.6 layout instead -- the
        // best-seen-tracking wrapper this module's own top-level
        // documentation describes, proven end to end here.
        let config = BnfConfig {
            block_size: 2,
            beta: 1,
            tau: 0.0,
        };
        let wrapped = bnf(&graph, &config);
        assert_eq!(
            wrapped,
            vec![0, 1, 2, 3, 4],
            "bnf() must return BNP's own 0.6 layout, not this iteration's worse 0.4 one"
        );
        let wrapped_or = overlap_ratio(&graph, &wrapped, 2);
        assert!(
            (wrapped_or - 0.6).abs() < 1e-9,
            "expected 0.6, got {wrapped_or}"
        );
    }

    // ---- The required comparative proof: BNF >= BNP >= naive on a real -
    // ---- build_vamana-constructed graph at realistic scale -------------

    #[test]
    fn bnf_beats_bnp_beats_naive_on_a_real_vamana_graph() {
        let n = 500;
        let dims = 32;
        let mut rng = StdRng::seed_from_u64(42);
        let points: Vec<f32> = (0..n * dims)
            .map(|_| rng.random_range(-50.0f32..50.0))
            .collect();
        let config = VamanaConfig {
            r: 16,
            l_param: 32,
            alpha: 1.2,
        };
        let result = build_vamana(&points, n, dims, &config, &mut rng);

        let block_size = 16;
        let identity: Permutation = (0..n).collect();
        let naive_or = overlap_ratio(&result.graph, &identity, block_size);

        let bnp_perm = bnp(&result.graph, block_size);
        let bnp_or = overlap_ratio(&result.graph, &bnp_perm, block_size);

        // beta=50, tau=-1.0: since OR(G) gain is always in [-1, 1], `gain <
        // -1.0` can never hold, so this configuration never early-stops and
        // always runs the full 50 iterations -- giving BNF's own iterative
        // refinement real room to compound, rather than risking a small or
        // zero tau halting after one iteration before any refinement has a
        // chance to help (see this module's own top-level documentation,
        // "Third," for why a single early iteration can be a poor stopping
        // point on its own).
        let bnf_config = BnfConfig {
            block_size,
            beta: 50,
            tau: -1.0,
        };
        let bnf_perm = bnf(&result.graph, &bnf_config);
        let bnf_or = overlap_ratio(&result.graph, &bnf_perm, block_size);

        assert_is_bijection(&bnp_perm, n);
        assert_is_bijection(&bnf_perm, n);

        // Real, measured numbers at n=500/dims=32/R=16/block_size=16,
        // seed 42 -- see docs/roadmap.md's M2-3 entry and docs/ledger.md
        // for the exact values this assertion is pinned against and their
        // interpretation. This is the RFC's own cited claim
        // ("BNF ≈ 0.3-0.6 vs naive ≈ 0") checked against a real, not
        // literature-cited, STRAND-constructed graph.
        println!("naive OR(G)={naive_or:.4} bnp OR(G)={bnp_or:.4} bnf OR(G)={bnf_or:.4}");
        assert!(
            naive_or < 0.05,
            "naive/ID-order OR(G) should be close to the paper's own \
             measured ~0 baseline, got {naive_or}"
        );
        assert!(
            bnp_or > naive_or,
            "BNP must beat the naive baseline: bnp={bnp_or} naive={naive_or}"
        );
        assert!(
            bnf_or > bnp_or,
            "BNF must strictly beat BNP at this configuration: bnf={bnf_or} bnp={bnp_or}"
        );
    }
}
