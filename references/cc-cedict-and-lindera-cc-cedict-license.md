# CC-CEDICT / `lindera-cc-cedict` — share-alike license, why it is rejected as a STRAND default

Vendored 2026-08-19. Grounding for RFC 0004's post-approval Discussion amendment
(invariant 6, `CLAUDE.md` §5; `docs/roadmap.md` M1-1).

**Source:** `https://raw.githubusercontent.com/lindera/lindera/main/lindera-cc-cedict/NOTICE.txt`,
fetched live in full.

The notice states Lindera's Chinese dictionary sub-crate bundles
`CC-CEDICT-MeCab-0.1.0-20200409`, itself derived from CEDICT
(`https://www.mdbg.net/chinese/dictionary?page=cedict`). Its license, quoted
directly from the notice: **"This work is licensed under a Creative Commons
Attribution-ShareAlike 4.0 International License"**
(`https://creativecommons.org/licenses/by-sa/4.0/`). CC BY-SA 4.0 permits
commercial and non-commercial use and modification on two conditions: attribution,
and — the operative one here — that "if you remix, transform, or build upon the
material, you must distribute your contributions under the same license as the
original."

## Why this is a real blocker, not a formality

`CLAUDE.md` §1 states STRAND's license policy in absolute terms: "Every dependency
must be Apache-2.0-compatible. No exceptions." That policy has so far been applied
to code dependencies (RaBitQ reference implementations, `cwida/FastLanes`, tantivy,
FAISS). A vendored *dictionary* is a data dependency, but invariant 6 makes
analyzer conformance vectors — including, necessarily, real dictionary-segmented
example text and its expected token stream — normative
(`conformance/analyzers/`, `spec/analyzer-descriptors.md` §7): a
dictionary-segmentation conformance vector cannot be produced or checked without
the actual dictionary content driving it. If that dictionary is CC BY-SA licensed,
the share-alike clause would attach to derivative works built from it — plausibly
including a golden conformance vector file distributed as part of STRAND's own
Apache-2.0-licensed `conformance/` tree. That is precisely the kind of downstream
obligation `CLAUDE.md`'s "no exceptions" policy exists to avoid. Whether a specific
golden-file vector actually counts as a CC BY-SA "adaptation" under a strict legal
reading is exactly the sort of ambiguity this project's stated policy — grounded in
its own R3/R9 practice of checking licenses via primary sources rather than
assuming — says not to take a chance on.

This is why CC-CEDICT (and, by extension, `lindera-cc-cedict`) is named and
rejected explicitly in the RFC amendment rather than silently passed over: Jieba's
own dictionary is MIT (`references/jieba-and-jieba-rs-license.md`), a real,
available, unencumbered alternative for Chinese specifically, but it does not
extend to Japanese, Thai, or Lao, which is the deciding factor against building
STRAND's default around a Chinese-specific tool at all (see the RFC amendment).
