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
//! Scope, deliberately narrow: one field, one segment, no compaction, no
//! merge. `build_field` builds every field with positions by default
//! (matching tantivy's own default for a `TEXT` field);
//! `build_field_without_positions` opts a field out entirely, using RFC
//! 0009's short, positions-free term-info record (`blob_type_id = 4`)
//! instead of paying `TermInfo`'s 12-byte-per-term dead weight for a
//! feature that field will never use (`rfcs/0009-per-term-overhead-
//! reduction.md`). Filter bitmaps (RFC 0006) are a separate field kind,
//! not included. Multi-field blob addressing is unsolved project-wide (RFC
//! 0008's/RFC 0009's own Non-goals) — this module assumes exactly one
//! `family_id = 1` blob of each `blob_type_id` per segment, which is true
//! for a single-field index and stays a stated boundary, not a silent
//! assumption, until that question is resolved.
//!
//! Document length (`dl`) and the collection average (`avdl`) that
//! `strand_core::scoring::Bm25Profile` needs are not yet a registered blob
//! anywhere in the spec (RFC 0007's Non-goals name "document-length
//! block-max bounds" as real, separate future work) — `search_bm25` below
//! takes `doc_lengths` as a caller-supplied slice rather than inventing
//! storage for it.

use std::collections::{BTreeMap, HashMap};

use strand_core::container::{BlobEntry, ChunkCodec, StorageClass, Tier};
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
pub fn build_field(docs: &[&str]) -> FieldBlobs {
    build_field_impl(docs, true)
}

/// `build_field`, but this field never carries positions (RFC 0009 Design
/// §2): no phrase queries will ever be possible against it, and in
/// exchange it pays none of `TermInfo`'s 12-byte-per-term dead weight for
/// `positions_offset`/`positions_length` fields it will never populate.
/// The returned `FieldBlobs.positions` is empty and `to_blob_specs`
/// already omits it from the segment.
pub fn build_field_without_positions(docs: &[&str]) -> FieldBlobs {
    build_field_impl(docs, false)
}

fn build_field_impl(docs: &[&str], with_positions: bool) -> FieldBlobs {
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
            per_doc_positions.entry(token).or_default().push(position as u32);
        }
        // Documents are processed in ascending doc_ordinal order and each
        // contributes at most one entry per term, so every term's postings
        // list is already strictly increasing by construction —
        // build_postings's precondition. Positions within a document are
        // pushed in token order, so they're already strictly increasing —
        // build_positions's precondition.
        for (term, positions) in per_doc_positions {
            per_term.entry(term.into_bytes()).or_default().push((doc_ordinal as u32, positions));
        }
    }

    build_field_from_postings(per_term, doc_lengths, with_positions)
}

/// Builds a field's blobs directly from already-extracted per-term postings
/// — the shared core `build_field`/`build_field_without_positions` and any
/// other real data source (e.g. `strand-tools convert`'s tantivy importer)
/// both funnel into. `per_term` maps each term's bytes (already in
/// unsigned UTF-8 byte order, invariant 11 — a `BTreeMap` guarantees this)
/// to that term's postings: `(doc_ordinal, within_document_positions)`
/// pairs, in ascending `doc_ordinal` order. `doc_lengths[i]` is document
/// `i`'s token count (`spec/scoring-profiles.md`'s `dl` input).
///
/// # Panics
///
/// Panics if any term's `doc_ordinal`s are not strictly increasing, or if
/// `with_positions` is `true` and any posting's positions are not
/// non-empty and strictly increasing (`postings::build_postings`'s and
/// `positions::build_positions`'s own preconditions — this function does
/// not re-validate them itself, matching `build_field_impl`'s own
/// by-construction guarantee for the analyzer-based path).
pub fn build_field_from_postings(
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
            let doc_positions: Vec<Vec<u32>> = term_postings.iter().map(|(_, p)| p.clone()).collect();
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

    let refs: Vec<(&[u8], TermInfo)> =
        terms_with_info.iter().map(|(term, info)| (term.as_slice(), *info)).collect();
    let (term_dict, term_info) = if with_positions {
        term_dictionary::build_term_dictionary(&refs).expect("terms are sorted by construction")
    } else {
        term_dictionary::build_term_dictionary_short(&refs).expect("terms are sorted by construction")
    };

    FieldBlobs {
        term_dict,
        term_info,
        postings: postings_bytes,
        positions: positions_bytes,
        has_positions: with_positions,
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
    /// Opens a field's blobs from a resident segment's raw bytes and its
    /// already-decoded `Hotcache`'s blob registry
    /// (`strand_core::container::Hotcache::blobs`) — no further round trip,
    /// per invariant 3's one-wave rule. The term-info blob may be either
    /// shape (`blob_type_id = 1` or `4`, tried in that order); the
    /// positions blob is always optional, and additionally never present
    /// when the short (`4`) term-info shape is in use (RFC 0009 Design
    /// §2's mutual-exclusivity rule) — this method does not itself verify
    /// that exclusivity (Non-goals: no mechanism exists yet to check it
    /// against a real field identity, `spec/term-dictionary.md` §3a).
    pub fn open(segment_bytes: &'a [u8], blobs: &[BlobEntry]) -> Result<Self, FieldReaderError> {
        let find = |blob_type_id: u16| {
            blobs.iter().find(|b| b.family_id == LEXICAL_FAMILY && b.blob_type_id == blob_type_id)
        };
        let slice_of =
            |entry: &BlobEntry| &segment_bytes[entry.offset as usize..(entry.offset + entry.length) as usize];

        let dict_entry =
            find(BLOB_TYPE_TERM_DICTIONARY).ok_or(FieldReaderError::MissingBlob("term-dictionary"))?;
        let postings_entry = find(BLOB_TYPE_POSTINGS).ok_or(FieldReaderError::MissingBlob("postings"))?;

        let term_dict = TermDictionary::open(slice_of(dict_entry).to_vec())
            .map_err(FieldReaderError::TermDictionary)?;

        let term_info = if let Some(entry) = find(BLOB_TYPE_TERM_INFO) {
            TermInfoSource::Full(
                TermInfoStore::new(slice_of(entry)).map_err(FieldReaderError::TermInfoStore)?,
            )
        } else if let Some(entry) = find(BLOB_TYPE_TERM_INFO_SHORT) {
            TermInfoSource::Short(
                ShortTermInfoStore::new(slice_of(entry)).map_err(FieldReaderError::TermInfoStore)?,
            )
        } else {
            return Err(FieldReaderError::MissingBlob("term-info"));
        };

        let postings = slice_of(postings_entry);
        let positions = find(BLOB_TYPE_POSITIONS).map(slice_of);

        Ok(FieldReader { term_dict, term_info, postings, positions })
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
        let reader = PostingsReader::new(&self.postings[start..end], info.doc_freq as usize).ok()?;
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
        let postings_reader =
            PostingsReader::new(&self.postings[postings_start..postings_end], info.doc_freq as usize).ok()?;
        let (ordinals, term_freqs) = postings_reader.decode_all();

        let pos_start = info.positions_offset as usize;
        let pos_end = pos_start + info.positions_length as usize;
        let positions_reader =
            PositionsReader::new(&positions_bytes[pos_start..pos_end], info.doc_freq as usize).ok()?;
        let all_positions = positions_reader.decode_all(&term_freqs);

        Some(ordinals.into_iter().zip(all_positions).collect())
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
