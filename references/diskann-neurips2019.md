# DiskANN (Subramanya et al., NeurIPS 2019)

Vendored excerpt. Source: NeurIPS 2019 proceedings abstract page,
`proceedings.neurips.cc`. Fetched 2026-08-18.

Cited by: `docs/research/README.md` R1, `docs/lineage.md` ("From DiskANN" — the
two-tier memory model).

**Title:** DiskANN: Fast Accurate Billion-point Nearest Neighbor Search on a Single
Node
**Authors:** Suhas Jayaram Subramanya, Fnu Devvrit, Harsha Vardhan Simhadri,
Ravishankar Krishnaswamy, Rohan Kadekodi
**Venue:** NeurIPS 2019 (Advances in Neural Information Processing Systems 32)

## Abstract (excerpt)

> The paper describes a graph-based indexing and search system called DiskANN "that
> can index, store, and search a billion point database on a single workstation with
> just 64GB RAM and an inexpensive solid-state drive."

## What this grounds

Confirms the paper this project cites for the two-tier memory model (compressed
codes in memory for routing/candidate generation, full-precision vectors on SSD for
reranking) that `docs/lineage.md` generalizes into STRAND's tiered vector blob, and
for R1's "50–200 dependent fetches" graph-beam-search framing that motivates the
cluster-shaped alternative.
