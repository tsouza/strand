# STRAND

STRAND — **S**parse-**T**erm **R**etrieval **A**nd **N**earest-neighbor
**D**ense-vector — is an open, engine-agnostic storage format for search
indexes, designed for object storage first. It carries lexical (BM25)
postings and dense-vector ANN indexes in one container, over one stable
64-bit row-ID space, so hybrid search can fuse scores across both without
a second identity system. The mission, in one sentence: *CIFF you can
query in place on S3, extended to vectors* — cold-open reads a small,
bounded number of large, independent byte ranges, never a dependent
pointer chain.

STRAND is a **format**: a normative specification plus a Rust reference
implementation. It is not a search engine, a database, or a query planner.
The one exception is a thin read-only DataFusion `TableProvider`, planned
for the M5 milestone, whose only job is to prove a stranger's query engine
can sit on top of the format without STRAND itself becoming one.

License: Apache-2.0. See `LICENSE`.

## Status

STRAND is pre-1.0 and under active development. M0 (container + manifest)
and M1 (lexical: postings, positions, term dictionary, block-max, filter
bitmaps, analyzer descriptors) are implemented. M2 (vectors: flat storage,
RaBitQ quantization, the cluster-family cold-native blob, closure
replication) is substantially implemented; the warm-tier graph blob family
is still an open RFC. M3 (compaction, hybrid fusion, the multi-segment
benchmark) and M4/M5 (interchange, a second reader, the DataFusion
consumer) are largely still open work. `docs/roadmap.md` is the current,
maintained breakdown of what's shipped and what's left, task by task, with
its dependency graph; `docs/milestones.md` states each milestone's full
scope and gating conditions.

The manifest's compare-and-swap commit protocol is formally verified: a
TLA+ model (`verification/`) is TLC-checked against real safety
invariants. A TLAPS mechanized proof and a Deterministic Simulation
Testing harness are the model's two remaining, not-yet-built artifacts,
and gate the start of M3's compaction work specifically, since a
merge/compaction commit is the highest-consequence action this protocol
has.

## Repository layout

```
spec/           normative specification, one chapter per layer
rfcs/           numbered design RFCs (draft / approved / implemented)
crates/
  strand-core     row-ID logic, container/chunk/block read-write,
                  the manifest CAS commit protocol, table metadata
  strand-lexical  postings, positions, term dictionary, filter bitmaps,
                  analyzer descriptors
  strand-vector   flat vectors, RaBitQ quantization, the cluster-family
                  vector blob, closure replication, codebook compatibility
  strand-tools    CLI: inspect a segment, import a tantivy field
bench/          benchmarks against real MinIO (byte budgets, latency,
                 throughput — see docs/benchmarks.md)
conformance/    golden files a second implementation can verify against,
                 without this repository's own code
references/     vendored primary sources every numeric or design claim
                 in this repository cites back to
docs/           the project constitution's supporting reference material
                 (lineage, data-structure defaults, the roadmap, the
                 settled/open ledger, condensed research grounding)
verification/   the TLA+ model of the manifest commit protocol
```

`CLAUDE.md` is this project's constitution: the non-negotiable design
invariants, the manifest's safety rules, the performance-benchmarking
discipline, and the writing and sourcing standards every spec chapter,
RFC, and commit message in this repository follows.

## Building and testing

Requires Rust (edition 2024) and Docker (several tests and all of
`bench/` exercise a real MinIO instance via `testcontainers`, not a mock).

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

`cargo test` (without `--workspace`) skips `bench/`, since it always links
the AWS SDK; run `cargo test -p strand-bench` or `--workspace` to reach it.
Individual benchmarks are real binaries, for example:

```sh
cargo run -p strand-bench --bin cold-open
cargo run -p strand-bench --bin vector-cold-open
```

Results land in `bench/results/*.json`, each cited from the spec or RFC
text its measurement grounds.

## Using `strand-tools`

```sh
# Decode a segment's footer and hotcache, print a report.
cargo run -p strand-tools -- inspect path/to/segment.bin

# Import one field of an existing tantivy index into a STRAND segment.
cargo run -p strand-tools -- convert \
  --index-dir path/to/tantivy-index --field body --output segment.bin
```

## Design documents

Start with `CLAUDE.md`, then `docs/lineage.md` for the prior art STRAND
builds on (Lucene, tantivy/Quickwit, PISA, Lance, Iceberg, Puffin, SPANN,
DiskANN, FastLanes, CIFF) and what didn't survive contact with production
(Indri, Galago, BitFunnel, the Optane-era formats, Pilosa). Each RFC in
`rfcs/` names the prior art it evolves from, includes a worked byte-level
example, and ships its own adversarial "how this could be wrong" review
before anything is implemented against it.
