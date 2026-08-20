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

//! Wires the lexical family's four already-approved blobs (term-dictionary
//! FST, term-info store, postings, positions — RFC 0005/0007/0008) into a
//! real `strand-core` segment, and back: a writer path from raw document
//! text to `strand_core::segment::BlobSpec`s, and a reader path from a
//! resident segment's blob registry to working term and phrase queries.
//! This is the first real composition of the two crates — until now each
//! blob type was built and tested in isolation (byte-exact golden files,
//! in-memory round trips) with nothing assembling them into a segment or
//! resolving a query end to end.
//!
//! Scope, deliberately narrow: one segment, no compaction, no merge.
//! `build_field` builds every field with positions by default (matching
//! tantivy's own default for a `TEXT` field); `build_field_without_positions`
//! opts a field out entirely, using RFC 0009's short, positions-free
//! term-info record (`blob_type_id = 4`) instead of paying `TermInfo`'s
//! 12-byte-per-term dead weight for a feature that field will never use
//! (`rfcs/0009-per-term-overhead-reduction.md`). Filter bitmaps (RFC 0006)
//! are a separate field kind, not included.
//!
//! Multi-field blob addressing (roadmap item X-1,
//! `rfcs/0001-container-rowid-manifest.md` Discussion — the gap RFC 0008's
//! and RFC 0009's own Non-goals both flagged as unsolved project-wide) is
//! now solved at the registry level:
//! every blob this module builds carries a `field_id`
//! (`strand_core::container::field_id_from_name`, `spec/container.md`
//! §5a), computed from the field's own declared name and passed to
//! `build_field`/`build_field_without_positions`/`build_field_from_postings`
//! explicitly. `FieldReader::open`/`open_by_name` select a field's blobs by
//! `(family_id, blob_type_id, field_id)`, not `(family_id, blob_type_id)`
//! alone, so a segment may now hold any number of fields, each with its own
//! term-dictionary/term-info/postings/positions blobs, without collision.
//!
//! Document length (`dl`) and the collection average (`avdl`) that
//! `strand_core::scoring::Bm25Profile` needs are not yet a registered blob
//! anywhere in the spec (RFC 0007's Non-goals name "document-length
//! block-max bounds" as real, separate future work) — `search_bm25` below
//! takes `doc_lengths` as a caller-supplied slice rather than inventing
//! storage for it.

use std::collections::{BTreeMap, HashMap};

use strand_core::container::{BlobEntry, ChunkCodec, StorageClass, Tier, field_id_from_name};
use strand_core::scoring::Bm25Profile;
use strand_core::segment::BlobSpec;

use crate::positions::{self, PositionsReader};
use crate::postings::{self, PostingsReader};
use crate::term_dictionary::{
    self, ShortTermInfoStore, TermDictionary, TermDictionaryError, TermInfo, TermInfoStore,
    TermInfoStoreError,
};

/// `family_id` all four blobs this module builds share (`spec/
/// term-dictionary.md`, `spec/postings.md`, `spec/positions.md`:
/// `family_id = 1`, "lexical").
const LEXICAL_FAMILY: u16 = 1;
/// Registered `blob_type_id`s within the lexical family (`spec/
/// container.md` §9).
const BLOB_TYPE_TERM_DICTIONARY: u16 = 0;
const BLOB_TYPE_TERM_INFO: u16 = 1;
const BLOB_TYPE_POSTINGS: u16 = 2;
const BLOB_TYPE_POSITIONS: u16 = 3;
const BLOB_TYPE_TERM_INFO_SHORT: u16 = 4;

/// A writer-chosen byte alignment for these raw-mappable blobs
/// (`spec/container.md` §5 leaves the exact value to the writer, not the
/// spec) — 8, matching RFC 0001's own worked example.
const BLOB_ALIGNMENT: u16 = 8;

/// The built blobs for one field, plus what a caller needs to place them in
/// a segment and, later, score matches. `positions` is empty and
/// `has_positions` is `false` when built via `build_field_without_positions`
/// (RFC 0009 Design §2) — `to_blob_specs` reads `has_positions` to decide
/// which term-info record shape and whether to emit a positions blob at
/// all; nothing else in this struct's shape changes.
pub struct FieldBlobs {
    pub term_dict: Vec<u8>,
    pub term_info: Vec<u8>,
    pub postings: Vec<u8>,
    pub positions: Vec<u8>,
    has_positions: bool,
    /// This field's registry-entry `field_id` (`spec/container.md` §5a):
    /// `field_id_from_name` applied to the field name passed to whichever
    /// builder produced this `FieldBlobs`. Every `BlobSpec` `to_blob_specs`
    /// emits carries this same value, which is what lets two fields'
    /// same-`blob_type_id` blobs coexist in one segment.
    pub field_id: u64,
    /// One entry per document, in the same order as `docs` was given to
    /// `build_field` — that order is this field's row-ID/doc-ordinal space.
    pub doc_lengths: Vec<u32>,
}

/// Analyzes every document with `analyzer::analyze_lucene_en_word_only`
/// (the same chain `bench/src/msmarco_index.rs` uses) and builds this
/// field's term dictionary, term-info store, postings, and positions
/// blobs. `docs[i]` becomes doc ordinal `i` — the row-ID space a segment
/// built from this field's blobs must use (`spec/row-ids.md` §1). Token
/// index within each document (`0`-based) becomes that token's within-
/// document position for the positions blob (`spec/positions.md` §2).
pub fn build_field(field_name: &str, docs: &[&str]) -> FieldBlobs {
    build_field_impl(field_name, docs, true)
}

/// `build_field`, but this field never carries positions (RFC 0009 Design
/// §2): no phrase queries will ever be possible against it, and in
/// exchange it pays none of `TermInfo`'s 12-byte-per-term dead weight for
/// `positions_offset`/`positions_length` fields it will never populate.
/// The returned `FieldBlobs.positions` is empty and `to_blob_specs`
/// already omits it from the segment.
pub fn build_field_without_positions(field_name: &str, docs: &[&str]) -> FieldBlobs {
    build_field_impl(field_name, docs, false)
}

fn build_field_impl(field_name: &str, docs: &[&str], with_positions: bool) -> FieldBlobs {
    // BTreeMap<Vec<u8>, _> iterates in unsigned byte order (invariant 11) —
    // exactly the order build_term_dictionary requires, with no separate
    // sort step. Each posting carries its within-document positions
    // alongside its term frequency, so postings and positions are built
    // from the same source data rather than two separate passes (even when
    // `with_positions` is false, tracking positions here is free — they're
    // simply never encoded into a positions blob below).
    let mut per_term: BTreeMap<Vec<u8>, Vec<(u32, Vec<u32>)>> = BTreeMap::new();
    let mut doc_lengths = Vec::with_capacity(docs.len());

    for (doc_ordinal, text) in docs.iter().enumerate() {
        let tokens = crate::analyzer::analyze_lucene_en_word_only(text);
        doc_lengths.push(tokens.len() as u32);

        let mut per_doc_positions: HashMap<String, Vec<u32>> = HashMap::new();
        for (position, token) in tokens.into_iter().enumerate() {
            per_doc_positions
                .entry(token)
                .or_default()
                .push(position as u32);
        }
        // Documents are processed in ascending doc_ordinal order and each
        // contributes at most one entry per term, so every term's postings
        // list is already strictly increasing by construction —
        // build_postings's precondition. Positions within a document are
        // pushed in token order, so they're already strictly increasing —
        // build_positions's precondition.
        for (term, positions) in per_doc_positions {
            per_term
                .entry(term.into_bytes())
                .or_default()
                .push((doc_ordinal as u32, positions));
        }
    }

    build_field_from_postings(field_name, per_term, doc_lengths, with_positions)
}

/// Builds a field's blobs directly from already-extracted per-term postings
/// — the shared core `build_field`/`build_field_without_positions` and any
/// other real data source (e.g. `strand-tools convert`'s tantivy importer)
/// both funnel into. `field_name` becomes this field's registry `field_id`
/// (`field_id_from_name`, `spec/container.md` §5a) — the value every blob
/// `to_blob_specs` emits carries, and the value a reader must pass back to
/// `FieldReader::open`/`open_by_name` to find these blobs again. `per_term`
/// maps each term's bytes (already in unsigned UTF-8 byte order, invariant
/// 11 — a `BTreeMap` guarantees this) to that term's postings:
/// `(doc_ordinal, within_document_positions)` pairs, in ascending
/// `doc_ordinal` order. `doc_lengths[i]` is document `i`'s token count
/// (`spec/scoring-profiles.md`'s `dl` input).
///
/// # Panics
///
/// Panics if `field_name` is empty (`field_id_from_name`'s own
/// precondition), if any term's `doc_ordinal`s are not strictly increasing,
/// or if `with_positions` is `true` and any posting's positions are not
/// non-empty and strictly increasing (`postings::build_postings`'s and
/// `positions::build_positions`'s own preconditions — this function does
/// not re-validate them itself, matching `build_field_impl`'s own
/// by-construction guarantee for the analyzer-based path).
pub fn build_field_from_postings(
    field_name: &str,
    per_term: BTreeMap<Vec<u8>, Vec<(u32, Vec<u32>)>>,
    doc_lengths: Vec<u32>,
    with_positions: bool,
) -> FieldBlobs {
    let mut postings_bytes = Vec::new();
    let mut positions_bytes = Vec::new();
    let mut terms_with_info: Vec<(Vec<u8>, TermInfo)> = Vec::with_capacity(per_term.len());
    for (term, term_postings) in per_term {
        let ordinals: Vec<u32> = term_postings.iter().map(|&(o, _)| o).collect();
        let term_freqs: Vec<u32> = term_postings.iter().map(|(_, p)| p.len() as u32).collect();

        let posting_bytes = postings::build_postings(&ordinals, &term_freqs);

        let (positions_offset, positions_length) = if with_positions {
            let doc_positions: Vec<Vec<u32>> =
                term_postings.iter().map(|(_, p)| p.clone()).collect();
            let position_bytes = positions::build_positions(&doc_positions);
            let offset = positions_bytes.len() as u64;
            let length = position_bytes.len() as u32;
            positions_bytes.extend_from_slice(&position_bytes);
            (offset, length)
        } else {
            (0, 0)
        };

        let info = TermInfo {
            doc_freq: ordinals.len() as u32,
            postings_offset: postings_bytes.len() as u64,
            postings_length: posting_bytes.len() as u32,
            positions_offset,
            positions_length,
        };
        postings_bytes.extend_from_slice(&posting_bytes);
        terms_with_info.push((term, info));
    }

    let refs: Vec<(&[u8], TermInfo)> = terms_with_info
        .iter()
        .map(|(term, info)| (term.as_slice(), *info))
        .collect();
    let (term_dict, term_info) = if with_positions {
        term_dictionary::build_term_dictionary(&refs).expect("terms are sorted by construction")
    } else {
        term_dictionary::build_term_dictionary_short(&refs)
            .expect("terms are sorted by construction")
    };

    FieldBlobs {
        term_dict,
        term_info,
        postings: postings_bytes,
        positions: positions_bytes,
        has_positions: with_positions,
        field_id: field_id_from_name(field_name),
        doc_lengths,
    }
}

impl FieldBlobs {
    /// Wraps the built blobs as `strand_core::segment::BlobSpec`s, ready
    /// for `SegmentBuilder::add_blob` — the registered classification each
    /// spec chapter already pins (`storage-class: raw-mappable`,
    /// `tier: cold-fetchable`). Four blobs (term-dictionary, the 28-byte
    /// term-info record, postings, positions) when this field carries
    /// positions; three (term-dictionary, the 16-byte short term-info
    /// record, postings — no positions blob at all) when it doesn't
    /// (`build_field_without_positions`, RFC 0009 Design §2).
    pub fn to_blob_specs(&self) -> Vec<BlobSpec> {
        let spec = |blob_type_id: u16, data: Vec<u8>| BlobSpec {
            family_id: LEXICAL_FAMILY,
            blob_type_id,
            field_id: self.field_id,
            storage_class: StorageClass::RawMappable,
            tier: Tier::ColdFetchable,
            alignment: BLOB_ALIGNMENT,
            chunk_codec: ChunkCodec::None,
            chunk_codec_level: 0,
            data,
        };
        let mut specs = vec![
            spec(BLOB_TYPE_TERM_DICTIONARY, self.term_dict.clone()),
            spec(BLOB_TYPE_POSTINGS, self.postings.clone()),
        ];
        if self.has_positions {
            specs.push(spec(BLOB_TYPE_TERM_INFO, self.term_info.clone()));
            specs.push(spec(BLOB_TYPE_POSITIONS, self.positions.clone()));
        } else {
            specs.push(spec(BLOB_TYPE_TERM_INFO_SHORT, self.term_info.clone()));
        }
        specs
    }
}

#[derive(Debug)]
pub enum FieldReaderError {
    /// A required `blob_type_id` has no matching entry in the segment's
    /// blob registry.
    MissingBlob(&'static str),
    TermDictionary(TermDictionaryError),
    TermInfoStore(TermInfoStoreError),
}

impl std::fmt::Display for FieldReaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldReaderError::MissingBlob(name) => write!(f, "segment has no {name} blob"),
            FieldReaderError::TermDictionary(e) => write!(f, "term dictionary: {e}"),
            FieldReaderError::TermInfoStore(e) => write!(f, "term-info store: {e:?}"),
        }
    }
}

impl std::error::Error for FieldReaderError {}

/// Either term-info record shape, resident (`spec/term-dictionary.md` §3,
/// §3a) — which one a `FieldReader` holds is decided entirely by which
/// `blob_type_id` was present in the segment's registry (RFC 0009 Design
/// §2: "the registry entry's own `blob_type_id` is the shape declaration,
/// not a new flag anywhere else"), never by a separate flag here either.
enum TermInfoSource<'a> {
    Full(TermInfoStore<'a>),
    Short(ShortTermInfoStore<'a>),
}

impl TermInfoSource<'_> {
    fn get(&self, ordinal: u64) -> Option<TermInfo> {
        match self {
            TermInfoSource::Full(store) => store.get(ordinal),
            TermInfoSource::Short(store) => store.get(ordinal),
        }
    }
}

/// A resident field: the term dictionary, term-info store (either shape —
/// see `TermInfoSource`), postings, and (when present) positions blobs,
/// located from a segment's already-decoded blob registry
/// (`spec/container.md` §5) and ready to resolve real term and phrase
/// queries — the reader half of this module.
pub struct FieldReader<'a> {
    term_dict: TermDictionary<Vec<u8>>,
    term_info: TermInfoSource<'a>,
    postings: &'a [u8],
    /// `None` when the segment carries no positions blob for this field —
    /// a genuinely optional blob (`spec/positions.md` §1), not an error:
    /// either the field opted out entirely (`build_field_without_positions`,
    /// always `None` here) or the field supports positions but this
    /// particular term doesn't have any (`lookup_with_positions` checks
    /// `positions_length` separately). `phrase_query`/`lookup_with_positions`
    /// return no matches rather than erroring when this is absent.
    positions: Option<&'a [u8]>,
}

impl<'a> FieldReader<'a> {
    /// Opens one field's blobs from a resident segment's raw bytes and its
    /// already-decoded `Hotcache`'s blob registry
    /// (`strand_core::container::Hotcache::blobs`) — no further round trip,
    /// per invariant 3's one-wave rule. `field_id` selects which field
    /// among any number the segment may hold (`spec/container.md` §5a,
    /// roadmap item X-1): every candidate entry must match `field_id`
    /// exactly, not just `family_id`/`blob_type_id`, so two fields' blobs
    /// of the same `blob_type_id` never collide. Most callers that already
    /// know the field's name should call `open_by_name` instead, which
    /// computes `field_id` the same way a writer did. The term-info blob
    /// may be either shape (`blob_type_id = 1` or `4`, tried in that
    /// order); the positions blob is always optional, and additionally
    /// never present when the short (`4`) term-info shape is in use (RFC
    /// 0009 Design §2's mutual-exclusivity rule) — this method does not
    /// itself verify that exclusivity (Non-goals: no mechanism exists yet
    /// to check it against a real field identity, `spec/term-dictionary.md`
    /// §3a).
    pub fn open(
        segment_bytes: &'a [u8],
        blobs: &[BlobEntry],
        field_id: u64,
    ) -> Result<Self, FieldReaderError> {
        let find = |blob_type_id: u16| {
            blobs.iter().find(|b| {
                b.family_id == LEXICAL_FAMILY
                    && b.blob_type_id == blob_type_id
                    && b.field_id == field_id
            })
        };
        let slice_of = |entry: &BlobEntry| {
            &segment_bytes[entry.offset as usize..(entry.offset + entry.length) as usize]
        };

        let dict_entry = find(BLOB_TYPE_TERM_DICTIONARY)
            .ok_or(FieldReaderError::MissingBlob("term-dictionary"))?;
        let postings_entry =
            find(BLOB_TYPE_POSTINGS).ok_or(FieldReaderError::MissingBlob("postings"))?;

        let term_dict = TermDictionary::open(slice_of(dict_entry).to_vec())
            .map_err(FieldReaderError::TermDictionary)?;

        let term_info = if let Some(entry) = find(BLOB_TYPE_TERM_INFO) {
            TermInfoSource::Full(
                TermInfoStore::new(slice_of(entry)).map_err(FieldReaderError::TermInfoStore)?,
            )
        } else if let Some(entry) = find(BLOB_TYPE_TERM_INFO_SHORT) {
            TermInfoSource::Short(
                ShortTermInfoStore::new(slice_of(entry))
                    .map_err(FieldReaderError::TermInfoStore)?,
            )
        } else {
            return Err(FieldReaderError::MissingBlob("term-info"));
        };

        let postings = slice_of(postings_entry);
        let positions = find(BLOB_TYPE_POSITIONS).map(slice_of);

        Ok(FieldReader {
            term_dict,
            term_info,
            postings,
            positions,
        })
    }

    /// `open`, computing `field_id` from `field_name` the same way a writer
    /// did (`field_id_from_name`, `spec/container.md` §5a) — the ergonomic
    /// entry point for a caller that already knows the field's name and
    /// does not want to depend on `strand_core::container::field_id_from_name`
    /// directly. Two conforming implementations calling this with the same
    /// `field_name` against the same segment always resolve to the same
    /// blobs (invariant 11): no catalog blob or coordination is needed.
    pub fn open_by_name(
        segment_bytes: &'a [u8],
        blobs: &[BlobEntry],
        field_name: &str,
    ) -> Result<Self, FieldReaderError> {
        Self::open(segment_bytes, blobs, field_id_from_name(field_name))
    }

    /// Full query resolution (`spec/postings.md` §6): term string → FST
    /// ordinal → `TermInfo` → decode this term's whole postings list.
    /// Returns `None` on a miss (a normal outcome, `spec/term-dictionary.md`
    /// §2), not an error.
    pub fn lookup(&self, term: &str) -> Option<Vec<(u32, u32)>> {
        let ordinal = self.term_dict.get(term.as_bytes())?;
        let info = self.term_info.get(ordinal)?;
        let start = info.postings_offset as usize;
        let end = start + info.postings_length as usize;
        let reader =
            PostingsReader::new(&self.postings[start..end], info.doc_freq as usize).ok()?;
        let (ordinals, term_freqs) = reader.decode_all();
        Some(ordinals.into_iter().zip(term_freqs).collect())
    }

    /// `lookup`, then scores every match with `Bm25Profile`
    /// (`spec/scoring-profiles.md`'s `bm25`), ranked descending — the first
    /// real "search, not just decode" path in this project. `doc_lengths`
    /// must be the same per-document length array `build_field` returned
    /// (see this module's own doc comment on why it's caller-supplied, not
    /// blob-backed, today).
    pub fn search_bm25(
        &self,
        term: &str,
        doc_lengths: &[u32],
        profile: &Bm25Profile,
    ) -> Option<Vec<(u32, f64)>> {
        let matches = self.lookup(term)?;
        let doc_count = doc_lengths.len() as u64;
        let avdl = doc_lengths.iter().map(|&l| l as f64).sum::<f64>() / doc_count as f64;
        let doc_freq = matches.len() as u64;

        let mut scored: Vec<(u32, f64)> = matches
            .into_iter()
            .map(|(doc_ordinal, tf)| {
                let dl = doc_lengths[doc_ordinal as usize] as f64;
                let score = profile.score(doc_freq, doc_count, tf as f64, dl, avdl);
                (doc_ordinal, score)
            })
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        Some(scored)
    }

    /// `search_bm25`, translated from this field's local doc ordinals
    /// into real row-IDs (`spec/row-ids.md` §1: `row_id = row_id_base +
    /// local_ordinal`) instead — the same arithmetic
    /// `crates/strand-datafusion/src/lexical_table.rs` already performs
    /// for its own purpose (`row_id = hotcache.row_id_base +
    /// doc_ordinal`), needed here because a lexical result list only
    /// becomes fusable against a vector result list
    /// (`strand_vector::query::Candidate::row_id`, already a real row-ID)
    /// once both sides speak the same row-ID space — `docs/roadmap.md`'s
    /// M3-6 entry names this exact translation as the real, previously
    /// missing piece of glue hybrid RRF fusion needs. `row_id_base` MUST
    /// be the same segment's hotcache-declared base this field's blobs
    /// were built and written under (`spec/container.md` §4); this
    /// function does not itself read the hotcache, so a caller passing
    /// the wrong base silently produces wrong row-IDs — the same
    /// caller-supplied-base contract `filter_deleted` already has in
    /// `strand-vector`. Order is preserved: results stay sorted by
    /// descending BM25 score, only each entry's identity changes from a
    /// local ordinal to a global row-ID.
    pub fn search_bm25_row_ids(
        &self,
        term: &str,
        doc_lengths: &[u32],
        profile: &Bm25Profile,
        row_id_base: u64,
    ) -> Option<Vec<(u64, f64)>> {
        let scored = self.search_bm25(term, doc_lengths, profile)?;
        Some(
            scored
                .into_iter()
                .map(|(doc_ordinal, score)| (row_id_base + u64::from(doc_ordinal), score))
                .collect(),
        )
    }

    /// `lookup`, plus each match's within-document positions
    /// (`spec/positions.md` §6's full decode) — `None` if the term misses,
    /// or if this field's positions blob is absent, or if this specific
    /// term has `positions_length == 0` (`spec/positions.md` §1's per-term
    /// "absent" signal).
    pub fn lookup_with_positions(&self, term: &str) -> Option<Vec<(u32, Vec<u32>)>> {
        let ordinal = self.term_dict.get(term.as_bytes())?;
        let info = self.term_info.get(ordinal)?;
        if info.positions_length == 0 {
            return None;
        }
        let positions_bytes = self.positions?;

        let postings_start = info.postings_offset as usize;
        let postings_end = postings_start + info.postings_length as usize;
        let postings_reader = PostingsReader::new(
            &self.postings[postings_start..postings_end],
            info.doc_freq as usize,
        )
        .ok()?;
        let (ordinals, term_freqs) = postings_reader.decode_all();

        let pos_start = info.positions_offset as usize;
        let pos_end = pos_start + info.positions_length as usize;
        let positions_reader =
            PositionsReader::new(&positions_bytes[pos_start..pos_end], info.doc_freq as usize)
                .ok()?;
        let all_positions = positions_reader.decode_all(&term_freqs);

        Some(ordinals.into_iter().zip(all_positions).collect())
    }

    /// Enumerates every `(term, doc_ordinal, term_freq)` triple this field's
    /// term dictionary and postings actually store — a full field-level
    /// table scan, not a point lookup. This is the read surface
    /// `strand-datafusion`'s `TableProvider` (roadmap item M5-1) is built
    /// on: it is, deliberately, everything a resident lexical field can
    /// answer without a term string in hand already. Two real things this
    /// does *not* expose, because the blobs themselves do not carry them:
    /// the document's original text (never stored — `spec/postings.md`,
    /// `spec/term-dictionary.md` store term statistics and positions, not
    /// raw content) and each document's length (`dl`), which per this
    /// module's own doc comment is not a registered blob at all today and
    /// exists only as `FieldBlobs.doc_lengths`, in memory, at write time.
    ///
    /// `doc_ordinal` is the field's local row position (`spec/row-ids.md`
    /// §1): a caller that also holds this segment's `row_id_base` recovers
    /// the real global row-ID as `row_id_base + doc_ordinal`.
    ///
    /// Decode failures (a malformed `TermInfo` record, a postings slice
    /// that doesn't decode, an out-of-range offset) are skipped per term
    /// rather than panicking — this reads bytes that may have arrived over
    /// the network, and one corrupt term-info record should not take down
    /// a whole table scan, matching `lookup`'s own existing policy of
    /// treating a decode failure as "no match" rather than an error.
    pub fn all_postings(&self) -> impl Iterator<Item = (String, u32, u32)> + '_ {
        self.term_dict
            .iter()
            .flat_map(move |(term_bytes, ordinal)| {
                let term = String::from_utf8_lossy(&term_bytes).into_owned();
                self.decode_term_postings(ordinal)
                    .into_iter()
                    .map(move |(doc_ordinal, tf)| (term.clone(), doc_ordinal, tf))
            })
    }

    /// Decodes one term's full postings list by FST ordinal, or an empty
    /// list on any malformed input (see `all_postings`'s doc comment on
    /// why this degrades rather than panics).
    fn decode_term_postings(&self, ordinal: u64) -> Vec<(u32, u32)> {
        let Some(info) = self.term_info.get(ordinal) else {
            return Vec::new();
        };
        let start = info.postings_offset as usize;
        let end = start.saturating_add(info.postings_length as usize);
        let Some(slice) = self.postings.get(start..end) else {
            return Vec::new();
        };
        let Ok(reader) = PostingsReader::new(slice, info.doc_freq as usize) else {
            return Vec::new();
        };
        let (ordinals, term_freqs) = reader.decode_all();
        ordinals.into_iter().zip(term_freqs).collect()
    }

    /// Real phrase query resolution (`spec/positions.md` §6): documents
    /// where `terms[0], terms[1], ...` occur at consecutive within-document
    /// positions, in order. Returns an empty result — never an error — if
    /// any term misses or this field carries no positions. Correctness-
    /// first, not yet using `PositionsReader::positions_for_doc`'s
    /// block-targeted skip (real, separate follow-on): every candidate
    /// term's postings and positions are fully decoded via
    /// `lookup_with_positions` before intersecting.
    pub fn phrase_query(&self, terms: &[&str]) -> Vec<u32> {
        if terms.is_empty() {
            return Vec::new();
        }

        let mut per_term: Vec<HashMap<u32, Vec<u32>>> = Vec::with_capacity(terms.len());
        for &term in terms {
            match self.lookup_with_positions(term) {
                Some(matches) => per_term.push(matches.into_iter().collect()),
                None => return Vec::new(),
            }
        }

        let mut candidates: Vec<u32> = per_term[0].keys().copied().collect();
        for map in &per_term[1..] {
            candidates.retain(|doc| map.contains_key(doc));
        }
        candidates.sort_unstable();

        let mut matches = Vec::new();
        'doc: for doc in candidates {
            for &start in &per_term[0][&doc] {
                let aligned = per_term[1..]
                    .iter()
                    .enumerate()
                    .all(|(i, map)| map[&doc].contains(&(start + (i + 1) as u32)));
                if aligned {
                    matches.push(doc);
                    continue 'doc;
                }
            }
        }
        matches
    }
}
