# R8 — Rust SIMD policy (portable_simd, wide, pulp)

Vendored excerpts. Fetched 2026-08-18.

Cited by: `docs/research/README.md` R8, `CLAUDE.md` invariant 9 (scalar-normative,
SIMD-as-optimization).

## `rust-lang/portable-simd` tracking issue #364

**Source:** `github.com/rust-lang/portable-simd/issues/364`.

Filed September 2023 to track blockers before `std::simd` stabilization; still open,
confirming invariant 9's "nightly-only with no stabilization in sight" characterization.
Three concrete open API questions the issue lists:

1. **Lane-count bound.** The `LaneCount<N>: SupportedLaneCount` bound is
   "exceptionally cumbersome," with a proposal to make it a post-monomorphization
   error instead of an explicit trait bound.
2. **Mask element type.** Unresolved whether masks for `Simd<f32, N>` should use
   `Simd<i32, N>`, or match the vector's element type directly (which would need
   casts in some cases).
3. **Swizzle functions.** Current swizzle APIs are "difficult to use and not a very
   good API," limited by Rust's current const-generics capabilities; the issue notes
   arbitrary-swizzle stabilization could be deferred as a known limitation.

## Shnatsel, "The state of SIMD in Rust in 2025"

**Source:** `shnatsel.medium.com/the-state-of-simd-in-rust-in-2025-32c263e5f53d`
(Medium returned HTTP 403 to a direct, unauthenticated fetch; the excerpt below is
reconstructed from search-engine result snippets quoting the article directly,
cross-checked against the tracking-issue confirmation above for the shared
nightly-only claim — not independently re-read from the full article body, and
flagged as such).

Published November 2025.

Reported comparison of the three stable-Rust SIMD options invariant 9 names:

- **`std::simd`** — "supports all instruction sets LLVM supports with unparalleled
  platform support and pairs well with the multiversion crate, but it's nightly-only
  and will remain such for the foreseeable future, making it unusable in most
  situations."
- **`wide`** — "a mature, established option that supports NEON, WASM and all x86
  instruction sets, but doesn't support multiversioning except through very exotic
  and limited approaches like cargo-multivers."
- **`pulp`** — "has built-in multiversioning and is reasonably mature and complete,
  powering faer with proven performance," with the limitation that "it only
  operates on the native SIMD width, requiring code to handle variable width
  chunks."

Stated recommendation: use `std::simd` if nightly is acceptable, `wide` if
multiversioning isn't needed, otherwise `pulp` (or `macerator`).

This is the direct source for invariant 9's choice: `wide` or `pulp` with runtime
multiversion dispatch on stable Rust, `portable_simd` excluded as a dependency —
and specifically corroborates why `pulp`, not `wide`, is the fallback whenever
runtime dispatch is actually needed, since `wide` has no real multiversioning story.
