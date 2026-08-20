# DiskANN (Subramanya et al., NeurIPS 2019)

Vendored excerpt. Source: NeurIPS 2019 proceedings, full paper PDF,
`proceedings.neurips.cc/paper_files/paper/2019/file/09853c7fb1d3f8ee67a61b6bf4a7f8e6-Paper.pdf`.
Abstract-only fetched 2026-08-18 (superseded); **full paper (title page,
§1–§3, and the first page of §4/§5, including Algorithms 1–3 and the
billion-point experiment parameters) fetched 2026-08-20** for
`rfcs/0014-graph-blob-family.md`'s Vamana-construction and on-disk-layout
grounding, per `CLAUDE.md` §3's "never implement against a remembered
spec" rule — the earlier abstract-only vendoring was insufficient to
ground a wire-format RFC's algorithm and byte-layout claims.

Cited by: `docs/research/README.md` R1, `docs/lineage.md` ("From DiskANN" —
the two-tier memory model), `rfcs/0014-graph-blob-family.md`.

**Title:** DiskANN: Fast Accurate Billion-point Nearest Neighbor Search on a
Single Node
**Authors:** Suhas Jayaram Subramanya, Fnu Devvrit, Harsha Vardhan Simhadri,
Ravishankar Krishnaswamy, Rohan Kadekodi
**Venue:** NeurIPS 2019 (Advances in Neural Information Processing Systems 32)

## Abstract (excerpt)

> The paper describes a graph-based indexing and search system called DiskANN
> "that can index, store, and search a billion point database on a single
> workstation with just 64GB RAM and an inexpensive solid-state drive." ...
> "we introduce Vamana, a new graph-based ANNS index that is more versatile
> than the existing graph indices even for in-memory indices."

## Notation (§1.2, verbatim definitions)

`P` denotes the dataset with `|P| = n`. Directed graphs `G = (P, E)`, letting
`P` also denote the vertex set. For a point `p ∈ P`, `N_out(p)` denotes the
set of out-edges incident on `p`. `x_p` denotes the vector data for `p`, and
`d(p, q) = ||x_p − x_q||` is Euclidean distance. All experiments in the paper
use Euclidean metric.

## Algorithm 1: GreedySearch(s, x_q, k, L)

> **Data:** Graph `G` with start node `s`, query `x_q`, result size `k`,
> search list size `L ≥ k`.
> **Result:** Result set `L` containing `k`-approx NNs, and a set `V`
> containing all the visited nodes.
>
> ```
> initialize sets L ← {s} and V ← ∅
> while L \ V ≠ ∅ do
>     let p* ← argmin_{p ∈ L\V} d(x_p, x_q)
>     update L ← L ∪ N_out(p*) and V ← V ∪ {p*}
>     if |L| > L then
>         update L to retain closest L points to x_q
> return [closest k points from L; V]
> ```

## Algorithm 2: RobustPrune(p, V, α, R)

> **Data:** Graph `G`, point `p ∈ P`, candidate set `V`, distance threshold
> `α ≥ 1`, degree bound `R`.
> **Result:** `G` is modified by setting at most `R` new out-neighbors for
> `p`.
>
> ```
> V ← (V ∪ N_out(p)) \ {p}
> N_out(p) ← ∅
> while V ≠ ∅ do
>     p* ← argmin_{p' ∈ V} d(p*, p')      // closest remaining candidate
>     N_out(p) ← N_out(p) ∪ {p*}
>     if |N_out(p)| = R then break
>     for p' ∈ V do
>         if α · d(p*, p') ≤ d(p, p') then remove p' from V
> ```

The α parameter (§2.2): "we would like to ensure that the distance to the
query decreases by a multiplicative factor of `α > 1` at every node along the
search path, instead of merely decreasing as in the SNG property." Larger α
keeps more long-range edges (candidates that are not "shadowed" by a closer
already-selected neighbor survive pruning), at the cost of higher average
degree and construction time.

## Algorithm 3: Vamana Indexing Algorithm

> **Data:** Database `P` with `n` points, parameters `α`, `L`, `R`.
> **Result:** Directed graph `G` over `P` with out-degree `≤ R`.
>
> ```
> initialize G to a random R-regular directed graph
> let s denote the medoid of dataset P
> let σ denote a random permutation of 1..n
> for 1 ≤ i ≤ n do
>     let [L; V] ← GreedySearch(s, x_σ(i), 1, L)
>     run RobustPrune(σ(i), V, α, R) to update out-neighbors of σ(i)
>     for all points j in N_out(σ(i)) do
>         if |N_out(j) ∪ {σ(i)}| > R then
>             run RobustPrune(j, N_out(j) ∪ {σ(i)}, α, R) to update
>             out-neighbors of j
>         else
>             update N_out(j) ← N_out(j) ∪ {σ(i)}
> ```

§2.3, verbatim on the two-pass convention: "Vamana constructs a directed
graph `G` in an iterative manner... The graph is initialized so that each
vertex has `R` randomly chosen out-neighbors... we let `s` denote the medoid
of the dataset `P`, which will be the starting node for the search
algorithm... Our overall algorithm makes two passes over the dataset, the
first pass with `α = 1`, and the second with a user-defined `α ≥ 1`. We
observed that a second pass results in better graphs, and that running both
passes with the user-defined `α` makes the indexing algorithm slower as the
first pass computes a graph with higher average degree which takes longer."
Figure 1's caption: "the algorithm goes through the first pass with `α = 1`,
followed by the second pass where it introduces long range edges" — the
α > 1 pass is specifically what adds long-range (not merely spatially local)
edges to the graph.

## §3: DiskANN system design — on-disk layout (§3.1–§3.3, verbatim/paraphrased)

**§3.1 Index construction at scale.** To index more points than fit in RAM,
DiskANN partitions the dataset into `k` overlapping clusters via k-means,
assigns each point to its `ℓ`-closest centers (`ℓ = 2` typically), builds a
separate in-memory Vamana graph per cluster, then "finally merge all the
different graphs into a single graph by taking a simple union of edges."

**§3.2 DiskANN Index Layout (verbatim):** "We store the compressed vectors
of all the data points in memory, and store the graph along with the
full-precision vectors on the SSD. On the disk, for each point `i`, we store
its full precision vector `x_i` followed by the identities of its `≤ R`
neighbors. If the degree of a node is smaller than `R`, we pad with zeros,
so that computing the offset within the disk of the data corresponding to
any point `i` is a simple calculation, and does not require storing the
offsets in memory."

**§3.3 DiskANN Beam Search (paraphrased + verbatim fragment):** A naive
search runs Algorithm 1 fetching `N_out(p*)` from SSD as needed — reliable
but round-trip-heavy. To reduce round trips without wasting compute, DiskANN
"fetch[es] the neighborhoods of a small number, `W` (say 4, 8), of the
closest points in `L \ V` in one shot, and update `L` to be the top `L`
candidates in `L` along with all the neighbors retrieved in this step... We
refer to this modified search algorithm as BeamSearch. If `W = 1`, this
search resembles normal greedy search... if `W` is too large, say 16 or
more, then both compute and SSD bandwidth could be wasted."

**§3.5 Implicit re-ranking (verbatim fragment):** "full-precision coordinates
stored for each point next to its neighborhood on the disk... when we
retrieve the neighborhood of a point during search, we also retrieve the
full coordinates of the point without incurring extra disk reads. This is
because reading a `4KB`-aligned disk address into memory is no more
expensive than reading `512B`... full precision coordinates *essentially
piggyback* on the cost of expanding the neighborhoods."

## §4 experiment parameters (real, cited numbers — not invented for this RFC)

**In-memory comparison (§4.1):** "All HNSW indices were constructed using
`M = 128, ef_C = 512`, while Vamana indices used `L = 125, R = 70, C = 3000,
α = 2`" for SIFT1M/GIST1M; `R = 60, L = 70, C = 500` for DEEP1M.

**Hop-count comparison (§4.2):** Vamana needs "2–3 times fewer hops to
converge on large datasets compared to HNSW and NSG," measured with
BeamSearch width `W = 4` for all three algorithms at a target 5-recall@5 of
98%.

**Billion-scale, SIFT1B `bigann` (§4.3), verbatim figures:** single-shot
index: `L = 125, R = 128, α = 2`, ~2 days build on a 1,792GB-RAM machine,
peak memory ≈ 1,100GB, **average degree 113.9**. Merged index: `k = 40`
shards, `ℓ = 2` closest shards, per-shard `L = 125, R = 64, α = 2`,
~5 days build under 64GB RAM, **348GB total, average degree 92.1**. Finding
(verbatim): "The single index outperforms the merged index, which traverses
more links to reach the same neighborhood, thus increasing search latency.
This could possibly be because the in- and out-edges of each node in the
merged index are limited to about `ℓ/k = 5%` of all points." Single-index
1-recall@1 of 98.68% with <5ms latency; abstract's headline figure: "> 5000
queries a second with < 3ms mean latency and 95%+ 1-recall@1 on a 16 core
machine."

**Disk/SSD figures motivating the round-trip target (§1):** "An inexpensive
retail-grade SSD requires a few hundred microseconds to serve a random read
and can service about ∼300K random reads per second... the main challenges
in designing a performant SSD-resident index lie in reducing (a) the number
of random SSD accesses to a few dozen, and (b) the number of round trip
requests to disk to under ten, preferably five." §3.3 continues: "operating
at peak throughput results in disk read latencies of over a millisecond...
we have found that operating at a lower load factor [30–40%] can strike a
good balance between latency and throughput."

## What this grounds

The complete Vamana construction algorithm (Algorithms 1–3, transcribed
verbatim above, not paraphrased from memory) grounds `rfcs/0014-graph-blob-
family.md`'s registration of DiskANN/Vamana as the format's one registered
graph-index algorithm (invariant 8), its on-disk node-record layout
(§3.2's vector-then-padded-neighbor-list convention, adapted to STRAND's own
container discipline), its merge-semantics argument (§4.3's own single-vs-
merged latency finding, cited directly as real evidence that graph merging
has a real, measured search-quality cost), and its warm-tier napkin-math
translation (the retail-SSD random-read and round-trip figures above).
