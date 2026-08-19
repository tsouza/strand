# `rust_icu` — native ICU4C bindings, license, and the dictionary-based classic break iterator

Vendored 2026-08-19. Grounding for RFC 0004's post-approval Discussion amendment
(invariant 6, `CLAUDE.md` §5; `docs/roadmap.md` M1-1).

## Crate identity and license

**Source:** `https://crates.io/api/v1/crates/rust_icu`.

- Max stable version: **5.7.0**.
- Declared license: **Apache-2.0** (the Rust binding code itself; the linked ICU4C
  library carries ICU's own Unicode-3.0 terms, `references/
  icu-license-word-break-dictionaries.md`).
- Repository: `https://github.com/google/rust_icu` — "Native bindings to the ICU4C
  library from Unicode."
- Newest version `created_at`: **2026-07-06** — about six weeks before this fetch,
  actively maintained, published under Google's account.

## ICU4C's classic word-break iterator is dictionary-based for the invariant-6 scripts

**Source:** `https://unicode-org.github.io/icu/userguide/boundaryanalysis/`, fetched
live.

"ICU provides dictionary support for word boundaries in Chinese, Japanese, Thai,
Lao, Khmer and Burmese. Use of the dictionaries is automatic when text in one of the
dictionary languages is encountered." The dictionaries live at
`icu4c/source/data/brkitr/dictionaries` and compile into ICU's binary data during
the standard build. Unlike ICU4X's `WordSegmenter::new_auto()`
(`references/icu4x-icu-segmenter-crate.md`), ICU4C's default classic break iterator
does not fork between an LSTM model and a dictionary per script — dictionary lookup
is simply what happens automatically for these six languages, matching invariant
6's own assumption without a constructor-selection trap.

## Trade-off against the ICU4X path

`rust_icu` requires linking against a system- or build-provided ICU4C library — a
native C/C++ dependency, not a pure-Rust crate. This has two consequences relevant
to `CLAUDE.md`'s own discipline:

1. **Invariant 11 (byte determinism).** "Two independent implementations given the
   same logical input MUST produce the same index." A pure-Rust dependency pinned
   by `Cargo.lock` gives every implementation the same compiled artifact by
   construction; a system-linked ICU4C's exact build (compiler flags, exact patch
   version, distro packaging) can vary across platforms unless STRAND additionally
   vendors and pins an exact ICU4C source build — real, but avoidable, additional
   engineering the ICU4X path does not need.
2. **Invariant 9 / repository conventions (`CLAUDE.md` §8).** STRAND's reference
   implementation targets "edition 2024... `unsafe` only in reviewed, documented
   blocks," with SIMD kept to stable-Rust crates and no nightly dependency; a native
   C-library FFI binding is a heavier, different kind of dependency than anything
   else the reference implementation currently links.

`rust_icu`'s own code license (Apache-2.0) and the ICU4C dictionaries it exposes
(Unicode-3.0 plus the same permissive third-party notices covering `cjdict.txt` and
`laodict.txt`) are not the blocker — the blocker, if any, is the native-dependency
reproducibility question, which is why this RFC amendment recommends the pure-Rust
ICU4X path as the default rather than `rust_icu`, while noting `rust_icu` remains a
legitimate, Apache-2.0-compatible alternative an engine could declare instead, since
invariant 6 only requires the descriptor name a dictionary identity and version, not
STRAND's own reference implementation's specific dependency.
