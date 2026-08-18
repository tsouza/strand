# RFC 0003: Scoring profiles

- **Status:** Draft
- **Milestone:** M1 — Lexical (`docs/milestones.md`)
- **Spec chapters produced:** `spec/scoring-profiles.md`
- **Invariants exercised:** 5 (`CLAUDE.md` §5)

## Summary

Defines the scoring-profile mechanism invariant 5 requires: a named identifier plus
parameters, pinning the exact formula two profiles use. `bm25` is this format's own
canonical BM25 definition, grounded directly in Robertson & Zaragoza's own formula
(`references/robertson-zaragoza-bm25-and-beyond.md`). `lucene-parity` is BM25
evaluated exactly as Apache Lucene 10.5.1's `BM25Similarity` does — a distinct
profile, not a restatement of `bm25`, because Lucene's real implementation differs
from the canonical form in two independently-grounded ways: its idf formula, and its
one-byte document-length quantization
(`references/lucene-bm25similarity-and-smallfloat.md`). This RFC pins both formulas,
the profile descriptor's schema, and score parity's exact meaning, with a worked
example computing real numbers under both profiles so the difference is not
abstract.

## Motivation

`CLAUDE.md` invariant 5 already settles the design tensions an earlier draft of this
project's own constitution left unresolved: scoring inputs (collection stats,
per-term doc frequencies, per-document lengths) are stored losslessly regardless of
which profile eventually scores them, "BM25" is treated as a family with named
instances rather than one function, and Lucene parity is defined precisely as parity
*within Lucene's own lossy norm quantization* — the harness computes Lucene's
quantized norm from STRAND's lossless lengths and compares like with like, rather
than either demanding literal byte-parity against a lossy value or promising parity
without ever pinning a formula. What invariant 5 does not yet do is state the actual
formulas, the descriptor's wire shape, or a checkable example — that is this RFC's
job, and it is a prerequisite for M1's Lucene-parity benchmark
(`docs/milestones.md` M1: "Lucene parity per invariant 5").

## Non-goals

**BM25F** (the multi-field, per-stream-weighted BM25 variant Robertson & Zaragoza
also define) is not specified here. Nothing about the descriptor schema below
forecloses registering it later as a third named profile; it is simply not needed
until STRAND has a multi-field scoring story, which is not M1 scope.

**Per-document length's exact definition** — which tokens are counted, at which
point in the analysis chain, per field — is invariant 6's job, owed by the M1
analyzer-descriptor RFC (not yet drafted). This RFC treats a document's length as an
opaque, losslessly-stored per-document count, however that count is eventually
defined; the worked example below states its own token counts by fiat, as a toy
input, not as a claim about how a real analyzer would produce them. When the
analyzer-descriptor RFC lands, `lucene-parity`'s norm computation additionally
inherits Lucene's own `discountOverlaps = true` default (excluding zero-position-
increment tokens from the count,
`references/lucene-bm25similarity-and-smallfloat.md`) — noted here as a real,
sourced constraint on that future RFC, not resolved by this one.

**Where the descriptor physically lives inside a segment** is also not pinned here.
Invariant 5 says profile descriptors are "carried in blob metadata," but the
container's `blob_entry` (`spec/container.md` §5) is a fixed 34-byte binary struct
with no room for variable-length JSON, and the lexical family's own blob layout
(postings, positions, term dictionary) is the still-open R2 RFC's job, gated on R9's
measurement (`docs/ledger.md`). Pinning an exact byte offset for a structure that
depends on a blob layout that doesn't exist yet would be premature. This RFC instead
defines the descriptor's *content* — its schema, exactly serialized as it will be
wherever it ends up — and states the placement constraint the R2 RFC must satisfy:
whatever mechanism carries the descriptor MUST NOT add a round trip beyond invariant
3's ≤2-RTT open budget, the same constraint the hotcache itself satisfies.

**Query-time fusion, ranking logic, and score explanation** stay out of the format
per `CLAUDE.md` §1; this RFC pins the number a profile produces per term, not what a
query planner does with the sum.

## Design

### 1. The scoring-profile descriptor

A scoring-profile descriptor is two fields:

| field        | type                        | notes                                              |
| ------------ | --------------------------- | --------------------------------------------------- |
| `profile_id` | string                      | registered profile identifier — `"bm25"` or `"lucene-parity"` at v0.1 |
| `parameters` | object, profile-specific    | see each profile below                             |

Serialized as JSON (matching the manifest's own binary-for-wire-structures,
JSON-for-metadata split, `spec/manifest.md` §1) — a descriptor is read once per
segment open, not on the hot per-posting path, so JSON's parse cost is irrelevant
where invariant 11's determinism concerns bind (wire structures decoded into a fixed
byte layout); a descriptor's determinism concern is the same smaller one the manifest
already has — stable, documented key ordering for human diffability, not
bit-exactness.

### 2. The `bm25` profile

Parameters: `k1` (float, default `1.2`), `b` (float, `0 ≤ b ≤ 1`, default `0.75`).
These defaults are Lucene's own (`references/lucene-bm25similarity-and-smallfloat.md`)
and sit inside Robertson & Zaragoza's own stated "reasonably good" range, `1.2 < k1 <
2` and `0.5 < b < 0.8` (`references/robertson-zaragoza-bm25-and-beyond.md`) —
choosing them is not an independent decision this RFC makes casually, it is picking
the one combination already validated by both the framework's own authors and the
most widely deployed implementation.

For a query term with document frequency `n` in a collection of `N` documents
carrying the term's field, and a document with term frequency `tf` and length `dl` in
a collection whose average length (for that field) is `avdl`:

```
idf(n, N)        = ln((N - n + 0.5) / (n + 0.5))
norm(dl, avdl)    = k1 * ((1 - b) + b * dl / avdl)
score(tf, n, N, dl, avdl) = idf(n, N) * tf / (tf + norm(dl, avdl))
```

This is Robertson & Zaragoza's own canonical form (eq. 3.15,
`references/robertson-zaragoza-bm25-and-beyond.md`), not the "common variant" that
adds a `(k1 + 1)` factor to the numerator. The paper is explicit that the variant
factor "does not affect the ranking produced" — it is a constant multiplier applied
identically to every term, present specifically to make a term's BM25 weight match
its plain RSJ weight at `tf = 1`, a compatibility concern this format does not have.
Omitting it keeps `bm25`'s formula identical in shape to `lucene-parity`'s (below),
which is deliberate: it isolates the two profiles' actual differences (idf, norm
quantization) instead of burying them under an unrelated numerator constant.

A document's full score for a query is the sum of `score(...)` over the query's
terms present in that document — the classic sum-of-term-weights structure both
profiles share (`references/robertson-zaragoza-bm25-and-beyond.md` §2.4).

### 3. The `lucene-parity` profile

Same `k1`/`b` parameters and defaults. Two formula differences from `bm25`, both
sourced directly from Lucene 10.5.1's `BM25Similarity`
(`references/lucene-bm25similarity-and-smallfloat.md`), not assumed:

**A different idf formula:**

```
idf_lucene(n, N) = ln(1 + (N - n + 0.5) / (n + 0.5))
```

The `+1` inside the logarithm is Lucene's own addition; `bm25`'s idf has no such
term. (`score(...)` otherwise has the identical shape — Lucene's real `doScore`
algebraically reduces to `weight * tf / (tf + norm)` with `weight = boost * idf`, the
same structure as `bm25`'s, confirmed by reading Lucene's source directly, not
assumed from a rewritten-for-performance appearance.)

**A quantized document length.** `bm25` uses `dl` as stored, losslessly. `lucene-
parity` MUST first pass `dl` through Lucene's own one-byte encode/decode round trip
before computing `norm`:

```
dl_lucene = byte4_decode(byte4_encode(dl))
norm_lucene(dl, avdl) = k1 * ((1 - b) + b * dl_lucene / avdl)
```

`byte4_encode`/`byte4_decode` is Lucene's `SmallFloat.intToByte4`/`byte4ToInt` — an
integer-valued, 4-significant-bit encoding, **not** the classic `byte315` float
encoding some older Lucene documentation describes
(`references/lucene-bm25similarity-and-smallfloat.md` names this explicitly as a
correction this RFC's own research made against an initial wrong assumption). Its
defining property, worth stating normatively since a conformance harness needs it
verified: document lengths from `0` to `23` tokens encode **exactly** — the encoded
byte equals the token count, no information loss — and only lengths `24` and above
enter the lossy floating encoding. `spec/scoring-profiles.md` MUST reproduce the
exact algorithm (`references/lucene-bm25similarity-and-smallfloat.md`'s quoted source,
not a paraphrase), because a conformance harness computing `lucene-parity` scores
against a different quantization function would silently fail parity on every
document whose length exceeds 23 tokens — the overwhelming majority of real
documents.

**Score parity, defined precisely (invariant 5).** Engine B evaluating a segment's
declared profile MUST match engine A's score within a stated floating-point
tolerance — this RFC pins that tolerance at **relative error ≤ 1e-5** for `f64`
arithmetic (STRAND's own reference scorer) compared against `f32` arithmetic (Lucene's,
per the source's own `float` types throughout `BM25Similarity`), loose enough to
absorb the float/double precision difference without absorbing an actual formula
divergence — chosen as a starting point for the parity harness to validate or
tighten, not asserted as empirically measured. For `lucene-parity` specifically,
"matching Lucene" means matching a harness that computes Lucene's quantized norm
from STRAND's own lossless length, per invariant 5's own resolution — never a
demand that STRAND's stored length itself be lossy.

## Worked example

Toy collection: 4 documents share a field. The term `"whale"` has document frequency
`n = 1` (it appears in exactly one of the four documents). That document has `dl =
41` tokens for the field, and `"whale"` occurs `tf = 3` times in it. The field's
average length across the collection is `avdl = 40`. Both profiles use `k1 = 1.2, b
= 0.75`. All figures below are computed, not hand-derived, per `CLAUDE.md` §2.

**`bm25` profile** (uses `dl = 41` exactly, as stored):

| step | computation | value |
| ---- | ----------- | ----- |
| `idf` | `ln((4 - 1 + 0.5) / (1 + 0.5))` = `ln(3.5 / 1.5)` = `ln(2.333333)` | `0.847298` |
| `norm` | `1.2 * ((1 - 0.75) + 0.75 * 41 / 40)` = `1.2 * (0.25 + 0.768750)` | `1.222500` |
| `score` | `0.847298 * 3 / (3 + 1.222500)` = `0.847298 * 3 / 4.222500` | `0.601988` |

**`lucene-parity` profile** (quantizes `dl = 41` through Lucene's norm byte first):

| step | computation | value |
| ---- | ----------- | ----- |
| `byte4_encode(41)` | `intToByte4(41)` | `40` (the byte) |
| `byte4_decode(40)` | `byte4ToInt(40)` | `40` (tokens — `41` rounded down to `40`, the first document length at which this encoding is lossy) |
| `idf_lucene` | `ln(1 + (4 - 1 + 0.5) / (1 + 0.5))` = `ln(1 + 2.333333)` = `ln(3.333333)` | `1.203973` |
| `norm_lucene` | `1.2 * ((1 - 0.75) + 0.75 * 40 / 40)` = `1.2 * (0.25 + 0.75)` | `1.200000` |
| `score_lucene` | `1.203973 * 3 / (3 + 1.200000)` = `1.203973 * 3 / 4.200000` | `0.859981` |

The two profiles score the same term, document, and collection **43% differently**
(`0.859981 / 0.601988 ≈ 1.4286`) — entirely from the idf `+1` and the one-token
quantization loss, with the underlying formula shape identical. This is the concrete
demonstration of why invariant 5 makes Lucene parity its own profile rather than an
assumed consequence of implementing `bm25`: an engine that implements only `bm25`
and calls itself "Lucene-compatible" would be wrong by a margin that grows, not
shrinks, as document lengths grow past the 23-token exact-encoding ceiling.

## Napkin math (`CLAUDE.md` §7)

Not a cold-path structure in the invariant-3 sense — a scoring-profile descriptor is
metadata read once at segment open, not a per-posting structure fetched during query
execution. The binding constraint, stated for whichever RFC ends up placing the
descriptor's bytes: **it must add zero round trips**, arriving inside whatever the
open protocol already fetches wholesale (today, the hotcache; potentially the lexical
blob's own header once R2 lands). A design that requires a dedicated GET for the
descriptor would violate invariant 3's ≤2-RTT budget for no good reason, since the
descriptor is at most a few dozen bytes (a profile identifier string plus two
floats) — orders of magnitude under any budget this format has ever needed a napkin
calculation to justify.

## Invariant-11 checklist

- **Endianness:** not applicable — the descriptor is JSON, not a binary wire
  structure; invariant 11's little-endian pin applies to binary structures decoded
  into a fixed byte layout.
- **Term sort order:** not applicable at this layer.
- **Chunk codec:** not applicable — too small to chunk-compress; whichever blob ends
  up carrying it declares its own chunk codec, if any, independently.
- **Checksums:** covered by whatever blob's registry entry ends up carrying the
  descriptor's bytes (`spec/container.md` §5's `checksum` field); this RFC does not
  introduce a new checksum scope.
- **Codec-variant provenance:** not applicable — no compression codec is used here.
- **Stochastic-transform provenance:** not applicable — nothing here is stochastic.
- **Golden files:** the worked example above, once implemented, becomes a
  `conformance/` golden file: the descriptor's JSON bytes, and the exact score both
  profiles produce for the worked example's toy collection — the first real,
  checkable BM25-parity test vector this project has.

## How this could be wrong

**Implementing from a remembered norm-encoding scheme instead of a checked one.**
This RFC's own drafting process caught, in itself, the exact failure mode `CLAUDE.md`
§3 exists to prevent: an initial assumption that Lucene's one-byte norm uses the
widely-blogged-about `byte315` float encoding, corrected only by fetching and reading
Lucene's actual current source
(`references/lucene-bm25similarity-and-smallfloat.md`). Nearest grave: this is the
same texture of error as the SIMD-BP128 misnaming and the Gorder misapplication
`CLAUDE.md` §3 already names — a model blending adjacent, plausible-sounding
technique names from memory instead of the actual, checked one. Had this RFC shipped
the wrong quantization function, every `lucene-parity` score for a document over 23
tokens would have silently diverged from real Lucene, and the parity benchmark would
either falsely fail (blocking M1) or, worse, pass by coincidence on a test corpus
whose documents all happened to be short.

**Grounding against `main` instead of a released version.** Lucene's `main` branch
carries an unreleased `k3` query-term-saturation parameter this RFC's own research
found and deliberately excluded, pinning `lucene-parity` against the actual released
`10.5.1` tag instead
(`references/lucene-bm25similarity-and-smallfloat.md`). A future session extending
this profile to match a Lucene version that ships `k3` enabled by default (it
currently defaults to disabled, `k3 = -1`, even where the parameter exists) would
need to re-open this RFC, not silently reinterpret it.

**The `(k1+1)` numerator variant is itself a trap for a future reader, not just this
one.** Robertson & Zaragoza's own paper documents the variant as common in published
implementations. A future session grounding a *different* engine's parity profile
(Elasticsearch, PISA, Anserini — none audited in this RFC) should not assume it
matches either `bm25` or `lucene-parity` here without checking that engine's own
source the same way this RFC checked Lucene's; the paper is explicit that both
conventions are in real use.

**The classic idf formula's exact citation is honestly incomplete.** This RFC's
`bm25` idf formula is grounded in the widely-established Robertson–Sparck-Jones form,
but `references/robertson-zaragoza-bm25-and-beyond.md` states plainly that this
research pass did not independently re-derive that formula's exact equation number
from the primary source's earlier binary-independence-model sections — the paper's
own eq. 3.15 (the term-weighting formula) was read directly and is fully confirmed;
the idf component plugged into it was not re-traced to its own numbered equation in
the same pass. A future session should close this gap before treating the citation
as complete, rather than assume it already was.

## Alternatives considered

**A single "bm25" profile with a `lucene_compatible: bool` flag** instead of two
named profiles. Rejected: invariant 5 already settled this — named profiles, not
boolean flags on one profile, because the two differ in more than one axis (idf
formula and norm quantization, independently), and a flag conflates "which formula"
with "which quantization" as if they were one toggle. The worked example shows they
compound; a flag would obscure that.

**Byte-parity against Lucene's stored norm** instead of quantization-aware parity.
Rejected explicitly by invariant 5, restated here with the reasoning made concrete:
STRAND stores lossless lengths (invariant 5's own requirement), so a byte-parity
demand against Lucene's lossy stored norm would be internally contradictory — the
worked example's `dl = 41` case shows exactly why: STRAND's own stored value is `41`,
not `40`; only a harness that *recomputes* Lucene's quantization from the lossless
`41` produces a comparable number.

**Adding the `(k1+1)` factor to `bm25` for familiarity**, since it appears in many
textbook presentations and some real engines. Rejected: the primary source itself
calls this a variant, not the canonical form, and including it here would obscure
this RFC's own comparison between `bm25` and `lucene-parity` under a constant that
provably does not change ranking — better to keep `bm25` as clean as the framework's
own authors define it.

## Open questions / follow-on RFCs

- The descriptor's exact placement inside a segment — embedded in the lexical
  blob's own header vs. a dedicated sibling blob — is deferred to the R2/postings RFC
  (`docs/ledger.md` R2), which owns the lexical blob's byte layout. This RFC only
  binds that future placement to the zero-extra-round-trip constraint stated in
  Napkin math above.
- Per-document length's precise definition (which tokens count, `discountOverlaps`-
  equivalent behavior) is the M1 analyzer-descriptor RFC's job (invariant 6,
  `docs/milestones.md`) and is a real, currently unresolved dependency this RFC's
  Non-goals section names rather than assumes away.
- The floating-point tolerance this RFC proposes (relative error ≤ 1e-5) is stated
  as a starting point, not an empirically validated figure — the M1 Lucene-parity
  benchmark (`docs/milestones.md`) should confirm or tighten it against real
  measured scores, not just this RFC's worked example.
- BM25F, named as a Non-goal, is a natural third profile once STRAND has a
  multi-field scoring story; not scheduled.
- Other engines' own BM25 variants (Elasticsearch, PISA, Anserini) are not audited
  here; a future profile targeting any of them needs its own from-source grounding,
  per "How this could be wrong" above.
