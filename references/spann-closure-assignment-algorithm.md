# SPANN's closure-assignment algorithm: the real criterion and parameters

Vendored 2026-08-19, for M2-1 (`docs/roadmap.md`; RFC 0010 Non-goals/Open
questions — "SPANN-style closure-replication's metadata slot and construction
algorithm"). Source: `arxiv.org/abs/2111.08566` (Chen et al., SPANN, NeurIPS
2021), fetched via its ar5iv HTML rendering (`ar5iv.labs.arxiv.org/html/
2111.08566` — arXiv's own maintained LaTeX-to-HTML conversion, not a
third-party OCR; `references/spann-body-figures.md` already established that
this paper's own raw PDF-to-text extraction is comprehensible for prose but
this session used the HTML rendering specifically to get exact-rendered
equations, which `pdftotext -layout` mangles for anything with subscripts).
Cross-confirmed against a second, independent primary source: the paper's own
reference implementation, `github.com/microsoft/SPTAG`, fetched live via
GitHub code search (`mcp__plugin_github_github__search_code`), not from
memory, per `CLAUDE.md` §3.

Cited by: `rfcs/0010-vector-blob-cluster-family.md` Discussion — post-approval
amendment (M2-1); `crates/strand-vector/src/closure.rs`.

## The algorithm: closure (multi-cluster) assignment, §3.2.2 "Posting list
## expansion"

Setup, quoted verbatim (the sentence introducing the notation): "assign a
vector to multiple closest clusters instead of only the closest one if the
distance between the vector and these clusters are nearly the same." Cluster
centroids are indexed by distance to a given vector **x**, ascending:
`Dist(x, c_i1) ≤ Dist(x, c_i2) ≤ ... ≤ Dist(x, c_ik)` (`c_i1` is x's nearest
centroid, its primary cluster).

**Equation 2 (the closure criterion), quoted verbatim (LaTeX rendering
normalized to plain notation):**

> `x ∈ X_ij ⟺ Dist(x, c_ij) ≤ (1 + ε₁) × Dist(x, c_i1)`

A vector `x` is additionally assigned to (replicated into) cluster `c_ij`'s
posting list `X_ij` iff its distance to that cluster's centroid is within a
factor of `(1 + ε₁)` of its distance to its own nearest (primary) centroid.
Because the right-hand side is fixed per vector and the left-hand side is
monotonically non-decreasing as `j` increases (centroids are already ordered
by distance to `x`), once a candidate `c_ij` fails this test every farther
candidate `c_i(j+1), ...` fails it too — the criterion can be evaluated by
walking the distance-sorted candidate list and stopping at the first failure,
which is exactly how `crates/strand-vector/src/closure.rs` implements it.

**The chosen value of ε₁**, quoted verbatim (§4.2, experimental setup): "set
ϵ₁ for posting list expansion to 10.0." This is a real, unusually loose
threshold — `(1 + 10.0) = 11×` the primary distance — not a typo or a
misread decimal point (independently re-confirmed by a second, separately
worded fetch of the same section, which reproduced the identical digits and
explicitly characterized it as "an expansive threshold for capturing
boundary points across distant clusters," not a tight "nearly the same"
tolerance despite the introductory sentence's own looser wording). In
practice this ratio test's looseness is not the binding constraint: it
operates over an already-small candidate set (the head index's own
approximate-nearest-centroids search, not every centroid in the index), and
the replica cap below is what actually bounds the real assignment count in
SPANN's own experiments.

## The replica cap

Quoted verbatim (§4.2.3, ablation on the number of closure replicas): "using
more than one replicas improves the performance significantly. However, when
the number of replicas is larger than 8, the performance cannot be improved
any more. Therefore, we choose 8 replicas for all of our experiments."

**Independently confirmed by the reference implementation.** SPTAG's own
build-time parameter, `AnnService/inc/Core/SPANN/ParameterDefinitionList.h`:

```
DefineSSDParameter(m_replicaCount, int, 8, "ReplicaCount")
```

`ReplicaCount` defaults to `8`, matching the paper's own stated choice
exactly — real, shipped, load-bearing corroboration that the "8" figure is
not an ar5iv rendering artifact, and that "replica count" in SPANN's own
vocabulary means the **total** number of posting lists a vector lands in
(primary plus closure replicas), not "extra replicas beyond the primary" —
the parameter name and the paper's own "at most 8 closure replicas for each
vector" phrasing both read as the total. `crates/strand-vector/src/
closure.rs`'s `max_replicas` field adopts this same total-count convention.

## The RNG-rule secondary pruning

Quoted verbatim, the rule that skips an otherwise-qualifying candidate to
reduce redundancy between nearby posting lists: "Dist(c_ij, x) > Dist(c_i(j-1),
c_ij)" — when this holds, candidate `c_ij` is skipped. `c_i(j-1)` is the
*previous entry in the same distance-sorted-by-x ordering* (the `(j-1)`-th
nearest centroid to `x` overall), not "the previously *accepted* replica" —
the paper's own indexing is over the full ordered candidate list, and the
rule is evaluated per-candidate independent of whether the immediately
preceding candidate itself passed its own tests.

**Partial corroboration, not full**: SPTAG ships a distinctly named
`RNGFactor` parameter (default `1.0`, `AnnService/inc/Core/Common/
NeighborhoodGraph.h`) used in `RelativeNeighborhoodGraph.h`'s own edge-pruning
check (`m_fRNGFactor * dist(...) < item.Dist`) — the identical mathematical
shape (a distance-ratio threshold between a candidate and a reference point,
factor 1.0 meaning no loosening) confirms SPANN's codebase genuinely
implements *an* RNG-style pruning rule with this exact structure, but that
specific shipped code path builds the **head index's own centroid-to-centroid
neighborhood graph** (used for query-time centroid navigation), not
necessarily the identical call site the paper's §3.2.2 closure-assignment
text describes. This session did not locate a SPTAG source line applying the
RNG check specifically at closure/posting-list-assignment time (as opposed to
head-index construction) via GitHub code search alone — a real, named
residual gap. `crates/strand-vector/src/closure.rs`'s implementation of this
secondary rule is therefore grounded in the paper's own §3.2.2 text (the
primary source, independently re-extracted twice with identical results) but
carries a real, stated caveat that its exact call-site fidelity to SPTAG's
own closure-assignment code was not independently re-derived from that
repository's source, unlike the epsilon and replica-count parameters above.
Because this rule is strictly a redundancy-*reducer* (it only ever removes
candidates the epsilon test already accepted, never adds any), an
implementation that got its precise formulation wrong would misjudge the
realized replication factor, not the format's correctness or its worst-case
byte-cost bound (`max_replicas` alone already bounds that) — named precisely
in `rfcs/0010-vector-blob-cluster-family.md`'s Discussion amendment and its
own "how this could be wrong."

## A STRAND-specific interpretation choice this grounding leaves open

Neither the paper's Eq. 2 nor the RNG-rule sentence specifies whether `Dist`
is Euclidean distance or squared Euclidean distance. `crates/strand-vector`
already has an established convention throughout (`kmeans.rs`'s
`squared_distance`, `query.rs`'s `centroid_distance` for the L2 metric): all
internal L2 distance computations are **squared** Euclidean (monotonic with
true distance, cheaper, no accuracy loss for ranking or ratio comparisons
that don't mix with non-squared quantities elsewhere). `closure.rs` follows
that same convention for consistency with the rest of this crate, which
means its `epsilon` is applied to squared distances — equivalent to a `√(1 +
ε₁)` ratio on true (unsquared) distances, not the `(1 + ε₁)` ratio a literal
unsquared reading of Eq. 2 would give. This is a real, stated STRAND-specific
interpretation of an ambiguous upstream formula (`CLAUDE.md` §3's "never
invent a format decision mid-session" bar is about wire-format decisions;
this is a construction-side numerical convention, analogous to `kmeans.rs`'s
own already-accepted choice to run entirely in squared-distance space) —
named here, and in RFC 0010's Discussion amendment, rather than left
implicit.
