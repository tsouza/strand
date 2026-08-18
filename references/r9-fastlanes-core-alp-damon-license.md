# R9 — FastLanes core paper, ALP, DaMoN GPU paper, and the license audit

Vendored excerpts. Fetched 2026-08-18. (`references/fastlanes-arm-sve-vs-neon.md` and
`references/agner-fog-pdep-pext-latency.md` were already vendored in an earlier
session; this file covers R9's remaining named sources.)

Cited by: `docs/research/README.md` R9, `docs/lineage.md` ("From FastLanes"),
`CLAUDE.md` §1 ("The FastLanes license is unaudited").

## FastLanes core paper (Afroozeh & Boncz, VLDB 2023)

**Source:** `vldb.org/pvldb/vol16/p2132-afroozeh.pdf`. Read directly as a PDF (the
fetch tool could not decode it).

**Title:** The FastLanes Compression Layout: Decoding >100 Billion Integers per
Second with Scalar Code
**Authors:** Azim Afroozeh, Peter Boncz (CWI)
**Venue:** PVLDB 16(9): 2132–2144, 2023. doi:10.14778/3598581.3598587

Confirms, verbatim, the core claims `docs/lineage.md` attributes to this paper:

> "We address the software development, maintenance and future-proofness challenges
> of hardware diversity, by defining a virtual 1024-bits instruction set that
> consists of simple operators supported by all SIMD dialects; and also,
> importantly, by scalar code. [...] Importantly, modern compilers can auto-vectorize
> our scalar code-path without loss of performance."

> "Micro-benchmarks on Intel, AMD, Apple and AWS CPUs show that FastLanes
> accelerates decoding by factors (>40 values per CPU cycle)."

The "Unified Transposed Layout" — reordering 1024 tuples into eight 8×16 transposed
blocks in the order "04261537" — is the specific mechanism behind the transposed
tuple order `docs/lineage.md` and invariant 10 reference; the paper states this
specific block order explicitly as its named contribution.

**Note on the paper's own license.** The PDF's footer states the paper text itself
is under "Creative Commons BY-NC-ND 4.0 International License" — this is the
*paper's* copyright license (standard for VLDB proceedings), and is unrelated to and
must not be conflated with the *code* license below.

## ALP (Afroozeh, Kuffó, Boncz, SIGMOD 2024)

**Source:** `ir.cwi.nl/pub/33334/33334.pdf` (CWI's own repository; freely
accessible, unlike the paywalled ACM DOI page). Read directly as a PDF.

**Title:** ALP: Adaptive Lossless floating-Point Compression
**Authors:** Azim Afroozeh, Leonardo Kuffó, Peter Boncz (CWI)
**Venue:** SIGMOD '24, June 9–15, 2024, Santiago, Chile

> "ALP is 1-2 orders of magnitude faster in [de]compression than all competing
> schemes, while providing an excellent compression ratio." ... "Its high speeds
> stem from our implementation in scalar code that auto-vectorizes, using building
> blocks provided by our FastLanes library."

Confirms ALP as the FastLanes-family companion float codec `docs/research/README.md`
R9 names as relevant to the flat vector blob, and confirms it explicitly builds on
FastLanes' own scalar-auto-vectorizing building blocks.

## DaMoN '24 GPU decode paper

**Source:** `ir.cwi.nl/pub/34260/34260.pdf` (CWI's own repository). Read directly as
a PDF.

**Title:** Accelerating GPU Data Processing using FastLanes Compression
**Authors:** Azim Afroozeh, Lotte Felius, Peter Boncz (CWI)
**Venue:** DaMoN '24, June 10, 2024, Santiago, Chile. doi:10.1145/3662010.3663450

> "We show that compression can be a win-win for GPU data processing: it not only
> allows to store more data in GPU global memory, but can also *accelerate* data
> processing." ... "Our experiments show that FastLanes decompression significantly
> outperforms previous decompression methods in micro-benchmarks, and can make
> end-to-end SSB queries up to twice faster compared to uncompressed query
> processing."

The paper also names a real limitation worth carrying into any R9 RFC: "an access
granularity of decoding vectors of 1024 values is too large for a single GPU warp
due to register pressure," mitigated with "mini-vectors" — a caveat on the
1024-value granularity's portability to GPU decode paths specifically, not just a
clean win.

## License audit: `cwida/FastLanes` — resolved, MIT

**Source:** GitHub's license API, `gh api repos/cwida/FastLanes/license`, 2026-08-18.

> SPDX license identifier: `MIT`. `https://github.com/cwida/FastLanes/blob/dev/LICENSE`.

This resolves the license-audit gate `CLAUDE.md` §1 and `docs/ledger.md` R9 both
name as still open ("the cwida/FastLanes license is unaudited and gates any R9
adoption"): the code repository is MIT-licensed, which is Apache-2.0-compatible
(the same status already accepted for tantivy and FAISS elsewhere in this project).
This clears the license half of R9's three-gate requirement (measured margin over
hand-vectorized BP128/FastPFOR on postings distributions, and the inverted-index
application gap, remain open — this vendoring resolves licensing only, not R9 as a
whole).
