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

//! The project's actual thesis, exercised for the first time end to end
//! (`CLAUDE.md`'s mission sentence, `docs/roadmap.md`'s M3-6 entry): one
//! real segment carrying both blob families over one shared row-ID space,
//! a real BM25 query against the lexical field, a real ANN query against
//! the vector field, and the two real result sets fused with
//! `strand_core::fusion::fuse` (Reciprocal Rank Fusion) into one ranking.
//!
//! Two closest existing patterns this test composes rather than
//! reinvents: `crates/strand-lexical/tests/field_end_to_end.rs` (build a
//! lexical field, commit it through a real manifest, query it cold) and
//! `crates/strand-vector/tests/segment_assembly.rs` /
//! `crates/strand-vector/tests/query_a_real_cluster.rs` (build the four
//! vector blobs, assemble a real segment, run a real nprobe scan). Both
//! fields here share one `SegmentBuilder` and one commit, so they share
//! one `row_id_base` and one `row_id_count = 6` — the real row-ID space
//! invariant 1 promises, not two segments glued together after the fact.
//!
//! ## The worked example, by hand
//!
//! Six documents, row-IDs `0..6` (a fresh, empty store's first commit
//! puts `row_id_base = 0`, confirmed below rather than assumed). The
//! lexical field's term `"widget"` occurs in exactly two documents (doc 0,
//! one token; doc 1, two tokens), so BM25 ranks doc 0 first: shorter
//! documents score higher for equal term frequency, the same
//! length-normalization property `field_end_to_end.rs`'s own `"park"`
//! test already established. Every other document (2..6) never matches
//! `"widget"` at all.
//!
//! The vector field's six raw vectors are deliberately simple: every
//! dimension of row `i` holds the same scalar `v_i`, and the query is the
//! all-zero vector, so after reranking against the flat (exact,
//! unquantized) vectors, the true squared L2 distance from the query to
//! row `i` is exactly `64 * v_i^2` — a value anyone can compute by hand,
//! not just cross-check against a brute-force loop. With `v_3 = 0.1`,
//! `v_2 = 0.2`, `v_0 = 0.3`, `v_4 = 0.4`, `v_1 = 0.5`, `v_5 = 0.6`, the
//! exact distances are `0.64, 2.56, 5.76, 10.24, 16.0, 23.04` for rows
//! `3, 2, 0, 4, 1, 5` respectively — already in ascending order, so that
//! is the vector ranking, best (nearest) first.
//!
//! Neither ranking alone puts a document appearing in *both* rankings in
//! first place: row 0 is BM25's best match but only the vector ranking's
//! *third*-nearest; row 3 is the vector ranking's nearest neighbor but
//! never matches `"widget"` at all. RRF (`k = 60`,
//! `references/cormack-clarke-buettcher-2009-reciprocal-rank-fusion.md`)
//! sums `1/(k + rank)` per ranking a row-ID appears in:
//!
//! - row 0: lexical rank 1 (`1/61`) + vector rank 3 (`1/63`) ≈ `0.032266`
//! - row 1: lexical rank 2 (`1/62`) + vector rank 5 (`1/65`) ≈ `0.031514`
//! - row 3: vector rank 1 only (`1/61`) ≈ `0.016393`
//! - row 2: vector rank 2 only (`1/62`) ≈ `0.016129`
//! - row 4: vector rank 4 only (`1/64`) ≈ `0.015625`
//! - row 5: vector rank 6 only (`1/66`) ≈ `0.015152`
//!
//! Row 0 and row 1 — mediocre on the vector side alone, never even the
//! single best vector match — outrank row 3, the vector ranking's actual
//! nearest neighbor, once both signals are fused. That is RRF's real
//! value, not a coincidence of this example's numbers: a document good
//! enough (not best) on two independent signals beats a document
//! excellent on only one. This test asserts exactly that fused order and
//! those exact scores, computed independently of `fuse`'s own
//! implementation (this file applies the paper's formula itself, by
//! hand, to the ranks the real query machinery actually produced) — the
//! same worked-example rigor `docs/ledger.md` already holds M2-1's SPANN
//! closure-replication test and M5-1's SQL end-to-end test to.

use rand::SeedableRng;
use rand::rngs::StdRng;

use strand_core::container::{ChunkCodec, Footer, Hotcache, StorageClass, Tier};
use strand_core::fusion::{self, DEFAULT_RRF_K};
use strand_core::manifest::{commit, read_snapshot};
use strand_core::scoring::Bm25Profile;
use strand_core::segment::{BlobSpec, SegmentBuilder, write_segment};
use strand_core::store::{ConditionalStore, InMemoryStore};
use strand_lexical::field::{FieldReader, build_field};
use strand_vector::descriptor::{self, DescriptorReader, DistanceMetric};
use strand_vector::flat::{FlatVectorsReader, build_flat_vectors};
use strand_vector::navigation::{NavigationTierReader, build_navigation_tier};
use strand_vector::posting_list::{ClusterInput, PostingListReader, build_posting_lists};
use strand_vector::quantize::{MetricType, quantize_one_bit};
use strand_vector::query::{rerank, scan_selected_clusters, select_nprobe_clusters};
use strand_vector::rotate::rotate_fht_kac;

const VECTOR_FAMILY: u16 = 3;
const BLOB_FLAT_VECTORS: u16 = 0;
const BLOB_DESCRIPTOR: u16 = 1;
const BLOB_NAVIGATION_TIER: u16 = 2;
const BLOB_POSTING_LISTS: u16 = 3;

const DOC_COUNT: usize = 6;
const DIMS: u32 = 64;

/// Every dimension of row `i`'s raw vector holds this same scalar — see
/// this file's module doc comment for why that makes the exact rerank
/// distance from an all-zero query `64 * v_i^2`, computable by hand.
const ROW_SCALARS: [f32; DOC_COUNT] = [0.3, 0.5, 0.2, 0.1, 0.4, 0.6];

fn open_segment_bytes(bytes: &[u8]) -> Hotcache {
    let footer_bytes: [u8; 40] = bytes[bytes.len() - 40..].try_into().unwrap();
    let footer = Footer::decode(&footer_bytes).expect("valid footer");
    let start = footer.hotcache_offset as usize;
    let end = start + footer.hotcache_length as usize;
    Hotcache::decode(&bytes[start..end]).expect("valid hotcache")
}

fn find_blob(hotcache: &Hotcache, family_id: u16, blob_type_id: u16) -> (usize, usize) {
    let entry = hotcache
        .blobs
        .iter()
        .find(|b| b.family_id == family_id && b.blob_type_id == blob_type_id)
        .unwrap_or_else(|| panic!("no family_id={family_id} blob_type_id={blob_type_id} blob"));
    (entry.offset as usize, entry.length as usize)
}

#[test]
fn hybrid_rrf_fuses_a_real_lexical_ranking_and_a_real_vector_ranking_over_one_row_id_space() {
    // ---- Lexical field: "widget" matches only docs 0 and 1. ----
    const DOCS_BODY: [&str; DOC_COUNT] = [
        "widget",                           // doc 0: dl=1, has "widget"
        "widget extra",                     // doc 1: dl=2, has "widget"
        "gamma delta epsilon",              // doc 2: dl=3, no match
        "zeta eta theta iota",              // doc 3: dl=4, no match
        "kappa lambda mu nu xi",            // doc 4: dl=5, no match
        "omicron pi rho sigma tau upsilon", // doc 5: dl=6, no match
    ];
    let field = build_field("body", &DOCS_BODY);
    assert_eq!(field.doc_lengths, vec![1, 2, 3, 4, 5, 6]);

    // ---- Vector field: a real quantization descriptor, a single
    // cluster containing all six rows, and the flat (exact) vectors used
    // for reranking. ----
    let padded_dims = DIMS as usize; // already a multiple of 64
    let mut rng = StdRng::seed_from_u64(20_260_820);
    let descriptor_bytes = descriptor::build_fht_kac(DIMS, DistanceMetric::L2, 1, &mut rng);
    let descriptor_reader = DescriptorReader::new(&descriptor_bytes).expect("valid descriptor");
    let rotation_payload = descriptor_reader.rotation_payload();

    let raw_centroid = vec![0.0f32; padded_dims];
    let rotated_centroid = rotate_fht_kac(&raw_centroid, padded_dims, rotation_payload);

    let raw_vectors: Vec<Vec<f32>> = ROW_SCALARS.iter().map(|&v| vec![v; padded_dims]).collect();
    let mut flat_data = Vec::with_capacity(DOC_COUNT * padded_dims);
    for v in &raw_vectors {
        flat_data.extend_from_slice(v);
    }
    let flat_bytes = build_flat_vectors(&flat_data, DOC_COUNT, padded_dims);

    let mut compact_codes = Vec::new();
    let mut f_add = Vec::with_capacity(DOC_COUNT);
    let mut f_rescale = Vec::with_capacity(DOC_COUNT);
    let mut f_error = Vec::with_capacity(DOC_COUNT);
    for raw_vector in &raw_vectors {
        let rotated = rotate_fht_kac(raw_vector, padded_dims, rotation_payload);
        let q = quantize_one_bit(&rotated, &rotated_centroid, MetricType::L2);
        compact_codes.extend(q.compact_code);
        f_add.push(q.f_add);
        f_rescale.push(q.f_rescale);
        f_error.push(q.f_error);
    }
    // The posting list's own row-IDs are only known once the commit
    // below assigns row_id_base (spec/row-ids.md §1); they are computed
    // inside the commit closure as real_row_ids, never as local
    // ordinals — the posting list must always carry real, global
    // row-IDs, the same contract `crates/strand-vector/src/query.rs`'s
    // `Candidate.row_id` doc comment already states for this crate's
    // query path.

    // ---- Assemble one segment, one commit: both families share
    // row_id_base and row_id_count, the real shared row-ID space. ----
    let store = InMemoryStore::new();
    let snapshot = commit(&store, |row_id_base| {
        let real_row_ids: Vec<u64> = (0..DOC_COUNT as u64).map(|i| row_id_base + i).collect();

        let mut builder = SegmentBuilder::new(DOC_COUNT as u64);
        for blob in field.to_blob_specs() {
            builder.add_blob(blob);
        }

        // Cluster posting list first (its real directory, `built_dirs`,
        // is what the navigation tier below must declare — the same
        // dependency order `segment_assembly.rs` uses and cross-checks).
        let posting_input = ClusterInput {
            compact_codes: &compact_codes,
            f_add: &f_add,
            f_rescale: &f_rescale,
            f_error: &f_error,
            row_ids: &real_row_ids,
            ex_region: None,
        };
        let (posting_list_bytes, built_dirs) = build_posting_lists(&[posting_input], padded_dims);
        assert_eq!(built_dirs.len(), 1);
        assert_eq!(built_dirs[0].vector_count, DOC_COUNT as u32);

        // Navigation tier: a single cluster, centroid at the origin
        // (rotated), covering all six rows.
        let rotated_centroid_for_nav = rotate_fht_kac(&raw_centroid, padded_dims, rotation_payload);
        let navigation_bytes =
            build_navigation_tier(&rotated_centroid_for_nav, padded_dims, &built_dirs);

        builder.add_blob(BlobSpec {
            family_id: VECTOR_FAMILY,
            field_id: 0,
            blob_type_id: BLOB_FLAT_VECTORS,
            storage_class: StorageClass::RawMappable,
            tier: Tier::NotApplicable,
            alignment: 8,
            chunk_codec: ChunkCodec::None,
            chunk_codec_level: 0,
            data: flat_bytes.clone(),
        });
        builder.add_blob(BlobSpec {
            family_id: VECTOR_FAMILY,
            field_id: 0,
            blob_type_id: BLOB_DESCRIPTOR,
            storage_class: StorageClass::RawMappable,
            tier: Tier::ColdFetchable,
            alignment: 8,
            chunk_codec: ChunkCodec::None,
            chunk_codec_level: 0,
            data: descriptor_bytes.clone(),
        });
        builder.add_blob(BlobSpec {
            family_id: VECTOR_FAMILY,
            field_id: 0,
            blob_type_id: BLOB_NAVIGATION_TIER,
            storage_class: StorageClass::RawMappable,
            tier: Tier::ColdFetchable,
            alignment: 8,
            chunk_codec: ChunkCodec::None,
            chunk_codec_level: 0,
            data: navigation_bytes,
        });
        builder.add_blob(BlobSpec {
            family_id: VECTOR_FAMILY,
            field_id: 0,
            blob_type_id: BLOB_POSTING_LISTS,
            storage_class: StorageClass::RawMappable,
            tier: Tier::ColdFetchable,
            alignment: 8,
            chunk_codec: ChunkCodec::None,
            chunk_codec_level: 0,
            data: posting_list_bytes,
        });

        vec![write_segment(
            &store,
            "segments/hybrid-0.bin",
            &builder,
            row_id_base,
        )]
    })
    .expect("commit succeeds against an empty table");

    assert_eq!(snapshot.next_row_id, DOC_COUNT as u64);
    let read_back = read_snapshot(&store)
        .expect("read succeeds")
        .expect("a snapshot exists");
    let segment_ref = &read_back.segments[0];
    let (segment_bytes, _) = ConditionalStore::get(&store, &segment_ref.path)
        .expect("get succeeds")
        .expect("segment exists");
    let hotcache = open_segment_bytes(&segment_bytes);
    assert_eq!(hotcache.row_id_count, DOC_COUNT as u64);
    let row_id_base = hotcache.row_id_base;

    // ---- Real BM25 query against the lexical field, translated to real
    // row-IDs (the M3-6 gap this task closes: field.rs previously
    // returned only local ordinals). ----
    let lexical_reader = FieldReader::open_by_name(&segment_bytes, &hotcache.blobs, "body")
        .expect("body field's blobs are present");
    let profile = Bm25Profile::default();
    let lexical_ranked_row_ids = lexical_reader
        .search_bm25_row_ids("widget", &field.doc_lengths, &profile, row_id_base)
        .expect("\"widget\" is a real term");
    assert_eq!(
        lexical_ranked_row_ids
            .iter()
            .map(|&(row_id, _)| row_id)
            .collect::<Vec<_>>(),
        vec![row_id_base, row_id_base + 1],
        "doc 0 (shorter) must rank ahead of doc 1 for equal term frequency"
    );

    // ---- Real ANN query against the vector field: nprobe covering the
    // single cluster, then rerank against the exact flat vectors so the
    // final order is the exact, hand-computable L2 order this file's own
    // doc comment states. ----
    let (nav_offset, nav_length) = find_blob(&hotcache, VECTOR_FAMILY, BLOB_NAVIGATION_TIER);
    let navigation_bytes = &segment_bytes[nav_offset..nav_offset + nav_length];
    let navigation_reader =
        NavigationTierReader::new(navigation_bytes, padded_dims).expect("valid navigation tier");
    assert_eq!(navigation_reader.num_clusters(), 1);

    let (pl_offset, pl_length) = find_blob(&hotcache, VECTOR_FAMILY, BLOB_POSTING_LISTS);
    let posting_bytes = &segment_bytes[pl_offset..pl_offset + pl_length];
    let posting_reader = PostingListReader::new(posting_bytes);

    let (flat_offset, flat_length) = find_blob(&hotcache, VECTOR_FAMILY, BLOB_FLAT_VECTORS);
    let flat_bytes_resident = &segment_bytes[flat_offset..flat_offset + flat_length];
    let flat_reader =
        FlatVectorsReader::new(flat_bytes_resident, padded_dims).expect("valid flat vectors");
    assert_eq!(flat_reader.row_id_count(), DOC_COUNT);

    let raw_query = vec![0.0f32; padded_dims];
    let rotated_query = rotate_fht_kac(&raw_query, padded_dims, rotation_payload);
    let query_factors = strand_vector::estimate::QueryFactors::new(&rotated_query, 1);
    let selected_clusters =
        select_nprobe_clusters(&navigation_reader, &rotated_query, 1, MetricType::L2);
    assert_eq!(selected_clusters, vec![0]);

    let candidates = scan_selected_clusters(
        &navigation_reader,
        &posting_reader,
        &selected_clusters,
        &rotated_query,
        &query_factors,
        MetricType::L2,
        padded_dims,
        0,
    )
    .expect("valid cluster region");
    assert_eq!(
        candidates.len(),
        DOC_COUNT,
        "nprobe=1 with one cluster scans every row"
    );

    let reranked = rerank(
        candidates,
        &flat_reader,
        row_id_base,
        &raw_query,
        MetricType::L2,
    );
    let vector_ranked_row_ids: Vec<u64> = reranked.iter().map(|c| c.row_id).collect();
    assert_eq!(
        vector_ranked_row_ids,
        vec![3, 2, 0, 4, 1, 5]
            .into_iter()
            .map(|local: u64| row_id_base + local)
            .collect::<Vec<_>>(),
        "exact rerank order must match 64 * v_i^2 computed by hand in this file's doc comment"
    );
    // Cross-check the hand-stated exact distances themselves, not just
    // their order.
    let expected_distances = [
        (3, 0.64f32),
        (2, 2.56),
        (0, 5.76),
        (4, 10.24),
        (1, 16.0),
        (5, 23.04),
    ];
    for (reranked_entry, &(local, expected_dist)) in reranked.iter().zip(expected_distances.iter())
    {
        assert_eq!(reranked_entry.row_id, row_id_base + local as u64);
        assert!(
            (reranked_entry.estimate.estimate - expected_dist).abs() < 1e-4,
            "row {local}: expected exact distance {expected_dist}, got {}",
            reranked_entry.estimate.estimate
        );
    }

    // ---- Fuse the two real, row-ID-resolved rankings with RRF. ----
    let lexical_row_ids_only: Vec<u64> = lexical_ranked_row_ids.iter().map(|&(id, _)| id).collect();
    let rankings: [&[u64]; 2] = [&lexical_row_ids_only, &vector_ranked_row_ids];
    let fused = fusion::fuse(&rankings);

    // Hand-computed expectation: this file's own module doc comment
    // states each row's score as a direct application of the paper's
    // formula to the ranks the real queries above actually produced —
    // reproduced here as literal arithmetic, independent of
    // `fusion::fuse`'s own implementation.
    assert_eq!(DEFAULT_RRF_K, 60.0);
    let k = DEFAULT_RRF_K;
    let row = |local: u64| row_id_base + local;
    let expected_scores: Vec<(u64, f64)> = vec![
        (row(0), 1.0 / (k + 1.0) + 1.0 / (k + 3.0)), // lexical rank 1, vector rank 3
        (row(1), 1.0 / (k + 2.0) + 1.0 / (k + 5.0)), // lexical rank 2, vector rank 5
        (row(3), 1.0 / (k + 1.0)),                   // vector rank 1 only
        (row(2), 1.0 / (k + 2.0)),                   // vector rank 2 only
        (row(4), 1.0 / (k + 4.0)),                   // vector rank 4 only
        (row(5), 1.0 / (k + 6.0)),                   // vector rank 6 only
    ];
    // The expected list is already sorted descending by construction
    // (verified once, independently, so the assertion below is a real
    // check and not a tautology).
    for w in expected_scores.windows(2) {
        assert!(
            w[0].1 > w[1].1,
            "expected_scores must be strictly descending: {expected_scores:?}"
        );
    }

    assert_eq!(
        fused.iter().map(|f| f.row_id).collect::<Vec<_>>(),
        expected_scores
            .iter()
            .map(|&(id, _)| id)
            .collect::<Vec<_>>(),
        "fused row-ID order must match the hand-computed expectation exactly: {fused:?}"
    );
    for (fused_entry, &(expected_id, expected_score)) in fused.iter().zip(expected_scores.iter()) {
        assert_eq!(fused_entry.row_id, expected_id);
        assert!(
            (fused_entry.score - expected_score).abs() < 1e-9,
            "row {expected_id}: expected score {expected_score}, got {}",
            fused_entry.score
        );
    }

    // The real thesis assertion: row 0 and row 1 each rank ahead of row
    // 3, even though row 3 is the single nearest neighbor on the vector
    // side alone and neither row 0 nor row 1 is the vector ranking's
    // best match. Fusing genuinely changes the answer, not just the
    // presentation.
    let position_of = |row_id: u64| fused.iter().position(|f| f.row_id == row_id).unwrap();
    assert!(position_of(row(0)) < position_of(row(3)));
    assert!(position_of(row(1)) < position_of(row(3)));
    assert_eq!(
        vector_ranked_row_ids[0],
        row(3),
        "sanity: row 3 really is the best vector-only match"
    );
}
