# AWS PObserve and the P Language

Vendored excerpt, not the full documentation. Source: the P programming language
project's own documentation site, `https://p-org.github.io/P/whatisP/` (the P
project's GitHub repository, `p-org/P`, is MIT licensed, verified via the GitHub
API). Fetched 2026-08-18. Cited by `rfcs/0002-manifest-formal-verification.md` §2
for what PObserve actually does — this excerpt exists specifically to correct an
earlier draft of that RFC, which cited PObserve for the opposite claim.

---

PObserve bridges the gap between design-time verification and runtime behavior. It
validates service logs and execution traces from the running system against the
same P specification monitors that were verified during formal verification —
feeding events through P specification monitors to check global conformance and
identify violations. This extends formal verification guarantees "from design time
into production": the P specification is written and verified first, during
design, and PObserve then checks whether the production system's actual logs and
traces conform to that already-verified design, after the fact — not the reverse.
PObserve can validate service logs against P specifications in both testing and
production.

Teams across AWS building services including Amazon S3, EBS, DynamoDB, MemoryDB,
Aurora, EC2, and IoT have used P to reason about the correctness of their system
designs.

**Why this is cited, and for what.** PObserve's shape — observe real production
execution, reconcile the resulting traces against a pre-existing spec after the
fact — is what `rfcs/0002-manifest-formal-verification.md` calls "Workflow I," not
"Workflow II." An earlier draft of that RFC cited PObserve as evidence that
sequencing "spec drives implementation" (Workflow II) before "implementation drives
verification, checked after the fact" (Workflow I) is the direction proven to work
in production. That is backwards: PObserve is itself a production example of
Workflow I working, once the observation pipeline (structured logging with the
right event granularity) is engineered carefully. It is correctly cited as evidence
that Workflow I is *achievable* in production and worth attempting as this RFC's
second phase — not as grounds for attempting it first.
