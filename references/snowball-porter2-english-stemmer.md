# Snowball / Porter2 English stemmer — algorithm, license, and test vectors

Vendored findings. Fetched 2026-08-18. Groundwork for the not-yet-drafted M1
analyzer-descriptor RFC (invariant 6) — not yet cited by an approved RFC.

## What it is

**Source:** `snowballstem.org/algorithms/english/stemmer.html`; implementation and
test data at `github.com/snowballstem/snowball` and
`github.com/snowballstem/snowball-data`.

The Snowball project's English stemmer (commonly called "Porter2," an improved
successor to Martin Porter's original 1980 Porter algorithm, maintained by the same
author). Five main suffix-stripping steps plus a table of exceptions for irregular
or frequently-misstemmed words (e.g. `"sky"` is explicitly excepted from stemming to
`"sky"`, not reduced further).

## License

- `snowballstem/snowball` (the algorithm implementation): **BSD-3-Clause**, confirmed
  via GitHub's license API. Apache-2.0-compatible.
- `snowballstem/snowball-data` (the test vectors used below): GitHub's license API
  reports `NOASSERTION`; the repository's own `COPYING` file states the same
  BSD-3-Clause terms (Copyright Dr Martin Porter 2001, Richard Boulton 2004–2005),
  confirmed by reading the file directly rather than trusting the API summary.

## Real test vectors (not predicted from memory)

**Source:** `raw.githubusercontent.com/snowballstem/snowball-data/master/english/
voc.txt` (input words) and `.../output.txt` (their stems), line-aligned, 42,649
entries. Fetched and cross-referenced directly — an earlier draft of the analyzer-
descriptor worked example risked guessing stemmer output from memory (e.g. whether
`"quickly"` stems to `"quick"` or is left unstemmed), which this vendoring avoids by
checking the authoritative reference data instead.

| input      | stem     | line (voc.txt / output.txt) |
| ---------- | -------- | ---------------------------- |
| `whales`   | `whale`  | 41637                        |
| `whale`    | `whale`  | 41632                        |
| `swim`     | `swim`   | 36760                        |
| `swimming` | `swim`   | 36763                        |
| `quickly`  | `quick`  | 29952                        |
| `quick`    | `quick`  | 29945                        |
| `the`      | `the`    | 37423 (irrelevant to stemming — a stopword, removed before this step in a typical chain) |
| `running`  | `run`    | 32214                        |
| `run`      | `run`    | 32204                        |
| `runs`     | `run`    | 32215                        |
