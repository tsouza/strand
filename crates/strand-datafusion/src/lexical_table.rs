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

//! `StrandLexicalTable`: one STRAND segment's one lexical field, exposed as
//! a real Apache DataFusion `TableProvider` (verified against DataFusion
//! `55.0.0`'s actual trait, `datafusion/session/src/table.rs` in
//! `apache/datafusion` tag `55.0.0` — not implemented from memory, per
//! `CLAUDE.md` §3; vendored in
//! `references/datafusion-tableprovider-trait-55.0.0.md`).
//!
//! ## The relational schema, and why it stops where it does
//!
//! Three columns, all real, all decoded from bytes the field's blobs
//! actually store (`strand_lexical::field::FieldReader::all_postings`):
//!
//! | Column      | Type   | Source                                                          |
//! |-------------|--------|------------------------------------------------------------------|
//! | `row_id`    | UInt64 | `hotcache.row_id_base + doc_ordinal` (`spec/row-ids.md` §1)      |
//! | `term`      | Utf8   | a term dictionary entry (`spec/term-dictionary.md`)              |
//! | `term_freq` | UInt32 | that term's occurrence count in that document (`spec/postings.md`)|
//!
//! One row per `(document, term)` pair that actually co-occurs — a sparse
//! term-document matrix in coordinate form, term-level statistics rather
//! than document reconstruction. `spec/row-ids.md` §1 states the row-ID
//! arithmetic normatively ("`local_ordinal = row_id - row_id_base` is the
//! one arithmetic every reader needs"), so mapping a field's postings
//! `doc_ordinal` (already that field's local ordinal, per
//! `strand_lexical::field`'s own doc comment: "`docs[i]` becomes doc
//! ordinal `i` — the row-ID space a segment built from this field's blobs
//! must use") through `row_id_base + doc_ordinal` is a direct,
//! unambiguous reading of an already-approved spec chapter, not a new
//! design decision — no RFC is owed for it.
//!
//! Two real things a naive reader might expect that this table does
//! **not**, and cannot, expose, because the blobs themselves do not carry
//! them:
//!
//! - **No document text column.** `spec/term-dictionary.md` and
//!   `spec/postings.md` store term statistics — which terms, in which
//!   documents, how many times, and (separately) at which positions — never
//!   the document's original bytes. There is no way to reconstruct
//!   `"the dog runs in the park"` from a lexical field's blobs; only that
//!   `"dog"`, `"runs"`, and `"park"` each occurred once in whichever
//!   document that was.
//! - **No `doc_length` column.** `crates/strand-lexical/src/field.rs`'s own
//!   module doc comment states this plainly: per-document length (`dl`) is
//!   "not yet a registered blob anywhere in the spec" — it exists only as
//!   `FieldBlobs.doc_lengths`, an in-memory `Vec<u32>` returned to the
//!   *writer* at build time, never persisted into the segment. A reader
//!   opening only the segment's bytes — which is what a `TableProvider`
//!   does — has no way to recover it. `search_bm25` takes it as a
//!   caller-supplied slice for the same reason; this table cannot do
//!   better without inventing storage `strand-lexical` itself does not
//!   have yet (RFC 0007's own Non-goals name "document-length block-max
//!   bounds" as real, separate future work).
//!
//! ## What's out of scope for this first pass, named honestly
//!
//! - **The vector family** (`crates/strand-vector`). `docs/roadmap.md`
//!   M5-1 states the lexical family is the deliberately narrow first
//!   slice; a vector-backed table (nearest-neighbor search expressed as
//!   SQL, or a flat-vector row scan) is real, separate future work.
//! - **Multi-segment tables.** This type reads exactly one already-resident
//!   segment's bytes; it does not walk a manifest snapshot's
//!   `SegmentRef` list, open each segment, and expose their union as one
//!   logical table (row-ID ranges are already disjoint and directly
//!   concatenable per `spec/row-ids.md` §1, so this is mechanical, not a
//!   design question — but it is unbuilt).
//! - **Deletion-vector filtering.** A resident segment's deletion vector
//!   (invariant 2, Roaring-encoded tombstoned row-IDs) is not consulted
//!   here; every row `all_postings` yields is returned, tombstoned or not.
//! - **Multi-field tables.** One `StrandLexicalTable` is one field. Joining
//!   several fields of the same segment into one wider table (e.g. `title`
//!   and `body` as separate columns keyed by `row_id`) is straightforward
//!   composition on top of this type but is not done here.
//! - **Filter/limit pushdown into the decode step.** `all_postings` always
//!   decodes every term's full postings list at construction time; a `WHERE
//!   term = '...'` query still benefits from DataFusion's own in-memory
//!   filter operator, just not from `strand-lexical` skipping undecoded
//!   terms. `supports_filters_pushdown` is left at its default
//!   (`Unsupported` for everything), matching this milestone's own "thin"
//!   mandate (`CLAUDE.md` §1): the term dictionary could resolve an
//!   equality filter directly with a single FST lookup instead of a full
//!   scan, but that is a real optimization for a later pass, not required
//!   for a first, honest, correct `TableProvider`.

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use datafusion::arrow::array::{RecordBatch, StringArray, UInt32Array, UInt64Array};
use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use datafusion::catalog::Session;
use datafusion::datasource::memory::MemorySourceConfig;
use datafusion::datasource::source::DataSourceExec;
use datafusion::datasource::{TableProvider, TableType};
use datafusion::error::Result as DataFusionResult;
use datafusion::logical_expr::Expr;
use datafusion::physical_plan::ExecutionPlan;

use strand_core::container::{DecodeError, Footer, Hotcache};
use strand_lexical::field::{FieldReader, FieldReaderError};

/// The schema every `StrandLexicalTable` exposes — see this module's doc
/// comment for what each column is and, as importantly, what it is not.
pub fn lexical_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("row_id", DataType::UInt64, false),
        Field::new("term", DataType::Utf8, false),
        Field::new("term_freq", DataType::UInt32, false),
    ]))
}

/// Everything that can go wrong opening a segment's bytes as a
/// `StrandLexicalTable`: a malformed footer or hotcache (the segment isn't
/// a valid STRAND container at all), a missing field (`field_name` was
/// never built into this segment), or an Arrow construction failure (an
/// internal invariant of this module, not something a caller can trigger
/// with a well-formed segment — kept as a real variant rather than an
/// `unwrap` because this reads bytes that may have come over the network).
#[derive(Debug)]
pub enum LexicalTableError {
    /// The segment's final 40 bytes are not a valid footer
    /// (`spec/container.md` §3).
    Footer(DecodeError),
    /// The footer's declared hotcache range is out of bounds, or the bytes
    /// there do not decode as a hotcache (`spec/container.md` §4).
    Hotcache(DecodeError),
    /// The segment is too short to even contain a footer.
    Truncated,
    /// `field_name`'s blobs are missing or malformed
    /// (`strand_lexical::field::FieldReaderError`).
    Field(FieldReaderError),
    /// Building the materialized `RecordBatch` failed.
    Arrow(datafusion::arrow::error::ArrowError),
}

impl fmt::Display for LexicalTableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LexicalTableError::Footer(e) => write!(f, "invalid segment footer: {e:?}"),
            LexicalTableError::Hotcache(e) => write!(f, "invalid segment hotcache: {e:?}"),
            LexicalTableError::Truncated => {
                write!(f, "segment bytes are too short to contain a footer")
            }
            LexicalTableError::Field(e) => write!(f, "field: {e}"),
            LexicalTableError::Arrow(e) => write!(f, "building record batch: {e}"),
        }
    }
}

impl std::error::Error for LexicalTableError {}

/// A read-only DataFusion table over one field of one resident STRAND
/// segment. See this module's doc comment for the schema and its honest
/// scope.
///
/// Construction (`try_new`) does the real work: it decodes the segment's
/// footer and hotcache (invariant 3's cold-open path — a footer read, then
/// the hotcache range it names), opens the named field's term-dictionary,
/// term-info, and postings blobs, and fully materializes every
/// `(term, doc_ordinal, term_freq)` triple (`FieldReader::all_postings`)
/// into one Arrow `RecordBatch` up front. `scan` therefore never decodes
/// anything — it hands the already-built batch to DataFusion's own
/// `MemorySourceConfig`/`DataSourceExec`, the same machinery
/// `datafusion::datasource::MemTable` itself uses, honoring column
/// projection pushdown for free because `MemorySourceConfig::try_new`
/// already implements it.
#[derive(Debug)]
pub struct StrandLexicalTable {
    schema: SchemaRef,
    batch: RecordBatch,
}

impl StrandLexicalTable {
    /// Opens `field_name`'s blobs from `segment_bytes` — a complete,
    /// already-fetched segment, exactly as a cold reader would hold it
    /// after invariant 3's footer-then-hotcache open — and materializes
    /// this table. Returns `Err` if the bytes aren't a valid STRAND
    /// segment or the field isn't present; never panics on malformed
    /// input.
    pub fn try_new(segment_bytes: &[u8], field_name: &str) -> Result<Self, LexicalTableError> {
        let footer_start = segment_bytes.len().saturating_sub(40);
        let footer_bytes: [u8; 40] = segment_bytes
            .get(footer_start..)
            .and_then(|tail| tail.try_into().ok())
            .ok_or(LexicalTableError::Truncated)?;
        let footer = Footer::decode(&footer_bytes).map_err(LexicalTableError::Footer)?;

        let hotcache_start = footer.hotcache_offset as usize;
        let hotcache_end = hotcache_start.saturating_add(footer.hotcache_length as usize);
        let hotcache_bytes = segment_bytes
            .get(hotcache_start..hotcache_end)
            .ok_or(LexicalTableError::Truncated)?;
        let hotcache = Hotcache::decode(hotcache_bytes).map_err(LexicalTableError::Hotcache)?;

        let reader = FieldReader::open_by_name(segment_bytes, &hotcache.blobs, field_name)
            .map_err(LexicalTableError::Field)?;

        let mut row_ids: Vec<u64> = Vec::new();
        let mut terms: Vec<String> = Vec::new();
        let mut term_freqs: Vec<u32> = Vec::new();
        for (term, doc_ordinal, term_freq) in reader.all_postings() {
            row_ids.push(hotcache.row_id_base + u64::from(doc_ordinal));
            terms.push(term);
            term_freqs.push(term_freq);
        }

        let schema = lexical_schema();
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![
                Arc::new(UInt64Array::from(row_ids)),
                Arc::new(StringArray::from(terms)),
                Arc::new(UInt32Array::from(term_freqs)),
            ],
        )
        .map_err(LexicalTableError::Arrow)?;

        Ok(StrandLexicalTable { schema, batch })
    }
}

#[async_trait]
impl TableProvider for StrandLexicalTable {
    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }

    fn table_type(&self) -> TableType {
        TableType::Base
    }

    async fn scan(
        &self,
        _state: &dyn Session,
        projection: Option<&Vec<usize>>,
        _filters: &[Expr],
        _limit: Option<usize>,
    ) -> DataFusionResult<Arc<dyn ExecutionPlan>> {
        let partitions = vec![vec![self.batch.clone()]];
        let source = MemorySourceConfig::try_new(&partitions, self.schema(), projection.cloned())?;
        Ok(DataSourceExec::from_data_source(source))
    }
}
