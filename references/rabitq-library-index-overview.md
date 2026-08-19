# RaBitQ-Library — docs index page (distance metrics, recall figures, adopters)

Vendored excerpt. Source: `github.com/VectorDB-NTU/RaBitQ-Library`,
`docs/docs/index.md` (fetched live via `gh api
repos/VectorDB-NTU/RaBitQ-Library/contents/docs/docs/index.md`, 2026-08-19,
`main` branch HEAD at fetch time).

Cited by: RFC 0010 (`rfcs/0010-vector-blob-cluster-family.md`). An earlier
drafting pass of that RFC referenced this page's content without vendoring
it as its own file; this file corrects that.

## Distance metrics (verbatim)

"The RaBitQ Library supports estimating similarity metrics including
Euclidean distance, inner product and cosine similarity."

Grounds RFC 0010's `distance_metric` enum (`0` = L2, `1` = inner product,
`2` = cosine).

## Bit-width recall figures (verbatim)

"Using **4-bit, 5-bit and 7-bit** quantization usually suffices to produce
**90%, 95% and 99% recall** respectively without reranking."

This independently confirms, from the reference library's own current
documentation rather than the Extended-RaBitQ paper's abstract alone, the
figure `docs/research/README.md` R3 states and that
`references/rabitq-and-extended-rabitq.md` explicitly flagged as "not found
in this fetch's excerpt" (that fetch was abstract-level only). This figure
concerns the multi-bit Extended-RaBitQ path, which RFC 0010 does not
register (Non-goals) — it is cited here to resolve the previously-flagged
grounding gap, not because RFC 0010's own v0.1 (1-bit) design depends on it.

## Adopters (verbatim list)

"The RaBitQ algorithm has been implemented in many real-world systems in
industry including

- Milvus - IVF + RaBitQ (C++)
- Faiss - IVF + RaBitQ (C++)
- VSAG - HGraph + RaBitQ (C++)
- VectorChord - IVF + RaBitQ (Rust)
- Volcengine OpenSearch - DiskANN + RaBitQ
- CockroachDB - CSPANN + RaBitQ (Golang)
- ElasticSearch - HNSW + RaBitQ (Java - the algorithm is adopted with some
  minor modifications and renamed as "BBQ")
- Lucene - HNSW + RaBitQ (Java - the algorithm is adopted with some minor
  modifications and renamed as "BBQ")"

Matches and directly re-confirms, from a second independent source, the
adopter list `docs/research/README.md` R3 and `docs/ledger.md`'s license
audit already state.
