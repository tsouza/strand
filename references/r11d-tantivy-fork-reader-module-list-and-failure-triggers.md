# R11(d) — tantivy fork reader-module list, and the fork failure triggers it arms

Vendored excerpts. Fetched 2026-08-19, against live source (`gh api`), not memory, per
`CLAUDE.md` §3 — the same pinned tag R11(a) used, for consistency, plus one clearly
labeled excursion onto tantivy's unreleased `main` branch where that is the only way to
answer "how stable is this module" honestly (Part 2 below).

Cited by: `docs/ledger.md` R11(d), `docs/roadmap.md` M4-1(d), `docs/benchmarks.md`'s
tantivy-fork paragraph ("a reader-module list pinned in the R11 RFC" and its three
failure-trigger conditions), and — going forward — the M4-4 fork RFC this grounds.

Answers, up front: a STRAND-compatible tantivy fork touches two independent layers.
**Layer 1 — file virtualization** (which physical files exist, where their bytes live)
is solvable entirely through a custom `Directory` implementation, no tantivy-internals
patch required; R11(c) already confirmed this is exactly what Quickwit's own
`Directory`-implementing crates do. **Layer 2 — the byte layout inside each
component** is not pluggable at all (R11(a)'s finding stands); a fork must patch the
concrete reader types in `src/index/`, `src/postings/`, `src/positions/`,
`src/termdict/fst_termdict/`, and `src/fieldnorm/` directly. This document pins the
Layer-2 module list for a **lexical-only** fork (the scope `docs/roadmap.md`'s M4-4
entry already narrowed the fork to, since tantivy has no vector-blob equivalent), and
grounds three concrete facts discovered while reading current and near-current source
that bear directly on the three failure triggers `docs/benchmarks.md` already states:
tantivy's postings codec uses a different block granularity than STRAND's (128 vs.
256, a real, checkable mismatch), tantivy's default read path already computes and
uses a BM25 block-pruning bound (block-max-WAND) that STRAND's own postings blob does
not register at all yet — RFC 0007 explicitly defers it as future work — so the fork
inherits an open cross-project dependency, not just a byte-layout translation, and the
exact module surface a fork must touch is under real, recent, substantive churn —
including one 44-file internal refactor that landed on tantivy's `main` nine days
before this document was written.

---

## Part 0 — the two-layer framing, confirmed from source

**Source:** `src/index/segment.rs` (full file, tag `0.26.1`):

> ```rust
> impl Segment {
>     /// Returns the relative path of a component of our segment.
>     ///
>     /// It just joins the segment id with the extension
>     /// associated with a segment component.
>     pub fn relative_path(&self, component: SegmentComponent) -> PathBuf {
>         self.meta.relative_path(component)
>     }
>
>     /// Open one of the component file for a *regular* read.
>     pub fn open_read(&self, component: SegmentComponent) -> Result<FileSlice, OpenReadError> {
>         let path = self.relative_path(component);
>         self.index.directory().open_read(&path)
>     }
> }
> ```

Every component read starts here: `SegmentComponent` → a relative path
(`<segment-uuid>.<extension>`, e.g. `idx`, `pos`, `term`, `fast`, `fieldnorm`, `store`,
`del`) → `Directory::open_read`. tantivy expects **one physical file per component per
segment**; STRAND's container is one object with its own footer, blob registry, and
chunk/block split (`spec/container.md`). A custom `Directory` can map each of these
virtual per-extension paths to byte ranges inside one STRAND container object without
touching a single line of `segment.rs` or `segment_reader.rs` — this is Layer 1, and it
is not a novel claim: it is the identical extension point R11(c) confirmed Quickwit's
`HotDirectory`/`BundleDirectory`/`CachingDirectory` already use, unmodified, against
tantivy's public `Directory`/`FileHandle` traits (`references/r11c-quickwit-relicense-
and-hotcache-source.md`).

What a `Directory` swap cannot fix is **what the bytes mean once fetched** — the
concrete decode logic wired to each component. That is Layer 2, and it is what the
module list below pins.

---

## Part 1 — the Layer-2 reader-module list (lexical-only fork scope)

**Source repo:** `github.com/quickwit-oss/tantivy`, tag `0.26.1`
(commit `0093923d94157d9f1f63a292bb504bb8db401f2a`), the same pin as R11(a).

### A. Segment-open orchestration — must be patched

- **`src/index/segment_reader.rs`** — `SegmentReader::open_with_custom_alive_set`
  (lines ~148–208) is the literal entry point: it opens `SegmentComponent::Terms`
  as `CompositeFile::open`, `SegmentComponent::Postings`/`Positions` likewise as
  `CompositeFile::open`, then hands the resulting `CompositeFile`s to
  `InvertedIndexReader`. None of these calls accept a codec parameter — the fork's
  central patch site.
- **`src/index/inverted_index_reader.rs`** — `InvertedIndexReader` (struct + impl):
  `get_term_info`, `read_block_postings_from_terminfo`, `read_postings_from_terminfo`.
  This is the per-field object a query actually calls; it owns the concrete
  `TermDictionary`, and slices `postings_file_slice`/`positions_file_slice` by
  `TermInfo::postings_range`/`positions_range` before handing bytes to
  `BlockSegmentPostings::open` and `PositionReader::open`.
- **`src/index/segment_component.rs`** — the component-tag enum tantivy's `open_read`
  dispatches on. At the pinned tag it is a **closed seven-variant enum** (Postings,
  Positions, FastFields, FieldNorms, Terms, Store, Delete) — see Part 2 for why this
  line item is no longer simply true on `main`.
- **`src/index/segment.rs`** — `Segment::open_read`/`relative_path` (quoted above);
  read-only, no decode logic, but the fork's `Directory` must agree with its
  extension-to-path convention (or this file must be patched too, if the fork chooses
  to bypass the extension convention rather than emulate it).
- **`src/directory/composite_file.rs`** — `CompositeFile::open`/`CompositeWrite`. This
  is tantivy's own "several fields packed into one physical file" layer: a per-file
  footer holding a `(FileAddr, offset)` table (`VInt` count, then
  `offset`/`field`/`idx` triples, then a `u32` footer length). It is itself a
  tantivy-specific wire format, unrelated to STRAND's per-field blob layout
  (`spec/lexical.md`), so the fork must either reimplement this exact footer over
  STRAND bytes (pointless — it would mean writing tantivy's wire format, not reading
  STRAND's) or patch `segment_reader.rs`/`inverted_index_reader.rs` to route around
  `CompositeFile` entirely and address STRAND's own per-field offsets directly. Either
  way this module is in scope, not a pass-through.

### B. Postings decode — must be patched, and the mismatch is structural, not cosmetic

- **`src/postings/compression/mod.rs`** — `BlockDecoder`, `COMPRESSION_BLOCK_SIZE`.
  **Source** (full file read, tag `0.26.1`):
  > ```rust
  > use bitpacking::{BitPacker, BitPacker4x};
  > pub const COMPRESSION_BLOCK_SIZE: usize = BitPacker4x::BLOCK_LEN;
  > ```
  `BitPacker4x::BLOCK_LEN` is **128**. STRAND's own registered postings codec is
  **`BitPacker8x`, 256-value blocks** (`rfcs/0007-postings-codec.md`,
  `crates/strand-lexical/src/postings.rs`'s `BLOCK_LEN`, `docs/ledger.md`'s R9
  correction). This is a real, checkable block-granularity mismatch, not a relabeling:
  every loop in this module and in `block_segment_postings.rs`/`skip.rs` that indexes
  by `COMPRESSION_BLOCK_SIZE` assumes 128-wide blocks, and STRAND's on-disk blocks are
  256 values wide. A byte-identical drop-in is not possible; the block-iteration logic
  itself must change, not just the source of the bytes.
- **`src/postings/block_segment_postings.rs`** — `BlockSegmentPostings::open`/`reset`/
  `advance`, `decode_bitpacked_block`/`decode_vint_block`. Directly imports
  `COMPRESSION_BLOCK_SIZE` and `BlockDecoder` from the module above (confirmed by
  reading the file's own `use` block) and owns the `SkipReader` that walks block
  boundaries — the concrete type `InvertedIndexReader::read_block_postings_from_terminfo`
  constructs.
- **`src/postings/skip.rs`** — `SkipReader`, `BlockInfo::BitPacked`/`VInt`,
  `block_max_score`. **Source** (full file read, tag `0.26.1`):
  > ```rust
  > pub(crate) enum BlockInfo {
  >     BitPacked {
  >         doc_num_bits: u8, strict_delta_encoded: bool, tf_num_bits: u8, tf_sum: u32,
  >         block_wand_fieldnorm_id: u8, block_wand_term_freq: u32,
  >     },
  >     VInt { num_docs: u32 },
  > }
  > // Returns the block max score for this block if available.
  > pub fn block_max_score(&self, bm25_weight: &Bm25Weight) -> Option<Score> {
  >     match self.block_info {
  >         BlockInfo::BitPacked { block_wand_fieldnorm_id, block_wand_term_freq, .. } =>
  >             Some(bm25_weight.score(block_wand_fieldnorm_id, block_wand_term_freq)),
  >         BlockInfo::VInt { .. } => None,
  >     }
  > }
  > ```
  This is tantivy's block-max-WAND bound: two raw per-block statistics (a
  representative fieldnorm id, a max term frequency) written inline in each
  bit-packed block's header (`SkipSerializer::write_blockwand_max`, same file),
  coupled 1:1 to `BitPacker4x`'s 128-value block boundary, letting a query skip a
  whole block by computing `bm25_weight.score(...)` from those two bytes alone.
  **STRAND's postings blob does not yet have an equivalent.** RFC 0007 §6 registers
  exactly one block-max bound — the per-block maximum *doc-ordinal*, for doc-ID skip
  pruning — and its own Non-goals section is explicit that "term-frequency and
  document-length block-max bounds for BM25-scoring pruning (WAND/BlockMax-WAND-
  style)... are not registered here... Scoring-aware pruning bounds are real,
  separate, future work." So the fork inherits a real, open design gap here, not a
  translation task: tantivy's default read path uses a BM25 pruning bound STRAND's
  format does not carry. A fork's honest options are to drop tantivy's block-max-WAND
  early-exit when reading STRAND data (still correct — invariant 5's parity
  requirement is about scoring inputs and outputs matching, not about matching the
  pruning strategy used to reach them — just slower than tantivy's own native reads),
  or to wait on a future STRAND RFC that registers a scoring-aware block bound before
  trying to preserve the optimization. Either way, `skip.rs`'s `BlockInfo` handling
  is a real patch site.

  Separately, on the one bound STRAND does register: RFC 0007 §6 places `block_max`
  (max doc-ordinal per block) as a fixed, contiguous region at the front of the
  postings blob, binary-searchable without decoding the packed gap/term-frequency
  streams — a deliberate reading of invariant 4's "sibling... never inside a codec's
  private structures" wording that RFC 0007's own "Alternatives considered" section
  flags as contestable, since it keeps `block_max` inside the *same blob* rather than
  a separately registered one (to avoid growing `TermInfo` by another offset/length
  pair). tantivy's inline `block_wand_*` bytes are interleaved directly with each
  block's own bit-packed bytes — genuinely inside the codec's private structure in a
  way RFC 0007's placement is not. The sharper, source-grounded version of this
  mismatch is therefore not "sibling blob vs. no sibling blob" (both formats keep
  their registered bound out of the codec's packed bytes) but "STRAND has no
  scoring-pruning bound registered yet, while tantivy's default is welded to a data
  field STRAND's postings blob does not carry in this region at all."
- **`src/postings/serializer.rs`**, **`src/postings/postings.rs`**,
  **`src/postings/segment_postings.rs`** — `SegmentPostings` (the `Postings`-trait
  implementation actually returned to query execution), and the serializer path
  (read-only fork does not need the write half, but the record-option / freq-encoding
  enums these files share with the reader are needed for correct decode).

### C. Positions decode — must be patched

- **`src/positions/mod.rs`**, **`src/positions/reader.rs`** — `PositionReader::open`,
  constructed directly in `InvertedIndexReader::read_postings_from_terminfo` from
  `positions_file_slice.read_bytes_slice(term_info.positions_range.clone())`. STRAND's
  positions blob reuses the postings blob's own **256-value `BitPacker8x` blocks**
  directly (`rfcs/0008-positions.md` §5, whose own "Alternatives considered" explicitly
  rejects 128-value blocks — "matching Lucene and tantivy exactly... would require
  adding `BitPacker4x`, a distinct, incompatible packing format" — confirmed in the
  shipped code: `crates/strand-lexical/src/positions.rs` imports `BLOCK_LEN` straight
  from `postings.rs`, reusing the same block-codec building blocks rather than
  duplicating them). tantivy's positions encoding is also 128-value-block-coupled
  (`src/positions/mod.rs`'s own doc comment: "blocks of 128 deltas," and its
  `COMPRESSION_BLOCK_SIZE = BitPacker4x::BLOCK_LEN`, the same constant its postings
  module uses). So the granularity mismatch is not confined to postings: it is the
  *same* 256-vs-128 mismatch in both components, since STRAND deliberately made
  positions share postings' block width rather than match tantivy's — this is a real,
  concrete correction to an earlier draft's assumption that positions might be the
  closer granularity match; they are not, on either side.

  (Docs correction, made while grounding this document rather than left standing:
  `docs/ledger.md`'s R9 entry currently states "128 remains the real, separate default
  for the positions blob family, RFC 0008" — that sentence predates RFC 0008's own
  approved Alternatives-considered section quoted above and is stale; a future session
  correcting `docs/ledger.md` should fix it the same way the adjacent "Postings block
  size" entry already documents having been corrected once for the same reason. Not
  fixed in `docs/ledger.md` itself by this document, since that is R9's entry, not
  R11(d)'s, and this grounding pass's mandate is the tantivy fork module list, not an
  R9 audit — named here so it is not silently relied upon.)

### D. Term dictionary — must be patched, but the underlying machinery is shared lineage

- **`src/termdict/mod.rs`** — the `TermDictionary`/`TermDictionaryBuilder` dispatcher.
  **Source** confirms the compiled-in default (non-`quickwit`-feature) implementation:
  > ```rust
  > #[cfg(not(feature = "quickwit"))]
  > mod fst_termdict;
  > #[cfg(not(feature = "quickwit"))]
  > use fst_termdict as termdict;
  > #[cfg(not(feature = "quickwit"))]
  > const CURRENT_TYPE: DictionaryType = DictionaryType::Fst;
  > ```
  A plain `cargo build` of tantivy (the `quickwit` feature is opt-in, confirmed by
  `Cargo.toml`'s `[features]` block, `quickwit = ["sstable", "futures-util",
  "futures-channel"]`, absent from `default`) compiles the **FST-backed**
  `fst_termdict` module, not `sstable_termdict`. `TermDictionary::open` further
  round-trips a 4-byte `DictionaryType` tag from the end of the file and hard-errors
  if it doesn't match the compiled variant — so a fork must either write that tag or
  patch this check out.
- **`src/termdict/fst_termdict/mod.rs`**, **`src/termdict/fst_termdict/termdict.rs`**
  — the concrete FST-backed reader (`term_ord`, `get`, `range`, `stream`, `search`
  against a `tantivy_fst`-compiled automaton). This is the module that must change —
  but STRAND's own term dictionary is **also** an FST mapping term to ordinal,
  deliberately chosen from the same lineage (`rfcs/0005-term-dictionary.md`, which
  cites `references/tantivy-fst-termdict-and-fst-crate.md` directly and uses the real
  `fst` crate rather than inventing a novel structure). This is the one module in the
  list where the fork's job is closer to "point the existing FST-traversal machinery
  at STRAND's own compiled FST and term-info store" than "replace the algorithm" —
  worth recording as a real, asymmetric cost reduction relative to postings/positions.

### E. Field norms — must be patched, and it is load-bearing for invariant 5

- **`src/fieldnorm/reader.rs`**, **`src/fieldnorm/code.rs`** — `FieldNormReaders::open`
  (a `CompositeFile` over one fieldnorm sub-file per field), `FieldNormReader`,
  `fieldnorm_to_id`/`id_to_fieldnorm` (tantivy's one-byte lossy norm quantization —
  already the subject of `references/tantivy-fieldnorm-overlap-accounting.md`, vendored
  for the Lucene-parity profile work in invariant 5). A STRAND fork needs this module
  specifically to support the parity profile CLAUDE.md §5 defines ("parity within
  Lucene's one-byte norm quantization" — the equivalent tantivy-side comparison uses
  this exact quantization function), so it is in scope even though it is structurally
  simpler than postings (its `CompositeFile` wrapping is the same Layer-2 problem as
  the term dictionary's, not a new one).

### F. Directory — the one module that plausibly needs *no* patch

- **`src/directory/directory.rs`** — the `Directory` trait itself
  (`get_file_handle`/`open_read`/`atomic_read`/`watch`/…). Per Part 0, a fork
  implements this trait fresh (a `StrandDirectory`) rather than patching it — R11(c)'s
  precedent (Quickwit's three `Directory` implementations, zero tantivy-internals
  patches) is direct evidence this layer is genuinely swappable without a fork at all.
  Listed here for completeness, not as a patch site.

### Out of scope for a lexical-only fork (named, not silently dropped)

`src/fastfield/*` (backed by the separate `tantivy-columnar` workspace crate),
`src/store/*` (the row-oriented doc store), and `src/schema/document/*` (stored-field
value types) read back tantivy's fast-field and doc-store components, which have no
STRAND blob-family equivalent — STRAND has no analogue to tantivy's arbitrary
stored-document retrieval or generic fast fields. `docs/roadmap.md`'s M4-4 scope
correction already narrows the fork to "the spec chapters the fork actually reads";
this list makes that narrowing concrete. If a future session decides the fork should
also prove out fast-field or doc-store round-tripping (for example, to benchmark
against tantivy's own `TopDocs` collector, which reads fieldnorms *and* fast fields),
`src/fastfield/mod.rs`, `src/fastfield/readers.rs`, and `src/store/reader.rs` would
need to be added to this list — flagged here as a known extension point for the fork
RFC to accept or explicitly decline, not decided in this grounding pass.

---

## Part 2 — module stability, checked against real git history

`CLAUDE.md` §3's rule ("never implement against a remembered spec") cuts both ways:
a module list vendored today is itself a snapshot, and the task that produced it asked
explicitly whether these modules are stable or churny. They are churny — concretely,
not just "code changes sometimes."

### Release cadence

**Source:** `gh api repos/quickwit-oss/tantivy/releases`, most recent ten:

| Tag | Published |
|---|---|
| 0.26.1 | 2026-05-10 |
| 0.25.0 | 2025-08-20 |
| 0.24.1 | 2025-04-25 |
| 0.22.0 | 2024-04-12 |
| 0.21 | 2023-09-01 |
| 0.20.1 | 2023-06-12 |
| 0.20 | 2023-06-09 |
| 0.19.2 | 2023-02-10 |
| 0.19.1 | 2023-01-13 |
| 0.19 | 2023-01-09 |

Roughly two to three minor releases a year, each historically carrying real internal
changes (confirmed per-module below), not just point fixes.

### Per-module commit history (representative, most recent first)

**Source:** `gh api "repos/quickwit-oss/tantivy/commits?path=<file>"`.

`src/postings/skip.rs` (the block-max/skip-list module, Part 1.B): five substantive
commits inside eight months at fetch time — `1d06328c` "Add
`BlockSegmentPostings::rank()` for skip-list-based positional counting" (2026-06-12),
`63da5a21` "Optimizing top K using Adrien Grand's ideas" (2026-04-26), `57fe659f`
"make serializer pub" (2026-02-11), `6443b631` (2026-01-02), plus the original
`2481c87b` "Block wand" (2020-08-19) that introduced the inline-bound design this
document's Part 1.B relies on.

`src/postings/mod.rs`: `7373d54c` (2026-08-12, clippy), `6892995d` "10% faster
intersections: Use k-ary in block search" (2026-06-28), `57fe659f` (2026-02-11),
`12977bc7` "upgrade some dependancies" (2026-01-14), `735c588f` "fix union performance
regression" (2026-01-02) — real algorithmic work, not only lint fixes, landing roughly
monthly across 2026.

`src/index/segment_reader.rs` (Part 1.A, the fork's central patch site):
`0401b457` "feat: Extensible segment components via plugin trait (#2993)"
(2026-08-10 — see the case study below), `a27c6499` (2026-06-01), `be11f8a6` "Fix
opening positions file error" (2026-05-14), `c6912ce8` (2025-12-10).

`src/termdict/mod.rs` (Part 1.D): comparatively quiet — `945af922` (2025-07-02,
clippy), `6ca84a61` "make termdict always clone" (2025-01-08), `175a529c` "use
executor for cpu-heavy sstable decompression for automaton" (2025-01-03), then a gap
back to 2024-10.

`src/directory/directory.rs` (Part 1.F, the module that plausibly needs no patch):
the quietest of all — every commit in its last eight, back to 2022-11, is a clippy or
typo fix; no behavioral change. This is independent, source-grounded confirmation that
`Directory` is the genuinely stable extension point R11(c) already characterized it
as, in contrast to the reader-internals modules above.

### Case study: PR #2993, "Extensible segment components via plugin trait"

This is the concrete churn event the task asked to look for, and it is real and
recent: merged **2026-08-10**, nine days before this document's fetch date, authored
by ParadeDB (a company that already forks/embeds tantivy in production for its own
Postgres-native search extension — independent, if informal, evidence that
forking tantivy for a custom storage substitution is a validated path, not a
theoretical one). **Source:** `gh api repos/quickwit-oss/tantivy/commits/0401b457`,
commit message (verbatim excerpt):

> "Today every segment component — postings, fast fields, field norms, store — is
> hardcoded into several places. Adding a new per-segment data structure means
> forking Tantivy and editing each of those sites. This PR introduces a
> `SegmentPlugin` trait that lets a custom component participate in the full segment
> lifecycle — write, serialize, merge, garbage collection, space usage — through the
> same interface the built-ins use, without touching Tantivy internals. The four
> built-in components are themselves reimplemented as plugins. ... 4. The read side
> needs no plugin hook. Custom component data is read back through the existing
> public surface `SegmentReader::open_read`."

The commit touches **44 files** (`gh api repos/quickwit-oss/tantivy/commits/0401b457
--jq '.files | length'`), including every module named in Part 1.A
(`segment_component.rs`, `segment_reader.rs`, `segment.rs`, `index.rs`,
`index_meta.rs`) plus `postings/serializer.rs`, `fastfield/mod.rs`, and `store/mod.rs`.
Two facts matter for the fork, in tension with each other:

First, this does **not** overturn R11(a)'s core finding. Per the PR's own point 4, the
read side is unchanged — a plugin adds a *new* component read back through the same
generic `open_read: PathBuf -> FileSlice` surface a `Directory`-based fork already
plans to use; it does not add a way to swap the decoder for an *existing* component
(postings, terms, fast fields). A fork wanting to make tantivy read STRAND's own
postings encoding still needs to patch `InvertedIndexReader`/`BlockSegmentPostings`
exactly as Part 1.B describes; the plugin trait would let a fork *add* a STRAND-only
component alongside the built-ins with less internals surgery, which is a real but
narrower win than a codec SPI.

Second, it **does** date one specific sentence in R11(a)'s finding: `SegmentComponent`
is described there as "a closed seven-variant enum... a closed enum, not a registry."
**Source:** `gh api repos/quickwit-oss/tantivy/contents/src/index/segment_component.rs
?ref=main` (fetched 2026-08-19, explicitly *not* the pinned `0.26.1` tag — this is
`main`, unreleased, absent from `CHANGELOG.md`'s current "Tantivy 0.27.0 (Unreleased)"
section at fetch time):

> ```rust
> pub enum SegmentComponent {
>     Postings, Positions, FastFields, FieldNorms, Terms, Store, TempStore, Delete,
>     /// A custom component defined by a [`SegmentPlugin`](crate::SegmentPlugin).
>     /// The string is the file extension for this component.
>     Custom(String),
> }
> ```

The enum is no longer closed on `main` — it carries an open `Custom(String)` variant.
This does not change any conclusion in Part 1 (the fork still patches the same files
for the same reason: substituting the *existing* Postings/Terms/FieldNorms components'
byte layout, which `Custom` cannot do), but it is exactly the kind of fact this
document's own §3 obligation exists to surface: **R11(a)'s tag-pinned characterization
is already dated on `main`, three months after the tag it cites, by an unreleased PR.**
This is recorded here rather than silently folded into R11(a) itself — updating R11(a)
is a separate, smaller edit a future session should make when tantivy next cuts a
release that includes this change; this document's job is to arm the failure triggers
with the fact that the surface moves, not to re-litigate R11(a)'s already-approved
finding.

---

## Part 3 — the fork failure triggers, grounded

`docs/benchmarks.md`'s tantivy-fork paragraph already states three failure triggers
verbatim (not re-derived here, only armed and grounded):

1. **Conformance/parity gate**: "cannot pass the frozen v0.1 conformance manifest and
   the invariant-5 parity gate within 10 sessions of fork start."
2. **Maintenance-cost gate**: "fork maintenance (excluding first bring-up) exceeds
   15% of a milestone's commits in two consecutive milestones, measured from the §3
   commit log."
3. **Scope-leak gate**: "correct reads require modifying files outside the pinned
   reader-module list, which falsifies the engine-constant premise itself."

Trigger 3 was previously unarmed — there was no pinned list to check modified files
against. **Part 1 of this document is that list.** Concretely: Part 1.A–E (thirteen
files: `segment_reader.rs`, `inverted_index_reader.rs`, `segment_component.rs`,
`segment.rs`, `composite_file.rs`, `compression/mod.rs`, `block_segment_postings.rs`,
`skip.rs`, `serializer.rs`, `postings.rs`, `segment_postings.rs`, `positions/mod.rs`,
`positions/reader.rs`, `termdict/mod.rs`, `termdict/fst_termdict/mod.rs`,
`termdict/fst_termdict/termdict.rs`, `fieldnorm/reader.rs`, `fieldnorm/code.rs`) plus
a new `StrandDirectory` implementation (Part 1.F, additive, not a patch to
`directory.rs` itself). A future fork session that finds itself editing
`src/fastfield/*`, `src/store/*`, `src/schema/*`, `src/query/*`, or `src/indexer/*`
(the write path — the fork is read-only by design, per `docs/benchmarks.md`) to get a
correct read has hit trigger 3, exactly as stated.

Grounding for triggers 1 and 2, from Part 2's evidence rather than from restating the
numbers: `src/postings/skip.rs` — the file carrying both the block-granularity
mismatch (via its `COMPRESSION_BLOCK_SIZE` import) and the missing-scoring-bound gap
(Part 1.B) — and `src/postings/mod.rs`, which re-exports `skip.rs`'s
`BlockInfo`/`SkipReader` and carries its own real algorithmic churn, are two of the
thirteen pinned files that have taken real, non-lint commits roughly monthly through
2026. If the fork pins a commit and tantivy's upstream `main` moves at
that rate in exactly the files the fork has patched, a rebase onto a newer pin (not
required by `docs/benchmarks.md`'s "pins one tantivy commit... does not chase upstream
HEAD," but plausible if a later milestone wants a security fix or a real bug fix from
upstream) risks a merge-conflict-heavy patch reapplication in precisely the
highest-value files — direct fuel for trigger 2's 15%-of-commits ceiling. PR #2993 is
the sharpest available data point: a single upstream PR that touched every file in
Part 1.A within one week is a real, not hypothetical, instance of "a large fraction of
a milestone's fork-maintenance commits consumed by chasing one upstream refactor,"
which is exactly what trigger 2 exists to catch.

One additional, narrower observation worth recording for the eventual fork RFC's own
"how this could be wrong" section (not a fourth trigger — `docs/benchmarks.md`'s three
are the settled list, and this document does not have standing to add a fourth without
that RFC's own adversarial review): the missing-scoring-bound gap in Part 1.B (tantivy's
default read path uses a BM25 block-pruning bound STRAND's postings blob does not
register yet) is the concrete instance of trigger 1's parity gate most likely to eat
the ten-session budget, because it is not a decode-logic swap but a cross-project
dependency — the fork's own progress is coupled to whether a future STRAND RFC
registers a scoring-aware block bound before the fork tries to preserve tantivy's
optimization, or the fork simply accepts a slower (but still invariant-5-correct)
read path and moves on.

---

## What this resolves for R11(d)

`docs/ledger.md` R11(d) — "the fork reader-module list that arms the fork failure
triggers" — is answered: the list is Part 1 (thirteen files plus one new
`Directory` impl, scoped to a lexical-only fork per M4-4's own scope correction), and
trigger 3 is now checkable against it. R11(d) does not, and was never asked to, start
the fork itself (M4-4, separate and larger); it also surfaces one fact a later,
separate session should fold back into R11(a) proper — the `SegmentComponent` closed-
enum characterization is dated on tantivy's unreleased `main` by PR #2993 — recorded
here rather than acted on, since amending an already-approved finding is its own
edit, not a side effect of grounding a different roadmap item.
