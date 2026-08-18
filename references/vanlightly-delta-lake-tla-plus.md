# Jack Vanlightly — Delta Lake TLA+ Consistency Model

Vendored excerpt, not the full post or spec. Source: Jack Vanlightly,
"Understanding Delta Lake's consistency model,"
`https://jack-vanlightly.com/analyses/2024/4/29/understanding-delta-lakes-
consistency-model`, and the corresponding TLA+ source,
`github.com/Vanlightly/table-formats-tlaplus`, `delta-lake/basic_cow/
delta_lake.tla` (repository confirmed MIT licensed via the GitHub API). Fetched
2026-08-18. Cited by `rfcs/0002-manifest-formal-verification.md` §1/§4 for the
coarse, per-attempt action granularity this RFC's own model follows.

---

The model's four top-level actions, quoted from the post:

> "**StartOperation**: The delta log is loaded and the commit version recorded as
> the table version + 1."

> "**ReadDataFiles**: Any relevant data files are loaded into memory."

> "**WriteDataFiles**: The new files are optimistically written to the object
> store."

> "**TryCommitTxn**: The writer attempts to write the delta log entry. If it
> fails, it performs the data conflict check."

**Attribution correction.** These four action names and their descriptions are
Vanlightly's. The contrast this RFC draws against Raft's fine-grained,
message-passing TLA+ specs (`raft.tla`, part of the `spacejam/tla-rust` example
set — `references/spacejam-tla-rust.md`) is this RFC's own framing, not a
comparison Vanlightly's post makes; the post does not discuss Raft or
message-level modeling at all. An earlier draft of RFC 0002 implied the
granularity contrast itself came from Vanlightly's source; it does not, and the
attribution is corrected here.
