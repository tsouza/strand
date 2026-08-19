# RaBitQ-Library — rotator implementation (source-level, not abstract-level)

Vendored excerpt. Source: `github.com/VectorDB-NTU/RaBitQ-Library`,
`docs/docs/rabitq/rotator.md` and `include/rabitqlib/utils/rotator.hpp`
(fetched live via `gh api repos/VectorDB-NTU/RaBitQ-Library/contents/...`,
2026-08-19, at the `main` branch HEAD at fetch time).

Cited by: RFC 0010 (`rfcs/0010-vector-blob-cluster-family.md`), resolving
R3's rotation-provenance mechanism decision (`docs/ledger.md` R3:
"the rotation-provenance mechanism (materialized matrix vs generator+seed,
M2 RFC)"). Supersedes the abstract-only grounding in
`references/rabitq-and-extended-rabitq.md` for this specific question —
that file's own fetch was abstract-level only and explicitly did not reach
the rotation mechanism.

## Two registered rotator types (`docs/docs/rabitq/rotator.md`, verbatim except as noted)

`RotatorType::MatrixRotator` — "the classical Johnson Lindenstrauss
Transformation. It first samples a random gaussian matrix and orthogonalizes
it with QR decomposition. Then it multiplies the matrix to every vector."
Space: `D^2` floating-point numbers. Time: `O(D^2)`. `padding_requirement()`
(`rotator.hpp`) pads to `dim` itself — i.e. no real padding.

`RotatorType::FhtKacRotator` — the library's **default** ("By default, the
library uses the `FFHT + Kac's Walk` method"). "Samples 4 sequences of
random signs (i.e., Rademacher random variables). Then for each vector, it
repeats the following procedures 4 times: 1. Flip its coordinates with the
i-th sequence of sampled random signs. 2. Apply FFHT on the first/last `2^k`
coordinates (alternately), where `2^k` is the maximum power of 2 ≤ the
dimensionality. 3. Apply Givens rotation with a fixed angle θ = π/4 to the
1st and 2nd halves of coordinates." Space: `4D` binary values (`4D` bits).
Time: `O(D log D)`. `padding_requirement()` rounds `dim` up to the nearest
multiple of 64 (`round_up_to_multiple(dim, 64)`).

## Serialized state, read directly from source (not the docs page)

`MatrixRotator<T>::rand_mat_` is a `RowMajorMatrix<T>` of `dim * padded_dim`
elements, constructed as `Eigen::HouseholderQR` applied to a random Gaussian
matrix, then transposed (`qr.householderQ().transpose()`) — QR decomposition
is a floating-point-library-dependent numerical routine (different
BLAS/LAPACK implementations, or even the same implementation at different
optimization levels, are not guaranteed to produce bit-identical output for
the same input). `save`/`load` write/read `sizeof(float) * dim * padded_dim`
bytes verbatim (`std::memcpy` to/from a flat buffer) — the class does not
serialize a seed; it serializes the realized matrix.

`FhtKacRotator::flip_` is a `std::vector<uint8_t>` of size
`4 * padded_dim / 8` bytes (`kByteLen = 8`), filled from
`std::random_device` seeding `std::mt19937`, one `uint8_t` (8 packed sign
bits) per byte via `std::uniform_int_distribution<int>(0, 255)`. `save`/
`load` write/read `flip_.size()` bytes verbatim — again, the class
serializes the realized sign bytes, not a seed. `trunc_dim_` (the truncated
power-of-two subspace FHT operates on) and the corresponding FHT dispatch
helper are derived deterministically from `dim` alone (`floor_log2(dim)`),
not stored.

## What this grounds

The reference implementation's own answer to "materialize or regenerate
from seed" is, for **both** rotator types, to serialize the realized
random state directly (a full matrix for `MatrixRotator`, packed sign bytes
for `FhtKacRotator`) rather than a seed plus a normatively pinned generator.
For `MatrixRotator` this is also the only byte-determinism-safe choice
available without inventing a spec the reference implementation itself does
not follow: `std::mt19937` output is portable, but `Eigen::HouseholderQR`'s
floating-point orthogonalization is not guaranteed bit-identical across
implementations, so pinning a seed alone would not guarantee two conforming
readers regenerate the same matrix. `FhtKacRotator` sidesteps this because
its only random state is 4D bits with no floating-point-dependent
derivation step, and that state is cheap enough to store outright — 384
bytes at 768 padded dims (4 × 768 / 8) — removing any pressure to prefer a
seed for size reasons.
