# Lance — Vector index format

Vendored summary, not a verbatim excerpt (the fetch tool summarized rather than
quoted the page directly; treat the quoted fragments below as verbatim, the rest as
paraphrase). Source: `lance.org/format/index/vector/`. Fetched 2026-08-18.

Cited by: `docs/research/README.md` R1, `docs/lineage.md` ("From Lance").

## Structure

Lance splits each vector index into three components: clustering (IVF via k-means),
sub-index organization (FLAT or HNSW graphs), and quantization (Product Quantization,
Scalar Quantization, RaBitQ, or FLAT).

> "All vector indices are stored as regular Lance files, making them portable and
> easy to manage."

The index is a dual-file design: an index file holding the search structure, and an
auxiliary file holding quantized vector storage.

> "During search, only the most relevant clusters are examined, dramatically reducing
> search time."

## Row-ID linkage

The auxiliary file maintains a `_rowid` column (uint64) alongside quantized vector
representations, preserving the link between compressed vectors and their original
row identifiers in the Lance table — the same shape STRAND's row-ID space generalizes
(invariant 1), independent of blob family.

## What this grounds

Confirms the index-aware, index-internals-agnostic manifest pattern `docs/lineage.md`
attributes to Lance, and the general precedent of storing a row-ID-linked auxiliary
file alongside quantized codes that R1's cluster-family blob design follows.
