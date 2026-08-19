# RaBitQ-Library — rotation application (source-level): `rotate()`, FHT, flip_sign, kacs_walk

Vendored excerpt. Source: `github.com/VectorDB-NTU/RaBitQ-Library`,
`include/rabitqlib/utils/rotator.hpp`, `include/rabitqlib/utils/
fht_avx.hpp`, `include/rabitqlib/simd/rotator_dispatch.hpp`, and
`src/simd/rotator_avx2.cpp` (fetched live via `gh api
repos/VectorDB-NTU/RaBitQ-Library/contents/...`, 2026-08-19, `main` branch
HEAD at fetch time).

Cited by: `crates/strand-vector/src/rotate.rs`. Grounds rotation
*application* — as opposed to `references/rabitq-library-rotator-
source.md`, which grounds only the rotation *payload's storage format*
(the realized flip bytes / matrix, serialized verbatim). This file
supersedes that one's own note that rotation application "is not part of
this fetch" for `FhtKacRotator` specifically.

## `FhtKacRotator::rotate()` (`rotator.hpp`, verbatim)

```cpp
void rotate(const float* data, float* rotated_vec) const override {
    std::memcpy(rotated_vec, data, sizeof(float) * dim_);
    std::fill(rotated_vec + dim_, rotated_vec + padded_dim_, 0);

    if (trunc_dim_ == padded_dim_) {
        flip_sign(flip_.data(), rotated_vec, padded_dim_);
        fht_float_(rotated_vec);
        vec_rescale(rotated_vec, trunc_dim_, fac_);

        flip_sign(flip_.data() + (padded_dim_ / kByteLen), rotated_vec, padded_dim_);
        fht_float_(rotated_vec);
        vec_rescale(rotated_vec, trunc_dim_, fac_);

        flip_sign(flip_.data() + (2 * padded_dim_ / kByteLen), rotated_vec, padded_dim_);
        fht_float_(rotated_vec);
        vec_rescale(rotated_vec, trunc_dim_, fac_);

        flip_sign(flip_.data() + (3 * padded_dim_ / kByteLen), rotated_vec, padded_dim_);
        fht_float_(rotated_vec);
        vec_rescale(rotated_vec, trunc_dim_, fac_);

        return;
    }

    size_t start = padded_dim_ - trunc_dim_;

    flip_sign(flip_.data(), rotated_vec, padded_dim_);
    fht_float_(rotated_vec);
    vec_rescale(rotated_vec, trunc_dim_, fac_);
    kacs_walk(rotated_vec, padded_dim_);

    flip_sign(flip_.data() + (padded_dim_ / kByteLen), rotated_vec, padded_dim_);
    fht_float_(rotated_vec + start);
    vec_rescale(rotated_vec + start, trunc_dim_, fac_);
    kacs_walk(rotated_vec, padded_dim_);

    flip_sign(flip_.data() + (2 * padded_dim_ / kByteLen), rotated_vec, padded_dim_);
    fht_float_(rotated_vec);
    vec_rescale(rotated_vec, trunc_dim_, fac_);
    kacs_walk(rotated_vec, padded_dim_);

    flip_sign(flip_.data() + (3 * padded_dim_ / kByteLen), rotated_vec, padded_dim_);
    fht_float_(rotated_vec + start);
    vec_rescale(rotated_vec + start, trunc_dim_, fac_);
    kacs_walk(rotated_vec, padded_dim_);

    // This can be removed if we don't care about the absolute value of similarities.
    vec_rescale(rotated_vec, padded_dim_, 0.25F);
}
```

`trunc_dim_ = 1 << floor_log2(dim_)` and `fac_ = 1/sqrt(trunc_dim_)`
(constructor, already grounded in `references/rabitq-library-rotator-
source.md`) — computed from `dim_` (the raw, unpadded dimensionality), not
`padded_dim_`. `kByteLen = 8`. Note `data` is `dim_` (raw) elements long;
`rotate()` itself zero-extends into the `padded_dim_`-sized output buffer.

## `helper_float_1`/`helper_float_2` (`fht_avx.hpp`, verbatim) — the portable Fast Walsh-Hadamard Transform

```cpp
inline void helper_float_1(float *buf) {
  for (int j = 0; j < 2; j += 2) {
    for (int k = 0; k < 1; ++k) {
      float u = buf[j + k];
      float v = buf[j + k + 1];
      buf[j + k] = u + v;
      buf[j + k + 1] = u - v;
    }
  }
}
inline void helper_float_2(float *buf) {
  for (int j = 0; j < 4; j += 2) {
    for (int k = 0; k < 1; ++k) {
      float u = buf[j + k];
      float v = buf[j + k + 1];
      buf[j + k] = u + v;
      buf[j + k + 1] = u - v;
    }
  }
  for (int j = 0; j < 4; j += 4) {
    for (int k = 0; k < 2; ++k) {
      float u = buf[j + k];
      float v = buf[j + k + 2];
      buf[j + k] = u + v;
      buf[j + k + 2] = u - v;
    }
  }
}
```

`helper_float_3` and above (sizes 8, 16, 32, ...) compute the identical
mathematical transform via hand-written AVX2 inline assembly (butterfly
network expressed with `vpermilps`/`vaddsubps`/`vperm2f128` etc.) — read in
full during this fetch and confirmed to be the same Fast Walsh-Hadamard
Transform, not a different algorithm, just vectorized. Not reproduced here
verbatim: per invariant 9 (`CLAUDE.md` §5), the scalar implementation is
normative and a SIMD path is an optimization that must reproduce the
scalar result, not the other way around — `crates/strand-vector/src/
rotate.rs`'s `fht()` is the standard, textbook in-place recursive-doubling
FWHT algorithm, which `helper_float_1`/`helper_float_2` are literally
(hand-verified: both match the general algorithm's own output structure
for `n=2` and `n=4` exactly), generalized to any power-of-two `n`.
`choose_rotator()` restricts the supported range to `floor_log2(dim)` in
`6..=11` (`fht_float_` dispatches on `helper_float_6`..`helper_float_11`
in the constructor, already grounded) — i.e. `trunc_dim_` between 64 and
2048 — but the scalar algorithm generalizes to any power of two; STRAND's
own implementation does not reproduce that library-specific range
restriction, since it has no wire-format meaning.

## `flip_sign_avx2` / `kacs_walk_avx2` (`src/simd/rotator_avx2.cpp`, verbatim)

```cpp
void flip_sign_avx2(const uint8_t* flip, float* data, size_t dim) {
    constexpr size_t kFloatsPerChunk = 32;
    const __m256i bit_select = _mm256_setr_epi32(0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80);
    const __m256 sign_flip = _mm256_castsi256_ps(_mm256_set1_epi32(0x80000000));
    auto create_mask = [&](uint8_t byte_mask) -> __m256 {
        __m256i mask_bits = _mm256_set1_epi32(byte_mask);
        __m256i test = _mm256_and_si256(mask_bits, bit_select);
        __m256i cmp = _mm256_cmpeq_epi32(test, bit_select);
        return _mm256_and_ps(_mm256_castsi256_ps(cmp), sign_flip);
    };
    for (size_t i = 0; i < dim; i += kFloatsPerChunk) {
        uint32_t mask_bits;
        std::memcpy(&mask_bits, &flip[i / 8], sizeof(mask_bits));
        for (int b = 0; b < 4; ++b) {
            __m256 xor_mask = create_mask((mask_bits >> (b * 8)) & 0xFF);
            __m256 vec = _mm256_loadu_ps(&data[i + b * 8]);
            vec = _mm256_xor_ps(vec, xor_mask);
            _mm256_storeu_ps(&data[i + b * 8], vec);
        }
    }
}

void kacs_walk_avx2(float* data, size_t len) {
    // ! len % 16 == 0;
    for (size_t i = 0; i < len / 2; i += 8) {
        __m256 x = _mm256_loadu_ps(&data[i]);
        __m256 y = _mm256_loadu_ps(&data[i + (len / 2)]);
        __m256 new_x = _mm256_add_ps(x, y);
        __m256 new_y = _mm256_sub_ps(x, y);
        _mm256_storeu_ps(&data[i], new_x);
        _mm256_storeu_ps(&data[i + (len / 2)], new_y);
    }
}
```

**Scalar semantics, decoded from the SIMD** (no portable scalar fallback
is shipped by the library itself for these two functions — this is the
only source available, so the semantics were read out of the vector code
rather than transcribed from an existing scalar reference):

`flip_sign`: `bit_select`'s lane order (`0x01, 0x02, ..., 0x80`) means bit
`(d % 8)` of byte `flip[d / 8]` — **LSB-first** — governs dimension `d`'s
sign. This is the opposite convention from `crates/strand-vector/src/
quantize.rs`'s `pack_binary` (MSB-first) — a different function from a
different part of the reference implementation; no bit-order convention
carries over between them.

`kacs_walk`: for `i` in `0..len/2`, replace `(data[i], data[i+len/2])`
with `(data[i]+data[i+len/2], data[i]-data[i+len/2])` — a single
butterfly stage across the two halves of a `len`-length window.

## Independent verification

A standalone, dependency-free C++ program (plain loops for `fht`,
`flip_sign`, `kacs_walk`, and the full `rotate()` two-branch pipeline —
matching the above exactly) was compiled (`g++ -O2`) and run against
three cases: `dim=100, padded_dim=128` (general branch), `dim=padded_dim
=64` (the degenerate branch where `trunc_dim_` already equals
`padded_dim_`), and `dim=padded_dim=768` (the realistic embedding case —
768 is not a power of two, so this is the general branch in practice for
most real embedding widths). The `dim=768` case's own output confirms an
independent mathematical property beyond value-matching: a true rotation
preserves L2 norm, and the compiled program's own input/output sums of
squares matched to four decimal places (`1549.8966` vs `1549.8970`).
`crates/strand-vector/src/rotate.rs`'s tests assert this property directly
(both as a fixed case and as a `proptest` property across hundreds of
random inputs), not just value equality against the C++ output.

## What this grounds, and what remains open

Grounds: the complete `rotate()` algorithm for `FhtKacRotator`, both
branches, and the plain row-major matrix-vector product for
`MatrixRotator` (already grounded in `references/rabitq-library-rotator-
source.md`'s own doc comment). Not grounded: the AVX2/AVX512 SIMD kernels'
own bit-exact behavior at the instruction level (this fetch reads them for
scalar semantics only, per invariant 9); `choose_rotator()`'s library-side
error handling for out-of-range `dim` (STRAND's own scalar `fht()`
generalizes to any power of two, so this restriction has no wire-format
meaning and is not reproduced).
