# spacejam/tla-rust

Vendored summary, not a full reproduction (the repository's own README and TLA+
files are the primary source; it is MIT licensed, confirmed via the GitHub API).
Source: `github.com/spacejam/tla-rust`. Fetched 2026-08-18. Cited by
`rfcs/0002-manifest-formal-verification.md` §5/"How this could be wrong" as the
nearest prior-art precedent for a TLA+-model-plus-Rust-implementation
cross-validation architecture, and as this project's own named risk (an effort of
this shape that stalled).

---

The README states the project's goal as an open-source distributed store "that
takes correctness seriously at the local storage, sharding, and distributed
transactional layers," combining three techniques: modeling core algorithms in
TLA+ and checking them with TLC; implementing the system in Rust; and testing the
implementation under simulated failure conditions using `quickcheck` and
abstracted RPC/clocks.

**What the repository actually contains**, confirmed by listing its full file
tree (`gh api repos/spacejam/tla-rust/git/trees/master?recursive=1`): TLA+/PlusCal
source files (`atomic_add.tla`, `pcal_intro.tla`), a directory of vendored TLA+
example specs including `raft.tla` and the Paxos examples from Lamport's
"Specifying Systems," a `Makefile`, and a `README.md`. There is no Rust source
file (`.rs`) anywhere in the repository. The README references two *external*
projects, `rsdb` and `rasputin`, as where the Rust implementation would live, but
neither is part of this repository, and no evidence of either was located as part
of this citation check.

**Correction to an earlier draft.** RFC 0002 previously described this project as
having "attempted... this architecture" or "attempted close to this
architecture." That overstates it: only the TLA+ modeling half was ever
committed to this repository; no cross-validation implementation (Rust or
otherwise) was ever begun here. "Attempted the spec half only; no implementation
was ever begun" is the accurate characterization, used in this RFC's revised
text.
