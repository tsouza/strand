# RFC 0011: Multi-bit Extended-RaBitQ (RaBitQ+) registration

- **Status:** Approved. Adversarial review re-fetched all five live source
  files this RFC's citations depend on (`rabitq_impl.hpp`, `estimator.hpp`,
  `query.hpp`, `pack_excode.hpp`, `pack_excode_dispatch.hpp`, plus
  `data_layout.hpp` for `ExDataMap`'s exact field layout) and independently
  re-derived the worked example from scratch in Python (not reading this
  RFC's own C++ transcription). Found every formula, constant, and
  byte-layout claim verbatim correct — no transcription drift anywhere —
  but found 1 Critical, 4 Important, and 2 Minor gaps, all fixed. Critical:
  the zero-residual case (`data == centroid` exactly, real for any
  singleton k-means cluster) makes `ex_bits_code`'s residual-normalization
  step a `0.0 / 0.0` division, producing `NaN` in every dimension before
  the reference's own `ip_resi_xucb == 0 → infinity` guard ever runs
  (`NaN == 0.0` is false, so the guard is silently bypassed) — the same
  class of bug `quantize.rs`'s own ACPR already found Critical and fixed
  for the 1-bit path, left unaddressed by this RFC's first draft; fixed
  with an explicit pre-normalization zero-norm guard producing the same
  degenerate `f_add_ex = 0.0`, `f_rescale_ex = -0.0` the 1-bit path's own
  fix already established (Design §3). Important (4): `QueryFactors`'s
  concrete new signature was unspecified even though `G_kbxSumq`, unlike
  `G_k1xSumq`, depends on `bit_width` — fixed with an exact signature
  (`QueryFactors::new(rotated_query, bit_width)`) and the correction that
  `G_add`/`G_error` are not `QueryFactors` fields at all today, just
  locally recomputed in `estimate_distance` (Design §4); `posting_list.rs`'s
  `code_bytes_length_for`/`read_cluster` length validation had no path to
  accept a `bit_width > 1` cluster without an `ex_bits` parameter — fixed,
  named explicitly (Design §2); `query.rs`'s `scan_selected_clusters` — the
  actual integration point — was never named, leaving the query-resolution
  edit at spec-prose abstraction only — fixed with a concrete
  implementation note (Design §5); the byte-determinism carve-out's exact
  edit target in `spec/vectors.md` §4 was described but not pinned to a
  specific paragraph — fixed (Design §3). Minor (2): the vendored
  reference file didn't note `kTightStart` is `float`-typed, not `double`
  — fixed; a worked-example `f_error_ex` digit discrepancy the reviewer's
  independent re-derivation found turned out not to appear in this RFC's
  own text at all (it's an explicitly unstored, discarded value, Design
  §2) — confirmed moot, no edit needed.
- **Milestone:** M2 — Vectors, cluster-first (`docs/milestones.md`)
- **Spec chapters produced:** additively extends `spec/vectors.md` §2
  (relaxes `bit_width`'s "MUST be 1" constraint) and §4 (registers a new
  ex-code sub-region inside the existing cluster posting-list blob,
  `blob_type_id = 3`). No new blob type, no new `family_id`/`blob_type_id`
  registration, no descriptor byte-layout change.
- **Invariants exercised:** 1, 3, 7, 8, 9, 10, 11 (`CLAUDE.md` §5)

## Summary

RFC 0010 registered 1-bit RaBitQ only and named multi-bit Extended-RaBitQ
("RaBitQ+", the reference implementation's own name for this path — the
paper itself is titled generically, `references/rabitq-and-extended-
rabitq.md`) as a Non-goal, citing `docs/data-structures.md`'s own settled
kernel-selection principle: the multi-bit path computes distances as
**classical scalar quantization**, a genuinely different computation from
the 1-bit path's FastScan LUT machinery, and therefore cannot be silently
folded into RFC 0010's registration.

This RFC closes that Non-goal. It relaxes `spec/vectors.md` §2's
`bit_width` field from "MUST be 1" to "1 through 8, total bits per
dimension (1 sign bit plus 0 through 7 extra magnitude bits, `ex_bits =
bit_width - 1`)," and registers one new wire structure: an **ex-code
region**, appended inside the existing cluster posting-list blob's
per-cluster quantized-code region, present if and only if `bit_width > 1`.
Everything else about the format — the descriptor blob's byte layout, the
navigation tier, the cluster directory, the 1-bit FastScan batch region,
row-ids, merge semantics, query-resolution's outer shape — is unchanged.
This is deliberately the smallest wire-format extension that makes RaBitQ+
"the algorithm's concern, not this container-layer RFC's" (RFC 0010 Design
§4's own framing, extended here rather than reopened) actually
implementable, mirroring how RFC 0008 (positions) extended RFC 0007's
already-shipped postings layout rather than redesigning it.

Grounded against a newly vendored primary source
(`references/rabitq-library-multibit-quantization-source.md`, from the
same `RaBitQ-Library` repository RFC 0010 already audited Apache-2.0):
the encode-side `ex_bits_code_with_factor` algorithm, the query-side
`split_distance_boosting` formula, and the honest discovery that the
reference implementation computes but never persists or reads a per-vector
`f_error_ex` — the query path reuses the 1-bit region's already-stored
`f_error`, scaled by `1 / 2^ex_bits`, instead.

## Design

### 1. Descriptor blob (`spec/vectors.md` §2) — one field relaxed, nothing added

`bit_width` (offset 9, existing field) becomes: u8, MUST be in `1..=8`.
`bit_width = 1` is the unchanged RFC 0010 path (no ex-code region exists,
byte-for-byte identical to today). `bit_width > 1` additionally activates
the ex-code region defined below, with `ex_bits = bit_width - 1` extra
magnitude bits per dimension.

The upper bound of 8 is a deliberate narrowing, not the reference
library's own limit (which the ex-code packing dispatch shows supports
`ex_bits` up to 8, i.e. 9 total bits — `references/rabitq-library-
multibit-quantization-source.md`). Registering only up to `bit_width = 8`
covers the reference library's own demonstrated recall sweet spot — "4-bit,
5-bit and 7-bit quantization usually suffices to produce 90%, 95% and 99%
recall respectively without reranking" (`references/rabitq-library-index-
overview.md`) — with one bit of headroom, while avoiding standardizing a
9-bit-per-dimension code this session found no recall justification for.
**A real, flagged ambiguity in the vendored source**: that recall
sentence does not state whether "4-bit" counts `bit_width` (this RFC's
convention: 1 sign bit + 3 ex_bits) or `ex_bits` alone. This RFC assumes
`bit_width` (the more common convention in the scalar-quantization
literature, where "N-bit quantization" names the total per-dimension
storage), but this is not independently confirmed by any fetched source
and is stated here as exactly that: an assumption, not a grounded fact,
per `CLAUDE.md` §2. It does not change anything this RFC registers
either way — the wire format is parameterized by `bit_width` directly, not
by a recall target — only the Napkin math section's mapping from "the
reference's recall claims" to "how many extra bytes that costs" carries
the ambiguity forward, flagged at the point it's used.

Rotation is unaffected: the same rotated residual `data - centroid` (in
rotated space, per the existing precondition `quantize.rs`'s module doc
already states) feeds both the 1-bit sign code and the new ex-code.

### 2. Cluster posting lists (`spec/vectors.md` §4) — one new sub-region

Today: per-cluster region = `[quantized-code region][row-id array]`, where
"quantized-code region" is exactly the 1-bit FastScan batch-data region.
This RFC redefines "quantized-code region" as:

`[1-bit FastScan batch-data region (unchanged, always present)][ex-code
region (new; present iff bit_width > 1; length 0 and entirely absent
iff bit_width = 1)]`

followed by the row-id array, unchanged. `ClusterDirEntry`'s existing
fields (`region_offset`, `code_bytes_length`, `vector_count`) need no new
field: `code_bytes_length` already covers "however many bytes the
quantized-code region actually is," which now includes the ex-code region
when present. A reader that only understands `bit_width = 1` and never
inspects the descriptor's `bit_width` field would misinterpret a
`bit_width > 1` cluster's `code_bytes_length` as pure 1-bit batch data and
either overrun or leave bytes unconsumed — exactly why a reader MUST check
`bit_width` before parsing the quantized-code region at all, stated as a
normative MUST in the spec text, not left implicit.

**Ex-code region layout, per cluster.** Unlike the 1-bit region, this
region is **not batched** — `references/rabitq-library-ivf-and-batch-
layout-source.md`'s own `ExDataMap<T>` is explicitly per-vector, and
nothing in the classical-scalar-quantization kernel benefits from
FastScan-style batching (invariant 9: no SIMD LUT requirement drives this
path). `vector_count` entries, in the same ascending row-id order as the
row-id array and the 1-bit region's vector ordering, each exactly
`padded_dims * ex_bits / 8 + 8` bytes (the division is always exact:
`padded_dims` is always a multiple of 64 per both registered rotator
types, `spec/vectors.md` §2.1, so `padded_dims * ex_bits` is always a
multiple of 8 for any `ex_bits` in `0..=7`):

- `ex_code`: `padded_dims * ex_bits / 8` bytes. `padded_dims` per-dimension
  `ex_bits`-wide unsigned integer codes (range `0..2^ex_bits`), packed
  **STRAND's own way** (see Alternatives considered): MSB-first within
  each byte, dimensions in ascending order, bits written contiguously
  across byte boundaries with no per-dimension padding — the same
  MSB-first-per-byte convention `quantize.rs`'s `pack_binary` already uses
  for the 1-bit code, chosen here for in-codebase consistency even though
  the two packings serve unrelated algorithms.
- `f_add_ex`: little-endian f32.
- `f_rescale_ex`: little-endian f32.

**No `f_error_ex` field.** The reference implementation's own encoder
computes one (`references/rabitq-library-multibit-quantization-source.md`),
but its own query path never reads it — the error bound at query time is
computed from the 1-bit region's already-stored `f_error`, scaled by
`1 / 2^ex_bits` (Design §4, below). Persisting a value nothing ever reads
would be real, silent wire-format waste; this RFC does not register it.
`ExDataMap<T>::data_bytes() = padded_dim * ex_bits / 8 + sizeof(T) * 2`
already reflects exactly this — two stored factors, not three — confirming
this is the reference implementation's own real behavior, not a STRAND
simplification invented here.

Because the region isn't batched, there is no partial-batch zero-fill
concern (RFC 0010 Design §4's own invariant-11 fix) — every entry
corresponds to exactly one real vector.

**Concrete implementation note.** `posting_list.rs`'s existing
`code_bytes_length_for(vector_count, padded_dims)` and
`PostingListReader::read_cluster`'s own length validation compute the
1-bit-only region's expected byte length purely from `padded_dims`, with
no `ex_bits` parameter — both MUST gain one (`ex_bits: u8`, `0` for
`bit_width = 1`, reproducing today's exact byte count unchanged) so a
`bit_width > 1` cluster's directory-declared `code_bytes_length` validates
against the correct, larger expected length (1-bit region plus
`vector_count * (padded_dims * ex_bits / 8 + 8)`) instead of being
rejected as corrupt.

### 3. Encode-side algorithm (non-normative factor bytes, same carve-out as the 1-bit region)

`crates/strand-vector/src/quantize_ex.rs` (new module) transcribes
`ex_bits_code_with_factor` (`references/rabitq-library-multibit-
quantization-source.md`) verbatim: `best_rescale_factor`'s event-driven
greedy search for the rescale factor `t`, `quantize_ex`'s clamp-and-round
into `0..2^ex_bits`, the sign-complement step for negative-residual
dimensions, and the `f_add_ex`/`f_rescale_ex` formulas (metric-aware,
mirroring `one_bit_code_with_factor`'s structure with `cb = -(2^ex_bits -
0.5)` in place of `-0.5`).

`best_rescale_factor` is a genuine numerical search (a priority-queue-
ordered greedy walk breaking ties on floating-point comparisons), not a
closed-form formula — a materially different situation from the 1-bit
path's `quantize_one_bit`, whose code and factors are pure closed-form
arithmetic with one correct answer. This RFC extends `spec/vectors.md`
§4's existing carve-out (today scoped to `f_add`/`f_rescale`/`f_error`)
to cover the ex-code region's codes *and* factors together: **a reader
MUST NOT assume two independent, conforming writers produce identical
ex-code region bytes for the same logical vectors**, even given the same
scalar reference algorithm, because floating-point tie-breaking in the
greedy search is not pinned to a specific evaluation order by this RFC.
This is a real, wider carve-out than the 1-bit region's (which only
concerns factor *rounding*, not code *values* — `quantize_one_bit`'s
binary code is a pure sign function with no search involved). Registering
a byte-exact, cross-platform-deterministic tie-break rule for
`best_rescale_factor` was considered and rejected (Alternatives
considered) as unnecessary complexity for a quantity whose correctness
(the error-bound guarantee) is self-consistent regardless of which valid
`t` a writer's search converges to — precisely the same reasoning RFC
0010's own Non-goals used to leave k-means construction-side and
unstandardized.

Two performance-only optimizations in the reference (`faster_quantize_ex`
with a precomputed `t_const`, and `get_const_scaling_factors`'s own
100-sample Gaussian calibration) are construction-side speedups that
change *which* valid `t` a writer converges to, not the format — this RFC
registers only the exact per-vector `best_rescale_factor` search as the
normative reference algorithm and leaves the faster approximation as
unregistered, real, separate writer-side work (Non-goals).

**Critical correctness gap, found and closed during this RFC's own
adversarial review: the zero-residual case is unaddressed by the
reference and produces `NaN`, silently defeating the guard the reference
does have.** `ex_bits_code` normalizes the residual before quantizing —
`abs_res = abs(residual / ||residual||)` — and a zero residual (`data ==
centroid` exactly: a real, expected case for any singleton k-means
cluster, the identical scenario `quantize.rs`'s own ACPR found Critical
for the 1-bit path, `docs/ledger.md`'s entry) makes this a `0.0 / 0.0`
division, producing `NaN` in every dimension before `best_rescale_factor`
or `ex_bits_code_with_factor`'s own `ip_resi_xucb == 0 → infinity` guard
ever runs. That guard cannot catch this: by the time `ip_resi_xucb` is
computed, `xu_cb` is already `NaN` (derived from the already-`NaN`
`ex_code`), and IEEE 754 makes `NaN == 0.0` false, so the existing guard
is silently bypassed and `NaN` propagates onto the wire in `f_add_ex`/
`f_rescale_ex`.

**Fix, normative for this RFC's registered algorithm (a real, documented
divergence from the reference, the same class of divergence `quantize.rs`
already has for its own `.max(0.0)` clamp):** a writer MUST check the
residual's L2 norm for exactly zero *before* the normalization step and,
if zero, skip `best_rescale_factor`/`quantize_ex` entirely, producing the
defined degenerate output `ex_code = [0; padded_dims]`, `f_add_ex = 0.0`,
`f_rescale_ex = -0.0` — bit-for-bit the same degenerate values the 1-bit
path's own existing guard already produces for this case (`f_add = 0.0`,
`f_rescale = -0.0`, per `quantize.rs`'s ACPR entry). This is not an
arbitrary placeholder: substituting these values into the boosted
formula gives `ex_dist = 0 + G_add + (-0) * (...) = G_add`, which is
exactly correct — a zero residual means the database vector *is* the
centroid, so the true query-to-vector distance is exactly the
query-to-centroid distance (`G_add`) regardless of how many magnitude
bits were spent recording zero magnitude, with zero estimation error.

**A second, related but distinct precision note, not a bug**: the 1-bit
path's own stored binary code uses strict `residual > 0` (so a
*single* zero-valued dimension, in an otherwise-nonzero residual, gets
bit `0`), while `ex_bits_code_with_factor`'s internal `total_code`
reconstruction (used only to compute `xu_cb` for the factor formulas, never
stored) uses `residual >= 0` for that same per-dimension sign bit. This
asymmetry is the reference's own inherited behavior at the single-
dimension boundary, not something this RFC introduces or needs to correct
— it only becomes the Critical case above when the *entire* residual
vector is zero, handled by the explicit guard.

This RFC's byte-determinism carve-out (below) is the specific edit
`spec/vectors.md` §4 needs: **extend that section's existing paragraph**
("A reader MUST NOT assume two conforming writers produce identical
**factor** bytes for the same logical vectors unless they share that same
scalar reference — this chapter's own byte-layout guarantee covers
structure, not the factors' numeric provenance") to also name the ex-code
region's **codes**, not only its factors, in the same sentence — one
paragraph edited in place, not a new one added.

### 4. Query-side distance estimator (extends `estimate.rs`)

Given a rotated query, a rotated centroid, and one vector's stored 1-bit
factors (`f_add`, `f_rescale`, `f_error` — already read for the baseline
1-bit estimate, unconditionally) plus, when present, its ex-code region
(`ex_code`, `f_add_ex`, `f_rescale_ex`):

```
ip_x0_qr      = dot(rotated_query, unpacked 1-bit code bits)   -- already computed:
                                                                   estimate.rs's code_query_ip
ex_ip         = dot(rotated_query, ex_code as plain per-dimension integers)
G_kbxSumq     = sum(rotated_query) * -( (2^bit_width) - 1 ) / 2      -- bit_width = ex_bits + 1
ex_dist       = f_add_ex + G_add
                + f_rescale_ex * ( 2^ex_bits * ip_x0_qr + ex_ip + G_kbxSumq )
error_bound_ex = f_error * G_error / 2^ex_bits                        -- reuses the 1-bit f_error
[lb, ub]      = ex_dist ∓ error_bound_ex
```

`G_add` and `G_error` are computed exactly the way `estimate_distance`
already computes them today (`estimate.rs` lines 146–159: `norm^2`/`-ip`
for `G_add`, `norm` for `G_error`, metric-aware) — note precisely, since
`estimate.rs`'s current `QueryFactors` struct does **not** store these
today; they are local values `estimate_distance` recomputes on every call
from `query`/`centroid`. This RFC reuses that same computation for the
boosted formula, unchanged; nothing about `G_add`/`G_error` needs to move
into `QueryFactors`.

`G_kbxSumq` is new and, unlike `G_k1xSumq`, depends on `bit_width` (via
`c_b`). Concrete signature change this RFC specifies:
`QueryFactors::new` gains a required `bit_width: u8` parameter
(`QueryFactors::new(rotated_query: &[f32], bit_width: u8) -> Self`) and
computes both `g_k1x_sumq` (unchanged formula, always computed regardless
of `bit_width`, matching the reference's own `SplitBatchQuery`/
`SplitSingleQuery` constructors which precompute both constants
unconditionally) and a new `g_kbx_sumq: f32` field. Existing 1-bit-only
callers pass `bit_width = 1` (making `c_b` degenerate to the same `-0.5`
as `c_1`, so `g_kbx_sumq` becomes numerically identical to `g_k1x_sumq` in
that case — harmless, simply unused by the unmodified 1-bit estimate
path). A reader with `bit_width = 1` and no ex-code region present
continues to call the existing, unmodified `estimate_distance` exactly as
today; `g_kbx_sumq` is only read by the new boosted path below.

The boosted estimate needs a second per-vector input the existing
`QuantizedVector` (1-bit code + `f_add`/`f_rescale`/`f_error`) does not
carry: a new struct, `pub struct ExQuantizedVector { pub ex_code: Vec<u8>,
pub f_add_ex: f32, pub f_rescale_ex: f32 }` (`quantize_ex.rs`), and a new
function `pub fn estimate_distance_boosted(quantized: &QuantizedVector, ex:
&ExQuantizedVector, ex_bits: u8, query: &[f32], centroid: &[f32],
query_factors: &QueryFactors, metric: MetricType) -> DistanceEstimate`
(`estimate.rs`) implementing the formula above, reusing `quantized.f_error`
for `error_bound_ex` and `code_query_ip` for `ip_x0_qr` exactly as
specified. `estimate_distance` itself is unchanged — a caller decides
which function to call based on whether the descriptor's `bit_width > 1`
and the cluster's ex-code region is present, not based on a runtime branch
inside a single merged function.

**A deliberate generalization from the reference, stated plainly.** The
reference implementation's own query path computes only a lower bound
(`low_dist`) at this step, never an upper bound — every call site reads
`low_dist` alone, used for lower-bound pruning during a max-heap search
(`references/rabitq-library-multibit-quantization-source.md`). This RFC
instead defines a symmetric two-sided bound, `ex_dist ∓ error_bound_ex`,
matching `estimate.rs`'s existing 1-bit `DistanceEstimate` API
(`[lb, ub]`) for interface consistency within this codebase. This is sound
because the error term itself is not asymmetric in the underlying math —
`tmp_error`'s derivation (`references/rabitq-library-one-bit-quantization-
source.md`) bounds `|true - estimated|`, not a one-sided quantity — the
reference's own call sites simply never needed the upper half. Callers
that only need the lower bound (nprobe pruning, `query.rs`) are unaffected
either way.

### 5. Query resolution (`spec/vectors.md` §6, step 3)

Step 3 currently reads "Decode each fetched cluster's codes against the
query (FastScan, for `bit_width = 1`)." This RFC extends it: after the
FastScan 1-bit estimate, if the descriptor's `bit_width > 1` and the
cluster's ex-code region is present, a reader additionally computes the
boosted `ex_dist`/`[lb, ub]` above and MUST use it (not the 1-bit-only
estimate) as the candidate's ranked distance — the whole reason a writer
pays the extra bytes is a tighter estimate, and silently falling back to
the cheaper, looser 1-bit-only figure would make the feature invisible to
its own query path. A reader that does not implement the classical
scalar-quantization kernel at all (an intentionally minimal reader, say)
MUST reject a `bit_width > 1` descriptor rather than silently degrade to
1-bit-only ranking — `DescriptorError::UnsupportedBitWidth` already exists
for exactly this refusal (`descriptor.rs`), and this RFC does not weaken
it beyond widening the accepted range to `1..=8`.

**Concrete implementation note.** The actual integration point is
`query.rs`'s `scan_selected_clusters`, the function that already calls
`estimate_distance` for every candidate in a selected cluster today. It
MUST branch on the descriptor's `bit_width`: read the cluster's already-
fetched bytes for both the 1-bit region and (when `bit_width > 1`) the
ex-code region (via `PostingListReader::read_cluster`'s widened length
validation, Design §2), then call `estimate::estimate_distance_boosted`
in place of `estimate_distance` for that candidate when the ex-code region
is present. RFC 0010's own Design §6 was concrete enough to directly
drive `query.rs`'s original implementation; this RFC names the same
integration point explicitly rather than leaving it to be re-derived from
spec prose alone.

### 6. Merge semantics (`spec/vectors.md` §7) — unchanged, confirmed

The existing rule — "merging two segments' posting lists without
requantization requires their quantization descriptors to be
byte-identical; if they differ, a merge MUST requantize (rebuild)" —
already covers `bit_width` implicitly, since byte-identical descriptors
necessarily agree on `bit_width`. No new rule is needed; this RFC states
that explicitly rather than leaving it to inference.

## Worked example

`dim = 8`, `padded_dims = 8` (already a multiple of 64... — no: 8 is not a
multiple of 64. This worked example deliberately uses the same
unrealistically small `padded_dims = 8` RFC 0010's own worked example
used, for hand-checkability; a conforming real segment always has
`padded_dims` a multiple of 64 per `spec/vectors.md` §2.1, and this
example's byte formulas (`padded_dims * ex_bits / 8`) happen to stay exact
integers at `padded_dims = 8, ex_bits = 2` regardless, so the arithmetic
below is not affected by the toy dimensionality), `ex_bits = 2`
(`bit_width = 3`), `distance_metric = L2`.

`data = [1.0, -2.0, 3.5, 0.5, -1.5, 2.0, -0.25, 4.0]`, `centroid = [0.5,
-1.0, 2.0, 1.0, -1.0, 1.5, 0.0, 3.0]` — the same pair `quantize.rs`'s own
worked test case uses, so `residual = data - centroid = [0.5, -1.0, 1.5,
-0.5, -0.5, 0.5, -0.25, 1.0]`, matching the existing 1-bit worked example's
sign pattern (`0xA5`, `references/rabitq-library-one-bit-quantization-
source.md`).

Running the transcribed algorithm (verified by a standalone, compiled
C++ reimplementation of `ex_bits_code_with_factor`, `g++ -O2`, real
executed output, not hand-derived):

- `ex_code = [1, 1, 3, 2, 2, 1, 3, 2]` (each a 2-bit value, sign-complement
  already applied for the four negative-residual dimensions).
- Packed MSB-first per byte, dimensions ascending: bit sequence `01 01 11
  10 10 01 11 10` → `byte[0] = 0b01011110 = 0x5E`, `byte[1] = 0b10011110 =
  0x9E`.
- `f_add_ex = 21.200350467`, `f_rescale_ex = -0.794392523` (f64 from the
  reference transcription; a real writer stores these truncated to f32 per
  the wire format's own f32 requirement, the same precision handling
  `quantize.rs`'s own worked example already documents).

The same `(data, centroid)` pair under `distance_metric = InnerProduct`
produces the identical `ex_code` (sign and magnitude quantization do not
depend on the metric) with `f_add_ex = 0.943925234`, `f_rescale_ex =
-0.397196262` — confirming the metric only changes the factor formulas,
never the code, exactly mirroring the 1-bit path's own structure.

## Napkin math (`CLAUDE.md` §7)

This RFC's cost is **bytes fetched per selected cluster, not round
trips**: the ex-code region lives inside the same already-`cold-fetchable`
posting-list blob RFC 0010 registered, fetched in the same one Range GET
per cluster (invariant 3's one-wave rule is unaffected — no new blob, no
new GET). The real, honest cost is the byte-budget math (§7's provisional
100 MB per-segment figure) and the sizing law (`CLAUDE.md` §7's ~760,000-
vectors-per-segment figure) it feeds.

At `padded_dims = 768` (a realistic embedding width), each ex-code region
entry costs `768 * ex_bits / 8 + 8` bytes, added on top of RFC 0010's own
already-corrected, partial-batch-amortized 1-bit-only figure of ~131 bytes
per vector at a realistic 250-vectors/cluster average (RFC 0010's Approval
status line, ~131 MB per million 768d vectors):

| `bit_width` | `ex_bits` | claimed recall (assuming the `bit_width` reading, above) | added bytes/vector | combined bytes/vector | vectors/segment at the 100 MB budget |
| --- | --- | --- | --- | --- | --- |
| 1 | 0 | n/a (RFC 0010 baseline) | 0 | ~131 | ~760,000 (unchanged, `CLAUDE.md` §7) |
| 4 | 3 | ~90% | 296 | ~427 | ~234,000 |
| 5 | 4 | ~95% | 392 | ~523 | ~191,000 |
| 7 | 6 | ~99% | 584 | ~715 | ~140,000 |

**This is a real, substantial cost, stated plainly rather than softened.**
Reaching the reference's own claimed 99%-recall-without-reranking point
costs roughly 5.5× the bytes-per-vector of the 1-bit-only baseline, and
correspondingly shrinks the number of vectors that fit a single segment's
100 MB cold-open budget by roughly the same factor. Whether that trade is
worth it against RFC 0010's other lever — the existing, unconditional
optional reranking pass against full-precision vectors (`spec/vectors.md`
§6 step 5), which achieves near-100% recall at the cost of a second wave
outside the cold-open budget rather than a permanently larger first wave —
is a real, per-deployment tuning question this RFC does not resolve and
does not need to: both paths remain available, and this RFC's only job is
making the `bit_width > 1` path *legal and well-specified* wire format,
not recommending when to use it. `docs/data-structures.md`'s own framing
already anticipated this trade explicitly ("4/5/7-bit typically reaching
90/95/99% recall without reranking" — i.e., presented as an alternative
*to* reranking, not a supplement).

## Invariant-11 checklist

- **Endianness:** `f_add_ex`/`f_rescale_ex` little-endian f32, matching
  every other multi-byte wire value in this blob family.
- **Codec-variant provenance:** the ex-code packing is a STRAND-defined
  convention (MSB-first per byte, contiguous, no SIMD-interleave), fully
  pinned above — no ambiguity remains about which of several possible
  packings a byte sequence encodes.
- **Checksums:** unchanged — the ex-code region lives inside the same
  raw-mappable blob (`storage-class: raw-mappable`, invariant 10; no chunk
  compression, no checksum layer at this granularity, same as the rest of
  `blob_type_id = 3`).
- **Stochastic transform provenance:** none introduced — `best_rescale_
  factor` is deterministic given its floating-point inputs (no RNG), and
  the region's byte-level non-determinism across independent writers is
  explicitly carved out above (Design §3), the same treatment the 1-bit
  region's factors already receive.
- **Golden files:** none yet for this region specifically —
  `conformance/vectors/` gains one once implemented, per the existing
  M2 conformance-file convention.

## Alternatives considered

**Adopt the reference's own SIMD-shuffled ex-code packing
(`packing_2bit_excode` through `packing_7bit_excode`) instead of a
STRAND-defined plain packing.** Rejected. Two independent reasons, not
one: first, no portable scalar source exists for these six functions
anywhere in the fetched repository (`references/rabitq-library-multibit-
quantization-source.md`) — only AVX2/AVX512 intrinsics — so replicating
them exactly would mean guessing at intrinsic semantics the way
`rotate.rs`'s `flip_sign`/`kacs_walk` needed to, for a payoff this path
does not need: `docs/data-structures.md`'s own settled kernel-selection
principle routes `bit_width > 1` through **classical scalar-quantization
distance computation**, not FastScan LUT/register-shuffle machinery, so
there is no SIMD accumulation kernel on STRAND's side that this packing
would even feed. Second, and more fundamentally: baking a specific
vendor's SIMD register-shuffle pattern into a wire format is precisely the
Optane-era formats' mistake (`docs/lineage.md`) — "hardware-specific
choices baked into media layouts, unimplementable the day the hardware
died," the standing argument invariant 10 already generalizes into "no
vendor register width appears in spec text." A plain, portable,
byte-contiguous packing costs nothing at query time under the
classical-scalar-quantization kernel (a scalar reader unpacks
`ex_bits`-wide integers from a bitstream exactly as cheaply regardless of
interleave pattern) and remains implementable on any future hardware.

**Register a byte-exact, cross-platform-pinned tie-break rule for
`best_rescale_factor`'s priority-queue search**, so independent writers
would produce byte-identical ex-code regions. Rejected as unnecessary
complexity: the error-bound guarantee this format exists to provide holds
for *any* valid `t` a conforming search converges to, because the stored
factors are computed from whichever code was actually chosen, not from an
assumed idealized quantization (the same self-consistency property that
already let RFC 0010 leave k-means construction-side and unstandardized).
Pinning evaluation order would add real specification weight for a
guarantee nothing in this format's own invariants actually needs.

**Fold the ex-code region into a new, separate blob type** rather than
extending the existing cluster posting-list blob's per-cluster region.
Rejected: the ex-code region has no independent existence — it is always
paired one-to-one with a specific cluster's 1-bit region and row-id array,
fetched in the exact same Range GET (Design §2's "one new sub-region, not
one new blob" framing). A separate blob type would force a second Range
GET per selected cluster for no benefit, directly working against
invariant 3's one-wave rule.

## How this could be wrong

**Nearest grave: the Optane-era formats** (`docs/lineage.md`) —
"hardware-specific choices baked into media layouts, unimplementable the
day the hardware died." This RFC's single most consequential design
decision (Alternatives considered, above) is explicitly the choice *not*
to repeat this mistake: adopting the reference's own AVX2/AVX512-shuffled
ex-code packing would have tied STRAND's wire bytes to one vendor's
register-width choices for a computation this format's own kernel-
selection principle already routes through a scalar path. The residual
risk is not that this RFC repeats the Optane mistake, but the opposite
failure mode: if `docs/data-structures.md`'s kernel-selection principle
turns out to be wrong in practice — if a future SIMD implementation of the
classical-scalar-quantization kernel genuinely wants a specific interleave
for throughput — this RFC's plain packing would need a codec-registry
*variant* (a second registered packing, selected by descriptor metadata,
per invariant 8's "registered as named codecs" discipline) rather than a
silent format break. That is real, deferred, un-forced-by-this-RFC risk,
named here rather than assumed away.

**A genuinely unverified interpretive assumption**, named plainly in
Design §1: whether the reference's own "4-bit/5-bit/7-bit → 90/95/99%
recall" claim counts `bit_width` or `ex_bits`. This RFC's wire format is
correct either way (it is parameterized by the literal `bit_width` byte,
not by a recall target), but the Napkin math table's row labels could be
off by one bit-width step if the assumption is wrong — a real, bounded,
already-flagged risk, not a silent one.

**The byte-determinism carve-out extended to cover ex-codes, not just
factors, is a wider exception than any prior module in this crate
needed.** Every earlier RaBitQ-specific module this session built
(`quantize_one_bit`, `rotate_fht_kac`, `estimate_distance`) had exactly
one correct output, verified byte-exact against a compiled reference. This
RFC is the first to register a wire structure whose *values*, not just
their rounding, are legitimately writer-dependent. If a future compaction
or cross-segment codebook-sharing RFC (RFC 0010's own still-open Non-goal)
assumes byte-identical ex-code regions are a precondition for some cheap
merge path, this RFC's own carve-out would silently block it — a real
interaction this RFC does not resolve, named here for that future RFC to
inherit rather than rediscover.

## Non-goals

- **`bit_width` beyond 8** (`ex_bits` beyond 7). The reference library's
  own packing dispatch supports `ex_bits` up to 8; this RFC registers only
  up to 7, per Design §1's stated recall-driven rationale.
- **`faster_quantize_ex`/`get_const_scaling_factors`'s precomputed-`t_const`
  construction-time speedup.** A real, legitimate writer-side performance
  optimization this RFC does not standardize (Design §3) — any writer
  wanting it can implement it without a format change, since it only
  affects which valid `t` a writer's search converges to.
- **SIMD kernels for the classical scalar-quantization distance
  computation.** Per invariant 9, the scalar reference (`estimate.rs`'s
  extended formula, Design §4) is normative; a future SIMD path is a
  registered-in-code optimization, not a format concern, and is not
  attempted here.
- **A registered alternative ex-code packing variant for a future SIMD
  kernel.** Named as real, deferred risk in How this could be wrong,
  above — not designed here because no concrete SIMD implementation of
  the classical-scalar-quantization kernel exists yet to size the actual
  need against.
- **Cross-segment codebook sharing's interaction with the byte-
  determinism carve-out this RFC introduces.** Named in How this could be
  wrong, above; inherited by RFC 0010's own still-open Non-goal on the
  same topic, not resolved here.
