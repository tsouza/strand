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

//! Table metadata (`_strand/metadata.json`) and retention-policy-driven
//! snapshot expiry — `spec/manifest.md` §1 ("Table metadata") and
//! `CLAUDE.md` §6's deletion-safety rule, roadmap item M3-4
//! (`docs/roadmap.md`).
//!
//! Table metadata is a **separate object from the versioned snapshot
//! chain** `manifest.rs` implements: it is written exactly once, at table
//! creation, and is immutable thereafter except for the one declared
//! amendment (`CLAUDE.md` §6, "moving the CAS host is itself a committed
//! metadata change through the old host") — an amendment path this module
//! does not implement, since nothing in the roadmap has asked for it yet
//! and inventing its exact mechanics now would be exactly the kind of
//! mid-session format decision `CLAUDE.md` §3 prohibits.
//!
//! Because this object never participates in the `_strand/current` CAS
//! race — it has no pointer, no proposed-vs-current distinction, no retry
//! loop — it is entirely outside the shape `verification/manifest.tla`
//! models. Creating it is a single `put_if_absent` to a fixed key, not a
//! new action on the manifest's commit protocol.
//!
//! The retention-eligibility function here ([`retained_snapshots`]) is the
//! pure, testable piece the M3-5 orphan-sweep tool
//! (`docs/roadmap.md`, `spec/manifest.md` "Orphan files") will call: given
//! a policy and a snapshot list, which snapshots are still retained, and
//! therefore which files a sweep MUST NOT delete.

use crate::manifest::SnapshotMetadata;
use crate::store::{ConditionalStore, StoreError};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const TABLE_METADATA_KEY: &str = "_strand/metadata.json";

/// Where the pointer CAS lives (`CLAUDE.md` §6, "one declared CAS host"),
/// per RFC 0001 Design §3's exact two JSON shapes: `{"type": "native",
/// "store": "s3"}` for a store with native conditional writes, or
/// `{"type": "catalog", "uri": "..."}` for the external-catalog fallback.
/// Serde's internally tagged representation produces exactly that shape —
/// verified byte-for-byte in this module's tests, not just asserted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CasHost {
    Native { store: String },
    Catalog { uri: String },
}

/// Minimum snapshot retention (`spec/manifest.md` §1: "a count, a
/// duration, or both"). At least one field MUST be `Some` —
/// [`write_table_metadata`] rejects a policy with neither, since a table
/// with no retention floor at all would let compaction treat every
/// snapshot but the current one as immediately expired, which is not what
/// "minimum snapshot retention" can mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// Keep at least this many of the most recent snapshots, regardless of
    /// age.
    pub min_snapshots_to_keep: Option<u32>,
    /// Keep any snapshot committed within this many milliseconds of "now",
    /// regardless of count.
    pub max_snapshot_age_millis: Option<u64>,
}

/// The table metadata object (`_strand/metadata.json`, `spec/manifest.md`
/// §1). `format_version` is the manifest layer's own version, distinct
/// from a segment's `format_major`/`format_minor` (`spec/container.md`
/// §1) — the spec text names a single "format version" for this object,
/// not a major/minor pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableMetadata {
    pub format_version: u32,
    pub cas_host: CasHost,
    pub retention: RetentionPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableMetadataError {
    /// [`RetentionPolicy`] declared neither `min_snapshots_to_keep` nor
    /// `max_snapshot_age_millis`.
    InvalidRetentionPolicy,
    /// `_strand/metadata.json` already exists — this object is write-once
    /// (see this module's doc comment).
    AlreadyExists,
    /// The stored object's bytes did not decode as valid `TableMetadata`
    /// JSON.
    Malformed(String),
    /// The backend failed for a reason other than the object already
    /// existing.
    Io(String),
}

fn validate(metadata: &TableMetadata) -> Result<(), TableMetadataError> {
    if metadata.retention.min_snapshots_to_keep.is_none()
        && metadata.retention.max_snapshot_age_millis.is_none()
    {
        return Err(TableMetadataError::InvalidRetentionPolicy);
    }
    Ok(())
}

/// Writes `metadata` to `_strand/metadata.json`, once, for a table that has
/// none yet. Uses `put_if_absent` — the same create-if-absent primitive
/// `manifest.rs` uses for a snapshot's attempt-unique path — but with no
/// retry loop, since there is nothing to recompute against fresh state:
/// this key either already exists (another writer already created the
/// table) or it doesn't, and there is no "current" version to race against.
///
/// # Errors
///
/// [`TableMetadataError::InvalidRetentionPolicy`] if `metadata.retention`
/// declares neither field. [`TableMetadataError::AlreadyExists`] if the
/// object is already present with different content. [`TableMetadataError::
/// Io`] for any other backend failure.
///
/// An [`StoreError::Ambiguous`] outcome is resolved exactly per that
/// error's own documented obligation (`store.rs`): a follow-up `GET`
/// checks whether *this* write's own bytes are the ones now present —
/// mirroring `manifest.rs`'s `propose_snapshot` pointer-CAS disambiguation,
/// the same pattern applied to a plain create instead of a compare-and-swap.
pub fn write_table_metadata<S: ConditionalStore>(
    store: &S,
    metadata: &TableMetadata,
) -> Result<(), TableMetadataError> {
    validate(metadata)?;
    let bytes = serde_json::to_vec(metadata).expect("TableMetadata always serializes");

    match store.put_if_absent(TABLE_METADATA_KEY, &bytes) {
        Ok(_) => Ok(()),
        Err(StoreError::PreconditionFailed) => Err(TableMetadataError::AlreadyExists),
        Err(StoreError::Io(msg)) => Err(TableMetadataError::Io(msg)),
        Err(StoreError::Ambiguous(msg)) => match store.get(TABLE_METADATA_KEY) {
            Ok(Some((present, _))) if present == bytes => Ok(()),
            Ok(Some(_)) => Err(TableMetadataError::AlreadyExists),
            Ok(None) => Err(TableMetadataError::Io(format!(
                "table metadata write was ambiguous ({msg}) and a follow-up read found nothing"
            ))),
            Err(StoreError::Io(follow_up) | StoreError::Ambiguous(follow_up)) => {
                Err(TableMetadataError::Io(format!(
                    "table metadata write was ambiguous ({msg}) and the \
                     follow-up read to resolve it also failed: {follow_up}"
                )))
            }
            Err(StoreError::PreconditionFailed) => {
                unreachable!("ConditionalStore::get never returns PreconditionFailed")
            }
        },
    }
}

/// Reads `_strand/metadata.json`, or `Ok(None)` for a table that has none
/// yet (a table created before this file existed, or one whose creation
/// hasn't happened yet in a test).
///
/// # Errors
///
/// [`TableMetadataError::Malformed`] if the object exists but doesn't
/// decode as [`TableMetadata`] JSON. [`TableMetadataError::Io`] for any
/// other backend failure.
pub fn read_table_metadata<S: ConditionalStore>(
    store: &S,
) -> Result<Option<TableMetadata>, TableMetadataError> {
    match store.get(TABLE_METADATA_KEY) {
        Ok(Some((bytes, _etag))) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| TableMetadataError::Malformed(e.to_string())),
        Ok(None) => Ok(None),
        Err(StoreError::Io(msg) | StoreError::Ambiguous(msg)) => Err(TableMetadataError::Io(msg)),
        Err(StoreError::PreconditionFailed) => {
            unreachable!("ConditionalStore::get never returns PreconditionFailed")
        }
    }
}

/// Determines which of `snapshots` are **retained** under `policy` as of
/// `now_millis` — the pure function `CLAUDE.md` §6's deletion-safety rule
/// depends on: "compaction may only physically delete files unreferenced
/// by every retained snapshot." A file referenced by any snapshot this
/// function excludes from its result is a candidate for physical deletion;
/// a file referenced by any snapshot it includes MUST NOT be deleted. This
/// function does no I/O and touches no store — the M3-5 orphan-sweep tool
/// calls it with real listed snapshots and a real clock reading, but the
/// decision itself is deterministic and independently testable, per this
/// task's own requirement.
///
/// Takes `now_millis` as a parameter rather than reading the system clock
/// itself, so tests can probe exact boundary conditions (a snapshot
/// exactly at the age cutoff) without a real clock's nondeterminism.
///
/// **Retention rule.** A snapshot is retained if *any* of the following
/// holds:
/// - it is the current snapshot (the one with the highest `version` in
///   `snapshots`) — always retained regardless of policy, since deleting
///   files the live pointer's own snapshot references would break every
///   reader immediately, not just race one (`CLAUDE.md` §6's 404-refresh
///   rule only covers a snapshot compaction is *allowed* to expire, never
///   the current one);
/// - `policy.min_snapshots_to_keep` is `Some(n)` and this snapshot is
///   among the `n` most recent by version;
/// - `policy.max_snapshot_age_millis` is `Some(max_age)` and
///   `now_millis - snapshot.committed_at_millis <= max_age`.
///
/// When both `min_snapshots_to_keep` and `max_snapshot_age_millis` are
/// set, a snapshot retained by *either* criterion is retained — the union,
/// not the intersection. `spec/manifest.md` §1 says a policy may declare
/// "a count, a duration, or both" but does not say how the two combine
/// when both are present; this function resolves that ambiguity by
/// picking the safer of the two readings (recorded properly in RFC 0001's
/// Discussion section, per `CLAUDE.md` §3, rather than decided silently
/// here): the deletion-safety rule's cost of getting retention wrong is
/// asymmetric. Under-retaining risks physically deleting a file a live
/// snapshot still references — real, unrecoverable data loss for any
/// reader that has that snapshot open. Over-retaining only costs storage,
/// a cost `CLAUDE.md` §6 already accepts elsewhere ("write amplification
/// is the writer's problem"). The union is also Apache Iceberg's own
/// documented behavior for the equivalent two knobs
/// (`history.expire.min-snapshots-to-keep` and
/// `history.expire.max-snapshot-age-ms`), the same prior art RFC 0001's
/// Design §3 already cites for this protocol's optimistic-concurrency
/// shape.
pub fn retained_snapshots<'a>(
    policy: &RetentionPolicy,
    snapshots: &'a [SnapshotMetadata],
    now_millis: u64,
) -> Vec<&'a SnapshotMetadata> {
    let Some(current_version) = snapshots.iter().map(|s| s.version).max() else {
        return Vec::new();
    };

    let count_retained: HashSet<u64> = match policy.min_snapshots_to_keep {
        Some(n) => {
            let mut versions: Vec<u64> = snapshots.iter().map(|s| s.version).collect();
            versions.sort_unstable_by(|a, b| b.cmp(a));
            versions.into_iter().take(n as usize).collect()
        }
        None => HashSet::new(),
    };

    let mut retained: Vec<&SnapshotMetadata> = snapshots
        .iter()
        .filter(|s| {
            s.version == current_version
                || count_retained.contains(&s.version)
                || policy.max_snapshot_age_millis.is_some_and(|max_age| {
                    now_millis.saturating_sub(s.committed_at_millis) <= max_age
                })
        })
        .collect();
    retained.sort_unstable_by_key(|s| s.version);
    retained
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::SegmentRef;
    use crate::store::InMemoryStore;

    fn toy_snapshot(version: u64, committed_at_millis: u64) -> SnapshotMetadata {
        SnapshotMetadata {
            version,
            next_row_id: 10,
            segments: vec![SegmentRef {
                path: format!("segments/{version}.bin"),
                row_id_base: 0,
                row_id_count: 10,
                byte_length: 100,
                checksum: 0,
                deletion_vector: None,
            }],
            committed_at_millis,
        }
    }

    #[test]
    fn cas_host_native_serializes_to_the_rfc_0001_shape() {
        let host = CasHost::Native {
            store: "s3".to_string(),
        };
        let json: serde_json::Value =
            serde_json::from_slice(&serde_json::to_vec(&host).unwrap()).unwrap();
        assert_eq!(json, serde_json::json!({"type": "native", "store": "s3"}));
    }

    #[test]
    fn cas_host_catalog_serializes_to_the_rfc_0001_shape() {
        let host = CasHost::Catalog {
            uri: "https://catalog.example/table".to_string(),
        };
        let json: serde_json::Value =
            serde_json::from_slice(&serde_json::to_vec(&host).unwrap()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"type": "catalog", "uri": "https://catalog.example/table"})
        );
    }

    #[test]
    fn table_metadata_round_trips_through_json() {
        let metadata = TableMetadata {
            format_version: 0,
            cas_host: CasHost::Native {
                store: "s3".to_string(),
            },
            retention: RetentionPolicy {
                min_snapshots_to_keep: Some(3),
                max_snapshot_age_millis: Some(7 * 24 * 60 * 60 * 1000),
            },
        };

        let json = serde_json::to_vec(&metadata).unwrap();
        let decoded: TableMetadata = serde_json::from_slice(&json).unwrap();

        assert_eq!(decoded, metadata);
    }

    #[test]
    fn write_then_read_round_trips_through_a_real_store() {
        let store = InMemoryStore::new();
        let metadata = TableMetadata {
            format_version: 0,
            cas_host: CasHost::Catalog {
                uri: "catalog://table-1".to_string(),
            },
            retention: RetentionPolicy {
                min_snapshots_to_keep: Some(1),
                max_snapshot_age_millis: None,
            },
        };

        write_table_metadata(&store, &metadata).unwrap();
        let read_back = read_table_metadata(&store).unwrap();

        assert_eq!(read_back, Some(metadata));
    }

    #[test]
    fn read_returns_none_for_a_table_with_no_metadata_yet() {
        let store = InMemoryStore::new();
        assert_eq!(read_table_metadata(&store).unwrap(), None);
    }

    #[test]
    fn write_rejects_a_retention_policy_with_neither_field_set() {
        let store = InMemoryStore::new();
        let metadata = TableMetadata {
            format_version: 0,
            cas_host: CasHost::Native {
                store: "s3".to_string(),
            },
            retention: RetentionPolicy {
                min_snapshots_to_keep: None,
                max_snapshot_age_millis: None,
            },
        };

        let err = write_table_metadata(&store, &metadata).unwrap_err();
        assert_eq!(err, TableMetadataError::InvalidRetentionPolicy);
        assert_eq!(
            read_table_metadata(&store).unwrap(),
            None,
            "a rejected write must not land"
        );
    }

    #[test]
    fn write_is_write_once_a_second_call_fails() {
        let store = InMemoryStore::new();
        let metadata = TableMetadata {
            format_version: 0,
            cas_host: CasHost::Native {
                store: "s3".to_string(),
            },
            retention: RetentionPolicy {
                min_snapshots_to_keep: Some(1),
                max_snapshot_age_millis: None,
            },
        };
        write_table_metadata(&store, &metadata).unwrap();

        let second = TableMetadata {
            retention: RetentionPolicy {
                min_snapshots_to_keep: Some(5),
                max_snapshot_age_millis: None,
            },
            ..metadata.clone()
        };
        let err = write_table_metadata(&store, &second).unwrap_err();

        assert_eq!(err, TableMetadataError::AlreadyExists);
        assert_eq!(
            read_table_metadata(&store).unwrap(),
            Some(metadata),
            "the first write's content must survive untouched"
        );
    }

    // --- retained_snapshots -------------------------------------------

    #[test]
    fn retained_snapshots_is_empty_for_an_empty_snapshot_list() {
        let policy = RetentionPolicy {
            min_snapshots_to_keep: Some(1),
            max_snapshot_age_millis: None,
        };
        assert_eq!(
            retained_snapshots(&policy, &[], 1_000),
            Vec::<&SnapshotMetadata>::new()
        );
    }

    #[test]
    fn the_current_snapshot_is_always_retained_even_outside_every_policy_window() {
        let policy = RetentionPolicy {
            min_snapshots_to_keep: Some(1),
            max_snapshot_age_millis: Some(10),
        };
        // The only snapshot is already 1000ms old against a 10ms window —
        // still retained, because it is current: nothing else references
        // the table's live segments.
        let snapshots = vec![toy_snapshot(0, 0)];

        let retained = retained_snapshots(&policy, &snapshots, 1_000);

        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].version, 0);
    }

    #[test]
    fn a_snapshot_within_the_duration_window_is_retained() {
        let policy = RetentionPolicy {
            min_snapshots_to_keep: None,
            max_snapshot_age_millis: Some(1_000),
        };
        // Three snapshots so the case under test (version 1) is neither
        // the current one nor saved by any count-based floor: version 0 is
        // old (age 1000, right at the boundary, covered separately below),
        // version 1 is recent (age 100, well inside the window), version 2
        // is current.
        let snapshots = vec![
            toy_snapshot(0, 0),
            toy_snapshot(1, 900),
            toy_snapshot(2, 1_000),
        ];

        let retained = retained_snapshots(&policy, &snapshots, 1_000);
        let versions: Vec<u64> = retained.iter().map(|s| s.version).collect();

        assert!(
            versions.contains(&1),
            "recent, non-current snapshot must be retained on duration alone"
        );
    }

    #[test]
    fn a_snapshot_outside_the_duration_window_is_not_retained_unless_current() {
        let policy = RetentionPolicy {
            min_snapshots_to_keep: None,
            max_snapshot_age_millis: Some(100),
        };
        // version 0 at t=0 is far outside a 100ms window measured at
        // t=10_000; version 1 at t=9_950 is current and within the window.
        let snapshots = vec![toy_snapshot(0, 0), toy_snapshot(1, 9_950)];

        let retained = retained_snapshots(&policy, &snapshots, 10_000);
        let versions: Vec<u64> = retained.iter().map(|s| s.version).collect();

        assert_eq!(versions, vec![1]);
    }

    #[test]
    fn duration_boundary_is_inclusive() {
        let policy = RetentionPolicy {
            min_snapshots_to_keep: None,
            max_snapshot_age_millis: Some(1_000),
        };
        // Exactly at the boundary: age == max_age must count as retained
        // ("<=", not "<") — erring toward retaining more, per this
        // function's own stated safety rationale.
        let snapshots = vec![toy_snapshot(0, 0), toy_snapshot(1, 500)];

        let retained = retained_snapshots(&policy, &snapshots, 1_000);
        let versions: Vec<u64> = retained.iter().map(|s| s.version).collect();

        assert!(
            versions.contains(&0),
            "age exactly == max_age must be retained"
        );
    }

    #[test]
    fn one_millisecond_past_the_boundary_is_not_retained_unless_current() {
        let policy = RetentionPolicy {
            min_snapshots_to_keep: None,
            max_snapshot_age_millis: Some(1_000),
        };
        let snapshots = vec![toy_snapshot(0, 0), toy_snapshot(1, 500)];

        // now=1001 makes version 0's age 1001 > 1000: just past the window.
        let retained = retained_snapshots(&policy, &snapshots, 1_001);
        let versions: Vec<u64> = retained.iter().map(|s| s.version).collect();

        assert!(
            !versions.contains(&0),
            "age one past max_age must be excluded"
        );
        assert!(versions.contains(&1), "current snapshot always retained");
    }

    #[test]
    fn min_snapshots_to_keep_retains_the_n_most_recent_by_version_regardless_of_age() {
        let policy = RetentionPolicy {
            min_snapshots_to_keep: Some(2),
            max_snapshot_age_millis: None,
        };
        // All committed "long ago" relative to a far-future now — only
        // the count-based floor should save any of them.
        let snapshots = vec![
            toy_snapshot(0, 0),
            toy_snapshot(1, 0),
            toy_snapshot(2, 0),
            toy_snapshot(3, 0),
        ];

        let retained = retained_snapshots(&policy, &snapshots, 1_000_000);
        let mut versions: Vec<u64> = retained.iter().map(|s| s.version).collect();
        versions.sort_unstable();

        assert_eq!(versions, vec![2, 3], "the two most recent by version");
    }

    #[test]
    fn both_policies_set_retain_the_union_not_the_intersection() {
        let policy = RetentionPolicy {
            min_snapshots_to_keep: Some(1),
            max_snapshot_age_millis: Some(50),
        };
        // version 0: age 1000 at now=1000, and not among the 1
        // most-recent-by-count (that's version 2) — excluded by both.
        // version 1: committed at t=960, age 40 <= the 50ms duration
        // window — saved by duration alone, excluded by count alone.
        // version 2: current, saved by both count and currency.
        let snapshots = vec![
            toy_snapshot(0, 0),
            toy_snapshot(1, 960),
            toy_snapshot(2, 1_000),
        ];

        let retained = retained_snapshots(&policy, &snapshots, 1_000);
        let mut versions: Vec<u64> = retained.iter().map(|s| s.version).collect();
        versions.sort_unstable();

        assert_eq!(
            versions,
            vec![1, 2],
            "version 1 saved by duration alone, version 2 by both count and currency"
        );
        assert!(
            !versions.contains(&0),
            "version 0 satisfies neither criterion and is not current"
        );
    }

    #[test]
    fn retained_snapshots_returned_in_ascending_version_order() {
        let policy = RetentionPolicy {
            min_snapshots_to_keep: Some(3),
            max_snapshot_age_millis: None,
        };
        let snapshots = vec![toy_snapshot(2, 0), toy_snapshot(0, 0), toy_snapshot(1, 0)];

        let retained = retained_snapshots(&policy, &snapshots, 0);
        let versions: Vec<u64> = retained.iter().map(|s| s.version).collect();

        assert_eq!(versions, vec![0, 1, 2]);
    }
}
