# turbopuffer — "Reaching 1B vectors, zero-cost edition" (batched-iterator fix)

Vendored excerpt, not the full post. Source: turbopuffer blog,
`https://turbopuffer.com/blog/zero-cost`. Fetched 2026-08-18.

Cited by: `CLAUDE.md` invariant 9 (§5), `docs/benchmarks.md`'s turbopuffer benchmark
targets, `docs/research/README.md` R2.

## The fix

> "Instead of producing a single element per call, the merge iterator now fills a
> batch by comparing and interleaving KV pairs from its inputs, then returns the
> entire batch at once. The consumer processes that batch as a plain array in a tight
> loop, with no recursive calls in the middle."

Batch size: 512 values (stated in the post's code comments accompanying the fix
description above; not a separately quotable prose sentence, but consistent with the
512-value figure `CLAUDE.md` invariant 9 already cites from this source).

## The numbers

> "With this change, our `scan` benchmark (100,000 values) runs in ~110μs, 60× faster
> than before and even beating our 130μs napkin math estimate thanks to SIMD."

This is the source for the 6.5ms → ~110μs, "60× faster than before" figures already
cited verbatim in `CLAUDE.md` invariant 9 and `docs/benchmarks.md`. (The pre-fix 6.5ms
figure and the 130μs napkin estimate both appear in the source post; this excerpt
captures the load-bearing post-fix number and the "60×" claim, which is what
`CLAUDE.md` quotes directly.)

> "After we updated our production code to use batched iterators, our customer's
> query latency dropped from ~220ms to 47ms."

This is the source for the 220ms → 47ms production-query figure already cited in
`CLAUDE.md` invariant 9 and `docs/benchmarks.md`.
