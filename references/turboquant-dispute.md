# TurboQuant / RaBitQ dispute (ICLR 2026)

Vendored excerpts. Fetched 2026-08-18.

Cited by: `docs/research/README.md` R3 ("The TurboQuant dispute (ICLR 2026) is
procedural...").

## TurboQuant paper

**Source:** `arxiv.org/abs/2504.19874`.
**Title:** TurboQuant: Online Vector Quantization with Near-optimal Distortion Rate
**Authors:** Amir Zandieh, Majid Daliri, Majid Hadian, Vahab Mirrokni

> "Vector quantization, a problem rooted in Shannon's source coding theory, aims to
> quantize high-dimensional Euclidean vectors while minimizing distortion in their
> geometric structure."

The paper reports KV-cache compression results: "absolute quality neutrality with 3.5
bits per channel and marginal quality degradation with 2.5 bits per channel."

## The dispute account

**Source:** Milvus blog, "Interview with RaBitQ Authors: The TurboQuant Dispute and
Why the Storage Selloff Was a False Alarm,"
`milvus.io/blog/interview-with-rabitq-authors-the-turboquant-dispute-and-why-the-storage-selloff-was-a-false-alarm.md`.

On ICLR's response:

> "ICLR did not take action. We emailed them during the review period in September
> last year, but did not receive a response."

On Google's response:

> One TurboQuant co-author "acknowledged concerns and indicated they would revise the
> arXiv version to correct its inaccurate description of RaBitQ's optimality."

On comparative performance:

> "When evaluated under a standardized CPU environment, TurboQuant did not outperform
> our internal RaBitQ version in most of the cases we evaluated."

> TurboQuant's real impact "lies not in embedding compression but in potential
> KV-cache applications for language models, a different use case entirely from
> vector database retrieval."

## What this confirms

This fully verifies `docs/research/README.md` R3's summary as stated: ICLR took no
action on the dispute, a Google co-author agreed to correct the arXiv text, and
TurboQuant's published wins are specifically on KV-cache compression, not the
embedding-retrieval workload STRAND's quantization choice (RaBitQ/Extended-RaBitQ)
targets. No correction to R3's text is needed.

A companion Medium post by RaBitQ's first author (Jianyang Gao,
`medium.com/@gaojianyang0017/turboquant-and-rabitq-what-the-public-story-gets-wrong-23df83209c22`)
returned HTTP 403 to an unauthenticated fetch and was not independently vendored; the
Milvus interview above covers the same material with direct quotes from the same
authors.
