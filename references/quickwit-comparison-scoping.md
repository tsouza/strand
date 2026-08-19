# Quickwit — Scoping Notes for a Real Cold-Open Comparison

Vendored excerpts and CLI/API facts (fetched via `WebFetch`/`WebSearch`, 2026-08-19),
grounding the "Quickwit adapter" line in `docs/benchmarks.md` and answering, with
real facts rather than assumption, what a black-box cold-open comparison against
STRAND's `bench/src/field_cold_open.rs` would require. This is scoping research, not
an adapter build — no code was written against these facts yet. License: Quickwit is
Apache-2.0 under Datadog (already recorded in `docs/lineage.md`).

## What already transfers from the tantivy comparison

Quickwit is tantivy's own indexing internals plus an S3-native split format and
hotcache (`docs/lineage.md`) — it does not reinvent tantivy's postings codec. The
byte-size comparison already run against real tantivy
(`bench/src/tantivy_index.rs`, `docs/ledger.md`) already substantially reflects
Quickwit's own on-disk format for postings/positions/term-dictionary. What a
Quickwit-specific comparison adds is the one thing tantivy cannot measure at all:
a real GET-count and latency number for opening a split cold from S3, to put next
to STRAND's measured 3 GETs (`bench/src/field_cold_open.rs`).

## Deployment

Single static binary or Docker image (`quickwit/quickwit:0.9.0` referenced in the
current quickstart), runs all services in one process by default:

    quickwit run --config=./config/quickwit.yaml

Storage backend (S3/MinIO) is configured in the node config file, not CLI flags —
`QW_DEFAULT_INDEX_ROOT_URI=s3://<bucket>/indexes` and `QW_S3_ENDPOINT=http://<minio-host>:9000`
environment variables, per the published MinIO/Quickwit Docker Compose example. This
plugs directly into the same MinIO container `bench/src/lib.rs`'s `with_minio` already
starts via `testcontainers` — no second storage backend needed, only a second
containerized process pointed at the same endpoint.

## Fair-comparison tokenizer: `whitespace`

Quickwit's doc-mapping config exposes named tokenizers per text field
(`spec quoted from the index-config reference`):

- `raw` — "Does not process nor tokenize text... Filters out tokens larger than 255 bytes."
- `raw_lowercase` — same, plus lowercasing.
- `default` — "Chops the text on according to whitespace and punctuation... converts to lowercase."
- `en_stem` — `default` plus stemming.
- **`whitespace`** — "Chops the text on according to whitespace only. Doesn't remove
  long tokens or converts to lowercase."

`whitespace` is the fair-comparison choice: feed the same
`strand_lexical::analyzer::analyze_lucene_en_word_only`-produced, space-joined
token string used for the tantivy `PreTokenizedString` trick, and Quickwit will
tokenize on whitespace only — identical resulting tokens, no double-processing.
Doc-mapping must also declare `record: position` on the field for phrase queries to
be answerable at all (Quickwit's own doc-mapping reference; unconfirmed exact
YAML key name pending a real config draft — verify against a live index-config
example before relying on it).

## Ingestion and querying

Real, current CLI/REST commands (`quickwit.io/docs/get-started/quickstart`,
`quickwit.io/docs/reference/cli`):

    quickwit index create --index-config ./config.yaml
    curl -XPOST "http://127.0.0.1:7280/api/v1/<index>/ingest?commit=force" --data-binary @docs.ndjson
    curl "http://127.0.0.1:7280/api/v1/<index>/search?query=..."

NDJSON ingest, real REST endpoints, no exotic tooling — this part is comparably
simple to the tantivy comparison's own `IndexWriter`/`Searcher` calls.

## GET-count measurement: a real, usable metric exists

Quickwit exposes Prometheus metrics at `/metrics`, including
(`quickwit.io/docs/main-branch/reference/metrics`):

- **`quickwit_storage_object_storage_gets_total`** — "Number of objects fetched."
- `quickwit_storage_object_storage_puts_total`, `..._puts_parts`,
  `..._download_num_bytes`.

This is a real, direct counter — scrape `/metrics`, take the delta across one
query, exactly the same shape as `bench/src/lib.rs`'s own `CountingStore` wrapper
does for STRAND's own store calls. No need to instrument MinIO itself (a proxy or
access-log approach was the fallback plan before this was found; it is not needed).

## The hard part: Quickwit is a caching server, not a stateless library call

STRAND's own cold-open benchmark gets a genuinely cold measurement on every
iteration for free: `bench/src/field_cold_open.rs` calls a plain `ConditionalStore`
directly, with no process-level cache anywhere in that path, so 30 loop iterations
are 30 independently cold opens by construction.

Quickwit is architecturally the opposite: a long-running server process with named
caches for exactly this scenario (`fastfields`, `shortlived`, `splitfooter`, per the
metrics reference) — a query is not necessarily cold internally after the first one,
even against a "fresh" HTTP client. Getting a genuinely cold measurement per
iteration requires one of:

1. **Restart the Quickwit process between iterations.** Correct, but expensive per
   iteration (server startup is not free) and changes what's being measured
   (process-startup cost gets folded in unless carefully excluded).
2. **Query a fresh index/split per iteration** (e.g., N separate small indexes,
   query each exactly once) — avoids the restart cost but multiplies indexing work
   and needs N real indexes, not one.
3. **Find and use an explicit cache-bypass/eviction mechanism**, if Quickwit exposes
   one — not confirmed to exist in this research pass; the metrics reference lists
   cache hit/miss counters but no explicit "disable cache for this query" flag was
   found. Needs a direct check against the real config/CLI reference (or the source)
   before assuming it exists.

None of the three is free. This is the genuine scope difference from the tantivy
comparison, which needed zero special handling for this because a plain library
call has no server-side cache to defeat.

## Effort estimate

Meaningfully larger than the tantivy comparison (a Rust crate dependency and one
new bench binary). This needs: a second real process under `testcontainers`
(a `GenericImage` container, since no `testcontainers-modules::quickwit` was found
in this research pass — needs re-verification, not assumed absent), YAML config
authoring (node config plus a doc-mapping config, both first drafts unverified
against a live Quickwit instance), REST calls for ingest/query in place of a linked
API, `/metrics` scraping and delta computation for the GET count, and — the real
open design question — a defensible, honestly-stated methodology for the cold-cache
problem above. Order of magnitude: a focused, dedicated task on its own, not a
same-session extension of the tantivy work.

## Open items before implementation starts

- Confirm `record: position`'s exact YAML key/value against a real, live index
  config (not assumed from the reference summary above).
- Confirm whether a `testcontainers`-compatible Quickwit image/module exists, or a
  raw `GenericImage` container definition is needed from scratch.
- Confirm whether an explicit cache-bypass exists before committing to the
  restart-per-iteration or N-separate-indexes methodology.
- Decide and document which methodology is used, with the same honesty this
  project already applies to STRAND's own MinIO-on-`localhost` caveat
  (`docs/ledger.md`'s field-cold-open entry) — a Quickwit number measured on
  `localhost` still only confirms the GET-count half of any comparison, not
  real-network tail latency, exactly as STRAND's own number does.
