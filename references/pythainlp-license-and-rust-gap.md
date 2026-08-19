# PyThaiNLP — license, and the absence of a maintained Rust binding

Vendored 2026-08-19. Grounding for RFC 0004's post-approval Discussion amendment
(invariant 6, `CLAUDE.md` §5; `docs/roadmap.md` M1-1).

**Source:** `https://raw.githubusercontent.com/PyThaiNLP/pythainlp/dev/LICENSE`,
fetched live.

License: **Apache License, Version 2.0** — clean, no compatibility question at all.
PyThaiNLP is a serious, actively developed Thai NLP toolkit including dictionary-
and rule-based word tokenizers generally regarded as more accurate for Thai than
ICU's bundled `laodict.txt`-style word lists.

No maintained Rust binding or Rust reimplementation of PyThaiNLP's tokenizer was
found in this pass (this project's `WebSearch` budget was exhausted mid-session
before an exhaustive search could be completed — this absence is reported as "not
found in the searching done here," not as a proven negative). PyThaiNLP is
Python-only in its primary form; using it from STRAND's Rust reference
implementation would mean either an FFI/subprocess boundary or a from-scratch Rust
port, neither of which exists today. This is why PyThaiNLP is named as a
higher-accuracy alternative worth a future revisit (see the RFC amendment's Open
questions) rather than adopted as the default: the format only needs a *default*
identity engines can implement against, and a toolkit with no Rust path today does
not fit STRAND's own reference-implementation constraints as well as a dependency
already available as a Rust crate does.
