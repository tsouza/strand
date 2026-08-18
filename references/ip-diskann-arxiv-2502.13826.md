# In-Place Updates of a Graph Index for Streaming ANN (Xu et al.)

Vendored excerpt. Source: `arxiv.org/abs/2502.13826`. Fetched 2026-08-18.

Cited by: `docs/research/README.md` R7 (compaction and merge — graph indexes as
rebuild-on-merge).

**Title:** In-Place Updates of a Graph Index for Streaming Approximate Nearest
Neighbor Search
**Authors:** Haike Xu, Magdalen Dobson Manohar, Philip A. Bernstein, Badrish
Chandramouli, Richard Wen, Harsha Vardhan Simhadri

## Abstract (verbatim)

> "Indices for approximate nearest neighbor search (ANNS) are a basic component for
> information retrieval and widely used in database, search, recommendation and RAG
> systems. In these scenarios, documents or other objects are inserted into and
> deleted from the working set at a high rate, requiring a stream of updates to the
> vector index. Algorithms based on proximity graph indices are the most efficient
> indices for ANNS, winning many benchmark competitions. However, it is challenging
> to update such graph index at a high rate, while supporting stable recall after
> many updates. Since the graph is singly-linked, deletions are hard because there is
> no fast way to find in-neighbors of a deleted vertex. Therefore, to update the
> graph, state-of-the-art algorithms such as FreshDiskANN accumulate deletions in a
> batch and periodically consolidate, removing edges to deleted vertices and
> modifying the graph to ensure recall stability. In this paper, we present
> IP-DiskANN (InPlaceUpdate-DiskANN), the first algorithm to avoid batch
> consolidation by efficiently processing each insertion and deletion in-place. Our
> experiments using standard benchmarks show that IP-DiskANN has stable recall over
> various lengthy update patterns in both high-recall and low-recall regimes.
> Further, its query throughput and update speed are better than using the batch
> consolidation algorithm and HNSW."

## What this grounds

Confirms that even the state-of-the-art graph-index update algorithm (IP-DiskANN)
frames the *default* prior behavior — batch-then-consolidate, "removing edges to
deleted vertices and modifying the graph" — as the norm it is trying to avoid,
supporting invariant 1's characterization of graph indexes as effectively
rebuild-on-merge: consolidation is a graph-wide restructuring operation, not a
local, cheap edit, which is exactly why STRAND declares "rebuild" rather than
"concatenate + remap" as the graph-family merge strategy.
