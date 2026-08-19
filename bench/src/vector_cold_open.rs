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

//! A real M0-style byte-budget measurement for the vector blob family
//! (RFC 0010, `spec/vectors.md`), the gap RFC 0010's own Open questions
//! names: "the same way `bench/src/cold_open.rs` gave invariant 3 a real
//! measured baseline instead of only round-trip-count arithmetic... M2's
//! own milestone gate should not be considered met until one exists."
//!
//! This assembles a real four-blob-type vector segment — real k-means
//! (`strand_vector::kmeans`), real FhtKac rotation (`strand_vector::
//! rotate`), real 1-bit RaBitQ quantization (`strand_vector::quantize`'s
//! `quantize_one_bit`), real FastScan-batched posting lists and a real
//! navigation tier (`strand_vector::posting_list`, `strand_vector::
//! navigation`) — commits it to real MinIO via `strand-core`'s actual
//! manifest CAS protocol, opens it cold, and measures two separate things
//! per `spec/vectors.md` §1's own tier distinction:
//!
//! - the real byte lengths of the quantization-descriptor and
//!   navigation-tier blobs (`tier: cold-fetchable`, fetched wholesale at
//!   open) versus the 100 MB provisional cold-open byte budget
//!   (`CLAUDE.md` §7) — the number this benchmark exists to produce;
//! - the real GET count for the open sequence (pointer, snapshot, segment
//!   — invariant 3's ≤2 RTT bound past the footer read).
//!
//! One honest limitation, stated rather than hidden: `strand-core`'s
//! `ConditionalStore::get` has no Range-GET variant yet (`docs/ledger.md`
//! has no entry for one either — grep confirms no `get_range` exists in
//! this crate), so — exactly like `bench/src/cold_open.rs` and
//! `bench/src/field_cold_open.rs` before it — the network fetch this
//! benchmark actually issues at "open" pulls the *entire* segment object
//! in one GET, cluster posting lists and flat vectors included, not just
//! the cold-fetchable open-wave subset a conforming Range-GET reader would
//! fetch. The byte-budget figure this benchmark reports is therefore
//! computed from the real, on-the-wire blob lengths recorded in the
//! segment's own hotcache registry after a real cold decode — a real
//! measurement of what those two blobs actually weigh, not a formula
//! estimate — while the measured open latency is honestly the
//! whole-segment GET's latency, a strictly *harder* number than the
//! open-wave-only fetch a Range-GET-capable reader would see, and is
//! labeled as such in the report.
//!
//! Scale: `NUM_VECTORS` real 768-dimensional vectors — the lower end of
//! the ~10,000–50,000 range, chosen so real k-means (`O(n * k * dims)` per
//! iteration) finishes in a bounded time on a single core rather than
//! chasing RFC 0010's own 1,000,000-vector segment-sizing figure, which
//! this benchmark's report cross-checks against by extrapolation instead
//! of by brute-force scale.

use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::Serialize;
use strand_bench::{CountingStore, percentile, store_for, timed, with_minio, write_report};
use strand_core::container::{ChunkCodec, Footer, Hotcache, StorageClass, Tier};
use strand_core::manifest::{commit, read_snapshot};
use strand_core::segment::{BlobSpec, SegmentBuilder, write_segment};
use strand_core::store::ConditionalStore;
use strand_vector::descriptor::{self, DescriptorReader, DistanceMetric};
use strand_vector::flat::build_flat_vectors;
use strand_vector::kmeans::{self, recommended_cluster_count};
use strand_vector::navigation::{self, ClusterDirEntry};
use strand_vector::posting_list::{self, ClusterInput};
use strand_vector::quantize::{self, MetricType};
use strand_vector::rotate::rotate_fht_kac;

const FAMILY_ID: u16 = 3;
const BLOB_FLAT_VECTORS: u16 = 0;
const BLOB_DESCRIPTOR: u16 = 1;
const BLOB_NAVIGATION_TIER: u16 = 2;
const BLOB_POSTING_LISTS: u16 = 3;

const NUM_VECTORS: usize = 10_000;
const DIMS: usize = 768;
const KMEANS_MAX_ITERS: usize = 5;
const KMEANS_SEED: u64 = 20_260_819;
const BIT_WIDTH: u8 = 1;

/// `CLAUDE.md` §7's provisional cold-open byte budget, in decimal bytes
/// (the RFC's own Napkin math section states it uses decimal MB
/// throughout, matching the 100 MB figure's own usage).
const COLD_OPEN_BYTE_BUDGET: u64 = 100_000_000;

const ITERATIONS: usize = 30;

#[derive(Serialize)]
struct VectorColdOpenResult {
    iterations: usize,
    num_vectors: usize,
    dims: usize,
    padded_dims: usize,
    num_clusters: usize,
    bit_width: u8,
    kmeans_max_iters: usize,

    /// Real byte length of the committed segment's descriptor blob
    /// (`blob_type_id = 1`), read back from the real hotcache registry.
    descriptor_bytes: u64,
    /// Real byte length of the committed segment's navigation-tier blob
    /// (`blob_type_id = 2`), read back from the real hotcache registry.
    navigation_tier_bytes: u64,
    /// `descriptor_bytes + navigation_tier_bytes` — the real bytes a
    /// conforming Range-GET reader fetches wholesale at open, per
    /// `spec/vectors.md` §1.
    open_wave_bytes: u64,
    /// For reference only, NOT part of the open-wave budget (`spec/
    /// vectors.md` §1: posting lists are fetched per-cluster at query
    /// time, `tier: n/a` flat vectors are fetched only for reranking).
    posting_list_bytes_for_reference: u64,
    flat_vector_bytes_for_reference: u64,
    segment_total_bytes: u64,

    cold_open_byte_budget: u64,
    open_wave_pct_of_budget: f64,

    /// `open_wave_bytes` extrapolated to RFC 0010's own 1,000,000-vector
    /// napkin-math scale, using this run's real per-cluster navigation-tier
    /// cost and `recommended_cluster_count`'s `4*sqrt(N)` rule (the same
    /// rule RFC 0010's Napkin math uses) — a cross-check against that
    /// RFC's own formula-derived ≈12.4 MB figure, not an independent
    /// measurement at that scale.
    extrapolated_open_wave_bytes_at_1m_vectors: f64,

    /// GETs for the open sequence alone (pointer, snapshot, segment) —
    /// must be constant and \u{2264} 4 per invariant 3.
    get_count_per_open: u64,
    /// Latency of the real whole-segment GET this harness issues at open
    /// (no Range-GET reader exists yet in `strand-core` — see module doc).
    /// This is a harder number than a Range-GET-only open-wave fetch would
    /// be, since it downloads the posting-list and flat-vector blobs too.
    open_latency_ms_p50: f64,
    open_latency_ms_p90: f64,
    open_latency_ms_p99: f64,
    open_latency_ms_min: f64,
    open_latency_ms_max: f64,
}

fn open_segment_bytes(bytes: &[u8]) -> Hotcache {
    let footer_bytes: [u8; 40] = bytes[bytes.len() - 40..].try_into().unwrap();
    let footer = Footer::decode(&footer_bytes).expect("valid footer");
    let start = footer.hotcache_offset as usize;
    let end = start + footer.hotcache_length as usize;
    Hotcache::decode(&bytes[start..end]).expect("valid hotcache")
}

fn find_blob_len(hotcache: &Hotcache, blob_type_id: u16) -> u64 {
    hotcache
        .blobs
        .iter()
        .find(|b| b.family_id == FAMILY_ID && b.blob_type_id == blob_type_id)
        .unwrap_or_else(|| panic!("no family_id=3 blob_type_id={blob_type_id} in the registry"))
        .length
}

/// Assembles the vector family's four real blobs (`spec/vectors.md`) for
/// `n` real, random `dims`-dimensional vectors: real k-means on the raw
/// vectors, a real `FhtKacRotator` descriptor, real rotation applied to
/// both centroids and vectors, real 1-bit RaBitQ quantization per vector
/// against its assigned (rotated) centroid, and real FastScan-batched
/// posting lists plus a real navigation tier built from the result.
#[allow(clippy::too_many_arguments)]
fn build_real_vector_segment(
    n: usize,
    dims: usize,
    max_iters: usize,
    rng: &mut StdRng,
) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, usize) {
    eprintln!("Generating {n} real random {dims}-dim vectors...");
    let vectors: Vec<f32> = (0..n * dims)
        .map(|_| rng.random_range(-1.0f32..1.0))
        .collect();

    let num_clusters = recommended_cluster_count(n);
    eprintln!(
        "Running real k-means: n={n}, dims={dims}, k={num_clusters} (4*sqrt(n)), max_iters={max_iters}..."
    );
    let (kmeans_result, kmeans_elapsed_ms) =
        timed(|| kmeans::kmeans(&vectors, n, dims, num_clusters, max_iters, rng));
    eprintln!("k-means done in {kmeans_elapsed_ms:.0}ms");

    eprintln!("Building real FhtKac quantization descriptor...");
    let descriptor_bytes =
        descriptor::build_fht_kac(dims as u32, DistanceMetric::L2, BIT_WIDTH, rng);
    let descriptor_reader = DescriptorReader::new(&descriptor_bytes).expect("valid descriptor");
    let padded_dims = descriptor_reader.padded_dims() as usize;
    let flip = descriptor_reader.rotation_payload().to_vec();
    assert_eq!(
        padded_dims, dims,
        "dims=768 is already a multiple of 64; padding should be a no-op at this scale"
    );

    eprintln!("Rotating {num_clusters} real centroids...");
    let mut rotated_centroids = Vec::with_capacity(num_clusters * padded_dims);
    for c in 0..num_clusters {
        let centroid = &kmeans_result.centroids[c * dims..(c + 1) * dims];
        rotated_centroids.extend(rotate_fht_kac(centroid, padded_dims, &flip));
    }

    eprintln!("Rotating and quantizing {n} real vectors against their assigned real centroid...");
    let mut per_cluster_codes: Vec<Vec<u8>> = vec![Vec::new(); num_clusters];
    let mut per_cluster_f_add: Vec<Vec<f32>> = vec![Vec::new(); num_clusters];
    let mut per_cluster_f_rescale: Vec<Vec<f32>> = vec![Vec::new(); num_clusters];
    let mut per_cluster_f_error: Vec<Vec<f32>> = vec![Vec::new(); num_clusters];
    let mut per_cluster_row_ids: Vec<Vec<u64>> = vec![Vec::new(); num_clusters];

    let (_, quantize_elapsed_ms) = timed(|| {
        for i in 0..n {
            let data = &vectors[i * dims..(i + 1) * dims];
            let rotated = rotate_fht_kac(data, padded_dims, &flip);
            let c = kmeans_result.assignments[i];
            let rotated_centroid = &rotated_centroids[c * padded_dims..(c + 1) * padded_dims];
            let q = quantize::quantize_one_bit(&rotated, rotated_centroid, MetricType::L2);
            per_cluster_codes[c].extend(q.compact_code);
            per_cluster_f_add[c].push(q.f_add);
            per_cluster_f_rescale[c].push(q.f_rescale);
            per_cluster_f_error[c].push(q.f_error);
            per_cluster_row_ids[c].push(i as u64);
        }
    });
    eprintln!("Rotation + quantization done in {quantize_elapsed_ms:.0}ms");

    let clusters: Vec<ClusterInput> = (0..num_clusters)
        .map(|c| ClusterInput {
            compact_codes: &per_cluster_codes[c],
            f_add: &per_cluster_f_add[c],
            f_rescale: &per_cluster_f_rescale[c],
            f_error: &per_cluster_f_error[c],
            row_ids: &per_cluster_row_ids[c],
            ex_region: None,
        })
        .collect();

    let (posting_list_bytes, dirs): (Vec<u8>, Vec<ClusterDirEntry>) =
        posting_list::build_posting_lists(&clusters, padded_dims);
    let navigation_bytes =
        navigation::build_navigation_tier(&rotated_centroids, padded_dims, &dirs);
    let flat_bytes = build_flat_vectors(&vectors, n, dims);

    (
        descriptor_bytes,
        navigation_bytes,
        posting_list_bytes,
        flat_bytes,
        num_clusters,
    )
}

fn main() {
    let mut rng = StdRng::seed_from_u64(KMEANS_SEED);
    let (descriptor_bytes, navigation_bytes, posting_list_bytes, flat_bytes, num_clusters) =
        build_real_vector_segment(NUM_VECTORS, DIMS, KMEANS_MAX_ITERS, &mut rng);

    eprintln!(
        "Real blobs built: descriptor={} B, navigation_tier={} B, posting_lists={} B, flat_vectors={} B",
        descriptor_bytes.len(),
        navigation_bytes.len(),
        posting_list_bytes.len(),
        flat_bytes.len(),
    );

    with_minio(|endpoint, bucket| {
        let store = store_for(endpoint, bucket);

        let mut builder = SegmentBuilder::new(NUM_VECTORS as u64);
        builder.add_blob(BlobSpec {
            family_id: FAMILY_ID,
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
            family_id: FAMILY_ID,
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
            family_id: FAMILY_ID,
            field_id: 0,
            blob_type_id: BLOB_NAVIGATION_TIER,
            storage_class: StorageClass::RawMappable,
            tier: Tier::ColdFetchable,
            alignment: 8,
            chunk_codec: ChunkCodec::None,
            chunk_codec_level: 0,
            data: navigation_bytes.clone(),
        });
        builder.add_blob(BlobSpec {
            family_id: FAMILY_ID,
            field_id: 0,
            blob_type_id: BLOB_POSTING_LISTS,
            storage_class: StorageClass::RawMappable,
            tier: Tier::ColdFetchable,
            alignment: 8,
            chunk_codec: ChunkCodec::None,
            chunk_codec_level: 0,
            data: posting_list_bytes.clone(),
        });

        let snapshot = commit(&store, |row_id_base| {
            vec![write_segment(
                &store,
                "segments/vector-cold-open.bin",
                &builder,
                row_id_base,
            )]
        })
        .expect("commit succeeds against an empty table");
        let segment_total_bytes = snapshot.segments[0].byte_length;
        eprintln!("Committed real segment: {segment_total_bytes} bytes on real MinIO");

        let counting = CountingStore::new(&store);
        let mut latencies = Vec::with_capacity(ITERATIONS);
        let mut get_counts = Vec::with_capacity(ITERATIONS);
        let mut last_hotcache: Option<Hotcache> = None;

        for _ in 0..ITERATIONS {
            counting.reset();
            let (hotcache, elapsed_ms) = timed(|| {
                let snapshot = read_snapshot(&counting).unwrap().unwrap();
                let segment_ref = &snapshot.segments[0];
                let (segment_bytes, _) = counting.get(&segment_ref.path).unwrap().unwrap();
                open_segment_bytes(&segment_bytes)
            });
            latencies.push(elapsed_ms);
            get_counts.push(counting.get_count());
            last_hotcache = Some(hotcache);
        }

        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let get_count_per_open = get_counts[0];
        assert!(
            get_counts.iter().all(|&c| c == get_count_per_open),
            "GET count per cold open must be constant across iterations: {get_counts:?}"
        );
        assert!(
            get_count_per_open <= 4,
            "pointer + snapshot + segment open (\u{2264}2 RTT per invariant 3) should be \u{2264}4 GETs, got {get_count_per_open}"
        );

        // Real byte lengths, read back from the real hotcache registry of
        // the real committed-and-reopened segment — not the pre-commit
        // Vec lengths (though they match, since SegmentBuilder writes
        // blobs verbatim; this cross-checks that claim rather than
        // assuming it).
        let hotcache = last_hotcache.expect("at least one iteration ran");
        let descriptor_bytes_len = find_blob_len(&hotcache, BLOB_DESCRIPTOR);
        let navigation_tier_bytes_len = find_blob_len(&hotcache, BLOB_NAVIGATION_TIER);
        let posting_list_bytes_len = find_blob_len(&hotcache, BLOB_POSTING_LISTS);
        let flat_vector_bytes_len = find_blob_len(&hotcache, BLOB_FLAT_VECTORS);
        assert_eq!(descriptor_bytes_len, descriptor_bytes.len() as u64);
        assert_eq!(navigation_tier_bytes_len, navigation_bytes.len() as u64);

        let open_wave_bytes = descriptor_bytes_len + navigation_tier_bytes_len;
        let open_wave_pct_of_budget = open_wave_bytes as f64 / COLD_OPEN_BYTE_BUDGET as f64 * 100.0;

        // Extrapolation to RFC 0010's own 1,000,000-vector napkin-math
        // scale: this run's real per-cluster navigation-tier byte cost
        // (padded_dims*4 f32 centroid bytes + 24-byte cluster_dir entry)
        // times `recommended_cluster_count(1_000_000)`, plus this run's
        // real descriptor byte length and the fixed 8-byte navigation-tier
        // header — a cross-check against, not an independent confirmation
        // of, that RFC's own ≈12.4 MB formula result (both are the same
        // formula; this substitutes this run's actually-built byte
        // constants for the RFC's hand computation).
        let bytes_per_cluster = (DIMS * 4 + 24) as f64;
        let clusters_at_1m = recommended_cluster_count(1_000_000) as f64;
        let extrapolated_open_wave_bytes_at_1m_vectors =
            descriptor_bytes_len as f64 + 8.0 + bytes_per_cluster * clusters_at_1m;

        let result = VectorColdOpenResult {
            iterations: ITERATIONS,
            num_vectors: NUM_VECTORS,
            dims: DIMS,
            padded_dims: DIMS,
            num_clusters,
            bit_width: BIT_WIDTH,
            kmeans_max_iters: KMEANS_MAX_ITERS,
            descriptor_bytes: descriptor_bytes_len,
            navigation_tier_bytes: navigation_tier_bytes_len,
            open_wave_bytes,
            posting_list_bytes_for_reference: posting_list_bytes_len,
            flat_vector_bytes_for_reference: flat_vector_bytes_len,
            segment_total_bytes,
            cold_open_byte_budget: COLD_OPEN_BYTE_BUDGET,
            open_wave_pct_of_budget,
            extrapolated_open_wave_bytes_at_1m_vectors,
            get_count_per_open,
            open_latency_ms_p50: percentile(&latencies, 0.50),
            open_latency_ms_p90: percentile(&latencies, 0.90),
            open_latency_ms_p99: percentile(&latencies, 0.99),
            open_latency_ms_min: latencies[0],
            open_latency_ms_max: latencies[latencies.len() - 1],
        };

        println!(
            "vector cold open: {} vectors, {} clusters, open-wave (descriptor+navigation) = {} bytes \
             ({:.4}% of the {} MB budget), {get_count_per_open} GETs/open, whole-segment-GET latency p50={:.2}ms p90={:.2}ms (n={ITERATIONS})",
            result.num_vectors,
            result.num_clusters,
            result.open_wave_bytes,
            result.open_wave_pct_of_budget,
            result.cold_open_byte_budget / 1_000_000,
            result.open_latency_ms_p50,
            result.open_latency_ms_p90,
        );
        eprintln!(
            "extrapolated open-wave bytes at 1,000,000 vectors (RFC 0010 Napkin math scale): {:.0} \u{2248} {:.1} MB",
            result.extrapolated_open_wave_bytes_at_1m_vectors,
            result.extrapolated_open_wave_bytes_at_1m_vectors / 1_000_000.0
        );

        write_report("vector-cold-open", result);
    });
}
