# Vectors — cluster-native cold-open index

Normative for STRAND v0.1. Defines the vector blob family: flat full-precision
vectors, the RaBitQ quantization descriptor, the cluster navigation tier, and
the cluster posting lists that together make up STRAND's cold-native ANN
index. Approved by RFC 0010 (`rfcs/0010-vector-blob-cluster-family.md`); this
chapter states the settled result — see the RFC for the worked example, the
napkin math, alternatives considered, and the adversarial review. Registered
in `spec/container.md` §9: `family_id = 3` ("vector"), `blob_type_id = 0`
(flat vectors), `blob_type_id = 1` (quantization descriptor), `blob_type_id =
2` (cluster navigation tier), `blob_type_id = 3` (cluster posting lists).

Reference implementation: none yet — `crates/strand-vector` is created at
M2 implementation time (`docs/milestones.md`); this chapter is normative
ahead of code, per this project's RFC-then-implement workflow (`CLAUDE.md`
§3). Golden files: none yet; RFC 0010's worked example is the first
`conformance/vectors/` vector once implemented.

This chapter registers **1-bit RaBitQ only**. Multi-bit Extended-RaBitQ is
explicitly out of scope (RFC 0010 Non-goals) and requires its own follow-on
RFC before any `bit_width` other than `1` is valid.

## 1. Scope: one vector column, four blobs, one shared quantization descriptor

A field (vector column) registers exactly one quantization-descriptor blob,
exactly one cluster-navigation-tier blob, and exactly one cluster-posting-list
blob. If reranking is enabled for that column, it additionally registers
exactly one flat-vector blob. All four blobs are `storage-class: raw-mappable`
(invariant 10), `alignment = 8` (`spec/container.md` §5); `blob_type_id ∈
{1, 2, 3}` are `tier: cold-fetchable`,
`blob_type_id = 0` is `tier: n/a` (invariant 7 — fetched only for reranking,
never part of the cold-open wave or its byte budget). The quantization
descriptor and navigation tier are fetched wholesale at open;
`blob_type_id = 3` (cluster posting lists) is `cold-fetchable` in the sense
that its per-cluster byte ranges are addressable without a further round
trip, not in the sense that the whole blob is fetched at open — a query
fetches only its selected `nprobe` clusters' ranges (§6).

## 2. Quantization descriptor (`blob_type_id = 1`)

Fixed 16-byte header, plus a variable-length trailing `rotation_payload`.

| offset | size | field                      | notes                                                                 |
| ------ | ---- | ---------------------------- | ------------------------------------------------------------------------ |
| 0      | 4    | `dims`                        | u32; true (unpadded) vector dimensionality                               |
| 4      | 4    | `padded_dims`                 | u32; `dims` rounded up per the registered rotator type's padding rule    |
| 8      | 1    | `distance_metric`             | u8: `0` = L2, `1` = inner product, `2` = cosine                          |
| 9      | 1    | `bit_width`                   | u8; MUST be `1` in v0.1                                                  |
| 10     | 1    | `rotator_type`                | u8: `0` = `MatrixRotator`, `1` = `FhtKacRotator` (**default**)           |
| 11     | 1    | reserved                      | MUST be zero; a reader MUST NOT reject nonzero (future format versions) but MUST NOT interpret it |
| 12     | 4    | `rotation_payload_length`     | u32                                                                       |
| 16     | `rotation_payload_length` | `rotation_payload` | bytes; realized random state, never a seed (§2.1)                        |

### 2.1 Rotation payload, by `rotator_type`

`rotator_type = 1` (`FhtKacRotator`, default): `padded_dims` is `dims`
rounded up to the nearest multiple of 64. `rotation_payload` is exactly
`4 * padded_dims / 8` bytes: four Rademacher sign sequences, packed as
plain bytes (8 sign bits per byte), in the same flat order the registered
reference implementation's own state buffer uses. A writer MUST realize
this state from a real source of randomness and serialize it verbatim; a
reader MUST NOT attempt to regenerate it from any seed — none is carried,
by design (RFC 0010 Design §2, Alternatives considered).

`rotator_type = 0` (`MatrixRotator`, registered, non-default): `padded_dims`
is `dims` rounded up to the nearest multiple of 64 — the same rule
`FhtKacRotator` uses, and **not** the reference implementation's own
(unpadded) convention for this rotator type; STRAND requires this so both
registered rotator types share one `padded_dims` semantics for §4's code
region and so every offset this family computes stays a multiple of 8 (§1's
`alignment = 8`). `rotation_payload` is exactly
`dims * padded_dims * 4` bytes: the realized `dims × padded_dims` row-major
float32 orthogonal rotation matrix, serialized verbatim. As with
`FhtKacRotator`, the *realized* matrix is always serialized, never a seed —
the matrix's own construction (QR decomposition of a random Gaussian
matrix) is not guaranteed bit-reproducible across floating-point library
implementations, so a seed alone would not let two conforming readers
regenerate identical bytes (RFC 0010 Design §2).

A reader MUST reject a quantization-descriptor blob whose
`rotation_payload_length` does not match the value the formula above
computes from `dims`/`padded_dims`/`rotator_type`.

## 3. Cluster navigation tier (`blob_type_id = 2`)

| offset | size | field | notes |
| ------ | ---- | ------- | ----- |
| 0      | 4    | `num_clusters` | u32 |
| 4      | 4    | reserved | u32; writer MUST set zero; reader MUST NOT reject nonzero but MUST NOT interpret it |
| 8      | `num_clusters * padded_dims * 4` | `centroid_table` | row-major f32, one row per cluster, cluster-index order, full precision |
| —      | `num_clusters * 24` | `cluster_dir` | one 24-byte entry per cluster, same cluster-index order as `centroid_table` |

`cluster_dir` entry (24 bytes):

| offset | size | field | notes |
| ------ | ---- | ------- | ----- |
| 0      | 8    | `region_offset` | u64; byte offset into the cluster-posting-list blob where this cluster's region begins |
| 8      | 8    | `code_bytes_length` | u64; byte length of this cluster's quantized-code region; that cluster's row-id array begins immediately after, at `region_offset + code_bytes_length` |
| 16     | 4    | `vector_count` | u32; the row-id array's length is exactly `vector_count * 8` bytes |
| 20     | 4    | reserved | writer MUST set zero; reader MUST NOT reject nonzero but MUST NOT interpret it |

A reader MUST be able to resolve every selected cluster's full byte range —
`[region_offset, region_offset + code_bytes_length + vector_count * 8)` —
from this blob alone, with no further round trip (invariant 3).

## 4. Cluster posting lists (`blob_type_id = 3`)

The concatenation of each cluster's region, in `cluster_dir`'s cluster-index
order. Each region is the quantized-code region immediately followed by
that cluster's row-id array — co-located deliberately so one Range GET per
selected cluster suffices (RFC 0010 Design §4).

**Quantized-code region** (1-bit only, v0.1): `ceil(vector_count / 32)`
batches, each exactly `padded_dims * 4 + 384` bytes. Within a batch: the
first `padded_dims * 32 / 8` bytes are the 1-bit codes for up to 32 vectors,
packed per the registered FastScan-batched layout (the byte *offsets* of
this region and the three factor arrays below are adopted by reference,
invariant 8; the intra-batch bit/lane order of the code bits themselves is
**not** independently specified by this chapter — RFC 0010's own How this
could be wrong names this a real gap); followed by three arrays of 32
little-endian f32 values each — `f_add`, `f_rescale`, `f_error`, the
codec's own per-vector distance-correction factors, in that order. A
cluster whose `vector_count` is not a multiple of 32 still pays the full
per-batch cost for its last, partially filled batch (registered-codec
behavior, not fixed by this format). A writer MUST zero-fill every unused
lane's code bits and its corresponding `f_add`/`f_rescale`/`f_error` slots
in a partially filled batch (invariant 11 byte determinism).

**Row-id array**, per cluster: exactly `vector_count` little-endian u64
values, in ascending row-id order.

## 5. Flat vectors (`blob_type_id = 0`)

`row_id_count * dims * 4` bytes — note `dims`, not `padded_dims`. Row-major
f32, one row per local ordinal in the segment's row-id order, dense (every
local ordinal present). Fetched only to rerank a candidate set the cluster
scan already produced; never part of the cold-open wave.

## 6. Query resolution

1. Compute the query's distance to every row of `centroid_table` (already in
   hand from the cold-open wave — no I/O); select the `nprobe` closest
   clusters.
2. For each selected cluster, issue one Range GET for
   `[region_offset, region_offset + code_bytes_length + vector_count * 8)`
   against the cluster-posting-list blob. All ranges are resolvable from the
   navigation tier alone; a conforming reader issues them as one parallel
   wave (invariant 3).
3. Decode each fetched cluster's codes against the query (FastScan, for
   `bit_width = 1`) to produce a ranked candidate row-id set. Under
   closure replication a row-id can appear in more than one scanned
   cluster; a reader MUST deduplicate by row-id, keeping each row-id's
   best (closest) estimated distance across the clusters it appeared in.
4. Filter the deduplicated candidate set against the segment's deletion
   vector (invariant 2), discarding tombstoned row-ids, before returning
   results or reranking.
5. Optionally, fetch the flat-vector blob's rows for the surviving
   candidates and recompute exact distances (a second wave, outside the
   cold-open budget).

## 7. Merge semantics (invariant 1)

- **Flat vectors:** `concatenate + remap`.
- **Quantization descriptor:** merging two segments' posting lists without
  requantization requires their quantization descriptors to be
  byte-identical. If they differ, a merge MUST requantize (`rebuild`).
- **Cluster navigation tier (centroids):** `rebalance` (LIRE-style,
  SPFresh). The rebalancing algorithm itself is unspecified by this chapter
  (RFC 0010 Non-goals); only the merge-strategy label is settled here.
- **Cluster posting lists:** `concatenate + remap` when descriptors match
  and cluster assignments are compatible with the merged, rebalanced
  navigation tier; `rebuild` otherwise. A vector whose cluster assignment
  changes under rebalancing MUST have its code region moved with it — a
  per-cluster contiguous layout does not compose under a plain byte-level
  concatenation across a rebalance.

## 8. Distance metrics

`distance_metric` (quantization descriptor, offset 8) is `0` (L2/Euclidean),
`1` (inner product), or `2` (cosine). A writer using `2` MUST normalize
vectors before quantization; this chapter does not further specify
normalization mechanics, which are a write-side concern outside this
chapter's read-side scope.
