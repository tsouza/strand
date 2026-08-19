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

//! M2-8 (`docs/roadmap.md`): builds real, independently-committed segments
//! — each a real footer/hotcache-decodable STRAND segment produced by
//! `strand-core`'s actual `SegmentBuilder`, not a bare byte array — with
//! deliberately compatible and deliberately incompatible codebooks, and
//! proves `strand_vector::codebook::check_descriptor_compatibility`
//! distinguishes them correctly by reading each segment's descriptor blob
//! back out through the real cold-open path (footer -> hotcache -> blob
//! registry), the same way a merge planner would touch two real segments
//! it is considering for a `concatenate + remap` merge (RFC 0010 Design
//! §7).

use rand::SeedableRng;
use rand::rngs::StdRng;
use strand_core::container::{ChunkCodec, Footer, Hotcache, StorageClass, Tier};
use strand_core::segment::{BlobSpec, SegmentBuilder};
use strand_vector::codebook::{
    CodebookCompatibility, CodebookMismatch, check_descriptor_compatibility,
};
use strand_vector::descriptor::{self, DescriptorReader, DistanceMetric};

const FAMILY_ID: u16 = 3;
const BLOB_DESCRIPTOR: u16 = 1;

fn open_segment_bytes(bytes: &[u8]) -> Hotcache {
    let footer_bytes: [u8; 40] = bytes[bytes.len() - 40..].try_into().unwrap();
    let footer = Footer::decode(&footer_bytes).expect("valid footer");
    let start = footer.hotcache_offset as usize;
    let end = start + footer.hotcache_length as usize;
    Hotcache::decode(&bytes[start..end]).expect("valid hotcache")
}

/// Builds a real, single-blob (descriptor-only) segment — everything a
/// pre-merge codebook compatibility check actually needs to touch, per
/// RFC 0010 Design §7's own criterion, without paying for a full
/// navigation-tier/posting-list/flat-vector assembly this check never
/// reads.
fn build_descriptor_only_segment(descriptor_bytes: &[u8]) -> Vec<u8> {
    let mut builder = SegmentBuilder::new(0);
    builder.add_blob(BlobSpec {
        family_id: FAMILY_ID,
        field_id: 0,
        blob_type_id: BLOB_DESCRIPTOR,
        storage_class: StorageClass::RawMappable,
        tier: Tier::ColdFetchable,
        alignment: 8,
        chunk_codec: ChunkCodec::None,
        chunk_codec_level: 0,
        data: descriptor_bytes.to_vec(),
    });
    builder.build(0)
}

/// Extracts the descriptor blob's bytes back out of a real committed
/// segment via the real footer/hotcache decode path (invariant 3) — the
/// same bytes a cold-open reader (or a merge planner) would have resident
/// after fetching the descriptor blob's own byte range.
fn descriptor_bytes_from_segment(segment_bytes: &[u8]) -> &[u8] {
    let hotcache = open_segment_bytes(segment_bytes);
    let entry = hotcache
        .blobs
        .iter()
        .find(|b| b.family_id == FAMILY_ID && b.blob_type_id == BLOB_DESCRIPTOR)
        .expect("descriptor blob present in registry");
    &segment_bytes[entry.offset as usize..(entry.offset + entry.length) as usize]
}

#[test]
fn two_segments_sharing_one_real_codebook_are_compatible() {
    // Simulates a real writer that trains one codebook and reuses it
    // across two segment-build runs (e.g. two batches committed against
    // the same table, sharing one previously-fitted rotation) — the
    // easy case RFC 0010 Design §7 already names as eligible for
    // `concatenate + remap`.
    let mut rng = StdRng::seed_from_u64(101);
    let shared_descriptor = descriptor::build_fht_kac(768, DistanceMetric::L2, 1, &mut rng);

    let segment_a = build_descriptor_only_segment(&shared_descriptor);
    let segment_b = build_descriptor_only_segment(&shared_descriptor);

    let bytes_a = descriptor_bytes_from_segment(&segment_a);
    let bytes_b = descriptor_bytes_from_segment(&segment_b);
    // Real, independent segments — not the same Vec reused by reference.
    assert_ne!(
        segment_a.as_ptr(),
        segment_b.as_ptr(),
        "must be two independently built segments, not one segment inspected twice"
    );
    assert_eq!(
        bytes_a, bytes_b,
        "both segments carry the same codebook bytes"
    );

    let reader_a = DescriptorReader::new(bytes_a).expect("valid descriptor");
    let reader_b = DescriptorReader::new(bytes_b).expect("valid descriptor");

    assert_eq!(
        check_descriptor_compatibility(&reader_a, &reader_b),
        CodebookCompatibility::Compatible,
        "two segments built from one shared, real codebook must be judged compatible"
    );
}

#[test]
fn two_segments_with_independently_trained_codebooks_are_incompatible() {
    // Simulates the real failure case RFC 0010 Design §7 and "How this
    // could be wrong" item 4 name directly: a writer that retrains its
    // codebook per segment (plausible, since RaBitQ's rotation is cheap
    // to regenerate) ends up with two segments whose descriptors share
    // every scalar field (dims, distance metric, bit width, rotator type)
    // but carry genuinely different realized rotation state — the case a
    // bare scalar-field check would miss.
    let mut rng_a = StdRng::seed_from_u64(101);
    let mut rng_b = StdRng::seed_from_u64(202);
    let descriptor_a = descriptor::build_fht_kac(768, DistanceMetric::L2, 1, &mut rng_a);
    let descriptor_b = descriptor::build_fht_kac(768, DistanceMetric::L2, 1, &mut rng_b);
    assert_ne!(
        descriptor_a, descriptor_b,
        "two independent RNG draws must actually produce different rotation payloads \
         for this test to exercise the case it claims to"
    );

    let segment_a = build_descriptor_only_segment(&descriptor_a);
    let segment_b = build_descriptor_only_segment(&descriptor_b);

    let bytes_a = descriptor_bytes_from_segment(&segment_a);
    let bytes_b = descriptor_bytes_from_segment(&segment_b);
    let reader_a = DescriptorReader::new(bytes_a).expect("valid descriptor");
    let reader_b = DescriptorReader::new(bytes_b).expect("valid descriptor");

    // Cheap scalar fields agree — this is exactly the "same knobs,
    // different training run" scenario, so only the content hash can
    // catch it.
    assert_eq!(reader_a.dims(), reader_b.dims());
    assert_eq!(reader_a.bit_width(), reader_b.bit_width());
    assert_eq!(reader_a.rotator_type(), reader_b.rotator_type());
    assert_eq!(reader_a.distance_metric(), reader_b.distance_metric());

    match check_descriptor_compatibility(&reader_a, &reader_b) {
        CodebookCompatibility::Incompatible(CodebookMismatch::ContentHash { a, b }) => {
            assert_ne!(a, b);
        }
        other => panic!(
            "expected Incompatible(ContentHash), got {other:?} — two independently \
             trained real codebooks with matching scalars must still be caught"
        ),
    }
}

#[test]
fn two_segments_with_structurally_different_codebooks_are_incompatible() {
    // A coarser real mismatch: different dimensionality entirely — two
    // segments built against different embedding models, for instance.
    // This is the cheap short-circuit case: caught from the 16-byte
    // descriptor header alone, before `content_hash` is even relevant.
    let mut rng = StdRng::seed_from_u64(303);
    let descriptor_768 = descriptor::build_fht_kac(768, DistanceMetric::L2, 1, &mut rng);
    let descriptor_384 = descriptor::build_fht_kac(384, DistanceMetric::L2, 1, &mut rng);

    let segment_a = build_descriptor_only_segment(&descriptor_768);
    let segment_b = build_descriptor_only_segment(&descriptor_384);

    let bytes_a = descriptor_bytes_from_segment(&segment_a);
    let bytes_b = descriptor_bytes_from_segment(&segment_b);
    let reader_a = DescriptorReader::new(bytes_a).expect("valid descriptor");
    let reader_b = DescriptorReader::new(bytes_b).expect("valid descriptor");

    assert_eq!(
        check_descriptor_compatibility(&reader_a, &reader_b),
        CodebookCompatibility::Incompatible(CodebookMismatch::Dims { a: 768, b: 384 })
    );
}
