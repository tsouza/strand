# Lemire & Boytsov — SIMD-BP128 vs SIMD-FastPFOR, and Bit-Packing Layout

Vendored excerpt, not the full paper. Source: D. Lemire, L. Boytsov,
"Decoding billions of integers per second through vectorization,"
*Software: Practice and Experience* 45(1), 2015 (arXiv:1209.2137v7,
fetched 2026-08-18 from `https://arxiv.org/pdf/1209.2137`). arXiv papers
are distributed under the submitter's chosen license; this reproduction is
a short excerpt for citation and technical commentary, not a claim of a
specific open license on the full paper. Cited to correct a documented
mistake in this project's own `CLAUDE.md` §3: "an early draft named
SIMD-BP128 while describing a patched-exception codec that does not exist
under that name" — this excerpt confirms the real name for that
patched-exception scheme is **SIMD-FastPFOR**, a distinct scheme from
SIMD-BP128, and pins the vertical-vs-horizontal bit-packing layout choice
relevant to any future SIMD-BP128 implementation this project registers.

---

### Binary packing (§2.6), the general technique

> "In our approach to binary packing, we assume that integers are small,
> so we only need to code a bit width b per block (to represent the
> range). Then, successive values are stored using b bits per integer
> using fast bit packing functions."

Binary packing has fixed-length blocks — "e.g., B = 32 or B = 128" (§2.7).
Patched coding (§2.8) is introduced as a *separate* technique to handle
one large outlier value forcing the whole block to a wide bit width:
"Zukowski et al. proposed patching: we use a small bit width b, but store
exceptions (values greater than or equal to 2^b) in a separate location.
They called this approach PFOR."

### SIMD-BP128 (§4/§6): exception-free, the paper's own definition

> "We also implemented a vectorized binary packing over blocks of 128
> integers (henceforth SIMD-BP128). Similar to regular binary packing, we
> want to keep the blocks aligned on 128-bit boundaries when using
> vectorized binary packing. To this end, we regroup 16 blocks into a
> meta-block of 2048 integers... the format of our binary packing schemes
> is as follows: SIMD-BP128 combines 16 blocks of 128 integers whereas
> BP32 combines 4 blocks of 32 integers. SIMD-BP128 employs (vertical)
> vectorized bit packing whereas BP32 relies on the regular bit packing."

SIMD-BP128 carries no exceptions — it is patch-free binary packing,
SIMD-decoded.

### SIMD-FastPFOR (§5): the actual patched-exception scheme

> "We also designed a new scheme, SIMD-FastPFOR: it is identical to
> FastPFOR except that it relies on vectorized bit packing for the
> truncated integers and the high bits of the exception values."

This — not SIMD-BP128 — is the paper's vectorized scheme that stores
exceptions. The abstract states both schemes' distinct roles plainly:
"we introduce a novel vectorized scheme called SIMD-BP128⋆ that improves
over previously proposed vectorized approaches... For even better
compression, we propose another new vectorized scheme (SIMD-FastPFOR)
that has a compression ratio within 10% of a state-of-the-art scheme
(Simple-8b) while being two times faster during decoding." (The `⋆`
suffix in the paper's own notation denotes a variant using vectorized
*differential* coding, not a different packing scheme — "Schemes with a
⋆ by their name use vectorized differential coding.")

### Vertical vs. horizontal SIMD layout (§4, §6.3): a measured choice, not interchangeable

Two layouts exist for arranging 128 integers' bit-packed codes across
SIMD lanes. Vertical (this paper's own layout, and SIMD-BP128's) decodes
with plain shift/AND/OR; horizontal (Willhalm et al.'s layout) additionally
needs an SSSE3 shuffle plus SSE4.1 multiply to realign split-byte
integers — "Willhalm et al. require SSE4.1 for their horizontal bit
packing whereas efficient bit packing using a vertical layout only
requires SSE2."

Measured performance difference (§6.3):

> "In Fig. 10b only, we report the unpacking speed when using the
> horizontal data layout as described by Willhalm et al. [47] (see § 4).
> When the bit widths range from 16 to 26, the vertical and horizontal
> techniques have the same speed. For small (< 8) or large (> 27) bit
> widths, our approach based on a vertical layout is preferable as it is
> up to 70% faster. Accordingly, all integer coding schemes are
> implemented using the vertical layout."

The related-work section (§7) restates the same comparison with a
slightly different figure for the general case: "our implementation of
bit unpacking over a vertical layout is sometimes between 50% to 70%
faster than our reimplementation over a horizontal layout based on the
work of Willhalm et al." Layout is therefore a real, measured design
axis a codec registration must pin (per this project's invariant 11,
"complete registration... including delta/d-gap variant"), not an
implementation detail interchangeable at will.
