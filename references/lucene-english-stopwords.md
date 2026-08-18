# Lucene — default English stopword list

Vendored excerpt, byte-exact via `curl`. Source: Apache Lucene, tag
`releases/lucene/10.5.1`,
`lucene/analysis/common/src/java/org/apache/lucene/analysis/en/EnglishAnalyzer.java`.
Fetched 2026-08-18. Groundwork for the not-yet-drafted M1 analyzer-descriptor RFC
(invariant 6) — not yet cited by an approved RFC.

## The list, exactly as declared in source

```java
static {
  final List<String> stopWords =
      Arrays.asList(
          "a", "an", "and", "are", "as", "at", "be", "but", "by", "for", "if", "in", "into", "is",
          "it", "no", "not", "of", "on", "or", "such", "that", "the", "their", "then", "there",
          "these", "they", "this", "to", "was", "will", "with");
  ...
  ENGLISH_STOP_WORDS_SET = CharArraySet.unmodifiableSet(stopSet);
```

33 words: `a, an, and, are, as, at, be, but, by, for, if, in, into, is, it, no, not,
of, on, or, such, that, the, their, then, there, these, they, this, to, was, will,
with`.

This is the exact default `EnglishAnalyzer` uses when constructed with no explicit
stopword set (`new EnglishAnalyzer()` delegates to `ENGLISH_STOP_WORDS_SET`) — the
natural "Lucene default English stopword list" identity for an analyzer descriptor
that declares Lucene-derived behavior, fetched directly rather than assumed from the
classic, widely-repeated 33-ish-word list folklore.
