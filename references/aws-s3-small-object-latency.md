# AWS S3 — small-object latency (whitepaper)

Vendored excerpt, not the full document. Source: "Best Practices Design Patterns:
Optimizing Amazon S3 Performance," AWS Whitepaper,
`https://docs.aws.amazon.com/pdfs/whitepapers/latest/s3-optimizing-performance-best-practices/s3-optimizing-performance-best-practices.pdf`
(HTML equivalent: `https://docs.aws.amazon.com/AmazonS3/latest/userguide/optimizing-performance-guidelines.html`
and `.../optimizing-performance-design-patterns.html`). Initial publication date
June 2019; document copyright dated 2026 at fetch time, so this is AWS's current,
actively maintained guidance, not a stale snapshot. Fetched 2026-08-19.

Cited by: `CLAUDE.md` §7 (the napkin-math rule's tail figure for SLO discussion),
`docs/ledger.md`'s "Pending figures" entry, `docs/milestones.md`'s M0 entry.

## The small-object latency figure

From the Introduction (page 2):

> "Other applications are sensitive to latency, such as social media messaging
> applications. These applications can achieve consistent small object latencies
> (and first-byte-out latencies for larger objects) of roughly 100–200
> milliseconds."

This is AWS's own stated figure for the latency well-tuned, latency-sensitive
applications achieve on small S3 object reads. **It is not a named percentile** —
the whitepaper does not say "p90," "p99," or any other percentile word anywhere
near this sentence, so it must not be cited as one. It is presented as the
"consistent" (i.e., typical, steady-state) latency band such applications achieve,
which is the closest current primary-source figure this project has found to the
SLO-discussion tail figure `CLAUDE.md` §7 needs. The previously drafted "~250ms
p90" figure was never traced to any AWS source and is retracted, not softened,
per `CLAUDE.md` §2's rule that a number without a vendored source sentence is
deleted rather than kept as an approximation.

## A related figure, not adopted here

The "Timeouts and Retries" section (page 8) adds a second, adjacent data point,
kept here for completeness but not used as the pinned §7 figure because it is a
median, not a tail statistic, and is phrased as a retry-timing guideline rather
than an SLO figure:

> "When you make smaller requests (for example, less than 512 KB), where median
> latencies are often in the tens of milliseconds range, a good guideline is to
> retry a GET or PUT operation after 2 seconds."

## Why this is the figure now cited, and what it does not resolve

This closes the "vendor the source sentence" half of the `CLAUDE.md` §7 pending
item: the figure is real, current, and AWS's own. It does **not** close the
"or replace it with a measured MinIO/S3 tail figure at M0" half — that
real-network measurement (MinIO with injected latency, or real S3) is a separate,
still-open item, distinct from this vendoring debt; `bench/results/cold-open.json`
so far only measures against MinIO on localhost with no injected network latency,
confirming the GET-count half of invariant 3, not a real-network tail figure.
