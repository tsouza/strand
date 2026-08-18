# Unicode WordBreakTest.txt (UAX #29 conformance data) and current CLDR version

Vendored excerpt. Fetched 2026-08-18. Groundwork for the not-yet-drafted M1
analyzer-descriptor RFC (invariant 6, `CLAUDE.md` §5) — not yet cited by an approved
RFC.

## Unicode version and word-break conformance data

**Source:** `unicode.org/Public/UCD/latest/ucd/auxiliary/WordBreakTest.txt`.

Corresponds to **Unicode 17.0.0** (dated 2025-03-24), the version `unicode.org/Public/UCD/latest/`
resolved to at fetch time.

This file is Unicode's own official UAX #29 conformance suite: each line gives a
sequence of code points with `÷` (break) and `×` (no-break) markers between them,
the canonical machine-checkable ground truth for word-boundary segmentation — the
natural source for this project's own normative token-stream conformance vectors
(invariant 6) to draw from or validate against, rather than a hand-rolled test case.

Two representative lines:

> `÷ 0041 × 0061 ÷ 003A ÷` — LATIN CAPITAL LETTER A, LATIN SMALL LETTER A (no break
> between them — a single word "Aa"), then a break, then COLON, then a break.

> `÷ 0030 × 0031 ÷ 003A ÷` — DIGIT ZERO, DIGIT ONE (no break — a single "01"), break,
> COLON, break. (The colon is flagged `MidLetter` in the file's own annotation —
> relevant to numeric/punctuation edge cases a real tokenizer profile must decide on.)

## Current CLDR version (as of this session's date, 2026-08-18)

**Source:** Unicode Consortium blog and CLDR release pages (`cldr.unicode.org`,
`github.com/unicode-org/cldr/releases`), via web search — not independently
byte-fetched from a primary release page in this pass, flagged as such.

Latest stable release: **CLDR 48.2**, paired with **ICU 78.3** (announced March
2026, "Unicode ICU 78.3 and CLDR 48.2 released"). CLDR 49 was in its submission/
alpha phase (release-49-alpha0, 2026-08-14) at fetch time, not yet a stable release —
an analyzer descriptor citing "current CLDR" today should cite 48.2, not 49, and a
future session revisiting this after CLDR 49 ships stable should re-check rather than
assume 49 is current by the time this is read.
