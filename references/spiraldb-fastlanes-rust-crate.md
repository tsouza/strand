# `spiraldb/fastlanes` — a real, maintained Rust FastLanes implementation

Vendored finding. Fetched 2026-08-18. Groundwork for R9 (`docs/ledger.md`) — not yet
cited by an approved RFC.

## What it is

**Source:** `github.com/spiraldb/fastlanes`, published as the `fastlanes` crate
(`crates.io/crates/fastlanes`, `docs.rs/fastlanes`). A Rust implementation of
Afroozeh & Boncz's FastLanes compression layout
(`references/r9-fastlanes-core-alp-damon-license.md`), maintained by spiraldb (the
org behind the Vortex columnar format project) — 185 stars, last updated 2026-08-17
(the day before this vendoring), so actively maintained, not an abandoned or toy
port.

## License

**Apache-2.0**, confirmed via GitHub's license API (`gh api
repos/spiraldb/fastlanes/license`) — simpler than the original `cwida/FastLanes`
C++ repository's MIT license (already confirmed Apache-2.0-compatible,
`references/r9-fastlanes-core-alp-damon-license.md`), and this Rust port is
Apache-2.0 directly, no compatibility question at all.

## API surface (from `docs.rs/fastlanes`)

Exposes `BitPacking`, `Delta`, `FoR` (Frame of Reference), `RLE`, and `Transpose` as
traits, plus the core `FastLanes` layout implementation — the primitives a postings
codec actually needs (delta/d-gap encoding, frame-of-reference, bit-packing at a
compile-time-known width).

Real hardware dispatch, not just a scalar reference: "x86-64 transpose
implementations: BMI2 (PEXT/PDEP) and AVX-512 VBMI," with "Portable scalar transpose
using a 64-bit gather and the classic 8×8 bit-matrix transpose... used as the
fallback when no SIMD implementation is available." The BMI2 PEXT/PDEP path is
directly relevant to the hardware trap `docs/ledger.md` R2 already names (Zen 1/2's
slow, non-pipelined PEXT/PDEP vs. Haswell/Zen3+'s fast path,
`references/agner-fog-pdep-pext-latency.md`) — a real R9 benchmark using this crate
would need to account for that trap the same way any hand-rolled kernel would.

## What this changes for R9

`docs/ledger.md` R9 lists "measure FastLanes against hand-vectorized BP128 and
FastPFOR on postings distributions" as an open, unmeasured gate. Before this
vendoring, doing that measurement in this Rust project would have required either
FFI bindings to the original C++ `cwida/FastLanes` implementation or a from-scratch
Rust port — real engineering overhead independent of the measurement itself. A
maintained, Apache-2.0, real-SIMD-dispatch Rust implementation existing already
removes that overhead: the R9 benchmark becomes primarily a matter of writing the
comparison harness and generating realistic postings distributions, not building a
FastLanes implementation from nothing. This does not resolve R9's actual open
question (the margin is still unmeasured) — it changes what building the answer
costs.
