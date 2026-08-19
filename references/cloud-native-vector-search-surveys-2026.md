# Two 2026 cloud-native vector search papers

Vendored excerpts. Fetched 2026-08-18.

Cited by: `docs/research/README.md` R1 ("the 2026 survey and its companion
benchmark").

## Survey: `arxiv.org/abs/2601.01937`

**Title:** Vector Search for the Future: From Memory-Resident, Static Heterogeneous
Storage, to Cloud-Native Architectures
**Authors:** Yitong Song, Xuanhe Zhou, Christian S. Jensen, Jianliang Xu
**Venue:** SIGMOD 2026 (submitted 2026-01-05)

### Abstract (verbatim)

> "Vector search (VS) has become a fundamental component in multimodal data
> management, enabling core functionalities such as image, video, and code
> retrieval. As vector data scales rapidly, VS faces growing challenges in balancing
> search, latency, scalability, and cost. The evolution of VS has been closely driven
> by changes in storage architecture. Early VS methods rely on all-in-memory designs
> for low latency, but scalability is constrained by memory capacity and cost. To
> address this, recent research has adopted heterogeneous architectures that offload
> space-intensive vectors and index structures to SSDs, while exploiting block
> locality and I/O-efficient strategies to maintain high search performance at
> billion scale. Looking ahead, the increasing demand for trillion-scale vector
> retrieval and cloud-native elasticity is driving a further shift toward
> memory-SSD-object storage architectures, which enable cost-efficient data tiering
> and seamless scalability."

**Caveat.** The abstract fetched does not itself state the specific claim
`docs/research/README.md` R1 attributes to "the 2026 survey" — that cluster indexes'
fetch granularity and lack of intra-query dependencies fit object storage where graph
beam search does not. That comparison may live in the paper's body; this vendoring
confirms the paper's existence, scope, and general thesis (the memory→SSD→object-
storage architectural shift) but not that specific sentence.

## Benchmark: `arxiv.org/abs/2511.14748`

**Title:** Cloud-Native Vector Search: A Comprehensive Performance Analysis
**Authors:** Zhaoheng Li, Wei Ding, Silu Huang, Zikang Wang, Yuanjin Lin, Ke Wu,
Yongjoo Park, Jianjun Chen

### Abstract (excerpt)

> "Vector search has been widely employed in recommender system and
> retrieval-augmented-generation pipelines, commonly performed with vector indexes to
> efficiently find similar items in large datasets."

The fetched content confirms this paper compares cluster-based and graph-based
indexes for cloud-native vector search over remote storage, analyzing bottlenecks and
performance across workload conditions — consistent with its role as R1's
"companion benchmark" — but the specific dependent-fetch-chain framing was not
independently re-quoted from this fetch.

**Update, 2026-08-19.** This paper's body (not just its abstract) has since been
fetched and searched directly, resolving R1's own "13.0 GB vs 7.5 GB index at
replica 8 vs 2 on GIST1M" and "up to 3.14× QPS" figures, both of which live here
(Table 4 and Figure 14, §5.3) — not in SPANN's own paper, which was independently
fetched in full and confirmed not to contain them. Full verbatim quotes, the table,
and the resolution are vendored in `references/spann-body-figures.md`.
