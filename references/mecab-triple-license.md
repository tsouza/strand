# MeCab — triple license (GPL/LGPL/BSD), and why this doesn't settle the question by itself

Vendored 2026-08-19. Grounding for RFC 0004's post-approval Discussion amendment
(invariant 6, `CLAUDE.md` §5; `docs/roadmap.md` M1-1).

**Source:** `https://raw.githubusercontent.com/taku910/mecab/master/mecab/COPYING`,
fetched live in full (4 lines, verbatim):

> MeCab is copyrighted free software by Taku Kudo <taku@chasen.org> and
> Nippon Telegraph and Telephone Corporation, and is released under
> any of the GPL (see the file GPL), the LGPL (see the file LGPL), or the
> BSD License (see the file BSD).

A recipient may choose any one of the three. The **BSD** option is, on its own
terms, Apache-2.0-compatible — the same conclusion `CLAUDE.md` §1 already reaches
for MIT-family permissive licenses. This means MeCab's *code* license is not
actually the blocker a triple-license listing might suggest at a glance: a
distributor can pick BSD and be in the same permissive-license family as every
other accepted dependency.

## Why MeCab (the C/C++ library) is still not recommended

STRAND's Rust reference implementation has no existing C/C++ FFI dependency of
MeCab's shape, and linking the actual MeCab library (rather than a from-scratch
Rust reimplementation) would carry the same native-dependency reproducibility
concern already raised against `rust_icu`
(`references/rust-icu-icu4c-binding-crate.md`) — a real, but secondary, argument.
The more direct reason MeCab itself is not this RFC amendment's recommendation:
Lindera already exists as a mature, actively maintained, pure-Rust reimplementation
of MeCab-shaped tokenization (`references/lindera-rust-morphological-analyzer.md`),
so there is no need to take on MeCab's own C++ build and its license-choice
bookkeeping just to reach the same IPADIC-backed Japanese segmentation Lindera
already provides natively in Rust. MeCab's dictionary story (IPADIC via NAIST/ICOT,
`references/icu-license-word-break-dictionaries.md`) is identical either way, since
Lindera vendors the same IPADIC release.
