# R5 — Manifest and commit sources (AWS conditional writes, Vanlightly's Iceberg analysis)

Vendored excerpts. Fetched 2026-08-18.

Cited by: `docs/research/README.md` R5, `docs/lineage.md` ("From Iceberg"),
RFC 0001 §3 (`rfcs/0001-container-rowid-manifest.md`).

The Iceberg spec's "Optimistic Concurrency" section itself is already vendored —
`references/iceberg-commit-conflict-resolution-and-retry.md` — and confirmed
byte-level against `apache/iceberg`'s `format/spec.md`; not re-fetched here.

## AWS S3 conditional writes — the two announcements

**If-None-Match (create-if-absent).** Source:
`aws.amazon.com/about-aws/whats-new/2024/08/amazon-s3-conditional-writes/`.
Published **August 20, 2024**.

> "Amazon S3 adds support for conditional writes that can check for the existence of
> an object before creating it." ... "This capability can help you more easily
> prevent applications from overwriting any existing objects when uploading data."

Available via the `if-none-match` HTTP header on `PutObject`/`CompleteMultipartUpload`,
all AWS Regions, no additional charge.

**If-Match (ETag CAS on existing objects).** Source:
`aws.amazon.com/about-aws/whats-new/2024/11/amazon-s3-functionality-conditional-writes/`.
Published **November 25, 2024** (one day off the "November 26" some secondary
sources report — this is the date on AWS's own announcement page, fetched directly).

> Clients "can now perform conditional-write checks on an object's ETag by
> specifying it via the HTTP if-match header in the API request. S3 then evaluates
> if the object's ETag matches the value provided in the API request before
> committing the write."

Both dates confirm `docs/research/README.md` R5's "If-None-Match GA in all regions
August 20, 2024; If-Match ETag CAS November 26, 2024" — the November date is
corrected here by one day (25th, not 26th) against AWS's own primary source; a minor
drift, noted rather than silently perpetuated.

## Jack Vanlightly — "Understanding Apache Iceberg's Consistency Model, Part 3"

**Source:** `jack-vanlightly.com/analyses/2024/8/6/apache-icebergs-consistency-model-part-3`.
Part of a four-table-format series (Iceberg, Hudi, Paimon, Delta Lake); STRAND's
`references/vanlightly-delta-lake-tla-plus.md` already vendors the Delta Lake
installment from the same series — this is the Iceberg-specific one R5 additionally
names.

The post builds a formal model (in Fizzbee, not TLA+ — the author's stated reason:
"expressing a lot of the table format logic in Python [was] easier than TLA+") of
Iceberg's writer/object-store/catalog roles and four commit operations, and checks
three safety properties (sequential sequence numbers, no dangling delete files,
consistent reads across table versions).

**The finding:** a real concurrency anomaly under merge-on-read with snapshot
isolation — Spark's "No new delete files" validation check, which prevents an
UPDATE/MERGE from silently un-deleting a row a concurrent DELETE removed, is not
enabled for DELETE operations themselves. Two concurrent operations (an UPDATE and a
DELETE on the same row) can both commit, violating snapshot isolation. Vanlightly's
own conclusion: "Iceberg includes all the necessary checks; it's up to the compute
engine to correctly enable them" — the format provides the mechanism, but leaves
its correct use to each engine, a design trade-off distinct from what caused the
anomaly.

**Relevance to STRAND, stated plainly rather than implied.** This finding is about
Iceberg's per-row delete-file validation, a mechanism STRAND's manifest does not
have (deletion vectors are Roaring bitmaps at the row-ID level, not per-file delete
records with engine-toggleable validation). It is not a defect this project inherits.
Its value here is as a second, independent data point — alongside RFC 0002's own
TLA+ effort — that formal modeling of a table-format commit protocol finds real bugs
missed by design review and testing, reinforcing the working-method rationale
`CLAUDE.md` §3 and RFC 0002 already state.
