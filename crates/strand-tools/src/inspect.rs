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

//! `strand-tools inspect`: decodes a segment's footer and hotcache and
//! formats a human-readable report. Pure logic, kept separate from `main.rs`
//! so it's testable without spawning the binary.

use std::fmt::Write as _;
use strand_core::container::{ChunkCodec, DecodeError, Footer, Hotcache, StorageClass, Tier};

const FOOTER_SIZE: usize = 40;

#[derive(Debug)]
pub enum InspectError {
    /// Fewer bytes than a footer trailer alone requires.
    TooShort,
    Footer(DecodeError),
    Hotcache(DecodeError),
}

impl std::fmt::Display for InspectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InspectError::TooShort => {
                write!(
                    f,
                    "file is shorter than a {FOOTER_SIZE}-byte footer trailer"
                )
            }
            InspectError::Footer(e) => write!(f, "invalid footer: {e:?}"),
            InspectError::Hotcache(e) => write!(f, "invalid hotcache: {e:?}"),
        }
    }
}

fn storage_class_label(s: StorageClass) -> &'static str {
    match s {
        StorageClass::ChunkCompressed => "chunk-compressed",
        StorageClass::RawMappable => "raw-mappable",
    }
}

fn tier_label(t: Tier) -> &'static str {
    match t {
        Tier::NotApplicable => "n/a",
        Tier::ColdFetchable => "cold-fetchable",
        Tier::Warm => "warm",
    }
}

fn chunk_codec_label(c: ChunkCodec) -> &'static str {
    match c {
        ChunkCodec::None => "none",
        ChunkCodec::Zstd => "zstd",
    }
}

/// Decodes `bytes` as a segment and formats a human-readable report, per
/// RFC 0001 §1's footer/hotcache layout.
pub fn format_report(bytes: &[u8]) -> Result<String, InspectError> {
    if bytes.len() < FOOTER_SIZE {
        return Err(InspectError::TooShort);
    }
    let footer_bytes: [u8; FOOTER_SIZE] = bytes[bytes.len() - FOOTER_SIZE..].try_into().unwrap();
    let footer = Footer::decode(&footer_bytes).map_err(InspectError::Footer)?;

    let mut report = String::new();
    writeln!(report, "file size: {} bytes", bytes.len()).unwrap();
    writeln!(
        report,
        "format version: {}.{}",
        footer.format_major, footer.format_minor
    )
    .unwrap();
    writeln!(report, "footer: ok (checksum verified)").unwrap();
    writeln!(
        report,
        "hotcache: offset={} length={}",
        footer.hotcache_offset, footer.hotcache_length
    )
    .unwrap();

    let hotcache_start = footer.hotcache_offset as usize;
    let hotcache_end = hotcache_start + footer.hotcache_length as usize;
    let hotcache =
        Hotcache::decode(&bytes[hotcache_start..hotcache_end]).map_err(InspectError::Hotcache)?;

    writeln!(
        report,
        "row-ids: [{}, {}) (count={})",
        hotcache.row_id_base,
        hotcache.row_id_base + hotcache.row_id_count,
        hotcache.row_id_count
    )
    .unwrap();
    writeln!(report, "blobs: {}", hotcache.blobs.len()).unwrap();
    for (i, blob) in hotcache.blobs.iter().enumerate() {
        writeln!(
            report,
            "  [{i}] family={} type={} field={:016x} storage_class={} tier={} alignment={} \
             chunk_codec={} offset={} length={} checksum={:016x}",
            blob.family_id,
            blob.blob_type_id,
            blob.field_id,
            storage_class_label(blob.storage_class),
            tier_label(blob.tier),
            blob.alignment,
            chunk_codec_label(blob.chunk_codec),
            blob.offset,
            blob.length,
            blob.checksum,
        )
        .unwrap();
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_for_the_rfc_worked_example_segment() {
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/container/toy-segment.bin"
        ))
        .unwrap();

        let report = format_report(&bytes).unwrap();

        assert!(report.contains("file size: 110 bytes"), "{report}");
        assert!(report.contains("footer: ok"), "{report}");
        assert!(report.contains("hotcache: offset=8 length=62"), "{report}");
        assert!(
            report.contains("row-ids: [1000, 1002) (count=2)"),
            "{report}"
        );
        assert!(report.contains("blobs: 1"), "{report}");
        assert!(
            report.contains(
                "[0] family=0 type=0 field=0000000000000000 storage_class=raw-mappable \
                 tier=n/a alignment=8 chunk_codec=none offset=0 length=8"
            ),
            "{report}"
        );
    }

    #[test]
    fn report_rejects_bytes_too_short_to_hold_a_footer() {
        let result = format_report(&[0u8; 10]);

        assert!(matches!(result, Err(InspectError::TooShort)));
    }

    #[test]
    fn report_rejects_bad_magic() {
        let mut bytes = vec![0u8; 40];
        bytes[0..4].copy_from_slice(b"NOPE");

        let result = format_report(&bytes);

        assert!(matches!(
            result,
            Err(InspectError::Footer(
                strand_core::container::DecodeError::BadMagic
            ))
        ));
    }
}
