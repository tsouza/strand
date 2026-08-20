# RFC 0014: Graph blob family — warm-tier DiskANN/Vamana index

- **Status:** Draft. This RFC's own inline review (below, "How this could be
  wrong") is real and substantive, but it is written by the same session
  that drafted the design, not by an independent second pass — per
  `CLAUDE.md` §3's "agent designs, agent implements — but not in the same
  breath" principle, that makes it a self-review, not the adversarial
  review Approval requires. Three concrete reasons this particular RFC
  needs a reader who did not write it, stated precisely rather than
  generically: **first**, Design §2's physical-slot-indexed adjacency
  representation is a real STRAND-original container-layer decision built
  *on top of* two cited papers, not read directly out of either of them —
  DiskANN's own on-disk layout indexes by point ID with no permutation, and
  neither paper specifies how a reader resolves a vertex ID to a physical
  block offset after Starling's own block shuffling; this RFC's resolution
  (Design §2, §3) is this session's own design, and the exact place
  `CLAUDE.md` §3 says most needs an outside reader. **Second**, Design §5's
  decision to ship v0.1 with no in-memory compressed-code cache has a real,
  quantified, worse-than-DiskANN's-own-published-numbers round-trip cost
  (Napkin math, Worked example) that this session found only by tracing a
  worked example by hand, not by citing a source that validates it — an
  independent pass may weigh that trade differently. **Third**, no
  `bench/` measurement exists for this family yet (pure design-phase work,
  per this task's own scope: no crate code was written), so every latency
  figure below is a literature translation, not a STRAND-measured result,
  unlike RFC 0010's own Discussion amendments, which added real `bench/`
  numbers before that RFC's design claims were treated as settled.
- **Milestone:** M2 — Vectors, cluster-first (`docs/milestones.md`), the
  "warm-tier graph blob family (persisted-permutation node order, ordering
  algorithm per R1's evidence)" named there as "in-scope but explicitly
  second"; tracked as M2-3 (`docs/roadmap.md`), which names this RFC's two
  deliverables explicitly: "the graph-blob family (warm tier,
  DiskANN/Vamana)... including R1's second half (the node-order permutation
  algorithm question, Starling vs. an untested alternative)."
- **Spec chapters produced:** none yet. This RFC proposes a new chapter,
  `spec/graph-vectors.md`, and additively extending `spec/container.md` §9
  with a new `family_id = 5` ("graph"), both written only at Approval —
  every existing spec chapter in this repository states an already-settled
  result, and this RFC is not settled (the same discipline
  `rfcs/0013-puffin-export-sidecar.md`'s own header states).
- **Invariants exercised:** 1 (`CLAUDE.md` §5 — this RFC is the concrete
  detailing of the `rebuild` merge strategy invariant 1 already names for
  graph indexes, Design §6), 3 (argued *not* to govern this family's
  query path, grounded in its own literal text, Design §5 and Napkin math),
  7 (`tier: warm` — this family's own registration reason for existing,
  Design §7), 8 (adjacency layout adopted from the literature verbatim;
  physical-slot addressing argued as container-layer metadata, not adjacency
  invention, Design §2), 9 (beam width as a reader-tunable batch parameter,
  Design §8), 10 (storage class, alignment), 11 (byte determinism —
  Invariant-11 checklist, below).

## Summary

Registers STRAND's second vector-index family: a warm-tier, DiskANN/Vamana
graph blob, the family `CLAUDE.md`'s own mission statement names as
"in-scope but explicitly second" and `docs/lineage.md` credits for "the
two-tier memory model... generalize[d] into the tiered vector blob." Two new
blob types under a proposed `family_id = 5` ("graph"): a graph node-record
blob (each node's row-id, full-precision vector, and degree-bounded,
zero-padded adjacency list, laid out in **persisted-permutation** physical
order per `docs/data-structures.md`'s already-settled format decision) and a
node-order permutation directory (a dense row-id-indexed array resolving a
row-id to its physical slot, needed only for entry-point seeding and merge
bookkeeping, never on the ordinary query hot path).

This RFC resolves R1's second half — the node-order permutation *algorithm*
question `docs/roadmap.md` M2-3 names explicitly ("Starling vs. an untested
alternative") — with newly re-fetched, full-paper grounding (not the
abstract-only vendoring that predated this session): STRAND registers
Starling's **BNF (Block Neighbor Frequency)** heuristic as the default
block-shuffling algorithm, argued against a real, cited, unmeasured
alternative (reusing an existing k-means cluster assignment as physical
order) rather than against a straw man, and against Gorder, which
`docs/data-structures.md` already rules out on record.

Registers Vamana's exact construction algorithm (Algorithms 1–3, transcribed
verbatim in `references/diskann-neurips2019.md`) as the format's one
registered graph-index algorithm, per invariant 8. Finds, and states
honestly rather than glossing over, a real v0.1 design cost: because this
RFC does not register an in-memory compressed-code cache (deferred,
Non-goals), its query-resolution algorithm must fetch every node it ever
adds to its candidate list, not only nodes it chooses to expand — a real,
worse-than-DiskANN's-own-published-figures round-trip cost, quantified in
Napkin math and the Worked example, not hidden behind a citation to
DiskANN's own more optimized numbers.

## Motivation

`CLAUDE.md`'s own mission sentence draws the line this RFC exists on: "Cold
vector search in v0.1 is cluster-shaped. Graph indexes are in the format as
a warm-tier blob family and are explicitly not the cold-open story." R1's
own grounding (`docs/research/README.md`) is why: "a dependent chain of
50–200 fetches, i.e. 5–20 seconds at 100ms" — object-storage round-trip
latency turns graph beam search's dependent hop-by-hop access pattern into a
query nobody would accept. `docs/lineage.md`'s own "From DiskANN" paragraph
credits DiskANN's two-tier memory model as real, adopted prior art for the
tiered vector blob generally, while naming its "one-node-per-I/O-unit
layout" as "warm-tier prior art, not a cold-path design" — this RFC is
where that warm-tier prior art actually gets registered as a blob family,
rather than remaining a design-lineage footnote.

Registering it is still real work, not a formality, for three reasons this
RFC's own Design section engages directly. **First**, `docs/data-
structures.md` already settles that graph node order is "a **persisted
permutation** (the format decision, settled)" but leaves "the ordering
algorithm... explicitly open — Gorder targets graph-analytics traversal, not
ANN beam search, and is not a candidate; Starling's block shuffling is the
relevant literature, and R1's RFC picks with evidence." This RFC is that
RFC. **Second**, `docs/roadmap.md`'s own M2-3 entry states this is "not a
small task — it is a full new blob family requiring its own RFC (design,
worked example, napkin math, adversarial review) before any code, the same
weight RFC 0010 itself carried for the cluster family" — this RFC is sized
accordingly. **Third**, and least obvious until traced through: DiskANN's
own published performance (§4 of `references/diskann-neurips2019.md`, "> 5000
queries a second with < 3ms mean latency") depends on machinery this RFC
does *not* register in v0.1 — an in-memory compressed-code cache and a
tuned entry-point strategy — so simply citing DiskANN's own numbers as "what
STRAND's graph family delivers" would be exactly the kind of unearned
number `CLAUDE.md` §2 forbids. This RFC's Motivation is therefore narrower
and more honest than "add graph search": register a byte-exact, warm-tier,
`rebuild`-on-merge graph blob whose v0.1 read path is *correct* and
*reasoned about honestly*, not one whose performance claims borrow a
citation it hasn't earned yet.

Nearest prior art, named per `CLAUDE.md` §4: DiskANN (Subramanya et al.,
NeurIPS 2019) for the Vamana construction algorithm and the on-disk
node-record layout this RFC adapts (`references/diskann-neurips2019.md`);
Starling (Wang et al., SIGMOD 2024) for the block-shuffling node-order
algorithm this RFC registers as default (`references/starling-sigmod2024.md`);
IP-DiskANN (Xu et al., `arxiv.org/abs/2502.13826`) for confirming that even
the state-of-the-art in-place-update algorithm treats batch consolidation as
the default behavior it is trying to avoid, supporting this RFC's own
`rebuild` merge-strategy argument (Design §6); and RFC 0010's own cluster
family, whose registration discipline (blob registry table shape, worked
example rigor, napkin-math structure) this RFC follows directly as its
closest in-repository precedent.

## Non-goals

- **Cold-open compatibility.** This family is `tier: warm` by design,
  exempted from invariant 3's one-wave/two-round-trip budget by invariant
  7's own text ("`tier: warm` (assumes NVMe-class latency; graph families
  live here)"). Making graph beam search satisfy invariant 3 is not
  attempted, is not possible per R1's own arithmetic, and is not this RFC's
  job (Design §5, Napkin math argue this precisely rather than asserting
  it).
- **An in-memory compressed-code cache** (DiskANN's own PQ-compressed
  routing layer, `docs/lineage.md`'s "compressed codes for routing, full
  precision only for reranking"). Real, valuable, deferred — Design §5
  states exactly what its absence costs in v0.1 (a real, quantified
  round-trip regression versus DiskANN's own published figures), and Open
  questions names the follow-on RFC. Not silently assumed unnecessary.
- **An in-memory navigation graph for query-aware entry points** (Starling's
  own §4.2 component, `references/starling-sigmod2024.md`). This RFC
  registers Starling's block-shuffling *disk layout* component only, not
  its navigation-graph *entry-point* component — a second, independent
  optimization this RFC does not need to adopt to be internally consistent
  (Design §3 already resolves entry-point storage via a single persisted
  `entry_point_slot`), named as real follow-on work (Open questions).
- **Construction-time Vamana hyperparameters** (`R`, `L`, `α`, the
  block-shuffling iteration cap `β` and gain threshold `τ`, block size in
  KB). These are writer-side tuning inputs, out of scope for a read-side
  wire-format RFC, the same discipline RFC 0010 applied to k-means centroid
  count (RFC 0010 Non-goals). This RFC cites DiskANN's own published values
  (`references/diskann-neurips2019.md` §4) as real reference points, not as
  normative requirements.
- **In-place/streaming updates** (FreshDiskANN- and IP-DiskANN-style
  consolidation-free edits). Not merely deferred — a structural non-fit:
  STRAND's immutable-segment model (invariant 2) already commits to
  delete-and-reinsert plus deferred physical removal, and this RFC's own
  Design §6 registers `rebuild` as this family's merge strategy, so an
  in-place single-segment update algorithm has no STRAND write path to plug
  into in the first place.
- **A pluggable multi-algorithm graph registry** (HNSW, NSG, or others as
  alternative registered codecs). This RFC registers exactly one graph
  algorithm, Vamana, per invariant 8's "registered codec" discipline;
  Alternatives considered argues why, with DiskANN's own head-to-head data,
  rather than asserting it.
- **SIMD kernel validation for beam-search distance computation.** Deferred
  for the same reason RFC 0010 deferred its own FastScan ARM/SIMD
  validation — real, separate, measured work (Design §8).
- **Query-time vertex caching** (DiskANN's own §3.4, "caching frequently
  visited vertices in DRAM"). A legitimate reader-side optimization, not a
  wire-format requirement — named, not mandated, the same treatment RFC
  0010 gave `nprobe` tuning.

## Design

### 1. Blob registration

Two new blob types under a proposed `family_id = 5` ("graph"), added to
`spec/container.md` §9's registry at Approval, using the existing 42-byte
`blob_entry` struct unmodified (`spec/container.md` §5) — this RFC needs no
new container-level fields:

| `family_id` | `blob_type_id` | blob type                     | `storage_class` | `tier` | `alignment` |
| ----------- | -------------- | ------------------------------ | ---------------- | ------ | ----------- |
| 5           | 0              | graph node records              | raw-mappable      | warm   | 8           |
| 5           | 1              | node-order permutation directory | raw-mappable     | warm   | 4           |

Both are `storage-class: raw-mappable` (invariant 10): fixed-width,
offset-addressed structures with no benefit from a chunk-compression
wrapper — exactly the case that storage class exists for. A field registers
exactly one node-record blob and, only if node order is a nontrivial
permutation (`shuffle_algorithm ≠ 0`, §2 below), one permutation-directory
blob, per vector field with a graph index.

### 2. Graph node-record blob (`blob_type_id = 0`)

A fixed 16-byte header, followed by `node_count` fixed-width node records
laid out in **physical slot order** — record `k`'s byte offset is always
`16 + k * record_size`, with `record_size` a value every reader derives from
the header rather than a separately stored redundant field (the same
discipline RFC 0010's own M2-1 amendment applied after its adversarial
review flagged a redundant stored count as unnecessary,
`rfcs/0010-vector-blob-cluster-family.md` Design §3).

**Header** (16 bytes):

| offset | size | field               | notes                                                                 |
| ------ | ---- | --------------------- | ----------------------------------------------------------------------- |
| 0      | 4    | `node_count`            | u32; equals the field's `row_id_count` (`spec/container.md` §4)         |
| 4      | 4    | `dims`                  | u32; true vector dimensionality — no rotation padding in this family (invariant 8: no quantization is registered here, §5 below) |
| 8      | 1    | `max_out_degree`        | u8; Vamana's `R` (`references/diskann-neurips2019.md` Algorithm 3)      |
| 9      | 1    | `shuffle_algorithm`     | u8: `0` = none (ID/insertion-contiguous), `1` = BNP, `2` = BNF (**default**), `3` = BNS — see §4 |
| 10     | 2    | reserved                | writer MUST set zero; reader MUST NOT reject nonzero but MUST NOT interpret it |
| 12     | 4    | `entry_point_slot`      | u32; the physical slot of this graph's designated search entry point (conventionally the dataset medoid, Algorithm 3 — this RFC does not mandate medoid specifically, only that exactly one fixed entry point is designated) |

`record_size = 8 (row_id) + dims * 4 (vector) + 4 (neighbor_count) + 4
(reserved) + max_out_degree * 4 (neighbor_slots)` bytes, derived identically
by every reader from the header's own `dims` and `max_out_degree` fields.

**Node record** (`record_size` bytes, one per physical slot, in physical
slot order):

| offset (within record) | size                | field             | notes |
| ------------------------ | -------------------- | ------------------ | ----- |
| 0                         | 8                     | `row_id`            | u64; this node's stable row-ID (invariant 1) — the field that lets a reader recover which logical row a physical slot holds, and the field a deletion-vector check filters on |
| 8                         | `dims * 4`            | `vector`            | row-major f32, this node's full-precision coordinates (DiskANN's own `x_i`, `references/diskann-neurips2019.md` §3.2) |
| 8 + dims*4                | 4                     | `neighbor_count`    | u32; this node's real out-degree, `≤ max_out_degree` |
| 12 + dims*4               | 4                     | reserved            | writer MUST set zero; reader MUST NOT reject nonzero but MUST NOT interpret it |
| 16 + dims*4               | `max_out_degree * 4`  | `neighbor_slots`    | u32 array; the **physical slot indices** (not row-ids — see below) of this node's out-neighbors, the first `neighbor_count` entries real, the remainder zero-padded |

**Why physical slots, not row-ids, for `neighbor_slots` — a real, named
departure from DiskANN's own reference layout.** DiskANN's own on-disk
layout (`references/diskann-neurips2019.md` §3.2) stores "the identities of
its `≤ R` neighbors" directly as dataset point IDs, because DiskANN's own
layout is **not** reordered — a point's ID *is* its physical position
(`offset = i * record_size`, "does not require storing the offsets in
memory," verbatim). STRAND's own settled decision — node order is a
persisted permutation (`docs/data-structures.md`) — breaks that property on
purpose, trading it for Starling's locality gains (§4, below). Once physical
position and logical identity diverge, a neighbor reference has to be one or
the other, and this RFC registers physical slots: resolving a neighbor
during beam-search traversal (the hot path, dependent-chain-of-fetches by
construction) becomes one arithmetic step (`16 + slot * record_size`) with
no directory lookup at all, rather than a row-id-to-slot lookup on every
single hop. This is a container-layer addressing choice, not an invented
adjacency *layout* — the graph's own topology (which nodes connect to which)
is unchanged from whatever Vamana's construction algorithm produced;
invariant 8's novelty budget is spent here exactly the way RFC 0010 spent it
choosing to store real row-ids rather than local ordinals in its own
posting-list row-id arrays (`rfcs/0010-vector-blob-cluster-family.md`
Design §4) — a wire-representation decision about identity, not a departure
from the registered algorithm's own literature. The real cost of this
choice — every neighbor-slot value depends on physical layout, so *any*
reshuffle rewrites the entire adjacency region, not just a directory — is
named honestly in How this could be wrong, not hidden here.

**Padding determinism (invariant 11).** A writer MUST zero-fill every
`neighbor_slots` entry beyond `neighbor_count` and the record's own reserved
field — the same normative padding-determinism discipline RFC 0010 applied
to its own partially filled FastScan batches
(`rfcs/0010-vector-blob-cluster-family.md` Design §4), stated here for the
same reason: invariant 11 requires two conforming writers given the same
logical input to produce byte-identical output, and an unspecified padding
value would break that even though no conforming reader ever interprets it.

### 3. Node-order permutation directory (`blob_type_id = 1`)

A dense array of `node_count` little-endian u32 values, indexed by **local
ordinal** (row-id order, `spec/row-ids.md` §1's `local_ordinal = row_id -
row_id_base`) — the same density convention RFC 0010's own flat-vector blob
uses. Entry `i` gives the physical slot holding local ordinal `i`'s node
record: `physical_slot = permutation[local_ordinal]`.

This blob is deliberately **not** on the ordinary nearest-neighbor query hot
path. Design §5's traversal starts from `entry_point_slot` (already
physical, stored directly in the node-record blob's own header) and follows
`neighbor_slots` (already physical) at every hop — neither step ever needs
`local_ordinal → slot`. This directory exists for two narrower, real needs:
**first**, a query variant seeded by an existing row-id (a caller supplying
"find the neighbors of the vector already stored at row-id `X`" rather than
an arbitrary query vector) needs to resolve that row-id to a starting slot
without a linear scan. **Second**, and more load-bearing, a merge or rebuild
that reconstructs this blob needs a systematic mapping from every retained
row-id's old identity to its new physical slot — exactly the bookkeeping
Design §6's `rebuild` merge strategy performs. Both are real uses, not a
speculative completeness gesture; Napkin math sizes this blob's own real,
small cost.

### 4. Resolving the node-order permutation algorithm: Starling vs. an untested alternative

`docs/data-structures.md` already rules out one candidate on record: "Gorder
targets graph-analytics traversal, not ANN beam search, and is not a
candidate." This RFC does not re-litigate that; it names and weighs a real,
different candidate the existing documents do not yet address, so "Starling
vs. an untested alternative" (`docs/roadmap.md` M2-3) is answered against a
real second option, not a straw man.

**The untested alternative: physical order by existing k-means cluster
assignment.** If the same field also carries a cluster-family index (RFC
0010), its centroid assignment is *already computed* at no extra
construction cost — placing graph node records in cluster-assignment order
(instead of running a dedicated block-shuffling pass) is a real, free-seeming
idea: spatially nearby points are plausible proxies for graph-neighbor
locality, and it would let the graph family reuse infrastructure this format
is already building rather than adding a new algorithm.

**Why this RFC rejects it as the v0.1 default, argued rather than
assumed.** Three real problems, not one. **First**, it is genuinely
untested — no source this project has fetched measures a locality metric
(Starling's own `OR(G)`, `references/starling-sigmod2024.md` §4.1, Eq. 5)
for cluster-order graph placement; Starling's own BNF algorithm, by
contrast, is measured directly against real datasets at `OR(G) ≈ 0.3`–`0.6`
against a naive baseline's `OR(G) ≈ 0`, with a stated 43.9× end-to-end
throughput improvement. Choosing the unmeasured option as the *default*
would mean pinning a wire-format convention on a hypothesis, which invariant
11's determinism discipline does not itself forbid (any deterministic
algorithm is pinnable) but which this project's own "a number without a
vendored source sentence is deleted, not softened" standard (`CLAUDE.md`
§2) argues against doing casually. **Second**, it creates a cross-family
coupling this format's registry discipline otherwise avoids — every other
blob family (lexical, filter, vector-cluster, deletion) is self-contained,
addressable by its own `family_id`/`blob_type_id`/`field_id` triple with no
dependency on a *different* family's blob existing for the same field; a
graph blob whose node order depends on a cluster blob's centroid assignment
would be the first exception, and would leave undefined what a graph-only
field (no cluster index at all) is supposed to do. **Third**, and most
concretely: Vamana's own construction algorithm *deliberately* keeps
long-range edges. DiskANN's own Figure 1 caption (`references/diskann-
neurips2019.md`) states plainly that "the algorithm goes through the first
pass with `α = 1`, followed by the second pass where it introduces long
range edges" — the `α > 1` `RobustPrune` pass specifically preserves
neighbor candidates that are *not* spatially shadowed by a closer one, which
is precisely what gives Vamana graphs their short diameter and good
hop-count properties (§4.2 of the same reference: "2–3 times fewer hops...
due to its ability to add more long-range edges"). A cluster-order
placement would systematically scatter exactly these deliberately-long
edges across distant physical regions — the opposite of what a locality
metric like `OR(G)` wants — while Starling's BNF algorithm optimizes
directly against the graph's own *real, already-built* edge set, long-range
edges included, not a proxy for it.

**The registered default: Starling's BNF (Block Neighbor Frequency),
Algorithm II of `references/starling-sigmod2024.md` §4.1**, transcribed in
full in that file. Argued, not merely cited: BNF is the paper's own stated
middle ground ("we recommend BNF due to its adept balance between
efficiency and effectiveness") between BNP (`O(|V|)`, weakest locality gain
— a single ID-order pass with no iterative refinement) and BNS (highest
measured `OR(G)` gain but `O(β · o³ · ε · |V|)`, cubic in average out-degree
`o`); BNF's own complexity, `O(β · o · |V|)`, is linear in `o`, and the
paper's own measured construction overhead is small — "BNF only occupies
3%~10% of the graph index construction cost." STRAND registers `shuffle_
algorithm = 2` (BNF) as v0.1's default and registers all three (`1` = BNP,
`2` = BNF, `3` = BNS) as conforming — a writer MAY choose BNP for
construction-time speed or BNS for maximum locality, and `shuffle_algorithm`
exists precisely so a reader (or `strand-tools inspect`) can report which
was used, though the choice affects only I/O locality, never correctness:
Design §2's physical-slot addressing works identically regardless of *how*
slots were assigned.

**What Theorem 4.1 (`references/starling-sigmod2024.md`) forecloses, stated
precisely.** The block-shuffling problem — find the layout maximizing
`OR(G)` — is proven NP-hard with no polynomial-time finite-approximation-
factor algorithm, "unless P=NP." This is exactly why this RFC registers a
*specific heuristic algorithm* as the byte-determinism-relevant convention,
not an underspecified "optimal locality" goal: invariant 11 requires two
conforming writers given the same logical input to be able to reproduce the
same result, and "compute the optimal layout" is not a reproducible target
in the way "run BNF with these parameters" is (block size, `β`, `τ` are
writer-chosen tuning inputs, Non-goals — the *algorithm* is what's pinned,
not its hyperparameters, the same way RFC 0007's postings codec pins BP128's
layout without pinning a specific compressor invocation).

### 5. Query resolution: GreedySearch/BeamSearch without a compressed-code cache

STRAND's warm-tier reader runs DiskANN's own GreedySearch (Algorithm 1,
`references/diskann-neurips2019.md`), generalized to beam width `W` exactly
as DiskANN's own BeamSearch (§3.3) describes, over the physical-slot
addressing Design §2 registers:

1. Fetch the node-record blob's 16-byte header (or keep it resident — it is
   tiny) to obtain `entry_point_slot`. Fetch that slot's record: `L ← {s}`,
   `V ← ∅`, where `s` is the entry point.
2. While `L \ V ≠ ∅`: let `p* ← argmin_{p ∈ L\V} d(x_p, x_q)` over the
   *already-fetched* members of `L \ V`. Add `p*` to `V`. For each of
   `p*`'s `neighbor_slots` not already in `L`, **fetch that slot's record**
   (this is the real cost named below) and add it to `L`. If `|L| > L_param`,
   trim `L` to the `L_param` points closest to `x_q`.
3. Return the closest `k` points from `L`, filtering out any row-id present
   in the segment's deletion vector (invariant 2) from the *returned*
   result set — but not from `V`: a tombstoned node's edges remain usable
   for traversal until compaction rebuilds the graph without it, the same
   deferred-physical-removal model invariant 2 already states, and exactly
   the behavior IP-DiskANN's own abstract (`references/ip-diskann-
   arxiv-2502.13826.md`) names as the standard prior approach: "accumulate
   deletions in a batch and periodically consolidate, removing edges to
   deleted vertices."
4. Optionally issue `W` of step 2's fetches in parallel per iteration
   (DiskANN's own BeamSearch), trading some wasted bandwidth for fewer
   round trips — Design §8.

**The real cost this registers, stated rather than hidden: step 2 must
fetch every node it ever adds to `L`, not only nodes chosen as `p*`.**
DiskANN's own system avoids this by computing `argmin` over `L \ V` using
**compressed vectors held in memory** (`references/diskann-neurips2019.md`
§3.3: "Distance calculations to guide the best vertices... to read from disk
can be done using the compressed vectors") and only fetching a node's full
record — and therefore only paying a disk read — when it is *chosen* to
expand. This RFC does not register that compressed-code cache in v0.1
(Non-goals), so there is no way to estimate `d(x_p, x_q)` for a
not-yet-fetched candidate: every neighbor discovered at step 2 must be
fetched immediately to know its distance, whether or not it survives the
`L_param` trim. The Worked example traces this exactly — a query converging
in 2 real *hops* (expansions) still costs 4 real *fetches*, because two
trimmed-away candidates were fetched to learn they should be trimmed. This
is a real, structural consequence of the scoping choice in Non-goals, not
an oversight; Napkin math quantifies it at realistic scale and states
plainly what it costs relative to DiskANN's own published figures.

### 6. Merge semantics (invariant 1)

**`rebuild`** — confirming and detailing, not merely repeating, invariant
1's own one-line characterization ("graph indexes — neighbor structure does
not compose under concatenation"). Three independent, real reasons, not one
assertion:

**First, the `RobustPrune` pruning property is a global, not local,
guarantee.** Algorithm 2 (`references/diskann-neurips2019.md`) prunes a
node's candidate neighbor set `V` — which Algorithm 3's own construction
loop populates from `GreedySearch`'s *visited set over the graph as it
exists so far* — down to `R` edges satisfying the `α`-RNG property (§2.2:
"the distance to the query decreases by a multiplicative factor of `α > 1`
at every node along the search path"). Two segments' graphs were each
pruned against *their own* node population only; naively unioning their
edge sets does not preserve this property across the union, because neither
segment's construction ever considered the other segment's points as
pruning candidates. A query might reach a node that exists only in the
"foreign" half of a naively merged graph with no `α`-RNG-guaranteed short
path to get there.

**Second, DiskANN's own paper measures a real, honest cost even for its own
purpose-built merge procedure.** §4.3 (`references/diskann-neurips2019.md`)
describes a *construction-time* co-designed merge — partition the dataset
into `k = 40` overlapping k-means shards up front (`ℓ = 2` closest shards
per point specifically so the union has enough connectivity), build a
Vamana graph per shard, then union the edge sets — and still finds,
verbatim: "The single index outperforms the merged index, which traverses
more links to reach the same neighborhood, thus increasing search latency."
This is DiskANN's own best-case merge, engineered from the start for
mergeability, and it still costs real search-path length. STRAND's segments
are committed independently, with no shared construction-time coordination
between them (invariant 2's immutable-segment model, `spec/manifest.md`'s
commit protocol) — the overlapping-shard precondition DiskANN's own
technique depends on does not hold for two arbitrary, independently-built
STRAND segments, so even that paper's own most favorable merge story does
not transfer here as a cheaper alternative to `rebuild`.

**Third, the state-of-the-art in-place-update literature treats
consolidation (a rebuild-shaped operation) as the default it is trying to
escape, not a fallback.** IP-DiskANN's own abstract (`references/
ip-diskann-arxiv-2502.13826.md`) frames its contribution against "state-of-
the-art algorithms such as FreshDiskANN [which] accumulate deletions in a
batch and periodically consolidate, removing edges to deleted vertices and
modifying the graph" as the *prior standard*, not an edge case — even the
paper explicitly designed to avoid batch consolidation is arguing against a
world where consolidation (structurally a rebuild) is the norm. Nothing in
that paper claims an in-place algorithm for merging two *independently
constructed* graphs, only for applying a single graph's own streaming
insertions and deletions in place — a different problem than STRAND's
cross-segment merge.

**Stated plainly, the practical consequence:** a STRAND compaction that
merges graph-family segments MUST re-run Vamana's construction algorithm
(Algorithm 3) over the retained row-ids' vectors, producing a genuinely new
graph, new physical slot assignment, and new permutation directory — not a
byte-level splice of the constituent segments' node-record blobs. This is a
real, first-class compaction cost this format states honestly, the same way
`CLAUDE.md` §6 already states honestly that "write amplification is the
writer's problem" for immutable segments generally. Because this family
carries no quantization codebook (Non-goals, Design §5), it has none of RFC
0010's own cross-segment-codebook-compatibility question (RFC 0010 Design
§7, M2-8) — a genuine, if narrow, simplification relative to the cluster
family, worth naming as a positive contrast rather than only costs.

### 7. Tier and storage-class summary (invariants 7, 10)

Both blob types are `tier: warm` — invariant 7's own literal text ("assumes
NVMe-class latency; graph families live here") is this family's entire
registration rationale, not an incidental classification. Neither blob is
ever part of a cold-open wave; neither counts against the 100 MB
cold-open byte budget (`CLAUDE.md` §7), which governs `tier: cold-fetchable`
blobs specifically. Both are `storage-class: raw-mappable` (invariant 10):
dense wire bytes, decompression-free reads, matching invariant 10's
guidance that SIMD/alignment concerns belong to the reader's own
decompression arena, not wire bytes — doubly moot here since nothing in
this family is compressed at all.

### 8. Batch-shaped reads and kernel normativity (invariant 9)

DiskANN's own BeamSearch (§3.3, quoted in Design §5) already frames its
central optimization as a batch: fetch `W` candidates' records in one
parallel wave per hop rather than one at a time. This is naturally
`next_batch()`-shaped, matching invariant 9's stated API shape, with `W`
itself — unlike RFC 0010's fixed-32 FastScan batch, which is pinned by the
registered codec's own nibble-LUT width — a genuine **reader-side tuning
parameter**, exactly the latitude invariant 9 grants ("the batch *size* is
a stated per-implementation parameter with a recommended range"). DiskANN's
own stated recommended range, cited rather than invented: "`W` (say 4, 8)...
if `W` is too large, say 16 or more, then both compute and SSD bandwidth
could be wasted" (`references/diskann-neurips2019.md` §3.3) — this RFC
adopts that range as a non-normative recommendation, not a wire-format
requirement, since `W` affects only query-time I/O scheduling, never the
bytes this family's blobs contain. The distance computation itself
(Euclidean, per Design §5) has no registered codec of its own to make
normative the way RFC 0007's postings decode or RFC 0010's FastScan
estimator do — it is a scalar floating-point L2 distance over full-precision
`f32` vectors already resolved from Design §2's record layout, with no
quantization step in this family's v0.1 scope (Non-goals) requiring a
scalar-vs-SIMD equivalence pair at all.

## Worked example

A tiny, real, hand-checkable graph: `dims = 2` (Euclidean arithmetic
tractable by hand), `max_out_degree R = 2`. Five points at the corners of a
unit square plus one outlier, chosen for hand-checkable distances: `A`
(row-id 10) = `(0, 0)`, `B` (row-id 11) = `(1, 0)`, `C` (row-id 12) =
`(0, 1)`, `D` (row-id 13) = `(1, 1)`, `E` (row-id 14) = `(2, 2)`.

**Illustrative graph topology** (a plausible Vamana output for hand-
checkability, not a hand-execution of Algorithm 3 — the same convention RFC
0010's own worked example used for its illustrative centroids): directed
edges `A→{B,C}`, `B→{A,D}`, `C→{D,A}`, `D→{B,E}`, `E→{D}` — a 4-cycle
`A-B-D-C-A` plus a pendant `E`, `E`'s out-degree `1 < R`, deliberately
exercising the zero-padding rule. Entry point (illustrative medoid): `A`.

**Illustrative physical slot assignment** (a plausible BNF output, not a
hand-execution of Starling's Algorithm 1 — for the same reason): slot 0 =
`B`, slot 1 = `A`, slot 2 = `C`, slot 3 = `D`, slot 4 = `E`.

**Node-order permutation directory** (20 bytes: `node_count = 5` × u32,
indexed by local ordinal in row-id order `A, B, C, D, E`, giving each
ordinal's physical slot `[1, 0, 2, 3, 4]`):

`01 00 00 00 00 00 00 00 02 00 00 00 03 00 00 00 04 00 00 00`

**Graph node-record blob header** (16 bytes): `node_count = 5` →
`05 00 00 00`; `dims = 2` → `02 00 00 00`; `max_out_degree = 2` → `02`;
`shuffle_algorithm = 2` (BNF) → `02`; reserved → `00 00`; `entry_point_slot
= 1` (slot of `A`) → `01 00 00 00`.

Header bytes: `05 00 00 00 02 00 00 00 02 02 00 00 01 00 00 00`

`record_size = 8 + 2*4 + 4 + 4 + 2*4 = 32` bytes. Records, in physical slot
order (offsets relative to the blob's own start, header occupying
`[0, 16)`):

**Slot 0 = B** (offset `[16, 48)`): `row_id = 11` → `0B 00 00 00 00 00 00
00`; `vector = (1.0, 0.0)` → `00 00 80 3F 00 00 00 00`; neighbors `{A, D}`
→ physical `{1, 3}`, `neighbor_count = 2` → `02 00 00 00`; reserved →
`00 00 00 00`; `neighbor_slots = [1, 3]` → `01 00 00 00 03 00 00 00`.

`0B 00 00 00 00 00 00 00 00 00 80 3F 00 00 00 00 02 00 00 00 00 00 00 00 01 00 00 00 03 00 00 00`

**Slot 1 = A** (offset `[48, 80)`): `row_id = 10` → `0A 00 00 00 00 00 00
00`; `vector = (0.0, 0.0)` → `00 00 00 00 00 00 00 00`; neighbors `{B, C}`
→ physical `{0, 2}`, `neighbor_count = 2` → `02 00 00 00`; reserved →
`00 00 00 00`; `neighbor_slots = [0, 2]` → `00 00 00 00 02 00 00 00`.

`0A 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00 00 00 00 00 00 00 00 00 00 00 02 00 00 00`

**Slot 2 = C** (offset `[80, 112)`): `row_id = 12` → `0C 00 00 00 00 00 00
00`; `vector = (0.0, 1.0)` → `00 00 00 00 00 00 80 3F`; neighbors `{D, A}`
→ physical `{3, 1}`, `neighbor_count = 2` → `02 00 00 00`; reserved →
`00 00 00 00`; `neighbor_slots = [3, 1]` → `03 00 00 00 01 00 00 00`.

`0C 00 00 00 00 00 00 00 00 00 00 00 00 00 80 3F 02 00 00 00 00 00 00 00 03 00 00 00 01 00 00 00`

**Slot 3 = D** (offset `[112, 144)`): `row_id = 13` → `0D 00 00 00 00 00 00
00`; `vector = (1.0, 1.0)` → `00 00 80 3F 00 00 80 3F`; neighbors `{B, E}`
→ physical `{0, 4}`, `neighbor_count = 2` → `02 00 00 00`; reserved →
`00 00 00 00`; `neighbor_slots = [0, 4]` → `00 00 00 00 04 00 00 00`.

`0D 00 00 00 00 00 00 00 00 00 80 3F 00 00 80 3F 02 00 00 00 00 00 00 00 00 00 00 00 04 00 00 00`

**Slot 4 = E** (offset `[144, 176)`): `row_id = 14` → `0E 00 00 00 00 00 00
00`; `vector = (2.0, 2.0)` → `00 00 00 40 00 00 00 40`; neighbors `{D}` →
physical `{3}`, `neighbor_count = 1` → `01 00 00 00`; reserved →
`00 00 00 00`; `neighbor_slots = [3, 0]` — the second entry is
**zero-padding**, per Design §2's determinism rule, not a real edge to
slot 0 — → `03 00 00 00 00 00 00 00`.

`0E 00 00 00 00 00 00 00 00 00 00 40 00 00 00 40 01 00 00 00 00 00 00 00 03 00 00 00 00 00 00 00`

Total graph node-record blob: `16 + 5×32 = 176` bytes.

**Query trace** — `q = (0.9, 0.1)`, `L_param = 2`, `k = 1`, beam width `W =
1` (DiskANN's own stated `W = 1` ⇒ "resembles normal greedy search," the
simplest case to trace by hand):

Fetch `entry_point_slot = 1` (`A`, offset `[48, 80)`, 32 bytes) — 1 fetch.
`L = {A}`, `V = ∅`. `d(q, A) = √(0.9² + 0.1²) = √0.82 ≈ 0.9055`.

Iteration 1: `L \ V = {A}` → `p* = A`. `A`'s `neighbor_slots = [0, 2]` (`B`,
`C`), neither yet known — fetch both (2 fetches: slot 0 offset `[16, 48)`,
slot 2 offset `[80, 112)`). `d(q, B) = √(0.1² + 0.1²) = √0.02 ≈ 0.1414`.
`d(q, C) = √(0.9² + 0.9²) = √1.62 ≈ 1.2728`. `L = {A, B, C}`, `V = {A}`,
`|L| = 3 > L_param = 2` → trim to the 2 closest: `{B (0.1414), A (0.9055)}`,
dropping `C` (`1.2728`) — `C` was fetched only to learn it should be
dropped.

Iteration 2: `L \ V = {B, A} \ {A} = {B}` → `p* = B` (already fetched, no
new read for `B` itself). `B`'s `neighbor_slots = [1, 3]` (`A`, `D`); `A`
already known (`V`), `D` unknown — fetch it (1 fetch: slot 3, offset `[112,
144)`). `d(q, D) = √(0.1² + 0.9²) = √0.82 ≈ 0.9055`. `L = {A, B, D}`,
`V = {A, B}`, `|L| = 3 > 2` → trim to 2: `B (0.1414)` and a tie between `A`
and `D` at `0.9055`, broken toward `A` (first-seen) — `L = {B, A}`.

Iteration 3: `L \ V = {B, A} \ {A, B} = ∅` — loop ends.

Return closest `k = 1` from `L`: **`B`, row_id 11**, `d ≈ 0.1414` — the true
nearest neighbor of `q` among all five points (checking every point:
`A ≈ 0.9055`, `B ≈ 0.1414`, `C ≈ 1.2728`, `D ≈ 0.9055`, `E = √(1.1² + 1.9²)
= √4.82 ≈ 2.1954`), correctly returned by a search that visited (expanded)
only 2 of the 5 nodes — DiskANN's own claimed hop-count advantage
(`references/diskann-neurips2019.md` §4.2) borne out even at this tiny
scale — but that **fetched 4 of the 5** (`A`, `B`, `C`, `D`), because `C`
and `D` were both read merely to learn their distance before being trimmed
away, exactly Design §5's named cost of shipping without a compressed-code
cache. Real hops (expansions): 2. Real disk reads: 4. Both figures are
carried into Napkin math below.

## Napkin math (`CLAUDE.md` §7)

**Why invariant 3's cold-path accounting does not govern this family —
argued from its own text, not asserted.** Invariant 3 states its scope
precisely: "Object storage is the primary target, and **cold** access is
chunk-shaped and wave-addressable... Beyond the open, no **cold** read path
may depend on data-dependent pointer chasing." Invariant 7 draws the exact
line this family sits on the far side of: "Index blobs declare `tier:
cold-fetchable`... or `tier: warm` (assumes NVMe-class latency; graph
families live here)." Design §5's traversal is a textbook dependent chain
of pointer-chasing fetches — the literal thing invariant 3's one-wave rule
forbids for cold reads — registered here specifically because invariant 7
already carves out `tier: warm` as the declared exception, not because this
RFC is quietly ignoring invariant 3. `CLAUDE.md` §7's own calibration line
makes the stakes concrete: "50–200 dependent fetches at 100ms is 5–20
seconds — the graph-cold baseline being escaped." What §7's general
discipline *does* still require, independent of the cold-open budget:
"justified in GETs and bytes read, not only in CPU" — so this RFC still
owes real read-count and byte arithmetic, translated into the warm-tier
latency regime rather than exempted from arithmetic altogether.

**Per-fetch latency, real cited figures, not invented.** DiskANN's own
paper (`references/diskann-neurips2019.md` §1): "An inexpensive retail-grade
SSD requires a few hundred microseconds to serve a random read and can
service about ∼300K random reads per second," with "disk read latencies of
over a millisecond" only once a device is pushed to peak queue depth —
DiskANN's own stated target is "reducing... the number of round trip
requests to disk to under ten, preferably five" at that per-read cost. This
is two to three orders of magnitude below the ~100ms object-storage
round-trip figure `CLAUDE.md` §7 pins for the cold path — the entire reason
a dependent-fetch pattern is viable here and is not viable there.

**This RFC's own read-count honesty, quantified against DiskANN's own
figures.** The Worked example's tiny graph converged in 2 hops but 4
fetches — a `4/2 = 2×` fetch-to-hop ratio, `R`-bounded in the worst case
(each hop can discover up to `max_out_degree` new, never-before-seen
candidates, all requiring an immediate fetch under Design §5's no-cache
design). At DiskANN's own real, cited construction parameters (`R = 64`–
`128`, `references/diskann-neurips2019.md` §4), and at Starling's own cited
figure for an *unoptimized entry point* at scale (`references/starling-
sigmod2024.md` §3.1: "even searching for only the top-10 nearest neighbors
may generate a path of hundreds of hops" at "tens of millions of vectors"),
the honest worst-case fetch bound — `hops × R`, crediting no overlap
between hops' neighbor sets, since no source this RFC cites quantifies real
inter-hop overlap for Vamana graphs specifically — reaches into the tens of
thousands of fetches in the pessimistic case. This is a real number this
RFC states rather than rounds away, not a specific measured prediction: no
STRAND benchmark exists yet for this family (no crate code, this task's own
scope), and real graphs' neighbor-list overlap between hops is genuinely
unquantified in every source vendored here. Even at that pessimistic bound,
though, the *order of magnitude* stays the right side of the line this
family exists to be on: `10,000` fetches × `~100–300μs` (DiskANN's own
retail-SSD figure) is on the order of `1–3` seconds — slow relative to
DiskANN's own `<3ms` published figure (which depends on the compressed-code
cache and tuned entry points this RFC defers, Non-goals), but still one to
four orders of magnitude below the `5–20` seconds `CLAUDE.md` §7 states for
the equivalent *cold*, object-storage-latency version of the same
dependent-fetch pattern. The gap between "slower than DiskANN's own tuned
system" and "the cold-tier baseline this family exists to escape" is real
headroom this RFC's v0.1 scope has not spent, not a claim that v0.1's
unoptimized reader is fast.

**Node-record byte cost, per vector, at RFC 0010's own 768-dimension
convention, `R = 128` (DiskANN's own SIFT1B single-index parameter,
`references/diskann-neurips2019.md` §4.3):** `8 (row_id) + 768×4 (vector) +
4 (neighbor_count) + 4 (reserved) + 128×4 (neighbor_slots) = 8 + 3,072 + 8 +
512 = 3,600` bytes/node. Against RFC 0010's own 1-bit-RaBitQ cluster-family
floor of `116` bytes/vector (`rfcs/0010-vector-blob-cluster-family.md`
Napkin math), this is **≈31× larger per vector** — the direct, honest cost
of storing full-precision vectors with no quantization at all (Non-goals),
compounding the "explicitly second" framing with a real number: this family
is not only architecturally unsuited to cold object storage (the dependent-
fetch argument above), it is also, in its own v0.1 registration, far
heavier per vector than the family that is. At `10⁶` such nodes, the
node-record blob alone totals **≈3.6 GB** — a real figure worth stating
plainly, since nothing in this family's `tier: warm` registration bounds it
against any budget the way the 100 MB cold-open figure bounds the cluster
family; a warm-tier reader is assumed to have NVMe-class *local* capacity
for structures at this scale, not the object-storage economics invariant 3
governs.

**Permutation directory byte cost:** `node_count × 4` bytes — at RFC 0010's
own realistic segment scale (~760,000 768d vectors/segment, RFC 0010 Napkin
math), **≈3.04 MB** — small enough to keep memory-resident for the whole
segment, matching this blob's own narrow role (Design §3): a merge/rebuild
input and an occasional row-id-seeded query, never the per-hop hot path.

## Invariant-11 checklist

- **Endianness:** little-endian throughout — the node-record header, every
  node record's `row_id` (u64), `vector` (f32), `neighbor_count`/
  `neighbor_slots` (u32), and the permutation directory's own u32 entries.
- **Term sort order:** not applicable — this family has no term dictionary.
- **Chunk codec:** not applicable — both blob types are `storage-class:
  raw-mappable`, no chunk wrapper.
- **Checksums:** covered by each blob's own registry entry
  (`spec/container.md` §5, §6); no new checksum scope introduced here.
- **Codec-variant provenance:** the graph-construction algorithm (Vamana,
  Algorithms 1–3) and the node-order-permutation algorithm (Starling's BNF,
  Algorithm II) are both cited to their real, now-fully-vendored primary
  sources (`references/diskann-neurips2019.md`, `references/starling-
  sigmod2024.md`) and transcribed verbatim, not re-derived from memory
  (`CLAUDE.md` §3). Unlike RFC 0010's own quantization codec, this family's
  registered algorithms are **construction-time** procedures whose *output*
  (a graph, a physical order) is what the wire format pins — not a
  bit-exact reader-side decode kernel, since there is no per-read
  transform to reproduce (Design §8).
- **Padding determinism:** a node's unused `neighbor_slots` entries (beyond
  `neighbor_count`) and every record's reserved field MUST be zero-filled
  (Design §2) — not left to writer discretion.
- **Stochastic-transform provenance:** **not applicable to this family's
  wire bytes**, a real distinction from RFC 0010's RaBitQ rotation worth
  stating precisely rather than silently skipping. Invariant 11's
  provenance requirement targets a transform a *reader* must reproduce
  bit-exactly at read time (RaBitQ's rotation is applied to every query
  vector); Vamana's own randomness (Algorithm 3's random `R`-regular
  initial graph, its random permutation `σ` of construction order) is
  entirely construction-time — it shapes the final, deterministic graph
  structure that gets persisted, but no reader ever re-executes it. This is
  the same category RFC 0010's own Non-goals placed k-means centroid
  computation in: construction-time randomness producing a fixed output,
  not a read-time transform.
- **Golden files:** unlike RFC 0010's own worked example, which had to mark
  97% of its posting-list bytes opaque (a real, working quantization
  encoder was required to fill them), **every byte of this RFC's worked
  example is real and fully specified** — there is no opaque quantized
  payload in this family's v0.1 scope, since it carries no codec at all.
  `conformance/graph/toy-node-records.bin` (176 bytes) and `conformance/
  graph/toy-permutation-directory.bin` (20 bytes) are both directly
  derivable, byte-for-byte, from the Worked example above, once
  implemented — a stronger starting position than RFC 0010's own Approval-
  time golden-file gap.

## How this could be wrong

**Nearest grave: Indri and Galago** (`docs/lineage.md`: "well-specified
academic formats that died with their labs, because a format nobody's
production engine is economically forced to read is a paper artifact").
This is the sharpest-fitting grave in the map, not a generic pick, because
`CLAUDE.md`'s own mission sentence already frames this exact family as
secondary to the format's actual differentiated claim: "CIFF you can query
in place on S3, extended to vectors" — and this family, by its own `tier:
warm` registration, is explicitly *not* the "in place on S3" story. STRAND's
real argument for existing is the cold-object-storage story the cluster
family (RFC 0010) tells; a real adopter who wants graph-quality ANN recall
against NVMe-resident data already has DiskANN itself, Faiss, or any of a
dozen production graph-ANN libraries with years of tuning STRAND's own v0.1
graph blob does not match (Napkin math's own honest fetch-count admission).
If no real deployment ever chooses STRAND's own graph family over reaching
for one of those instead — plausible, since nothing about *this* RFC gives
a warm-tier deployment a reason STRAND's graph blob beats a purpose-built
graph-ANN library on its own turf — this becomes exactly the Indri/Galago
pattern: a well-specified structure, worked example and all, that nobody's
production engine is economically forced to read, because the one thing
this format uniquely offers (a shared row-ID space with the lexical and
cluster-vector families for cross-family fusion, invariant 1) does not,
by itself, outweigh giving up a decade of graph-ANN engine tuning. This RFC
does not resolve that risk — no design choice here can, since it is a
question about adoption, not correctness — but it names it plainly rather
than assuming registering the family is self-evidently worth the design
weight `docs/roadmap.md` itself already flagged as heavy ("the same weight
RFC 0010 itself carried").

**Second: the compressed-code-cache gap is a real, load-bearing performance
regression, not a cosmetic one.** Napkin math's own honest worst-case bound
— tens of thousands of fetches in the pessimistic case, `1`–`3` seconds of
wall time even at NVMe-class per-fetch latency — is not close to DiskANN's
own published `<3ms` figure, and the gap is not a rounding error: it is
the direct, structural consequence of Non-goals' choice not to register a
compressed-code cache in v0.1. A future implementation session that builds
against this RFC and then benchmarks it against DiskANN's own published
numbers as an implicit baseline will find a real, large discrepancy that
this RFC's own text predicts but does not close. Named here so that
discrepancy is not mistaken for a bug when it is found.

**Third: physical-slot-indexed adjacency (Design §2) has a real,
quantified rewrite cost this RFC did not shy away from naming, but also did
not fully cost out.** Every neighbor-slot value is coupled to physical
layout; unlike a row-id-based alternative (Alternatives considered), *any*
reshuffle — not only a merge/rebuild, but hypothetically re-running a
different `shuffle_algorithm` over an unchanged graph topology purely to
improve locality — requires rewriting every node record's `neighbor_slots`
array, not merely the permutation directory. This RFC did not compute the
byte cost of that rewrite (it is exactly the node-record blob's own full
size, Napkin math's `≈3.6 GB`/million-vectors figure), because v0.1 has no
registered "re-shuffle without a full rebuild" operation in the first place
(Design §6: any graph-family structural change is already a `rebuild`) —
but a future RFC that wants a cheaper reshuffle-only operation would find
this coupling is exactly what stands in its way, and should not discover
that as a surprise.

**Fourth, smaller: the NVMe-class latency assumption (invariant 7's own
phrase) is a real hardware-tier bet, though a broad one, not a narrow
vendor-specific one — worth distinguishing from the Optane grave rather
than conflating with it.** RFC 0010's own "How this could be wrong" named
Optane as its nearest grave for a different reason (a wire-format constant,
`kBatchSize = 32`, that turned out to be algorithm-shaped, not
hardware-shaped, after a real audit). This RFC's `tier: warm` label makes
no wire-format-level hardware assumption at all — no byte layout here
depends on any storage medium's block size or latency class, unlike the
Optane-era formats' own media-specific layouts. The bet is narrower and
softer: a *deployment-level* assumption that a "warm" tier means something
in the ~100μs–low-ms range, which the retail-SSD figures above ground for
2019-era commodity NVMe and remains true, if anything more so, on 2026
hardware. Named for completeness, not because this RFC finds it a live
risk on the same order as the first three.

## Alternatives considered

**Cluster-assignment-order physical placement, instead of Starling's block
shuffling.** Covered fully in Design §4 — rejected as v0.1's *default*
specifically for being unmeasured against a real, cited alternative that
is measured, for introducing a cross-family coupling no other blob family
has, and for conflicting with Vamana's own deliberately-long-range edges.
Not dismissed as worthless — named again in Open questions as a real,
concrete, empirically-testable follow-on hypothesis a future session could
actually measure `OR(G)` for.

**Row-id-indexed (rather than physical-slot-indexed) `neighbor_slots`.**
Would decouple adjacency-list content from physical layout entirely — a
reshuffle would only ever need to rewrite the (much smaller) permutation
directory, not every node record. Rejected as the v0.1 default because it
adds one directory lookup (`row_id → local_ordinal → slot`, an arithmetic
step plus one memory-resident-array read, cheap under invariant 7's own
NVMe-class-latency assumption) to *every single hop* of the hot,
dependent-fetch traversal path — exactly the path Design §5's own Napkin
math already shows is this family's dominant cost. Trading a large,
rare cost (full-blob rewrite on reshuffle, Design §2, How this could be
wrong) for a small, constant, per-hop cost paid on every query was judged
the better v0.1 default given this family's own stated purpose (minimizing
warm-tier query latency), but the trade is real and reversible by a future
RFC, not obviously correct forever.

**HNSW or NSG as the registered graph algorithm, instead of Vamana.**
Rejected on DiskANN's own head-to-head data, not by assumption:
`references/diskann-neurips2019.md` §4.2 measures Vamana converging in
"2–3 times fewer hops... compared to HNSW and NSG," directly attributed to
`α > 1`'s long-range edges, which neither HNSW nor NSG's own construction
tunes for ("both HNSW and NSG have no tunable parameter `α` and implicitly
use `α = 1`," §2.4). Hop count is the single most consequential metric for
a dependent-fetch-bound warm-tier reader (Napkin math), making this the
one graph algorithm in the literature this project has fetched whose own
authors measured the specific property this format's access pattern most
needs. NSG carries an additional real construction cost DiskANN's own paper
names directly — an approximate k-nearest-neighbor graph as a "time and
memory intensive" prerequisite (§2.4) — that Vamana avoids by starting from
a random graph (Algorithm 3).

**Gorder, as the node-order permutation algorithm.** Already rejected on
record before this RFC (`docs/data-structures.md`): "Gorder targets
graph-analytics traversal, not ANN beam search, and is not a candidate."
This RFC does not re-open that finding, only confirms it stands unchanged
by this session's own (independent) re-grounding of Starling.

## Open questions / follow-on RFCs

- **An in-memory compressed-code cache** (DiskANN's own PQ-routing layer,
  Design §5, Non-goals). The single highest-value follow-on: closing this
  gap is what would let this family's real fetch counts approach DiskANN's
  own published figures rather than the pessimistic bound Napkin math
  states. Needs its own wire-format registration (per-node compressed codes
  in physical-slot order, likely reusing RFC 0010's already-registered
  1-bit RaBitQ descriptor shape by reference, adapted to a non-FastScan-
  batched, single-node-at-a-time access pattern — a real, separate design
  question, not attempted here).
- **An in-memory navigation graph for query-aware entry points** (Starling's
  own §4.2 component, Non-goals). Independent of the compressed-code
  question — this optimization shortens the *hop count* itself by starting
  search closer to the query, rather than reducing the *cost per hop* the
  way a compressed-code cache does. Both are real, separately valuable,
  separately deferred.
- **Measuring `OR(G)` for cluster-assignment-order physical placement**
  (Alternatives considered, Design §4) — a real, concrete, cheap experiment
  a future session could run directly against Starling's own published
  metric, rather than leaving the "untested alternative" untested forever.
- **A real inter-hop neighbor-overlap measurement for Vamana graphs**,
  closing the honest gap Napkin math names: the pessimistic `hops × R`
  fetch bound credits no reuse between hops' discovered candidates, and no
  source vendored here quantifies the real figure. A future `bench/`
  harness building a real Vamana graph and tracing real queries against it
  (the same discipline RFC 0010's own `bench/src/vector_cold_open.rs`
  applied) would replace this RFC's worst-case bound with a measured one.
- **Real construction-time hyperparameter guidance** (`R`, `L`, `α`, block
  size, `β`, `τ`) for STRAND's own writer implementation, once one exists —
  this RFC cites DiskANN's and Starling's own published values as real
  reference points but does not prescribe STRAND-specific defaults, the
  same scoping choice RFC 0010 made for k-means centroid count.
- **ARM/SIMD kernel validation** for the scalar Euclidean-distance
  computation Design §8 leaves unregistered as a dedicated kernel — real,
  separate, measured work, the same category as RFC 0010's own deferred
  FastScan ARM validation and M2-7 (`docs/roadmap.md`).
- **A real `bench/` measurement for this family**, mirroring RFC 0010's own
  post-Approval `bench/src/vector_cold_open.rs` — the single biggest gap
  named throughout this RFC's own Status line and Napkin math: every
  latency and fetch-count figure here is literature-translated arithmetic,
  not a STRAND-measured result, because no crate code exists yet (this
  task's own scope).
