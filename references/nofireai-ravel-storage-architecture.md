# NOFireAI/ravel: Storage, Catalog, and Commit-Protocol Architecture

Vendored excerpt and source-code findings, fetched 2026-08-20 to ground
`docs/lineage.md`'s ravel entry and close `docs/roadmap.md`'s D-4 item — real,
current primary sources, not a remembered summary of prior conversational
survey (`CLAUDE.md` §3). Every claim below was re-fetched live in this session
and checked against real source text or real, currently-committed Rust source,
not carried forward from an earlier session's summary without independent
re-verification.

**Sources (all fetched live via `gh api repos/NOFireAI/ravel/contents/<path>`,
`main` branch, 2026-08-20):**

- `repos/NOFireAI/ravel` (repository metadata) — `.license.spdx_id`.
- `README.md` — project overview, architecture summary, verification claims.
- `docs/catalog-and-mvcc.md` — key layout, sealed-hours definition and seal
  lemma, fold reconcile pass, commit tokens, snapshot resolution algorithm.
- `docs/consistency-model.md` — acknowledgement, visibility, read-your-write,
  and catalog-staleness semantics.
- `docs/object-store-contract.md` — the object-store trait, CAS/conditional-
  write semantics, and the mandatory-capabilities table.
- `crates/ravel-catalog/src/config.rs`, `crates/ravel-catalog/src/catalog.rs`
  — real, current Rust source for the two specific numeric defaults this
  entry depends on (`prefix_list_crossover_requests`,
  `fold_reconcile_window_hours`).

## License

`gh api repos/NOFireAI/ravel --jq '.license.spdx_id'` returns `Apache-2.0`,
confirmed live in this session via GitHub's license-detection API — the same
method this project already uses for RaBitQ and FastLanes (`CLAUDE.md` §1).
The README's own footer states the same: "License. Apache 2.0. See LICENSE."
41 stars, default branch `main`, at fetch time.

## What ravel is (README, "Why it is built this way")

> "Ravel is an OpenTelemetry-native database for metrics, logs, and traces
> where object storage is the only durable component. No write-ahead log. No
> ingester quorum. No StatefulSet. Kill any Ravel process at any instant, and
> every acknowledged write is still there."

> "Ravel makes the object store the first stop. An ingest shard builds an
> immutable columnar segment in memory, PUTs it, PUTs a commit record, and
> only then answers the exporter. The response carries a commit token. Pass
> that token back to a query and you read your own write, with no listing
> race."

This is a full telemetry datastore, not a search-index format: it ships its
own OTLP/Prometheus-Remote-Write ingest, PromQL and SQL query engines, an
alerting subsystem, a Kubernetes operator, and per-tenant encryption — none of
which STRAND has or wants. The structural overlap that matters to STRAND is
narrower and lives entirely at the storage/catalog layer, covered below.

## Key layout (`docs/catalog-and-mvcc.md`, "Key layout (all under one bucket
## root)")

Quoted in part, the rows relevant to the commit/catalog comparison:

> ```
> t/<tenant_hash>/m/l0/<shard>/<writer_id>.<epoch>.<seq>.<hash16>.rseg      data
> t/<tenant_hash>/m/c/<shard>/<ingest_hour>/<writer_id>.<epoch>.<seq>.cmt   commit
> t/<tenant_hash>/m/l1/<shard>/<ingest_hour>/<input_set_hash16>.<part:04>.<hash16>.rseg   L1 part
> t/<tenant_hash>/m/c/<shard>/<ingest_hour>/l1.<input_set_hash16>.cmt       compaction record
> t/<tenant_hash>/catalog/<signal>/snap/<watermark>.<hash16>.csnap         snapshot part (immutable)
> t/<tenant_hash>/catalog/<signal>/HEAD                                    head pointer (mutable, CAS)
> ```

Every data object (`l0`/`l1` segments), every commit record, and every
snapshot part is immutable and content-addressed (hash-suffixed); the only
mutable object in the entire keyspace is the per-tenant, per-signal `HEAD`
pointer, updated exclusively by CAS. This is architecturally the same shape as
STRAND's manifest layer (`CLAUDE.md` §6): immutable segments, immutable
snapshot metadata, one mutable current pointer, commit by CAS on that pointer.

## Commit protocol and CAS (`docs/object-store-contract.md`, "Mandatory
## capabilities (production)")

> "`Etag` is content identity (equality checks). `Version` is an opaque
> precondition token for CAS: S3 etag, GCS generation, Azure etag. The two
> coincide on S3 and differ elsewhere; commit-protocol code uses only
> `Version` for CAS and only `Etag` for content-identity assertions."

The mandatory-capabilities table, quoted verbatim:

| Capability | Flag | Used by |
|---|---|---|
| Strongly consistent create + read-after-write | consistent_read | commit visibility |
| Strongly consistent list-after-write | consistent_list | commit discovery |
| `CreateIfAbsent` conditional put | create_if_absent | commit records, data objects |
| Version CAS put | cas_version | catalog HEAD pointers |
| Byte-range + suffix reads | suffix_range | footer-first segment reads |
| Paginated prefix listing | prefix_list | discovery, GC |

> "Conditional-put failure maps by mode: under `CreateIfAbsent` a
> precondition failure surfaces as `AlreadyExists`; under `CasVersion` as
> `PreconditionFailed`. A conformance test asserts both against real S3 and
> MinIO (the memory oracle alone cannot catch a uniform mapping)."

Data and commit records use `CreateIfAbsent` (append-only, no CAS race);
`HEAD` alone uses `Version` CAS. This maps directly onto STRAND's own
`CLAUDE.md` §6 distinction between segment/manifest-file writes and the single
current pointer's compare-and-swap.

Acknowledgement is strict by default (`docs/consistency-model.md`,
"Acknowledgement semantics"):

> "Strict mode (default): An OTLP export is acknowledged only after every
> batch it contributed to has (a) its L0 data object durably stored and (b)
> its commit record created... After a strict ack, no crash of any Ravel
> process may lose that data. Object-store durability is the floor: data
> survives anything the object store survives."

## Sealed hours and the seal lemma (`docs/catalog-and-mvcc.md`, "Sealed
## hours")

Ravel batches commit records into ingest-hour buckets and defines, for bucket
`H`, a wall-clock condition under which that bucket's commit-record set is
provably final — quoted in full because the precise arithmetic is the point:

> "Definition. For an ingest-hour bucket H (unix hours), let
> `end(H) = (H + 1) * 3600 s`. H is **sealed** at wall time T iff:
>
> ```
> T >= end(H) + max_flush_lifetime + clock_skew_allowance + fold_safety_margin
> ```
>
> with `max_flush_lifetime` (default 1 h) and `clock_skew_allowance`
> (default 5 m) as configured for the tenant's writers and catalog, and
> `fold_safety_margin` a catalog config (default 15 m)."

> "Seal lemma: the commit-record set of a sealed bucket is immutable. Proof
> sketch from the rules above: `ingest_hour_bucket` is pinned at flush open
> from the writer's clock ("Pinned flush identity"); a flush older than
> `max_flush_lifetime` is abandoned and MUST NOT be published afterward (GC
> interlock, ADR-0010 §11); so the last possible publish for bucket H happens
> before `end(H) + max_flush_lifetime` on the writer's clock, which is
> within `clock_skew_allowance` of true time. `fold_safety_margin` absorbs
> the folder's own clock error. Therefore one strongly consistent LIST of a
> sealed bucket (the store contract's listing guarantee, the same one orphan
> GC relies on, docs/consistency-model.md "Deletion and GC") observes the
> full and final set."

This lemma is the load-bearing correctness argument for everything downstream
of it: once an hour bucket is sealed, the catalog's periodic "fold" (below)
may summarize it once and carry that summary forward by reference forever,
never re-listing it in the common case.

## Fold reconcile pass and the prefix-scan crossover
(`docs/catalog-and-mvcc.md`, "Fold reconcile pass (ADR-0063 section 4)";
`docs/catalog-and-mvcc.md`, snapshot-resolution phase 1)

The catalog's background "fold" precomputes an immutable snapshot part per
sealed hour so that query-time snapshot resolution can skip listing
everything below a watermark. Because a compaction record, retention
tombstone, or selective-erasure rewrite can land in a bucket long after that
bucket sealed and was already folded, a naive incremental fold (which only
lists hours strictly after the previous watermark) would silently miss such
late records forever. The fix is a bounded reconcile pass, quoted:

> "**Window.** Hours in `[watermark_hour_old - fold_reconcile_window_hours,
> watermark_hour_old]`, inclusive at both ends... `fold_reconcile_window_hours`
> defaults to 26 hours: `protection_horizon` (24 h, the age gate before the
> sweeper may physically delete a superseded compaction input) plus slack.
> Because the sweeper only deletes an input after that horizon, any record
> whose supersession could invalidate a snapshot entry is observed by a
> reconcile pass before its inputs can disappear."

> "**Cheap common case.** Each window bucket is re-listed... A bucket
> holding only immutable L0 records cannot have changed since it was folded
> (seal lemma above), so it is skipped with no record GET."

Independently confirmed against real, currently-committed source
(`crates/ravel-catalog/src/config.rs`, line 85):

```rust
pub const DEFAULT_FOLD_RECONCILE_WINDOW_HOURS: u32 = 26;
```

Separately, snapshot resolution itself (not the fold) switches between two
listing strategies depending on query-window width, quoted from
`docs/catalog-and-mvcc.md`'s "Snapshot resolution (Phase 1)":

> "- **Per-bucket loop** (narrow/warm windows): LIST
>   `t/<th>/m/c/<shard>/<hour>/` per bucket (paginated; callers dedup keys).
>   Cost `shard_count * hours`, one LIST per bucket, empty buckets included...
> - **Prefix scan** (wide windows, at or above
>   `prefix_list_crossover_requests` suffix buckets, default 720): one
>   drained recursive LIST per shard over `t/<th>/m/c/<shard>/` (paginated),
>   grouping the returned keys client-side by `(shard, ingest_hour)` and
>   keeping the buckets in the window. Cost `O(objects / page_size)`,
>   independent of window width..."

Independently confirmed against real source
(`crates/ravel-catalog/src/config.rs`, line 124):

```rust
pub const DEFAULT_PREFIX_LIST_CROSSOVER_REQUESTS: u64 = 720;
```

**Correction to an earlier conversational characterization of this session's
own prior survey.** An earlier summary of this same finding, carried into
this task's own prompt, attributed both defaults to
`crates/ravel-catalog/src/fold.rs`. Live re-verification in this session finds
that attribution imprecise: `fold.rs` exists (4,137 lines, confirmed via
`gh api repos/NOFireAI/ravel/contents/crates/ravel-catalog/src`) and
extensively *references* both config fields (`fold_reconcile_window_hours`
appears at `fold.rs` lines 143, 588, 950, 981, 3504, 3623), but the fields
themselves are **declared and defaulted in `crates/ravel-catalog/src/
config.rs`** (`DEFAULT_FOLD_RECONCILE_WINDOW_HOURS` at line 85,
`DEFAULT_PREFIX_LIST_CROSSOVER_REQUESTS` at line 124, both wired into
`CatalogConfig::default()`), and the `prefix_list_crossover_requests` switch
itself is consumed in `crates/ravel-catalog/src/catalog.rs` (line 976,
`listing_suffix_buckets >= self.config.prefix_list_crossover_requests`), not
in `fold.rs` at all — the fold and the resolve-time prefix-scan crossover are
two different mechanisms in two different files that happen to share
`CatalogConfig`. This is exactly the failure mode `CLAUDE.md` §3 warns
against (trusting a remembered/summarized location instead of the live
source) and is recorded here so it is not repeated.

## Terminology summary

"Sealed hour," "seal lemma," "fold," and "reconcile pass" are ravel's own
terms of art, used above exactly as the source docs use them — none are
STRAND terminology and none are proposed for adoption here; they are
recorded for the lineage citation's own precision.
