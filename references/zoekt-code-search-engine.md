# Zoekt: Trigram-Based Code Search Engine

Vendored excerpt, fetched 2026-08-20 to ground `docs/lineage.md`'s Zoekt entry
and close `docs/roadmap.md`'s D-4 item — real, current primary sources, not a
remembered summary (`CLAUDE.md` §3). This fetch also independently
re-verifies, against Zoekt's own real design documentation rather than from
memory, the characterization of Zoekt already cited in `docs/ledger.md`'s
"code row-IDs stay file-granular" settled entry ("Zoekt and ctags recompute
symbol positions wholesale, with no persisted identity across a re-index at
all").

**Sources (all fetched live via `gh api repos/sourcegraph/zoekt/contents/
<path>`, `main` branch, 2026-08-20):**

- `repos/sourcegraph/zoekt` (repository metadata) — `.license.spdx_id`.
- `LICENSE` — the actual license file text.
- `README.md` — project overview, usage, fork history.
- `doc/design.md` — indexing algorithm, index shard format, ranking signals.
- `doc/ctags.md` — the ctags symbol-extraction integration.
- `doc/faq.md` — supplementary confirmation of ranking-signal usage.

## License

`gh api repos/sourcegraph/zoekt --jq '.license.spdx_id'` returns
`Apache-2.0`, confirmed live via GitHub's license-detection API. The actual
`LICENSE` file (fetched separately, not inferred from the API alone) is the
standard Apache License, Version 2.0 boilerplate — confirmed by reading its
opening "TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION"
section directly. This resolves the licensing question this project had left
open (`docs/roadmap.md`'s original D-4 wording: "license and technical shape
not yet checked in this project") without qualification: Zoekt is
Apache-2.0 today, at the repository this project depends on
(`github.com/sourcegraph/zoekt`).

The README itself flags a real fork history worth recording precisely,
because Sourcegraph-authored projects have had license changes elsewhere in
their history and this project's own instructions call for checking the
current state rather than assuming it:

> "**Note:** This has been the maintained source for Zoekt since 2017, when
> it was forked from the original repository
> [github.com/google/zoekt](https://github.com/google/zoekt)."

The fork is from Google's original zoekt (also Apache-2.0), not from a
Sourcegraph product with different licensing terms; nothing in the README,
`doc/`, or `LICENSE` suggests any subsequent relicensing.

## What Zoekt is (README, "Background")

> "Zoekt supports fast substring and regexp matching on source code, with a
> rich query language that includes boolean operators (and, or, not). It can
> search individual repositories, and search across many repositories in a
> large codebase. Zoekt ranks search results using a combination of code-
> related signals like whether the match is on a symbol. Because of its
> general design based on trigram indexing and syntactic parsing, it works
> well for a variety of programming languages."

Unlike ravel (this task's other vendored candidate), Zoekt is directly
comparable to STRAND's lexical half: both are trigram/token-indexed
substring-and-regexp search systems over source code corpora, not adjacent
storage infrastructure.

## Positional trigram index (`doc/design.md`, "Positional trigrams")

> "We build an index of ngrams (n=3), where we store the offset of each
> ngram's occurrence within a file. For example, if the corpus is 'banana'
> then we generate the index
>
> ```
> "ban": 0
> "ana": 1,3
> "nan": 2
> ```
>
> If we are searching for a string (eg. 'The quick brown fox'), then we look
> for two trigrams (eg. 'The' and 'fox'), and check that they are found at
> the right distance apart."

> "Empirically, [the index] is about 3x the corpus size, composed of 2x
> (offsets), and 1x (original content)."

## Index format — file-granular shard layout (`doc/design.md`, "Index
## format")

Quoted in full, because this is the section that settles the granularity
question:

> "The index is organized in shards, where each shard is a file, laid out
> such that it can be mmap'd efficiently. A shard can contain one repository
> or, after shard merging, multiple repositories in a compound shard. The
> basic data in an index shard are the following
>
>    * file contents
>    * filenames
>    * the content posting lists (varint encoded)
>    * the filename posting lists (varint encoded)
>    * branch masks
>    * metadata (repository name, index format version, etc.)
>
> In practice, the shard size is about 3.5x the corpus size, composed of
> original content, posting lists, and other metadata."

The addressable unit inside a shard is the **file** (a "file blob" carrying a
per-branch bitmask, per the "Branches" section below) — there is no separate,
persisted symbol-level object in the shard's own data list. Trigram posting
lists point into file content; nothing in this section, or anywhere else in
`doc/design.md`, describes a symbol-indexed or symbol-addressed structure
distinct from the file-level content and posting lists.

> "Branches: Each file blob in the index has a bitmask, representing the
> branches in which the content is found... With this technique, we can
> index many similar branches of a repository with little space overhead."

## Symbols are a ranking signal computed by ctags, not a persisted identity
(`doc/design.md`, "Ranking"; `doc/ctags.md`)

> "In absense of advanced signals (e.g. pagerank on symbol references),
> ranking options are limited: the following signals could be used for
> ranking
>
>    * number of atoms matched
>    * closeness to matches for other atoms
>    * quality of match: does match boundary coincide with a word boundary?
>    * file latest update time
>    * filename lengh
>    * tokenizer ranking: does a match fall comment or string literal?
>    * symbol ranking: it the match a symbol definition?
>
> For the latter, it is necessary to find symbol definitions and other
> sections within files on indexing. Several (imperfect) programs to do this
> already exist, eg. `ctags`."

`doc/ctags.md` confirms the mechanism is an external tool invoked at index
time, not a Zoekt-native persisted symbol table:

> "Ctags generates indices of symbol definitions in source files... Zoekt
> supports [universal-ctags](https://github.com/universal-ctags)... It is
> strongly recommended to use Universal Ctags... running on the Linux
> platform. From this version on, universal ctags will be called using
> seccomp, which guarantees that security problems in ctags cannot escalate
> to access to the indexing machine."

The README's installation section states the same framing directly: "It is
also recommended to install Universal ctags, as symbol information is a key
signal in ranking search results" — symbol information is explicitly a
*ranking signal*, computed by a sandboxed external process re-run at index
time, not a row/identity Zoekt itself tracks across index builds. Nothing in
`doc/design.md`, `doc/ctags.md`, `doc/faq.md`, `doc/indexing.md`, or the
README describes symbol renames, symbol deletions, or any cross-reindex
identity concept for a symbol — reindexing a repository (`zoekt-git-index`)
regenerates the shard, including its ctags-derived symbol boosts, from
scratch each time; the design doc's own description of a format-version
upgrade path ("generate shards in the new format, kill old search service,
start new search service, delete old shards") is the closest thing to a
reindex-lifecycle description in the docs, and it is whole-shard replacement,
consistent with symbols being recomputed rather than incrementally
maintained.

## Confirmation of `docs/ledger.md`'s existing characterization

`docs/ledger.md`'s "code row-IDs stay file-granular" settled entry
(2026-08-20) already states: "Zoekt and ctags recompute symbol positions
wholesale, with no persisted identity across a re-index at all." Every
primary source fetched for this entry — `doc/design.md`'s "Index format" and
"Ranking" sections, `doc/ctags.md` in full, and the README's own framing of
symbol information as a ranking signal — is **consistent with, and confirms,
this characterization**. Zoekt's shard is organized around files (content,
filenames, and posting lists over both), symbol positions are supplied by an
external ctags invocation used only to boost ranking, and no document
anywhere in the fetched set describes a persisted, addressable, cross-reindex
symbol identity of any kind. This is an independent confirmation, not a
correction: `docs/ledger.md`'s row-ID-granularity decision needs no revision
from this fetch.
