# R11(b) — FAISS `InvertedLists` external-storage feasibility, plain vs. FastScan

Vendored excerpts. Fetched 2026-08-19, against live source (`gh api`), not memory,
per `CLAUDE.md` §3.

**Source repo:** `github.com/facebookresearch/faiss`. **Pinned:** tag `v1.15.0`
(lightweight tag resolving directly to commit `20f14b31a6d54e243a3d1de6ae193fc4c3ec18ed`,
published 2026-08-03 per `gh api repos/facebookresearch/faiss/releases/latest`) —
current at fetch time (2026-08-19). Tip of `main` at fetch time was
`7059eaf7da7eddda62e71367e684d4bdedd7f94f` (2026-08-19); the tagged release is cited
throughout, matching this project's practice (R11(a)) of pinning a release rather than
a moving branch. License: MIT, already vendored and byte-verified
(`references/faiss-LICENSE.txt`, `docs/ledger.md`).

Cited by: `docs/ledger.md` R11, `docs/roadmap.md` M4-1(b) and M5-3,
`docs/benchmarks.md`'s FAISS inverted-lists adapter paragraph.

Answers, up front: **(1) yes** — a custom `InvertedLists` subclass backed by STRAND's
own cluster-family blob fully serves plain `IndexIVFRaBitQ` search. The generic
`IndexIVF` search path (`search_preassigned`, inherited unmodified by
`IndexIVFRaBitQ`) reads every list exclusively through the abstract `InvertedLists`
interface — `list_size`, `get_codes`, `get_ids` — with no `dynamic_cast` to any
concrete type anywhere in the call path. **(2) yes, but only for search, and only
after a repack** — FastScan's *query-time* code (`IndexIVFFastScan::search_*`,
inherited by `IndexIVFRaBitQFastScan`) is, surprisingly, equally generic: it also
reads codes exclusively through the base `InvertedLists` interface, never
`dynamic_cast`-ing to `BlockInvertedLists`. What *does* require a literal
`BlockInvertedLists` is the **write** path — `IndexIVFFastScan::add_with_ids` (throw
text: "only block inverted lists supported") and `IndexIVFRaBitQFastScan`'s own
`postprocess_packed_codes` each `dynamic_cast<BlockInvertedLists*>(invlists)` and
throw if it fails, then reach through to that class's public `codes`/`ids` members
directly, bypassing the virtual interface entirely. So a STRAND-backed `InvertedLists` can never be built *through* FAISS's own
`add_with_ids`; it must instead be populated by decoding STRAND's own RaBitQ blob
bytes directly into FAISS's block-packed byte layout, then handed to FastScan search
through `IndexIVF::replace_invlists(InvertedLists*, bool own)` — a public method
taking the abstract base pointer, not the concrete type. The repack is real and its
cost is quantifiable (below); it is not, however, a *per-query* cost if done once at
segment open, matching `docs/benchmarks.md`'s existing "any repack is adapter-side,
charged to load, and stated" language, now confirmed against source rather than
asserted.

---

## Part 1 — the abstract `InvertedLists` interface

**Source:** `faiss/invlists/InvertedLists.h`, full file read at the pinned tag.

The interface a custom implementation must satisfy splits into pure-virtual
(mandatory) and virtual-with-default (optional) methods:

> ```cpp
> struct InvertedLists {
>     size_t nlist;     ///< number of possible key values
>     size_t code_size; ///< code size per vector in bytes
>     bool use_iterator = false;
>
>     virtual size_t list_size(size_t list_no) const = 0;
>     virtual const uint8_t* get_codes(size_t list_no) const = 0;
>     virtual const idx_t* get_ids(size_t list_no) const = 0;
>     virtual size_t add_entries(size_t list_no, size_t n_entry,
>             const idx_t* ids, const uint8_t* code) = 0;
>     virtual void update_entries(size_t list_no, size_t offset,
>             size_t n_entry, const idx_t* ids, const uint8_t* code) = 0;
>     virtual void resize(size_t list_no, size_t new_size) = 0;
>
>     virtual void release_codes(size_t list_no, const uint8_t* codes) const;
>     virtual void release_ids(size_t list_no, const idx_t* ids) const;
>     virtual idx_t get_single_id(size_t list_no, size_t offset) const;
>     virtual const uint8_t* get_single_code(size_t list_no, size_t offset) const;
>     virtual void prefetch_lists(const idx_t* list_nos, int nlist_in) const;
>     virtual bool is_empty(size_t list_no, void* inverted_list_context = nullptr) const;
>     virtual InvertedListsIterator* get_iterator(size_t list_no,
>             void* inverted_list_context = nullptr) const;
> };
> ```

Six methods are pure virtual and therefore mandatory even for a read-only backend:
`list_size`, `get_codes`, `get_ids`, `add_entries`, `update_entries`, `resize`. FAISS
ships its own answer to "what does a read-only backend do about the three write
methods it must nonetheless implement": `ReadOnlyInvertedLists` — quoted from
`faiss/invlists/InvertedLists.cpp` (full file read at the pinned tag):

> ```cpp
> size_t ReadOnlyInvertedLists::add_entries(size_t, size_t, const idx_t*, const uint8_t*) {
>     FAISS_THROW_MSG("not implemented");
> }
> void ReadOnlyInvertedLists::update_entries(size_t, size_t, size_t, const idx_t*, const uint8_t*) {
>     FAISS_THROW_MSG("not implemented");
> }
> void ReadOnlyInvertedLists::resize(size_t, size_t) {
>     FAISS_THROW_MSG("not implemented");
> }
> ```

A STRAND-backed `InvertedLists` — segments are immutable per invariant 2, so a reader
adapter is read-only by construction — derives from `ReadOnlyInvertedLists` for
exactly this reason and inherits these three throwing stubs unmodified, satisfying the
compiler's pure-virtual requirement without writing dead code.

The three remaining mandatory methods are the real implementation surface:

- **`list_size(list_no)`** — the row count of one IVF cluster's posting list.
  STRAND's cluster-family blob already carries this per invariant 7's navigation
  tier; no new bytes needed, just a lookup.
- **`get_codes(list_no)`** — `must be released by release_codes`, `@return codes
  size list_size * code_size` (doc comment, `InvertedLists.h` line ~79–84). This is
  the one method that costs real engineering: it must return a stable pointer to
  `list_size * code_size` contiguous bytes matching FAISS's own RaBitQ code layout
  exactly (Part 1's byte contract, established by `IndexIVFRaBitQ::encode_vectors`,
  below). STRAND's own RaBitQ blob (RFC 0010, invariant 11's provenance-pinning
  rules for the random rotation) already stores quantized codes and per-vector
  factors; the adapter's job is to slice out exactly the bytes for one list_no and
  hand back a pointer — no bit-level repacking is needed for the plain path, only a
  byte-offset computation, because `IndexIVFRaBitQ`'s codes are flat, fixed-size
  per-vector arrays, not FastScan's interleaved blocks (confirmed in Part 3).
- **`get_ids(list_no)`** — the row-ID array for that list, `size list_size`. This is
  the exact place STRAND's stable 64-bit row-IDs (invariant 1) surface to FAISS:
  `idx_t` is FAISS's own 64-bit signed integer type, and STRAND's row-IDs slot in
  directly, with the concatenate-and-remap merge semantics invariant 1 already
  requires of IVF-family blobs matching what FAISS's own IDs expect (monotonically
  meaningful only within a list; global uniqueness is the row-ID contract's job, not
  FAISS's).

`prefetch_lists` — `virtual void prefetch_lists(const idx_t* list_nos, int nlist_in)
const;`, default no-op — is the one optional method worth overriding regardless: it
is FAISS's own named hook for "the search planner knows in advance which lists
`nprobe` will touch; go fetch them now." A STRAND adapter's override is exactly
invariant 3's one-wave rule turned into a concrete call site: issue the parallel
byte-range GETs for all `nlist_in` lists here, populate an adapter-owned decode cache,
and let `get_codes`/`get_ids` (called later, once per list, by the search loop) be
cheap synchronous lookups into that cache rather than a GET each.

## Part 2 — `OnDiskInvertedLists`: the closest real precedent, and its real limits

**Source:** `faiss/invlists/OnDiskInvertedLists.h` and `.h`'s companion `.cpp`, full
files read at the pinned tag.

FAISS's own external-storage `InvertedLists` is real prior art but a narrower thing
than "arbitrary remote storage": it is a single memory-mapped file **in FAISS's own
private on-disk layout**, not a generic byte-range-fetch abstraction. The header's own
description:

> ```
> /** On-disk storage of inverted lists.
>  *
>  * The data is stored in a mmapped chunk of memory (base pointer ptr,
>  * size totsize). Each list is a range of memory that contains (object
>  * List) that contains:
>  *
>  * - uint8_t codes[capacity * code_size]
>  * - followed by idx_t ids[capacity]
>  * ...
>  */
> struct OnDiskInvertedLists : InvertedLists {
>     using List = OnDiskOneList;
>     std::vector<List> lists;   // size nlist
>     std::string filename;
>     size_t totsize;
>     uint8_t* ptr;   // mmap base pointer
>     bool read_only;
>     ...
> };
> ```

The read methods are pointer arithmetic into that one mmap, nothing more —
`OnDiskInvertedLists.cpp`, lines 385–400:

> ```cpp
> const uint8_t* OnDiskInvertedLists::get_codes(size_t list_no) const {
>     if (lists[list_no].offset == INVALID_OFFSET) { return nullptr; }
>     return ptr + lists[list_no].offset;
> }
> const idx_t* OnDiskInvertedLists::get_ids(size_t list_no) const {
>     if (lists[list_no].offset == INVALID_OFFSET) { return nullptr; }
>     return (const idx_t*)(ptr + lists[list_no].offset +
>                           code_size * lists[list_no].capacity);
> }
> ```

and the mmap itself is a plain POSIX call against one local file (`do_mmap`, lines
268–290): `mmap(nullptr, totsize, prot, MAP_SHARED, fileno(f), 0)` opened from
`filename` via `fopen`. There is no S3, no byte-range GET, no pluggable I/O backend
anywhere in this class — "on-disk" means "on a local (or locally mounted) disk,
mmapped whole." What it demonstrates, and what genuinely transfers to a STRAND
adapter, is the *shape of the contract*: `get_codes`/`get_ids` return raw pointers
into memory that must already be resident and stably addressed — no async, no
`Result`, no copy-out. This confirms the design constraint a STRAND adapter's
`InvertedLists::get_codes` must satisfy: by the time it returns, the bytes for that
list must already be fetched, decoded, and held in an adapter-owned buffer whose
lifetime outlives the caller's use (released, if at all, only when `release_codes` is
later called — the default implementation is a no-op, so "never released, cached for
the segment's open lifetime" is a legitimate and probably the right choice for a
cold-open STRAND adapter). What does **not** transfer is the mmap mechanism itself:
`OnDiskInvertedLists` is not a template for "swap in an S3 client," it is a
demonstration that FAISS's read path tolerates *any* pointer-returning backend, mmap
or otherwise, exactly the property a STRAND adapter needs and the property confirmed
independently in Part 1's interface reading.

## Part 3 — plain `IndexIVFRaBitQ`: flat codes, the generic `IndexIVF` search path

**Source:** `faiss/IndexIVFRaBitQ.h`, full file; `faiss/IndexIVFRaBitQ.cpp`, full
file; `faiss/IndexIVF.cpp`, the `search_preassigned` region (lines ~430–610 and
830–900), read at the pinned tag.

`IndexIVFRaBitQ` declares no `InvertedLists`-related override at all — no custom list
type, no `get_codes` interposition. Its constructor sets a flat, ordinary
`code_size` exactly the way any `IndexIVF` subtype does (`IndexIVFRaBitQ.cpp`, lines
27–43):

> ```cpp
> IndexIVFRaBitQ::IndexIVFRaBitQ(Index* quantizer_in, const size_t d_in,
>         const size_t nlist_in, MetricType metric, bool own_invlists_in,
>         uint8_t nb_bits_in)
>         : IndexIVF(quantizer_in, d_in, nlist_in, 0, metric, own_invlists_in),
>           rabitq(d_in, metric, nb_bits_in) {
>     code_size = rabitq.code_size;
>     if (own_invlists_in) { invlists->code_size = code_size; }
>     is_trained = false;
>     by_residual = true;
> }
> ```

and `encode_vectors` (lines 56–87) confirms the per-vector code layout a STRAND
adapter's `get_codes` slice must reproduce byte-for-byte: `rabitq.compute_codes_core`
writes the RaBitQ sign-bit pattern plus factors into a flat `code_size`-byte buffer
per vector, with no interleaving across vectors — `codes + i * (code_size +
coarse_size)` is straight-line indexing, not a block layout.

Search runs entirely on `IndexIVF`'s own inherited `search_preassigned` (no override
in `IndexIVFRaBitQ.h`), which is the generic list machinery `docs/benchmarks.md`
already named. The read call site — `faiss/IndexIVF.cpp`, confirmed by grep across
the whole file to have zero `dynamic_cast` to any concrete `InvertedLists` subtype —
looks exactly like this (lines ~890–901, representative of every call site in the
file, including the analogous block at 559–581 and the reconstruction path at
1039–1040):

> ```cpp
> std::unique_ptr<InvertedListScanner> scanner(
>         get_InvertedListScanner(store_pairs, sel, params));
> ...
> if (invlists->use_iterator) {
>     iterator.reset(invlists->get_iterator(key, inverted_list_context));
> } else {
>     InvertedLists::ScopedCodes scodes(invlists, key);
>     InvertedLists::ScopedIds ids(invlists, key);
>     ...
> }
> ```

`invlists` here is the plain `InvertedLists*` base-class member every `IndexIVF`
carries; `ScopedCodes`/`ScopedIds` are the RAII wrappers over the plain virtual
`get_codes`/`get_ids`/`release_codes`/`release_ids` calls from Part 1 — the same
methods any subclass provides. Nothing in this file, or in `IndexIVFRaBitQ.cpp`,
narrows that to a specific concrete type. This settles question (1) affirmatively and
concretely: a STRAND-cluster-blob-backed `InvertedLists` subclass, implementing the
three mandatory read methods plus the three throwing write stubs from
`ReadOnlyInvertedLists`, is sufficient — architecturally, not just plausibly — to
serve `IndexIVFRaBitQ` search over externally-hosted storage, with **no FAISS fork
required**.

## Part 4 — FastScan: search is generic, the write path is not

**Source:** `faiss/invlists/BlockInvertedLists.h` and its `.cpp`; `faiss/IndexIVFFastScan.h`
and its `.cpp`; `faiss/IndexIVFRaBitQFastScan.h` and its `.cpp`; `faiss/IndexIVF.h`
(`replace_invlists`); all read in full at the pinned tag.

### 4a. `BlockInvertedLists`: a concrete packed layout, not an abstract requirement

`BlockInvertedLists` (`faiss/invlists/BlockInvertedLists.h`) is a normal
`InvertedLists` subclass — not a separate interface — that stores codes in
`n_per_block`-vector blocks of `block_size` bytes, interpreted through a
`CodePacker*`:

> ```cpp
> struct BlockInvertedLists : InvertedLists {
>     size_t n_per_block = 0; // nb of vectors stored per block
>     size_t block_size = 0;  // nb bytes per block
>     const CodePacker* packer = nullptr; // required to interpret block content
>     std::vector<AlignedTable<uint8_t>> codes;
>     std::vector<std::vector<idx_t>> ids;
>     ...
> };
> ```

and it deliberately poisons the base class's `code_size` field so nothing
misinterprets it (`InvertedLists.h`, line ~69–71, quoted in Part 1's header comment):
`static const size_t INVALID_CODE_SIZE = static_cast<size_t>(-1);` —
`BlockInvertedLists`'s constructor passes exactly this sentinel to the base class
(`BlockInvertedLists.cpp`, line 26: `InvertedLists(nlist_in,
InvertedLists::INVALID_CODE_SIZE)`), because per-vector code size is genuinely
undefined for a layout where vectors interleave inside a block. A grep of both
`IndexIVFFastScan.cpp` and `IndexIVFRaBitQFastScan.cpp` for `invlists->code_size`
returns zero hits — confirming nothing in the FastScan search path reads that field at
all; block interpretation is driven entirely by the *index's own* `bbs`/`M`/`M2`
members and a freshly constructed `CodePacker` (`get_CodePacker()`), never by
anything read off the `InvertedLists` object itself.

### 4b. Search: exclusively through the base `InvertedLists` interface

A grep across all of `IndexIVFFastScan.cpp` for `dynamic_cast<BlockInvertedLists`
turns up exactly two hits, both in code-management (write) functions, never in a
search function:

> ```
> 90:    auto bil = dynamic_cast<BlockInvertedLists*>(invlists);      // init_code_packer
> 161:    BlockInvertedLists* bil = dynamic_cast<BlockInvertedLists*>(invlists); // add_with_ids
> ```

Every *search*-side read of the inverted lists in the same file — `search_implem_1`,
`search_implem_2`, `search_implem_10`, `search_implem_12`, `search_implem_14`,
`reconstruct_from_offset`, `reconstruct_orig_invlists` — instead uses the identical
generic pattern already shown for the plain path:

> ```cpp
> // IndexIVFFastScan.cpp, representative of every search-time read site
> size_t list_size = invlists->list_size(list_no);
> InvertedLists::ScopedCodes codes(invlists, list_no);
> InvertedLists::ScopedIds ids(invlists, list_no);
> ```

`IndexIVFRaBitQFastScan.cpp`'s own `make_knn_scanner` (lines 836–848) confirms this
holds for the RaBitQ FastScan specialization too: it constructs a scanner object
(`rabitq_ivf_make_knn_scanner`) parameterized on `this` (the index) and query
parameters, with no `InvertedLists`-type dependency of its own — the scanner consumes
whatever bytes the inherited `IndexIVFFastScan` search loop hands it via the same
`ScopedCodes` calls. This means the SIMD kernels that interpret packed block bytes are
wired to the **byte layout** (block-interleaved 4-bit codes plus, for RaBitQ, embedded
per-vector factor bytes after the bit pattern — `code_packing_stride()` returning
`code_size` is exactly this override, `IndexIVFRaBitQFastScan.cpp` line 207–210), not
to the C++ type that produced them. A custom `InvertedLists` subclass whose
`get_codes(list_no)` returns bytes already in that exact packed layout will be read
correctly by FastScan search, with **no dynamic_cast to fail and no FAISS-side code
change** — architecturally identical to Part 3's finding for the plain path, with one
added requirement: the bytes themselves must already be block-packed, which STRAND's
own wire format is never going to hand over directly (see 4d).

### 4c. Build: genuinely requires a literal `BlockInvertedLists`

`add_with_ids` (`IndexIVFFastScan.cpp`, lines 119–225) is where the requirement
tightens. After computing flat codes for a batch, it does not go through the abstract
`add_entries` at all:

> ```cpp
> BlockInvertedLists* bil = dynamic_cast<BlockInvertedLists*>(invlists);
> FAISS_THROW_IF_NOT_MSG(bil, "only block inverted lists supported");
> ...
> size_t list_size = bil->list_size(list_no);
> bil->resize(list_no, list_size + i1 - i0);
> ...
> bil->ids[list_no][ofs] = id;              // direct public-member write
> ...
> pq4_pack_codes_range(list_codes.data(), M, list_size, list_size + i1 - i0,
>         bbs, M2, bil->codes[list_no].data(), pack_stride, get_block_stride());
> postprocess_packed_codes(list_no, list_size, i1 - i0, list_codes.data());
> ```

and `IndexIVFRaBitQFastScan::postprocess_packed_codes` (lines 220–245) does the same
— `dynamic_cast<BlockInvertedLists*>(invlists)`, then `bil->codes[list_no].data()`
written to directly to splice in RaBitQ's per-vector factor bytes at their in-block
offset. Both are reaching past the virtual interface into `BlockInvertedLists`'s own
public `codes`/`ids` vectors because the packing arithmetic (`pq4_pack_codes_range`,
the aux-data offset math in `rabitq_utils::get_block_aux_ptr`) is written directly
against that class's storage layout, not against `add_entries`'s generic contract.
There is no path by which a custom `InvertedLists` subclass can be *built into* by
calling `IndexIVFFastScan::add_with_ids` — the throw is unconditional and literal
("only block inverted lists supported" is the exact, current exception string).

### 4d. The repack: quantified, and where it actually has to happen

Because build requires `BlockInvertedLists` but STRAND's wire bytes (invariant 10)
will never be pre-packed into FAISS's own block-interleaved layout — doing so would
bake one engine's SIMD register-shuffle layout into the spec, exactly the thing
invariant 10 forbids — every STRAND→FastScan path needs a repack somewhere between
"STRAND's flat RaBitQ blob bytes" and "bytes `IndexIVFFastScan`'s search loop can
read." FAISS's own conversion constructor, `IndexIVFRaBitQFastScan(const
IndexIVFRaBitQ& orig, int bbs_in)` (`IndexIVFRaBitQFastScan.cpp`, lines 101–197), is
direct, load-bearing evidence for exactly this repack, and it is worth quoting in
full because it is simultaneously proof that the source side can be arbitrary (it
reads `orig.invlists` through the same generic `ScopedCodes`/`ScopedIds` calls as
everywhere else — so a STRAND-backed `InvertedLists` works as the *source*) and proof
that the destination must be a fresh, owned `BlockInvertedLists`:

> ```cpp
> replace_invlists(new BlockInvertedLists(nlist, get_CodePacker()), true);
> #pragma omp parallel for if (nlist > 100)
> for (idx_t list_no = 0; list_no < static_cast<idx_t>(nlist); list_no++) {
>     const size_t nb = orig.invlists->list_size(list_no);
>     if (nb == 0) continue;
>     AlignedTable<uint8_t> flat_codes(nb * code_size);
>     InvertedLists::ScopedCodes orig_codes(orig.invlists, list_no);
>     for (size_t i = 0; i < nb; i++) {
>         const uint8_t* orig_code = orig_codes.get() + i * orig.code_size;
>         uint8_t* fs_code = flat_codes.get() + i * code_size;
>         for (size_t j = 0; j < static_cast<size_t>(d); j++) {
>             // re-derive each of d sign bits from orig's packed bit pattern
>             ... if (bit_value) rabitq_utils::set_bit_fastscan(fs_code, j);
>         }
>         memcpy(fs_code + bit_pattern_size, orig_code + bit_pattern_size, storage_size);
>     }
>     std::unique_ptr<CodePacker> packer(get_CodePacker());
>     AlignedTable<uint8_t> block_codes(roundup(nb, bbs) / bbs * packer->block_size);
>     for (size_t i = 0; i < nb; i++) {
>         packer->pack_1(flat_codes.get() + i * code_size, i, block_codes.get());
>     }
>     invlists->add_entries(list_no, nb,
>             InvertedLists::ScopedIds(orig.invlists, list_no).get(), block_codes.get());
> }
> orig_invlists = orig.invlists;
> ```

This is a full-segment, one-time repack: for every one of `ntotal` vectors, re-derive
`d` sign bits from the source encoding and one `CodePacker::pack_1` call
(`faiss/impl/CodePacker.h`: `pack_1(flat_code, offset, block)`, block-relative,
`0 <= offset < nvec`). Cost is `O(ntotal · d)` bit work plus `O(ntotal)` pack calls —
the same order as encoding the vectors once, run once per segment open, not once per
query. `docs/benchmarks.md`'s existing "any repack is adapter-side, charged to load,
and stated" is precisely this operation, now grounded against the literal FAISS code
that performs it, and the recommended adapter shape follows directly: at segment open,
build (or reuse a cached) in-memory `IndexIVFRaBitQ` over a STRAND-backed
`InvertedLists` (Part 3, no repack needed there), then run this exact conversion
constructor once to produce a `BlockInvertedLists`-backed `IndexIVFRaBitQFastScan` for
the FastScan-accelerated queries against that segment. The alternative — a custom
`InvertedLists` whose `get_codes` repacks a list's bytes into block layout on first
touch and caches it (Part 4b's finding that this is legal at search time) — pays the
identical per-vector cost but spread across the first query that touches each list
rather than paid up front for every list; it trades a slower cold-open for a slower
first-query-per-list, is architecturally sound per 4b, but duplicates work invariant 3
already wants front-loaded into the open's parallel wave, so the whole-segment
up-front repack is the better fit for STRAND's own cold-open model and the one this
finding recommends.

## What this settles for R11(b)

**(1) Plain `IndexIVFRaBitQ` over external storage: yes, unconditionally, no fork.**
A `ReadOnlyInvertedLists`-derived, STRAND-cluster-blob-backed subclass implementing
`list_size`/`get_codes`/`get_ids` (byte-identical to `IndexIVFRaBitQ::encode_vectors`'s
flat per-vector layout) and overriding `prefetch_lists` for invariant 3's parallel
wave is sufficient. Confirmed by reading every call site in `IndexIVF.cpp`'s
`search_preassigned` and finding zero concrete-type dependencies.

**(2) FastScan over external storage: yes for search, never for build, and the
repack cost is real and quantified.** `IndexIVFFastScan`/`IndexIVFRaBitQFastScan`
search reads codes exclusively through the same abstract `InvertedLists` interface —
confirmed by an exhaustive grep showing every `dynamic_cast<BlockInvertedLists>` site
across both files — two in `IndexIVFFastScan.cpp` (`init_code_packer`,
`add_with_ids`), one in `IndexIVFRaBitQFastScan.cpp`
(`postprocess_packed_codes`) — sits in a code-*writing* function, never in any
`search_implem_*` path. So a STRAND-backed
`InvertedLists` whose `get_codes` returns already-block-packed bytes is legal at
search time. What forces a repack is that STRAND's wire bytes are never themselves
block-packed (invariant 10), and FAISS's own build path (`add_with_ids`) unconditionally
requires a literal `BlockInvertedLists` object (`FAISS_THROW_IF_NOT_MSG(bil, "only
block inverted lists supported")`) — there is no way to populate FastScan's packed
storage except by producing a real `BlockInvertedLists`, whether via the literal
conversion constructor quoted in 4d or by a hand-rolled equivalent doing the same
`CodePacker::pack_1` work. The cost is `O(ntotal · d)` bit-level work plus one
`CodePacker::pack_1` call per vector, paid once per segment open if front-loaded
(recommended, matches invariant 3's wave model) or once per list on first touch if
deferred to `get_codes` (legal, not recommended, spreads the identical cost into query
latency instead of open latency).

**No FAISS fork is required for either kernel.** This resolves the architectural
question the ledger's R11(b) entry poses: the generic `InvertedLists` extension point
genuinely reaches all the way into FastScan's search machinery, not just the plain
IVF path — a narrower and more favorable finding than `docs/benchmarks.md`'s prior
"whether external storage can feed that path... is R11(b)'s question" hedge assumed
might be needed. The real cost is confined to the write/conversion side, is a one-time
per-segment-open expense, and is now quantified rather than open. `docs/roadmap.md`
M5-3 (the FAISS adapter) is unblocked by this finding: the design is "custom
`ReadOnlyInvertedLists`-derived class for both kernels; plain path needs no repack;
FastScan path repacks once at open via the conversion-constructor pattern," and that
design can be written up as an RFC and implemented without touching FAISS's own
source.
