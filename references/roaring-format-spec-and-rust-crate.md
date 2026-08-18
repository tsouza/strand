# The Roaring bitmap format spec, and the `roaring` Rust crate

Vendored excerpts, byte-exact via `curl` and a real compiled worked example.
Fetched 2026-08-18. Groundwork for the M1 filter-bitmap RFC (`docs/data-
structures.md`'s "settled" Roaring default) — not yet cited by an approved RFC.

## The official format spec

**Source:** `github.com/RoaringBitmap/RoaringFormatSpec`, `README.md`, fetched
directly via `curl` (not summarized). **License: Apache-2.0**, confirmed via
GitHub's license API.

> "Let us recap that Roaring bitmaps are designed to store sets of 32-bit
> (unsigned) integers. Thus a Roaring bitmap can contain up to 4294967296
> integers. They are made of three types of 16-bit containers: array, bitset and
> run containers."

Constants: `SERIAL_COOKIE_NO_RUNCONTAINER = 12346`, `SERIAL_COOKIE = 12347`,
`NO_OFFSET_THRESHOLD = 4`. All words little-endian throughout — matching
invariant 11's own endianness pin independently.

**Cookie header.** Two forms: (1) `SERIAL_COOKIE_NO_RUNCONTAINER`, a plain 32-bit
cookie value followed by a 32-bit container count — used when no container is a
run container (the empty-bitmap case, and the form a serializer may always choose
for run-container-free bitmaps, per the spec's own isomorphism note below); (2) the
lower 16 bits of a 32-bit value equal to `SERIAL_COOKIE`, with the upper 16 bits
holding `container_count - 1`, followed by `(size + 7) / 8` bytes as a per-
container run-container bitset.

**Descriptive header.** 4 bytes per container: 16-bit key (the container's most
significant bits) + 16-bit `cardinality - 1`.

**Offset header.** Present when the `NO_RUNCONTAINER` cookie is used, or when the
`SERIAL_COOKIE` form is used with `container_count >= NO_OFFSET_THRESHOLD` (4):
one 32-bit absolute byte offset per container, from the stream's start.

**Container storage.** Array containers (cardinality ≤ 4096): sorted 16-bit
values, 2 bytes each. Bitset containers (cardinality > 4096): exactly 8192 bytes
(64-bit words, bit `j % 64` of word `j / 64` marks presence of value `j`). Run
containers: a 16-bit run count, then `(start: u16, length_minus_1: u16)` pairs.

> "In practice, implementations can ensure isomorphism by, for example, always
> serializing bitmaps without run containers with the `SERIAL_COOKIE_NO_RUNCONTAINER`
> cookie."

**The 64-bit extension is explicitly less standardized than the 32-bit form,
worth naming precisely.** The spec's own extension section states one real Java
implementation (the ART-based `Roaring64Bitmap`) "is not compatible with this
Serialization format," and a second (`Roaring64NavigableMap`) also "is not
compatible with this serialization format (which does not handle signed keys)" —
i.e., there is no single, universally-interoperable 64-bit wire form the way there
is for 32-bit. This is the direct grounding for a design choice STRAND's own RFC
makes: use the standard 32-bit form only, indexed by local ordinal (already
bounded well under 2^32 for any real segment), never the less-interoperable 64-bit
extension.

## The `roaring` Rust crate — real, spec-compliant, license-confirmed

**Source:** `github.com/RoaringBitmap/roaring-rs`, crate `roaring`, version
`0.11.5` (confirmed via the workspace member's own `roaring/Cargo.toml`, not the
top-level workspace manifest, which carries no version/license fields of its
own). **License:** `MIT OR Apache-2.0`, confirmed via the crate's own `Cargo.toml`.

**Spec compliance confirmed from source, not assumed:** `roaring/src/bitmap/
serialization.rs` declares `SERIAL_COOKIE_NO_RUNCONTAINER: u32 = 12346`,
`SERIAL_COOKIE: u16 = 12347`, `NO_OFFSET_THRESHOLD: usize = 4` — an exact match to
the official spec's constants, fetched and grepped directly rather than trusted
from the crate's documentation summary alone.

## Real worked-example bytes (built with the actual crate, not hand-derived)

Three local ordinals `{1, 2, 5}` in one `RoaringBitmap`, serialized via
`serialize_into`:

```
3A 30 00 00 01 00 00 00 00 00 02 00 10 00 00 00 01 00 02 00 05 00
```

22 bytes, decoded field-by-field against the spec above: `3A 30 00 00` = `0x0000303A`
= `12346` = `SERIAL_COOKIE_NO_RUNCONTAINER`; `01 00 00 00` = container count `1`;
`00 00 02 00` = descriptive header (key `0`, cardinality − 1 = `2`, i.e. cardinality
`3`); `10 00 00 00` = offset header, container starts at byte `16`; `01 00 02 00 05
00` = the array container itself, three sorted 16-bit values `1, 2, 5`. `16 + 6 =
22`, matching the reported length exactly. Round-trip confirmed
(`RoaringBitmap::deserialize_from`), and rebuilding the same logical bitmap twice
independently produced byte-identical output (same-process, same-version — the same
narrower determinism claim already made for the `fst` crate in RFC 0005, not
independently re-tested across platforms or crate versions here either).

A second bitmap, local ordinals `{0, 3, 4}`, serializes to 22 bytes as well:

```
3A 30 00 00 01 00 00 00 00 00 02 00 10 00 00 00 00 00 03 00 04 00
```

These two bitmaps and a real `fst`-crate value dictionary (`"blue" -> 0, "red" ->
1`, 53 bytes, built the same way as RFC 0005's own FST worked example) are the
basis for the filter-bitmap RFC's worked example.
