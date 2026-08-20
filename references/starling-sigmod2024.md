# Starling (Wang et al., SIGMOD 2024)

Vendored excerpt. Source: `arxiv.org/pdf/2401.02116` (the SIGMOD 2024 paper's
arXiv preprint; the ACM DOI page, `dl.acm.org`, returned HTTP 403 to an
unauthenticated fetch). Abstract-only fetched 2026-08-18 (superseded);
**full paper (title/abstract, §1–§2, §3.1 "I/O-efficiency analysis," §4
"Data layout" including Definition 1, Theorem 4.1 and its NP-hardness proof
sketch, Algorithms I–III (BNP/BNF/BNS) and Algorithm 1's full pseudocode,
§4.2 in-memory navigation graph, and the start of §5 "Search strategy")
fetched 2026-08-20** for `rfcs/0014-graph-blob-family.md`'s node-order-
permutation grounding, per `CLAUDE.md` §3's rule against implementing
against a remembered spec.

Cited by: `docs/research/README.md` R1, `docs/ledger.md` R1 ("the graph-blob
ordering algorithm — Starling's block shuffling is the literature"),
`docs/data-structures.md` ("Node order within a graph blob is a persisted
permutation... Starling's block shuffling is the relevant literature, and
R1's RFC picks with evidence"), `rfcs/0014-graph-blob-family.md`.

**Title:** Starling: An I/O-Efficient Disk-Resident Graph Index Framework for
High-Dimensional Vector Similarity Search on Data Segment
**Authors:** Mengzhao Wang, Weizhi Xu, Xiaomeng Yi, Songlin Wu, Zhangyang
Peng, Xiangyu Ke, Yunjun Gao, Xiaoliang Xu, Rentong Guo, Charles Xie
**Venue:** SIGMOD 2024 (Proc. ACM Manag. Data)

## Abstract (excerpt)

> "Starling... has two primary components: (1) a data layout incorporating
> an in-memory navigation graph and a reordered disk-based graph with
> enhanced locality, reducing the search path length and minimizing disk
> bandwidth wastage; and (2) a block search strategy designed to minimize
> costly disk I/O operations during vector query execution... Starling can
> accommodate up to 33 million vectors in 128 dimensions [on 2GB memory,
> 10GB disk], offering HVSS with over 0.9 average precision and top-10
> recall rate, and latency under 1 millisecond... 43.9× higher throughput
> with 98% lower query latency compared to state-of-the-art methods."

## §3.1 — Why the naive (ID-contiguous) layout wastes I/O (verbatim figures)

Motivating the whole paper: on the DiskANN baseline, "`T_I/O` constitutes up
to 92.5% of `T_total`, while `T_comp` and `T_other` collectively occupy less
than 7.5%" and, per Table 2's BIGANN evaluation, "up to 94% of data read from
the disk is wasted" — because a naive layout assigns ID-consecutive vertices
to the same disk block, and "ID-consecutive vertices in a block do not imply
spatial proximity."

## §4.1 — Definitions and the overlap-ratio locality metric

**Definition 1 (Block-Level Graph Layout, verbatim):** "a scheme that
assigns `|V|` vertices to `ρ` blocks," where each vertex needs `γ` KB for its
vector data, a neighbor count `λ`, and a neighbor-ID list of maximum length
`Λ`; block size is `η` KB; each block holds at most `ε = ⌊η/γ⌋` vertices;
`ρ = ⌈|V|/ε⌉` blocks total. Worked numeric example given in the paper
itself: DiskANN on 33M-vector BIGANN, 128-dim, `Λ = 31`, `η = 4KB` (the
paper's own stated default disk block size) → `γ = (128 + 4 + 31×4)/1024
KB`, `ε = 16`, `ρ = 2,062,500`.

**Overlap ratio `OR(u)` (Eq. 5, verbatim):** for vertex `u` in block `B(u)`
with true graph-neighbor set `N(u)`, `OR(u) = |B(u) ∩ N(u)| / (|B(u)| − 1)`
when `|B(u)| > 1`, else `0`. `OR(G)` is the mean of `OR(u)` over all
vertices. "A graph layout with optimal data locality has `OR(G) = 1`... we
get `OR(G)` close to 0 for the DiskANN graph index on the 33 million BIGANN"
— i.e., the naive layout is measured, not assumed, to have essentially zero
locality.

**Definition 2 (Block Shuffling, verbatim):** "Given a graph layout for a
disk-based graph index `G`, the block shuffling aims to get a new layout
that maximizes the `OR(G)` while satisfying Def. 1."

**Theorem 4.1 (verbatim):** "The block shuffling problem is NP-hard and does
not have a polynomial time approximation algorithm with a finite
approximation factor unless P=NP." Proved by reduction from the (strongly
NP-complete) triple shuffling problem; full proof in the paper's Appendix A,
not re-derived here. This is the reason Starling registers concrete
heuristic algorithms rather than defining "the optimal layout" as the
target — no algorithm can compute that target in general.

## §4.1 — The three heuristic shuffling algorithms

**Algorithm I: Block Neighbor Padding (BNP).** Fills blocks one at a time in
ascending vertex-ID order: assign the next unassigned vertex and (as many of)
its neighbors as fit to the current block; open a new block once full. Time
complexity `O(|V|)` — a single pass. Weakest locality gain of the three,
because a vertex's neighbors may already have been placed in earlier blocks
by the time it is visited.

**Algorithm II: Block Neighbor Frequency (BNF).** Iteratively reassigns each
vertex to whichever block currently holds the most of its neighbors (highest
neighbor frequency), repeating until an iteration cap `β` or an `OR(G)`-gain
threshold `τ` is reached. Full pseudocode (Algorithm 1 in the paper, "Block
Shuffling by BNF," transcribed in full — inputs: block-level layout from
BNP, max iterations `β`, gain threshold `τ`):

> ```
> B = {B_0, ..., B_{ρ-1}} ← all blocks
> while iterations ≤ β do
>     D ← mapping of vertex IDs to block IDs
>     forall B_i ∈ B do B_i ← ∅               // clear all blocks
>     forall u ∈ V do
>         H ← ⋃_{a ∈ N(u)} {D(a)}              // all neighbors' block IDs
>         while H ≠ ∅ do
>             x ← block ID with the most neighbors in H
>             if B_x is not full then
>                 B_x ← B_x ∪ {u}; break
>             H = H \ {x}                       // remove the full block
>         if H = ∅ then add u to an empty block in B
>     if OR(G) gain < τ then break
> return new layout of G
> ```

Time complexity `O(β · o · |V|)`, `o` the average out-degree. The paper's own
recommendation (verbatim): "we recommend BNF due to its adept balance
between efficiency and effectiveness."

**Algorithm III: Block Neighbor Swap (BNS).** Inspired by the NN-Descent
method from graph-construction literature. Starting from BNP's or BNF's
layout, repeatedly finds a pair of neighboring vertices in different blocks
with low overlap ratio and swaps them if the swap increases the sum of the
two blocks' `OR`. Proven monotonically non-decreasing in `OR(G)` (Lemma
4.2) but the most expensive: `O(β · o³ · ε · |V|)`.

**Analysis (verbatim):** "Among our three block shuffling algorithms, BNP
emerges as the fastest... BNF's efficiency is contingent on the number of
iterations and does not ensure the convergence of `OR(G)`. However, BNF
demonstrates proficiency, both in terms of efficiency and effectiveness...
BNS, although possessing the highest time complexity, guarantees that
`OR(G)` does not decrease with iterations... All three algorithms notably
improve `OR(G)`... with BNS exhibiting the most significant improvement,
followed by BNF and then BNP." Measured range on real datasets: naive
`OR(G) ≈ 0`; shuffled layouts reach roughly `0.3`–`0.6` (§5.1, Fig. 9(a)).

**Time/space cost (verbatim):** "block shuffling only scans vertices and
performs simple statistics, without any vector calculation... [it]
introduces a relatively low additional time cost compared to the graph index
construction process. For example, BNF only occupies 3%~10% of the graph
index construction cost." Space cost: `O(|V| · (D + Λ))`, unchanged by
shuffling — "we only adjust the order of vertices and do not add any extra
information."

**Remarks (verbatim, on generality and comparison to graph partitioning):**
"(1) Our block shuffling methods can work with any block size, not just the
default 4KB... we can extend to 8KB or 16KB blocks by modifying the block
size. (2) Block shuffling is similar to but not identical to graph
partitioning... current graph partitioning methods thrive on real-world
graphs with clustering properties but may falter in graph index for vectors
[because] our graph index is based on high-dimensional vectors, where
neighbors exhibit similarity and navigation traits (with about 50% long
links)... We evaluated some advanced graph partitioning methods for our
block shuffling task, but they only gave limited improvement. For example,
BNF shows 40% higher `OR(G)` than an advanced graph partitioning method —
KGGGP — on the SSNPP dataset."

## §4.2 — In-memory navigation graph

Built by (1) randomly sampling a small subset `V'` of vectors (`|V'| < 10%`
of the segment in the paper's own evaluation) based on the segment's memory
budget, then (2) running the *same* graph-construction algorithm used for
the disk-based graph (HNSW, NSG, or Vamana — the paper is graph-algorithm-
agnostic here) over just that subset. At query time, this small in-memory
graph is searched first (no disk I/O) to obtain query-close entry points for
the disk-based search, shortening the disk-side search path `ℓ`. Time cost:
`O(|V'| log|V'|)`, "only 5.5% of the total index processing time" in the
paper's evaluation. Storage format is identical to the disk-based graph's
own per-vertex record shape.

## §5.1 — Block search and block pruning (verbatim, pruning ratio `σ`)

Because real shuffled layouts reach `OR(G) ≈ 0.3`–`0.6`, not `1`, "some
vertices in a block are irrelevant." Starling's block search "sorts the
vertices in a block by their distance to the query vector in ascending
order. Then, it only checks the neighbor IDs of the top-`((ε − 1) · σ)`
vertices for new search candidates," `σ ∈ (0, 1]` a tunable pruning ratio;
the paper's own evaluation found `σ = 0.3` optimal.

## What this grounds

Resolves the open half of R1 (`docs/data-structures.md`, `docs/ledger.md`):
which node-order-permutation algorithm STRAND's graph blob registers.
`rfcs/0014-graph-blob-family.md` adopts BNF as the registered default,
citing this file's own transcription of Algorithm 1, the paper's explicit
efficiency/effectiveness recommendation, and the measured `OR(G)` and
construction-overhead figures above — not an untested guess. The NP-hardness
theorem (Theorem 4.1) grounds why STRAND pins a *specific heuristic
algorithm* as the registered default rather than an underspecified
"optimal locality" target, which invariant 11's byte-determinism discipline
would leave unpinnable. The in-memory navigation graph (§4.2) is named in
`rfcs/0014-graph-blob-family.md`'s Open questions as real, separate,
un-registered future work, not adopted by this RFC.
