# Starling (Wang et al., SIGMOD 2024)

Vendored excerpt. Source: `arxiv.org/abs/2401.02116` (the SIGMOD 2024 paper's arXiv
preprint; the ACM DOI page, `dl.acm.org`, returned HTTP 403 to an unauthenticated
fetch). Fetched 2026-08-18.

Cited by: `docs/research/README.md` R1, `docs/ledger.md` R1 ("the graph-blob ordering
algorithm (Starling's block shuffling is the literature)").

**Title:** Starling: An I/O-Efficient Disk-Resident Graph Index Framework for
High-Dimensional Vector Similarity Search on Data Segment
**Authors:** Mengzhao Wang, Weizhi Xu, Xiaomeng Yi, Songlin Wu, Zhangyang Peng,
Xiangyu Ke, Yunjun Gao, Xiaoliang Xu, Rentong Guo, Charles Xie
**Venue:** SIGMOD 2024

## Abstract (excerpt)

> "Starling, an I/O-efficient disk-resident graph index framework that optimizes
> data layout and search strategy within the segment. It has two primary components:
> (1) a data layout incorporating an in-memory navigation graph and a reordered
> disk-based graph with enhanced locality, reducing the search path length and
> minimizing disk bandwidth wastage; and (2) a block search strategy designed to
> minimize costly disk I/O operations during vector query execution."

## What this grounds

Confirms Starling as the "block shuffling" / disk-layout-reordering prior art
`docs/ledger.md` names as the literature basis for R1's still-open graph-blob node-
ordering algorithm question (component (1) above — the reordered disk-based graph
with enhanced locality — is what "block shuffling" refers to). The R1 RFC still owes
picking a specific ordering algorithm "with evidence"; this vendoring confirms the
paper exists and describes the technique, not which specific ordering STRAND should
adopt.
