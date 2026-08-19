# RaBitQ-Library — multi-bit (Extended-RaBitQ / "RaBitQ+") encode and query source

Vendored excerpt. Source: `github.com/VectorDB-NTU/RaBitQ-Library`,
`include/rabitqlib/quantization/rabitq_impl.hpp` (namespace `ex_bits`),
`include/rabitqlib/quantization/pack_excode.hpp`,
`include/rabitqlib/simd/pack_excode_dispatch.hpp`,
`include/rabitqlib/index/estimator.hpp`, and
`include/rabitqlib/index/query.hpp` (fetched live via `gh api
repos/VectorDB-NTU/RaBitQ-Library/contents/...`, 2026-08-19, `main` branch
HEAD at fetch time).

Cited by: the follow-on RFC for multi-bit Extended-RaBitQ registration
(RFC 0010 Open questions), `crates/strand-vector/src/quantize_ex.rs`,
`crates/strand-vector/src/estimate.rs`'s multi-bit path.

## Encode side (`rabitq_impl.hpp`, namespace `rabitqlib::quant::rabitq_impl::ex_bits`)

`kConstEpsilon = 1.9` (shared with the 1-bit path, `references/
rabitq-library-one-bit-quantization-source.md`).

`kTightStart`, a fixed `std::array<float, 9>` (`float`-typed constants, not
`double`) lookup table indexed by `ex_bits` (1..8), giving the starting
fraction of the search range `best_rescale_factor` begins its greedy walk
from: `{0, 0.15, 0.20, 0.52, 0.59, 0.71, 0.75, 0.77, 0.81}`.

**`best_rescale_factor(o_abs, dim, ex_bits)`** — an event-driven greedy
search for the scalar rescale factor `t` that maximizes the cosine
similarity between the `ex_bits`-quantized magnitude vector and the true
(normalized, absolute-valued) residual vector `o_abs`. Read directly from
source, verbatim structure:

```cpp
double max_o = *std::max_element(o_abs, o_abs + dim);
double t_end = static_cast<double>(((1 << ex_bits) - 1) + kNEnum) / max_o;  // kNEnum = 10
double t_start = t_end * kTightStart[ex_bits];

std::vector<int> cur_o_bar(dim);
double sqr_denominator = static_cast<double>(dim) * 0.25;
double numerator = 0;
for (size_t i = 0; i < dim; ++i) {
    int cur = static_cast<int>((t_start * o_abs[i]) + kEps);  // kEps = 1e-5
    cur_o_bar[i] = cur;
    sqr_denominator += (cur * cur) + cur;
    numerator += (cur + 0.5) * o_abs[i];
}

// min-heap of (next_t, dim_index): the t value at which bucket i's code
// would next increment, ordered ascending.
std::priority_queue<std::pair<double, size_t>, std::vector<std::pair<double, size_t>>,
                     std::greater<>> next_t;
for (size_t i = 0; i < dim; ++i) {
    next_t.emplace(static_cast<double>(cur_o_bar[i] + 1) / o_abs[i], i);
}

double max_ip = 0, t = 0;
while (!next_t.empty()) {
    auto [cur_t, update_id] = next_t.top(); next_t.pop();
    cur_o_bar[update_id]++;
    int update_o_bar = cur_o_bar[update_id];
    sqr_denominator += 2 * update_o_bar;   // (b^2+b) - ((b-1)^2+(b-1)) = 2b
    numerator += o_abs[update_id];         // (b+0.5)*o - (b-0.5)*o = o

    double cur_ip = numerator / std::sqrt(sqr_denominator);
    if (cur_ip > max_ip) { max_ip = cur_ip; t = cur_t; }

    if (update_o_bar < (1 << ex_bits) - 1) {
        double t_next = static_cast<double>(update_o_bar + 1) / o_abs[update_id];
        if (t_next < t_end) next_t.emplace(t_next, update_id);
    }
}
return t;
```

**`quantize_ex(o_abs, code, dim, ex_bits)`**: `t = best_rescale_factor(...)`;
for each dimension, `code[i] = clamp(floor(t * o_abs[i] + kEps), 0, 2^ex_bits
- 1)`; accumulates `ipnorm = sum((code[i] + 0.5) * o_abs[i])`; returns
`ipnorm_inv = 1 / ipnorm` (or `1.0` if not a normal float — zero or
non-finite `ipnorm`).

**`ex_bits_code(residual, dim, ex_bits, ex_code)`**: `abs_res =
abs(residual / ||residual||)` (L2-normalized, then absolute value);
`ipnorm_inv = quantize_ex(abs_res, ex_code, dim, ex_bits)`; then, for every
dimension where the (signed, un-normalized) residual is negative, the code
is bitwise-complemented within its `ex_bits` width: `ex_code[j] = (~tmp) &
((1 << ex_bits) - 1)`. This is the value that is actually packed and
written to disk (`ex_bits_compact_code`, below) — the sign-complemented
magnitude code, not the raw pre-complement quantization.

**`ex_bits_code_with_factor(data, centroid, dim, ex_bits, ex_code, &f_add_ex,
&f_rescale_ex, &f_error_ex, metric_type)`**: computes `residual = data -
centroid`; `ipnorm_inv = ex_bits_code(residual, dim, ex_bits, ex_code)`
(this call already applies the sign complement above); then, purely as a
local working array (not returned or stored), reconstructs `total_code[i] =
ex_code[i] + (sign_bit[i] << ex_bits)` where `sign_bit[i] = residual[i] >=
0`, and `cb = -(2^ex_bits - 0.5)`, `xu_cb[i] = total_code[i] + cb`. The
remaining factor computation is structurally identical to
`one_bit_code_with_factor` (`references/rabitq-library-one-bit-quantization-
source.md`) with `xu_cb`/`cb` substituted for the 1-bit versions:

```cpp
T l2_sqr = l2norm_sqr(residual, dim);
T l2_norm = sqrt(l2_sqr);
T ip_resi_xucb = dot_product(residual, xu_cb, dim);
T ip_cent_xucb = dot_product(centroid, xu_cb, dim);
if (ip_resi_xucb == 0) ip_resi_xucb = infinity;
T tmp_error = l2_norm * kConstEpsilon *
    sqrt(((l2_sqr * l2norm_sqr(xu_cb, dim)) / (ip_resi_xucb * ip_resi_xucb) - 1) / (dim - 1));

// L2:
f_add_ex = l2_sqr + 2 * l2_sqr * ip_cent_xucb / ip_resi_xucb;
f_rescale_ex = ipnorm_inv * -2 * l2_norm;
f_error_ex = 2 * tmp_error;
// IP:
f_add_ex = 1 - dot_product(residual, centroid, dim) + l2_sqr * ip_cent_xucb / ip_resi_xucb;
f_rescale_ex = ipnorm_inv * -l2_norm;
f_error_ex = tmp_error;
```

**`ex_bits_compact_code`**: calls `ex_bits_code_with_factor` into a plain
per-dimension `uint8_t` array, then `packing_rabitqplus_code(ex_code, dim,
ex_bits)` to bit-pack it — the packed form is what `ExDataMap` stores
(`references/rabitq-library-ivf-and-batch-layout-source.md`).

**`f_error_ex` is computed by the encoder but never read at query time** —
confirmed by `estimator.hpp` below, which sources its error term from the
1-bit path's already-stored `f_error`, scaled by `1 / 2^ex_bits`, not from
any per-vector `f_error_ex`. `ExDataMap<T>::data_bytes()`'s own
`sizeof(T) * 2` (not `* 3`) already reflects this: only `f_add_ex` and
`f_rescale_ex` are persisted.

## Packing (`pack_excode.hpp`, `simd/pack_excode_dispatch.hpp`)

`packing_1bit_excode` and `packing_8bit_excode` have portable scalar
sources (a 16-lane bit-interleave and a plain `memcpy`, respectively).
`packing_2bit_excode` through `packing_7bit_excode` dispatch to
`rabitqlib::simd::packing_Nbit_excode`, declared in `pack_excode_dispatch.hpp`
with AVX2 and AVX512 variants — **no portable scalar implementation ships
for these six widths**; their definitions live in `.cpp` translation units
this fetch did not pull (analogous to `flip_sign`/`kacs_walk`,
`references/rabitq-library-rotation-application-source.md`). The comment
above the dispatcher states directly why the layout is hardware-shaped:
"To compute inner product with the support of SIMD, the packed codes need
to be stored in different patterns" — i.e., this is a SIMD-*accumulation*-
friendly interleave, not a property the RaBitQ+ algorithm itself requires.
Because `docs/data-structures.md`'s own settled kernel-selection principle
routes the multi-bit path through **classical scalar-quantization distance
computation, not FastScan LUT/register-shuffle machinery**, STRAND has no
normative reason to adopt this SIMD-specific interleave, and the follow-on
RFC registers its own plain, bit-contiguous packing instead (see that RFC's
Design section and Alternatives considered).

## Query side (`estimator.hpp`, `query.hpp`)

**`split_distance_boosting`** (and its inlined duplicates
`split_single_fulldist`/`split_single_fulldist_direct`) — the formula that
combines the already-computed 1-bit estimate with the ex-bits code for a
tighter distance:

```cpp
float ex_dist =
    cur_ex.f_add_ex() + q_obj.g_add() +
    (cur_ex.f_rescale_ex() *
     (static_cast<float>(1 << ex_bits) * ip_x0_qr +
      ip_func_(q_obj.rotated_query(), cur_ex.ex_code(), padded_dim) +
      q_obj.kbxsumq()));

float low_dist = est_dist_or_ex_dist - (cur_bin.f_error() * g_error / static_cast<float>(1 << ex_bits));
```

`ip_x0_qr` is the same intermediate the 1-bit path already computes: the
raw (unscaled) dot product between the rotated query and the *unpacked*
1-bit binary code — in this codebase, the quantity `crates/strand-vector/
src/estimate.rs`'s `code_query_ip` already computes for the 1-bit path.
`ip_func_(rotated_query, ex_code, padded_dim)` is a plain dot product
between the rotated query and the ex-code array read as plain per-dimension
integers (0..2^ex_bits - 1) — no unpacking-to-bits step, unlike the 1-bit
path. `cur_bin.f_error()` is the 1-bit path's own already-stored
`f_error` — confirming the "no separate stored error factor" finding
above. No symmetric upper bound is computed anywhere in this library's own
query path (only `low_dist`); the follow-on RFC's own Design section states
and justifies STRAND's choice to generalize this into a symmetric two-sided
bound, matching the existing 1-bit `estimate_distance`'s own `[lb, ub]`
convention, since the underlying error term is not asymmetric in the
math — only the reference's own call sites happen to consume the lower
bound alone (for lower-bound pruning during search).

**`SplitSingleQuery`/`SplitBatchQuery`** (`query.hpp`) compute the new
per-query constant this path needs, `kbxsumq`:

```cpp
float c_1 = -static_cast<float>((1 << 1) - 1) / 2.F;                    // unchanged, 1-bit path
float c_b = -static_cast<float>((1 << (ex_bits + 1)) - 1) / 2.F;        // NEW: total_bits = ex_bits + 1
T sumq = std::accumulate(rotated_query, rotated_query + padded_dim, T(0));
G_k1xSumq_ = sumq * c_1;   // unchanged
G_kbxSumq_ = sumq * c_b;   // NEW
```

Note `c_b` is parameterized by `ex_bits + 1` (i.e. `total_bits`, the
combined sign-plus-magnitude bit width), not `ex_bits` alone — the same
`cb = -(2^B - 0.5)`-style constant `one_bit_code_with_factor` uses for
`B = 1`, generalized to `B = ex_bits + 1` here. `g_add()`/`g_error()`
(`set_g_add`) are unchanged from the 1-bit path (`references/
rabitq-library-estimator-source.md`): `G_add = norm^2` (L2) or `-ip` (IP);
`G_error = norm`, both metrics.

## What this grounds

The follow-on RFC's ex-code region byte layout (encode algorithm,
factor formulas, which factors are actually persisted), the multi-bit
query-side distance-boosting formula, and the explicit, reasoned departure
from the reference's own SIMD-specific ex-code packing in favor of a
STRAND-defined plain bit-contiguous one.
