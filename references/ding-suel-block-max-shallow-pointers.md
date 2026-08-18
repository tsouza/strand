# Ding & Suel — Block-Max WAND's "Shallow" vs "Deep" Pointer Movement

Vendored excerpt, not the full paper. Source: S. Ding, T. Suel, "Faster
Top-k Document Retrieval Using Block-Max Indexes," SIGIR 2011, pp.
993–1002 (fetched 2026-08-18 from a third-party-hosted PDF mirror,
`https://user.ceng.metu.edu.tr/~isikligil/ceng334/HW3/outputs/recover1/bmw.pdf`,
after the paper's own institutional host, `cs.nyu.edu`, returned 403
Forbidden — this is the same paper, ACM SIGIR 2011, ISBN
978-1-4503-0757-4; the mirror is used only because the canonical host is
unreachable, not as an independent source). Short quotation for citation
and technical commentary. Cited to pin the exact terminology for this
project's invariant 4 (block-max bounds) and to keep block-skipping
terminologically distinct from "processing compressed data directly" (see
`references/roaring-bitmaps-container-operations.md` for the latter,
genuinely distinct technique) — this project's own compressed-data-
processing investigation found these two ideas get conflated if not
pinned against a primary source.

---

### The core distinction, in the paper's own words

> "In the traditional DAAT query processing, one core function is called
> Next(d,list(i)) or NextGEQ(d, list(i))... this function receives a docID
> d and an inverted list list(i) as inputs and returns the first docID
> after the current docID in list(i) that is equal to or greater than d.
> The call to this particular function usually involves a decompression
> of one block in list(i). We call this a deep pointer movement due to
> the reason that it usually involves a block decompression."

> "As we have the max score for each block, we design another function
> called NextShallow(d,list(i)) which only moves the current pointer to
> the corresponding block without decompression (using d and information
> about the block boundaries in the table). We call this a shallow
> pointer movement. We use two main ideas in our modified algorithm: (i)
> we use the global maximum scores to determine a candidate pivot, as in
> WAND, but then use the block maximum scores to check if the candidate
> pivot is a real pivot, and (ii) we use shallow instead of deep pointer
> movements whenever possible."

### What this technique actually is — and is not

This is early termination / safe top-k pruning: deciding which blocks
never need to be touched at all, using a precomputed per-block bound (the
paper's own Block-Max Index, storing "the maximum impact score" per
block — matching this project's invariant 4 principle of block-max bounds
as raw, scoring-independent statistics, though the paper's own bound is
an impact score, not the raw per-block max-tf/min-doclen fields invariant
4 pins). It is not a technique for computing an operation (AND, scoring,
intersection) on postings while they remain compressed — a shallow
pointer movement's whole point is that the block's compressed bytes are
never touched in the first place. That is a different question from
whether an already-selected, to-be-processed block can be scored or
intersected without full decompression — a question the SIMD-postings
literature (`references/lemire-boytsov-simd-bp128.md`) answers negatively
for BP128/FastPFOR-style codecs.
