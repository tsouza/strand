# CIFF — Common Index File Format, protobuf schema and framing

Vendored source, fetched live 2026-08-19/20 from the canonical repository
`github.com/osirrc/ciff` (OSIRRC community), Apache-2.0 licensed (repository
root `LICENSE`). Two files reproduced in full below: the wire schema itself
(`src/main/protobuf/CommonIndexFileFormat.proto`) and the top-level framing
description from `README.md`. Fetched via `gh api
repos/osirrc/ciff/contents/...` against the repository's default branch
(`master`, confirmed via `gh api repos/osirrc/ciff` — not `main`, which
404s), not recalled from memory (`CLAUDE.md` §3). Cited by
`crates/strand-tools/src/ciff.rs` (STRAND's CIFF importer, roadmap item
M4-2).

## Framing (README.md)

> All data are contained in a single file, with the extension `.ciff`. The
> file comprises a sequence of delimited protobuf messages defined
> [here](src/main/protobuf/CommonIndexFileFormat.proto), exactly as
> follows:
>
> + A `Header`
> + Exactly the number of `PostingsList` messages specified in the
>   `num_postings_lists` field of the `Header`
> + Exactly the number of `DocRecord` messages specified in the `num_docs`
>   field of the `Header`

`src/main/java/io/osirrc/ciff/ReadCIFF.java`, the project's own reference
reader, confirms this order in code: it reads one `Header`, then loops
`header.getNumPostingsLists()` times reading `PostingsList`, then loops
`header.getNumDocs()` times reading `DocRecord`, each via
`parseDelimitedFrom(fileIn)`.

"Delimited" is protobuf's own `writeDelimitedTo()` convention (Google's own
technique, linked from the README): every message is prefixed with its own
byte length encoded as a protobuf varint (unsigned LEB128 — each byte's low
7 bits are a value group, high bit set means "more bytes follow"), with no
other separator. There is no outer wrapper message; a conforming reader
reads a varint, then reads exactly that many bytes and parses them as the
next message in sequence.

## Schema (`src/main/protobuf/CommonIndexFileFormat.proto`, verbatim)

```protobuf
syntax = "proto3";

package io.osirrc.ciff;

// An index stored in CIFF is a single file comprised of exactly the following:
//  - A Header protobuf message,
//  - Exactly the number of PostingsList messages specified in the num_postings_lists field of the Header
//  - Exactly the number of DocRecord messages specified in the num_doc_records field of the Header
// Each message is written using message.writeDelimitedTo(), which prefixes each message with its varint encoded size.
// The protobuf messages are defined below.

// This is the CIFF header. It always comes first.
message Header {
  int32 version = 1;              // Version.

  int32 num_postings_lists = 2;   // Exactly the number of PostingsList messages that follow the Header.
  int32 num_docs = 3;             // Exactly the number of DocRecord messages that follow the PostingsList messages.

  // The total number of postings lists in the collection; the vocabulary size. This might differ from
  // num_postings_lists, for example, because we only export the postings lists of query terms.
  int32 total_postings_lists = 4;

  // The total number of documents in the collection; might differ from num_doc_records for a similar reason as above.
  int32 total_docs = 5;

  // The total number of terms in the entire collection. This is the sum of all document lengths of all documents in
  // the collection.
  int64 total_terms_in_collection = 6;

  // The average document length. We store this value explicitly in case the exporting application wants a particular
  // level of precision.
  double average_doclength = 7;

  // Description of this index, meant for human consumption. Describing, for example, the exporting application,
  // document processing and tokenization pipeline, etc.
  string description = 8;
}

// An individual posting.
message Posting {
  int32 docid = 1; //the *delta-gap* compressed docid
  int32 tf = 2;
}

// A postings list, comprised of one ore more postings.
message PostingsList {
  string term = 1;   // The term.
  int64 df = 2;      // The document frequency.
  int64 cf = 3;      // The collection frequency.
  repeated Posting postings = 4;
}

// A record containing metadata about an individual document.
message DocRecord {
  int32 docid = 1;               // Refers to the docid in the postings lists.
  string collection_docid = 2;   // Refers to a docid in the external collection.
  int32 doclength = 3;           // Length of this document.
}
```

Note the header comment's own inconsistency, reproduced verbatim above and
worth flagging rather than silently correcting: the file-level comment says
"the num_doc_records field", but the actual field name in the `Header`
message is `num_docs` (there is no field literally named
`num_doc_records`). `ReadCIFF.java` uses `header.getNumDocs()` throughout,
confirming `num_docs` is the real field; this vendored copy keeps the
comment's wording exactly as written upstream rather than fixing it, since
`CLAUDE.md` §2 forbids inventing text a source didn't say — but importer
code and this crate's own documentation are grounded against the message
field name, `num_docs`, not the drifted comment.

## Delta-gap docid encoding, confirmed against a real encoder

`Posting.docid` is delta-gap compressed ("the *delta-gap* compressed
docid" per the schema's own inline comment) but the `.proto` file does not
itself state the accumulator's starting point. Grounded instead against a
second real implementation, the Rust crate `pisa-engine/ciff` (Apache-2.0,
`Cargo.toml`), specifically its encoder in `src/lib.rs`:

```rust
let mut last_doc = 0;
for (docid, tf) in posting_pairs {
    let mut posting = Posting::new();
    posting.set_docid(docid - last_doc);
    posting.set_tf(tf);
    postings_list.postings.push(posting);
    last_doc = docid;
}
```

and its decoder in the same file:

```rust
postings.iter().scan(0_u32, |prev, p| {
    *prev += u32::try_from(p.get_docid()).expect("Negative ID");
    Some(*prev)
})
```

Both confirm the accumulator starts at `0`: the first posting's `docid`
field is a delta from `0` (so it equals the absolute docid when the first
matching document is docid `0`), and every subsequent posting's `docid`
field is `this_docid - previous_docid`. Decoding is therefore a running
sum seeded at `0`, consumed in file order, requiring each term's postings
to already be sorted ascending by absolute docid (a delta-gap scheme is
only compact, and only unambiguous to decode this way, for a
non-decreasing sequence).

## Real test fixture: `pisa-engine/ciff`'s toy CIFF export

`pisa-engine/ciff`'s `tests/test_data/toy-complete-20200309.ciff` (337
bytes) is a real, complete CIFF export — `num_docs == total_docs == 3`,
`num_postings_lists == total_postings_lists == 9` — described by its own
embedded `Header.description` as "Export of toy 3-document collection from
Anserini's io.anserini.integration.TrecEndToEndTest test case". Fetched via
`gh api repos/pisa-engine/ciff/contents/tests/test_data/
toy-complete-20200309.ciff` and vendored at
`conformance/ciff/toy-complete-20200309.ciff` for
`crates/strand-tools/src/ciff.rs`'s own tests — a real fixture built by a
real, independent CIFF exporter, not a hand-rolled one, per `CLAUDE.md`
§3's "start from usage, not structure" applied to test fixtures.
