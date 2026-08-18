# SPFresh (Xu et al., SOSP 2023)

Vendored excerpt. Source: Microsoft Research publication page,
`microsoft.com/en-us/research/publication/spfresh-incremental-in-place-update-for-billion-scale-vector-search/`.
Fetched 2026-08-18. (The DOI, `doi.org/10.1145/3600006.3613166`, redirects to
`dl.acm.org`, which returned HTTP 403 to an unauthenticated fetch; the MSR page is
the accessible primary source used instead.)

Cited by: `docs/research/README.md` R1 and R7, `docs/lineage.md` ("From SPANN /
SPFresh / turbopuffer"), `CLAUDE.md` invariant 1 (rebalance merge strategy).

**Title:** SPFresh: Incremental In-Place Update for Billion-Scale Vector Search
**Authors:** Yuming Xu, Hengyu Liang, Jin Li, Shuotao Xu, Qi Chen, Qianxi Zhang, Cheng
Li, Ziyue Yang, Fan Yang, Yuqing Yang, Peng Cheng, Mao Yang
**Venue:** SOSP '23 (October 2023)

## The LIRE resource-efficiency claim

> "With LIRE, SPFresh provides superior query latency and accuracy to solutions
> based on global rebuild, with only 1% of DRAM and less than 10% cores needed at the
> peak compared to the state-of-the-art, in a billion scale disk-based vector index
> with a 1% of daily vector update rate."

This is the exact source for the "only 1% of DRAM and less than 10% cores needed at
the peak" figure `docs/research/README.md` R7 already cites verbatim — confirmed
against the primary source rather than memory.

## LIRE's mechanism

> "SPFresh introduces LIRE (Lightweight Incremental Rebalancing), which achieves
> low-overhead vector updates by only reassigning vectors at the boundary between
> partitions, where in a high-quality vector index the amount of such vectors are
> deemed small."

Grounds the "rebalance" merge strategy invariant 1 declares for centroid layers.

## Not verified from this fetch (flagged, not asserted)

R7's claim that "static SPANN centroids degrade under drift (updating one-third of
vectors costs more than a point of recall and 4× tail latency)" was not found in the
excerpt this fetch retrieved. It may live deeper in the paper's evaluation section;
it remains unverified by this vendoring pass and should not be treated as confirmed
until checked against the full paper.
