# `unicode-segmentation`'s word-filtering criterion (for `token_retention: "word-only"`)

Vendored excerpt. Source: `docs.rs/unicode-segmentation/latest/unicode_segmentation/
trait.UnicodeSegmentation.html`, the `unicode_words()` method's documentation.
Fetched 2026-08-18. Grounds RFC 0004's `token_retention: "word-only"` value, found
missing a normative criterion by that RFC's adversarial review — UAX #29 itself
defines only break/no-break positions between characters, never a rule classifying
the resulting segment as a "word" versus not (confirmed by reading the annex text
directly; no rule-status or word/non-word classification language exists in it).

## The criterion

> "'words' are just those substrings which, after splitting on UAX#29 word
> boundaries, contain any alphanumeric characters. That is, the substring must
> contain at least one character with the Alphabetic property, or with
> General_Category=Number."

A segment produced by UAX #29 word-boundary splitting is retained under
`"word-only"` if and only if it contains at least one character with the Unicode
`Alphabetic` property or `General_Category = Number` (any Number subcategory: Nd,
Nl, No). Segments consisting entirely of whitespace, punctuation, or other
non-alphanumeric content are discarded.

This is not part of UAX #29's own normative text — it is a specific, named
implementation choice (this crate's own), adopted here as the precise criterion
`token_retention: "word-only"` means, closing the gap the adversarial review found:
two conformant descriptors could otherwise disagree on edge cases (an isolated
combining-mark run, a standalone symbol) with no stated rule to appeal to.
