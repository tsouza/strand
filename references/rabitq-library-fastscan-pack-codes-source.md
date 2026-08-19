# RaBitQ-Library — FastScan `pack_codes`: the intra-batch bit/lane order (source-level)

Vendored excerpt. Source: `github.com/VectorDB-NTU/RaBitQ-Library`,
`include/rabitqlib/fastscan/fastscan.hpp` and
`include/rabitqlib/quantization/rabitq_impl.hpp` (fetched live via `gh api
repos/VectorDB-NTU/RaBitQ-Library/contents/...`, 2026-08-19, `main` branch
HEAD at fetch time).

Cited by: RFC 0010 (`rfcs/0010-vector-blob-cluster-family.md`) Discussion —
post-approval amendment. This closes the gap that RFC's own Design §4,
Non-goals, "How this could be wrong," and Open questions all named
explicitly: `references/rabitq-library-ivf-and-batch-layout-source.md`
grounds the *byte offsets* of a FastScan batch (code region vs. the three
factor arrays) but not the *bit-level* layout of the 1-bit codes within
the code region itself. This file grounds that remaining piece.

## Confirmed call site: this is the 1-bit path, not a different codec

`rabitq_impl.hpp`'s `one_bit_batch_code` (namespace `rabitqlib::quant::
rabitq_impl::one_bit`) is the function that produces exactly the byte
layout `BatchDataMap::bin_code()` addresses. It first computes plain,
sequential, one-bit-per-dimension `compact_codes` (`num * padded_dim / 8`
bytes, via `one_bit_compact_codes`/`one_bit_compact_code` — the actual
RaBitQ quantization arithmetic that decides which bit is set, out of scope
for STRAND's wire-format RFC, exactly as already stated), then calls:

```cpp
fastscan::pack_codes(padded_dim, compact_codes.data(), num, packed_code);
```

`one_bit_batch_code`'s own doc comment states `// ! padded_dim % 64 == 0` —
a real requirement of this specific function, not merely a convenience
`FhtKacRotator`'s own padding rule happens to satisfy. RFC 0010 already
requires `padded_dims` to be a multiple of 64 for **both** registered
rotator types (Design §2, for alignment reasons); this confirms that
requirement is also load-bearing for the registered 1-bit packing codec
itself, not only a STRAND-side alignment convenience.

## `fastscan::pack_codes` — full source (`fastscan.hpp`)

```cpp
constexpr static size_t kBatchSize = 32;  // number of vectors in each batch

constexpr static std::array<int, 16> kPerm0 = {
    0, 8, 1, 9, 2, 10, 3, 11, 4, 12, 5, 13, 6, 14, 7, 15
};  // data order of packed quantization code

template <typename T, class TA>
static inline void get_column(
    const T* src, size_t rows, size_t cols, size_t row, size_t col, TA& dest
) {
    size_t k = 0;
    size_t max_k = std::min(rows - row, dest.size());
    for (; k < max_k; ++k) {
        dest[k] = src[((k + row) * cols) + col];
    }
    if (k < dest.size()) {
        std::fill(dest.begin() + k, dest.end(), 0);
    }
}

inline void pack_codes(
    size_t padded_dim, const uint8_t* quantization_code, size_t num, uint8_t* blocks
) {
    size_t num_rd = (num + 31) & ~31;  // round up num of vecs to multiple of batch size(32)
    size_t cols = padded_dim / 8;

    std::array<uint8_t, 32> col;
    std::array<uint8_t, 32> col_0;  // upper 4 bits
    std::array<uint8_t, 32> col_1;  // lower 4 bits

    for (size_t row = 0; row < num_rd; row += kBatchSize) {
        for (size_t i = 0; i < cols; ++i) {
            get_column(quantization_code, num, cols, row, i, col);
            for (size_t j = 0; j < 32; ++j) {
                col_0[j] = col[j] >> 4;
                col_1[j] = col[j] & 15;
            }
            for (size_t j = 0; j < 16; ++j) {
                uint8_t val0 = col_0[kPerm0[j]] | (col_0[kPerm0[j] + 16] << 4);
                uint8_t val1 = col_1[kPerm0[j]] | (col_1[kPerm0[j] + 16] << 4);
                blocks[j] = val0;
                blocks[j + 16] = val1;
            }
            blocks += 32;
        }
    }
}
```

Attributed in the source itself to Faiss's own FastScan documentation
("The implementation is largely based on the implementation of Faiss" —
`https://github.com/facebookresearch/faiss/wiki/Fast-accumulation-of-PQ-and-AQ-codes-(FastScan)`).

## The algorithm, stated precisely

Input: `compact_codes`, a conceptual `num × cols` byte matrix (`cols =
padded_dim / 8`), row-major, one row per vector — the plain, sequential
one-bit-per-dimension code `one_bit_compact_code` produces. This
intermediate representation is **not** part of STRAND's wire format; only
`pack_codes`'s output is.

For each batch of up to 32 vectors (rows `row .. row+31`, zero-filled for
any row ≥ `num` — `get_column`'s own `std::fill(..., 0)` when
`rows - row < dest.size()`, i.e. **the reference implementation itself
zero-fills a partial final batch's absent vector rows before packing**,
independently confirming the zero-fill rule RFC 0010 Design §4 already
requires, not merely inventing a compatible convention):

For each byte-column `i` in `0 .. cols`:
1. `col[v] = compact_codes[row+v][i]` for `v` in `0..32` (the `i`-th code
   byte of each of this batch's 32 vector slots, zero for absent slots).
2. Split each into nibbles: `hi[v] = col[v] >> 4`, `lo[v] = col[v] & 0xF`.
3. For `j` in `0..16`, using the fixed permutation
   `kPerm0 = [0, 8, 1, 9, 2, 10, 3, 11, 4, 12, 5, 13, 6, 14, 7, 15]`:
   - output byte at position `i*32 + j` = `hi[kPerm0[j]] | (hi[kPerm0[j]+16] << 4)`
   - output byte at position `i*32 + 16 + j` = `lo[kPerm0[j]] | (lo[kPerm0[j]+16] << 4)`

Each column contributes exactly 32 output bytes (16 from the `hi` loop, 16
from the `lo` loop), and there are `cols = padded_dim/8` columns per
batch, so the batch's code region is exactly `cols * 32 = padded_dim * 4`
bytes — matching, byte-for-byte, RFC 0010's own already-derived
`padded_dims * 32 / 8 = padded_dims * 4` byte formula for this region
(`references/rabitq-library-ivf-and-batch-layout-source.md`). This fetch
adds the missing byte *order* within that already-correct byte *count* —
it does not change any RFC 0010 arithmetic, only completes what was
previously adopted by reference without being independently pinned.

## What this grounds

The complete, byte-exact intra-batch bit/lane order for STRAND's 1-bit
code region (`spec/vectors.md` §4), closing the gap RFC 0010's own Design
§4, Non-goals, "How this could be wrong" (finding "Third"), Invariant-11
checklist, and Open questions all named explicitly as unresolved at
Approval time. Also independently confirms RFC 0010's own zero-fill
padding-determinism rule matches the reference implementation's actual
behavior for a partial final batch, not merely a STRAND-invented
compatible convention.
