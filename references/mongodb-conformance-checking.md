# MongoDB — Conformance Checking Against TLA+ Specs

Vendored excerpt, not the full post. Source: A. Jesse Jiryu Davis (MongoDB
Distributed Systems Research Group), "Conformance Checking At MongoDB: Testing
That Our Code Matches Our TLA+ Specs," `https://emptysqua.re/blog/mongodb-
conformance-checking/` (mirrored at `mongodb.com/company/blog/engineering/
conformance-checking-at-mongodb-testing-our-code-matches-our-tla-specs`). Fetched
2026-08-18. Personal/company blog post, no stated reuse license; the passages
below are short quotations reproduced for citation and technical commentary, not a
claim of a permissive license on the full post. Cited by
`rfcs/0002-manifest-formal-verification.md` §2 for the real trace-checking failure
this RFC's sequencing (Workflow II before Workflow I) is designed to avoid — this
excerpt exists specifically to correct an earlier draft's embellishment of this
same anecdote.

---

On the chosen spec's granularity versus the real implementation's steps (the
RaftMongo.tla case):

> "when an old leader votes for a new one, *first* the old leader steps down,
> *then* the new leader steps up. The spec we chose for trace-checking wasn't
> focused on the election protocol, though, so for simplicity, the spec assumed
> these two actions happened at once."

On attempting to work around the mismatch:

> "We tried to paper over the difference with some post-processing in our Python
> script, but it never worked."

On the retrospective conclusion:

> [we] "decided we should have backtracked, making our spec much more complex and
> realistic, but we'd run out of time."

On the effort actually spent:

> "Judah and I put in 10 weeks of effort without successfully trace-checking one
> spec."

**What this does and does not support.** The 10-week figure describes the total
trace-checking effort across the experiment, not specifically how long the
leader-step-down/step-up mismatch alone took to surface or diagnose — the post
does not separately break out that duration. The post contains no language
characterizing the mismatch as "deterministic" or "100%-consistent" as opposed to
intermittent; it describes a structural difference between the spec's and the
implementation's action boundaries, which repeated post-processing attempts could
not paper over, but does not use the vocabulary of determinism versus probability
at all. An earlier draft of RFC 0002 attributed both the ~10-week figure and that
determinism framing to this source; neither is supported as stated, and both are
corrected here rather than restated.
