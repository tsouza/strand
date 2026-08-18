# R4 — Analyzer conformance sources (UAX #29, PRI #308, Hiemstra et al. 2023)

Vendored excerpts. Fetched 2026-08-18.

Cited by: `docs/research/README.md` R4, `CLAUDE.md` invariant 6 (normative
conformance vectors).

## UAX #29 (Unicode Text Segmentation)

**Source:** `unicode.org/reports/tr29/`.

Scope: default segmentation boundaries for grapheme clusters, words, and sentences.
(Line-boundary determination is a separate annex, UAX #14.)

The document states: "This is a stable document and may be used as reference
material or cited as a normative reference by other specifications" — but this
stability claim is about the *document's process status*, not immunity from
cross-version change: the same page notes "additional changes to the rules are made
when new information becomes available" for complex scripts, and the annex is
versioned alongside each Unicode release (version 47 accompanies Unicode 17.0.0 as of
this fetch). This is the basis for invariant 6's claim that UAX #29 is "explicitly
unstable across Unicode versions" — the instability is in the rule content across
versions, not a claim that the specification process itself is unstable.

## PRI #308 (U+202F NARROW NO-BREAK SPACE background)

**Source:** `unicode.org/L2/L2015/15295-pri308-bkgnd.html`.

Confirms the exact change invariant 6 cites: reclassifying U+202F's Word_Break
property.

> "This change in word segmentation can be accomplished in more than one way ...
> result in the desired word segmentation behavior by changing U+202F from its
> current value (WB=XX) to (WB=EX)."

Two implementation routes were considered — directly adding U+202F to
ExtendNumLet, or changing its General_Category from Space_Separator to
Connector_Punctuation (deriving the same Word_Break value) — with the stated goal of
fixing Mongolian word segmentation without disturbing French typography's use of the
character: "would not impact *line* breaking behavior — this change is only intended
to modify default *word* segmentation behavior."

## Hiemstra, Hendriksen, Kamphuis, de Vries — "Challenges of Index Exchange for
Search Engine Interoperability" (2023)

**Source:** `djoerdhiemstra.com/wp-content/uploads/ossym2023.pdf`. Fetched and read
directly as a PDF (not summarized by the fetch tool, which could not decode it;
read via the PDF reader instead).

**Authors:** Djoerd Hiemstra, Gijs Hendriksen, Chris Kamphuis, Arjen P. de Vries
(Radboud University)

The paper studies CIFF (the Common Index File Format) specifically, and states the
exact problem invariant 6 cites:

> "Consistent tokenization between index and queries remains an unsolved problem of
> index exchange, at least outside the narrow scope of information retrieval
> experiments that use benchmark test collections with a small set of test queries."

Their experiment (Terrier and GeeseDB indexes cross-imported via CIFF, TREC Robust04)
shows real, measured retrieval-quality degradation from tokenizer mismatch — e.g.
Table 1b: MAP drops from 0.234 (matched tokenizer) to 0.081 (GeeseDB using its
default NLTK tokenizer against a Terrier-built CIFF index, on the subset of topics
containing a hyphen or period) — and that a shared generic tokenizer recovers the
matched-tokenizer performance (0.234). This is the direct empirical evidence behind
invariant 6's claim that undeclared analysis renders an index non-portable, and the
reason this project makes analyzer descriptors and conformance vectors normative
rather than advisory.

## Not vendored in this pass

`unicode-rs/unicode-segmentation` (a Rust crate, not a text source with quotable
claims) and the Lucene `Version` / ICU-CLDR versioning docs (lower-priority,
implementation-reference material rather than load-bearing citations) were not
fetched in this pass; they remain owed if a future session needs them for the M1
analyzer-descriptor RFC specifically.
