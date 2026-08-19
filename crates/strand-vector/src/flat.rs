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

//! The flat vectors blob for the vector family: full-precision vectors,
//! fetched only for reranking (`tier: n/a`, invariant 7). Layout is
//! normative per `spec/vectors.md` §5, approved by RFC 0010
//! (`rfcs/0010-vector-blob-cluster-family.md`).

/// Builds a flat-vector blob: `row_id_count * dims * 4` bytes, row-major
/// little-endian f32, one row per local ordinal in row-id order — note
/// `dims`, not `padded_dims` (`spec/vectors.md` §5).
///
/// # Panics
///
/// Panics if `vectors.len() != row_id_count * dims` or if `dims == 0`.
pub fn build_flat_vectors(vectors: &[f32], row_id_count: usize, dims: usize) -> Vec<u8> {
    assert!(dims > 0, "dims must be non-zero");
    assert_eq!(
        vectors.len(),
        row_id_count * dims,
        "vectors must be exactly row_id_count*dims f32 values"
    );
    let mut out = Vec::with_capacity(vectors.len() * 4);
    for &v in vectors {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlatVectorsError {
    Truncated,
}

/// A resident flat-vector blob (`spec/vectors.md` §5).
#[derive(Debug, Clone, Copy)]
pub struct FlatVectorsReader<'a> {
    bytes: &'a [u8],
    dims: usize,
}

impl<'a> FlatVectorsReader<'a> {
    pub fn new(bytes: &'a [u8], dims: usize) -> Result<Self, FlatVectorsError> {
        assert!(dims > 0, "dims must be non-zero");
        if !bytes.len().is_multiple_of(dims * 4) {
            return Err(FlatVectorsError::Truncated);
        }
        Ok(FlatVectorsReader { bytes, dims })
    }

    pub fn row_id_count(&self) -> usize {
        self.bytes.len() / (self.dims * 4)
    }

    /// The `local_ordinal`-th vector's `dims` f32 components.
    ///
    /// # Panics
    ///
    /// Panics if `local_ordinal >= self.row_id_count()`.
    pub fn vector(&self, local_ordinal: usize) -> Vec<f32> {
        assert!(
            local_ordinal < self.row_id_count(),
            "local_ordinal out of range"
        );
        let start = local_ordinal * self.dims * 4;
        (0..self.dims)
            .map(|i| {
                let off = start + i * 4;
                f32::from_le_bytes(self.bytes[off..off + 4].try_into().unwrap())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let dims = 3;
        let vectors: Vec<f32> = vec![1.0, 2.0, 3.0, -1.5, 0.0, 100.25];
        let bytes = build_flat_vectors(&vectors, 2, dims);
        assert_eq!(bytes.len(), 2 * dims * 4);

        let reader = FlatVectorsReader::new(&bytes, dims).expect("valid blob");
        assert_eq!(reader.row_id_count(), 2);
        assert_eq!(reader.vector(0), vec![1.0, 2.0, 3.0]);
        assert_eq!(reader.vector(1), vec![-1.5, 0.0, 100.25]);
    }

    #[test]
    fn rejects_truncated_bytes() {
        assert_eq!(
            FlatVectorsReader::new(&[0u8; 5], 3).unwrap_err(),
            FlatVectorsError::Truncated
        );
    }
}
