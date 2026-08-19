# RaBitQ-Library — FastScan `accumulate()`: the SIMD decode kernels (source-level)

Vendored excerpt. Source: `github.com/VectorDB-NTU/RaBitQ-Library`,
`src/simd/fastscan_avx2.cpp`, `src/simd/fastscan_avx512.cpp`,
`include/rabitqlib/simd/fastscan_dispatch.hpp`, and `src/simd/dispatch.cpp`
(fetched live via `gh api repos/VectorDB-NTU/RaBitQ-Library/contents/...`,
2026-08-19, `main` branch HEAD at fetch time). `include/rabitqlib/fastscan/
fastscan.hpp` is re-quoted here only where needed for cross-reference; its
full text is already vendored in
`references/rabitq-library-fastscan-pack-codes-source.md`.

Cited by: RFC 0010 (`rfcs/0010-vector-blob-cluster-family.md`) "How this
could be wrong" and Open questions — both sections named this exact gap:
the earlier pack_codes fetch found the ISA-independent packing algorithm
but not the actual SIMD `accumulate()` decode kernels whose register-width
choices would settle whether `kBatchSize = 32` is algorithm-shaped or has
residual hardware provenance. This file closes that gap.

## Where the files actually live

The declarations RFC 0010's Open questions named
(`include/rabitqlib/simd/fastscan_avx2.cpp`) do not exist at that path —
`include/rabitqlib/simd/` holds only dispatch *headers*
(`fastscan_dispatch.hpp`), declaring the functions with no bodies. The
definitions live under `src/simd/fastscan_avx2.cpp` and
`src/simd/fastscan_avx512.cpp`. Confirmed by listing both directories
live (`gh api .../contents/include/rabitqlib/simd` and
`gh api .../contents/src/simd`) before fetching, rather than guessing the
corrected path.

## The dispatch header (`include/rabitqlib/simd/fastscan_dispatch.hpp`, full text)

```cpp
#pragma once

#include <cstddef>
#include <cstdint>

namespace rabitqlib::fastscan::simd {

void accumulate_avx2(
    const uint8_t* __restrict__ codes,
    const uint8_t* __restrict__ lp_table,
    uint16_t* __restrict__ result,
    size_t dim
);
void transfer_lut_hacc_avx2(const uint16_t* lut, size_t dim, uint8_t* hc_lut);
void accumulate_hacc_avx2(
    const uint8_t* __restrict__ codes,
    const uint8_t* __restrict__ hc_lut,
    int32_t* accu_res,
    size_t dim
);

void accumulate_avx512(
    const uint8_t* __restrict__ codes,
    const uint8_t* __restrict__ lp_table,
    uint16_t* __restrict__ result,
    size_t dim
);
void transfer_lut_hacc_avx512(const uint16_t* lut, size_t dim, uint8_t* hc_lut);
void accumulate_hacc_avx512(
    const uint8_t* __restrict__ codes,
    const uint8_t* __restrict__ hc_lut,
    int32_t* accu_res,
    size_t dim
);

}  // namespace rabitqlib::fastscan::simd
```

The decisive detail is already visible here: `accumulate_avx2` and
`accumulate_avx512` share **the exact same signature** —
`(codes, lp_table, uint16_t* result, size_t dim)` — with no batch-size
parameter and no ISA-conditional result width. Whatever batch size each
produces, it is fixed by the caller's contract, not by which one gets
compiled or dispatched.

## The runtime dispatch (`src/simd/dispatch.cpp`, relevant excerpt)

```cpp
namespace rabitqlib::fastscan {

using AccumulateFn = void (*)(const uint8_t*, const uint8_t*, uint16_t*, size_t);
const AccumulateFn kAccumulateFn = [] {
    if (cpu::has_avx512_core()) {
        return simd::accumulate_avx512;
    } else if (cpu::has_avx2()) {
        return simd::accumulate_avx2;
    } else {
        rabitqlib::simd::missing_feature("fastscan accumulate");
    }
}();

void accumulate(
    const uint8_t* __restrict__ codes,
    const uint8_t* __restrict__ lp_table,
    uint16_t* __restrict__ result,
    size_t dim
) {
    kAccumulateFn(codes, lp_table, result, dim);
}

}  // namespace rabitqlib::fastscan
```

`kAccumulateFn` is a single function pointer of one fixed type, selected
once at process start by a CPU-feature check, and thereafter called
identically regardless of which ISA won. The public `accumulate()` entry
point that the rest of the library (and the packed wire format) actually
calls has no branch on ISA and no batch-size parameter at all — the batch
size is not a function of which kernel is running.

## `accumulate_avx2` (`src/simd/fastscan_avx2.cpp`, full function)

```cpp
void accumulate_avx2(
    const uint8_t* __restrict__ codes,
    const uint8_t* __restrict__ lp_table,
    uint16_t* __restrict__ result,
    size_t dim
) {
    size_t code_length = dim << 2;
    __m256i c, lo, hi, lut, res_lo, res_hi;

    __m256i low_mask = _mm256_set1_epi8(0xf);
    __m256i accu0 = _mm256_setzero_si256();
    __m256i accu1 = _mm256_setzero_si256();
    __m256i accu2 = _mm256_setzero_si256();
    __m256i accu3 = _mm256_setzero_si256();

    for (size_t i = 0; i < code_length; i += 64) {
        c = _mm256_loadu_si256((__m256i*)&codes[i]);
        lut = _mm256_loadu_si256((__m256i*)&lp_table[i]);
        lo = _mm256_and_si256(c, low_mask);
        hi = _mm256_and_si256(_mm256_srli_epi16(c, 4), low_mask);

        res_lo = _mm256_shuffle_epi8(lut, lo);
        res_hi = _mm256_shuffle_epi8(lut, hi);

        accu0 = _mm256_add_epi16(accu0, res_lo);
        accu1 = _mm256_add_epi16(accu1, _mm256_srli_epi16(res_lo, 8));
        accu2 = _mm256_add_epi16(accu2, res_hi);
        accu3 = _mm256_add_epi16(accu3, _mm256_srli_epi16(res_hi, 8));

        c = _mm256_loadu_si256((__m256i*)&codes[i + 32]);
        lut = _mm256_loadu_si256((__m256i*)&lp_table[i + 32]);
        lo = _mm256_and_si256(c, low_mask);
        hi = _mm256_and_si256(_mm256_srli_epi16(c, 4), low_mask);

        res_lo = _mm256_shuffle_epi8(lut, lo);
        res_hi = _mm256_shuffle_epi8(lut, hi);

        accu0 = _mm256_add_epi16(accu0, res_lo);
        accu1 = _mm256_add_epi16(accu1, _mm256_srli_epi16(res_lo, 8));
        accu2 = _mm256_add_epi16(accu2, res_hi);
        accu3 = _mm256_add_epi16(accu3, _mm256_srli_epi16(res_hi, 8));
    }

    accu0 = _mm256_sub_epi16(accu0, _mm256_slli_epi16(accu1, 8));
    __m256i dis0 = _mm256_add_epi16(
        _mm256_permute2f128_si256(accu0, accu1, 0x21),
        _mm256_blend_epi32(accu0, accu1, 0xF0)
    );
    _mm256_storeu_si256((__m256i*)result, dis0);

    accu2 = _mm256_sub_epi16(accu2, _mm256_slli_epi16(accu3, 8));
    __m256i dis1 = _mm256_add_epi16(
        _mm256_permute2f128_si256(accu2, accu3, 0x21),
        _mm256_blend_epi32(accu2, accu3, 0xF0)
    );
    _mm256_storeu_si256((__m256i*)&result[16], dis1);
}
```

Two stores of 16 `uint16_t` each (`result[0..16]`, `result[16..32]`) — 32
result values total, one per vector in the batch, regardless of `dim`.

## `accumulate_avx512` (`src/simd/fastscan_avx512.cpp`, full function, with the source's own comments kept)

```cpp
void accumulate_avx512(
    const uint8_t* __restrict__ codes,
    const uint8_t* __restrict__ lp_table,
    uint16_t* __restrict__ result,
    size_t dim
) {
    size_t code_length = dim << 2;
    __m512i c;
    __m512i lo;
    __m512i hi;
    __m512i lut;
    __m512i res_lo;
    __m512i res_hi;

    const __m512i lo_mask = _mm512_set1_epi8(0x0f);
    __m512i accu0 = _mm512_setzero_si512();
    __m512i accu1 = _mm512_setzero_si512();
    __m512i accu2 = _mm512_setzero_si512();
    __m512i accu3 = _mm512_setzero_si512();

    // ! here, we assume the code_length is a multiple of 64, thus the dim must be a
    // ! multiple of 16
    for (size_t i = 0; i < code_length; i += 64) {
        c = _mm512_loadu_si512(&codes[i]);
        lut = _mm512_loadu_si512(&lp_table[i]);
        lo = _mm512_and_si512(c, lo_mask);                        // code of vector 0 to 15
        hi = _mm512_and_si512(_mm512_srli_epi16(c, 4), lo_mask);  // code of vector 16 to 31

        res_lo = _mm512_shuffle_epi8(lut, lo);  // get the target value in lookup table
        res_hi = _mm512_shuffle_epi8(lut, hi);

        // since values in lookup table are represented as i8, we add them as i16 to avoid
        // overflow. Since the data order is 0, 8, 1, 9, 2, 10, 3, 11, 4, 12, 5, 13, 6, 14,
        // 7, 15, accu0 accumulates for vec 8 to 15 (the upper 8 bits need to be updated
        // since they stored useless info of vec 0 to 7) accu1 accumulates for vec 0 to 7
        // similar for accu2 and accu3
        accu0 = _mm512_add_epi16(accu0, res_lo);
        accu1 = _mm512_add_epi16(accu1, _mm512_srli_epi16(res_lo, 8));
        accu2 = _mm512_add_epi16(accu2, res_hi);
        accu3 = _mm512_add_epi16(accu3, _mm512_srli_epi16(res_hi, 8));
    }
    // remove the influence of upper 8 bits for accu0 and accu2
    accu0 = _mm512_sub_epi16(accu0, _mm512_slli_epi16(accu1, 8));
    accu2 = _mm512_sub_epi16(accu2, _mm512_slli_epi16(accu3, 8));

    // At this point, we already have the correct accumulating result (accu0: 8-15, accu1:
    // 0-7, accu2: 16-23, accu3: 24-31), but we still need to write them back to RAM. Also,
    // each accu contains 4 lines of __m128i and we need to sum them together to get the
    // final results. 512/16=32, so we can use one __m512i to contain all results. The
    // following codes are designed for this purpose. For detailed information, please check
    // the SIMD documentation.
    __m512i ret1 = _mm512_add_epi16(
        _mm512_mask_blend_epi64(0b11110000, accu0, accu1),
        _mm512_shuffle_i64x2(accu0, accu1, 0b01001110)
    );
    __m512i ret2 = _mm512_add_epi16(
        _mm512_mask_blend_epi64(0b11110000, accu2, accu3),
        _mm512_shuffle_i64x2(accu2, accu3, 0b01001110)
    );
    __m512i ret = _mm512_setzero_si512();

    ret = _mm512_add_epi16(ret, _mm512_shuffle_i64x2(ret1, ret2, 0b10001000));
    ret = _mm512_add_epi16(ret, _mm512_shuffle_i64x2(ret1, ret2, 0b11011101));

    _mm512_storeu_si512(result, ret);
}
```

One store of 32 `uint16_t` — again 32 result values, in a **single**
512-bit store this time, but still 32, not 64.

## The finding: register width does not explain 32

Three independent pieces of evidence in these two functions, taken
together, settle the question:

**1. Both kernels produce exactly 32 results, regardless of register
width.** AVX2's accumulator is 256 bits (32 bytes = 16 × `int16`); it
writes 32 results via two separate 16-wide stores (`result[0..16]` and
`result[16..32]`). AVX512's accumulator is 512 bits (64 bytes = 32 ×
`int16`) — twice as wide — and it writes the *same* 32 results, but in
one store. If `kBatchSize = 32` were dictated by "how many lanes fit in
this ISA's widest register," AVX512 would naturally batch 64 vectors per
call (512 bits / 16 bits per lane = 32 lanes of `int16`, but the codes
themselves are nibble-packed 2-per-byte, so a native-width AVX512 batch
would be 64 vectors, not 32 — exactly the "16 vs 64" alternative RFC
0010's own "How this could be wrong" section named as the open
possibility). It does not. AVX512's extra width is spent processing more
of the packed *code_length* (more `dim`-columns) per loop iteration —
the loop body is structurally identical between the two files, differing
only in vector width and in the final horizontal-combine step needed to
fold that width back down to 32 lanes. The batch size is a property of
the packed input format both kernels are handed, not of the register
executing the unpack.

**2. The packed input format itself (`pack_codes`, already vendored in
`references/rabitq-library-fastscan-pack-codes-source.md`) contains no
register-width constant anywhere** — no `#include <immintrin.h>`, no
ISA conditional, nothing tied to 256 or 512. It is one plain scalar
function producing one wire layout, consumed unchanged by whichever
`accumulate_*` variant the runtime dispatch happens to select. Both
`accumulate_avx2.cpp` and `accumulate_avx512.cpp` `#include
"rabitqlib/fastscan/fastscan.hpp"` for that same, single, ISA-independent
packed layout — there is exactly one `pack_codes`, not one per ISA.

**3. `kBatchSize = 32`'s own arithmetic traces to the LUT width, not a
register width.** `fastscan::pack_lut` (`fastscan.hpp`, already vendored)
builds a **16**-entry lookup table per 4-dimension codebook group —
`num_codebook = dim >> 2; ... for (j = 1; j < 16; ++j)` — a 16-entry table
because a 4-bit sub-code has exactly `2^4 = 16` possible values, a
property of the FastScan/PQ nibble-LUT trick itself (attributed in the
source to Faiss's own FastScan technique,
`facebookresearch/faiss/wiki/Fast-accumulation-of-PQ-and-AQ-codes-
(FastScan)`, which is in turn built on the SSSE3-era 128-bit `pshufb`
instruction — 16 bytes, 16-entry table lookups — that predates both AVX2
and AVX512 by roughly a decade). `pack_codes` packs two such nibbles per
byte (hi/lo, `col_0`/`col_1` in the vendored source) and interleaves them
across `kPerm0`'s 16 positions to cover 32 vector slots per batch: 16 (the
LUT width, fixed by 4 bits) × 2 (the hi/lo nibble pack) = 32. Both
`accumulate_avx2` and `accumulate_avx512`'s inner loops still shuffle
against a **16-byte-lane** table semantics — AVX2's `_mm256_shuffle_epi8`
and AVX512's `_mm512_shuffle_epi8` both operate *within* 128-bit lanes
(a documented x86 instruction-set property, not a choice this library
made), so widening the register from 256 to 512 bits adds more parallel
16-wide lanes operating on more `dim`-columns per instruction, not a
wider table or a bigger per-call batch.

**Conclusion: `kBatchSize = 32` is algorithm-shaped, not hardware-shaped.**
It falls out of the FastScan/PQ nibble-LUT trick's own fixed 16-entry,
4-bit sub-code table, doubled by hi/lo nibble packing — a data-parallelism
shape inherited from Faiss's FastScan technique (itself rooted in SSSE3's
128-bit `pshufb`) and preserved unchanged as a single ISA-independent wire
layout that both the AVX2 and AVX512 decode kernels consume identically,
each producing the same 32 results per call despite operating registers
of different widths. This resolves the residual-hardware-provenance
question RFC 0010's "How this could be wrong" section and Open questions
both named as unclosed at Approval time.

## What this does not claim

This grounds `kBatchSize = 32` specifically. It does not audit whether
some *other* RaBitQ-Library constant not touched by this fetch (e.g. the
high-accuracy `accumulate_hacc_*` variants' own internal grouping, or any
ARM/SVE kernel — out of scope per RFC 0010 Non-goals) carries hardware
provenance; those are separate, unaudited surfaces.
