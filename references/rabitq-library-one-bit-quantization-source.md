# RaBitQ-Library — one-bit quantization math and factor formulas (source-level)

Vendored excerpt. Source: `github.com/VectorDB-NTU/RaBitQ-Library`,
`include/rabitqlib/quantization/rabitq_impl.hpp` and
`include/rabitqlib/utils/space.hpp` (fetched live via `gh api
repos/VectorDB-NTU/RaBitQ-Library/contents/...`, 2026-08-19, `main` branch
HEAD at fetch time).

Cited by: `crates/strand-vector/src/quantize.rs`. Grounds the actual
RaBitQ quantization math — the sign-based binary-code selection and the
`f_add`/`f_rescale`/`f_error` distance-correction factor formulas — that
RFC 0010 (`rfcs/0010-vector-blob-cluster-family.md`) explicitly left out of
its own container-layer scope ("their role in RaBitQ's distance estimator
is the algorithm's concern, not this container-layer RFC's," Design §4).
This file is that algorithm's concern, grounded properly.

## Precondition: inputs are already rotated

This function operates on `data`/`centroid` that have **already** had
rotation applied — confirmed by the IVF construction path's own comment
(`references/rabitq-library-ivf-and-batch-layout-source.md`'s sibling
fetch of `docs/docs/index/ivf.md`): "we first rotate the centroid and
vectors in this cluster... then compute the 1-bit codes." Rotation
*application* (as opposed to rotation *payload storage*, which
`crates/strand-vector/src/descriptor.rs` already handles) is real,
separate, ungrounded-by-this-fetch work.

## `one_bit_code` / `one_bit_code_with_factor` (`rabitq_impl.hpp`, verbatim)

```cpp
constexpr float kConstEpsilon = 1.9;

template <typename T>
inline RowMajorArray<T> one_bit_code(
    const T* data, const T* centroid, size_t dim, int* binary_code
) {
    ConstRowMajorArrayMap<T> data_arr(data, 1, dim);
    ConstRowMajorArrayMap<T> cent_arr(centroid, 1, dim);
    RowMajorArray<T> residual_arr = data_arr - cent_arr;
    RowMajorArrayMap<int> x_u(binary_code, 1, static_cast<long>(dim));
    x_u = (residual_arr > 0).template cast<int>();
    return residual_arr;
}

template <typename T>
inline void one_bit_code_with_factor(
    const T* data, const T* centroid, size_t dim, int* binary_code,
    T& f_add, T& f_rescale, T& f_error, MetricType metric_type = METRIC_L2
) {
    RowMajorArray<T> residual_arr = one_bit_code(data, centroid, dim, binary_code);

    float cb = -((1 << 1) - 1) / 2.F;  // = -0.5, the bit_width=1 case
    RowMajorArrayMap<int> x_u(binary_code, 1, static_cast<long>(dim));
    RowMajorArray<T> xu_cb = x_u.template cast<T>() + cb;

    T l2_sqr = l2norm_sqr<T>(residual_arr.data(), dim);
    T l2_norm = std::sqrt(l2_sqr);

    T ip_resi_xucb = dot_product<T>(residual_arr.data(), xu_cb.data(), dim);
    T ip_cent_xucb = dot_product<T>(centroid, xu_cb.data(), dim);

    if (ip_resi_xucb == 0) {
        ip_resi_xucb = std::numeric_limits<T>::infinity();
    }

    T tmp_error =
        l2_norm * kConstEpsilon *
        std::sqrt(
            (((l2_sqr * l2norm_sqr<T>(xu_cb.data(), dim)) / (ip_resi_xucb * ip_resi_xucb)) - 1) /
            (dim - 1)
        );

    if (metric_type == METRIC_L2) {
        f_add = l2_sqr + (2 * l2_sqr * ip_cent_xucb / ip_resi_xucb);
        f_rescale = -2 * l2_sqr / ip_resi_xucb;
        f_error = 2 * tmp_error;
    } else if (metric_type == METRIC_IP) {
        f_add = 1 - dot_product<T>(residual_arr.data(), centroid, dim) +
                (l2_sqr * ip_cent_xucb / ip_resi_xucb);
        f_rescale = -l2_sqr / ip_resi_xucb;
        f_error = 1 * tmp_error;
    }
}
```

`one_bit_compact_code` (the entry point `one_bit_batch_code` — the real
call site confirmed in `references/rabitq-library-ivf-and-batch-layout-
source.md` — actually calls) is a thin wrapper: compute `binary_code` via
`one_bit_code_with_factor` above, then `pack_binary(binary_code.data(),
compact_code, padded_dim)`.

## `pack_binary`, `l2norm_sqr`, `dot_product` (`space.hpp`, verbatim)

```cpp
template <typename T>
inline T l2norm_sqr(const T* __restrict__ vec0, size_t dim) {
    ConstVectorMap<T> v0(vec0, dim);
    return v0.dot(v0);
}

template <typename T>
inline T dot_product(const T* __restrict__ vec0, const T* __restrict__ vec1, size_t dim) {
    ConstVectorMap<T> v0(vec0, dim);
    ConstVectorMap<T> v1(vec1, dim);
    return v0.dot(v1);
}

// pack 0/1 data to usigned integer
template <typename T>
inline void pack_binary(
    const int* __restrict__ binary_code, T* __restrict__ compact_code, size_t length
) {
    constexpr size_t kTypeBits = sizeof(T) * 8;
    for (size_t i = 0; i < length; i += kTypeBits) {
        T cur = 0;
        for (size_t j = 0; j < kTypeBits; ++j) {
            cur |= (static_cast<T>(binary_code[i + j]) << (kTypeBits - 1 - j));
        }
        *compact_code = cur;
        ++compact_code;
    }
}
```

`l2norm_sqr` and `dot_product` are plain sum-of-products (Eigen's
`.dot()` on a flat vector map) — no hidden normalization. `pack_binary` at
`T = uint8_t` (the type `one_bit_compact_code` actually uses) packs each
group of 8 consecutive `binary_code` entries into one byte, **MSB-first**:
`binary_code[i]` (the group's first entry) lands at bit 7, `binary_code[i+7]`
at bit 0.

## Independent verification: a standalone C++ reimplementation, compiled and run

This session did not merely transcribe the above — it wrote a
self-contained, dependency-free C++ program (plain loops, no Eigen)
implementing the identical formula, compiled it (`g++ -O2`), and ran it
against three test cases (dim 8 and 16, both `METRIC_L2` and `METRIC_IP`).
The Rust transcription in `crates/strand-vector/src/quantize.rs` is tested
against those real, executed outputs
(`quantize::tests::matches_the_reference_implementation_case1_l2` and
siblings) — not merely asserted to match by construction. One example,
hand-traced and confirmed against the executed output: `data = [1.0, -2.0,
3.5, 0.5, -1.5, 2.0, -0.25, 4.0]`, `centroid = [0.5, -1.0, 2.0, 1.0, -1.0,
1.5, 0.0, 3.0]` (both dim 8) → `residual = [0.5, -1, 1.5, -0.5, -0.5, 0.5,
-0.25, 1]` → `binary_code = [1,0,1,0,0,1,0,1]` → packed MSB-first =
`0b10100101 = 0xA5`, `f_add ≈ 20.0951`, `f_rescale ≈ -3.6957`, `f_error ≈
1.7687` (`METRIC_L2`).

## What this grounds, and what remains open

Grounds: the exact sign-based binary-code rule, the exact `f_add`/
`f_rescale`/`f_error` formulas for both registered metrics, and the exact
intra-byte bit order of the compact code `crate::fastscan::pack_codes`
then consumes as its own input. **Not** grounded by this fetch: rotation
*application* (the FFHT+Kac's-Walk or matrix-multiply transform itself,
as opposed to the rotation payload's storage format, already handled by
`crate::descriptor`); the query-side distance *estimator* that consumes
`f_add`/`f_rescale`/`f_error` against a query vector (FastScan's
`accumulate()` plus the higher-level formula built on top of it); k-means
clustering; and the multi-bit Extended-RaBitQ path (`ex_bits` — RFC 0010
Non-goals, unchanged).
