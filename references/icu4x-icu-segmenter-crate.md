# ICU4X `icu_segmenter` crate — dictionary-based `WordSegmenter`, license, and version

Vendored 2026-08-19. Grounding for RFC 0004's post-approval Discussion amendment
(invariant 6, `CLAUDE.md` §5; `docs/roadmap.md` M1-1).

## Crate identity and license

**Source:** `https://crates.io/api/v1/crates/icu` (the `icu` meta-crate, which
re-exports `icu_segmenter`).

- Max stable version: **2.3.0**.
- Declared license: **`Unicode-3.0`** (SPDX identifier — see
  `references/icu-license-word-break-dictionaries.md` for the full grant text).
- Repository: `https://github.com/unicode-org/icu4x`.
- Newest version `created_at`: **2026-08-13** — six days before this fetch,
  confirming active maintenance.

## `WordSegmenter` dictionary support

**Source:** `https://docs.rs/icu_segmenter/latest/icu_segmenter/struct.WordSegmenter.html`,
fetched live.

`WordSegmenter` exposes distinct constructors rather than one fixed strategy:

- **`new_dictionary` / `try_new_dictionary`** — "dictionary data for complex scripts
  (Chinese, Japanese, Khmer, Lao, Myanmar, and Thai)." Word-list lookup, not a
  trained model. This constructor's script coverage is a superset of invariant 6's
  named scripts (CJK, Thai, Lao) — it additionally covers Khmer and Myanmar.
- **`new_lstm` / `try_new_lstm`** — "LSTM data for complex scripts (Burmese, Khmer,
  Lao, and Thai)"; explicitly "there is not currently an LSTM model for Chinese or
  Japanese."
- **`new_auto` / `try_new_auto`** — "the LSTM model when available and the
  dictionary model for Chinese and Japanese," i.e. for Thai/Lao/Khmer/Myanmar this
  constructor prefers the **LSTM** path, not the dictionary path, whenever both
  exist. This is the constructor a naive `WordSegmenter::new()`-style default would
  reach for, and it is the wrong one for a descriptor whose schema field is named
  `segmentation_dictionary` and whose invariant assumes dictionary-based
  segmentation for Thai and Lao specifically, not a neural model.
- **`new_for_non_complex_scripts`** and **`new_neo_for_non_complex_scripts`** —
  UAX #29 rule-based segmentation for scripts that already have space/rule-visible
  boundaries; not relevant to the dictionary-segmented-script question.

Both dictionary and LSTM strategies ship as `compiled_data` — the crate bundles the
data at compile time; there is no runtime fetch. Custom `BufferProvider`/
`DataProvider` implementations exist for callers who want a different data source
or version, but the default path is a single pinned crate version's bundled data.

## Bearing on the RFC amendment

`try_new_dictionary()` is the constructor whose behavior matches invariant 6's own
assumption (word-list-based segmentation, an "identity and version" naming a
dictionary, not a model). `try_new_auto()` is a real trap: it silently substitutes
LSTM for exactly the two scripts (Thai, Lao) the descriptor schema's own name
implies are dictionary-segmented, so any future implementation session pinning this
crate MUST call `try_new_dictionary()` explicitly and say so in code comments and
the eventual implementation-track RFC, not rely on `try_new_auto()`'s default
selection.

The crate is pure Rust with a `Unicode-3.0` license reaching both the segmentation
code and its bundled dictionary data (word lists for Han, Kana, Thai, Lao, Khmer,
Myanmar derived from ICU's own upstream sources — see
`references/icu-license-word-break-dictionaries.md`); it introduces no native/C
build dependency, unlike `rust_icu`
(`references/rust-icu-icu4c-binding-crate.md`).
