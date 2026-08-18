# FMDSE — Formal Model Guided Conformance Testing for Blockchains

Vendored excerpt, not the full paper. Source: Filip Drobnjakovic, Amir Kashapov,
Matija Kupresanin, Bernhard Scholz, Pavle Subotic, "Formal Model Guided
Conformance Testing for Blockchains," arXiv:2501.08550 (submitted 15 Jan 2025,
revised 18 Jan 2025), `https://arxiv.org/abs/2501.08550` and
`https://arxiv.org/html/2501.08550v2`. Fetched 2026-08-18. Short quotation of the
abstract and evaluation figures, reproduced for citation and technical commentary
under arXiv's standard terms for reuse of publicly posted preprints, not a claim
of a specific open license on the full paper. Cited by
`rfcs/0002-manifest-formal-verification.md` §§2, 5 for the Workflow I/Workflow II
terminology this RFC adopts, and for the reported engineering cost of a
comparably-scoped dual-validation effort.

---

**Abstract:**

> "Modern blockchains increasingly consist of multiple clients that implement a
> single blockchain protocol. If there is a semantic mismatch between the
> protocol implementations, the blockchain can permanently split and introduce
> new attack vectors. Current ad-hoc test suites for client implementations are
> not sufficient to ensure a high degree of protocol conformance. As an
> alternative, we present a framework that performs protocol conformance testing
> using a formal model of the protocol and an implementation running inside a
> deterministic blockchain simulator. Our framework consists of two complementary
> workflows that use the components as trace generators and checkers. Our insight
> is that both workflows are needed to detect all types of violations. We have
> applied and demonstrated the utility of our framework on an industrial strength
> consensus protocol."

**Workflow definitions** (both are the paper's own terms, retained by this RFC):

- **Workflow I:** random traces are generated *from the simulator* (the
  implementation) via fuzzing, abstracted into model traces, and validated
  against the formal model using model checking — implementation drives,
  checked against the spec afterward.
- **Workflow II:** random model traces are generated *from the formal model* by
  fuzzing it via the TLC model checker, and the simulator's ability to execute
  equivalent traces is verified by checking state equivalence during execution —
  spec drives, implementation checked against it directly, with the action
  sequence known in advance.

**Reported effort and size** (the paper's own case study, an industrial consensus
protocol implemented in Go):

> "The specification is written in TLA+ and consists of 675 lines of code and
> 1282 lines of proof code."

> "The time for the proof to be machine-checked took 123 seconds (approx. 2
> minutes)."

> "The consensus protocol implementation is written in golang and consists of
> 2411 lines of code."

> "The deterministic simulator consists of 1000 lines of code for the core
> driver and another 1000 lines of code for the network/DES abstractions."

> "The effort to design, understand, specify and prove the protocol required
> approximately 2 person-months, distributed between 3 people."

**Correction to an earlier draft.** RFC 0002 previously described this cost as
"weeks, not days," which understates the reported ~2-person-month, 3-person
figure; the precise figure is used here instead.
