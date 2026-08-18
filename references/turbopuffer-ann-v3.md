# turbopuffer — "ANN v3" blog post (engine-global hierarchical index)

Vendored excerpt, not the full post. Source: turbopuffer blog,
`https://turbopuffer.com/blog/ann-v3`. Fetched 2026-08-18.

Cited by: `docs/benchmarks.md`'s turbopuffer benchmark targets ("Scale claims
(unverifiable, context only)"), `docs/research/README.md` R10.

## Scale and latency claims

> "100 billion vectors in a single search index" — "vector search over 200TiB of
> dense vector data"

> "200ms p99 query latency over 100 billion vectors" (headline claim)

Sustained throughput at that latency is on the order of 1,000 QPS ("serve a high rate
(> 1k QPS)"), with a stated theoretical ceiling around 10,000 QPS; this is distinct
from the platform-wide "25k+ queries/s" figure below, which spans all namespaces, not
one 100B-vector index.

## Platform-wide scale (footer claim, separate from the ANN v3 index-specific numbers)

> "turbopuffer is a fast search engine that hosts 1T+ documents, handles 10M+
> writes/s, and serves 25k+ queries/s"

Matches the "1T–2.5T documents, 10M+ writes/s, 25k+ queries/s fleet-wide" scale claims
already cited in `docs/benchmarks.md`, marked there as unverifiable, context-only —
this vendoring does not change that status; these remain turbopuffer's own unaudited
claims, not independently confirmed.

This is the capability ceiling `docs/research/README.md` R10 names as what a
format-level cross-segment pruning answer would be measured against — an
engine-global hierarchical index, which a manifest-level summary-blob mechanism is
explicitly not attempting to replicate at v0.1.
