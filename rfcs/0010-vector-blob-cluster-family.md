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
replication — and, on a real body-sourced replication ratio (still
extrapolated across an unmeasured baseline step, `references/spann-body-
figures.md`), over half of R1's 4× kill-criterion margin once realistic
replication is applied. Not close to falsifying the mission claim, but a
real correction, made honestly, not rounded away.

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
  density, grounded against real SPANN/Li et al. body figures rather than an
  unverified estimate (Discussion, below) — Napkin math), which makes deferring
  the knob that would make that cost visible and tunable the most
  consequential Non-goal in this RFC, not the least. M2's own milestone gate
  is therefore not fully met by this RFC alone (Open questions, below); a
  follow-on RFC that also does construction-side clustering owns the
  metadata slot and the construction algorithm together. **Resolved by
  M2-1 (Discussion — post-approval amendment, below, `docs/roadmap.md`):**
  the metadata slot (Design §3) and a real, SPANN-grounded construction
  algorithm (`crates/strand-vector/src/closure.rs`) now exist. Cross-
  segment codebook sharing at merge/compaction time (the next bullet) and
  compaction-time re-replication after a rebalance remain separate, still
  open, work (Open questions).
- **Cross-segment codebook sharing and retraining at merge/compaction time.**
  Named precisely in Design §7 (merge semantics) as a real, load-bearing,
  unresolved question — not silently assumed away. **Partially resolved by
  M2-8 (Discussion — post-approval amendment, below, `docs/roadmap.md`):**
  a cheap, `O(1)`-after-construction pre-merge compatibility *check*
  (`crates/strand-vector/src/codebook.rs`) now exists, so a merge planner
  can detect a codebook mismatch before attempting `concatenate + remap`
  rather than discovering it only when a merge already under way produces
  corrupted distance estimates. What M2-8 does **not** resolve, named
  precisely rather than folded in: the *policy* question of whether STRAND
  mandates one codebook per table by convention or defines an explicit
  requantization path for compaction, cluster-assignment compatibility
  with a merged, rebalanced navigation tier (Design §7's own second
  clause), and the actual merge/compaction code path that would call this
  check — all three remain real, separate, unimplemented work, owned by
  M3-1 (`docs/roadmap.md`).
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

**Closure-replication descriptor trailer (M2-1, added by this RFC's
Discussion — post-approval amendment below).** A fixed 8 bytes, always
present, immediately after `cluster_dir`:

| offset | size | field                    | notes |
| ------ | ---- | ------------------------ | ----- |
| 0      | 1    | `replication_policy`     | u8: `0` = none (every vector assigned to exactly its primary cluster — the all-zero, pre-M2-1-equivalent default); `1` = `spann-closure` (`crate::closure`, `references/spann-closure-assignment-algorithm.md`). A reader MUST NOT reject an unrecognized value: query resolution's own row-id deduplication (Design §6 step 3) already tolerates duplicate row-ids regardless of *why* they're duplicated, so a reader that doesn't recognize a future policy value still decodes and queries this blob correctly — it just can't explain the segment's replication cost in that policy's own terms |
| 1      | 1    | `max_replicas`           | u8; the per-vector replica cap actually used, total posting lists a vector may land in (primary included) — SPANN's own `ReplicaCount`. MUST be `0` when `replication_policy = 0`; MUST be `>= 1` when `replication_policy = 1` |
| 2      | 2    | reserved                 | u16; writer MUST set zero; reader MUST NOT reject nonzero but MUST NOT interpret it |
| 4      | 4    | `replication_epsilon`     | f32, little-endian; SPANN's own ε₁ actually used (below). MUST be `0.0` when `replication_policy = 0` |

This slot deliberately records the construction-time *policy and knobs*,
not a realized replication *factor*. The realized factor (total
(cluster, vector) assignments divided by distinct vectors) is already
exactly recoverable, with no new wire bytes, from data a cold-open reader
already has resident: sum `vector_count` across every `cluster_dir` entry
(already fetched wholesale as part of this same blob) and divide by the
segment's own `row_id_count` (already decoded from the container's
hotcache before this blob family is even touched, Design §1) — because
this field's flat-vector blob is dense over every row-id in the segment
(Design §5), this field's distinct vector count *is* the segment's
row-id count. An earlier draft of this trailer stored that count directly
(`distinct_row_id_count`, a redundant 8 bytes); the adversarial review
below caught the redundancy and it was removed before implementation,
per invariant 8's novelty-budget discipline.

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
   closure replication (Napkin math; the metadata slot and construction
   algorithm, Design §3 and `crate::closure`, M2-1), the same row-id can
   legitimately appear in more than one of the `nprobe` scanned clusters —
   a reader MUST deduplicate by row-id before ranking, keeping each
   row-id's best (closest) estimated distance across the clusters it
   appeared in.
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
batch), `vector_count = 2` (`02 00 00 00`), reserved `00 00 00 00`.
`replication_descriptor` trailer (M2-1, Discussion — post-approval
amendment, below): this worked example uses no closure replication, so the
trailer is 8 zero bytes — `replication_policy = 0`, `max_replicas = 0`,
reserved `00 00`, `replication_epsilon = 0.0` (`00 00 00 00`) —
`00 00 00 00 00 00 00 00`. Total navigation-tier blob:
`4 + 4 + 512 + 48 + 8 = 576` bytes.

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
earlier draft of this section stated); the navigation tier's own fixed
`num_clusters`/reserved header costs 8 bytes, and — since M2-1's Discussion
amendment below — its closure-replication descriptor trailer (Design §3)
adds a further fixed 8 bytes, both independent of `num_clusters` and both
negligible at this scale. **Total tier-1 (navigation tier + posting
lists), no replication: `118,592,000 + 12,288,000 + 96,000 + 8 + 8 + 400 =
130,976,416` ≈ 131.0 MB per million 768d vectors** — a real ~31% over the
`CLAUDE.md` §7 provisional 100 MB cold-open byte budget, and a real,
uncomfortable-but-honest correction to `docs/data-structures.md`'s own
stated sizing law, made here rather than left standing. This is nowhere
near R1's falsifying kill criterion (4× the budget), so it does not narrow
the mission claim. (The pre-M2-1 total quoted the navigation tier's own
8-byte header nowhere in this particular sum — an immaterial, pre-existing
8-byte omission in a 131-million-byte figure — while separately counting it
in the "bytes actually fetched" figure below; M2-1's amendment closes that
gap in the same edit that adds the new trailer, so the total's delta from
the previously stated `130,976,400` is `+16`, not the trailer's own `+8`
alone.)

This 131.0 MB figure is the whole quantized corpus per segment, not what a
single query fetches at open — see Design §8's `cold-fetchable` distinction.
**Bytes actually fetched wholesale at open** (descriptor + navigation
tier only): `400 + 8 (num_clusters/reserved header) + 12,288,000 + 96,000
+ 8 (closure-replication descriptor trailer, M2-1) = 12,384,416` ≈
**12.4 MB** — comfortably under the 100 MB budget on its own.

**This is no longer arithmetic alone.** `bench/src/vector_cold_open.rs`
(2026-08-19) closes the gap this RFC's own Open questions named: real
k-means (`strand_vector::kmeans`), a real `FhtKacRotator` descriptor, real
rotation (`strand_vector::rotate::rotate_fht_kac`), and real 1-bit RaBitQ
quantization (`strand_vector::quantize::quantize_one_bit`) build an actual
four-blob-type segment — 10,000 real 768-dimensional vectors, 400 clusters
(`4·√10,000`, `strand_vector::kmeans::recommended_cluster_count`) — committed
to real MinIO via `strand-core`'s actual manifest CAS protocol and reopened
cold 30 times. Real, measured result
(`bench/results/vector-cold-open.json`, re-run 2026-08-19 alongside M2-1 —
see Discussion, below — to include the new closure-replication descriptor
trailer): the descriptor and navigation-tier
blobs, read back from the real segment's own hotcache registry, total
**1,238,816 bytes** — **1.24% of the 100 MB budget** — in **3 GETs** (pointer,
snapshot, segment), matching invariant 3's bound exactly. Scaling this run's
own real per-cluster navigation-tier cost to RFC 0010's own 1,000,000-vector
napkin-math scale via the same `4·√N` rule gives **12,384,416 bytes**
— to the byte, the hand-computed **12.4 MB** figure directly above. The
formula was right; this is no longer just trusting the arithmetic. One
honest limitation carried over from `bench/src/cold_open.rs` and
`bench/src/field_cold_open.rs`: `strand-core`'s `ConditionalStore` has no
Range-GET variant yet, so the actual network fetch this benchmark issues
pulls the whole 33,984,740-byte segment (posting lists and flat vectors
included, not just the open-wave subset) — real whole-segment-GET latency
against local MinIO in the M2-1 re-run was p50 47.6ms / p90 57.7ms / p99
72.8ms (n=30) — lower than the original run's p50 92.6ms / p90 102.2ms /
p99 114.1ms, most plausibly host-load variance (both runs share MinIO's
own localhost-with-no-injected-latency caveat, and this project's shared
build host was under heavy concurrent load during the M2-1 session,
`docs/roadmap.md`), not a claim that Range-GET-only fetching improved —
this is still a strictly harder number than a Range-GET-only open-wave
fetch would show, and not yet the real-network tail figure `CLAUDE.md` §7
still lists as a placeholder.

The 131.0 MB figure is the segment-sizing quantity R1's own
still-open sizing-law work needs (`CLAUDE.md` §7's "segment count is
reported, never hidden" rule): at the corrected law, a segment that stays
within the 100 MB tier-1 budget holds roughly `100 / 131.0 ≈ 0.76` million
768d vectors — **~760,000 vectors/segment**, not the ~1,000,000
`CLAUDE.md` §7 previously stated — updated in place alongside this RFC's
Approval.

**Replication's cost — now a real, body-sourced figure, though still an
extrapolation across a step neither source paper measured.**
`docs/data-structures.md` names SPANN-style closure replication (up to 8×)
as a first-class knob. The figure this RFC depends on — 13.0 GB at replica
8 vs. 7.5 GB at replica 2 on GIST1M — was re-fetched and confirmed directly
from a paper's body, closing the item this RFC's own Open questions section
named: **the figure lives in the companion cloud-native benchmark paper**
(Li et al., `arxiv.org/abs/2511.14748`, Table 4, §5.3 — "Size metrics of
SPANN configurations on GIST1M"), not in SPANN's own NeurIPS 2021 paper,
which was independently fetched in full (`arxiv.org/pdf/2111.08566`, the
paper's only arXiv version) and confirmed to contain no GIST1M dataset and
no index-size-in-bytes figure anywhere
(`references/spann-body-figures.md`, `references/spann-neurips2021.md`'s
own updated caveat). `docs/research/README.md` R1's parenthetical — "on
GIST1M **in the benchmark**" — had this attribution right all along; this
RFC's own prior citation to SPANN's paper was the error, now corrected.
The ratio itself is unchanged and arithmetically exact: 13.0 / 7.5 = **1.73×**
growth from replica 2 to replica 8, on a real 1,000,000-vector, 960-
dimension GIST1M index. This RFC's own 131.0 MB baseline carries **no**
replication (replica 1, not replica 2), and neither paper reports a
replica-1 index-size figure at all — the lowest measured point in either
source is replica=2 — so applying the 1.73× ratio to a replica-1 baseline
remains a deliberately conservative **lower bound**, not a replica-8
prediction, exactly as this RFC already stated before the re-fetch: it
omits whatever the 1→2 step itself costs. `130,976,400 * 13.0 / 7.5 ≈
227,025,760` ≈ **≈227 MB**, ≈2.27× the 100 MB budget — over half the
margin to R1's 4× kill criterion, not a wide one, and likely an
*underestimate* of the true replica-8 cost since it skips the 1→2 step.
This figure is no longer provisional in the sense of "unconfirmed against
a primary source" — it is a real, quoted, body-table number, correctly
attributed — but the replica-1-baseline extrapolation it feeds into
remains a real, named limitation, not a measurement.

## Invariant-11 checklist

- **Endianness:** little-endian throughout — every multi-byte field in the
  quantization descriptor, the navigation tier's `centroid_table` (f32) and
  `cluster_dir` (u64/u32), the posting-list blob's row-id arrays (u64), and
  the closure-replication descriptor trailer's `replication_epsilon` (f32,
  M2-1).
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
  `conformance/vectors/toy-navigation-tier.bin` was regenerated at M2-1
  (568 → 576 bytes) to include the new all-zero closure-replication
  descriptor trailer; every other golden file in this family is unaffected
  (the trailer is additive to the navigation tier only).

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
100 MB budget before replication, and, now grounded against real SPANN/Li
et al. body figures rather than an unverified estimate (Discussion,
below), roughly ~2.27× over at a realistic
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
the true replica-1-to-replica-8 cost turns out higher than the 1.73×
replica-2-to-8 ratio this RFC applies as a lower bound (`references/spann-
body-figures.md` — neither SPANN's own paper nor the companion benchmark
paper reports a replica-1 index size, so the 1→2 step's real cost remains
unmeasured, a real possibility this RFC states rather than assumes away),
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

**Fourth: cross-segment codebook incompatibility (Design §7). Partially
resolved (M2-8, Discussion — post-approval amendment, below).** This RFC
registers `concatenate + remap` as the posting-list merge strategy only
when descriptors match byte-for-byte — a real constraint, not a hypothetical
one, since a production writer that retrains its codebook per segment
(plausible, since RaBitQ's rotation is cheap to regenerate) would silently
force every merge onto the more expensive `rebuild` path, with no format-
level signal warning the writer this is about to happen until compaction
time. This RFC named the constraint but did not give writers a way to
detect the mismatch cheaply before attempting a merge; M2-8 closes exactly
that gap with a cheap identity-and-compare mechanism
(`crates/strand-vector/src/codebook.rs`), argued out on its own below. What
remains genuinely open, not touched by M2-8: whether STRAND should mandate
one codebook per table by *policy* (so the mismatch this check detects
never arises in practice), an explicit requantization path for compaction
when it does, and the actual merge-planner code that calls this check as
part of a real merge decision — all real, separate, unresolved work
(Open questions, M3-1).

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
- ~~SPANN-style replication's metadata slot and construction algorithm~~ —
  done (M2-1, 2026-08-19, Discussion — post-approval amendment, below):
  the closure-replication descriptor trailer (Design §3) and a real,
  SPANN-grounded construction algorithm (`crates/strand-vector/src/
  closure.rs`, `references/spann-closure-assignment-algorithm.md`) both
  exist, with real tests including a hand-checked worked example
  (`crates/strand-vector/tests/closure_replication_end_to_end.rs`). Two
  narrower items this resolution does **not** close, named precisely
  rather than folded in: **compaction-time re-replication** — when
  `rebalance` moves a vector to a different primary cluster (Design §7),
  whether and how its closure replicas should be recomputed is real,
  separate, unimplemented work, since this RFC's construction algorithm
  runs at initial segment build time only; and **cross-segment codebook
  sharing at merge time**, already named below, which M2-8
  (`docs/roadmap.md`) tracks as a distinct question.
- ~~Fetching SPANN's real body figures~~ — done (2026-08-19): SPANN's own
  PDF body was fetched in full and confirmed to contain **no** GIST1M
  dataset and **no** index-size figure at any replica count
  (`references/spann-body-figures.md`). The real figure lives in the
  companion benchmark paper (Li et al., `arxiv.org/abs/2511.14748`, Table
  4, §5.3), also now fetched in full and quoted verbatim — real, measured,
  13.0 GB vs 7.5 GB at replica 8 vs 2 on GIST1M, the same 1.73× ratio this
  RFC already used, now grounded rather than flagged unverified (Napkin
  math). Still open, and a real limitation this fetch could not close:
  neither paper reports a replica=1 (unreplicated) index size, so applying
  the replica-8/replica-2 ratio to this RFC's own replica-1 tier-1 baseline
  remains a conservative extrapolation, not a direct measurement — a true
  "replica-1-through-8" figure does not exist in the literature found so
  far.
- ~~Updating `CLAUDE.md` §7's "roughly one segment per million 768d
  vectors" sentence~~ — done, alongside this Approval (`CLAUDE.md` §7 now
  reads ~760,000).
- ~~A cheap pre-merge codebook compatibility check~~ — done (M2-8,
  2026-08-19, Discussion — post-approval amendment, below):
  `crates/strand-vector/src/codebook.rs`'s `CodebookIdentity` and
  `check_compatibility` let a merge planner detect a codebook mismatch in
  `O(1)` per pair after one `O(n)`-in-payload-length identity computation
  per segment, with real tests distinguishing a shared, real codebook from
  two independently trained ones and from a structurally different one
  (`crates/strand-vector/tests/
  codebook_compatibility_across_segments.rs`). What M2-8 does **not**
  resolve, real and still open: **cross-segment codebook-sharing policy**
  (whether STRAND mandates one codebook per table/index by convention, or
  defines an explicit requantization path for compaction when segments
  disagree), **cluster-assignment compatibility** with a merged,
  rebalanced navigation tier (Design §7's own second `concatenate + remap`
  clause — this check closes only the codebook half), and **the actual
  merge-planner code** that would call this check as part of a real merge
  decision. All three are M3-1's job (`docs/roadmap.md`), not this RFC's.
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
- ~~A real M0-style byte-budget measurement for this blob family~~ —
  resolved (Discussion, below; `bench/src/vector_cold_open.rs`, Napkin math
  above). Real, measured result at a 10,000-vector/400-cluster scale
  (re-run 2026-08-19 alongside M2-1 to include the new closure-replication
  descriptor trailer):
  1,238,816 open-wave bytes (1.24% of the 100 MB budget), 3 GETs/open,
  p50=47.6ms whole-segment-GET latency against local MinIO — and the same
  run's own real bytes extrapolate to RFC 0010's 1,000,000-vector scale at
  12,384,416 bytes, matching this section's hand-computed figure exactly.
  Not yet closed: the real-network tail-latency figure (`CLAUDE.md` §7's
  own still-open placeholder) and a Range-GET-capable reader (today's
  benchmark fetches the whole segment object, not just the open-wave
  subset — the same limitation `bench/src/cold_open.rs` and
  `bench/src/field_cold_open.rs` already carry).

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

**2026-08-19 — a real M0-style byte-budget measurement, closing this RFC's
own Open questions item.** Prompted by "build a real M0-style byte-budget
measurement for the vector blob family, the same way `bench/src/
cold_open.rs` already gave invariant 3 a measured baseline for the generic
container open" — this RFC's own Open questions naming exactly this gap and
stating "M2's own milestone gate should not be considered met until one
exists."

`bench/src/vector_cold_open.rs` (new) follows `bench/src/cold_open.rs`'s
and `bench/src/field_cold_open.rs`'s established pattern: assemble a real
segment, commit it to real MinIO via `strand-core`'s actual manifest CAS
protocol (`testcontainers`, not a pre-existing manual container), reopen it
cold, measure. What makes this segment real rather than illustrative: 10,000
real random 768-dimensional vectors, clustered by the crate's own real
`strand_vector::kmeans::kmeans` (Lloyd's algorithm, k-means++ seeding,
`k = 400` from `recommended_cluster_count`'s `4·√N` rule — capped at 5
iterations, enough for non-degenerate, varying cluster sizes at this
benchmark's scale, not convergence, since cluster *quality* has no bearing
on blob *byte sizes*, which are what this benchmark measures), a real
`FhtKacRotator` descriptor (`descriptor::build_fht_kac`), real rotation
applied to both centroids and vectors (`rotate::rotate_fht_kac`), and real
1-bit RaBitQ quantization per vector against its assigned rotated centroid
(`quantize::quantize_one_bit`) — the same functions `crates/strand-vector`'s
own tests exercise, not synthetic byte fillers. The lower end of the RFC's
own suggested ~10,000–50,000-vector scale was chosen deliberately: real
k-means is `O(n·k·dims)` per iteration, and this scale keeps a single-core
run bounded (k-means alone took 22.4s in the actual run) while still large
enough to be a genuine multi-cluster, multi-batch segment, not a toy.

Real, measured result (`bench/results/vector-cold-open.json`, MinIO on
localhost, no injected network latency — the same caveat every prior M0
benchmark in this repository already carries): the descriptor blob (400
bytes) and navigation-tier blob (1,238,408 bytes), read back from the real
committed segment's own hotcache registry after a real footer/hotcache
decode, total **1,238,808 bytes** — **1.24% of the `CLAUDE.md` §7 100 MB
cold-open byte budget** — fetched in a constant, asserted **3 GETs** per
open (pointer, snapshot, segment), matching invariant 3's ≤4-GET bound with
room to spare. Extrapolating this run's own real per-cluster navigation-tier
byte cost to this RFC's own 1,000,000-vector napkin-math scale via the same
`4·√N` cluster-count rule gives **12,384,408 bytes** — matching the
hand-computed **≈12.4 MB** figure earlier in this Napkin math section to the
byte. This is the real confirmation the Open questions item asked for: the
formula was already right, and now real, executed code says so too, not
only arithmetic. **(Byte figures refreshed 2026-08-19 alongside M2-1,
below, to include the new closure-replication descriptor trailer: the
navigation-tier blob is now 1,238,416 bytes, the open-wave total 1,238,816
bytes — still 1.24% of the budget at this precision — and the
1,000,000-vector extrapolation 12,384,416 bytes; the ≤4-GET/room-to-spare
conclusion is unaffected. This paragraph's own narrative — including the
22.4s k-means timing — describes the original run and is left as history;
the re-run's own byte and latency figures are stated fully in the M2-1
Discussion entry below.)**

One limitation carried over honestly, not discovered new: `strand-core`'s
`ConditionalStore` trait has no Range-GET method yet — `get(&self, key)`
fetches a whole object — so the real network fetch this benchmark issues at
"open" downloads the entire 33,984,732-byte segment (the 2,025,728-byte
posting-list blob and the 30,720,000-byte flat-vector blob included), not
just the 1,238,808-byte open-wave subset a conforming Range-GET reader would
fetch. `bench/src/cold_open.rs` and `bench/src/field_cold_open.rs` already
carry this same limitation for the container and lexical families
respectively, so it is not new to this benchmark, but it means the reported
open **latency** (p50 = 92.6ms, p90 = 102.2ms, p99 = 114.1ms, n = 30) is a
strictly harder number than a Range-GET-only fetch would show — real
byte-count separation by blob type, not yet a real separated-latency
measurement. Implementing a Range-GET path on `ConditionalStore` and
re-measuring open latency on the open-wave subset alone remains real,
separate, unimplemented follow-on work.

Sections updated: Napkin math (gained the real-measurement paragraph
directly after the formula-derived ≈12.4 MB figure it confirms), Open
questions (struck the resolved item, with the real numbers inline). No
formula or sizing arithmetic in this RFC changes as a result of this
amendment — the real numbers confirm the existing formulas rather than
correcting them.

**2026-08-19 — SPANN replication figures re-grounded from a real body, not
an abstract.** Prompted by this RFC's own Open questions item: fetch
SPANN's real body figures to replace the provisional, explicitly-flagged-
unverified 1.73×/≈227 MB replication estimate.

`arxiv.org/pdf/2111.08566` (SPANN's own paper, the only version on arXiv)
was fetched in full and converted to text. It contains **no GIST1M dataset
and no index-size-in-bytes figure at any replica count** — its own
datasets are SIFT1M, SIFT1B, DEEP1B, and SPACEV1B, and its own replication
experiment (Figure 11) reports only recall/latency curves at replica
1/4/8/10, never a size number. The figures this RFC depends on — 13.0 GB
at replica 8 vs. 7.5 GB at replica 2 on GIST1M — turn out to live
elsewhere: `arxiv.org/abs/2511.14748` (Li et al., "Cloud-Native Vector
Search: A Comprehensive Performance Analysis"), also fetched in full,
Table 4 (§5.3), quoted verbatim in the newly vendored
`references/spann-body-figures.md`. `docs/research/README.md` R1's own
wording — "on GIST1M **in the benchmark**" — had this attribution correct
from the start; this RFC's Napkin math section and Open questions item had
mis-targeted the re-fetch at the wrong paper, an error this amendment
corrects rather than repeats.

The arithmetic itself is unchanged: 13.0 / 7.5 = 1.73×, the same ratio this
RFC already applied to its 131.0 MB replica-1 baseline as a conservative
lower bound, yielding the same ≈227 MB / ≈2.27× figure. What changes is the
citation and the confidence label: the ratio is now a real, quoted,
body-table number rather than one flagged unverified. What remains
genuinely open, confirmed rather than assumed: **no replica=1 index-size
figure exists in either paper** — the lowest measured point in the
literature found so far is replica=2 — so a true replica-1-through-8 curve
is not available, and the 1→2 step's real cost is still an unmeasured gap
this RFC's own lower-bound framing already anticipated.

Sections updated: Napkin math ("Replication's cost" paragraph, re-cited
and re-labeled, arithmetic unchanged), Open questions (item marked done,
with the narrower remaining gap — no replica=1 figure — stated precisely),
"How this could be wrong" (the R1-kill-criterion paragraph's forward-
looking clause updated to reflect what was and wasn't found), and two
reference files updated in place with the new finding
(`references/spann-neurips2021.md`,
`references/cloud-native-vector-search-surveys-2026.md`) alongside the new
`references/spann-body-figures.md`. `docs/ledger.md`'s R1 entry updated to
match.

**2026-08-19 — M2-1: the replication metadata slot and the closure-
replication construction algorithm (`docs/roadmap.md`).** Prompted
directly by this RFC's own Non-goals and Open questions, which named this
as "a stated M2 milestone deliverable this RFC does not complete" — the
one Non-goal this RFC's own text already flagged as "the most consequential
Non-goal in this RFC, not the least."

**Grounding, fetched live, not from memory (`CLAUDE.md` §3).** SPANN's own
closure-assignment algorithm (§3.2.2, "Posting list expansion") was fetched
via arXiv's `ar5iv` HTML rendering (`ar5iv.labs.arxiv.org/html/2111.08566`)
— the raw PDF-to-text extraction `references/spann-body-figures.md` already
used for this paper renders prose cleanly but mangles subscripted
equations, so this session used the maintained LaTeX-to-HTML conversion
instead, specifically for the equation itself. The criterion (Eq. 2): a
vector `x` is additionally assigned to a non-primary cluster `c_ij` iff
`Dist(x, c_ij) ≤ (1 + ε₁) × Dist(x, c_i1)`, `c_i1` being its primary
(nearest) cluster and centroids ordered by ascending distance to `x`. The
paper states `ε₁ = 10.0` for posting-list expansion (§4.2) and "at most 8
closure replicas for each vector" (§4.2.3, the same ablation that already
grounded the replica-8 figure this RFC's Napkin math already depended on).
Both numbers were **independently cross-checked against the paper's own
reference implementation**, `github.com/microsoft/SPTAG`, fetched live via
GitHub code search: `ReplicaCount` defaults to `8`
(`AnnService/inc/Core/SPANN/ParameterDefinitionList.h`), matching the
paper's stated choice exactly and confirming "replica count" means the
*total* posting lists a vector lands in, not extra replicas beyond the
primary. A secondary RNG-rule redundancy-pruning step (skip candidate
`c_ij` when `Dist(c_ij, x) > Dist(c_i(j-1), c_ij)`) was also fetched from
the same section and given only **partial** corroboration: SPTAG ships a
distinctly-named `RNGFactor` parameter with the identical mathematical
shape, but this session could not locate the specific call site applying
it at closure-assignment time (as opposed to head-index construction) via
code search alone. All of this — including the residual RNG-rule gap,
stated precisely rather than smoothed over — is vendored in the new
`references/spann-closure-assignment-algorithm.md`.

**The construction algorithm.** `crates/strand-vector/src/closure.rs`
implements `closure_replicate`: given raw (unrotated) vectors and
centroids, `crate::kmeans`'s own primary assignments, and a `ClosureConfig`
(`epsilon`, `max_replicas`, `apply_rng_rule`), it walks each vector's
non-primary clusters in ascending-distance order, applying Eq. 2's ratio
test (exploiting its own monotonicity to stop at the first failure) and,
optionally, the RNG-rule filter, capped at `max_replicas` total
assignments. `ClosureConfig::spann_default()` returns the paper's own
`epsilon = 10.0`, `max_replicas = 8`. `group_by_cluster` inverts the
per-vector assignment lists into the per-cluster vector-index lists
`crate::posting_list::ClusterInput` needs. One real, named STRAND-specific
interpretation choice, since the paper does not disambiguate: `Dist` is
implemented as **squared** Euclidean distance, matching this crate's own
established L2 convention throughout (`kmeans.rs`'s `squared_distance`,
`query.rs`'s `centroid_distance`) rather than introducing a second,
inconsistent unsquared distance function only for this one purpose —
`references/spann-closure-assignment-algorithm.md`'s own closing section
states this precisely.

**The metadata slot (Design §3, above).** A fixed 8-byte
`replication_descriptor` trailer, always present, appended immediately
after `cluster_dir` in the cluster navigation tier blob:
`replication_policy` (u8), `max_replicas` (u8), 2 reserved bytes, and
`replication_epsilon` (f32 LE). `crates/strand-vector/src/navigation.rs`
gained `ReplicationPolicy`, `ReplicationDescriptor`, and a new
`build_navigation_tier_with_replication` function; the existing
`build_navigation_tier` (every pre-M2-1 call site, unmodified) now simply
calls it with `ReplicationDescriptor::none()`, so **no existing caller
needed to change** — a deliberate compatibility-preserving design choice,
not an accident of the diff. `NavigationTierReader` gained `replication()`
and `realized_replication_factor(distinct_row_id_count)`.

**2026-08-19 — M2-8: a cross-segment codebook-identity mechanism and a
cheap pre-merge compatibility check (`docs/roadmap.md`).** Prompted
directly by this RFC's own "How this could be wrong" item 4 and Open
questions, and by `docs/roadmap.md` M2-8's own text naming a real,
deliberately-unresolved scoping tension between reading the codebook as a
construction-side concern (settled alongside M2's writer/clustering work)
and reading it as an M3-1 merge-planning input (the cost of getting it
wrong is only paid at merge time). This amendment does not adjudicate that
scoping question — it answers the narrower, load-bearing question both
readings agree is real regardless of which milestone owns it: *given two
segments' quantization descriptors, how does a reader or merge planner
decide, cheaply, whether their codebooks are compatible?*

**The identity mechanism.** Design §7 already states the compatibility
criterion in prose — descriptors must be byte-identical in `dims`,
`distance_metric`, `bit_width`, `rotator_type`, and `rotation_payload` — so
this amendment does not introduce a new wire field or bump any blob's byte
layout; the quantization descriptor blob (Design §2) is unchanged. Instead,
`crates/strand-vector/src/codebook.rs` adds `CodebookIdentity`, a small,
fixed-size, computed (not wire-serialized) summary: the four scalar fields
plus a 64-bit content hash (XxHash3-64, `spec/container.md`'s own
registered default checksum algorithm, invariant 11 — reused rather than
introducing a second hash into the project's vocabulary, invariant 8's
novelty-budget discipline) over `dims || distance_metric || bit_width ||
rotator_type || rotation_payload`, each field as its wire-format bytes.
Two considered-and-rejected inputs, named precisely because the task
framing offered both as candidates: a construction-time *version/
generation identifier* was rejected in favor of a *content hash*, because a
generation counter only proves two segments were built by the same writer
process in temporal sequence — it says nothing about whether a segment
built by a *different* writer, or the same writer after a restart with no
persisted counter, happens to carry a byte-identical codebook, which is
exactly the case Design §7's own criterion cares about. A content hash
answers the question Design §7 actually asks (are the bytes the same)
directly, with no dependency on write-path bookkeeping a reader can't see.
The reserved byte at descriptor offset 11 and the derived `padded_dims`
field are deliberately excluded from the hash input, argued in the
module's own doc comment: hashing the reserved byte would make this
project's own "reader MUST NOT interpret it" promise (`spec/vectors.md`
§2) leak into a compatibility decision through the back door, and
`padded_dims` is fully determined by `dims` under the shared padding rule
both registered rotator types use (`descriptor::padded_dims_for`), so
including it would be redundant, not informative — invariant 8's
novelty-budget discipline again, applied to what goes into the hash rather
than what goes on the wire.

**Why this is cheap "without needing to fully decode either codebook,"**
the framing the task itself posed, argued precisely rather than asserted:
building one `CodebookIdentity` costs `O(n)` in `rotation_payload`'s
length — for the registered default (`FhtKacRotator`, 384 bytes at 768
padded dims) this is negligible; for the non-default `MatrixRotator`
(`dims * padded_dims * 4` bytes — 2.36 MB at `dims = padded_dims = 768`,
Design §2) it is a real but bounded, one-time cost, no larger than the
cost a reader already pays once to fetch and use the descriptor blob at
all (invariant 7's cold-open wave includes it). Comparing two already-built
identities (`check_compatibility`) is `O(1)` — four scalar-byte
comparisons plus one `u64` comparison — with **no further access to either
segment's `rotation_payload`**. This is the concrete answer to "cheapest
to check without deserializing the full codebook": a merge planner
touching `N` segments pays `O(N)` total identity-building cost once, then
`O(1)` per pairwise comparison thereafter (`O(N)` or `O(N²)` comparisons
depending on planning strategy, but never re-touching payload bytes),
rather than `O(pairs × payload size)` a naive byte-for-byte comparison run
fresh on every pair would cost — the difference that actually matters at
the "on the order of a hundred segments" scale `CLAUDE.md` §7 already
names for the M3 amplification benchmark.

**The compatibility check function.** `check_compatibility` (and the
`DescriptorReader`-level convenience wrapper,
`check_descriptor_compatibility`) returns `CodebookCompatibility::
Compatible` or `Incompatible(CodebookMismatch)`, the mismatch variant
naming exactly which field first disagreed (`Dims`, `DistanceMetric`,
`BitWidth`, `RotatorType`, or `ContentHash` — checked in that order, cheap
fields first, so an obviously incompatible pair never touches the hash at
all). `Compatible` is documented precisely as **necessary, not
sufficient**, for `concatenate + remap`: Design §7's own second clause
(cluster-assignment compatibility with a merged, rebalanced navigation
tier) is separate, harder, still-unimplemented work this function does not
attempt — a merge planner that sees `Compatible` still has real work left
before it can safely choose `concatenate + remap` over `rebuild`, and this
RFC does not claim otherwise.

**How this could be wrong — the adversarial review this amendment is
required to pass, per `CLAUDE.md` §3, before this is treated as settled.**

*Does the metadata slot's format collide with anything already shipped?*
Checked directly, and one real near-miss caught before it shipped: the
navigation tier already has a 4-byte reserved field at offset 4 (`spec/
vectors.md` §3), and the tempting move was to repurpose it for
`replication_policy`/`max_replicas` rather than add new bytes. Rejected,
deliberately: that reserved field's own normative text says a reader "MUST
NOT interpret" nonzero bytes there — a promise made to any future format
version, not merely an implementation convenience — and this project has
no version-negotiation mechanism that would let a reader safely distinguish
"this reserved field now means something" from "this reserved field is
still genuinely inert," the exact ambiguity the reserved-field convention
exists to avoid. Appending a brand-new, always-present, fixed-size trailer
after `cluster_dir` instead means no existing byte's meaning changes for
anyone — purely additive. The real cost: every navigation-tier blob grows
by exactly 8 bytes, unconditionally, including the ones this RFC's own
worked example and golden file already pinned — both were updated in this
same pass (`conformance/vectors/toy-navigation-tier.bin`, 568 → 576 bytes;
Worked example, above), and `crates/strand-vector/tests/navigation.rs`'s
`cluster_dir_region` slicing was corrected to bound itself explicitly
(it previously sliced "to the end of the blob," silently correct only
because `cluster_dir` used to *be* the end of the blob).

A second near-miss, caught by this same review before implementation: an
earlier draft of this trailer also stored `distinct_row_id_count` directly
(8 more bytes), reasoning that a reader needs it to compute the realized
replication factor. It doesn't: the flat-vector blob's own dense-per-row-
id contract (Design §5 — every local ordinal present) means this field's
distinct vector count *is* the segment's `row_id_count`, already decoded
from the container's hotcache before this blob family is even touched
(invariant 3's own cold-open sequence). Storing it again would have been
redundant wire bytes for an already-derivable quantity, exactly what
invariant 8's novelty-budget discipline argues against. Removed before any
code was written against it.

*Does the construction algorithm add real cold-open byte cost the napkin
math needs to update?* Two separate answers, both stated in Napkin math
above. The trailer itself: yes, a fixed 8 bytes per navigation tier,
independent of `num_clusters` — immaterial at any real scale (the
1,000,000-vector napkin-math total moves from `130,976,400` to
`130,976,416`, `+16` once the pre-existing 8-byte header-omission in that
particular sum is also closed in the same edit; the "bytes fetched at
open" figure moves from `12,384,408` to `12,384,416`). Real *usage* of
closure replication: this was already the single largest cost lever this
RFC's own Napkin math names ("Replication's cost," ≈2.27× the budget at a
realistic replica-8-equivalent density) — that arithmetic is unchanged by
this amendment, because it was never about the metadata slot's own bytes;
it was always about the posting-list bytes real replication adds, which
this amendment now makes a real, per-segment, reader-visible, tunable
number instead of an assumed constant, which is the entire point of M2-1.

*The RNG-rule call-site gap.* Named precisely above and in `references/
spann-closure-assignment-algorithm.md`: this rule is strictly a redundancy
*reducer* (it only ever removes candidates the epsilon test already
accepted, never adds any), so an implementation that got its exact
call-site fidelity wrong would misjudge the *realized* replication factor
a real dataset produces, not the format's correctness or its worst-case
byte-cost bound — `max_replicas` alone already bounds that regardless of
whether the RNG rule fires. A real, bounded-consequence gap, not a
load-bearing one.

*Nearest grave from `docs/lineage.md`, per `CLAUDE.md` §3's requirement:*
the Optane-era formats — baking a specific, era-bound assumption into wire
bytes that becomes unusable when the assumption breaks. The mitigation is
already structural, not merely stated: `replication_policy` is an open,
forward-compatible enum, and a reader encountering an unrecognized value
(a future, non-SPANN replication algorithm) does not reject the blob —
query resolution's own row-id deduplication (Design §6 step 3) already
tolerates duplicate row-ids regardless of *why* they're duplicated, so
decoding and querying stay correct even for a policy byte this chapter
never registers. A future replication algorithm needs a new policy value
and, if its own knobs don't fit `max_replicas`/`epsilon`, a follow-on RFC
extending the trailer — not a wire-format break. `max_replicas` (u8) and
`replication_epsilon` (f32) themselves are SPANN-shaped, not proven
general: a genuinely different algorithm might need different knobs
entirely, which this design accepts as the cost of registering one real,
literature-grounded algorithm now rather than inventing a premature
abstraction over algorithms that don't exist yet (invariant 8).

*What this amendment explicitly does not resolve*, named in Non-goals and
Open questions above: **compaction-time re-replication** (this RFC's
construction algorithm runs at initial segment build time; whether and how
closure replicas should be recomputed when `rebalance` moves a vector's
primary cluster at merge time is real, separate, unimplemented work) and
**cross-segment codebook sharing** (`docs/roadmap.md` M2-8, unchanged by
this amendment).

**Real tests, including a hand-checked worked example, per `CLAUDE.md`
§3's "start from usage, not structure."**
`crates/strand-vector/tests/closure_replication_end_to_end.rs` has two:
`closure_replication_hand_checked_byte_layout` — 3 small vectors, 2
hand-placed centroids chosen so distances reduce to one-coordinate
arithmetic, a real call to `closure_replicate` and `group_by_cluster`, and
every resulting byte checked exactly (directory offsets, both clusters'
row-id arrays including the replicated row-id 200 appearing in both, and
the trailer's decoded fields); and
`a_boundary_vector_is_found_via_either_cluster_and_deduplicated_by_query_
resolution` — real k-means on two well-separated blobs plus one vector
placed exactly at their midpoint, real rotation and 1-bit RaBitQ
quantization, a real query at the midpoint scanning both clusters, and an
assertion that the boundary vector's row-id survives query resolution's
deduplication exactly once. `crates/strand-vector/src/closure.rs` and
`crates/strand-vector/src/navigation.rs` carry unit tests for the
algorithm's edge cases (max-replicas capping, a tight epsilon rejecting a
far candidate, an unrecognized policy byte decoding safely) independently.

**A real, re-measured byte-budget confirmation, not left to arithmetic
alone.** `bench/src/vector_cold_open.rs`'s extrapolation formula was
updated to add the new 8-byte trailer, and the benchmark was re-run
against real MinIO at the same 10,000-vector/400-cluster scale
`bench/results/vector-cold-open.json` already used. Real, measured result:
the descriptor and navigation-tier blobs now total **1,238,816 bytes**
(**1.2388%** of the 100 MB budget, up from the prior 1,238,808 bytes by
exactly the trailer's 8 bytes — confirming the code, not just the
arithmetic), still **3 GETs/open**. The 1,000,000-vector extrapolation now
gives **12,384,416 bytes**, matching this section's hand-computed
12,384,416 figure to the byte. This same re-run's whole-segment-GET
latency (p50 = 47.6ms, p90 = 57.7ms, p99 = 72.8ms, n = 30, real numbers
in `bench/results/vector-cold-open.json`) is lower than the original run's
(p50 = 92.6ms, p90 = 102.2ms, p99 = 114.1ms) — most plausibly host-load
variance from this project's shared build infrastructure being under heavy
concurrent load during this specific re-run, not a claim that anything
about the Range-GET limitation changed; both runs share the same
localhost-MinIO, no-injected-latency caveat and the same whole-segment-GET
limitation named above. Real-network tail latency remains the still-open
placeholder `CLAUDE.md` §7 already names.

Sections updated: Non-goals (the closure-replication bullet marked
resolved), Design §3 (gained the `replication_descriptor` trailer), Design
§6 (the stale "metadata slot not yet designed" parenthetical corrected),
Worked example (nav-tier total 568 → 576 bytes, trailer bytes spelled
out), Napkin math (the open-bytes and total-tier-1 figures recomputed by
`+16`; the real-measurement paragraph re-run and updated), Invariant-11
checklist (endianness and golden-files bullets), Open questions (item
marked done, with compaction re-replication and codebook sharing named as
the real remaining gaps), and this Discussion entry.
`conformance/vectors/toy-navigation-tier.bin` regenerated (576 bytes).
`docs/ledger.md` and `docs/roadmap.md` M2-1 entries updated to match.

**How this could be wrong — the M2-8 amendment's own adversarial review,
required per `CLAUDE.md` §3 before this is treated as settled.**

*Does the identity mechanism actually catch every real incompatibility?*
Within the scope Design §7 itself defines (byte-identical descriptor
fields), yes, by construction: the hash covers every byte Design §7 names
as load-bearing, in full, so any bit difference in any of those fields
changes the hash (barring a collision, addressed next) and any difference
in the four cheap scalar fields is caught even before the hash is
consulted. What this mechanism does **not** catch, because Design §7
itself does not claim it: two descriptors that are byte-identical but
whose *codes* were nonetheless produced inconsistently by a buggy writer
(e.g., a writer that serialized the correct rotation but applied a
different one when quantizing) — this is a writer-correctness bug outside
any wire-format check's reach, no different from any other codec's
implementation-fidelity risk (RFC 0007's postings decode carries the same
class of unstated assumption: the format can verify structure, not that a
writer's quantizer matches its own declared descriptor).

*Could two genuinely different codebooks hash-collide, or share a
generation ID by coincidence?* The generation-ID alternative was already
rejected above independent of this question, so only the hash-collision
risk applies. XxHash3-64 is a non-cryptographic hash with a 64-bit output;
a birthday-bound collision probability of roughly 2⁻³² becomes
non-negligible only past billions of *distinct* codebooks being compared
pairwise within one planning run, a scale this format's segment-count
figures (`CLAUDE.md` §7: "on the order of a hundred segments" for the M3
benchmark, ~760,000 vectors/segment for the whole corpus) are nowhere near
— and, more importantly, this is the same non-cryptographic-collision risk
class this project already accepts for chunk checksums under invariant 11
("every chunk carries a declared checksum, default xxHash3-64") without
additional mitigation, so this amendment does not invent a new risk
tolerance, it applies the project's existing one to a second use of the
same algorithm. Stated precisely rather than hidden: this check is a
*correctness aid* for a trusted writer's own merge planning, not a
security boundary against an adversarial descriptor crafted to collide —
no part of this project's threat model treats segment producers as
adversarial, and this amendment does not change that.

*Is the check cheap enough to actually run before every merge without
becoming its own bottleneck?* Argued quantitatively above (the `O(N)`-
build/`O(1)`-compare paragraph); the one caveat worth stating plainly is
that `MatrixRotator`'s multi-megabyte `rotation_payload` makes the
*first* identity computation per segment non-trivial (not `O(1)`,
genuinely `O(payload size)`) — real, bounded, one-time cost, not
disguised as free, and the reason this design caches the computed
identity rather than re-hashing on every comparison.

*Nearest grave from `docs/lineage.md`, per `CLAUDE.md` §3's requirement:*
BitFunnel and the other formats that conflated a cheap structural check
with a full semantic guarantee, then discovered the gap in production —
the risk here is a future caller reading `CodebookCompatibility::
Compatible` as "safe to merge" rather than "safe to consider merging,"
skipping Design §7's cluster-assignment clause. Mitigated the same way the
M2-1 replication-policy amendment mitigated its own nearest-grave risk:
structurally, not just by a comment — `CodebookCompatibility::Compatible`'s
own doc comment states the necessary-not-sufficient scope directly at the
call site, and the function's name (`check_compatibility`, not
`safe_to_merge` or similar) does not overclaim what it decides.

**Real tests, per `CLAUDE.md` §3's "start from usage, not structure."**
`crates/strand-vector/src/codebook.rs`'s own unit tests cover each
`CodebookMismatch` variant in isolation (dims, distance metric, bit width,
rotator type, and same-scalars-different-payload) plus the reusable-
identity shape a real merge planner uses. The task's own requirement — two
segments with deliberately compatible and deliberately incompatible
codebooks — is met at the segment level, not only the descriptor-bytes
level, in the new
`crates/strand-vector/tests/codebook_compatibility_across_segments.rs`:
three tests build real, independently-committed, footer/hotcache-
decodable segments via `strand-core`'s actual `SegmentBuilder` (the same
real-segment discipline `segment_assembly.rs` established), extract each
segment's descriptor blob back out through the real registry-lookup path,
and confirm `check_descriptor_compatibility` correctly judges (1) two
segments sharing one real, RNG-drawn codebook as `Compatible`, (2) two
segments with independently-trained real codebooks — same `dims`,
`bit_width`, `rotator_type`, `distance_metric`, genuinely different
`rotation_payload` from two different RNG seeds — as
`Incompatible(ContentHash)`, the exact "scalars agree, payload doesn't"
case a coarser check would miss, and (3) two segments with different
`dims` entirely as `Incompatible(Dims)`, the cheap short-circuit case.

**What this amendment explicitly does not resolve**, named in Non-goals,
"How this could be wrong" item 4, and Open questions above, and left for
M3-1 (`docs/roadmap.md`): the codebook-sharing **policy** question (whether
STRAND mandates one shared codebook per table by convention, or requires
compaction to requantize when segments disagree), **cluster-assignment
compatibility** with a merged, rebalanced navigation tier (Design §7's
second `concatenate + remap` clause), and **the merge-planner code path**
itself, which does not exist anywhere in this codebase yet — this
amendment gives it a compatibility function to call, not a caller.

`cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D
warnings` both clean. Sections updated: Non-goals (the cross-segment
codebook bullet), "How this could be wrong" item 4 (marked partially
resolved), Open questions (the compatibility-check half struck as done,
the policy half restated precisely), and this Discussion entry.
`docs/ledger.md` and `docs/roadmap.md` M2-8 entries updated to match.
