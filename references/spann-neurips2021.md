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

`docs/research/README.md` R1 cites specific numbers — the "13.0 GB vs 7.5 GB index
at replica 8 vs 2 on GIST1M" figure and the "up to 3.14× QPS" I/O-congestion figure
— that this fetch, retrieving the abstract only, could not confirm.

**Update, 2026-08-19.** This paper's full PDF body has since been fetched directly
(`arxiv.org/pdf/2111.08566`, the only version on arXiv) and searched exhaustively.
The two figures above are **not in this paper at all**: GIST1M does not appear
anywhere in it (this paper's datasets are SIFT1M, SIFT1B, DEEP1B, and SPACEV1B), and
no index-size-in-bytes number for any replica count is reported anywhere in the
text. R1's own parenthetical wording — "...on GIST1M **in the benchmark**" — was
correctly attributing these figures to the companion benchmark paper
(`arxiv.org/abs/2511.14748`) all along, not to this one. The real figures, quoted
verbatim from that paper's Table 4 and Figure 14 (§5.3), are vendored in
`references/spann-body-figures.md`. This paper's own genuine replication finding is
narrower and different: Figure 11 reports recall/latency curves at replica counts 1,
4, 8, and 10, and the text states plainly that performance stops improving past 8
replicas, which is why SPANN's own experiments use replica=8 as the default — no
size figure accompanies it.
