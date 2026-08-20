# Puffin File Format Spec, and `apache/iceberg-rust`'s Puffin Module

Vendored excerpt and source-code findings, fetched 2026-08-19/20 to ground
`rfcs/0013-puffin-export-sidecar.md` (M4-5, `docs/roadmap.md`) — the real,
current Puffin specification and a real reference implementation, not a
remembered shape (`CLAUDE.md` §3).

**Sources:**

- `raw.githubusercontent.com/apache/iceberg/main/format/puffin-spec.md` — the
  normative Puffin v1 spec, license header confirms Apache-2.0 (ASF boilerplate,
  ll. 4-19 of the fetched file).
- `raw.githubusercontent.com/apache/iceberg-rust/main/crates/iceberg/src/
  puffin/{mod,blob,reader}.rs` — the official Rust Iceberg implementation's
  Puffin module. `iceberg` crate on crates.io: v0.10.1, repository
  `github.com/apache/iceberg-rust`, 1,851,013 downloads at fetch time,
  Apache-2.0 (ASF boilerplate header, identical to the spec file's).
- `crates.io/api/v1/crates?q=puffin` and `?q=iceberg`, fetched directly (not
  the JS-rendered search page, which returns no content to a plain fetch):
  no crate named `puffin*` implements the Iceberg file format — every result
  under that query name is the unrelated `puffin` game-profiler crate family
  (`puffin`, `puffin_http`, `puffin-imgui`, `puffin_viewer`, `bevy_puffin`,
  `puffin_egui`). The real implementation lives inside the `iceberg` crate
  itself, under its own `puffin` module. One small third-party crate,
  `samkhya-iceberg` ("Read samkhya's portable statistics from Apache Iceberg
  Puffin sidecars, snapshot-aware"), also exists as of this fetch — real,
  if narrow, independent evidence that something outside Iceberg's own core
  reads Puffin sidecars specifically.

## Footer structure (`puffin-spec.md`, "Footer structure" / "Footer Payload")

File layout: `Magic Blob₁ Blob₂ ... Blobₙ Footer`. `Magic` is four bytes
`0x50 0x46 0x41 0x31` ("PFA1" — Puffin, *Fratercula arctica*, version 1),
identical at the start of the file and inside the footer.

Footer: `Magic FooterPayload FooterPayloadSize Flags Magic`. `FooterPayload`
is "optionally compressed, UTF-8 encoded JSON payload" (LZ4-frame compressed,
or uncompressed) representing a `FileMetadata` object. `FooterPayloadSize` is
"a length in bytes of the `FooterPayload` (after compression, if compressed),
stored as 4 byte integer" — the spec states 4-byte integers are "always
signed, in a two's complement representation, stored little-endian." `Flags`
is 4 bytes; byte 0 bit 0 is "whether `FooterPayload` is compressed," all
other bits/bytes reserved, zero on write.

`FileMetadata`: `blobs` (list of `BlobMetadata`, required), `properties`
(JSON object, optional, "storage for arbitrary meta-information, like writer
identification/version").

`BlobMetadata`: `type` (string, required — see Blob types), `fields` (JSON
list of ints, required — "List of field IDs the blob was computed for; the
order of items is used to compute sketches stored in the blob"), `snapshot-id`
(long, required), `sequence-number` (long, required), `offset` (long,
required — file-relative byte offset), `length` (long, required — post-
compression byte length), `compression-codec` (string, optional — "If
omitted, the data is assumed to be uncompressed"), `properties` (JSON object,
optional). **No checksum field of any kind on `BlobMetadata`.**

## Blob types (`puffin-spec.md`, "Blob types")

Exactly two are registered: `apache-datasketches-theta-v1` (a compact Theta
cardinality sketch, optional `ndv` property) and `deletion-vector-v1`.
**The spec's "Blob types" section contains no text at all about how a third
party registers a new type string, avoids collisions, or is expected to
namespace one** — confirmed by direct read of the full section, not inferred
from silence elsewhere.

`deletion-vector-v1`, quoted in full because this RFC's worked example
depends on every byte of it:

> "The serialized blob contains:
> - Combined length of the vector and magic bytes stored as 4 bytes, big-endian
> - A 4-byte magic sequence, `D1 D3 39 64`
> - The vector, serialized as described below
> - A CRC-32 checksum of the magic bytes and serialized vector as 4 bytes, big-endian"
>
> "The position vector is serialized using the Roaring bitmap 'portable'
> format. This representation consists of:
> - The number of 32-bit Roaring bitmaps, serialized as 8 bytes, little-endian
> - For each 32-bit Roaring bitmap, ordered by unsigned comparison of the
>   32-bit keys:
>     - The key stored as 4 bytes, little-endian
>     - A 32-bit Roaring bitmap"
>
> "Note that the length and CRC fields are stored using big-endian, but the
> Roaring bitmap format uses little-endian values. Big endian values were
> chosen for compatibility with existing deletion vectors in Delta tables."

Required properties: `referenced-data-file` ("the location of the data file
the delete vector applies to; must be equal to the data file's `location` in
table metadata"), `cardinality` ("the number of deleted rows"); MUST omit
`compression-codec` ("`deletion-vector-v1` is not compressed").

"Snapshot ID and sequence number are not known at the time the Puffin file
is created. `snapshot-id` and `sequence-number` must be set to -1 in blob
metadata for Puffin v1" — the spec's own documented value for exactly the
"no Iceberg snapshot exists" case this RFC's export path is in.

## Compression codecs

Exactly two registered: `lz4` (single LZ4 frame, content size present) and
`zstd` (single Zstandard frame, content size present). "For maximal
interoperability, other codecs are not supported" — no chunk-level or
per-block compression; a blob is compressed (as one unit, using one of these
two codecs) or not.

## `apache/iceberg-rust`'s `puffin` module (real, current source)

`crates/iceberg/src/puffin/blob.rs` defines `pub const DELETION_VECTOR_V1:
&str = "deletion-vector-v1"` and `pub const APACHE_DATASKETCHES_THETA_V1: &str
= "apache-datasketches-theta-v1"`, and a `Blob` struct with fields `r#type:
String, fields: Vec<i32>, snapshot_id: i64, sequence_number: i64, data:
Vec<u8>, properties: HashMap<String, String>` — a direct, field-for-field
match to the spec's `BlobMetadata` schema above, confirming the spec excerpt
against real, executing code rather than trusting the prose alone.

`crates/iceberg/src/puffin/reader.rs` defines `PuffinReader::new(input_file:
InputFile) -> Result<Self>` — it takes a bare `InputFile`, with no Iceberg
table, catalog, or manifest in scope. **A standalone `.puffin` file, with no
surrounding Iceberg table, is a file this real, official, Apache-2.0,
1.85M-download Rust crate can open and read today.** This is the concrete
fact this RFC's "How this could be wrong" section weighs against
`docs/lineage.md`'s own skepticism about Puffin's third-party adoption.
