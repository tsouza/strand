# RaBitQ-Library — IVF data layout, FastScan batching, and byte formulas (source-level)

Vendored excerpt. Source: `github.com/VectorDB-NTU/RaBitQ-Library`,
`docs/docs/index/ivf.md`, `include/rabitqlib/fastscan/fastscan.hpp`, and
`include/rabitqlib/quantization/data_layout.hpp` (fetched live via `gh api
repos/VectorDB-NTU/RaBitQ-Library/contents/...`, 2026-08-19, `main` branch
HEAD at fetch time).

Cited by: RFC 0010 (`rfcs/0010-vector-blob-cluster-family.md`), for the
cluster posting-list blob's quantized-code region byte formula. This file
is distinct from `references/rabitq-library-compact-code-source.md`, which
vendors `docs/docs/compact_code.md` and documents a **different, per-vector,
non-batched** code layout — RaBitQ-Library ships both: a simple per-vector
packed format (`pack_excode.hpp`, for the multi-bit ex-code region) and a
FastScan-batched format (`data_layout.hpp`, for the 1-bit code region, used
by the IVF index). RFC 0010 registers the batched format for its 1-bit
posting-list region and the per-vector format for nothing in v0.1 (multi-bit
is out of scope) — an earlier drafting pass of that RFC conflated the two
files' content, citing the per-vector file for the batched formula; this
file corrects that by vendoring the batched-format source directly.

## IVF top-level data layout (`docs/docs/index/ivf.md`, verbatim except as noted)

"The main data layout for our IVF is organized as follows:

```
[batch data]    // 1-bit code and factors
[ex_data]       // code for remaining bits
[ids]           // PID of vectors (organized by clusters)
[cluster_lst]   // List of clusters' metadata in IVF
```"

Four flat, top-level regions — all clusters' batch data concatenated, then
all clusters' ex-data, then all ids (grouped by cluster), then the
directory — optimized for a fully RAM-resident index where any region is
randomly addressable at zero extra cost. RFC 0010 deliberately does not
mirror this exact split (its own Design §4 states why: co-locating each
cluster's own codes and ids cuts the cold-fetch GET count in half compared
to this in-memory-optimized layout).

Also from the same page, the recommended cluster count: "we recommend to
tune cluster_num around 4 * the square root of the dataset following
Faiss."

## FastScan batch size (`fastscan.hpp`)

```cpp
constexpr static size_t kBatchSize = 32;  // number of vectors in each batch
```

## Per-batch and per-vector byte formulas (`data_layout.hpp`, read directly from source)

`BatchDataMap<T>` (the 1-bit code + factors layout addressed by
`[batch data]` above) constructs its internal pointers as: `bin_code` at
byte 0; `f_add` (T, `kBatchSize` elements) starting at byte
`padded_dim * kBatchSize / 8`; `f_rescale` immediately after `f_add`
(`kBatchSize` more T elements); `f_error` immediately after `f_rescale`
(`kBatchSize` more T elements). Its own `data_bytes()`:

```cpp
static size_t data_bytes(size_t padded_dim) {
    return (padded_dim * fastscan::kBatchSize / 8) +
           (sizeof(T) * fastscan::kBatchSize * 3);
}
```

At `T = float` (4 bytes) and `kBatchSize = 32`: `data_bytes(padded_dim) =
padded_dim * 4 + 384` bytes per batch, laid out as `[1-bit codes for up to
32 vectors, padded_dim * 32 / 8 bytes][f_add: 32 × f32][f_rescale: 32 ×
f32][f_error: 32 × f32]` — three per-vector distance-correction factors per
batch slot, 12 bytes/vector amortized over a full batch, that
`references/rabitq-library-compact-code-source.md`'s per-vector-only
formula does not include at all.

`ExDataMap<T>` (the `[ex_data]` region, multi-bit only, out of RFC 0010's
v0.1 scope):

```cpp
static size_t data_bytes(size_t padded_dim, size_t ex_bits) {
    return ex_bits > 0 ? (padded_dim * ex_bits / 8) + (sizeof(T) * 2) : 0;
}
```

Per-vector (not batched): `ex_code` bytes plus two f32 factors
(`f_add_ex`, `f_rescale_ex`).

Total-cluster allocation, confirming batches are counted, not vectors,
against `IVF::batch_data_bytes()`:

```cpp
size_t batch_data_bytes(const std::vector<size_t>& cluster_sizes) const {
    size_t total_blocks = 0;
    for (auto size : cluster_sizes) {
        total_blocks += div_round_up(size, fastscan::kBatchSize);
    }
    return total_blocks * BatchDataMap<float>::data_bytes(padded_dim_);
}
```

A cluster whose size is not a multiple of `kBatchSize` still contributes a
full extra block at full `data_bytes()` cost for its remainder — the
reference implementation's own behavior, not something RFC 0010 introduces.
The source read in this fetch does not show how the unused lanes of that
final partial block are filled (zero-fill vs. duplicate vs. undefined);
RFC 0010 pins this normatively as a STRAND-format requirement (zero-fill),
since it is not resolved by this vendored source.

## What this grounds

The `padded_dims * 4 + 384` per-batch-of-32 byte formula RFC 0010's Design
§4 and Napkin math sections use, the `kBatchSize = 32` wire-format constant,
and the `4·√N` cluster-count convention used in the sizing-law arithmetic —
all three now cited to this file, not to
`references/rabitq-library-compact-code-source.md`, which documents an
unrelated per-vector layout.
