# Ottaviano & Venturini — Partitioned Elias-Fano Indexes

Vendored excerpt, not the full paper. Source: Giuseppe Ottaviano and
Rossano Venturini, "Partitioned Elias-Fano Indexes," ACM SIGIR 2014
(DOI 10.1145/2600428.2609615). Fetched 2026-08-18 as PDF from the
first author's university page,
`http://groups.di.unipi.it/~ottavian/files/elias_fano_sigir14.pdf`,
text extracted with `pdftotext`. Short quotations reproduced for
citation and technical commentary, not a claim of a permissive license
on the full paper. Cited by `docs/research/r2-hybrid-codec-methodology.md`
for the space and query-time comparison between partitioned Elias-Fano
(PEF) and block-based PFOR-family postings codecs — and vendored
specifically because the whole-project convergence audit found the
methodology document carrying an ~11–12% figure with no in-repo source
and, on checking this paper, with the **direction reversed**.

**Precision note on codec families.** The paper's block-based baseline
is **OptPFD**, a PForDelta variant. It does not benchmark BP128 or
SIMD-BP128 themselves. Any claim in this repository that leans on these
numbers speaks about the PFOR family via OptPFD, not about BP128
directly.

---

From the abstract/introduction (space-time positioning):

> "Binary Interpolative Coding is only 2%-8% smaller but up to 5.5
> times slower; OptPFD is roughly 12% larger and almost always slower;
> Varint-G8IU is 10%-40% faster but more than 2.5 times larger."

Table 2 ("Overall space in gigabytes, and average bits per docId and
frequency"; percentages are relative to EF ε-optimal, the partitioned
variant):

| index        | Gov2 space (GB) | ClueWeb09 space (GB) |
| ------------ | --------------- | -------------------- |
| EF ε-optimal | 4.65            | 15.94                |
| OptPFD       | 5.22 (+12.3%)   | 17.80 (+11.6%)       |

So **OptPFD is 11.6–12.3% larger than partitioned Elias-Fano**, not the
other way around: on these collections, PEF holds the compression
advantage over the PFOR-family baseline.

On AND (conjunctive) query times, §5.2:

> "Compared to OptPFD, EF ε-optimal is 14% to 26% faster in all cases
> on general queries. The gap becomes even higher for selective
> queries, ranging from 34% to 40%."

On OR (disjunctive, full-scan) query times, §5.2, Table 3:

> "Unsurprisingly, as OR needs to scan the whole lists, block-based
> indexes perform better than Elias-Fano indexes, since they are
> optimized for raw decoding speed. However, the edge is not as high as
> one could expect, ranging from 7% to 17%."

(Table 3's OptPFD row specifically is 7–12% faster than EF ε-optimal
on OR queries; the 17% upper end includes the other block-based
baselines.)
