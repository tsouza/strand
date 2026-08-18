# R6 — The second engine (CIFF, Quickwit/Datadog, ParadeDB, Lance table format)

Vendored excerpts. Fetched 2026-08-18.

Cited by: `docs/research/README.md` R6, `docs/lineage.md` ("From CIFF"), `CLAUDE.md`
§1 (M5's rationale).

## CIFF (Lin et al., SIGIR 2020)

**Source:** `arxiv.org/pdf/2003.08276` (the abstract page,
`arxiv.org/abs/2003.08276`, was fetched first but the load-bearing sentence lives in
the paper body; read directly as a PDF).

**Title:** Supporting Interoperability Between Open-Source Search Engines with the
Common Index File Format
**Authors:** Jimmy Lin, Joel Mackenzie, Chris Kamphuis, Craig Macdonald, Antonio
Mallia, Michał Siedlaczek, Andrew Trotman, Arjen de Vries

The exact sentence `docs/research/README.md` R6 quotes as "speed... not important
concerns," confirmed verbatim in context:

> "we intend for this to be an *exchange* format and not an *operational* one — that
> is, we expect each system to read Ciff and transform the contents into the
> system's own internal representation. [...] Speed of reading/writing this format as
> well as compactness are *not* important concerns, since the format is not meant to
> be computed over; thus, we deliberately eschew exotic compression schemes that may
> result in smaller output sizes at the cost of decoding complexity."

This is the paper's own explicit design non-goal, stated by its authors — directly
grounding invariant 8's contrast ("Every gap is a MUST here") between CIFF's
exchange-only design and STRAND's operational one.

## Datadog acquires Quickwit (relicense to Apache-2.0)

**Source:** `datadoghq.com/blog/datadog-acquires-quickwit/`.

Acquisition date: **January 9, 2025**.

> "To ensure continued support for the open source community, Quickwit will be
> releasing a major update under the Apache License 2."

Confirms the AGPL-to-Apache-2.0 relicense `docs/lineage.md` and R6 cite.

## ParadeDB / pg_search

**Source:** `github.com/paradedb/paradedb`.

> "One Postgres for your application data, full-text search, vector retrieval, and
> aggregations." ... "Tantivy — powers full-text search" ... "Your application data
> and your search engine live in one database, with no second system to deploy and
> nothing to sync."

Confirms ParadeDB embeds tantivy inside Postgres via `pg_search`, as R6 states.

## Lance table format

**Source:** `lance.org/format/table/`.

> "Indices are part of the table format lifecycle. The table metadata describes
> index discovery and transactional coordination, while the detailed search
> structures remain separate index formats."

The manifest references index metadata through an `index_section` pointer rather
than embedding index internals — confirms the index-aware, index-internals-agnostic
manifest pattern `docs/lineage.md` and RFC 0001 attribute to Lance, from the table-
format page specifically (the vector-index-format page was already vendored
separately, `references/lance-vector-index-format.md`).
