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

//! Cross-segment codebook identity and a cheap pre-merge compatibility
//! check (`docs/roadmap.md` M2-8; RFC 0010 Design §7, "How this could be
//! wrong" item 4, Open questions; `rfcs/0010-vector-blob-cluster-family.md`
//! Discussion — post-approval amendments).
//!
//! RFC 0010 Design §7 already states the merge criterion in prose: two
//! segments' cluster posting lists are eligible for `concatenate + remap`
//! only when their quantization descriptors are **byte-identical** (same
//! `dims`, `distance_metric`, `bit_width`, `rotator_type`, and
//! `rotation_payload`); otherwise the codes are not comparable and the
//! merge strategy degrades to `rebuild` (invariant 1's vocabulary,
//! `CLAUDE.md` §5). What RFC 0010 left open was a way to *check* that
//! cheaply, before attempting a merge, without a full byte-for-byte
//! comparison of two potentially multi-megabyte `rotation_payload`s
//! (`MatrixRotator`'s payload is `dims * padded_dims * 4` bytes — 2.36 MB
//! at `dims = padded_dims = 768`) on every pair a merge planner considers.
//!
//! [`CodebookIdentity`] answers that: a small, fixed-size summary of a
//! segment's quantization descriptor, computed once per segment (`O(n)` in
//! `rotation_payload`'s length — the same descriptor bytes a reader already
//! fetched wholesale as part of the cold-open wave, invariant 7, to *use*
//! the codebook at all, so this adds no new I/O), after which
//! [`check_compatibility`] compares two identities in `O(1)` — a handful of
//! fixed-size scalar equality checks — touching neither segment's full
//! `rotation_payload` again. This closes only the codebook-identity half of
//! RFC 0010 Design §7's `concatenate + remap` precondition: cluster-
//! assignment compatibility with a merged, rebalanced navigation tier is
//! separate, still-open work (RFC 0010 Non-goals; `docs/roadmap.md` M3-1),
//! and this module does not attempt it.

use crate::descriptor::DescriptorReader;

/// A small, fixed-size summary of a quantization descriptor
/// (`crate::descriptor`), cheap to compare for compatibility without
/// touching either segment's full `rotation_payload` again once built.
///
/// `content_hash` is XxHash3-64 — `spec/container.md`'s own registered
/// default checksum algorithm (invariant 11), reused here rather than
/// introducing a second hash algorithm into the project's vocabulary
/// (invariant 8's novelty-budget discipline) — computed over exactly the
/// fields RFC 0010 Design §7 names as the byte-identity criterion: `dims`
/// (little-endian u32), `distance_metric`, `bit_width`, `rotator_type` (one
/// byte each), then `rotation_payload` verbatim, in that order. Two fields
/// the wire descriptor also carries are deliberately excluded: `padded_dims`
/// (fully determined by `dims` and the shared 64-multiple padding rule both
/// registered rotator types use — `crate::descriptor::padded_dims_for` —
/// so hashing it would be redundant with `dims`) and the reserved byte at
/// offset 11 (`spec/vectors.md` §2's own normative text: "reader MUST NOT
/// interpret" it — a byte this project has promised never carries meaning,
/// so folding it into a compatibility identity would be a real, avoidable
/// way for two semantically identical descriptors to be judged
/// incompatible over bytes neither writer nor reader is allowed to give
/// meaning to).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodebookIdentity {
    /// True (unpadded) vector dimensionality.
    pub dims: u32,
    /// `DistanceMetric` discriminant (`crate::descriptor::DistanceMetric`).
    pub distance_metric: u8,
    /// RaBitQ bit width, `1..=8` (RFC 0010 / RFC 0011).
    pub bit_width: u8,
    /// `RotatorType` discriminant (`crate::descriptor::RotatorType`).
    pub rotator_type: u8,
    /// XxHash3-64 over `dims || distance_metric || bit_width ||
    /// rotator_type || rotation_payload`, each field as its wire-format
    /// bytes, in that order.
    pub content_hash: u64,
}

impl CodebookIdentity {
    /// Computes the identity of a resident quantization descriptor. `O(n)`
    /// in `rotation_payload`'s length — the same descriptor bytes a reader
    /// already fetched wholesale as part of the cold-open wave (invariant
    /// 7) to use the codebook at all; this adds no new fetch, only a hash
    /// pass over bytes already in hand. A merge planner computes this once
    /// per segment and reuses the result for every pairwise comparison,
    /// rather than re-hashing (or re-comparing raw bytes) on every pair.
    pub fn from_descriptor(reader: &DescriptorReader<'_>) -> Self {
        let payload = reader.rotation_payload();
        let mut canonical = Vec::with_capacity(4 + 1 + 1 + 1 + payload.len());
        canonical.extend_from_slice(&reader.dims().to_le_bytes());
        canonical.push(reader.distance_metric() as u8);
        canonical.push(reader.bit_width());
        canonical.push(reader.rotator_type() as u8);
        canonical.extend_from_slice(payload);
        let content_hash = twox_hash::XxHash3_64::oneshot(&canonical);
        CodebookIdentity {
            dims: reader.dims(),
            distance_metric: reader.distance_metric() as u8,
            bit_width: reader.bit_width(),
            rotator_type: reader.rotator_type() as u8,
            content_hash,
        }
    }
}

/// Why two codebooks are, or are not, compatible for a cheap posting-list
/// merge, per RFC 0010 Design §7's own byte-identical-descriptor criterion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodebookCompatibility {
    /// The two descriptors are byte-identical over every field RFC 0010
    /// Design §7 names. **Necessary, not sufficient**, for
    /// `concatenate + remap`: a merge planner must still confirm the two
    /// segments' cluster assignments are compatible with the merged
    /// navigation tier's rebalanced centroids (RFC 0010 Design §7's own
    /// second clause) before actually choosing that strategy over
    /// `rebuild` — real, separate work this check does not attempt
    /// (`docs/roadmap.md` M3-1).
    Compatible,
    /// The two descriptors differ in a way that makes their quantized
    /// codes not directly comparable; merging these segments' posting
    /// lists requires `rebuild` (full requantization against one shared
    /// codebook), not `concatenate + remap`.
    Incompatible(CodebookMismatch),
}

/// The field (checked in this fixed order) two codebooks were first found
/// to disagree on. Cheap fields are compared first so an obviously
/// incompatible pair (different dimensionality, most commonly) is rejected
/// without ever needing `content_hash` — which is itself already `O(1)` to
/// compare once computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodebookMismatch {
    /// Different vector dimensionality — codes are different lengths and
    /// cannot be concatenated at all.
    Dims { a: u32, b: u32 },
    /// Different distance metric — distances are not comparable across
    /// segments even if code layout happened to match.
    DistanceMetric { a: u8, b: u8 },
    /// Different RaBitQ bit width — different code layout entirely
    /// (RFC 0011's ex-code region only exists for `bit_width > 1`).
    BitWidth { a: u8, b: u8 },
    /// Different rotator type (`MatrixRotator` vs `FhtKacRotator`) —
    /// vectors were rotated by structurally different transforms.
    RotatorType { a: u8, b: u8 },
    /// Every scalar field matched, but the realized rotation state itself
    /// differs — two segments trained (or re-trained) their codebook
    /// independently, e.g. two independent k-means-plus-fresh-rotation runs
    /// with identical configuration. This is the case a bare
    /// dims/bit_width/rotator_type check would miss: the scalars can be
    /// identical while the actual rotation differs, silently corrupting
    /// distance estimates under a naive concatenation (RFC 0010 Design §7).
    ContentHash { a: u64, b: u64 },
}

/// Checks whether two codebooks are compatible for a cheap
/// `concatenate + remap` posting-list merge, per RFC 0010 Design §7.
/// `O(1)`: every field on [`CodebookIdentity`] is fixed-size, so this does
/// no I/O and touches neither segment's `rotation_payload` again.
pub fn check_compatibility(a: &CodebookIdentity, b: &CodebookIdentity) -> CodebookCompatibility {
    if a.dims != b.dims {
        return CodebookCompatibility::Incompatible(CodebookMismatch::Dims {
            a: a.dims,
            b: b.dims,
        });
    }
    if a.distance_metric != b.distance_metric {
        return CodebookCompatibility::Incompatible(CodebookMismatch::DistanceMetric {
            a: a.distance_metric,
            b: b.distance_metric,
        });
    }
    if a.bit_width != b.bit_width {
        return CodebookCompatibility::Incompatible(CodebookMismatch::BitWidth {
            a: a.bit_width,
            b: b.bit_width,
        });
    }
    if a.rotator_type != b.rotator_type {
        return CodebookCompatibility::Incompatible(CodebookMismatch::RotatorType {
            a: a.rotator_type,
            b: b.rotator_type,
        });
    }
    if a.content_hash != b.content_hash {
        return CodebookCompatibility::Incompatible(CodebookMismatch::ContentHash {
            a: a.content_hash,
            b: b.content_hash,
        });
    }
    CodebookCompatibility::Compatible
}

/// Convenience wrapper: builds both identities and checks them in one call
/// — the shape a merge planner touching two segments' resident descriptor
/// blobs for the first time actually calls.
pub fn check_descriptor_compatibility(
    a: &DescriptorReader<'_>,
    b: &DescriptorReader<'_>,
) -> CodebookCompatibility {
    check_compatibility(
        &CodebookIdentity::from_descriptor(a),
        &CodebookIdentity::from_descriptor(b),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::{DistanceMetric, build_fht_kac_with_payload, build_matrix};

    fn payload(dims: u32, seed_byte: u8) -> Vec<u8> {
        let padded = crate::descriptor::padded_dims_for(dims);
        let len = (4 * padded / 8) as usize;
        (0..len).map(|i| seed_byte.wrapping_add(i as u8)).collect()
    }

    #[test]
    fn identical_descriptors_are_compatible() {
        let p = payload(768, 1);
        let a = build_fht_kac_with_payload(768, DistanceMetric::L2, 1, &p);
        let b = build_fht_kac_with_payload(768, DistanceMetric::L2, 1, &p);
        let ra = DescriptorReader::new(&a).unwrap();
        let rb = DescriptorReader::new(&b).unwrap();
        assert_eq!(
            check_descriptor_compatibility(&ra, &rb),
            CodebookCompatibility::Compatible
        );
    }

    #[test]
    fn same_scalars_different_rotation_payload_is_incompatible() {
        // The case a bare dims/bit_width/rotator_type check would miss:
        // two independently trained codebooks with identical configuration
        // but genuinely different realized rotation state.
        let a = build_fht_kac_with_payload(768, DistanceMetric::L2, 1, &payload(768, 1));
        let b = build_fht_kac_with_payload(768, DistanceMetric::L2, 1, &payload(768, 2));
        let ra = DescriptorReader::new(&a).unwrap();
        let rb = DescriptorReader::new(&b).unwrap();
        assert!(matches!(
            check_descriptor_compatibility(&ra, &rb),
            CodebookCompatibility::Incompatible(CodebookMismatch::ContentHash { .. })
        ));
    }

    #[test]
    fn different_dims_is_incompatible() {
        let a = build_fht_kac_with_payload(768, DistanceMetric::L2, 1, &payload(768, 1));
        let b = build_fht_kac_with_payload(512, DistanceMetric::L2, 1, &payload(512, 1));
        let ra = DescriptorReader::new(&a).unwrap();
        let rb = DescriptorReader::new(&b).unwrap();
        assert_eq!(
            check_descriptor_compatibility(&ra, &rb),
            CodebookCompatibility::Incompatible(CodebookMismatch::Dims { a: 768, b: 512 })
        );
    }

    #[test]
    fn different_distance_metric_is_incompatible() {
        let p = payload(64, 3);
        let a = build_fht_kac_with_payload(64, DistanceMetric::L2, 1, &p);
        let b = build_fht_kac_with_payload(64, DistanceMetric::Cosine, 1, &p);
        let ra = DescriptorReader::new(&a).unwrap();
        let rb = DescriptorReader::new(&b).unwrap();
        assert_eq!(
            check_descriptor_compatibility(&ra, &rb),
            CodebookCompatibility::Incompatible(CodebookMismatch::DistanceMetric { a: 0, b: 2 })
        );
    }

    #[test]
    fn different_bit_width_is_incompatible() {
        let p = payload(64, 4);
        let a = build_fht_kac_with_payload(64, DistanceMetric::L2, 1, &p);
        let b = build_fht_kac_with_payload(64, DistanceMetric::L2, 4, &p);
        let ra = DescriptorReader::new(&a).unwrap();
        let rb = DescriptorReader::new(&b).unwrap();
        assert_eq!(
            check_descriptor_compatibility(&ra, &rb),
            CodebookCompatibility::Incompatible(CodebookMismatch::BitWidth { a: 1, b: 4 })
        );
    }

    #[test]
    fn different_rotator_type_is_incompatible_even_with_matching_scalars() {
        let dims = 64u32;
        let padded = crate::descriptor::padded_dims_for(dims);
        let fht_payload = payload(dims, 5);
        let matrix_payload = vec![0u8; (dims * padded * 4) as usize];
        let a = build_fht_kac_with_payload(dims, DistanceMetric::L2, 1, &fht_payload);
        let b = build_matrix(dims, DistanceMetric::L2, 1, &matrix_payload);
        let ra = DescriptorReader::new(&a).unwrap();
        let rb = DescriptorReader::new(&b).unwrap();
        assert_eq!(
            check_descriptor_compatibility(&ra, &rb),
            CodebookCompatibility::Incompatible(CodebookMismatch::RotatorType { a: 1, b: 0 })
        );
    }

    #[test]
    fn identity_is_reusable_across_many_pairwise_comparisons() {
        // The shape a real merge planner uses: compute each segment's
        // identity once, then compare pairs cheaply with no further access
        // to rotation_payload — exercised directly here rather than only
        // through the convenience wrapper.
        let p = payload(768, 9);
        let a = build_fht_kac_with_payload(768, DistanceMetric::L2, 1, &p);
        let b = build_fht_kac_with_payload(768, DistanceMetric::L2, 1, &p);
        let c = build_fht_kac_with_payload(768, DistanceMetric::L2, 1, &payload(768, 10));
        let ra = DescriptorReader::new(&a).unwrap();
        let rb = DescriptorReader::new(&b).unwrap();
        let rc = DescriptorReader::new(&c).unwrap();

        let ia = CodebookIdentity::from_descriptor(&ra);
        let ib = CodebookIdentity::from_descriptor(&rb);
        let ic = CodebookIdentity::from_descriptor(&rc);

        assert_eq!(
            check_compatibility(&ia, &ib),
            CodebookCompatibility::Compatible
        );
        assert!(matches!(
            check_compatibility(&ia, &ic),
            CodebookCompatibility::Incompatible(CodebookMismatch::ContentHash { .. })
        ));
        assert!(matches!(
            check_compatibility(&ib, &ic),
            CodebookCompatibility::Incompatible(CodebookMismatch::ContentHash { .. })
        ));
    }
}
