// Copyright the STRAND authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! `strand-tools convert-ciff`: imports a real CIFF (Common Index File
//! Format) export into a real STRAND segment file — this repository's
//! namesake prior art (`CLAUDE.md`'s mission sentence: "CIFF you can query
//! in place on S3, extended to vectors") and the other half of the
//! "tantivy/CIFF importers" pair `CLAUDE.md`'s repository shape names for
//! `strand-tools convert` (roadmap item M4-2).
//!
//! CIFF's wire format is a Protocol Buffers schema, fetched live from its
//! canonical repository — `github.com/osirrc/ciff`'s
//! `src/main/protobuf/CommonIndexFileFormat.proto` — and vendored verbatim
//! at `references/ciff-common-index-file-format.md`, not implemented from
//! memory (`CLAUDE.md` §3). The delta-gap docid accumulator's starting
//! point, which the `.proto` file itself does not state, is grounded
//! against a second real implementation, `pisa-engine/ciff`'s real
//! encoder/decoder in Rust (same reference file). This module hand-writes
//! the four message shapes with `#[derive(prost::Message)]` rather than
//! generating them from the `.proto` at build time: the field shapes are
//! small, stable, and now pinned by test fixtures, and skipping
//! `prost-build` means this crate carries no build-time dependency on a
//! system `protoc` binary.
//!
//! Framing (also vendored in the reference file above, from CIFF's own
//! `README.md` and confirmed against its reference reader,
//! `ReadCIFF.java`): a `.ciff` file is one `Header` message, followed by
//! exactly `Header.num_postings_lists` `PostingsList` messages, followed
//! by exactly `Header.num_docs` `DocRecord` messages — each message
//! "delimited" (Google's own `writeDelimitedTo()` convention: a protobuf
//! varint byte-length prefix, then exactly that many message bytes, with
//! no other separator).
//!
//! **Honest scope: what CIFF cannot provide losslessly.** Two real gaps,
//! not silently gap-filled:
//!
//! 1. **No positions.** `Posting` carries a term-frequency count (`tf`),
//!    never within-document token positions — `docs/lineage.md`'s own
//!    CIFF entry names this gap ("no positions"). This importer always
//!    calls `build_field_from_postings` with `with_positions = false`;
//!    the resulting field carries no positions blob and can never answer
//!    a phrase query. Internally, each posting is represented as a
//!    placeholder positions vector whose *length* equals CIFF's real `tf`
//!    value (`build_field_from_postings` derives term frequency from that
//!    length, matching every other real caller of that function) but
//!    whose *contents* are meaningless filler — safe only because
//!    `with_positions = false` guarantees those contents are never read
//!    into any output blob (`strand_lexical::field::build_field_from_postings`'s
//!    own `with_positions` branch skips `p` entirely when `false`).
//! 2. **No external document identity.** `DocRecord.collection_docid` is a
//!    real field (the source collection's own document ID string) that
//!    this importer reads and uses only to validate the input, then
//!    discards: no blob type in the lexical family's registry
//!    (`spec/container.md` §9) has a slot for an external-ID mapping, so
//!    there is nowhere lossless to put it yet. A future spec chapter
//!    could add one; until then, round-tripping a CIFF file through this
//!    importer and back out loses the ability to map a STRAND row-ID back
//!    to the original collection's document ID.
//!
//! Everything else CIFF carries maps losslessly onto invariant 5's
//! scoring inputs: per-term document frequency (`PostingsList.df`,
//! cross-checked against the real postings count), per-document length
//! (`DocRecord.doclength`), and the collection average/total (validated,
//! not separately stored, since `spec/scoring-profiles.md`'s inputs are
//! `dl` per document and the derived `avdl`, not a cached collection
//! total). `PostingsList.cf` (collection frequency, i.e. summed `tf`) is
//! validated against the real postings but likewise not stored
//! separately: it is always recomputable from the postings blob's own
//! per-document `tf` values, which the STRAND term-info record has no
//! field for either, matching this crate's tantivy importer's own
//! omission of `cf`.
//!
//! Deliberately narrow, stated scope, matching `convert.rs`'s own
//! precedent: only a **complete-document-catalog** CIFF export is
//! accepted (`Header.num_docs == Header.total_docs`), so every STRAND row
//! ordinal `0..num_docs` has a real `DocRecord` behind it and the row-ID
//! space is dense by construction (`spec/row-ids.md` §1). A
//! **partial-vocabulary** export (`Header.num_postings_lists <
//! Header.total_postings_lists` — CIFF's own header comment names this as
//! a real, intentional case: "we only export the postings lists of query
//! terms") is accepted; the resulting field simply cannot answer queries
//! for terms outside that subset, an honest limitation of the input file,
//! not of this importer.

use std::collections::BTreeMap;
use std::path::Path;

use strand_lexical::field::{FieldBlobs, build_field_from_postings};

/// CIFF's `Header` message (`references/ciff-common-index-file-format.md`).
/// Field tags and types are copied verbatim from the vendored `.proto`.
#[derive(Clone, PartialEq, prost::Message)]
struct Header {
    #[prost(int32, tag = "1")]
    version: i32,
    #[prost(int32, tag = "2")]
    num_postings_lists: i32,
    #[prost(int32, tag = "3")]
    num_docs: i32,
    #[prost(int32, tag = "4")]
    total_postings_lists: i32,
    #[prost(int32, tag = "5")]
    total_docs: i32,
    #[prost(int64, tag = "6")]
    total_terms_in_collection: i64,
    #[prost(double, tag = "7")]
    average_doclength: f64,
    #[prost(string, tag = "8")]
    description: String,
}

/// CIFF's `Posting` message: `docid` is delta-gap compressed (decoded by
/// `import_ciff`'s own running-sum accumulator below), `tf` is the real,
/// direct term frequency.
#[derive(Clone, PartialEq, prost::Message)]
struct Posting {
    #[prost(int32, tag = "1")]
    docid: i32,
    #[prost(int32, tag = "2")]
    tf: i32,
}

/// CIFF's `PostingsList` message.
#[derive(Clone, PartialEq, prost::Message)]
struct PostingsList {
    #[prost(string, tag = "1")]
    term: String,
    #[prost(int64, tag = "2")]
    df: i64,
    #[prost(int64, tag = "3")]
    cf: i64,
    #[prost(message, repeated, tag = "4")]
    postings: Vec<Posting>,
}

/// CIFF's `DocRecord` message.
#[derive(Clone, PartialEq, prost::Message)]
struct DocRecord {
    #[prost(int32, tag = "1")]
    docid: i32,
    #[prost(string, tag = "2")]
    collection_docid: String,
    #[prost(int32, tag = "3")]
    doclength: i32,
}

#[derive(Debug)]
pub enum CiffImportError {
    Io(std::io::Error),
    /// A length-delimited message's protobuf bytes failed to decode.
    Decode {
        what: &'static str,
        source: prost::DecodeError,
    },
    /// The file ended before a declared varint length prefix, or before
    /// the message bytes that length prefix promised.
    UnexpectedEof {
        expected: &'static str,
    },
    /// A varint length prefix used more than 10 bytes (the maximum for a
    /// 64-bit protobuf varint) without terminating.
    VarintTooLong,
    /// `Header.num_docs != Header.total_docs`: a partial document catalog
    /// (this importer's Non-goals, module doc comment).
    PartialDocExport {
        num_docs: i32,
        total_docs: i32,
    },
    /// A header count field (`num_docs`, `num_postings_lists`) was
    /// negative.
    NegativeHeaderCount {
        field: &'static str,
        value: i32,
    },
    /// A `PostingsList.df` disagreed with its own `postings` field's real
    /// length — the same consistency check CIFF's own reference reader
    /// (`ReadCIFF.java`) performs.
    PostingsCountMismatch {
        term: String,
        declared_df: i64,
        actual: usize,
    },
    /// The same term string appeared in two `PostingsList` messages.
    DuplicateTerm(String),
    /// A `Posting.docid` was negative, or its delta-decoded absolute
    /// docid did not strictly increase over the previous posting in the
    /// same list — `build_field_from_postings`'s own precondition, caught
    /// here as a `Result` rather than a downstream panic on untrusted
    /// external input.
    NonMonotonicOrNegativePosting {
        term: String,
    },
    /// A delta-decoded posting docid, or a `DocRecord.docid`, fell outside
    /// `0..Header.num_docs`.
    DocidOutOfRange {
        context: &'static str,
        docid: i64,
        num_docs: i32,
    },
    /// The same `docid` appeared in two `DocRecord` messages.
    DuplicateDocRecord {
        docid: i32,
    },
    /// A `DocRecord.doclength` was negative.
    NegativeDoclength {
        docid: i32,
        value: i32,
    },
    /// Fewer than `num_docs` distinct `DocRecord` messages were seen for
    /// some docid in range — an incomplete, non-dense catalog.
    MissingDocRecord {
        docid: i32,
    },
    /// `Header.total_terms_in_collection` did not match the real sum of
    /// `DocRecord.doclength` — CIFF's own integrity check
    /// (`ReadCIFF.java`), enforced here as a hard error since this
    /// importer already requires a complete document catalog.
    DoclengthSumMismatch {
        header_value: i64,
        computed: i64,
    },
}

impl std::fmt::Display for CiffImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CiffImportError::Io(e) => write!(f, "I/O error reading CIFF file: {e}"),
            CiffImportError::Decode { what, source } => {
                write!(f, "failed to decode CIFF {what} message: {source}")
            }
            CiffImportError::UnexpectedEof { expected } => {
                write!(f, "unexpected end of file while reading {expected}")
            }
            CiffImportError::VarintTooLong => {
                write!(f, "length-prefix varint exceeded 64 bits")
            }
            CiffImportError::PartialDocExport {
                num_docs,
                total_docs,
            } => write!(
                f,
                "CIFF file's document catalog is partial (num_docs={num_docs} != \
                 total_docs={total_docs}); this importer only accepts a complete \
                 document catalog"
            ),
            CiffImportError::NegativeHeaderCount { field, value } => {
                write!(f, "CIFF Header.{field} is negative: {value}")
            }
            CiffImportError::PostingsCountMismatch {
                term,
                declared_df,
                actual,
            } => write!(
                f,
                "term {term:?}: declared df={declared_df} but postings list has {actual} entries"
            ),
            CiffImportError::DuplicateTerm(term) => {
                write!(f, "term {term:?} appears in more than one PostingsList")
            }
            CiffImportError::NonMonotonicOrNegativePosting { term } => write!(
                f,
                "term {term:?}: a posting's docid is negative or not strictly \
                 increasing after delta-decoding"
            ),
            CiffImportError::DocidOutOfRange {
                context,
                docid,
                num_docs,
            } => write!(
                f,
                "{context}: docid {docid} is outside the valid range 0..{num_docs}"
            ),
            CiffImportError::DuplicateDocRecord { docid } => {
                write!(f, "docid {docid} appears in more than one DocRecord")
            }
            CiffImportError::NegativeDoclength { docid, value } => write!(
                f,
                "DocRecord for docid {docid} has a negative doclength: {value}"
            ),
            CiffImportError::MissingDocRecord { docid } => {
                write!(f, "no DocRecord was seen for docid {docid}")
            }
            CiffImportError::DoclengthSumMismatch {
                header_value,
                computed,
            } => write!(
                f,
                "Header.total_terms_in_collection={header_value} but the real sum of \
                 DocRecord.doclength is {computed}"
            ),
        }
    }
}

impl std::error::Error for CiffImportError {}

impl From<std::io::Error> for CiffImportError {
    fn from(e: std::io::Error) -> Self {
        CiffImportError::Io(e)
    }
}

/// Reads one protobuf-style unsigned LEB128 varint from the front of
/// `buf`, advancing `buf` past the bytes consumed — the length prefix
/// `writeDelimitedTo()` writes ahead of every CIFF message
/// (`references/ciff-common-index-file-format.md`).
fn read_varint_length(buf: &mut &[u8]) -> Result<usize, CiffImportError> {
    let mut result: u64 = 0;
    let mut shift: u32 = 0;
    loop {
        let &byte = buf.first().ok_or(CiffImportError::UnexpectedEof {
            expected: "a varint length prefix",
        })?;
        *buf = &buf[1..];
        result |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(result as usize);
        }
        shift += 7;
        if shift >= 64 {
            return Err(CiffImportError::VarintTooLong);
        }
    }
}

/// Reads one "delimited" protobuf message (varint length prefix, then
/// exactly that many message bytes) from the front of `buf`, advancing
/// `buf` past both.
fn read_delimited<T: prost::Message + Default>(
    buf: &mut &[u8],
    what: &'static str,
) -> Result<T, CiffImportError> {
    let len = read_varint_length(buf)?;
    if buf.len() < len {
        return Err(CiffImportError::UnexpectedEof { expected: what });
    }
    let (message_bytes, rest) = buf.split_at(len);
    let message =
        T::decode(message_bytes).map_err(|source| CiffImportError::Decode { what, source })?;
    *buf = rest;
    Ok(message)
}

/// Imports a real CIFF file at `ciff_path` into a `FieldBlobs` named
/// `field_name` (CIFF has no field concept of its own — the whole file is
/// one flat term/postings space — so the caller names the single STRAND
/// field this content becomes), and returns it alongside the row count
/// (`Header.num_docs` as `u64`, the row-ID count a segment built from
/// these blobs must use, `spec/row-ids.md` §1).
///
/// See this module's own doc comment for the precise, honest boundary of
/// what CIFF can and cannot provide losslessly.
pub fn import_ciff(
    ciff_path: &Path,
    field_name: &str,
) -> Result<(FieldBlobs, u64), CiffImportError> {
    let bytes = std::fs::read(ciff_path)?;
    let mut cursor: &[u8] = &bytes;

    let header: Header = read_delimited(&mut cursor, "Header")?;
    if header.num_docs < 0 {
        return Err(CiffImportError::NegativeHeaderCount {
            field: "num_docs",
            value: header.num_docs,
        });
    }
    if header.num_postings_lists < 0 {
        return Err(CiffImportError::NegativeHeaderCount {
            field: "num_postings_lists",
            value: header.num_postings_lists,
        });
    }
    if header.num_docs != header.total_docs {
        return Err(CiffImportError::PartialDocExport {
            num_docs: header.num_docs,
            total_docs: header.total_docs,
        });
    }
    let num_docs = header.num_docs as usize;
    let full_vocabulary = header.num_postings_lists == header.total_postings_lists;

    let mut per_term: BTreeMap<Vec<u8>, Vec<(u32, Vec<u32>)>> = BTreeMap::new();
    let mut sum_tf: i64 = 0;

    for _ in 0..header.num_postings_lists {
        let posting_list: PostingsList = read_delimited(&mut cursor, "PostingsList")?;
        if posting_list.postings.len() as i64 != posting_list.df {
            return Err(CiffImportError::PostingsCountMismatch {
                term: posting_list.term,
                declared_df: posting_list.df,
                actual: posting_list.postings.len(),
            });
        }

        let term_bytes = posting_list.term.clone().into_bytes();
        if per_term.contains_key(&term_bytes) {
            return Err(CiffImportError::DuplicateTerm(posting_list.term));
        }

        let mut doc_postings: Vec<(u32, Vec<u32>)> =
            Vec::with_capacity(posting_list.postings.len());
        // Delta-gap decode: a running sum seeded at 0
        // (`references/ciff-common-index-file-format.md`'s grounding
        // against `pisa-engine/ciff`'s real encoder/decoder).
        let mut running_docid: i64 = 0;
        let mut previous_docid: Option<i64> = None;
        for posting in &posting_list.postings {
            running_docid += i64::from(posting.docid);
            let strictly_increasing = match previous_docid {
                Some(prev) => running_docid > prev,
                None => running_docid >= 0,
            };
            if !strictly_increasing {
                return Err(CiffImportError::NonMonotonicOrNegativePosting {
                    term: posting_list.term.clone(),
                });
            }
            if running_docid < 0 || running_docid as usize >= num_docs {
                return Err(CiffImportError::DocidOutOfRange {
                    context: "posting",
                    docid: running_docid,
                    num_docs: header.num_docs,
                });
            }
            previous_docid = Some(running_docid);

            // CIFF gives term frequency directly (`Posting.tf`), never
            // per-occurrence positions (module doc comment, gap 1). This
            // placeholder vector's length carries that real tf value
            // through to `build_field_from_postings`, which derives term
            // frequency from vector length; its (meaningless) contents
            // are never read because `with_positions = false` below.
            let tf = posting.tf.max(0) as usize;
            sum_tf += i64::from(posting.tf);
            doc_postings.push((running_docid as u32, vec![0u32; tf]));
        }
        per_term.insert(term_bytes, doc_postings);
    }

    let mut doc_lengths_seen: Vec<Option<u32>> = vec![None; num_docs];
    let mut sum_doclength: i64 = 0;
    for _ in 0..header.num_docs {
        let doc_record: DocRecord = read_delimited(&mut cursor, "DocRecord")?;
        if doc_record.docid < 0 || doc_record.docid as usize >= num_docs {
            return Err(CiffImportError::DocidOutOfRange {
                context: "DocRecord",
                docid: i64::from(doc_record.docid),
                num_docs: header.num_docs,
            });
        }
        if doc_record.doclength < 0 {
            return Err(CiffImportError::NegativeDoclength {
                docid: doc_record.docid,
                value: doc_record.doclength,
            });
        }
        let slot = &mut doc_lengths_seen[doc_record.docid as usize];
        if slot.is_some() {
            return Err(CiffImportError::DuplicateDocRecord {
                docid: doc_record.docid,
            });
        }
        sum_doclength += i64::from(doc_record.doclength);
        *slot = Some(doc_record.doclength as u32);
        // `DocRecord.collection_docid` (the external, source-collection
        // document ID) is read as part of decoding this message but
        // deliberately goes no further — module doc comment, gap 2.
    }

    let doc_lengths: Vec<u32> = doc_lengths_seen
        .into_iter()
        .enumerate()
        .map(|(docid, len)| {
            len.ok_or(CiffImportError::MissingDocRecord {
                docid: docid as i32,
            })
        })
        .collect::<Result<_, _>>()?;

    if sum_doclength != header.total_terms_in_collection {
        return Err(CiffImportError::DoclengthSumMismatch {
            header_value: header.total_terms_in_collection,
            computed: sum_doclength,
        });
    }
    // The collection-frequency cross-check only holds for a *complete*
    // vocabulary export: a query-term subset (this importer's Non-goals,
    // module doc comment) legitimately sums to less than
    // total_terms_in_collection, and that is not corruption.
    if full_vocabulary && sum_tf != header.total_terms_in_collection {
        return Err(CiffImportError::DoclengthSumMismatch {
            header_value: header.total_terms_in_collection,
            computed: sum_tf,
        });
    }

    let field_blobs = build_field_from_postings(field_name, per_term, doc_lengths, false);
    Ok((field_blobs, num_docs as u64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use strand_core::container::{Footer, Hotcache};
    use strand_core::segment::SegmentBuilder;
    use strand_lexical::field::FieldReader;

    /// A real 337-byte CIFF export, fetched from `pisa-engine/ciff`'s own
    /// test fixtures (`tests/test_data/toy-complete-20200309.ciff`) and
    /// vendored at `conformance/ciff/` — not hand-rolled
    /// (`references/ciff-common-index-file-format.md`). Its embedded
    /// `Header.description` names its real provenance: "Export of toy
    /// 3-document collection from Anserini's
    /// io.anserini.integration.TrecEndToEndTest test case." A complete
    /// export: num_docs == total_docs == 3, num_postings_lists ==
    /// total_postings_lists == 9.
    const TOY_CIFF: &[u8] = include_bytes!("../../../conformance/ciff/toy-complete-20200309.ciff");

    fn open_segment_bytes(bytes: &[u8]) -> Hotcache {
        let footer_bytes: [u8; 40] = bytes[bytes.len() - 40..].try_into().unwrap();
        let footer = Footer::decode(&footer_bytes).expect("valid footer");
        let start = footer.hotcache_offset as usize;
        let end = start + footer.hotcache_length as usize;
        Hotcache::decode(&bytes[start..end]).expect("valid hotcache")
    }

    #[test]
    fn imports_a_real_ciff_file_and_answers_real_term_queries() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("toy.ciff");
        std::fs::write(&path, TOY_CIFF).expect("write fixture");

        let (field_blobs, row_count) = import_ciff(&path, "body").expect("import succeeds");
        assert_eq!(row_count, 3);
        assert_eq!(field_blobs.doc_lengths.len(), 3);

        let mut builder = SegmentBuilder::new(row_count);
        for blob in field_blobs.to_blob_specs() {
            builder.add_blob(blob);
        }
        let segment_bytes = builder.build(0);
        let hotcache = open_segment_bytes(&segment_bytes);

        let reader = FieldReader::open_by_name(&segment_bytes, &hotcache.blobs, "body")
            .expect("term-dictionary/term-info/postings blobs are present");

        // Grounded against the fixture's own real bytes
        // (`references/ciff-common-index-file-format.md`'s worked
        // decode): term "01" has df=1, appearing once in doc 0.
        let doc0_hits: Vec<u32> = reader
            .lookup("01")
            .expect("'01' is a real term in the fixture")
            .into_iter()
            .map(|(d, _)| d)
            .collect();
        assert_eq!(doc0_hits, vec![0]);

        // No positions blob was built (CIFF cannot supply positions) —
        // this importer's own stated Non-goal (module doc comment).
        assert!(reader.lookup_with_positions("01").is_none());
    }

    #[test]
    fn rejects_a_truncated_ciff_file() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("truncated.ciff");
        // Cut the fixture off mid-Header: real files are always at least
        // this long, so this is a genuine truncation, not an edge case
        // the fixture happens not to exercise.
        std::fs::write(&path, &TOY_CIFF[..10]).expect("write truncated fixture");

        match import_ciff(&path, "body") {
            Err(CiffImportError::UnexpectedEof { .. }) => {}
            Err(other) => panic!("expected UnexpectedEof, got {other}"),
            Ok(_) => panic!("expected an error, got Ok"),
        }
    }
}
