# RaBitQ-Library — compact binary-code storage format (source-level)

Vendored excerpt. Source: `github.com/VectorDB-NTU/RaBitQ-Library`,
`docs/docs/compact_code.md` (fetched live via `gh api
repos/VectorDB-NTU/RaBitQ-Library/contents/...`, 2026-08-19, `main` branch
HEAD at fetch time).

Cited by: RFC 0010 (`rfcs/0010-vector-blob-cluster-family.md`), for the
quantized-code wire layout registered in the cluster posting-list blob.
Registered by reference, per invariant 8 ("don't invent encodings...
registered as named codecs") — this RFC does not re-derive the packing
scheme, it adopts the reference implementation's own compact-storage
format as the wire bytes, the same way RFC 0007 adopts `BitPacker8x`'s
packed bytes without re-deriving bit-packing arithmetic.

## Verbatim (lightly reformatted for prose flow, per `CLAUDE.md` §2)

"RaBitQLib supports to quantize codes with different bit widths, i.e., 1,
2, 3, 4, 5, 6, 7 and 8. These bit widths except 8 are unaligned with byte
alignment. Thus, we need to design a specialized compact storage format
for the code vector for each bit width. We pad the dimensionality to a
multiple of 64 for the ease of alignment." Implementation:
`rabitqlib/quantization/pack_excode.hpp`.

## Per-bit-width layout, at padded dimensionality 64k (documented for every 64 dimensions)

- **1-bit:** "The code sequentially stores the binary value for each of the
  64 dimensions" — 8 bytes per 64 dims, i.e. `padded_dim / 8` bytes total,
  a plain bitset in dimension order.
- **2-bit:** stored in a 16-byte array per 64 dims. "The 0-th byte stores
  the 2-bit codes of the 0-th, 16-th, 32-th and 48-th dimensions... This
  storage allows efficient unpacking with SIMD, i.e., shifting and masking
  with SSE" — an interleaved (strided) layout, not sequential, chosen for
  SIMD unpack efficiency.
- **3-bit:** documented as "2-bit + 1-bit" — two separate compact arrays
  concatenated, one at each constituent bit width, not a single 3-bit-per-
  slot packing.
- **4-bit:** 32-byte array per 64 dims, same interleaved-by-stride pattern
  as 2-bit (0-th byte holds dims 0 and 16, etc.), "allows efficient
  unpacking with SIMD."
- **5-bit:** documented as "4-bit + 1-bit," same composition pattern as
  3-bit.
- **6-bit:** 48-byte array per 64 dims, a three-part split — first 16
  bytes hold the full 6-bit codes for dims 0–15 plus the upper 2 bits of
  dims 32–47; second 16 bytes hold dims 16–31 plus the upper 2 bits of
  dims 48–63; third 16 bytes hold the lower 4 bits of dims 32–47 and 48–63
  — again SIMD-shift-and-mask oriented.
- **7-bit:** documented as "6-bit + 1-bit."
- **8-bit:** "aligned with byte arrays and needs no specialized design" —
  one byte per dimension, `padded_dim` bytes total.

## What this grounds

Per-vector code size for bit-width `b` at padded dimensionality `padded_dim`
(itself `dim` rounded up to a multiple of 64, per the rotator's own padding
requirement for `FhtKacRotator` — `references/rabitq-library-rotator-
source.md`):

- `b` ∈ {1, 2, 4, 6, 8}: `padded_dim * b / 8` bytes, exactly (each is
  either the plain bitset or one of the documented interleaved arrays).
- `b` ∈ {3, 5, 7}: the sum of the two constituent widths' byte counts
  (`b=3` → the 2-bit array's bytes plus the 1-bit array's bytes, etc.),
  documented as literal concatenation ("2-bit + 1-bit"), not a bit-width-3
  native packing.

Every documented width is byte-exact and independent of vector content —
the layout is purely a function of `dim` and `b` — which is what makes it
safe to register as a fixed-size opaque payload per vector in a posting
list, addressable by `(cluster_offset + local_index * code_size)` without
a length table.
