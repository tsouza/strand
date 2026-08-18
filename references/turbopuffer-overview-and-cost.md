# turbopuffer — Product overview (p90 figures, cost comparison)

Vendored excerpt, not the full page. Source: turbopuffer blog / product overview,
`https://turbopuffer.com/blog/turbopuffer`. Fetched 2026-08-18.

Cited by: `docs/benchmarks.md`'s turbopuffer benchmark targets ("Published p90s from
their benchmark post").

## Published p90 latency figures

For 1M vectors, 768 dimensions (3GB):
- Cold queries: **444ms p90**
- Warm queries: **10ms p90**

For 1M documents, BM25 full-text (300MB):
- Cold queries: **285ms p90**
- Warm queries: **18ms p90**

These match the four p90 figures already cited in `docs/benchmarks.md`'s "Published
p90s from their benchmark post" line.

## Architecture framing (context, not independently verified against primary source
beyond this page)

> "it is an object-storage-first storage engine where object storage is the source of
> truth (LSM)."

> "each search namespace is simply a prefix on object storage"

The design optimizes "to limit it to a maximum of three roundtrips for sub-second cold
latency" on vector queries — consistent with the architecture page's 3-roundtrip
structured cold path (`references/turbopuffer-architecture.md`).

## Cost comparison (context only, not a load-bearing STRAND claim)

The page states an approximate 50× storage-cost advantage over RAM-resident
incumbents, illustrated as roughly $70/TB/month (S3 + SSD cache) versus roughly
$3600/TB/month (RAM + 3x SSD replication). Not cited elsewhere in this repo's
normative text; recorded here only because it appeared on the fetched page.
