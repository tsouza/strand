# R11(c) — Quickwit License Status and Split/Hotcache Source Audit

Vendored findings for `docs/ledger.md`'s R11(c): "Quickwit split/hotcache internals
post-relicense, testing the inherits-from-the-fork hypothesis." Everything below is
fetched directly from `github.com/quickwit-oss/quickwit` and `github.com/quickwit-oss/
tantivy` (raw file content and the GitHub REST API for commit metadata), and from
Quickwit's own acquisition blog post, on 2026-08-19. No claim here is taken from
memory or from this project's own prior, unverified note in `docs/lineage.md` ("Quickwit
is now Apache-2.0 under Datadog") — that note is independently confirmed below, byte-level
for the license and source-level for the hotcache/split code.

## Part 1 — License status

### Current LICENSE file (verified byte-level)

Fetched from `https://raw.githubusercontent.com/quickwit-oss/quickwit/main/LICENSE`,
2026-08-19. First lines:

    Apache License
    Version 2.0, January 2004
    http://www.apache.org/licenses/

The file is the standard Apache-2.0 license text in full (verified by fetch; not
reproduced here beyond the header, per this project's own citation discipline for
long license texts — see `references/tantivy-LICENSE.txt`, `references/faiss-LICENSE.txt`
for the same pattern). **Every source file checked in Part 2 below also carries a
per-file Apache-2.0 header**, e.g. `quickwit-directories/src/hot_directory.rs`:

    // Copyright 2021-Present Datadog, Inc.
    //
    // Licensed under the Apache License, Version 2.0 (the "License");
    // you may not use this file except in compliance with the License.
    // You may obtain a copy of the License at
    //
    //     http://www.apache.org/licenses/LICENSE-2.0
    //
    // Unless required by applicable law or agreed to in writing, software
    // distributed under the License is distributed on an "AS IS" BASIS,
    // WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.

### The relicense event itself (commit-level, not just current-state)

GitHub API history of the `LICENSE` path (`api.github.com/repos/quickwit-oss/quickwit/
commits?path=LICENSE`) shows exactly one relicensing commit, with a same-day follow-up
fix:

| commit (short) | date (UTC) | message |
| --- | --- | --- |
| `c8f03d4fb7` | 2021-04-13 | Initial commit |
| `ef6245478c` | 2021-05-07 | Added license information |
| `3bb2781604` | 2025-01-23 | Relicense to Apache 2.0 (#5645) |
| `54f9667c7b` | 2025-02-25 | Fix license change |

Commit `3bb2781604a50bab891c57812eedd42a0a32e712` ("Relicense to Apache 2.0 (#5645)",
authored 2025-01-23) is the operative change. Its file-status list, fetched from
`api.github.com/repos/quickwit-oss/quickwit/commits/3bb2781604a50bab891c57812eedd42a0a32e712`,
shows the license files touched:

    LICENSE                    added
    LICENSE.md                 removed
    LICENSE_AGPLv3.0.txt       removed

This is direct, primary confirmation that the prior license was **AGPLv3** (the removed
file's own name: `LICENSE_AGPLv3.0.txt`) and the new one is **Apache-2.0** — not
paraphrase, the actual GitHub diff metadata. The same commit touched essentially the
entire codebase (every source file's license header line), including, notably for
Part 2 below, every file in `quickwit-directories/src/` (`bundle_directory.rs`,
`caching_directory.rs`, `debug_proxy_directory.rs`, `hot_directory.rs`,
`storage_directory.rs`, `union_directory.rs`) — the split/hotcache code was relicensed
in the same commit as everything else, with no separate carve-out.

### The announcement

Datadog's acquisition of Quickwit and the relicense were announced together. Quickwit's
own blog post (`quickwit.io/blog/quickwit-joins-datadog`, fetched 2026-08-19) states:

> "to ensure our open-source community can continue, we will soon release a major
> update of both Quickwit with a relicense to Apache License 2.0 and tantivy."

> "we will soon release a new version of Quickwit under the Apache License 2.0."

The announcement is dated around 2025-01-09 (per the corroborating Hacker News
discussion thread and Datadog's own mirrored post, `datadoghq.com/blog/datadog-acquires-
quickwit/`); the actual code relicense landed two weeks later, 2025-01-23, in commit
`3bb2781604` above — a real, verifiable case of "announced" preceding "committed," not
a discrepancy.

### Compatibility verdict

**Quickwit is Apache-2.0, confirmed at the file level (license text) and the commit
level (the relicense diff), not just asserted in a blog post.** This is fully compatible
with `CLAUDE.md`'s "every dependency must be Apache-2.0-compatible, no exceptions" —
Apache-2.0 is this project's own license, so depending on or vendoring Quickwit code
(subject to retaining its copyright notices and NOTICE obligations, standard Apache-2.0
terms, same as this project's own existing tantivy/FAISS vendoring discipline) carries
no license conflict. This closes the license half of R11(c) cleanly: unlike the AGPLv3
status this project's earlier lineage note implicitly worried about superseding, there
is no copyleft obstacle to studying, adapting patterns from, or even directly reusing
(under Apache-2.0 terms) Quickwit's split/hotcache code today.

## Part 2 — The split/hotcache source: does it patch tantivy, or consume it?

### Where the code lives

`quickwit/quickwit-directories/` — its own crate, `Cargo.toml` description field:

> `description = "Custom \`tantivy::Directory\` implementations for Quickwit"`

`src/lib.rs`'s own doc comment states the whole crate's design in one paragraph
(fetched 2026-08-19, `raw.githubusercontent.com/quickwit-oss/quickwit/main/quickwit/
quickwit-directories/src/lib.rs`):

> "This crate contains all of the building pieces that make quickwit's IO possible.
>
> - The `StorageDirectory` just wraps a `Storage` trait to make it compatible with
>   tantivy's Directory API.
> - The `BundleDirectory` bundles multiple files into a single file.
> - The `HotDirectory` wraps another directory with a static cache.
> - The `CachingDirectory` wraps a Directory with a dynamic cache.
> - The `DebugDirectory` acts as a proxy to another directory to instrument it and
>   record all of its IO."

Every one of these is a **wrapper implementing tantivy's own public `Directory` trait**,
by the crate's own description — not a patch to tantivy's source.

### `HotDirectory` — the actual hotcache implementation

`hot_directory.rs` imports only tantivy's public API surface:

    use tantivy::directory::error::OpenReadError;
    use tantivy::directory::{FileHandle, FileSlice, OwnedBytes};
    use tantivy::error::DataCorruption;
    use tantivy::{Directory, HasLen, Index, IndexReader, ReloadPolicy, TantivyError};

`HotDirectory` wraps an arbitrary `D: Directory` plus a serialized static byte-range
cache, and implements `Directory` itself:

    pub struct HotDirectory {
        inner: Arc<InnerHotDirectory>,
    }

    impl HotDirectory {
        pub fn open<D: Directory>(
            underlying: D,
            hot_cache_bytes: OwnedBytes,
        ) -> anyhow::Result<HotDirectory> { ... }
    }

    impl Directory for HotDirectory {
        fn get_file_handle(&self, path: &Path) -> Result<Arc<dyn FileHandle>, OpenReadError> {
            let file_length = self.inner.cache.get_file_length(path)...;
            let underlying_filehandle = self.inner.underlying.get_file_handle(path)?;
            ...
        }
        fn exists(&self, path: &std::path::Path) -> Result<bool, OpenReadError> { ... }
        fn atomic_read(&self, path: &std::path::Path) -> Result<Vec<u8>, OpenReadError> { ... }
        crate::read_only_directory!();
    }

`read_only_directory!()` is a local macro (defined in `lib.rs`) that stubs out the
mutation-side `Directory` methods (`atomic_write`, `delete`, `open_write`,
`sync_directory`, `watch`, `acquire_lock`) with `unimplemented!("read-only")` or
no-ops — again, ordinary trait-method overrides, nothing that requires access to
tantivy internals.

The hotcache byte format itself (`HotDirectoryMeta`, `SliceCacheIndex`,
`StaticDirectoryCacheBuilder`) is Quickwit's own `postcard`-serialized structure — a
file-path-to-byte-range index over a set of cached slices — entirely defined in this
crate, using only tantivy's `OwnedBytes`/`FileSlice` value types to hold the results.

### `BundleDirectory` — the split file format

`bundle_directory.rs`'s own doc comment states the split's on-disk layout directly:

> "`BundleDirectory` is a read-only directory that makes it possible to open a split
> and serve the file it contains via tantivy's `Directory`. It is the `Directory`
> equivalent of `BundleStorage`.
>
> Split Format:
> `[Files][FilesMetadata][FilesMetadata length 8 byte Little endian][Hotcache][Hotcache
> length 8 byte Little endian]`"

So a Quickwit split is: concatenated tantivy segment files, then a bundle-metadata
block (a Quickwit-defined file-offset table), then the hotcache block, with two trailing
8-byte little-endian length fields — a self-describing, footer-first layout, structurally
the same *shape* of idea as STRAND's own footer/hotcache region (`spec/container.md`
§4) but its own independently-defined byte format, not shared wire bytes.
`get_hotcache_from_split` and `read_split_footer` (both re-exported from `lib.rs`) parse
exactly this layout, again using only `tantivy::directory::{FileHandle, FileSlice}` and
`tantivy::{Directory, HasLen}` from the public API, plus Quickwit's own
`quickwit_storage::Storage` trait for the actual byte fetch:

    use quickwit_storage::{BundleStorageFileOffsets, OwnedBytes, Storage, StorageResult};
    use tantivy::directory::error::OpenReadError;
    use tantivy::directory::{FileHandle, FileSlice};
    use tantivy::{Directory, HasLen};

    impl Directory for BundleDirectory { ... }

### The dependency itself: git-pinned, feature-gated, but still the public API

`quickwit/quickwit-directories/Cargo.toml`:

    [dependencies]
    tantivy = { workspace = true }

Resolved at the workspace root, `quickwit/Cargo.toml`:

    tantivy = { git = "https://github.com/quickwit-oss/tantivy/", rev = "86641f7", default-features = false, features = [
      "lz4-compression",
      "mmap",
      "quickwit",
      "zstd-compression",
      "columnar-zstd-compression",
    ] }

Two things worth separating here, checked directly rather than assumed:

1. **`github.com/quickwit-oss/tantivy` is tantivy's own canonical repository, not a
   separate internal Quickwit fork.** tantivy's own `Cargo.toml` (fetched from that
   same repo's `main` branch) names itself:

       [package]
       name = "tantivy"
       version = "0.27.0"
       license = "MIT"
       homepage = "https://github.com/quickwit-oss/tantivy"
       repository = "https://github.com/quickwit-oss/tantivy"

   Quickwit's team is tantivy's maintainer team; there is no separate "vanilla
   tantivy" repository this diverges from. This project's own `references/
   tantivy-LICENSE.txt` and `docs/ledger.md` already treat tantivy as MIT-licensed
   without flagging a repository ambiguity, consistent with this.

2. **The `quickwit` Cargo feature is a normal, mainline, opt-in feature flag on
   tantivy itself — not a divergent source patch.** tantivy's own `Cargo.toml`
   `[features]` section:

       quickwit = ["sstable", "futures-util", "futures-channel"]

   This turns on tantivy's own `sstable` component plus two async-utility
   dependencies. It is declared and gated exactly like every other feature in that
   same block (`mmap`, `stopwords`, `lz4-compression`, …) — an additive, optional
   capability switch shipped in tantivy's mainline source, not a private patch
   applied only inside a Quickwit-held fork branch.

The git-pin (`rev = "86641f7"`, an unreleased commit rather than a numbered
`crates.io` release) reflects that Quickwit tracks tantivy's development trunk closely
— unsurprising, since they are the same maintainers — not that Quickwit runs
divergent tantivy internals.

## Part 3 — Testing the hypothesis

The ledger's question: does Quickwit's split/hotcache code sit on top of tantivy in a
way that would transfer to or interoperate with a STRAND-modified tantivy fork, or is
it its own independent, incompatible thing layered on internals a STRAND fork would
diverge from?

**Verdict, in two parts, because the honest answer is not a single yes/no:**

**(a) The mechanism transfers; the code does not.** Every piece of Quickwit's
split/hotcache implementation examined above — `HotDirectory`, `BundleDirectory`,
`CachingDirectory`, `StorageDirectory` — is an ordinary implementation of tantivy's
*public* `Directory` and `FileHandle` traits, built entirely from `tantivy::directory::*`
and top-level `tantivy::{Directory, HasLen, ...}` exports. None of the source read
in this audit touches a private tantivy module, patches a tantivy struct in place, or
requires modifying tantivy's own crate to add the capability. This is exactly the
extension point `docs/benchmarks.md`'s tantivy-fork entry already relies on ("a
storage-level `Directory` abstraction") and exactly the shape STRAND's own fork plan
already commits to (engine-constant: "the engine's native read path is retained
intact... only the bytes differ"). A STRAND-modified tantivy fork that preserves the
`Directory`/`FileHandle`/`FileSlice`/`OwnedBytes` public surface — which is the whole
point of keeping the fork's changes confined to a pinned reader-module list per
`docs/benchmarks.md` — would present the identical extension point Quickwit's own code
already proves is sufficient to build a real, production split/hotcache layer against.
In that narrow, structural sense, the hypothesis is **confirmed**: nothing about
Quickwit's approach requires internals a conforming STRAND fork would necessarily
diverge from.

**(b) But "interoperate" cannot mean running Quickwit's actual `BundleDirectory`/
`HotDirectory` code against STRAND segment bytes — the wire formats are unrelated.**
Quickwit's split footer is its own `postcard`-serialized, Quickwit-defined layout
(`[Files][FilesMetadata][...][Hotcache][...]`, `HotDirectoryMeta`'s own
`file_lengths`/`slice_offsets` structure); STRAND's footer/hotcache region
(`spec/container.md` §4) is an independently specified byte layout, invariant-11-pinned
to little-endian, byte-deterministic wire structures of its own. A STRAND read path
inside a tantivy fork still has to write its own `Directory` implementation — a
`StrandDirectory`, structurally the sibling of `BundleDirectory`, not a reuse of it.
Nothing in this crate is a drop-in adapter for foreign footer bytes; every one of its
byte-parsing functions (`split_footer`, `read_split_footer`, `StaticDirectoryCache::open`)
is hard-coded to Quickwit's own format. So the hypothesis, read as "Quickwit's code
would transfer wholesale," is **not** supported — what transfers is the *pattern*
(implement `Directory` against your own footer format), already credited in
`docs/lineage.md`'s "From tantivy / Quickwit" paragraph, not the implementation.

**(c) One real complication for STRAND's own fork plan, surfaced by this audit and
not previously recorded:** "vanilla tantivy" is not a single stable target to fork
from. tantivy's canonical repository is itself Quickwit-maintained, ships a
Quickwit-specific opt-in feature (`quickwit`), and Quickwit's own build pins an
arbitrary unreleased git commit rather than a numbered release. STRAND's tantivy-fork
RFC should pin its own specific commit explicitly for this reason (already planned,
`docs/benchmarks.md`: "it pins one tantivy commit, stated in every report") and should
not assume a numbered `crates.io` tantivy release and tantivy's live git trunk behave
identically — Quickwit's own dependency practice is direct evidence that even
tantivy's primary downstream consumer does not treat the released crate as sufficient
and tracks trunk instead.

## Net for R11(c)

License: Apache-2.0, confirmed byte-level and commit-level, fully compatible with
`CLAUDE.md`'s dependency policy, no exceptions needed. Hypothesis: the *inherits-from-
the-fork* claim is confirmed at the architecture level (Quickwit's split/hotcache
layer is an ordinary public-API `Directory` consumer, the same extension point a
STRAND tantivy fork would expose) and rejected at the code-reuse level (the wire
formats are independent; a STRAND `Directory` implementation still has to be written
from scratch, the same way Quickwit's was). The Quickwit adapter's payoff described in
`docs/benchmarks.md` — "Quickwit's internal hotcache cold-open versus STRAND's
specified one, same engine" — remains intact and, if anything, better grounded now:
both sides would be two independent `Directory` implementations over the *same* tantivy
fork commit, which is precisely the engine-constant comparison `docs/benchmarks.md`
already calls for.
