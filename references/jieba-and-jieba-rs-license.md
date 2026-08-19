# Jieba (Python/C++) and `jieba-rs` — license and scope

Vendored 2026-08-19. Grounding for RFC 0004's post-approval Discussion amendment
(invariant 6, `CLAUDE.md` §5; `docs/roadmap.md` M1-1).

## Original Jieba

**Source:** `https://raw.githubusercontent.com/fxsjy/jieba/master/LICENSE`, fetched
live.

MIT License, "Copyright (c) 2013 Sun Junyi." Standard MIT terms — permission to
use/copy/modify/merge/publish/distribute/sublicense/sell with notice retention, "AS
IS" warranty disclaimer. The project's default dictionary (`dict.txt`) ships inside
the same MIT-licensed repository with no separate license file found for it,
i.e. it is bundled under the same MIT grant. Apache-2.0-compatible on the same
precedent already applied to tantivy, FAISS, and `cwida/FastLanes` (`CLAUDE.md` §1).

## `jieba-rs`

**Source:** `https://raw.githubusercontent.com/messense/jieba-rs/main/README.md`,
fetched live.

A pure-Rust reimplementation (benchmarked directly against `cppjieba`, not wrapping
it), MIT-licensed, with a `default-dict` Cargo feature — enabled by default — that
embeds a copy of Jieba's own dictionary. Actively maintained (recent 0.10-series
releases, CI badges present).

## Why Jieba/`jieba-rs` is not recommended as STRAND's default

Chinese-only. Invariant 6 requires one schema slot's default to cover CJK
(Chinese **and** Japanese, at minimum), Thai, and Lao. Jieba has no Japanese, Thai,
or Lao segmentation capability at all — adopting it would still require pairing it
with at least two more, unrelated, independently-licensed and independently-
versioned dependencies to cover the rest of invariant 6's script list, the same
multi-dependency fragmentation problem that rules out building the default around
Lindera's per-script sub-crates (`references/lindera-rust-morphological-analyzer.md`).
Its clean MIT licensing is genuine and noted for completeness — a future engine
implementing only Chinese search is free to declare `jieba-rs` as its
`segmentation_dictionary` identity, since invariant 6 only requires a declared
identity and version, not conformance to STRAND's own recommended default.
