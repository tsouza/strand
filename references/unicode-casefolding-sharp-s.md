# Unicode `CaseFolding.txt` — U+00DF (German ß) has no simple/common mapping

Vendored excerpt. Source: `unicode.org/Public/UCD/latest/ucd/CaseFolding.txt`.
Fetched 2026-08-18. Grounds RFC 0004's `case_folding: "full-case-fold"` example
(German ß), found stated without citation by that RFC's adversarial review.

## The line for U+00DF

```
00DF; F; 0073 0073; # LATIN SMALL LETTER SHARP S
```

## Status-field definitions (file's own header)

> "C: common case folding, common mappings shared by both simple and full mappings."
> "F: full case folding, mappings that cause strings to grow in length."

U+00DF (ß) has only an `F` (full) mapping, to `0073 0073` ("ss") — no `C` (common)
or simple mapping exists for it at all. This confirms the claim RFC 0004 makes:
ordinary lowercasing (which only ever applies simple, length-preserving mappings)
cannot normalize ß to "ss"; only full case-folding does, because ß has no
length-preserving case mapping in the standard.
