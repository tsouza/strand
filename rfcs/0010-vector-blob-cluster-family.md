# RFC 0010: Vector blob family — cluster-native cold-open index

- **Status:** Approved. Adversarial review re-fetched every reference file this
  RFC cited and cross-checked every arithmetic claim independently. Found 6
  Critical, 14 Important, and 7 Minor findings, all fixed. Critical: (1) the
  single load-bearing sizing formula (`BatchDataMap`'s per-batch byte cost) was
  cited to `references/rabitq-library-compact-code-source.md`, a file that in
  fact documents a *different*, non-batched, per-vector code layout — the real
  FastScan-batched source (`data_layout.hpp`) had been fetched live in-session
  but never vendored as its own file, and every citation pointed at the wrong
  one. Fixed by vendoring the real source properly
  (`references/rabitq-library-ivf-and-batch-layout-source.md`,
  `references/rabitq-library-index-overview.md`) and correcting every
  citation throughout this RFC, `spec/vectors.md`, and `docs/ledger.md` — the
  exact "sibling fetch, never vendored" failure `CLAUDE.md` §3 exists to
  prevent. (2) Five citations pointed at unvendored sources, including one
  literal unfilled placeholder (`references/...ivf.md`); fixed by the same
  vendoring pass. (3) The DiskANN citation justifying full-precision centroids
  was inverted — `docs/lineage.md`'s own text (which this RFC cited) says
  DiskANN quantizes *routing* structures and keeps full precision for
  reranking, the opposite of what this RFC claimed; fixed by citing SPANN
  instead (which genuinely does keep centroids full-precision) everywhere the
  claim appeared. (4) A real byte-determinism gap: a partially filled
  FastScan batch's unused lanes had no specified content, violating invariant
  11; fixed with a new normative zero-fill rule (Design §4, `spec/vectors.md`
  §4) and a new Invariant-11 checklist item. (5) The corrected sizing law used
  a full-batch-amortized *floor* (108/116 bytes/vector) as if it were a
  realistic average, missing the partial-batch padding waste Design §4 names
  two paragraphs earlier; recomputed at a realistic 250-vectors/cluster
  average and 4,000 clusters, real tier-1 cost is **~131 MB per million 768d
  vectors** (not the ~128.4 MB first draft, and not `docs/data-structures.md`'s
  pre-RFC ~100 MB), ~31% over the provisional 100 MB budget — corrected
  everywhere it appears, including `CLAUDE.md` §7's own "roughly one segment
  per million 768d vectors" line, updated to ~760,000. (6) The `nprobe`-only
  posting-list blob was labeled `tier: cold-fetchable` without the design
  actually fetching it wholesale, creating an incoherent budget comparison;
  fixed with an explicit Design §8 paragraph distinguishing bytes fetched at
  open (~12.4 MB) from the whole-corpus sizing figure (~131 MB). Important
  (14, summarized): invariant 9 was claimed but never engaged — fixed with a
  new Design §9; `MatrixRotator`'s unpadded dimensionality contradicted the
  code-region formula and left the row-id array potentially misaligned —
  fixed by requiring 64-multiple padding for both rotator types; no
  `alignment` field had been declared for any of the four blobs — fixed
  (`alignment = 8`); the round-trip count (5–6 stated) didn't reproduce from
  its own enumerated stages (6–7 actual) — fixed; the replication estimate
  applied a replica-2→8 ratio to a replica-1 baseline and overclaimed
  "measured" when its own source flags the figures as unverified — fixed by
  correcting the arithmetic, relabeling the estimate provisional throughout,
  and naming a real SPANN body-figure fetch as owed work; deletion-vector
  filtering and closure-replication deduplication were absent from query
  resolution — fixed with two new normative steps; u64 row-ids vs. cheaper
  u32 local ordinals had no stated rationale — fixed, citing
  `spec/row-ids.md` §3's rebalance-merge case directly; no prior-art
  paragraph existed despite `CLAUDE.md` §4's requirement, and Lance's
  row-ID-linked auxiliary quantized-vector file — the closest actual
  precedent — was never mentioned — fixed; the replication metadata slot's
  deferral undersold that it drops a named M2 milestone deliverable — fixed,
  stated plainly in Non-goals and `docs/milestones.md`; three files asserted
  RFC 0010 was already Approved while its own Status line read Draft —
  resolved by this line itself. Minor (7): the descriptor blob's total size
  was misstated in Napkin math (384 vs. the correct 400 bytes including its
  16-byte header); MB/MiB units were mixed within one paragraph — fixed to
  decimal MB throughout; the Invariant-11 checklist's "codec-variant
  provenance" and "golden files" items overclaimed completeness given the
  intra-batch bit-order gap and the worked example's opaque code payload —
  both reworded to state precisely what is and isn't pinned; "comfortably
  under" language understated a margin that is in fact over half consumed —
  reworded to state the ratio neutrally; the ledger's R3 entry still listed
  rotation-provenance as Open after this RFC resolved it — fixed in the same
  pass as this Approval; the reserved-byte reader rule was inconsistent
  between this RFC and `spec/vectors.md` — aligned; closure-replication
  duplicate row-ids across scanned clusters were unhandled in query
  resolution — fixed with the same dedup step noted above.
- **Milestone:** M2 — Vectors, cluster-first (`docs/milestones.md`)
- **Spec chapters produced:** `spec/vectors.md`; additively extends
  `spec/container.md` §9 (registers `family_id = 3` "vector", `blob_type_id = 0`
  "flat vectors", `blob_type_id = 1` "quantization descriptor", `blob_type_id = 2`
  "cluster navigation tier", `blob_type_id = 3` "cluster posting lists")
- **Invariants exercised:** 1, 3, 7, 8, 9, 10, 11 (`CLAUDE.md` §5)

## Summary

Registers STRAND's cold-native vector index: the cluster-shaped family
`docs/data-structures.md` already settles as R1's answer to cold ANN over
object storage — a navigation tier of full-precision centroids small enough
to fetch wholesale, posting lists of RaBitQ-quantized codes fetched in a
bounded, parallel wave keyed by nprobe, and a flat full-precision vector blob
touched only for reranking. Four new blob types under a new `family_id = 3`
("vector"): flat vectors, a quantization descriptor (dims, distance metric,
bit width, rotation provenance), the cluster navigation tier (centroids plus
a per-cluster directory), and the cluster posting lists themselves.

This RFC registers **1-bit RaBitQ only** for v0.1 — the bit-width Extended-
RaBitQ multi-bit path (`docs/data-structures.md`'s own "kernel selection
pinned by bit-width" principle) is real, separate, un-grounded-by-this-RFC
work, named precisely in Non-goals rather than glossed over. It resolves R3's
open rotation-provenance question (`docs/ledger.md`: "materialized matrix vs
generator+seed, M2 RFC") with **new primary-source grounding fetched in this
session** — the reference RaBitQ-Library's own source, not its abstract —
and finds, and states plainly rather than softening, that the previously
stated sizing law ("1-bit codes cost dims/8 bytes per vector") undercounts
the real cost by roughly a fifth once the codec's own per-vector correction
factors and STRAND's row-ids are counted, and undercounts further once
partial-batch padding waste and centroid-table overhead are added: real
tier-1 cost is ~131 MB per million 768d vectors, not ~100 MB, without
replication — and, on a provisional, explicitly unverified estimate, over
half of R1's 4× kill-criterion margin once realistic replication is
applied. Not close to falsifying the mission claim, but a real correction,
made honestly, not rounded away.

## Motivation

`docs/data-structures.md`'s "Vector index shape" paragraph and
`docs/research/README.md` R1 already settle the cluster-shaped conclusion —
turbopuffer's, SPANN's, and the 2026 survey/benchmark literature's convergent
finding that cluster indexes fit object storage's chunk-shaped access where
graph beam search's dependent chain of 50–200 fetches (5–20 seconds at the
~100ms round-trip figure) does not (`references/turbopuffer-architecture.md`,
`references/spann-neurips2021.md`). What was **not yet settled before this
RFC** — `docs/data-structures.md` marked it "blob design open, RFC
required" prior to this RFC's own edit to that entry — is the concrete wire
layout: how centroids, quantized codes, per-vector correction factors, and
row-ids actually sit in bytes such that a reader can resolve nprobe cluster
byte ranges from data already in hand, per invariant 3's one-wave rule,
without inventing the RaBitQ quantization math itself (invariant 8: "don't
invent encodings... novelty budget is spent on the container... not the
parts [that are] already standardized").

This RFC's own nearest prior art, named per `CLAUDE.md` §4: SPANN's
centroids-in-memory/posting-lists-on-disk split (`references/spann-
neurips2021.md`) for the two-tier navigation/posting-list shape; SPFresh's
LIRE incremental rebalancing (`references/spfresh-sosp2023.md`) for the
navigation tier's own merge strategy (Design §7); and Lance's row-ID-linked
auxiliary quantized-vector file (`references/lance-vector-index-format.md`
— "the auxiliary file maintains a `_rowid` column... alongside quantized
vector representations") for the exact codes-plus-row-ids pairing Design §4
registers, including the choice of storing real row-ids rather than a
reference-implementation-style internal ID (Alternatives considered).
turbopuffer's own published production evidence
(`references/turbopuffer-architecture.md`) is the standing benchmark
target, not an implementation source (`docs/lineage.md`).

RaBitQ itself is already settled (R3, `docs/data-structures.md`): Extended-
RaBitQ is the de-facto industry choice (Milvus, Faiss, Elasticsearch/Lucene's
"BBQ," turbopuffer, CockroachDB — confirmed again, directly, by this
session's own fetch of the reference library's own adopters list,
`references/rabitq-library-index-overview.md`), and both
reference-implementation repositories are Apache-2.0
(`references/rabitq-and-extended-rabitq.md`). What R3 left open for this RFC
specifically was the rotation-provenance mechanism — invariant 11 requires
"either the materialized transform or a normatively specified generator plus
seed" for every stochastic transform, and RaBitQ's random rotation is
exactly that. This session fetched the reference implementation's actual
rotator source (`include/rabitqlib/utils/rotator.hpp`,
`docs/docs/rabitq/rotator.md`) rather than trusting memory or the
abstract-only prior vendoring pass, per `CLAUDE.md` §3's standing rule — and
found a real, decisive answer: the library's own default rotator
(`FhtKacRotator`) serializes 4×padded-dims **bits** of realized random sign
state directly, not a seed, because its only randomness is those sign bits —
cheap enough (384 bytes at 768 padded dims) that there is no size pressure
to prefer a seed. Its alternative (`MatrixRotator`) would need a seed to be
worth it (D² floats is expensive to materialize), but its construction runs
a floating-point QR decomposition (`Eigen::HouseholderQR`) whose bit-exact
output is not guaranteed portable across BLAS/LAPACK implementations — a
real byte-determinism risk this RFC resolves by pinning the cheap,
determinism-safe default as STRAND's own default and registering the
expensive alternative as non-default with the risk stated, not hidden.

The corrected sizing-law finding (Napkin math, below) is the second real
motivation: `docs/data-structures.md`'s stated "~100 MB per million 768d
vectors" already carries the caveat "before navigation structure," but does
not quantify it, and does not count the quantized code's own per-vector
correction factors at all. This session fetched the reference
implementation's exact per-batch and per-cluster byte formulas
(`include/rabitqlib/quantization/data_layout.hpp`,
`references/rabitq-library-ivf-and-batch-layout-source.md`) and computed the
real number: navigation tier plus posting lists run closer to ~131 MB per
million 768d vectors before SPANN-style closure replication, not ~100 MB.
This doesn't change R1's conclusion — it is nowhere near the 4× kill
threshold — but per `CLAUDE.md` §2's own rule, a number this project depends on that
turns out to be a real undercount gets corrected here and in
`docs/data-structures.md`, not left to round favorably.

## Non-goals

- **Multi-bit Extended-RaBitQ** (bit widths 2–8). `docs/data-structures.md`
  already commits to a *different* kernel for this path (classical scalar-
  quantization distance computation, not FastScan LUT), and this session's
  grounding of the reference implementation's ex-code layout
  (`ExDataMap<T>`, `references/rabitq-library-ivf-and-batch-layout-source.md`)
  is real but was not carried through to a full wire-format registration here —
  narrowing scope to one coherent slice per `CLAUDE.md` §9, the same
  discipline RFC 0007 applied by deferring positions to RFC 0008. A named,
  real follow-on RFC (Open questions, below).
- **The graph-blob family** (DiskANN/Vamana warm tier). Explicitly second
  per the M2 milestone text (`docs/milestones.md`) and per
  `docs/data-structures.md`'s own "explicitly not the cold-open story"
  framing. Its own open question — the node-order permutation algorithm,
  Starling vs. an untested alternative — is R1's second half, un-touched
  here.
- **SPANN-style closure replication's actual construction algorithm and its
  metadata slot.** `docs/milestones.md`'s M2 entry names "the replication
  knob and tier-1 sizing limits in blob metadata" as a stated deliverable;
  this RFC does not deliver it — the metadata *slot* for a replication
  factor is not added to any blob this RFC registers. Stated plainly, not
  glossed as an unremarkable deferral: this RFC's own Napkin math section
  computes replication as the single largest cost lever in the whole sizing
  picture (≈2.3× the cold-open budget at a realistic replica-8-equivalent
  density, on an unverified estimate — Napkin math), which makes deferring
  the knob that would make that cost visible and tunable the most
  consequential Non-goal in this RFC, not the least. M2's own milestone gate
  is therefore not fully met by this RFC alone (Open questions, below); a
  follow-on RFC that also does construction-side clustering owns the
  metadata slot and the construction algorithm together.
- **Cross-segment codebook sharing and retraining at merge/compaction time.**
  Named precisely in Design §7 (merge semantics) as a real, load-bearing,
  unresolved question — not silently assumed away.
- **Centroid count / clustering algorithm.** This RFC's worked example and
  napkin math use the `4·√N` convention the reference library's own IVF
  documentation recommends, citing Faiss
  (`references/rabitq-library-ivf-and-batch-layout-source.md`, see Design
  §3), but the actual clustering algorithm (k-means or otherwise) that
  produces centroids at segment-build time is construction-side, out of
  scope for a read-side wire-format RFC.
- **ARM/SIMD kernel validation for the FastScan decode path.** Out of scope
  for the same reason RFC 0007 left it open for postings — real, separate,
  measured work. (The intra-batch bit/lane order itself — a narrower,
  load-bearing gap at Approval time — is now resolved: Discussion, below,
  and `spec/vectors.md` §4 pin the complete `pack_codes` algorithm
  normatively.)
- **How a reader obtains and applies the deletion vector's own bytes.**
  Design §6 now requires (step 4) that a reader filter the quantized-scan
  candidate set against the segment's deletion vector before reranking —
  but this RFC does not say where that blob's own bytes come from at query
  time (whether it is part of the cold-open wave, fetched lazily, or
  assumed already resident), since the deletion-vector blob itself belongs
  to invariant 2's general machinery, not this family. Real, separate,
  unresolved work if it turns out this family needs its own fetch-timing
  answer rather than inheriting whatever the general deletion-vector
  read path already does.

## Design

### 1. Blob registration

Four new blob types under a new `family_id = 3` ("vector"), added to
`spec/container.md` §9's registry, using the same 34-byte `blob_entry`
struct every prior blob type uses unmodified (`storage_class`, `tier`,
`alignment`, `chunk_codec` fields already generic — this RFC needs no new
container-level fields):

| `family_id` | `blob_type_id` | blob type                  | `storage_class`  | `tier`          | `alignment` |
| ----------- | -------------- | --------------------------- | ----------------- | --------------- | ----------- |
| 3           | 0              | flat vectors                 | raw-mappable       | n/a (rerank only, invariant 7) | 8 |
| 3           | 1              | quantization descriptor      | raw-mappable       | cold-fetchable  | 8 |
| 3           | 2              | cluster navigation tier      | raw-mappable       | cold-fetchable  | 8 |
| 3           | 3              | cluster posting lists        | raw-mappable       | cold-fetchable  | 8 |

All four are `storage-class: raw-mappable` (invariant 10) — none of these
blobs benefit from a chunk-compression wrapper: the flat-vector blob is
dense floats a reranker mmaps directly, and the other three are exactly the
fixed-width, offset-addressed structures raw-mappable storage exists for.
`tier: n/a` on the flat-vector blob is invariant 7's own literal rule ("Raw
full-precision vectors are one blob, fetched only for reranking") — it is
never part of the cold-open wave, and its bytes do not count against the
100 MB cold-open byte budget this RFC's Napkin math section computes
against. `alignment = 8` on all four (`spec/container.md` §5's required
field for every raw-mappable blob) covers the widest scalar any of these
layouts stores (f32 centroids and codes, u64 row-ids and directory
offsets); Design §2 restricts `padded_dims` to always be a multiple of 64
for both registered rotator types specifically so every offset this family
computes — including each cluster's row-id array start, `region_offset +
code_bytes_length` — is automatically a multiple of 8, with no separate
padding rule needed.

A field registers exactly one navigation-tier blob and exactly one
posting-list blob per vector column (matching the term-dictionary/postings
pairing convention RFC 0005/0007 already established), plus one
quantization-descriptor blob shared by both, and, if reranking is enabled
for that column, one flat-vector blob.

### 2. Quantization descriptor (`blob_type_id = 1`)

Fixed 16-byte header, plus a variable-length trailing rotation payload.

| offset | size | field                   | notes                                                              |
| ------ | ---- | ------------------------ | ------------------------------------------------------------------- |
| 0      | 4    | `dims`                   | u32; true (unpadded) vector dimensionality                          |
| 4      | 4    | `padded_dims`            | u32; `dims` rounded up per the registered rotator's own padding rule |
| 8      | 1    | `distance_metric`        | u8: `0` = L2 (Euclidean), `1` = inner product, `2` = cosine          |
| 9      | 1    | `bit_width`              | u8; MUST be `1` in this RFC (Non-goals)                              |
| 10     | 1    | `rotator_type`           | u8: `0` = `MatrixRotator`, `1` = `FhtKacRotator` (**default**)       |
| 11     | 1    | reserved                 | writer MUST set zero; reader MUST NOT reject nonzero (future format versions) but MUST NOT interpret it |
| 12     | 4    | `rotation_payload_length`| u32                                                                  |
| 16     | `rotation_payload_length` | `rotation_payload` | bytes; see below                                     |

`rotator_type = 1` (`FhtKacRotator`, the reference library's own default —
`references/rabitq-library-rotator-source.md`): `padded_dims` is `dims`
rounded up to the nearest multiple of 64, and `rotation_payload` is
`4 * padded_dims / 8` bytes — the four Rademacher sign sequences the
reference implementation's own `flip_` buffer stores, copied verbatim, in
the same order that buffer's own `save()`/`load()` pair uses (a flat byte
array, no further structure). This is STRAND's **registered default**: the
payload is genuinely tiny (384 bytes at 768 padded dims) and every byte is
integer sign state with no floating-point derivation step, so a conforming
reader that copies these bytes verbatim reproduces the identical rotation
bit-for-bit on any platform, satisfying invariant 11 without needing to pin
a cross-language PRNG.

`rotator_type = 0` (`MatrixRotator`): STRAND deliberately deviates from the
reference implementation's own `padding_requirement()` (which returns `dim`
unchanged, i.e. no padding, for this rotator type) and instead **requires
`padded_dims` to be `dims` rounded up to the nearest multiple of 64, the
same rule `FhtKacRotator` uses** — a writer using this rotator type MUST
apply that padding before rotating, even though the reference library's own
`MatrixRotator` class does not. This is a real, named STRAND-specific
constraint, not a description of the reference implementation's own
behavior: without it, the two registered rotator types would feed different
`padded_dims` values into Design §4's single code-region formula, and a
`dims` not already a multiple of 8 would misalign the row-id array within
the cluster posting-list blob (§1's `alignment = 8` requirement). Applying
one shared padding rule to both types closes that gap entirely rather than
patching it with a second, rotator-specific alignment case. `rotation_payload`
is `dims * padded_dims * 4` bytes — the realized `dims × padded_dims`
row-major float32 orthogonal matrix, copied verbatim from the reference
implementation's own `rand_mat_` buffer (computed against STRAND's own
padded dimensionality, not the reference implementation's unpadded one).
This RFC registers `MatrixRotator` as a valid,
conforming choice (the reference library ships it, and some deployments may
already have trained indexes using it) but **not the default**, and names
the reason precisely rather than merely disfavoring it by convention: the
matrix is the output of `Eigen::HouseholderQR` applied to a random Gaussian
matrix, and QR decomposition's bit-exact output is not portable across
BLAS/LAPACK implementations or optimization levels
(`references/rabitq-library-rotator-source.md`). Because this RFC requires
the *realized* matrix to be serialized (never a seed, for either rotator
type — the reference implementation itself never serializes a seed), this
determinism risk is fully contained to the *writer* that first trains a
`MatrixRotator` index: once serialized, every conforming reader reproduces
identical rotation from the stored bytes regardless of platform, the same
guarantee `FhtKacRotator` gives, just paid for in `D²` bytes instead of `4D`
bits. The risk this RFC actually flags is size and construction cost, not
cross-reader determinism — worth stating precisely rather than
overstating.

A reader MUST reject a quantization-descriptor blob whose
`rotation_payload_length` does not equal the value the applicable formula
above computes from `dims`, `padded_dims`, and `rotator_type` — a mismatch
means the descriptor is either corrupt or was written against a rotator
variant this chapter does not register.

### 3. Cluster navigation tier (`blob_type_id = 2`)

The wholesale-fetched routing structure — small enough to be part of the
cold-open wave in full, per invariant 7's parenthetical definition of
`cold-fetchable`.

| offset | size | field                          | notes |
| ------ | ---- | -------------------------------- | ----- |
| 0      | 4    | `num_clusters`                    | u32 |
| 4      | 4    | reserved                          | u32; writer MUST set zero (8-byte-aligns what follows); reader MUST NOT reject nonzero but MUST NOT interpret it |
| 8      | `num_clusters * padded_dims * 4`  | `centroid_table` | row-major f32, one row per cluster, in cluster-index order, **at full precision** — SPANN keeps its centroids-in-memory routing structure full-precision, quantizing only the on-disk posting-list payload (`references/spann-neurips2021.md`); this RFC follows that split rather than DiskANN's, whose own two-tier model quantizes the *routing* structure itself ("compressed codes for routing... full precision only for reranking," `docs/lineage.md`) — the opposite split, not a precedent for this choice (Alternatives considered discusses quantized centroids directly rather than leaning on a citation that argues the other way) |
| —      | `num_clusters * 24` | `cluster_dir`      | one 24-byte entry per cluster, in the same cluster-index order as `centroid_table`; see below |

`cluster_dir` entry (24 bytes):

| offset | size | field               | notes                                                                 |
| ------ | ---- | --------------------- | ----------------------------------------------------------------------- |
| 0      | 8    | `region_offset`        | u64; byte offset into the posting-list blob (`blob_type_id = 3`) where this cluster's data begins |
| 8      | 8    | `code_bytes_length`     | u64; byte length of this cluster's quantized-code region; the row-id array begins immediately after, at `region_offset + code_bytes_length` |
| 16     | 4    | `vector_count`          | u32; number of vectors in this cluster (the row-id array's length is exactly `vector_count * 8` bytes) |
| 20     | 4    | reserved                | writer MUST set zero; reader MUST NOT reject nonzero but MUST NOT interpret it |

Centroids and the directory are co-located in cluster-index order
deliberately (not centroids-then-directory-in-arbitrary-order): a reader
comparing a query against every centroid to select its nprobe closest
clusters can walk both tables index-aligned in one pass, and `cluster_dir`
directly gives the byte range invariant 3 requires be resolvable with no
further round trip — the entire posting-list blob's per-cluster addressing
comes from this one wholesale fetch.

### 4. Cluster posting lists (`blob_type_id = 3`)

One contiguous blob, laid out as the concatenation of each cluster's own
region, in the same cluster-index order `cluster_dir` uses. Each cluster's
region is itself two parts back to back: the quantized-code region, then
that cluster's row-id array.

This deliberately differs from the reference implementation's own in-memory
layout, which groups all clusters' code batches together, then all
clusters' ex-data, then all ids, then the directory
(`references/rabitq-library-ivf-and-batch-layout-source.md`:
`[batch data][ex_data][ids][cluster_lst]` as four
flat top-level regions) — a layout that optimizes for "the whole index is
resident in RAM," where any region can be indexed independently. STRAND's
one-wave cold-fetch model optimizes for the opposite case: a reader wants
**one** contiguous Range GET per selected cluster, not two. Co-locating a
cluster's codes immediately before its own row-ids means `cluster_dir`'s
single `[region_offset, region_offset + region_length)` range (where
`region_length` is not stored directly but is always
`code_bytes_length + vector_count * 8`, both already in the directory
entry) covers everything nprobe needs for that cluster in one GET. This is
exactly `docs/data-structures.md`'s own stated principle for the
chunk/block split — "layouts SHOULD co-locate data accessed together" —
applied to a case that principle already anticipates but a naive port of
the reference library's in-memory layout would have missed.

**Quantized-code region**, per cluster: `ceil(vector_count / 32)` batches,
each exactly `BatchDataMap::data_bytes(padded_dims)` bytes —
`padded_dims * 32 / 8 + 4 * 32 * 3` bytes, i.e. `padded_dims * 4 + 384`
bytes per batch — the byte *offsets* registered verbatim from the reference
implementation's own struct layout
(`references/rabitq-library-ivf-and-batch-layout-source.md`, `data_layout.hpp`),
not re-derived: within a batch, the
first `padded_dims * 32 / 8` bytes are the 1-bit codes for up to 32 vectors
(FastScan-batched), followed by three arrays of 32 little-endian f32 values each
(`f_add`, `f_rescale`, `f_error` — the codec's own per-vector distance-
correction factors; their role in RaBitQ's distance estimator is the
algorithm's concern, not this container-layer RFC's). The exact intra-batch
bit/lane order **within** the 1-bit code sub-region is now fully resolved
(Discussion — post-approval amendment, below, closing the gap this RFC
originally left open at Approval): `spec/vectors.md` §4 pins the complete
`fastscan::pack_codes` nibble-shuffle algorithm normatively, adopted
verbatim from the reference implementation (invariant 8), not
re-derived. A cluster whose
`vector_count` is not a multiple of 32 still pays the full per-batch cost
for its last, partially-filled batch — this is the registered codec's own
behavior (`total_blocks * BatchDataMap::data_bytes`, independent of the
last batch's real occupancy), inherited rather than fixed here. This RFC
**also** pins the partial batch's unused lanes normatively, since
invariant 11 requires byte determinism — the fetch that closed the
intra-batch bit-order gap (Discussion, below) confirms this rule matches
the reference implementation's own behavior exactly: `pack_codes`'s own
`get_column` helper zero-fills any batch slot beyond the real vector count
before packing, so this is not a STRAND-invented convention layered on top
of unspecified reference behavior, it is the reference behavior, stated
normatively: **a writer MUST zero-fill every unused lane's code bits and
all three corresponding `f_add`/
`f_rescale`/`f_error` slots in a partially filled batch.** Named honestly
in How this could be wrong as real, inherited padding waste, not fixed the
way RFC 0007 fixed the analogous problem for lexical postings.

**Row-id array**, per cluster: exactly `vector_count` little-endian u64
values, in ascending row-id order (STRAND's own row-id contract, invariant
1 — the reference implementation's internal `PID` type and ordering
convention are not adopted here; row-ids are STRAND's, not borrowed). Real
row-ids (8 bytes), not 4-byte local ordinals, deliberately (Alternatives
considered): `spec/row-ids.md` §3 names exactly this family's merge case —
"Rebalance (centroid layers). Row-IDs move between clusters as centroids
shift under drift, but the row-ID values themselves are unchanged — only
which posting list currently contains a given row-ID changes" — a merge
under `rebalance` moves entries between clusters, so a local ordinal
(meaningful only relative to a single segment's own dense array) would need
remapping on every rebalance, exactly the cost stable row-IDs exist to
avoid (`spec/row-ids.md` §3's "concatenate + remap" description: "the
row-ID values the entries reference are copied through unchanged").

### 5. Flat vectors (`blob_type_id = 0`)

`row_id_count * dims * 4` bytes (note: `dims`, **not** `padded_dims` — the
rotation padding is an artifact of the rotation transform, not the true
data, and a reranker needs the true vector), row-major f32, one row per
local ordinal in the segment's row-id order, dense (every local ordinal
present, matching how every other per-row blob in this format works).
`tier: n/a` (invariant 7): this blob is never fetched at cold-open, only
after the cluster-posting-list scan has produced a candidate row-id set
small enough to rerank.

### 6. Query resolution

1. From the navigation tier (already in hand from the cold-open wave):
   compute the query's distance to every centroid (a cheap, purely local
   computation over `num_clusters * padded_dims` floats already fetched —
   no I/O), select the `nprobe` closest clusters.
2. For each selected cluster, read its `cluster_dir` entry and issue one
   Range GET for `[region_offset, region_offset + code_bytes_length +
   vector_count * 8)`. All `nprobe` ranges are known without any further
   round trip (invariant 3's one-wave rule) and are issued as one parallel
   wave — wall time of one round trip, request count of `nprobe`, both
   reported per `docs/data-structures.md`'s own stated accounting
   convention.
3. Decode each fetched cluster's quantized codes against the query (via the
   registered codec's own FastScan or classical distance estimator,
   depending on `bit_width` — 1-bit only in this RFC), producing a
   candidate row-id set ranked by estimated distance. Under SPANN-style
   closure replication (Napkin math; metadata slot not yet designed,
   Non-goals), the same row-id can legitimately appear in more than one of
   the `nprobe` scanned clusters — a reader MUST deduplicate by row-id
   before ranking, keeping each row-id's best (closest) estimated distance
   across the clusters it appeared in.
4. Filter the deduplicated candidate set against the segment's deletion
   vector (invariant 2), discarding any tombstoned row-id, before either
   returning results or reranking.
5. Optionally, rerank: fetch the flat-vector blob's rows for the surviving
   candidate row-ids (a second wave, outside the cold-open budget per
   invariant 7) and recompute exact distances.

### 7. Merge semantics (invariant 1)

Each blob type in this family declares its own merge strategy, per
invariant 1's requirement that every blob family state this honestly rather
than gloss it as "compaction":

- **Flat vectors:** `concatenate + remap` — a dense, row-id-ordered array,
  merged the same way any per-row blob is: local ordinals are remapped to
  the merged segment's new ordinal space, values copied verbatim.
- **Quantization descriptor:** segment-scoped, effectively a constant.
  Merging two segments' posting lists without requantization requires their
  quantization descriptors to be **byte-identical** (same `dims`,
  `distance_metric`, `bit_width`, `rotator_type`, and `rotation_payload`) —
  if they differ, the codes are not comparable, and a naive byte
  concatenation would silently corrupt distance estimates. This RFC states
  this constraint precisely and leaves its resolution open (Design's own
  Non-goals; Open questions, below): whether STRAND requires all segments
  in a table to share one codebook by convention, or defines an explicit
  requantization path for compaction, is real, separate, unresolved work.
- **Cluster navigation tier (centroids):** `rebalance`, per invariant 1's
  own naming ("centroid layers, LIRE-style"). SPFresh's LIRE algorithm
  (`references/spfresh-sosp2023.md`) is the named literature mechanism —
  incremental centroid rebalancing as data changes, rather than either a
  naive re-cluster-from-scratch or a naive concatenation of two old centroid
  sets. The algorithm itself is real, separate, unimplemented work (Non-
  goals); this RFC settles only the merge-strategy *label*.
- **Cluster posting lists:** `concatenate + remap` when constituent
  segments share a byte-identical quantization descriptor **and** their
  cluster assignments are compatible with the merged navigation tier's
  rebalanced centroids — which is not automatic: if rebalancing moves a
  vector to a different cluster, its code must move with it, and the
  per-cluster contiguous layout (Design §4) does not compose under a plain
  byte-level concatenation the way lexical postings' `concatenate + remap`
  does. Where descriptors differ, or a full recluster is chosen, the
  strategy degrades to `rebuild` (full requantization). Both paths are
  named honestly here rather than asserting the easy case (identical
  descriptor, no reassignment) is the only one that matters.

### 8. Tier and storage-class summary (invariants 7, 10)

All four blob types are `storage-class: raw-mappable` — dense wire bytes,
no chunk-compression wrapper, decompression-free reads via mmap or direct
range GET, matching invariant 10's guidance that SIMD alignment is the
reader's arena-decompression concern, not a wire-byte concern, and doubly
so here since these blobs are never compressed in the first place.
`blob_type_id ∈ {1, 2, 3}` are `tier: cold-fetchable`; `blob_type_id = 0`
(flat vectors) is `tier: n/a`, per invariant 7's literal text.

`cold-fetchable` covers two genuinely different access patterns here, both
consistent with invariant 3's underlying requirement (every byte range a
cold query may need is resolvable from data already in hand, with no
pointer-chasing round trip) even though only one of them is literally
"fetched wholesale." The quantization descriptor and navigation tier
(`blob_type_id ∈ {1, 2}`) genuinely are fetched wholesale, in full, at open
— invariant 7's own parenthetical definition of the term. The
cluster-posting-list blob (`blob_type_id = 3`) is not: a query fetches only
its `nprobe` selected clusters' byte ranges, typically a small fraction of
the whole blob. Both satisfy invariant 3 (every range needed is resolvable
without a further round trip, from the navigation tier already fetched
wholesale) but only the first is "wholesale" in the literal sense. This
distinction matters for the Napkin math section's own two separate figures:
bytes *actually fetched* at open (descriptor plus navigation tier, ~12.4 MB
at realistic scale) versus the *whole quantized corpus'* size (~131 MB),
which is the relevant quantity for the cold-open byte *budget* — a segment
so large that even its selectively-fetched posting-list blob cannot fit on
one storage tier is the actual constraint the 100 MB figure polices, not a
claim that every query reads all of it.

### 9. Kernel normativity and batch-shaped reads (invariant 9)

The scalar decode/distance-estimation kernel for this family's registered
codec (1-bit RaBitQ, FastScan) is normative — it defines the bit-exact
result any SIMD kernel MUST reproduce, per invariant 9's general rule,
applied here exactly as RFC 0007 applies it to postings decode. A reader
implementation's per-cluster decode-and-score step is naturally
batch-shaped already: Design §6 step 2 fetches whole clusters (each a
multiple of the registered 32-vector `kBatchSize`, save the last, padded
per Design §4's zero-fill rule), and a conforming reader's own internal
interface over a fetched cluster's codes SHOULD expose a `next_batch()`-
shaped iteration over its 32-vector batches, matching invariant 9's stated
API shape, with the batch size in this case fixed by the registered codec
(32) rather than a reader-side tuning parameter the way lexical postings'
batch size is. SIMD paths for the FastScan decode step MUST pass
property-based equivalence tests against the scalar reference, per
invariant 9 generally; no such kernel exists yet (Non-goals: ARM/SIMD
validation is real, separate, un-benchmarked work).

## Worked example

A tiny, real, hand-checkable index: `dims = 64` (chosen so `padded_dims =
dims` under the default `FhtKacRotator`, since 64 is already a multiple of
64 — this avoids a separate padding-arithmetic digression without picking
an unrealistic dimensionality), `distance_metric = 0` (L2), `bit_width = 1`,
`rotator_type = 1` (default). Two clusters: cluster 0 has 3 vectors (row-ids
`100, 101, 102`), cluster 1 has 2 vectors (row-ids `200, 201`) — both well
under the 32-vector batch size, so each cluster is exactly one (partially
filled) batch, exercising the padding-waste behavior Design §4 names.

**Quantization descriptor blob**, 16-byte header + 32-byte payload = 48
bytes total:

`dims = 64` → `40 00 00 00`. `padded_dims = 64` → `40 00 00 00`.
`distance_metric = 0` → `00`. `bit_width = 1` → `01`. `rotator_type = 1`
→ `01`. reserved → `00`. `rotation_payload_length = 4*64/8 = 32` →
`20 00 00 00`. Header: `40 00 00 00 40 00 00 00 00 01 01 00 20 00 00 00`
(16 bytes).

`rotation_payload` (32 bytes, illustrative — a real writer draws these from
a real RNG; this worked example uses a simple, exactly reproducible
sequence so the byte layout itself, not the randomness, is what a reader
checks by hand): the sequence `0x00, 0x01, 0x02, ..., 0x1F` (32 bytes,
ascending).

Full blob (48 bytes):
`40 00 00 00 40 00 00 00 00 01 01 00 20 00 00 00 00 01 02 03 04 05 06 07 08
09 0A 0B 0C 0D 0E 0F 10 11 12 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F`.

**Cluster navigation tier blob**: `num_clusters = 2` → `02 00 00 00`,
reserved → `00 00 00 00`. `centroid_table`: 2 rows of 64 f32 each. For
hand-checkability this worked example uses constant-valued illustrative
centroids rather than 128 distinct numbers: centroid 0 is `1.0f` repeated
64 times (`00 00 80 3F` repeated 64 times, 256 bytes), centroid 1 is
`-1.0f` repeated 64 times (`00 00 80 BF` repeated 64 times, 256 bytes) —
real IEEE-754 little-endian encodings, mechanically checkable, standing in
for a real writer's real k-means output. `cluster_dir`: cluster 0 —
`region_offset = 0` (`00×8`), `code_bytes_length = BatchDataMap::data_bytes(64)
= 64*4 + 384 = 640` (`80 02 00 00 00 00 00 00`), `vector_count = 3`
(`03 00 00 00`), reserved `00 00 00 00`; cluster 1 — `region_offset =
640 + 3*8 = 664` (`98 02 00 00 00 00 00 00`), `code_bytes_length = 640`
(same, since cluster 1's 2 vectors also occupy exactly one full-cost
batch), `vector_count = 2` (`02 00 00 00`), reserved `00 00 00 00`. Total
navigation-tier blob: `4 + 4 + 512 + 48 = 568` bytes.

**Cluster posting-list blob**, `1320` bytes total (`640 + 24` for cluster 0
plus `640 + 16` for cluster 1): the `640`-byte quantized-code region per
cluster is opaque payload produced by the registered FastScan-batched 1-bit
RaBitQ encoder — this RFC does not hand-derive real RaBitQ-encoded bytes
(Design §4 registers the code region's byte offsets, not the quantization
arithmetic that fills it or the intra-batch bit order, both real, named
gaps — How this could be wrong; a real conformance vector requires running
the actual reference kernel or STRAND's own encoder once implemented, per
this RFC's own Non-goals). Both clusters here use only 3 and 2 of their
one batch's 32 lanes respectively — a real conforming writer zero-fills the
remaining 29 and 30 lanes' code bits and all three factor arrays' unused
slots (Design §4's padding-determinism rule), which this worked example
does not spell out byte-for-byte only because the code region itself is
already marked opaque above. Row-ids are real and hand-checkable: cluster 0's row-id array (at
blob offset `640`, 24 bytes) is `100, 101, 102` as little-endian u64 —
`64 00 00 00 00 00 00 00 65 00 00 00 00 00 00 00 66 00 00 00 00 00 00 00`;
cluster 1's row-id array (at blob offset `640 + 24 + 640 = 1304`, 16 bytes)
is `200, 201` — `C8 00 00 00 00 00 00 00 C9 00 00 00 00 00 00 00`.

Resolving a query with `nprobe = 2` (both clusters, for this tiny example):
the reader already has the navigation tier from the cold-open wave, computes
distance to both centroids (no I/O), selects both, and issues two parallel
Range GETs — `[0, 664)` for cluster 0 and `[664, 1320)` for cluster 1 —
against the posting-list blob's absolute byte range (the blob's own
`offset` from its `blob_entry`, plus each `region_offset`). Both ranges are
fully determined by the navigation tier already in hand; no further round
trip is needed, confirming invariant 3's one-wave rule holds for this
layout.

## Napkin math (`CLAUDE.md` §7)

All byte figures below use decimal MB (10⁶ bytes), matching `CLAUDE.md`
§7's own usage of the 100 MB budget figure — no MiB/MB mixing.

**End-to-end round trips**, from the pointer read: (1) pointer GET, (2)
snapshot metadata GET, (3) segment open — ≤2 RTT per invariant 3, typically
1 when the hotcache fits the speculative tail window — (4) the cold-open
wave itself: the quantization descriptor and navigation-tier blobs, both
`tier: cold-fetchable`, fetched wholesale; at realistic scale (below) these
total ~12.4 MB (not "single-digit megabytes" — see the open-time-bytes
figure below), too large to fold into a hotcache-adjacent speculative-tail
read the way smaller blob types' worked examples do, so this arithmetic
counts it as its own round trip; (5) the cluster wave — `nprobe` parallel
Range GETs into the posting-list blob, wall time of one round trip
regardless of `nprobe`'s value, per Design §6; (6) optionally, rerank — one
further round trip against the flat-vector blob, explicitly outside the
cold-open budget (invariant 7). Counting all six stages: a genuinely cold
query with reranking costs **6–7 round trips** end to end (6 at the typical
1-RTT open, 7 at the conservative 2-RTT open), roughly **600–700ms** at the
`references/turbopuffer-architecture.md` planning figure of ~100ms/RTT — a
query that skips reranking costs one fewer, **5–6**. Both are the same
order of magnitude as turbopuffer's own measured 874ms true-cold p50
(`docs/benchmarks.md`), though not a claim of parity: STRAND's accounting
here starts from the pointer read (`CLAUDE.md` §7's own rule that "the
comparison engine's '3–4 roundtrips' includes its metadata trip"), and no
STRAND implementation of this design has been built or measured yet — this
is round-trip-count arithmetic, not a benchmark result.

**The corrected sizing law.** `docs/data-structures.md` currently states
"1-bit codes cost dims/8 bytes per vector... before navigation structure."
Using this RFC's own grounded formula (Design §4): at `padded_dims = 768`
(a realistic embedding width), one full 32-vector batch costs
`768 * 4 + 384 = 3,456` bytes, i.e. **108 bytes/vector** amortized over a
**full** batch — a floor, not an average; see below for the effect of
partial-batch waste at a realistic cluster size — not `768/8 = 96`
bytes/vector. The gap is the codec's own per-vector correction factors
(`f_add`, `f_rescale`, `f_error` — three f32 values, 12 bytes/vector),
which the previously stated sizing law did not count at all. Adding
STRAND's own row-id array (8 bytes/vector, Design §4): **116 bytes/vector**
at the full-batch floor for codes + factors + row-ids, a real 21% over the
previously stated 96 bytes/vector figure.

At 1,000,000 768d vectors, `4·√N ≈ 4,000` clusters (Design §3), the
realistic figure accounts for partial-batch waste rather than the floor:
250 vectors/cluster on average, `ceil(250/32) = 8` batches/cluster,
`8 * 3,456 = 27,648` code+factor bytes/cluster plus `250 * 8 = 2,000`
row-id bytes/cluster — `29,648` bytes/cluster, `× 4,000 = 118,592,000`
bytes for posting lists (**≈118.6 MB**, ~2.3% over the 116 B/vector floor's
116,000,000-byte estimate, from partial-batch padding). Adding the
navigation tier: `centroid_table` costs `4,000 * 768 * 4 = 12,288,000`
bytes; `cluster_dir` costs `4,000 * 24 = 96,000` bytes; the quantization
descriptor costs `16 + 384 = 400` bytes (a 16-byte header plus the 384-byte
`FhtKacRotator` payload, Design §2 — negligible, but not the 384 bytes an
earlier draft of this section stated). **Total tier-1 (navigation tier +
posting lists), no replication: `118,592,000 + 12,288,000 + 96,000 + 400 =
130,976,400` ≈ 131.0 MB per million 768d vectors** — a real ~31% over the
`CLAUDE.md` §7 provisional 100 MB cold-open byte budget, and a real,
uncomfortable-but-honest correction to `docs/data-structures.md`'s own
stated sizing law, made here rather than left standing. This is nowhere
near R1's falsifying kill criterion (4× the budget), so it does not narrow
the mission claim.

This 131.0 MB figure is the whole quantized corpus per segment, not what a
single query fetches at open — see Design §8's `cold-fetchable` distinction.
**Bytes actually fetched wholesale at open** (descriptor + navigation
tier only): `400 + 8 (num_clusters/reserved header) + 12,288,000 +
96,000 = 12,384,408` ≈ **12.4 MB** — comfortably under the 100 MB budget on
its own. The 131.0 MB figure is the segment-sizing quantity R1's own
still-open sizing-law work needs (`CLAUDE.md` §7's "segment count is
reported, never hidden" rule): at the corrected law, a segment that stays
within the 100 MB tier-1 budget holds roughly `100 / 131.0 ≈ 0.76` million
768d vectors — **~760,000 vectors/segment**, not the ~1,000,000
`CLAUDE.md` §7 previously stated — updated in place alongside this RFC's
Approval.

**Replication's cost — a provisional, explicitly unverified estimate, not a
measured one.** `docs/data-structures.md` names SPANN-style closure
replication (up to 8×) as a first-class knob. `docs/research/README.md` R1
cites SPANN's GIST1M benchmark: 13.0 GB at replica 8 vs. 7.5 GB at replica
2 — a **1.73×** growth from replica 2 to replica 8. This RFC's own 131.0 MB
baseline carries **no** replication (replica 1, not replica 2), so the
1.73× ratio is not directly applicable — it omits whatever the 1→2 step
itself costs, and applying it anyway is a deliberately conservative
**lower bound**, not a replica-8 prediction: `130,976,400 * 1.73 ≈
226,589,172` ≈ **≈227 MB**, ≈2.27× the 100 MB budget — over half the
margin to R1's 4× kill criterion, not a wide one, and likely an
*underestimate* of the true replica-8 cost since it skips the 1→2 step.
Critically, `references/spann-neurips2021.md` itself states its 13.0/7.5 GB
figures were read from an abstract-only fetch and explicitly "were not
re-confirmed here... and should be checked against the PDF body directly
before being treated as independently vendored" — this session did not
re-fetch SPANN's PDF body, so the ≈227 MB figure is provisional, built on a
ratio this project's own vendoring notes flag as unverified, and Open
questions names fetching SPANN's real replica-1-through-8 table as owed
work before this figure can be trusted for R1's actual sizing-law
decision.

## Invariant-11 checklist

- **Endianness:** little-endian throughout — every multi-byte field in the
  quantization descriptor, the navigation tier's `centroid_table` (f32) and
  `cluster_dir` (u64/u32), and the posting-list blob's row-id arrays (u64).
- **Term sort order:** not applicable — this family has no term dictionary.
- **Chunk codec:** not applicable — all four blob types are
  `storage-class: raw-mappable`, no chunk wrapper.
- **Checksums:** covered by each blob's own registry entry
  (`spec/container.md` §5, §6); no new checksum scope introduced here.
- **Codec-variant provenance:** this RFC's own precise registration — RaBitQ
  1-bit, `FhtKacRotator` (default) or `MatrixRotator` (registered,
  non-default) rotation, the reference library's own FastScan-batched
  code-region byte offsets and now the complete intra-batch bit/lane order
  (`references/rabitq-library-ivf-and-batch-layout-source.md`,
  `references/rabitq-library-fastscan-pack-codes-source.md`), cited to the
  reference implementation's actual source and independently verified by
  executing it (Discussion), not re-derived from memory alone, per
  `CLAUDE.md` §3. Complete as of the Discussion amendment — a real gap at
  Approval time, closed before implementation began.
- **Padding determinism:** a partially filled final batch's unused lanes
  (both the code bits and the three factor arrays) MUST be zero-filled
  (Design §4) — not left to writer discretion, which invariant 11 would
  otherwise leave unresolved for this codec.
- **Stochastic-transform provenance:** the load-bearing case invariant 11
  names explicitly. Resolved in Design §2: materialized rotation state for
  both registered rotator types (never a seed), with the size/determinism
  trade-off between them stated precisely rather than asserted safe by
  default.
- **Golden files:** the worked example pins the descriptor blob, the
  navigation tier, and both clusters' row-id arrays real and byte-exact —
  the first `conformance/vectors/` vector's non-opaque fields, once
  implemented. It does **not** pin a real golden file on its own: the
  quantized-code payload (97% of the worked example's posting-list bytes)
  is explicitly marked opaque, not fabricated, so a genuine golden file for
  this family requires a working encoder and is owed at M2 implementation
  time (Open questions), not satisfied by this worked example alone.

## How this could be wrong

**Nearest grave: the Optane-era formats** (`docs/lineage.md`) — "hardware-
specific choices baked into media layouts, unimplementable the day the
hardware died." This RFC adopts `kBatchSize = 32` (the reference
implementation's own FastScan batch size,
`references/rabitq-library-ivf-and-batch-layout-source.md`, `fastscan.hpp`)
as a **wire-format** constant, not merely an implementation
detail — every reader that ever decodes this blob family must agree on 32,
forever, regardless of what SIMD width its own hardware favors. This
question is now **resolved, not merely well-evidenced**: a second
Discussion amendment (below) fetched the actual SIMD `accumulate()`
decode kernels — `src/simd/fastscan_avx2.cpp` and
`src/simd/fastscan_avx512.cpp`, vendored in full in
`references/rabitq-library-fastscan-accumulate-source.md` — the file this
RFC's own Open questions had named as the one thing that would settle it.
Both kernels share one identical function signature, are selected by a
single runtime function pointer (`src/simd/dispatch.cpp`'s
`kAccumulateFn`), and — despite AVX512's accumulator being twice AVX2's
width (512 bits vs. 256 bits) — both produce **exactly 32 result values
per call, never 64**. If 32 were a register-width artifact, the wider
AVX512 kernel would naturally batch 64 vectors; instead its extra width is
spent processing more of the packed `dim`-columns per loop iteration, with
an extra horizontal-combine step folding the wider accumulators back down
to the same 32-lane result. The batch size traces instead to the
FastScan/PQ nibble-lookup trick's own fixed 16-entry table (`pack_lut`
builds one 16-entry LUT per 4-bit sub-code — `2^4 = 16` — a property of
the sub-code width, not of any vendor's ISA) doubled by `pack_codes`'s
hi/lo nibble packing: 16 × 2 = 32. That LUT technique is attributed in the
source itself to Faiss's FastScan design, which in turn is built on
SSSE3's 128-bit `pshufb` instruction (16-byte, 16-entry lookups) —
predating AVX2 and AVX512 by roughly a decade — and both `_mm256_
shuffle_epi8` and `_mm512_shuffle_epi8` still operate within 128-bit lanes
regardless of the surrounding register's total width, a documented
property of the x86 instruction set, not a choice this library made.
`kBatchSize = 32` is algorithm-shaped: it is the LUT width's own fixed
data-parallelism shape, adopted unchanged into this format's wire bytes,
and carries no residual dependency on any specific vendor's register
width. Adopting an external codec's own batch constant as this format's
own wire bytes remains, in general, exactly the kind of decision the
Optane grave warns about — the finding here is that in this specific case
the audit came back clean, not that the category of risk was never real.

**Second, and the more consequential risk: R1's own kill criterion.** This
RFC's corrected sizing law (Napkin math) runs ~31% over the provisional
100 MB budget before replication, and, on a provisional and explicitly
unverified estimate, roughly ~2.27× over at a realistic
replica-8-equivalent density — over half the margin to the 4× threshold
that would falsify the mission claim, not a wide one, and real headroom
consumed that the previously stated (uncorrected) sizing law didn't show
consumed at all. A direct, load-bearing consequence: at the corrected law,
a segment fitting the 100 MB tier-1 budget holds ~760,000 768d vectors, not
the ~1,000,000 `CLAUDE.md` §7 currently states — a real, ~24% reduction in
vectors-per-segment that this RFC's own numbers force, propagating directly
into M3's segment-count-amplification benchmark and every downstream
estimate that assumes the old figure. If a future RFC's real, measured
centroid count runs meaningfully higher than the `4·√N` convention this RFC
borrows (Design §3), if dimensionality climbs toward 1536d rather than
768d (`CLAUDE.md` §5 invariant 7's own `96/128/192 B` scaling table), or if
the replication estimate above turns out low once SPANN's real body figures
are fetched (a real possibility this RFC states rather than assumes away),
the margin narrows further still — this RFC's own numbers are the first
real input to that future accounting, not a promise that the margin stays
wide.

**Third (resolved by Discussion amendment, below): the intra-batch
bit/lane order.** At Approval time, this RFC left a real, named gap: the
byte *offsets* of the code region and the three factor arrays within a
FastScan batch were grounded
(`references/rabitq-library-ivf-and-batch-layout-source.md`), but the
bit-level layout of the 1-bit codes themselves within that region was not
— a from-scratch, clean-room implementation could not have been written
against this chapter for the code bits specifically. A follow-up fetch
(prompted by "start with the FastScan grounding fetch," the immediate next
step this RFC's own Open questions named) closed it: `fastscan::
pack_codes`'s complete algorithm is now vendored
(`references/rabitq-library-fastscan-pack-codes-source.md`) and
independently re-executed against a synthetic input to confirm the
transcription, not merely copied. Kept here, not deleted, as a record of
what this RFC's own review correctly flagged as unmitigated at Approval —
this is exactly the class of gap the M4 clean-room read (`CLAUDE.md` §9)
is designed to catch, caught and closed before implementation began rather
than at that milestone.

**Fourth: cross-segment codebook incompatibility (Design §7).** This RFC
registers `concatenate + remap` as the posting-list merge strategy only
when descriptors match byte-for-byte — a real constraint, not a hypothetical
one, since a production writer that retrains its codebook per segment
(plausible, since RaBitQ's rotation is cheap to regenerate) would silently
force every merge onto the more expensive `rebuild` path, with no format-
level signal warning the writer this is about to happen until compaction
time. This RFC names the constraint; it does not yet give writers a way to
detect the mismatch cheaply before attempting a merge, which is real,
separate, unresolved work (Open questions).

## Alternatives considered

**Storing centroids pre-quantized (also RaBitQ-coded), not full precision.**
Would shrink the navigation tier's ~12.3 MB/million-vectors contribution
substantially. Rejected for v0.1: centroids are compared against a raw
query vector to select nprobe, and SPANN's own centroids-in-memory routing
structure is kept full-precision while only the on-disk posting-list
payload is quantized (`references/spann-neurips2021.md`) — the precedent
this RFC's cluster-family design is actually built on (DiskANN's own
two-tier split runs the opposite way, quantizing the routing structure
itself, and is not a precedent for this choice — Design §3). Quantizing the
routing tier risks systematically misrouting queries away from their true
nearest clusters in a way this RFC has not measured. Real, separate,
un-grounded work if pursued (Open questions).

**Splitting the posting-list blob into two blobs (all-clusters'-codes, then
all-clusters'-ids), matching the reference library's own in-memory layout
exactly.** Rejected: this is the reference library's own choice for a
fully-RAM-resident index, where any region is randomly addressable at zero
extra cost. STRAND's cold-open model pays a real per-GET round-trip cost
(the ~100ms planning figure), so co-locating each cluster's codes and ids
(Design §4) trades a marginally more complex per-cluster offset accounting
for cutting the GET count per selected cluster in half — the right trade
for this format's actual access pattern, even though it means STRAND's
wire layout is not a byte-identical mirror of the reference library's own
on-disk format (which this RFC never claimed as a goal — only the
*quantization codec itself*, not the index's outer layout, is adopted by
reference).

**Generator + seed for `FhtKacRotator`'s sign bits, instead of
materializing them.** Would shrink the rotation payload from 384 bytes to
~8 bytes (one u64 seed) at 768 padded dims. Rejected: the savings are
already negligible in absolute terms (384 bytes is a rounding error against
even this RFC's own tiny worked example, let alone a real segment), and a
seed would require this RFC to additionally pin an exact, cross-language,
bit-reproducible PRNG algorithm (the reference implementation's own
`std::mt19937`, specifically) as a **normative format requirement** — a
real, avoidable determinism burden this RFC sidesteps entirely by
materializing the (already cheap) realized state instead, exactly matching
what the reference implementation's own `save()`/`load()` pair already does
(Design §2).

## Open questions / follow-on RFCs

- **Multi-bit Extended-RaBitQ registration** (Non-goals) — the classical
  scalar-quantization distance kernel, the `ExDataMap` per-vector byte
  formula this session already fetched but did not wire-format-register
  here (`references/rabitq-library-ivf-and-batch-layout-source.md`), and the
  bit-width-to-recall figures this session's own fetch of the reference
  library's `index.md` newly confirmed with a real primary source (4/5/7-bit
  → 90/95/99% recall without reranking, `references/rabitq-library-index-
  overview.md`) — resolving the gap `docs/research/README.md` R3 flagged as
  "not found in this fetch's excerpt" when only the paper's abstract had
  been vendored. A ledger update recording this resolution is due alongside
  this RFC.
- **SPANN-style replication's metadata slot and construction algorithm**
  (Non-goals) — a real design surface this RFC's own napkin math shows is
  worth the design effort (~2.27× the budget at a realistic, provisional
  replica-8-equivalent density estimate is a real cost the format should
  let a deployment see and tune, not hide), and a stated M2 milestone
  deliverable this RFC does not complete.
- **Fetching SPANN's real body figures** (`arxiv.org/abs/2111.08566`'s PDF,
  not the abstract this session's WebFetch attempt confirmed does not
  surface index-size tables) to replace the Napkin math section's
  provisional, explicitly-flagged-unverified 1.73×/≈227 MB replication
  estimate with a genuinely grounded replica-1-through-8 figure.
- ~~Updating `CLAUDE.md` §7's "roughly one segment per million 768d
  vectors" sentence~~ — done, alongside this Approval (`CLAUDE.md` §7 now
  reads ~760,000).
- **Cross-segment codebook-sharing policy, and a cheap pre-merge
  compatibility check** (Design §7, How this could be wrong) — whether
  STRAND mandates one codebook per table/index by convention, or defines a
  compatibility check plus an explicit requantization path for compaction,
  so a writer is not surprised by an expensive `rebuild` only at merge
  time.
- **The graph-blob family (warm tier)** — R1's second half, the node-order
  permutation algorithm question, entirely untouched by this RFC.
- ~~The intra-batch bit/lane order~~ — resolved (Discussion, below; `spec/
  vectors.md` §4).
- ~~FastScan `kBatchSize = 32`'s hardware-vs-algorithm provenance~~ —
  resolved (Discussion, below; `references/rabitq-library-fastscan-
  accumulate-source.md`). Both `accumulate_avx2` and `accumulate_avx512`
  produce exactly 32 results per call despite AVX512's register being
  twice AVX2's width; 32 traces to the FastScan/PQ nibble-LUT's fixed
  16-entry table (`2^4` sub-code values) doubled by hi/lo nibble packing,
  not to any register width. Algorithm-shaped, not hardware-shaped.
- **A real M0-style byte-budget measurement** for this blob family, the
  same way `bench/src/cold_open.rs` gave invariant 3 a real measured
  baseline instead of only round-trip-count arithmetic — this RFC's Napkin
  math is arithmetic against grounded formulas, not a benchmark result, and
  M2's own milestone gate should not be considered met until one exists.

## Discussion — post-approval amendments

**2026-08-19 — intra-batch bit/lane order resolved.** Prompted by "start
with the FastScan grounding fetch" — the user directly requesting the
follow-on work this RFC's own Open questions named as the natural next
step before implementation. At Approval, the one wire-format-relevant gap
this RFC left genuinely open was the bit-level layout of 1-bit codes
*within* a FastScan batch's code region: the byte offsets of that region
were grounded, but which bit of which vector's code lands at which byte
was adopted by reference without independent verification.

Fetched live: `include/rabitqlib/fastscan/fastscan.hpp` in full (the
`pack_codes` function, `kPerm0`, `get_column`) and the confirming call site
in `include/rabitqlib/quantization/rabitq_impl.hpp` (`one_bit_batch_code`,
in `namespace ...::one_bit`, calling `fastscan::pack_codes` directly on the
output of `one_bit_compact_codes` — proving this is genuinely the 1-bit
RaBitQ path this RFC registers, not an unrelated codec sharing the same
file). Vendored at `references/rabitq-library-fastscan-pack-codes-source.md`.

The algorithm: for each byte-column of a batch's 32 (zero-padded) vector
slots, split each byte into hi/lo nibbles, then use a fixed 16-entry
permutation (`kPerm0`) to interleave pairs of vectors' nibbles into 32
output bytes — full detail and the exact permutation table in the vendored
file and now normatively in `spec/vectors.md` §4. This was **independently
re-executed**, not merely transcribed: a faithful Python port run against a
synthetic 2-vector input (`padded_dim = 64`, arbitrary illustrative byte
values `0x12 0x34 0x56 0x78 0x9A 0xBC 0xDE 0xF0` for vector 0 and
`0x11 0x22 0x33 0x44 0x55 0x66 0x77 0x88` for vector 1, the rest of the
32-slot batch zero-padded) produced 256 bytes (`= padded_dim * 4`,
confirming the byte-count formula unchanged), whose first 32 bytes were
then checked by hand against the algorithm's own definition, column by
column — bytes 0–15 are `0x1 0x0 0x1` followed by 13 zero bytes, and bytes
16–31 are `0x2 0x0 0x1` followed by 13 zero bytes — matching the executed
output exactly. This is the same "computed with real executed
code... not hand-derived" discipline RFC 0007's worked example already
established as this project's standard.

Two secondary findings fell out of the same fetch, both strengthening
existing RFC decisions rather than requiring new ones: (1)
`one_bit_batch_code`'s own doc comment requires `padded_dim % 64 == 0` —
confirming Design §2's STRAND-specific requirement that `MatrixRotator`
share `FhtKacRotator`'s 64-multiple padding is not merely a STRAND
alignment convenience layered on a codec that would otherwise tolerate an
unpadded dimensionality, but a real requirement of the registered codec
itself. (2) `get_column`'s own zero-fill behavior for batch slots beyond
the real vector count independently confirms Design §4's padding-
determinism rule (zero-fill unused lanes) matches the reference
implementation's actual behavior, not a STRAND-invented compatible
convention layered on unspecified behavior.

Sections updated: Design §4 (citation and framing, no arithmetic change —
the byte-count formula was already correct, only the byte *order* was
missing), Design §2 (strengthened rationale for `MatrixRotator`'s padding
requirement), Non-goals (removed the now-resolved item, kept ARM/SIMD
kernel validation as still out of scope), Invariant-11 checklist
("codec-variant provenance" now reads complete rather than partial), "How
this could be wrong" (the intra-batch-order paragraph marked resolved; the
`kBatchSize` hardware-provenance paragraph strengthened with real evidence
that the packing algorithm itself carries no register-width assumption,
while noting the SIMD `accumulate()` decode kernel — a separate file, not
fetched — is what would fully close that adjacent, narrower risk), Open
questions (struck the resolved item, kept the narrower SIMD-dispatch
question), and `spec/vectors.md` §4 (gained the complete normative
algorithm). No arithmetic in this RFC changes as a result of this
amendment — this closes a specification-completeness gap, not a sizing or
round-trip correction.

**2026-08-19 — FastScan `kBatchSize = 32` hardware-vs-algorithm provenance
resolved.** The immediate next step the prior amendment's own "What
remains genuinely unfetched" sentence named: `src/simd/fastscan_avx2.cpp`
and `src/simd/fastscan_avx512.cpp`, the actual SIMD `accumulate()` decode
kernels, fetched live and vendored in full
(`references/rabitq-library-fastscan-accumulate-source.md`). The
declarations live at `include/rabitqlib/simd/fastscan_dispatch.hpp`
(header only, both ISA variants declared with one shared signature); the
definitions live under `src/simd/`, not `include/rabitqlib/simd/` as the
prior Open-questions note assumed — confirmed by listing both directories
live before fetching rather than guessing the path.

The finding: `accumulate_avx2` and `accumulate_avx512` are selected by a
single runtime function pointer of one fixed type
(`fastscan::kAccumulateFn`, `src/simd/dispatch.cpp`) and, despite
AVX512's accumulator being twice AVX2's register width (512 bits vs.
256 bits), both produce **exactly 32 result values per call** — AVX2 via
two 16-wide stores, AVX512 via one 32-wide store, never 64 either way. A
register-width-driven batch size would predict AVX512 naturally batching
64 vectors; it does not. AVX512's extra width instead processes more of
the packed `dim`-columns per loop iteration, with an added
horizontal-combine step folding the wider partial sums back down to the
same 32-lane result. Tracing further: `fastscan::pack_lut` builds one
16-entry lookup table per 4-bit sub-code (`2^4 = 16`, fixed by the
sub-code width, not by any ISA), and `pack_codes` (already vendored)
doubles that via hi/lo nibble packing — 16 × 2 = 32. Both `_mm256_
shuffle_epi8` and `_mm512_shuffle_epi8` operate within 128-bit lanes
regardless of total register width, a documented x86 property inherited
from SSSE3's `pshufb`-based FastScan/PQ lookup trick (the source's own
attribution to Faiss's FastScan design) that predates both AVX2 and
AVX512. `kBatchSize = 32` is algorithm-shaped, not hardware-shaped: it is
adopted unchanged into this format's wire bytes with no residual
dependency on any specific vendor's register width, closing the risk
invariant 10 exists to prevent for this specific constant.

Sections updated: "How this could be wrong" (the `kBatchSize`
hardware-provenance paragraph now states the resolution and its evidence
rather than a residual, well-evidenced-but-unproven risk) and Open
questions (the item struck as resolved). No arithmetic in this RFC
changes, no wire format changes, and no shipped code changes — `kBatchSize
= 32` in `crates/strand-vector/src/fastscan.rs` was already correct; this
amendment closes a citation gap in the RFC's own adversarial review, not a
design or implementation defect.
