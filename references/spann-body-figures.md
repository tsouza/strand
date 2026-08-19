# SPANN index-size figures: where they actually live

Fetched 2026-08-19, in response to RFC 0010's own Open questions item ("start with
the FastScan grounding fetch" led into this follow-on: fetching SPANN's real body
figures to replace the Napkin math section's provisional 1.73×/≈227 MB replication
estimate).

Cited by: `rfcs/0010-vector-blob-cluster-family.md` Napkin math ("Replication's
cost"); `docs/ledger.md` R1.

## Summary of the finding

The GIST1M index-size-vs-replication-factor figures RFC 0010 and
`docs/research/README.md` R1 depend on (13.0 GB vs 7.5 GB at replica 8 vs 2) are
**real and now independently re-confirmed from a paper's body** — but the paper is
**not** SPANN's own NeurIPS 2021 paper. It is the companion benchmark paper
`docs/research/README.md` R1 already cites separately (Li et al.,
`arxiv.org/abs/2511.14748`, already partially vendored in
`references/cloud-native-vector-search-surveys-2026.md`). SPANN's own paper was
fetched in full below and confirmed **not** to contain these figures at all — R1's
parenthetical wording ("13.0 GB vs 7.5 GB index at replica 8 vs 2 on GIST1M in the
benchmark") already said "in the benchmark," which turns out to have been the
correct attribution all along; RFC 0010's Napkin math section and Open questions
item mis-attributed the fetch target to SPANN's own PDF instead. Both papers are now
checked directly, not from memory, per `CLAUDE.md` §3.

## Part 1 — SPANN's own paper (`arxiv.org/abs/2111.08566`): fetched in full, no
## GIST1M and no index-size table

`arxiv.org/pdf/2111.08566` was fetched directly (the arXiv abstract page confirms
only one version exists: v1, submitted 2021-11-05, 127 KB source). The PDF was
converted to text (`pdftotext -layout`, 13 pages total: 11 pages of body plus
references) and searched exhaustively for `GIST`, `replica`, `index size`, `13.0`,
`7.5`, `congestion`, `QPS`, and `granularity`.

**Datasets used in this paper** (§5, "Experiment"), quoted verbatim:

> "1. SIFT1M dataset [3] is the most commonly used dataset generated from images for
> evaluating..."
> "2. SIFT1B dataset [3] is a classical dataset for evaluating the performance of
> ANNS algorithms..."
> "3. DEEP1B dataset [8] is a dataset learned from deep image classification model
> which contains..."
> "4. SPACEV1B dataset [6] (O-UDA license) is a dataset from commercial search
> engine which..."

GIST1M does not appear anywhere in this paper — not in the dataset list, not in any
table or figure caption, not in the text. The paper's own replication experiment
(§4.2, Figure 11, "Different numbers of closure replicas") reports **recall/latency
curves at 1, 4, 8, and 10 replicas**, with this text:

> "Figure 11 demonstrates the performance of different numbers of replicas for
> closure clustering assignment. From the result, we can see that using more than
> one replicas improves the performance significantly. However, when the number of
> replicas is larger than 8, the performance cannot be improved any more. Therefore,
> we choose 8 replicas for all of our experiments."

This confirms replica=8 as SPANN's own chosen default and confirms the paper tests
replica counts 1, 4, 8, 10 — but it reports **no index-size-in-bytes number
anywhere**, for any replica count, on any dataset. There is no `GIST1M`, no `13.0`,
no `7.5`, no `3.14`, no `congestion`, and no `QPS` string anywhere in the extracted
text. This is a genuine, confirmed absence, not a failed extraction: the earlier
abstract-only fetch's own caveat
(`references/spann-neurips2021.md`, "should be checked against the PDF body
directly") is now checked, and the answer is that the figures are not in this PDF at
all, at any confidence level.

## Part 2 — the real source: Li et al., `arxiv.org/abs/2511.14748`, Table 4 and
## Figure 14 (§5.3)

`arxiv.org/pdf/2511.14748` was fetched directly and converted to text
(`pdftotext -layout`; v2, `arXiv:2511.14748v2 [cs.DB] 5 Dec 2025`). Title: "Cloud-
Native Vector Search: A Comprehensive Performance Analysis [Experiments, Analysis
and Benchmark]." Authors: Zhaoheng Li, Wei Ding, Silu Huang, Zikang Wang, Yuanjin
Lin, Ke Wu, Yongjoo Park, Jianjun Chen (UIUC / ByteDance).

Dataset (Table 2, "Summary of Datasets for Evaluation"), quoted verbatim from the
extracted table row: `GIST1M [4]  1000000  960  FLOAT32  1000  Image` — 1,000,000
vectors, 960 dimensions, FLOAT32, image modality. Default SPANN indexing parameters
for GIST1M (Table 3): `centroid% = 16`, `replica# = 8`.

**Table 4, "Size metrics of SPANN configurations on GIST1M"** (§5.3), reproduced
verbatim from the extracted table:

| Configuration                        | Index size (GB) | No. lists | Avg. list size (KB) |
| ------------------------------------- | ---------------- | --------- | -------------------- |
| SPANN (centroid%=16, replica=8)       | 13.0              | 159K      | 166                   |
| SPANN (centroid%=16, replica=4)       | 10.5              | 159K      | 138                   |
| SPANN (centroid%=16, replica=2)       | 7.5               | 159K      | 99                    |
| SPANN (centroid%=32, replica=8)       | 14.0              | 271K      | 119                   |

No replica=1 row exists in this table, or anywhere else found in either paper — the
lowest replica count with a measured index size is replica=2. This is a real, stated
gap: neither paper measures an unreplicated (replica=1) SPANN index size directly.

The surrounding prose confirms the table is measuring real, built indexes, not
projected figures — quoted verbatim (§5.3, "SPANN: Replication Count Impacts Index
Quality"):

> "Another method for reducing SPANN's posting list sizes without increasing posting
> list count (i.e., BKT tree costs) is to decrease vector replication. We test two
> SPANN indexes with reduced vector replication counts of 4 and 2 in Fig 16: Despite
> data read per query being reduced at various nprobe values due to smaller posting
> list sizes (Table 4), these indexes do not necessarily result in better QPS-recall
> trade-offs... the replica=2 index requires 3-4× higher nprobe versus the replica=8
> index to reach the same recall at all recall levels (Fig 16c), ultimately resulting
> in more data read per query at the same recall (Fig 16a) and lower QPS (e.g., 2.00×
> more data read and 1.92× lower QPS vs. replica=8 @ 0.97 recall)."

**The 3.14× figure** is a QPS ratio tied to **centroid granularity**, not
replication — a different knob than the replica-count table above, quoted verbatim
(§5.3, "SPANN: More Centroids Benefit I/O-Congested Setups"):

> "Fig 14 reports the QPS ratio of an alternative SPANN index built with a higher
> centroid%=32 versus the default SPANN index built with centroid%=16. As
> hypothesized in §3, the former achieves QPS gains versus the latter on high recall
> and/or concurrency scenarios (up to 3.14×) as the centroid%=32 count index contains
> more posting lists each of significantly smaller size (by 32.1%, Table 4), which
> significantly reduces the amount of data read per query (1.47× at 0.995 recall,
> Fig 15a)."

The extracted per-cell ratio table backs the "up to 3.14×" claim directly: at
nprobe=32 (recall 80.33%) and concurrency=64, the extracted row reads
`32 (80.33%)  0.66  1.28  1.62  3.14` (columns are QPS ratios at concurrency 1, 4,
16, 64 respectively) — the `3.14` cell is the concurrency=64 ratio at this
nprobe/recall point, matching the "up to 3.14×" prose exactly.

This 3.14× figure is not used anywhere in RFC 0010 — `docs/research/README.md` R1
cites it as background on the granularity knob generally, and no cold-path
arithmetic in RFC 0010 depends on it. It is vendored here for completeness and
because it lives in the same table/section as the replication figures RFC 0010 does
depend on.

## What this resolves, precisely

`docs/research/README.md` R1's own parenthetical — "13.0 GB vs 7.5 GB index at
replica 8 vs 2 on GIST1M **in the benchmark**" — was correctly attributing these
figures to the benchmark paper all along; it was RFC 0010's own Napkin math section
and Open questions item that mis-targeted the re-fetch at SPANN's paper instead of
the benchmark paper. Both papers are now fetched in full (not abstract-only) and the
figures are confirmed real, quoted verbatim from a table, with the exact section
(§5.3, Table 4) and dataset configuration (GIST1M, 1M × 960d FLOAT32,
centroid%=16) named. The **1.73× ratio** RFC 0010 already computed (13.0 / 7.5) is
arithmetically unchanged and now rests on a body-table citation instead of an
abstract-only, explicitly-flagged-unverified one. What remains genuinely
unavailable, in either paper, is a replica=1 (no closure replication) index-size
figure — applying the replica-8/replica-2 ratio to RFC 0010's own replica-1 tier-1
baseline is still an extrapolation across a step neither paper measured directly,
and RFC 0010's own text already flags this as a conservative lower bound rather than
a replica-8 prediction; that caveat stands unchanged by this fetch.
