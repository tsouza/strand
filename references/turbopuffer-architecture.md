# turbopuffer — Architecture docs (cold-query round-trip structure)

Vendored excerpt, not the full page. Source: turbopuffer architecture documentation,
`https://turbopuffer.com/architecture`. Fetched 2026-08-18.

Cited by: `CLAUDE.md` §7 (the napkin-math rule), `docs/benchmarks.md`'s turbopuffer
benchmark targets, RFC 0001's napkin-math section, `docs/research/README.md` R1.

## The planning figure

> "From first principles, each roundtrip to object storage takes ~100ms."

This is the source for the ~100ms per object-storage round trip figure `CLAUDE.md` §7
pins as "the planning figure."

## Cold-open p50/cached p50

> "The first query to a namespace reads object storage directly and is slow (p50=874ms
> for 1M documents) [...] subsequent, cached queries to that node are faster
> (p50=14ms for 1M documents)."

This is the source for the measured p50 = 874ms true-cold figure `docs/benchmarks.md`
already cites and explicitly treats as the number STRAND's cold-open story competes
with — not the smaller ~400ms structured-path budget below, which is a first-
principles estimate, not a measured p50.

## The structured cold-query path

The documentation describes a three-roundtrip structured path for a cold query:
"Roundtrip 1" fetches metadata; "Roundtrip 2" accesses the filter index, centroid
index, and unindexed WAL; "Roundtrip 3" retrieves clusters.

> "The 3–4 required roundtrips for a cold query often take as little as ~400ms."

This is the source for the "often as little as ~400ms" structured-path figure
`docs/benchmarks.md` already cites and explicitly distinguishes from the measured
874ms true-cold p50 (a different source, turbopuffer's benchmark post, separately
vendored) — `docs/benchmarks.md` states the 874ms measured figure, not this
first-principles ~400ms budget, is the number STRAND's cold-open story should be
judged against.

## Cached-query figure (context only, not part of the cold-path claim)

> "When the namespace is cached in NVME/memory rather than fetched directly from
> object storage, the query time drops dramatically to p50=14 [ms]."

Matches the p50 = 14ms cached-query figure already cited in `docs/benchmarks.md`.
