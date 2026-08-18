# SPANN (Chen et al., NeurIPS 2021)

Vendored excerpt. Source: `arxiv.org/abs/2111.08566`. Fetched 2026-08-18.

Cited by: `docs/research/README.md` R1 (cluster-shaped cold vector search), `docs/lineage.md`
("From SPANN / SPFresh / turbopuffer").

**Title:** SPANN: Highly-efficient Billion-scale Approximate Nearest Neighbor Search
**Authors:** Qi Chen, Bing Zhao, Haidong Wang, Mingqin Li, Chuanjie Liu, Zengzhong Li,
Mao Yang, Jingdong Wang
**Venue:** NeurIPS 2021

## Abstract (verbatim)

> "The in-memory algorithms for approximate nearest neighbor search (ANNS) have
> achieved great success for fast high-recall search, but are extremely expensive
> when handling very large scale database. Thus, there is an increasing request for
> the hybrid ANNS solutions with small memory and inexpensive solid-state drive (SSD).
> In this paper, we present a simple but efficient memory-disk hybrid indexing and
> search system, named SPANN, that follows the inverted index methodology. It stores
> the centroid points of the posting lists in the memory and the large posting lists
> in the disk. We guarantee both disk-access efficiency (low latency) and high recall
> by effectively reducing the disk-access number and retrieving high-quality posting
> lists. In the index-building stage, we adopt a hierarchical balanced clustering
> algorithm to balance the length of posting lists and augment the posting list by
> adding the points in the closure of the corresponding clusters. In the search stage,
> we use a query-aware scheme to dynamically prune the access of unnecessary posting
> lists. Experiment results demonstrate that SPANN is 2× faster than the
> state-of-the-art ANNS solution DiskANN to reach the same recall quality 90% with
> same memory cost in three billion-scale datasets. It can reach 90% recall@1 and
> recall@10 in just around one millisecond with only 32GB memory cost."

## What this grounds

Confirms the centroids-in-memory / posting-lists-on-disk architecture
`docs/lineage.md` attributes to SPANN, and the 2× speedup over DiskANN at 90% recall
already implicit in R1's framing.

## Not verified from this fetch (flagged, not asserted)

`docs/research/README.md` R1 cites specific numbers from this paper's body — the
"13.0 GB vs 7.5 GB index at replica 8 vs 2 on GIST1M" figure and the "up to 3.14×
QPS" I/O-congestion figure. This fetch retrieved the abstract only, via the arXiv
listing page; those two figures live in the paper's body/tables and were not
re-confirmed here. They remain as previously stated (marked "prior knowledge" in
the README before this vendoring pass) and should be checked against the PDF body
directly before being treated as independently vendored.
