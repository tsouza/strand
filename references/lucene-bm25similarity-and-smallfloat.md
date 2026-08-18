# Lucene — `BM25Similarity` and `SmallFloat` (norm encoding)

Vendored excerpts, byte-exact via `curl` (not the summarizing WebFetch model — this
file's contents are load-bearing for byte-level conformance work). Source: Apache
Lucene, tag `releases/lucene/10.5.1` (the latest release at fetch time; verified
distinct from the `main` branch, which carries an unreleased `k3` query-term-
saturation parameter this vendoring deliberately does not ground against — see
below). Files:
`lucene/core/src/java/org/apache/lucene/search/similarities/BM25Similarity.java`,
`lucene/core/src/java/org/apache/lucene/search/similarities/Similarity.java`,
`lucene/core/src/java/org/apache/lucene/util/SmallFloat.java`. Fetched 2026-08-18.

Cited by: the M1 scoring-profiles RFC (drafted 2026-08-18), `CLAUDE.md` invariant 5
("parity within Lucene's one-byte norm quantization").

## Default parameters

```java
public BM25Similarity() {
  this(1.2f, 0.75f, true);
}
```

`k1 = 1.2`, `b = 0.75`, `discountOverlaps = true`.

## idf formula

```java
/** Implemented as <code>log(1 + (docCount - docFreq + 0.5)/(docFreq + 0.5)</code>. */
protected float idf(long docFreq, long docCount) {
  return (float) Math.log(1 + (docCount - docFreq + 0.5D) / (docFreq + 0.5D));
}
```

This has a `+1` inside the logarithm that the classic Robertson–Sparck-Jones idf
form does not
(`references/robertson-zaragoza-bm25-and-beyond.md`) — Lucene's own comment doesn't
explain why, but the well-known practical reason (not independently confirmed against
a Lucene source comment in this vendoring pass) is that the classic form goes
negative for terms occurring in more than half the collection, which the `+1` avoids.

## Score formula (the tf/length-normalization component)

```java
/**
 * precomputed norm[256] with k1 * ((1 - b) + b * dl / avgdl)
 */
cache[i] = 1f / (k1 * ((1 - b) + b * LENGTH_TABLE[i] / avgdl));
...
private float doScore(float freq, float normInverse) {
  // In order to guarantee monotonicity with both freq and norm without
  // promoting to doubles, we rewrite freq / (freq + norm) to
  // 1 - 1 / (1 + freq * 1/norm).
  ...
  return weight - weight / (1f + freq * normInverse);
}
```

Algebraically, `doScore(freq, normInverse) = weight * freq / (freq + norm)` where
`weight = boost * idf` and `norm = k1 * ((1-b) + b*dl/avgdl)` — this is exactly the
Robertson–Zaragoza canonical form (eq. 3.15,
`references/robertson-zaragoza-bm25-and-beyond.md`), with no `(k1+1)` numerator
factor. Lucene's real implementation matches the paper's own canonical definition
directly; it is the rewritten-for-monotonicity-without-doubles form that differs in
appearance, not in the underlying formula.

## Document length for scoring (`computeNorm`, in `Similarity.java`)

```java
public long computeNorm(FieldInvertState state) {
  final int numTerms;
  if (state.getIndexOptions() == IndexOptions.DOCS) {
    numTerms = state.getUniqueTermCount();
  } else if (discountOverlaps) {
    numTerms = state.getLength() - state.getNumOverlap();
  } else {
    numTerms = state.getLength();
  }
  return SmallFloat.intToByte4(numTerms);
}
```

With `discountOverlaps = true` (BM25Similarity's default), Lucene's document length
for scoring purposes is `state.getLength() - state.getNumOverlap()` — total token
count minus tokens at a zero position increment (synonym-at-the-same-position-style
overlaps). This already differs from a literal token count whenever a field carries
overlapping tokens, which any invariant-6 conformance work computing "per-document
length" for Lucene parity must account for, not assume away.

`computeNorm` actually has a third branch this file's earlier text omitted (found by
the RFC 0003 adversarial review, 2026-08-18): when `state.getIndexOptions() ==
IndexOptions.DOCS` (a field indexed with no term frequencies at all), Lucene uses
`state.getUniqueTermCount()` instead of `length - numOverlap`. This branch is out of
scope for BM25 parity — BM25 requires term frequencies, so a DOCS-only field cannot
be BM25-scored meaningfully regardless of which norm it carries — but it is a real
branch of the exact function this file quotes in full above, and is noted here for
completeness rather than left silently missing from the commentary.

## Norm encoding: `SmallFloat.intToByte4`/`byte4ToInt` — NOT the classic `byte315`

**This is the load-bearing correction this vendoring pass exists to make.** An
initial, memory-based assumption was that Lucene's one-byte norm uses the widely-
blogged-about `floatToByte315`/`byte315ToFloat` scheme (3 mantissa bits, zero-
exponent 15, from Lucene's 2.x–4.x era). The actual current `BM25Similarity`
(confirmed against both the `main` branch and the released `10.5.1` tag) uses a
different, newer scheme entirely: `SmallFloat.intToByte4`/`byte4ToInt`, an integer-
valued (not float-valued) encoding with 4 significant bits, built on
`longToInt4`/`int4ToLong`:

```java
public static byte intToByte4(int i) {
  if (i < 0) throw new IllegalArgumentException(...);
  if (i < NUM_FREE_VALUES) {
    return (byte) i;
  } else {
    return (byte) (NUM_FREE_VALUES + longToInt4(i - NUM_FREE_VALUES));
  }
}
```

`NUM_FREE_VALUES = 255 - longToInt4(Integer.MAX_VALUE)`, computed (not asserted)
during this vendoring pass: `longToInt4(2147483647) = 231`, so `NUM_FREE_VALUES =
24`. Consequence, worth pinning precisely for a worked example: **document lengths
0–23 tokens encode exactly** (the byte value equals the token count, no precision
loss at all); only document lengths ≥ 24 tokens fall into the lossy 4-significant-bit
floating encoding (`longToInt4`'s shift-and-truncate scheme). `byte4ToInt` is the
decode direction, used to build `BM25Similarity`'s 256-entry `LENGTH_TABLE`.

This means any "parity within Lucene's one-byte norm quantization" harness
(`CLAUDE.md` invariant 5) must implement `intToByte4`/`byte4ToInt` specifically, not
the older `byte315` scheme a from-memory implementation would likely reach for.

## Version scope

`k3` (query-term-frequency saturation) exists in Lucene's `main` branch (commit
`fa0e704d2a0f941f2bbca17184472ae0e2b3743e` at fetch time) but **not** in the released
`10.5.1` tag — confirmed by fetching both and diffing. This vendoring, and the
scoring-profiles RFC it grounds, targets `10.5.1`, the actual released version, not
an unreleased `main`-branch feature. `k3` defaults to `-1` (disabled) even where it
exists, so its absence from the parity target changes nothing about default-
configuration parity; a future session adding `k3` support should re-vendor against
whatever release first ships it, not against `main`.
