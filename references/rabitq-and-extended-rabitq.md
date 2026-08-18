# RaBitQ / Extended-RaBitQ — paper and reference-implementation license audit

Vendored excerpt. Fetched 2026-08-18.

Cited by: `docs/research/README.md` R3, `CLAUDE.md` §1 ("The RaBitQ reference
implementations were license-audited in R3").

## Extended-RaBitQ paper

**Source:** `arxiv.org/abs/2409.09913`.

**Title:** Practical and Asymptotically Optimal Quantization of High-Dimensional
Vectors in Euclidean Space for Approximate Nearest Neighbor Search
**Authors:** Jianyang Gao, Yutong Gou, Yuexuan Xu, Yongyi Yang, Cheng Long, Raymond
Chi-Wing Wong

> "The new method inherits the theoretical guarantees of RaBitQ and achieves the
> asymptotic optimality in terms of the trade-off between space and error bounds."

The specific claim `docs/research/README.md` R3 makes about 4/5/7-bit quantization
reaching 90/95/99% recall without reranking was not found in this fetch's excerpt
(abstract-level only) — flagged as still-unverified, not re-confirmed by this
vendoring pass.

## Reference-implementation license audit (byte-exact, via GitHub's license API)

| Repository | SPDX license | Confirmed via |
| --- | --- | --- |
| `github.com/VectorDB-NTU/RaBitQ-Library` | Apache-2.0 | GitHub license API (`gh api repos/VectorDB-NTU/RaBitQ-Library/license`), 2026-08-18 |
| `github.com/gaoj0017/RaBitQ` | Apache-2.0 | GitHub license API (`gh api repos/gaoj0017/RaBitQ/license`), 2026-08-18 |

This confirms Apache-2.0 for both repositories `docs/research/README.md`'s source
list actually names, via GitHub's own reported SPDX identifier (computed from the
repository's real LICENSE file) rather than the "GitHub's license detection plus
third-party corroboration" the original audit described — this is a stronger check
than that caveat implies, and resolves the "byte-for-byte header reads were blocked"
caveat for these two repositories.

`github.com/gaoj0017/RaBitQ`'s own README states the repository has been superseded
and moved to `RaBitQ-Library`; it remains live and Apache-2.0-licensed as an archived
snapshot.

## A citation-count discrepancy found during this vendoring pass

`CLAUDE.md` §1 states "all three repositories are Apache-2.0," and
`docs/research/README.md` R3 repeats "all three reference repositories are
Apache-2.0" — but R3's own Sources line names only two:
`github.com/VectorDB-NTU/RaBitQ-Library` and `github.com/gaoj0017/RaBitQ`. No third
repository is named anywhere in either document. This vendoring pass could confirm
only the two actually named; the "three" count is either a stale figure from an
earlier draft that dropped a source, or was simply wrong from the start. Recorded in
`docs/ledger.md` rather than silently corrected, since the third repository's
identity — if one was ever intended — is not recoverable from this repo's own text.
