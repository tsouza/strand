# RaBitQ-Library — query-side distance estimator (source-level)

Vendored excerpt. Source: `github.com/VectorDB-NTU/RaBitQ-Library`,
`docs/docs/rabitq/estimator.md` and `include/rabitqlib/index/query.hpp`
(fetched live via `gh api repos/VectorDB-NTU/RaBitQ-Library/contents/...`,
2026-08-19, `main` branch HEAD at fetch time).

Cited by: `crates/strand-vector/src/estimate.rs`. Grounds the piece RFC
0010 Design §4 explicitly left out of the container layer's scope: how a
query vector, combined with a database vector's stored `f_add`/
`f_rescale`/`f_error` factors (`references/rabitq-library-one-bit-
quantization-source.md`), produces an estimated distance with an error
bound.

## The estimator formula (`estimator.md`, verbatim pseudocode)

```cpp
// Compute the estimated distance
// Note that G_add is dependent on the center vector.
float est_dist = F_add + G_add + F_rescale * (ip + G_kBxSumq)

// Compute the error bound
// Note that G_error is dependent on the center vector.
float error_bound = F_error * G_error

// Compute the lower and upper bounds of the estimated distance
float lb_dist = est_dist - error_bound
float ub_dist = est_dist + error_bound
```

`ip` is "the inner product between the binary code and the randomly
rotated query vector." One formula, shared by both registered metrics —
only `F_add`/`F_rescale`/`F_error` (encode-side, per-database-vector,
already implemented) and `G_add` (query-side, metric-dependent) differ.

`estimator.md`'s own factor table, condensed:

| Factor | L2 | Inner product |
| --- | --- | --- |
| `G_add` | `\|\|q_r - c\|\|^2` | `-<q_r, c>` |
| `G_error` | `\|\|q_r - c\|\|` | `\|\|q_r - c\|\|` (same) |
| `G_kBxSumq` (`G_k1xSumq` at bit-width 1) | `c_1 * S_q` | `c_1 * S_q` (same) |

where `c_1 = -((1<<1)-1)/2 = -0.5` — the identical constant
`crate::quantize`'s own `cb` specializes, shared by construction (both are
the bit-width-1 case of the same general `c_B = -((1<<B)-1)/2` formula).

## The real query-side factor computation (`query.hpp`, verbatim, `SplitBatchQuery`)

```cpp
explicit SplitBatchQuery(
    const T* rotated_query, size_t padded_dim, size_t ex_bits,
    MetricType metric_type = METRIC_L2, bool use_hacc = true
) : rotated_query_(rotated_query) {
    lookup_table_ = std::move(Lut<T>(rotated_query, padded_dim, use_hacc));
    metric_type_ = (metric_type == METRIC_IP) ? METRIC_IP : METRIC_L2;
    float c_1 = -static_cast<float>((1 << 1) - 1) / 2.F;
    float c_b = -static_cast<float>((1 << (ex_bits + 1)) - 1) / 2.F;
    T sumq = std::accumulate(rotated_query, rotated_query + padded_dim, static_cast<T>(0));
    G_k1xSumq_ = sumq * c_1;
    G_kbxSumq_ = sumq * c_b;
}

void set_g_add(T norm, T ip = 0) {
    if (metric_type_ == METRIC_L2) {
        G_add_ = norm * norm;
        G_error_ = norm;
    } else if (metric_type_ == METRIC_IP) {
        G_add_ = -ip;
        G_error_ = norm;
    }
}
```

**This settles a real ambiguity in the math notation, not just confirms
it.** `estimator.md`'s derivation writes the query term as `q_r' = P^{-1}
q_r` — a *reverse*-rotated query, defined in the same coordinate frame as
the stored binary code `x_u`. Read on its own, this could be misread as
requiring a second, inverse-rotation pipeline distinct from
`crate::rotate`'s forward transform. The real code above settles it:
every query-side class in `query.hpp` takes a parameter literally named
`rotated_query` and uses it directly — the constructor's own `Lut<T>
(rotated_query, padded_dim, ...)` and `sumq = accumulate(rotated_query,
...)` never rotate anything a second time. This is the *same* forward
`rotate()` applied to database vectors and centroids at index-build time
(already confirmed as the real construction order in
`references/rabitq-library-ivf-and-batch-layout-source.md`: "we first
rotate the centroid and vectors in this cluster... then compute the 1-bit
codes"). Because the forward rotation is orthogonal, `<x_u, P^{-1}q_r> =
<P x_u, q_r>` — building the lookup table from the forward-rotated query
and dotting it against the unrotated code bits computes the identical
mathematical quantity `estimator.md`'s notation describes, by a cheaper
route (one rotation per query, not one inverse rotation per candidate
compared against).

## What this grounds

The complete query-side estimator: the shared `est_dist`/bound formula,
the metric-dependent `G_add`, the metric-independent `G_error`/
`G_k1xSumq`, and — the load-bearing resolution — that no second, inverse
rotation exists anywhere in the real implementation. Not grounded: the
FastScan `accumulate()`/`pack_lut` LUT machinery itself (a SIMD-shaped
optimization of the same `ip` this file's formula already defines
directly; per invariant 9 the scalar computation is normative, so this
gap has no correctness consequence, only a performance one, named as
real, separate, unmeasured follow-on work); the multi-bit incremental
(`ex_bits`) extension `estimator.md`'s own "Incremental Distance
Estimation" section describes, out of scope per RFC 0010's own 1-bit-only
Non-goals.
