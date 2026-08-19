# Lindera — pure-Rust morphological analyzer, license, and dictionary sub-crates

Vendored 2026-08-19. Grounding for RFC 0004's post-approval Discussion amendment
(invariant 6, `CLAUDE.md` §5; `docs/roadmap.md` M1-1) — Lindera and its bundled
dictionaries are a live candidate for the Japanese/Chinese half of a
`segmentation_dictionary` default; ultimately not the one recommended (see the RFC
amendment for the reasoning).

## Crate identity and license

**Source:** `https://crates.io/api/v1/crates/lindera` and
`https://raw.githubusercontent.com/lindera/lindera/main/LICENSE`, both fetched live.

- Max stable version: **5.3.0**.
- Newest version `created_at`: **2026-08-16** — three days before this fetch,
  strongly actively maintained.
- Declared license: **MIT** — "Copyright (c) 2019 by the project authors... THE
  SOFTWARE IS PROVIDED 'AS IS'..." — standard MIT terms, Apache-2.0-compatible on
  the same precedent this project already applies to tantivy and FAISS
  (`CLAUDE.md` §1).
- Repository: `https://github.com/lindera/lindera` — a from-scratch Rust
  reimplementation (forked from `kuromoji-rs`), **not** an FFI wrapper around the
  C++ MeCab library, so it does not inherit MeCab's own triple GPL/LGPL/BSD license
  choice or any GPL/LGPL exposure at all.
- Sub-crates for individual dictionaries: `lindera-ipadic`, `lindera-unidic`,
  `lindera-ko-dic`, `lindera-cc-cedict`, `lindera-jieba`, `lindera-ipadic-neologd`.

## Dictionary-specific licensing (each carries its own `NOTICE.txt`, distinct from Lindera's own MIT code license)

- **`lindera-ipadic`** (Japanese, `mecab-ipadic-2.7.0-20070801`): NAIST copyright
  plus ICOT Free Software terms — permissive, redistribution-permitting,
  warranty-disclaiming, no copyleft. Byte-identical terms to the `IPADIC` notice
  already vendored in `references/icu-license-word-break-dictionaries.md` (both
  trace to the same NAIST/ICOT source). Apache-2.0-compatible.
- **`lindera-ko-dic`** (Korean, `mecab-ko-dic-2.1.1-20180720`): **Apache License
  2.0**, cleanest of all four. Korean is not one of invariant 6's named
  dictionary-segmented scripts (CJK, Thai, Lao) — Hangul text is conventionally
  space-delimited — so this is context, not a candidate STRAND needs to act on.
- **`lindera-cc-cedict`** (Chinese, `CC-CEDICT-MeCab-0.1.0-20200409`, derived from
  CEDICT via `mdbg.net/chinese/dictionary`): **"This work is licensed under a
  Creative Commons Attribution-ShareAlike 4.0 International License"** — a
  share-alike copyleft condition on the *data*. Full detail and the compatibility
  analysis: `references/cc-cedict-and-lindera-cc-cedict-license.md`.
- **`lindera-unidic`**: not separately fetched in this pass (Japanese-only, same
  NAIST-derived lineage as IPADIC by reputation; not needed to settle this RFC
  amendment since `lindera-ipadic`'s terms already establish the Japanese-dictionary
  license family is permissive).

## Bearing on the RFC amendment

Lindera is real evidence that a pure-Rust, actively maintained, MIT-licensed
morphological analyzer exists and is already used in production Rust search
tooling (`tantivy-lindera`). Its Japanese dictionary (IPADIC via ICOT/NAIST terms)
is clean. Its Chinese dictionary (CC-CEDICT) is not — CC BY-SA 4.0's share-alike
condition is a real license problem for a project that vendors normative
conformance golden files under Apache-2.0 (`conformance/analyzers/`, invariant 6).
Lindera also does not cover Thai or Lao at all — no Thai/Lao dictionary sub-crate
exists in this project — so on its own it cannot be a single default for all of
invariant 6's named scripts; it would need to be paired with an unrelated
Thai/Lao-specific dependency, multiplying the number of independently-versioned,
independently-licensed dictionary families the descriptor's `segmentation_dictionary`
field has to name identities for. This multi-dependency shape is the main reason
this RFC amendment does not recommend Lindera as STRAND's single default, in favor
of ICU4X's `icu_segmenter`, which covers Chinese, Japanese, Thai, and Lao from one
license family and one versioned dependency (`references/
icu4x-icu-segmenter-crate.md`).
