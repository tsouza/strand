# FastLanes — ARM SVE Measured Slower Than NEON for Bit-Unpacking

Vendored excerpt, not the full paper. Source: A. Afroozeh, P. Boncz, "The
FastLanes Compression Layout: Decoding >100 Billion Integers per Second
with Scalar Code," *Proceedings of the VLDB Endowment* 16(9), pp.
2132–2144, 2023 (fetched 2026-08-18 from
`https://www.vldb.org/pvldb/vol16/p2132-afroozeh.pdf`). Short quotation
for citation and technical commentary. Cited because this project targets
broad deployment including ARM (e.g. AWS Graviton), and the natural
assumption — that a newer, wider ARM SIMD ISA (SVE) outperforms the older
NEON — is contradicted by this paper's own direct measurement on the
exact bit-unpacking workload in question. `docs/ledger.md` records this
as: default to NEON on ARM targets; do not assume a newer/wider SIMD ISA
is faster without a benchmark proving it for the specific workload.

---

### The direct negative result

> "The SIMD implementations use explicit SIMD intrinsics. Note that for
> ARM64, all SIMD implementations are based on NEON instructions. This is
> because our experiments on Graviton3 showed that SVE is slower than
> NEON."

(Graviton3 is the one platform in this paper's own hardware set that
supports SVE at all — see its Table 2 entry: "AWS Graviton3 ARM64 NEON
(128-bits) modified / SVE (variable) Neoverse-V1, 2.6 GHz.")

### Cross-platform bit-unpacking speedup, and where ARM specifically lags

> "FastLanes decoding: thanks to SIMD it significantly outperforms Scalar
> across all platforms: 40x-70x for 8-bits, to 3x-4x for 64-bits types."

This holds across all six platforms tested (Intel Ice Lake, AMD Zen3,
AMD Zen4, Apple M1, AWS Graviton2, AWS Graviton3). But ARM's narrower,
fixed-width SIMD is called out as the specific limiting factor at wider
bit-widths:

> "We do see that Gravitons have weaker SIMD; which especially shows for
> 64-bits types. Apple M1 also has just 128-bit NEON."

And wider ISA support alone is not decisive even on x86 — the paper notes
a case where AVX-512 support doesn't translate to a speed advantage over
AVX2, for a stated microarchitectural reason:

> "Wider SIMD does not always equate more performance: despite supporting
> AVX512, Zen4 is not faster than Zen3. This is expected if the CPU
> executes one AVX512 instruction using two AVX2 (256-bits) units."
