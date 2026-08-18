# Roaring Bitmaps — Set Operations Run Directly on Compressed Containers

Vendored excerpt, not the full paper. Source: S. Chambi, D. Lemire,
O. Kaser, R. Godin, "Better bitmap performance with Roaring bitmaps,"
*Software: Practice and Experience* 46(5), 2016 (arXiv:1402.6407v9,
fetched 2026-08-18 from `https://arxiv.org/pdf/1402.6407`). Short
quotation for citation and technical commentary, not a claim of a
specific open license on the full paper. This project's invariant 2
already commits to Roaring for deletion vectors; no primary source for
that choice had been vendored before now. Cited by the compressed-data-
processing investigation this project ran: it confirms Roaring's set
operations genuinely execute on the compressed container representation
itself, with no decompression step — a real, mechanically-verified case
of "process compressed data directly," contrasted in that same
investigation with BP128/FastPFOR-style postings, where no such technique
exists in the literature (see `references/lemire-boytsov-simd-bp128.md`
and the ledger entry this reference supports).

---

### Headline result

> "On synthetic and real data, we find that Roaring bitmaps (1) often
> compress significantly better (e.g., 2×) and (2) are faster than the
> compressed alternatives (up to 900× faster for intersections). Our
> results challenge the view that RLE-based bitmap compression is best."

### The mechanism: no decompression step, for any container-pair case

> "We implemented various operations on Roaring bitmaps, including union
> (bitwise OR) and intersection (bitwise AND). A bitwise operation between
> two Roaring bitmaps consists of iterating and comparing the 16 high-bits
> integers (keys) on the first-level indexes... On equality, a
> second-level logical operation between the corresponding containers is
> performed."

**Bitmap vs. bitmap** — the container already *is* its own operable dense
word array; there is nothing to decompress to:

> "We iterate over 1024 64-bit words. For unions, we perform 1024 bitwise
> ORs and write the result to a new bitmap container."

**Array vs. array** — operates on the two sorted arrays directly, never
expanded to a bitmap:

> "For intersections, we use a simple merge (akin to what is done in
> merge sort) when the two arrays have cardinalities that differ by less
> than a factor of 64. Otherwise, we use galloping intersections."

**Array vs. bitmap** — direct membership test against the bitmap's raw
words, no conversion of either side:

> "When one of the two containers is a bitmap and the other one is a
> sorted dynamic array, the intersection can be computed very quickly: we
> iterate over the sorted dynamic array, and verify the existence of each
> 16-bit integer in the bitmap container."

### Why this beats RLE-based formats (WAH, Concise), in the authors' own words

> "On the Java platform we used for our experiments, we estimate that we
> can compute and write bitwise ORs at 700 million 64-bit words per
> second. If we further compute the cardinality of the result as we
> produce it, our estimated speed falls to about 500 million words per
> second... In contrast, competing methods like WAH and Concise must
> spend time to decode the word type before performing a single bitwise
> operation. These checks may cause expensive branch mispredictions or
> impair superscalar execution."

The advantage is specifically attributed to avoiding a per-word
type-decode branch that RLE-style formats require before any bitwise op
— not to a faster bitwise instruction, which is identical either way.
