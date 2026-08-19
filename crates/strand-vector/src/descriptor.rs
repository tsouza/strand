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

//! The quantization descriptor blob for the vector family: dimensionality,
//! distance metric, bit width, and rotation provenance. Layout is
//! normative per `spec/vectors.md` §2, approved by RFC 0010
//! (`rfcs/0010-vector-blob-cluster-family.md`) and extended by RFC 0011
//! (`rfcs/0011-multibit-extended-rabitq.md`) to widen `bit_width` from a
//! fixed `1` to the range `1..=8`.

use rand::RngCore;

/// Fixed header length; `rotation_payload` follows.
pub const HEADER_LEN: usize = 16;

/// The original v0.1 1-bit-only path (`spec/vectors.md`'s original
/// registration, RFC 0010). `bit_width` values in `2..=8` additionally
/// register an ex-code region (RFC 0011); values above `8` remain
/// unregistered.
pub const BIT_WIDTH: u8 = 1;

/// `bit_width`'s registered range: `1` (RFC 0010, no ex-code region) through
/// `8` (RFC 0011, `ex_bits = bit_width - 1` extra magnitude bits per
/// dimension).
pub const MAX_BIT_WIDTH: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceMetric {
    L2 = 0,
    InnerProduct = 1,
    Cosine = 2,
}

impl DistanceMetric {
    fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(DistanceMetric::L2),
            1 => Some(DistanceMetric::InnerProduct),
            2 => Some(DistanceMetric::Cosine),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotatorType {
    /// Registered, non-default (`spec/vectors.md` §2.1). `crate::
    /// orthogonal::generate_matrix_rotation` generates the realized
    /// orthogonal matrix (random Gaussian sampling plus Householder QR);
    /// [`build_matrix_generated`] wraps generation and serialization in
    /// one call, or [`build_matrix`] accepts an already-computed matrix.
    Matrix = 0,
    /// The registered default (`spec/vectors.md` §2.1).
    FhtKac = 1,
}

impl RotatorType {
    fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(RotatorType::Matrix),
            1 => Some(RotatorType::FhtKac),
            _ => None,
        }
    }
}

/// `dims` rounded up to the nearest multiple of 64 — the shared padding
/// rule both registered rotator types use (`spec/vectors.md` §2.1; for
/// `MatrixRotator` this is a STRAND-specific requirement, not the
/// reference implementation's own unpadded convention for that type).
pub fn padded_dims_for(dims: u32) -> u32 {
    dims.div_ceil(64) * 64
}

fn rotation_payload_len(dims: u32, padded_dims: u32, rotator_type: RotatorType) -> u32 {
    match rotator_type {
        RotatorType::FhtKac => 4 * padded_dims / 8,
        RotatorType::Matrix => dims * padded_dims * 4,
    }
}

#[allow(clippy::too_many_arguments)]
fn encode(
    dims: u32,
    padded_dims: u32,
    distance_metric: DistanceMetric,
    bit_width: u8,
    rotator_type: RotatorType,
    rotation_payload: &[u8],
) -> Vec<u8> {
    let expected_len = rotation_payload_len(dims, padded_dims, rotator_type);
    assert_eq!(
        rotation_payload.len() as u32,
        expected_len,
        "rotation_payload must be exactly the formula-computed length"
    );
    assert!(
        (1..=MAX_BIT_WIDTH).contains(&bit_width),
        "bit_width must be in 1..={MAX_BIT_WIDTH}"
    );
    let mut out = Vec::with_capacity(HEADER_LEN + rotation_payload.len());
    out.extend_from_slice(&dims.to_le_bytes());
    out.extend_from_slice(&padded_dims.to_le_bytes());
    out.push(distance_metric as u8);
    out.push(bit_width);
    out.push(rotator_type as u8);
    out.push(0); // reserved
    out.extend_from_slice(&(rotation_payload.len() as u32).to_le_bytes());
    out.extend_from_slice(rotation_payload);
    out
}

/// Builds a quantization descriptor blob with `FhtKacRotator` (the
/// registered default, `spec/vectors.md` §2.1): `padded_dims` is `dims`
/// rounded up to a multiple of 64, and the rotation payload is
/// `4 * padded_dims / 8` real random sign bytes, drawn from `rng`.
///
/// # Panics
///
/// Panics if `bit_width` is not in `1..=8` (RFC 0011).
pub fn build_fht_kac(
    dims: u32,
    distance_metric: DistanceMetric,
    bit_width: u8,
    rng: &mut impl RngCore,
) -> Vec<u8> {
    let padded_dims = padded_dims_for(dims);
    let payload_len = rotation_payload_len(dims, padded_dims, RotatorType::FhtKac) as usize;
    let mut rotation_payload = vec![0u8; payload_len];
    rng.fill_bytes(&mut rotation_payload);
    encode(
        dims,
        padded_dims,
        distance_metric,
        bit_width,
        RotatorType::FhtKac,
        &rotation_payload,
    )
}

/// Builds an `FhtKacRotator` descriptor with caller-supplied sign bytes
/// rather than drawing them from an RNG — for reconstructing a previously-
/// realized rotation (e.g. at merge time, `spec/vectors.md` §7) or for
/// golden-file/worked-example testing against a fixed byte sequence.
///
/// # Panics
///
/// Panics if `rotation_payload.len() != 4 * padded_dims_for(dims) / 8`, or
/// if `bit_width` is not in `1..=8` (RFC 0011).
pub fn build_fht_kac_with_payload(
    dims: u32,
    distance_metric: DistanceMetric,
    bit_width: u8,
    rotation_payload: &[u8],
) -> Vec<u8> {
    let padded_dims = padded_dims_for(dims);
    encode(
        dims,
        padded_dims,
        distance_metric,
        bit_width,
        RotatorType::FhtKac,
        rotation_payload,
    )
}

/// Builds a quantization descriptor blob with `MatrixRotator` (registered,
/// non-default, `spec/vectors.md` §2.1) from a caller-supplied, already
/// realized `dims × padded_dims` row-major f32 orthogonal matrix — for a
/// writer that generated (or loaded, at merge time) the matrix itself.
/// See [`build_matrix_generated`] to generate a fresh one in the same call.
///
/// # Panics
///
/// Panics if `rotation_payload.len() != dims * padded_dims_for(dims) * 4`,
/// or if `bit_width` is not in `1..=8` (RFC 0011).
pub fn build_matrix(
    dims: u32,
    distance_metric: DistanceMetric,
    bit_width: u8,
    rotation_payload: &[u8],
) -> Vec<u8> {
    let padded_dims = padded_dims_for(dims);
    encode(
        dims,
        padded_dims,
        distance_metric,
        bit_width,
        RotatorType::Matrix,
        rotation_payload,
    )
}

/// Builds a `MatrixRotator` descriptor blob with a **freshly generated**
/// rotation matrix (`crate::orthogonal::generate_matrix_rotation` —
/// random Gaussian sampling plus Householder QR orthogonalization,
/// matching the reference implementation's own algorithm), serialized in
/// one call — the `MatrixRotator` counterpart to [`build_fht_kac`].
///
/// # Panics
///
/// Panics if `bit_width` is not in `1..=8` (RFC 0011).
pub fn build_matrix_generated(
    dims: u32,
    distance_metric: DistanceMetric,
    bit_width: u8,
    rng: &mut impl rand::Rng,
) -> Vec<u8> {
    let padded_dims = padded_dims_for(dims);
    let matrix =
        crate::orthogonal::generate_matrix_rotation(dims as usize, padded_dims as usize, rng);
    let mut payload_bytes = Vec::with_capacity(matrix.len() * 4);
    for v in &matrix {
        payload_bytes.extend_from_slice(&v.to_le_bytes());
    }
    encode(
        dims,
        padded_dims,
        distance_metric,
        bit_width,
        RotatorType::Matrix,
        &payload_bytes,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorError {
    Truncated,
    UnknownDistanceMetric(u8),
    UnsupportedBitWidth(u8),
    UnknownRotatorType(u8),
    RotationPayloadLengthMismatch { declared: u32, computed: u32 },
}

/// A resident quantization descriptor blob (`spec/vectors.md` §2).
#[derive(Debug, Clone, Copy)]
pub struct DescriptorReader<'a> {
    bytes: &'a [u8],
}

impl<'a> DescriptorReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, DescriptorError> {
        if bytes.len() < HEADER_LEN {
            return Err(DescriptorError::Truncated);
        }
        let reader = DescriptorReader { bytes };
        let bit_width = reader.bit_width();
        if !(1..=MAX_BIT_WIDTH).contains(&bit_width) {
            return Err(DescriptorError::UnsupportedBitWidth(bit_width));
        }
        DistanceMetric::from_u8(bytes[8])
            .ok_or(DescriptorError::UnknownDistanceMetric(bytes[8]))?;
        let rotator_type = RotatorType::from_u8(bytes[10])
            .ok_or(DescriptorError::UnknownRotatorType(bytes[10]))?;
        let declared = reader.rotation_payload_length_field();
        let computed = rotation_payload_len(reader.dims(), reader.padded_dims(), rotator_type);
        if declared != computed {
            return Err(DescriptorError::RotationPayloadLengthMismatch { declared, computed });
        }
        if bytes.len() < HEADER_LEN + declared as usize {
            return Err(DescriptorError::Truncated);
        }
        Ok(reader)
    }

    pub fn dims(&self) -> u32 {
        u32::from_le_bytes(self.bytes[0..4].try_into().unwrap())
    }

    pub fn padded_dims(&self) -> u32 {
        u32::from_le_bytes(self.bytes[4..8].try_into().unwrap())
    }

    pub fn distance_metric(&self) -> DistanceMetric {
        DistanceMetric::from_u8(self.bytes[8]).expect("validated in new()")
    }

    pub fn bit_width(&self) -> u8 {
        self.bytes[9]
    }

    pub fn rotator_type(&self) -> RotatorType {
        RotatorType::from_u8(self.bytes[10]).expect("validated in new()")
    }

    fn rotation_payload_length_field(&self) -> u32 {
        u32::from_le_bytes(self.bytes[12..16].try_into().unwrap())
    }

    pub fn rotation_payload(&self) -> &'a [u8] {
        let len = self.rotation_payload_length_field() as usize;
        &self.bytes[HEADER_LEN..HEADER_LEN + len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn fht_kac_round_trips() {
        let mut rng = StdRng::seed_from_u64(42);
        let bytes = build_fht_kac(768, DistanceMetric::L2, 1, &mut rng);
        let reader = DescriptorReader::new(&bytes).expect("valid descriptor");
        assert_eq!(reader.dims(), 768);
        assert_eq!(reader.padded_dims(), 768);
        assert_eq!(reader.distance_metric(), DistanceMetric::L2);
        assert_eq!(reader.bit_width(), 1);
        assert_eq!(reader.rotator_type(), RotatorType::FhtKac);
        assert_eq!(reader.rotation_payload().len(), 384);
    }

    #[test]
    fn padding_rounds_up_to_multiple_of_64() {
        assert_eq!(padded_dims_for(1), 64);
        assert_eq!(padded_dims_for(64), 64);
        assert_eq!(padded_dims_for(65), 128);
        assert_eq!(padded_dims_for(768), 768);
        assert_eq!(padded_dims_for(1000), 1024);
    }

    #[test]
    fn matrix_round_trips() {
        let dims = 4u32;
        let padded_dims = padded_dims_for(dims);
        let payload = vec![0u8; (dims * padded_dims * 4) as usize];
        let bytes = build_matrix(dims, DistanceMetric::Cosine, 1, &payload);
        let reader = DescriptorReader::new(&bytes).expect("valid descriptor");
        assert_eq!(reader.rotator_type(), RotatorType::Matrix);
        assert_eq!(reader.distance_metric(), DistanceMetric::Cosine);
        assert_eq!(reader.rotation_payload(), &payload[..]);
    }

    #[test]
    fn matrix_generated_produces_a_real_orthogonal_matrix() {
        let dims = 5u32;
        let padded_dims = padded_dims_for(dims);
        let mut rng = StdRng::seed_from_u64(9);
        let bytes = build_matrix_generated(dims, DistanceMetric::L2, 1, &mut rng);

        let reader = DescriptorReader::new(&bytes).expect("valid descriptor");
        assert_eq!(reader.rotator_type(), RotatorType::Matrix);
        assert_eq!(reader.dims(), dims);
        assert_eq!(reader.padded_dims(), padded_dims);
        let payload = reader.rotation_payload();
        assert_eq!(payload.len(), (dims * padded_dims * 4) as usize);

        // Real orthonormality check, not just a length check: every row
        // is unit norm and mutually orthogonal (crate::orthogonal's own
        // deeper QR-property tests cover the decomposition itself; this
        // confirms the bytes round-tripped through serialization still
        // carry that property).
        let rows: Vec<Vec<f32>> = (0..dims as usize)
            .map(|r| {
                (0..padded_dims as usize)
                    .map(|c| {
                        let off = (r * padded_dims as usize + c) * 4;
                        f32::from_le_bytes(payload[off..off + 4].try_into().unwrap())
                    })
                    .collect()
            })
            .collect();
        for (i, row_i) in rows.iter().enumerate() {
            let norm_sqr: f32 = row_i.iter().map(|v| v * v).sum();
            assert!(
                (norm_sqr - 1.0).abs() < 1e-3,
                "row {i} not unit norm: {norm_sqr}"
            );
            for row_j in &rows[i + 1..] {
                let dot: f32 = row_i.iter().zip(row_j).map(|(&a, &b)| a * b).sum();
                assert!(dot.abs() < 1e-3, "rows not orthogonal: dot={dot}");
            }
        }
    }

    #[test]
    fn rejects_a_mismatched_rotation_payload_length() {
        let mut bytes = {
            let mut rng = StdRng::seed_from_u64(1);
            build_fht_kac(64, DistanceMetric::L2, 1, &mut rng)
        };
        // Corrupt the declared length field (offset 12..16).
        bytes[12] = 0xFF;
        let err = DescriptorReader::new(&bytes).unwrap_err();
        assert!(matches!(
            err,
            DescriptorError::RotationPayloadLengthMismatch { .. }
        ));
    }

    #[test]
    fn rejects_truncated_bytes() {
        assert_eq!(
            DescriptorReader::new(&[0u8; 4]).unwrap_err(),
            DescriptorError::Truncated
        );
    }
}
