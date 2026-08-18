# tantivy's FST term dictionary, `TermInfo`, and the `fst` crate

Vendored excerpts, byte-exact via `curl`, cross-checked between the `main` branch
and the actual latest release tag (`0.26.1`, published 2026-05-10) — every cited
file confirmed byte-identical between the two, so these citations target a real
release, not an unreleased branch. Fetched 2026-08-18. Groundwork for the M1 term-
dictionary RFC (`docs/data-structures.md`'s "settled default: an FST mapping term to
ordinal, as in Lucene and tantivy") — not yet cited by an approved RFC.

## tantivy's default term dictionary is still FST-based — but not universally

**Source:** `src/termdict/mod.rs`, tantivy `0.26.1`.

```rust
#[cfg(not(feature = "quickwit"))]
mod fst_termdict;
#[cfg(not(feature = "quickwit"))]
use fst_termdict as termdict;

#[cfg(feature = "quickwit")]
mod sstable_termdict;
#[cfg(feature = "quickwit")]
use sstable_termdict as termdict;
```

Plain tantivy (no `quickwit` feature) defaults to `fst_termdict` — confirming
`docs/data-structures.md`'s existing "as in tantivy" claim is still accurate for the
default build, closing the "believed to ship exception-free bitpacking... unverified
against current tantivy source" caveat `docs/ledger.md` R2 already names for a
different claim, but resolving the parallel term-dictionary question this vendoring
was for. **A real caveat this vendoring surfaces that wasn't previously stated
anywhere in this project's docs**: tantivy built with the `quickwit` feature flag
switches to a *different* term dictionary, `sstable_termdict` (its own `sstable/`
crate, block-based, not FST-based at all) — meaning "tantivy's term dictionary" is
not a single, unconditional claim; it depends on which build. STRAND's own lineage
credits both tantivy and Quickwit as prior art (`docs/lineage.md`); a future
tantivy-fork benchmark (M4) needs to state which term-dictionary variant it's
comparing against.

## tantivy's `fst_termdict`: a two-part design

**Source:** `src/termdict/fst_termdict/termdict.rs`, tantivy `0.26.1`.

Part 1 — an `tantivy_fst::MapBuilder` (a fork of the `fst` crate, below) mapping
every term's raw bytes directly to a dense `u64` ordinal, assigned in strictly
increasing insertion order (keys MUST be inserted pre-sorted):

```rust
pub fn insert_key(&mut self, key: &[u8]) -> io::Result<()> {
    self.fst_builder.insert(key, self.term_ord).map_err(convert_fst_error)?;
    self.term_ord += 1;
    Ok(())
}
```

Part 2 — a separate `TermInfoStore`, indexed by that same dense ordinal, holding the
per-term metadata needed to locate postings (below). The FST itself never stores
metadata directly; it only ever maps `term bytes -> term ordinal`, and the ordinal is
purely a dense integer index into the second structure.

## `TermInfo` — the metadata a term ordinal resolves to

**Source:** `src/postings/term_info.rs`, tantivy `0.26.1`.

```rust
pub struct TermInfo {
    pub doc_freq: u32,
    pub postings_range: Range<usize>,
    pub positions_range: Range<usize>,
}
```

Serialized as `doc_freq: u32`, `postings_range.start: u64`, `postings_num_bytes:
u32`, `positions_range.start: u64`, `positions_num_bytes: u32` — 24 bytes per entry,
uncompressed. The struct's own doc comment states the real on-disk encoding is not
this fixed-size form directly: "in practice, `TermInfo` are encoded in blocks and
only the first `TermInfo` of a block is serialized uncompressed. The subsequent
`TermInfo` are delta encoded and bitpacked" — a block-delta scheme layered on top of
the fixed-size logical struct, not a naive flat array.

## The competing design: Lucene's BlockTree (sparse index, not a dense per-term FST)

**Source:** secondary (Apache Lucene's own `Lucene90BlockTreeTermsWriter`
documentation and package overview; not independently re-fetched from Lucene source
in this pass — flagged as such, weaker grounding than the tantivy citations above).

Lucene's term dictionary organizes terms into blocks by shared prefix (default block
size 25–48 terms) and builds an FST *index over the blocks*, not over individual
terms — the FST's leaves point at block locations, and the actual term bytes and
metadata for the matching block are read from a separate file (`.tim`) only after
the FST navigates to the right block. This makes Lucene's FST **sparse**: it has
roughly `1/block_size` as many entries as there are terms, deliberately trading a
small amount of extra seek/scan work within a block for a dramatically smaller
in-memory/cold-fetched index — the opposite tradeoff from tantivy's dense,
one-entry-per-term FST.

This is a real, load-bearing architectural fork for any STRAND design choice: a
dense FST (tantivy-style) is architecturally simpler (one lookup structure, not two
nested ones) but its size scales with total vocabulary size; a sparse FST
(Lucene-style) stays small regardless of vocabulary size at the cost of a second
lookup stage into block storage.

## `fst` crate — license, version, and an unresolved determinism question

**Source:** `github.com/BurntSushi/fst`. License confirmed via the crate's own
`Cargo.toml`: `license = "Unlicense/MIT"` (dual-licensed; Apache-2.0-compatible via
the MIT option). Current version `0.4.7` (crates.io). tantivy vendors its own fork,
`tantivy-fst` (`github.com/phiresky/tantivy-fst`), Unlicense-licensed, confirmed via
GitHub's license API.

**Not resolved by this vendoring pass, flagged honestly:** whether the `fst` crate's
compiled binary output is byte-for-byte deterministic given the same sorted input
keys inserted in the same order — i.e., whether two independent builds (potentially
on different crate versions, different platforms) of the same logical term set
produce identical bytes, which invariant 11 would require if a term-dictionary FST
blob is `storage-class: raw-mappable` (uncompressed, byte-exact golden-file
comparison) rather than `chunk-compressed` (round-trip-and-checksum verified only).
A search of the crate's own `src/raw/build.rs` found no explicit hashing or
randomization in the construction path, which is reassuring but not a
verification — determinism was not independently confirmed by an actual
build-twice-and-byte-compare test in this pass. A future RFC finalizing this design
MUST run that test before claiming invariant 11 conformance, not assume it from the
absence of an obvious counter-signal.
