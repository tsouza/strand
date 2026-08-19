# Milestones

Extracted at repository seeding from the seed constitution's milestones section
(now `CLAUDE.md` §10 — the seed's numbering differed). `CLAUDE.md` keeps a
one-line summary per milestone; this file is the full detail each milestone session
works from. Each milestone ends with a short written report in `docs/`: the numbers,
what they mean, what they changed. A benchmark that embarrasses us goes in the report
with an analysis, not in the memory hole.

**M0 — Container + manifest.** Container spec chapter, chunk/block split, storage-class
and alignment attributes, invariant-11 byte-determinism pins (endianness, chunk codec,
checksums), footer/hotcache layout, blob registry, row-ID chapter with per-family
merge-semantics declarations, **manifest chapter with the CAS commit protocol and the
`CLAUDE.md` §6 safety rules (declared CAS host, deletion-safety retention, reader 404-refresh,
orphan rule)**. `strand-core` read/write; `strand-tools inspect`. Golden files. Vendor
`references/` and `docs/research/` from `docs/research/README.md` — **partially done**:
`references/` holds the R2 and RFC-0002 grounding, all four turbopuffer pages, both
adapter LICENSE files, and — as of 2026-08-18 — R1/R3–R9's core primary sources (the
"full kickstart report" this entry previously also listed as owed was a retracted
phantom deliverable, never a real document — `docs/ledger.md`). A small residue of
paper-body-only figures and two lower-priority R4 sources remain flagged as owed;
`docs/ledger.md` lists them precisely. The batch-shaped
reader trait (invariant 9's frozen API shape) was originally listed here but is **not
yet implemented** — no `next_batch()` interface exists in the code; it carries forward
as an M1 prerequisite, since M1's postings kernels are its first real consumer
(tracked as an open item in `docs/ledger.md`). Benchmarks and tests: cold end-to-end
open (pointer → planned query) GET count and latency against MinIO; manifest commit
contention (two writers racing the pointer); crash tests (writer dies before commit →
orphans, reader on expired snapshot → 404-refresh path). Two originally-listed items
were structurally deferred at M0, not missed: parallel-wave aggregate throughput
could not be measured until a `tier: cold-fetchable` vector blob existed — M2 shipped
that blob, and roadmap item X-5 measured it afterward (`bench/src/
parallel_range_fetch.rs`, real numbers in `CLAUDE.md` §7 and `docs/ledger.md`) — and
the compaction crash test (deleting a file under a retained
snapshot must be impossible) requires M3's compaction — the deletion-safety rule is
normative in `spec/manifest.md` §4 but untestable until the sweep exists. The
tail-latency deliverable is partially met: local-MinIO p50/p90/p99 exist in
`bench/results/cold-open.json`, confirming the GET-count half of invariant 3. The
separate `CLAUDE.md` §7 SLO tail figure this entry used to call a "placeholder" is
now resolved a different way, not by this benchmark: a real AWS primary source was
located and vendored (`references/aws-s3-small-object-latency.md`, `docs/ledger.md`),
so §7 no longer carries an unsourced number. What remains genuinely open is a
real-network measurement of STRAND's *own* cold-open sequence — MinIO with injected
latency, or real S3 — which this benchmark still does not provide. Implementation went
beyond this list in one respect: the store abstraction distinguishes definite from
ambiguous backend failures (`StoreError::Ambiguous`), `commit()` disambiguates an
ambiguous pointer CAS with a follow-up read (RFC 0001's Discussion section records
the amendment), and a `proptest`-based fuzzer drives randomized concurrent-writer
rounds through the protocol's safety invariants. The batch-shaped reader trait
(invariant 9's frozen `next_batch()` shape), originally listed here as an M0
deliverable but left unimplemented, is now real: `crates/strand-core/src/batch.rs`
defines `BatchReader`, and M1's postings kernels are its first consumer
(`PostingsReader::batches()`, one block per batch), closing the gap this entry used
to flag (`docs/ledger.md`).

**M1 — Lexical.** BP128 postings + positions + FST term dictionary + block-max
sibling blob + Roaring filter bitmaps. The **term-dictionary FST and term-info
store** are now RFC 0005 (`rfcs/0005-term-dictionary.md`, Approved) — adopts
tantivy's real two-part design (dense per-term FST, separate ordinal-indexed
term-info array) directly rather than inventing one, with a worked example built
from the actual `fst` crate (not illustrative bytes) and independently reproduced
byte-for-byte during review; also the first RFC to populate the blob-type registry
(`spec/container.md` §9). FST size at realistic vocabulary scale and the `fst`
crate's cross-platform byte-determinism are both still open (RFC 0005's own Open
questions). A reference implementation now exists (`crates/strand-lexical`,
new crate): `TermInfo` encode/decode, `TermInfoStore`'s direct-indexed reads, and
`TermDictionary`'s FST build/lookup, with the cat/dog/fish worked example pinned as
`conformance/term-dictionary/` golden files and reproduced byte-exact by the crate's
own tests — a third independent reproduction of the same bytes (RFC draft, ACPR
review, now the library) — plus a `proptest` round-trip property test over
arbitrary term sets. The postings blob those `TermInfo` offsets point into is
now implemented (RFC 0007, below); the positions blob is now implemented too
(RFC 0008, below). The **filter-bitmap blobs** are now RFC 0006
(`rfcs/0006-filter-bitmaps.md`, Approved) — a value-dictionary FST (identical shape
to RFC 0005 §2) paired with a small directory plus one standard 32-bit Roaring
bitmap per distinct value, indexed by local ordinal; the second RFC to extend the
blob-type registry (`family_id = 2`, "filter"). Its review surfaced a real,
same-version/same-platform byte-determinism risk in the `roaring` crate's
run-container promotion (`insert_range` can select a differently-serialized
container for an ordinary contiguous write), independently confirmed against the
crate's own source and closed with a normative MUST to always serialize without
run containers — the same class of risk RFC 0005 named for the `fst` crate, but
sharper: same build, same platform, different insertion API, different bytes,
absent the MUST. Cross-platform/cross-version determinism for both the `fst` and
`roaring` halves remains open (RFC 0006's own Open questions). Both RFCs now also
have a reference implementation (`crates/strand-lexical`): value-dictionary FST
build/lookup reuses the term-dictionary FST code directly (validating RFC 0006's
own "identical in shape" claim in working code), `build_filter_bitmap_store`
normalizes every bitmap with `remove_run_compression` before serializing, and a
dedicated test builds the identical logical bitmap through two different `roaring`
insertion APIs and asserts byte-identical output — a mechanical, not merely
prose, check that the no-run-containers MUST actually closes the gap the RFC 0006
review found. The blue/red worked example is pinned as `conformance/filter-
bitmaps/` golden files and reproduced byte-exact. The R2 RFC pins the exact d-gap variant
(invariant 11) and the block-max RFC pins the raw-statistics fields (invariant 4);
neither is drafted yet, both gated on R9's still-unmeasured margin
(`docs/ledger.md`) — though a maintained, Apache-2.0 Rust FastLanes implementation
now exists to measure against (`references/spiraldb-fastlanes-rust-crate.md`),
lowering what running that measurement actually costs, and a first real
decode-throughput measurement (`bench/results/codec-decode-throughput.json`) now
covers `BitPacker4x`/`BitPacker8x`/`FastLanes`/`FastPFOR` on both uniform and
synthetic-skewed data. The **scoring-profiles
chapter** (RFC 0003, `rfcs/0003-scoring-profiles.md`, Approved) defines the `bm25`
profile normatively and the Lucene-parity profile, both grounded byte-exact against
Robertson & Zaragoza's own formula and Lucene 10.5.1's real source, with a worked
example. A reference implementation now exists (`crates/strand-core/src/scoring.rs`):
both profiles' score formulas, and `SmallFloat.intToByte4`/`byte4ToInt` ported
directly from the vendored Lucene source rather than reimplemented from the paper
description, tested to 1e-6 against the RFC's own worked-example numbers
(`idf = 0.847298`, `score = 0.601988` for `bm25`; `score = 0.859981` for
`lucene-parity`). **Analyzer descriptor schema and the normative token-stream
vectors in `conformance/analyzers/` are gating deliverables of this milestone, not
metadata afterthoughts** — without them invariant 6 is a label; the schema and
per-document-length definition are now RFC 0004 (`rfcs/0004-analyzer-descriptors.md`,
Approved), with one real worked example as the first conformance vector. That
vector is now implemented and checked, not just described:
`crates/strand-lexical/src/analyzer.rs` runs the exact chain the worked example
names (UAX #29 `"word-only"` tokenization via `unicode-segmentation`, `"lower"`
case folding, `lucene-en-10.5.1` stopword removal, `snowball-porter2-en` stemming
via `rust-stemmers`) against `conformance/analyzers/lucene-en-word-only-01.json`,
reproducing `"The whales swim quickly."` → `["whale", "swim", "quick"]`,
`dl = 3`, byte-for-byte with the RFC. Both new crate dependencies are real,
licensed implementations (`unicode-rs/unicode-segmentation`,
`CurrySoftware/rust-stemmers`, both Apache-2.0-compatible), not hand-rolled
algorithms. The full vector suite across languages and scripts is still M1
execution work, not done by this one vector, and the CJK/Thai/Lao segmentation-
dictionary choice remains unresolved (RFC 0004's own Non-goals). **Tantivy importer
is now real**: `strand-tools convert --index-dir <path> --field <name> --output
<path>` (`crates/strand-tools/src/convert.rs`) opens a real tantivy index via
tantivy's own reader API (`InvertedIndexReader`, `TermDictionary::stream`,
`Postings` — not a hand-reimplementation of tantivy's binary format),
streams every term's real postings and positions, and feeds them into
`strand_lexical::field::build_field_from_postings` — the same real,
already-tested blob-building code `build_field` uses for text input, now
factored out as a shared entry point precisely so this importer could reuse
it rather than duplicate it. Tantivy's segment-local `DocId` becomes the
STRAND local ordinal directly, since both are already dense `0..num_docs`
spaces. Verified end to end, twice: a real Rust unit test builds a real
tantivy index, imports it, assembles a real STRAND segment, and runs real
term and phrase queries against it — including a true positional phrase
match ("dog cat" adjacent in one document) and a true negative ("dog park"
co-occurring but never adjacent) — and a manual CLI smoke test round-tripped
`convert` into `inspect`, confirming a real 527-byte, 4-blob, checksum-valid
segment on disk. Deliberately narrow, named scope, not silently assumed
general: only a single-segment, deletion-free tantivy index is accepted
(multi-segment merge and deletion-vector support are real, separate,
unattempted follow-on work); positions are always imported (a source field
indexed without them is out of scope for now). **Verified at real scale**:
`bench/src/tantivy_import_scale.rs` builds the same real MS MARCO sample
two independent ways — native `build_field` and a real tantivy index run
through the importer — and compares the resulting blobs byte-for-byte;
both matched exactly at 5,002 and at 50,238 real documents, the larger of
which is big enough to exercise multi-block postings/positions encoding
that the original 3-document unit test could not reach (`docs/ledger.md`
has the full byte counts). R2
codec bake-off lands here and confirms or swaps the postings default (including
verifying tantivy's actual current codec, per `docs/data-structures.md`); the R9 layout evaluation and
license audit MUST complete before the bake-off freezes the default, since a
FastLanes outcome changes both the default and the block granularity — the license
half is now resolved (`docs/ledger.md` R9), the layout-evaluation half is now
partially resolved: `bench/src/msmarco_index.rs` builds a real inverted index over
a ~520K-passage (5.9%) stride-sampled subset of the actual MS MARCO passage corpus
(8,841,823 passages, fetched via a Hugging Face mirror after Microsoft's own Azure
blob access was found revoked mid-session) using RFC 0004's own analyzer chain, and
feeds real doc-ID delta-gaps and term frequencies into
`codec_decode_throughput.rs`: FastPFOR's real compression advantage over
`BitPacker8x` is 4.26x on gaps and 5.2x on term frequencies — well under the
earlier synthetic 95/5 split's 17x, while its ~7-8x decode-speed cost held steady
between synthetic and real data (`docs/ledger.md` R9). Still not the full corpus or
ARM hardware, so R9 remains open, but the "real, not synthetic" gap this milestone
named is now measured, not asserted. **The postings codec itself is now RFC 0007**
(`rfcs/0007-postings-codec.md`, Approved) — registers `BitPacker8x` (256-value
blocks, vertical SIMD) as the default over FastPFOR and FastLanes on real,
measured decode-throughput and compression numbers, with a required
variable-length final block (no padding waste on short lists, which this session's
own real MS MARCO sample is 69% of, by count) and a real, measured block-max
sibling region (invariant 4) giving a ~7× skip-cost cut on multi-block lists. Its
own napkin math found something real worth carrying forward: postings + term-info
for the ~520K-passage sample already total ~73.2 MB, 73% of the 100 MB cold-open
budget, on under 6% of the full corpus (later found to be a 2.09× overestimate —
see the real-tantivy correction below) — a live input for R1's segment-sizing
work, not just this RFC's own concern. RFC 0007 is now implemented, not just
approved: `crates/strand-lexical/src/postings.rs` builds and reads the blob
exactly as `spec/postings.md` specifies, the RFC's own worked example round-trips
byte-exact against the new `conformance/postings/toy-postings.bin` golden file,
and property-based tests (`crates/strand-lexical/tests/postings_round_trip.rs`)
cover multi-block lists spanning `BitPacker8x`'s SIMD kernel and the
variable-length final block's scalar packer together, including skip queries
checked against a linear-scan reference. Explicitly out of scope: positions,
scoring-aware (term-frequency/document-length) block-max bounds, and ARM
validation — the RFC's own adversarial review caught and corrected a false claim
that the registered codec's crate mitigates the ARM gap; it doesn't, for the
specific 256-value format this RFC registers, and that gap is now stated
honestly rather than glossed over. **Positions are now designed, not just
deferred**: RFC 0008 (`rfcs/0008-positions.md`, Approved) registers a positions
blob (`spec/positions.md`, `family_id = 1`, `blob_type_id = 3`) that reuses RFC
0007's codec wholesale — the same `BitPacker8x`/scalar-packer block structure,
applied to within-document position deltas that reset per document rather than
once per whole term — bridged to postings via a new
`postings_block_pos_prefix` region so a phrase query can locate one document's
positions without decoding any postings or position block it doesn't need.
`total_term_freq` (the sum of a term's per-document term frequencies, needed
to size the position-block region but not recoverable without decoding) is
stored as a leading field inside the positions blob itself rather than growing
RFC 0005's already-implemented `TermInfo` record. This RFC's own initial
napkin-math measurement (extending `bench/src/msmarco_index.rs` to sum total
term occurrences on the same MS MARCO sample) found positions add roughly
another 19.3–27.4 MB on top of RFC 0007's originally-reported ~73.2 MB
postings-plus-term-info figure. **RFC 0008 is now implemented, not just
designed**: `crates/strand-lexical/src/positions.rs` builds and reads the
blob exactly as `spec/positions.md` specifies, reusing `postings.rs`'s
scalar packer and block-count helpers directly rather than duplicating
them; the RFC's own worked example round-trips byte-exact against the new
`conformance/positions/toy-positions.bin` golden file, including targeted-
lookup resolution for every document; and property-based tests
(`crates/strand-lexical/tests/positions_round_trip.rs`) cover multi-block
lists with postings-block and position-block boundaries stressed
independently (they're different counts), checking targeted lookups
against the original input directly, not against this blob's own decode
path. **Now wired into `crates/strand-lexical/src/field.rs`**: `build_field`
tracks each token's within-document position and builds the positions blob
alongside the other three; `FieldReader::phrase_query` resolves real
adjacent-position phrase matches (correctness-first — full decode and
intersection, not yet the block-targeted skip `PositionsReader::
positions_for_doc` makes possible). The end-to-end test now proves a real
positive phrase match and a real negative one (two terms that co-occur in
a document but are never adjacent), confirming this is true positional
matching, not co-occurrence.

**The MS MARCO-vs-tantivy benchmark this entry named as still owed has now
run, and it found and corrected a real overestimate.** `bench/src/
tantivy_index.rs` builds a real tantivy index over the identical corpus
sample and token stream RFC 0007/0008 measured (every document fed as a
`PreTokenizedString` from this project's own analyzer output, so tantivy's
own tokenizer is never invoked — isolating the comparison to on-disk format
efficiency and query latency): tantivy's real postings file is `≈ 29.50 MB`,
its real positions file `≈ 18.94 MB`, its real term dictionary `≈ 8.05 MB`,
total `≈ 59.11 MB` (`bench/results/tantivy-index-benchmark.json`; mean
term-query latency ~95.7μs, mean phrase-query latency ~414.7μs,
single-threaded). RFC 0007's `~61.6 MB` postings estimate — a stratified
4,016-list sample's real ~149-bytes/list mean, linearly extrapolated across
the full vocabulary — was `2.09×` tantivy's real number. Rather than accept
that gap, `bench/src/msmarco_index.rs` was extended to build every term's
*actual* RFC 0007 postings blob (`strand_lexical::postings::build_postings`,
the real shipped code, not a projection) across the full 413,364-term
vocabulary and sum real bytes: `≈ 29.49 MB` — a `0.05%` difference from
tantivy's real number, essentially exact. The extrapolation was wrong; the
codec was right, and this real cross-check against a battle-tested engine
confirms it rather than embarrassing it. RFC 0008's own positions bound held
up independently (`≈ 19.28 MB` vs. tantivy's real `≈ 18.94 MB`, `1.8%` off)
— it was never built on the flawed extrapolation. Both RFCs now carry this
correction in their own Discussion sections (`docs/ledger.md`'s R2 entry has
the full account); the corrected combined cold-open figure for postings +
term-info + positions is `≈ 60.35 MB` to `≈ 68.46 MB` — 60–68% of the 100 MB
budget, real headroom, not the `~92.5–100.6 MB` RFC 0008 originally reported
— though positions remaining an opt-in, per-field blob (§10) is still the
right default, just less urgent than first stated. Remaining benchmarks:
Lucene parity per invariant 5; bytes-fetched vs bytes-used across term
frequency deciles (the read-amplification number, `docs/data-structures.md`).
Adapter-based results appear in this milestone's report only after R11
verifies the respective extension point and the build-equivalence gate
passes; until then, harness and published-numbers baselines only.

**The segment/container layer and the lexical blobs are now actually wired
together, not just individually tested.** A maturity check found that
`strand-core` (segments, manifest) and `strand-lexical` (term dictionary,
postings) had never been composed — every test built one blob type in
isolation, and the existing MinIO cold-open benchmark opens a segment with
literal placeholder bytes, not real content; nothing anywhere resolved a
query string to a result. `crates/strand-lexical/src/field.rs` closes that
gap: `build_field` turns real document text into a field's three lexical
blobs, `to_blob_specs` wraps them with their already-registered
classification for `SegmentBuilder`, and `FieldReader` reads them back from
a resident segment's blob registry and resolves real term lookups and a
real BM25-ranked search. `crates/strand-lexical/tests/
field_end_to_end.rs` is the first real end-to-end test in the repo: real
text through the analyzer, into a real segment, committed through
`strand-core`'s actual manifest CAS protocol against a real store, read back
cold via a footer/hotcache decode, and queried — all passing. `docs/
ledger.md` has the full account, including the scope this first pass
deliberately leaves out (one field per segment, no filter bitmaps, no
merge — positions is wired in now, see below).

**Positions is wired into `field.rs` (real phrase queries), and the real
cold-open claim is now tested against real MinIO with real content.**
`FieldReader::phrase_query` resolves real adjacent-position matches — the
end-to-end test proves a true positional match, not co-occurrence (two
terms that appear in the same document but never adjacent correctly
return no match). Re-running the tantivy comparison with positions real on
both sides reversed the earlier "STRAND's total is smaller" reading:
STRAND's total segment is now 9–22% *larger* than tantivy's real index
once positions and term metadata are honestly counted, though postings
alone still ties or beats tantivy — `docs/ledger.md` has the full,
unflattering account. Separately, `bench/src/field_cold_open.rs` finally
tests this project's actual thesis rather than the postings codec's byte
efficiency: a real field (RFC 0005/0007/0008's blobs, real MS MARCO
passages) committed to real MinIO, opened cold, and queried with a real
BM25 search and a real phrase query. Result: **3 GETs per open at both
5,002 docs (1.45 MB) and 50,238 docs (8.57 MB)**, and running the real
queries after open costs the identical GET count as opening alone — a
real, measured confirmation of invariant 3's one-wave rule for an actual
query, not just an open, which nothing had checked before. This is a
claim tantivy cannot be compared against at all (no object-storage-native
open path exists in tantivy), and it is the comparison this project's
stated mission (`CLAUDE.md` §1) actually rests on — not total on-disk
size, which is the metric currently unfavorable to STRAND. `docs/
ledger.md` has the full numbers, including the honest caveat that MinIO
on `localhost` confirms the GET-count half of the claim, not the
real-network tail-latency figure `CLAUDE.md` §7 still lists as open.

**RFC 0009 narrows the total-size gap the previous paragraph names, with
real, confirmed numbers, not just a predicted fix.** Two per-term
fixed-overhead reductions (`rfcs/0009-per-term-overhead-reduction.md`,
Approved and implemented): omitting `postings_block_pos_prefix[0]` (always
`0` by construction, stored anyway until now) shrinks every term's
positions blob by 4 bytes unconditionally; a new 16-byte short term-info
record removes `TermInfo`'s 12-byte-per-term dead weight for fields that
opt out of positions entirely — now wired into `field.rs`
(`build_field_without_positions`) too, with its own predicted payoff
confirmed exactly the same way: `term_info` shrank from `890,008` to
`508,576` bytes at 10,003 documents (`381,432` bytes saved, exactly RFC
0009's own prediction) and from `3,804,472` to `2,173,984` at 100,476
(`1,630,488` bytes saved), with the whole positions blob unwritten on top
of that. Re-running the same real MS MARCO comparison confirmed the first
fix's predicted numbers exactly:
positions shrank from `620,503` to `493,359` bytes at 10,003 documents and
from `4,678,608` to `4,135,112` at 100,476, narrowing the positions-blob
gap against tantivy's real `.pos` from `33.2%` to `5.9%`, and from `16.8%`
to `3.3%`, respectively — and the total-segment gap from `22.5%` to
`16.0%`, and from `9.0%` to `5.1%`. The gap is smaller, real, and
honestly reported either way; `docs/ledger.md` has the full account,
including the one real design cost this RFC accepted: its fix to an
already-shipped, already-golden-filed blob layout is a breaking, in-place
change, not an additive one — RFC 0008's original positions golden file
is retired, not kept alongside the new one.

**M2 — Vectors, cluster-first.** Flat vector blob; RaBitQ codecs with kernel-per-
bit-width, the rotation descriptor field, and the rotation-provenance mechanism
(invariant 11); the **cluster-family cold-native blob** (navigation tier + wholesale
posting lists + rerank region) per the R1 RFC, with all posting-list offsets
resolvable from the navigation tier (invariant 3's one-wave rule), the replication
knob and tier-1 sizing limits in blob metadata, computed against `CLAUDE.md` §7's cold-open byte
budget. The warm-tier graph blob family (persisted-permutation node order, ordering
algorithm per R1's evidence) is in-scope but explicitly second. Benchmarks: cold and
warm ANN recall/latency with GET counts asserted; codec comparison RaBitQ vs
PQ-FastScan; the cold target to measure against is turbopuffer's published figures
(`docs/benchmarks.md`), with the asymmetry stated. Adapter-based results appear in this
milestone's report only after R11 verifies the respective extension point and the
build-equivalence gate passes; until then, harness and published-numbers baselines
only. **RFC 0010 (Approved) opens this milestone**: registers `family_id = 3`
("vector") with the flat vector blob, the RaBitQ quantization descriptor (rotation
descriptor field and rotation-provenance mechanism resolved — materialized state for
both registered rotator types, `spec/vectors.md` §2), and the cluster-family
cold-native blob (navigation tier + posting lists — the rerank region is the flat
vector blob itself, `blob_type_id = 0`). **1-bit RaBitQ only**; multi-bit
Extended-RaBitQ and the warm-tier graph blob family remain unimplemented, both named
follow-on work (RFC 0010 Non-goals). The FastScan code region's intra-batch
bit/lane order — the one wire-format gap RFC 0010's own review left open at
Approval — is now resolved (RFC 0010 Discussion, `spec/vectors.md` §4),
clearing the way to actually start `crates/strand-vector`. **Not yet met by
RFC 0010 alone**: the
replication knob this milestone names as a deliverable — RFC 0010's own napkin math
computes replication's real cost impact (a ~2.27×-the-budget estimate at realistic
replica-8-equivalent density, resting on a real body-sourced 1.73× ratio as of
2026-08-19 — SPANN's own paper has no GIST1M index-size figures at all; the real
13.0 GB/7.5 GB numbers live in the companion cloud-native benchmark paper's Table 4,
`references/spann-body-figures.md` — though still extrapolated from a replica-2
baseline since neither paper measures replica-1) but does not
add the metadata slot or construction algorithm; a follow-on RFC owns it. RFC 0010's
own corrected sizing law (~131 MB per million 768d vectors before replication, not
the previously assumed ~100 MB) is real, grounded arithmetic — **now backed by a
real measured M0-style benchmark, not only arithmetic**: `bench/src/
vector_cold_open.rs` assembles a real four-blob-type segment (10,000 real
768-dimensional vectors, real k-means clustering into 400 clusters, real
`FhtKacRotator` rotation, real 1-bit RaBitQ quantization — the same
`crates/strand-vector` functions its own tests exercise, no synthetic byte
fillers), commits it to real MinIO via `strand-core`'s actual manifest CAS protocol,
and reopens it cold 30 times. Measured result (`bench/results/vector-cold-open.json`):
the descriptor and navigation-tier blobs, read back from the real committed
segment's own hotcache registry, total 1,238,808 bytes — **1.24% of the 100 MB
cold-open byte budget** — in a constant 3 GETs per open, and this run's own real
per-cluster byte cost extrapolates to RFC 0010's 1,000,000-vector napkin-math scale
at 12,384,408 bytes, matching that RFC's hand-computed ≈12.4 MB figure to the byte
— the formula confirmed by real, executed code, not only trusted. One limitation
carried over honestly from `bench/src/cold_open.rs` and `bench/src/
field_cold_open.rs`: `strand-core`'s `ConditionalStore` has no Range-GET method yet,
so the real network fetch this benchmark issues at open pulls the whole segment
object (33,984,732 bytes, posting lists and flat vectors included), not just the
open-wave subset a conforming Range-GET reader would fetch — real byte-count
separation by blob type, not yet a real separated-latency measurement (measured
whole-segment-GET latency: p50 92.6ms, p90 102.2ms, n=30, MinIO on localhost, no
injected network latency). RFC 0010's own Open questions item asking for exactly
this measurement is now resolved (RFC 0010 Discussion, 2026-08-19); this milestone's
gate on it is met, with the Range-GET-reader and real-network-tail-latency
limitations named as real, separate follow-on work rather than silently closed.
**`crates/strand-vector` now exists**, implementing all four blob types'
wire format (`descriptor.rs`, `navigation.rs`, `posting_list.rs`, `flat.rs`, plus
`fastscan.rs` for the FastScan pack/unpack codec) with real round-trip tests, a
proptest suite, worked-example golden files (`conformance/vectors/`) matching RFC
0010's own worked example byte-for-byte, and a full end-to-end test assembling all
four blobs into a real segment via `strand-core`'s actual `SegmentBuilder`, opening
it cold, and re-reading every blob. **The real RaBitQ 1-bit quantization math is
now implemented too** (`quantize.rs`'s `quantize_one_bit`) — the sign-based binary
code and the `f_add`/`f_rescale`/`f_error` factor formulas, grounded against the
reference implementation's actual source and cross-checked against an
independently compiled and executed C++ reimplementation, with an end-to-end test
quantizing real vectors straight into a real posting-list blob. **Rotation
*application* is now implemented too** (`rotate.rs`'s `rotate_fht_kac` and
`rotate_matrix`) — the full `FhtKacRotator::rotate()` pipeline (sign flips, a
generalized Fast Walsh-Hadamard Transform, Kac's-walk mixing) grounded against the
reference source and independently cross-checked against a compiled C++
reimplementation, plus a `proptest` confirming the mathematical property a rotation
must have (L2-norm preservation) across hundreds of random inputs. A new
`tests/full_pipeline.rs` chains descriptor, rotation, quantization, and the
posting-list wire format together end to end for the first time: raw, unrotated
vectors in, real blob bytes out, read back bit-exact. **The query-side distance
estimator is implemented too** (`estimate.rs`'s `estimate_distance`), closing every
Non-goal RFC 0010's own Design §4 named as "the algorithm's concern" — grounded
against the reference implementation's formal derivation and real query-factor code,
with a real ambiguity in the math notation (whether a second, inverse-rotation
pipeline was needed) resolved by reading the actual code rather than picking an
interpretation. Verified against a compiled C++ cross-check (the true distance falls
inside the estimator's own bound, the real theoretical guarantee, not just matching
values) and a 2,000-trial statistical test (96%+ containment, checked statistically
rather than as a flaky proptest property since the guarantee is probabilistic by
design). `tests/query_a_real_cluster.rs` is the first genuinely full end-to-end test:
a real cluster written to a real blob, a real query scanned against it with no further
I/O, correctly ranking the true nearest neighbor. **Real k-means clustering is
implemented too** (`kmeans.rs`) — Lloyd's algorithm with k-means++ seeding, the first
module in this crate with no external reference to byte-match (the reference library
itself delegates clustering to Faiss rather than shipping its own; RFC 0010 Design §3
already named clustering as construction-side and wire-format-irrelevant), verified
by testing the properties a correct implementation must have (monotonically
non-increasing inertia, no empty clusters, deterministic given a seed, well-separated
blobs recovered exactly) rather than cross-checked against a compiled reference.
`tests/build_a_real_index.rs` is the capstone test so far: 200 raw vectors, clustered
from nothing, built into a real four-blob-type segment, opened cold, and queried
across every real cluster, correctly matching brute-force ground truth. **The
`nprobe` cluster-selection pipeline is implemented too** (`query.rs`), completing
RFC 0010 Design §6's query-resolution steps 1–3 — the first module this session with
no external reference to fetch or match, since it's STRAND's own already-specified
algorithm. `select_nprobe_clusters` picks the closest `nprobe` centroids (metric-aware,
no I/O); `scan_selected_clusters` decodes and estimates every candidate, deduplicating
by row-id under closure replication per the spec's own literal wording. Verified
against the real property the feature exists for: across four real queries on a real
400-vector index, once the true nearest neighbor was found at some `nprobe` it was
never lost at a larger one — recall monotonically non-decreasing in `nprobe`, not
merely "the code runs." **`MatrixRotator`'s matrix *generation* is implemented too**
(`orthogonal.rs`): random Gaussian sampling plus a from-scratch Householder QR
decomposition, matching the reference implementation's own algorithm. QR
decomposition is not unique — unlike every other numerically-precise module this
session, there is no single correct output to byte-match — so this module is verified
against the three properties that define a valid QR decomposition directly (`Q`
orthogonal, `R` upper triangular, `QR` reconstructs the input), at sizes up to a real
768-dimension embedding scale. `tests/matrix_rotator_pipeline.rs` carries a freshly
generated (not caller-supplied) matrix through descriptor serialization, rotation
application, quantization, and the posting-list wire format, bit-exact. **Multi-bit
Extended-RaBitQ (RaBitQ+) is registered and implemented too**, via a follow-on RFC 0011
(`rfcs/0011-multibit-extended-rabitq.md`) — unlike every other module this session, this
one genuinely needed a new RFC before any code, since RFC 0010's own Non-goals required
it and `docs/data-structures.md` already commits the multi-bit path to a different
kernel (classical scalar-quantization distance, not FastScan LUT). `bit_width` widens
from a fixed `1` to `1..=8`; a new ex-code region (`spec/vectors.md` §4.1) is appended
inside the existing cluster posting-list blob, packed STRAND's own plain,
bit-contiguous way rather than the reference's AVX-only SIMD-shuffled layout (no
portable scalar source exists for it — adopting it verbatim would have repeated the
Optane-era formats' mistake, `docs/lineage.md`). `quantize_ex.rs` (new) implements the
encode side (`best_rescale_factor`'s greedy search plus the RaBitQ+ factor formulas),
cross-checked against a compiled C++ reimplementation; `estimate.rs` gained
`estimate_distance_boosted`; `query.rs`'s `scan_selected_clusters` uses the boosted
estimate whenever `ex_bits > 0`. The RFC's own adversarial review caught a real
Critical bug before any code existed — an unaddressed zero-residual `NaN` case, the
same class of bug the 1-bit path's own ACPR found — fixed with the same degenerate-value
substitution the 1-bit path already uses. Verified against the real property the
feature exists for: on a real 50-vector cluster, the boosted estimate's mean-squared
error against true (unquantized) distance measurably beats the 1-bit-only estimate's,
confirming the extra bytes buy real accuracy. **Deletion-vector integration is
implemented too**, via a follow-on RFC 0012 (`rfcs/0012-deletion-vectors.md`) that
pulled the general invariant-2 mechanism forward from M3: a standalone deletion-vector
object (`spec/deletion.md`, `family_id = 4`, a bare standard Roaring bitmap, no
container framing, since segments are immutable and a deletion vector must be
revisable), a new optional `SegmentRef.deletion_vector` reference, and a second
manifest commit path (`commit_deletion_vector`) sharing `commit`'s exact CAS mechanics
via a newly-extracted `propose_snapshot` helper. `query.rs` gained `filter_deleted`,
implementing `spec/vectors.md` §6 step 4 for real. The adversarial review caught a
self-contradicting Critical bug in the first draft (a closure signature that could not
satisfy the RFC's own stated race-safety requirement) and a genuine, unmodeled
formal-verification gap (`verification/manifest.tla`'s `ProposeSnapshot` only ever
appends a segment; it has no shape for revising one in place) — both fixed or named
precisely rather than glossed. `tests/deletion_end_to_end.rs` proves the whole chain: a
real segment committed through the real manifest CAS protocol, a real deletion vector
committed against it, and a real vector-family query excluding the tombstoned row and
promoting the runner-up. **Reranking against the flat-vector blob is implemented
too** (Design §6 step 5, `query.rs`'s `exact_distance`/`rerank`), closing RFC 0010's
Non-goals list completely — no new RFC needed, since the flat-vector blob's format and
this exact step were already designed, cited (DiskANN/SPANN/turbopuffer's own
"quantize for the scan, exact-rerank the survivors" pattern, `docs/lineage.md`), and
adversarially reviewed when RFC 0010 was approved; this was wiring, not new design.
`tests/rerank_end_to_end.rs` proves the real property: a real, deliberately tight
40-vector cluster (the regime where 1-bit RaBitQ's lossiness can plausibly misorder
close candidates) is scanned and reranked, and the reranked order matches an
independently computed brute-force ordering exactly, row-id for row-id, not just
plausibly. **`verification/manifest.tla` is extended too**, closing the TLA+ model
correspondence gap RFC 0012's own review found: a new `ProposeDeletionVectorCommit`
action (guarded by a new `DeleteWriter` constant, the same pattern `DistinguishedWriter`
already established) models `commit_deletion_vector`'s revise-in-place commit shape,
and two new invariants (`SegmentCountNeverDecreases`,
`DeletionVectorCommitsOnlyReviseOneEntry`) are both confirmed load-bearing by real
mutation tests, not merely holding by construction. TLC re-verified clean at 5,943
states (up from 591), all nine invariants holding. **Still deliberately out of
scope**: `faster_quantize_ex`'s construction-time speedup (unregistered, real
writer-side optimization); and, named in RFC 0012's own Non-goals, compaction-time
physical removal and deletion-vector merge semantics — pulled forward to M3.

**M3 — Hybrid + deletes + merge.** The deletion-vector *mechanism* now exists (pulled
forward via RFC 0012, above); M3's own scope narrows to what that RFC named as
Non-goals: compaction implementing the
per-family merge semantics of invariant 1 (concatenate+remap for cluster blobs,
rebuild for graph blobs, rebalance for centroids), respecting `CLAUDE.md` §6's deletion-safety
rule, with merge cost benchmarked per strategy; the orphan-sweep tool; end-to-end
hybrid RRF across both blob families over one row-ID space. **The multi-segment
benchmark**: the same corpus at 1, 16, and ~128 segments, cold and warm, so
segment-count amplification is a measured curve feeding R10. Deliverable: **a
benchmark report measured against published figures, with the caching-fleet
asymmetry stated.**

**Manifest formal verification gates M3's own compaction work, on the roadmap for
the first time (RFC 0002 Discussion — post-approval amendments, below).**
RFC 0002 originally assigned itself no milestone ("cross-cutting... does not gate
any of M1–M5"), correct at the time since nothing beyond `commit`/`commit_deletion_
vector` existed to verify. Compaction is the reason that stops being true: it needs
its own new manifest commit shape (replacing a set of source segments with one
merged segment, atomically, under the same pointer CAS) — a **third** distinct
transition alongside `ProposeSnapshot`'s append and `ProposeDeletionVectorCommit`'s
revise-in-place. Piling a third unverified commit shape onto a protocol that still
has zero mechanized proof and zero cross-validation against the real Rust code is
exactly the kind of accumulating, unexamined risk this project's own verification
rigor discipline exists to catch before it compounds, not after. M3's compaction
work is therefore gated on: (1) a TLAPS mechanized proof of the TLA+ model as it
stands today (`commit` + `commit_deletion_vector`, RFC 0002's own remaining
artifact), and (2) the DST (Deterministic Simulation Testing) cross-validation
harness — Workflow II first per RFC 0002's own approved sequencing (TLC-generated
action sequences from the model, replayed against the real Rust code) — landing
*before* compaction's own commit-path design work starts, so compaction extends a
model already proven to correspond to the real code, not one more hopeful extension
stacked on an never-cross-validated base. Neither artifact is built yet
(`verification/README.md`).

**M4 — Interchange + independence.** CIFF importer (lossless where CIFF permits);
conformance manifest frozen at spec v0.1. **Second-reader parity must be real
independence**: an external contributor implementing from `conformance/` alone, or a
clean-room session given only `spec/` and `conformance/` with the Rust crates
withheld — this is also the acceptance test for invariant 11: two implementations,
same logical input, same index. If a stranger cannot implement from the conformance
manifest, the spec failed §2's test regardless of CI. Puffin blob-type packaging RFC.
The tantivy fork is the named primary second-reader path, built against the frozen
v0.1 conformance manifest, never against a moving spec; the clean-room option remains
the fallback and activates if any R11(d) failure trigger fires. Lucene `StrandCodec`
lands here as the JVM parity vehicle.

**R11(a), resolved 2026-08-19
(`references/r11a-tantivy-reader-surface-and-lucene-codec-spi.md`), confirms both
halves of this deliverable's shape ahead of time.** tantivy (verified at tag
`0.26.1`) has no codec SPI: `Directory` is a byte-range storage abstraction,
`SegmentComponent` is a closed seven-variant enum with one concrete reader/writer
compiled in per variant, and its `Postings` trait is a runtime query-result
iterator, not a wire-format registration point. "The tantivy fork" therefore means
literally forking `quickwit-oss/tantivy` and editing its internal reader/writer
modules (`src/index/segment_reader.rs`, `src/index/inverted_index_reader.rs`,
`src/index/segment_component.rs`, and the concrete `postings`/`termdict`/
`fastfield`/`store` modules) — never registering a plugin against a stable
extension point, because none exists. Lucene (verified at tag
`releases/lucene/10.5.1`) has the opposite shape: `Codec` declares eleven abstract
format methods resolved by name through `java.util.ServiceLoader` via
`META-INF/services/org.apache.lucene.codecs.Codec`. `StrandCodec` extends
`FilterCodec` (the documented delegation base class), overrides `postingsFormat()`
to return a `PostingsFormat` whose `FieldsProducer` reads STRAND's own postings
blob, delegates the other ten format methods to an existing codec
(`Lucene104Codec` is 10.5.1's current default), and registers its class name in
that services file — the documented, currently-shipping pattern Lucene's own
default codec uses, not a hypothesis.

**M5 — The consumer.** A thin, read-only **DataFusion TableProvider** over STRAND
segments — the answer to "name the second engine," written into scope on purpose
because the research concluded no forced reader exists and one must be built. Slices
of it should track earlier milestones (reading lexical blobs as M1 lands) so the spec
is stress-tested by a consumer while it can still change cheaply; M5 is where it
becomes a supported, benchmarked artifact. Without this milestone the project is
Indri with better licensing, and we have chosen not to be that in writing. The
TableProvider is additionally the hybrid-fusion benchmark host, running the `CLAUDE.md` §7 fusion
workload with its selectivity sweep. The FAISS adapter lands alongside M2's
benchmarks or here, per R11(b)'s feasibility finding.
