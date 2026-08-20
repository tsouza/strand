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

//! `strand-tools sweep`: the M3-5 orphan-sweep tool (`docs/roadmap.md`),
//! implementing `spec/manifest.md`'s already-normative "Orphan files" rule
//! (`CLAUDE.md` §6): "list the prefix, subtract everything referenced by
//! retained snapshots, delete the remainder older than the retention
//! window."
//!
//! **The retention window is a sweep parameter, not a `TableMetadata`
//! field — a real spec gap, resolved here per `CLAUDE.md` §3 rather than
//! decided silently.** `RetentionPolicy` (`table_metadata.rs`) governs
//! which *snapshots* stay eligible for a reader to still be using; the
//! Orphan files rule's "retention window" is a different thing entirely —
//! a grace period protecting an in-flight commit's segment/snapshot
//! objects from a sweep that runs concurrently with it, before that
//! commit's pointer CAS has landed (`CLAUDE.md` §6: a writer's objects are
//! orphaned, harmlessly, exactly when it crashes or loses the race between
//! writing them and advancing the pointer). Those two concerns have no
//! reason to share one number: a table's snapshot-retention policy is
//! reasonably measured in days, while the orphan safety margin only needs
//! to outlast the longest an in-flight write can plausibly still be
//! landing. Grounded against real prior art rather than invented from
//! memory (`CLAUDE.md` §3): Apache Iceberg's `remove_orphan_files`
//! procedure takes its own separate `older_than` parameter (default 3
//! days ago), entirely independent of `expire_snapshots`'s retention
//! properties (whose own equivalent, `history.expire.max-snapshot-age-ms`,
//! defaults to 5 days) — two separately documented parameters with two
//! different defaults (`references/iceberg-remove-orphan-files-procedure.md`).
//! [`DEFAULT_RETENTION_WINDOW_MILLIS`] mirrors that same default. Recorded
//! normatively in `rfcs/0001-container-rowid-manifest.md`'s Discussion
//! section (M3-5) and `spec/manifest.md`'s "Orphan files" paragraph.
//!
//! **Which snapshot objects feed `retained_snapshots` — "current" is
//! resolved from the real pointer, never guessed from a listing.** An
//! earlier version of this module picked "current" the way
//! `retained_snapshots` itself does internally — the highest version
//! number among the snapshot objects fed to it — applied to *every*
//! listed `_strand/snapshots/` object. That is unsound, not merely
//! imprecise: a writer that crashes after writing its snapshot object but
//! before its pointer CAS lands leaves an orphan whose version is exactly
//! `true_current.version + 1` (`manifest::commit`: the version is
//! computed from the state read at the *start* of the attempt), and nothing
//! ever advances the real pointer to catch up if that writer never
//! retries. A raw listing can therefore contain an orphan whose version
//! number is strictly **higher** than the real current snapshot's — "the
//! listed object with the highest version" would then misidentify that
//! orphan as current and protect its referenced files indefinitely,
//! exactly the failure this tool exists to avoid (the M0 crash-test
//! pattern, `tests/s3_store.rs`'s `orphaned_writer_crash_is_harmless_to_
//! readers`, reproduced for the sweep in this module's own tests).
//!
//! The fix: resolve the true current snapshot authoritatively via the
//! real CAS pointer (`strand_core::manifest::read_snapshot`, the same
//! function every conforming reader uses), not by inference. Any listed
//! snapshot object whose `version` exceeds `real_current.version` is then
//! **provably** an orphan — the pointer is proof no commit ever really
//! reached that version — and is never a `retained_snapshots` candidate,
//! however recent its own `committed_at_millis` looks. `real_current`'s
//! own segments and deletion vectors are always protected directly,
//! independent of whether the listing happens to include its own object.
//! Every OTHER listed snapshot object, with `version <= real_current.
//! version`, genuinely is ambiguous — real-historical-and-now-superseded,
//! or an orphan that lost a same-version race — and nothing in the wire
//! format distinguishes them, so all such objects are fed to
//! `retained_snapshots` together. This remains safe in the same way the
//! earlier draft argued: `retained_snapshots`'s count-based floor
//! deduplicates by version number (a `HashSet<u64>`), so a same-version
//! orphan cannot displace a real snapshot from the "N most recent" floor;
//! at worst it is itself also judged "retained" for one extra sweep
//! cycle, protecting its own files a little longer than strictly
//! necessary — the same over-retain-not-under-retain asymmetry
//! `table_metadata.rs`'s own doc comment already argues for. What changed
//! is bounding the candidate set from above by the one version number a
//! sweep can actually prove is real, rather than trusting the listing's
//! own maximum.

use std::collections::HashSet;

use strand_core::manifest::SnapshotMetadata;
use strand_core::store::{ConditionalStore, DeletableStore, ListableStore, StoreError};
use strand_core::table_metadata::{TableMetadataError, read_table_metadata, retained_snapshots};

/// Apache Iceberg's own `remove_orphan_files` default (`older_than`, "3
/// days ago" — `references/iceberg-remove-orphan-files-procedure.md`),
/// reused here as this sweep's own default retention window rather than
/// inventing a different number with no grounding.
pub const DEFAULT_RETENTION_WINDOW_MILLIS: u64 = 3 * 24 * 60 * 60 * 1000;

const SNAPSHOTS_PREFIX: &str = "_strand/snapshots/";
const CURRENT_POINTER_KEY: &str = "_strand/current";
const TABLE_METADATA_KEY: &str = "_strand/metadata.json";

/// Milliseconds since the Unix epoch — the real clock reading a live sweep
/// (the `strand-tools sweep` CLI) passes as [`sweep_orphans`]'s `now_millis`
/// argument. Kept as a real, callable function (rather than inlined at the
/// one call site) so tests can call the identical clock a real invocation
/// would.
pub fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_millis() as u64
}

#[derive(Debug)]
pub enum SweepError {
    /// The table has no `_strand/metadata.json` yet — a sweep needs a real
    /// `RetentionPolicy` to know which snapshots are still retained, and
    /// refuses to guess one (`table_metadata::write_table_metadata`
    /// already refuses to accept an empty policy for the same reason).
    NoTableMetadata,
    TableMetadata(TableMetadataError),
    Store(StoreError),
    /// Resolving the real current snapshot via the CAS pointer
    /// (`strand_core::manifest::read_snapshot`) failed — a sweep refuses
    /// to guess "current" from a raw listing (this module's own doc
    /// comment explains why that would be unsafe), so it cannot proceed
    /// without this.
    ReadSnapshot(strand_core::manifest::ReadError),
    /// A listed object under `_strand/snapshots/` did not decode as
    /// [`SnapshotMetadata`] JSON. The sweep aborts rather than silently
    /// skip it: an undecodable snapshot object is exactly the case where
    /// this tool cannot compute a trustworthy "still referenced" set, and
    /// guessing wrong here risks the one thing this tool must never do.
    MalformedSnapshot {
        path: String,
        error: String,
    },
}

impl std::fmt::Display for SweepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SweepError::NoTableMetadata => write!(
                f,
                "no _strand/metadata.json under this prefix — write table metadata \
                 (with a real RetentionPolicy) before sweeping"
            ),
            SweepError::TableMetadata(e) => write!(f, "table metadata: {e:?}"),
            SweepError::Store(e) => write!(f, "store: {e:?}"),
            SweepError::ReadSnapshot(e) => write!(f, "resolving the current snapshot: {e:?}"),
            SweepError::MalformedSnapshot { path, error } => {
                write!(f, "malformed snapshot object {path}: {error}")
            }
        }
    }
}

impl From<StoreError> for SweepError {
    fn from(e: StoreError) -> Self {
        SweepError::Store(e)
    }
}

impl From<TableMetadataError> for SweepError {
    fn from(e: TableMetadataError) -> Self {
        SweepError::TableMetadata(e)
    }
}

/// What one sweep did (or, under `dry_run`, would do) to every object it
/// found under the swept prefix, partitioned by disposition — so a caller
/// (the CLI, or a test) can inspect exactly what happened rather than only
/// a count.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SweepOutcome {
    /// Not referenced by any retained snapshot, and older than the
    /// retention window: physically deleted (or, under `dry_run`, would
    /// have been).
    pub deleted: Vec<String>,
    /// Referenced by at least one retained snapshot (or is the current
    /// pointer / table metadata object itself) — left untouched.
    pub retained_referenced: Vec<String>,
    /// Not referenced by anything retained, but younger than the
    /// retention window — left untouched this cycle, the safety margin
    /// against a commit still in flight.
    pub skipped_within_window: Vec<String>,
}

/// Runs one orphan sweep against everything under `prefix`
/// (`spec/manifest.md` "Orphan files"): resolves the real current
/// snapshot via the CAS pointer, reads the table's real `RetentionPolicy`
/// and every snapshot object under `{prefix}_strand/snapshots/`, computes
/// the retained subset via `retained_snapshots` against `now_millis`
/// (bounded above by the real current version — this module's own doc
/// comment explains why), unions every retained snapshot's own path plus
/// its `segments[].path`/`deletion_vector.path` (plus the current pointer
/// and table-metadata objects, which are never orphan-eligible), and
/// deletes everything else under `prefix` whose own `last_modified_millis`
/// is older than `retention_window_millis` — exactly the rule stated in
/// `CLAUDE.md` §6 and `spec/manifest.md`. Under `dry_run`, nothing is
/// actually deleted; [`SweepOutcome::deleted`] still reports what *would*
/// have been.
pub fn sweep_orphans<S>(
    store: &S,
    prefix: &str,
    retention_window_millis: u64,
    now_millis: u64,
    dry_run: bool,
) -> Result<SweepOutcome, SweepError>
where
    S: ConditionalStore + ListableStore + DeletableStore,
{
    let metadata = read_table_metadata(store)?.ok_or(SweepError::NoTableMetadata)?;

    // The one authoritative "current" snapshot — resolved via the real
    // CAS pointer, the same function every conforming reader uses, never
    // inferred from a raw listing's version numbers (this module's own
    // doc comment explains why that would be unsafe).
    let real_current =
        strand_core::manifest::read_snapshot(store).map_err(SweepError::ReadSnapshot)?;

    let snapshots_prefix = format!("{prefix}{SNAPSHOTS_PREFIX}");
    let listed_snapshots = store.list(&snapshots_prefix)?;

    let mut snapshots: Vec<(String, SnapshotMetadata)> = Vec::with_capacity(listed_snapshots.len());
    for obj in &listed_snapshots {
        let Some((bytes, _etag)) = store.get(&obj.key)? else {
            // Raced with a concurrent sweep or a real compaction: listed a
            // moment ago, gone now. Not this sweep's object to account
            // for either way — skip it rather than fail the whole sweep.
            continue;
        };
        let snapshot: SnapshotMetadata =
            serde_json::from_slice(&bytes).map_err(|e| SweepError::MalformedSnapshot {
                path: obj.key.clone(),
                error: e.to_string(),
            })?;
        snapshots.push((obj.key.clone(), snapshot));
    }

    let mut referenced: HashSet<String> = HashSet::new();
    referenced.insert(format!("{prefix}{CURRENT_POINTER_KEY}"));
    referenced.insert(format!("{prefix}{TABLE_METADATA_KEY}"));

    if let Some(real_current) = &real_current {
        // The current snapshot's own referenced files are always
        // protected directly from the authoritative read above,
        // independent of whether the listing happens to include its own
        // object (a listing race is not grounds to under-protect the
        // live snapshot).
        for segment in &real_current.segments {
            referenced.insert(segment.path.clone());
            if let Some(dv) = &segment.deletion_vector {
                referenced.insert(dv.path.clone());
            }
        }

        // Historical-retention candidates: every listed snapshot object
        // whose version cannot exceed the true current one. A version
        // strictly greater than `real_current.version` is provably an
        // orphan (the pointer is proof no commit ever really reached it)
        // and is never a retention-floor candidate.
        let mut candidates: Vec<SnapshotMetadata> = snapshots
            .iter()
            .map(|(_, s)| s.clone())
            .filter(|s| s.version <= real_current.version)
            .collect();
        if !candidates.iter().any(|s| s.version == real_current.version) {
            // Defensive: the real current object itself somehow wasn't
            // among the listed ones (a listing race). Its own version
            // must still anchor `retained_snapshots`'s internal
            // "current = max version" check correctly.
            candidates.push(real_current.clone());
        }

        let retained = retained_snapshots(&metadata.retention, &candidates, now_millis);
        let retained_versions: HashSet<u64> = retained.iter().map(|s| s.version).collect();

        for (path, snapshot) in &snapshots {
            if snapshot.version > real_current.version {
                continue;
            }
            if !retained_versions.contains(&snapshot.version) {
                continue;
            }
            referenced.insert(path.clone());
            for segment in &snapshot.segments {
                referenced.insert(segment.path.clone());
                if let Some(dv) = &segment.deletion_vector {
                    referenced.insert(dv.path.clone());
                }
            }
        }
    }
    // `real_current` is `None` for a table with no real commits yet:
    // nothing from the manifest layer is referenced, and every listed
    // snapshot object (if any exist at all) is an orphan candidate like
    // anything else under the prefix.

    let all_objects = store.list(prefix)?;
    let mut outcome = SweepOutcome::default();
    for obj in all_objects {
        if referenced.contains(&obj.key) {
            outcome.retained_referenced.push(obj.key);
            continue;
        }
        let age_millis = now_millis.saturating_sub(obj.last_modified_millis);
        if age_millis <= retention_window_millis {
            outcome.skipped_within_window.push(obj.key);
            continue;
        }
        if !dry_run {
            store.delete_object(&obj.key)?;
        }
        outcome.deleted.push(obj.key);
    }

    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use strand_core::manifest::{SegmentRef, commit};
    use strand_core::store::InMemoryStore;
    use strand_core::table_metadata::{
        CasHost, RetentionPolicy, TableMetadata, write_table_metadata,
    };

    fn write_metadata(store: &InMemoryStore, retention: RetentionPolicy) {
        write_table_metadata(
            store,
            &TableMetadata {
                format_version: 0,
                cas_host: CasHost::Native {
                    store: "test".to_string(),
                },
                retention,
            },
        )
        .unwrap();
    }

    /// Commits a real segment reference *and* writes a real (placeholder)
    /// object at `path` — unlike `manifest.rs`'s own same-named test
    /// helper, which only exercises the manifest layer and never writes
    /// segment bytes at all. This module's tests check real object
    /// presence/absence after a sweep, so the object must genuinely exist
    /// beforehand, the same way a real writer's `write_segment` would
    /// have put it there before referencing it in a commit.
    fn commit_one_segment(store: &InMemoryStore, path: &str, row_id_count: u64) {
        store.put_if_absent(path, b"segment bytes").unwrap();
        let path = path.to_string();
        commit(store, |next_row_id| {
            vec![SegmentRef {
                path: path.clone(),
                row_id_base: next_row_id,
                row_id_count,
                byte_length: 100,
                checksum: 0xf00d,
                deletion_vector: None,
            }]
        })
        .unwrap();
    }

    #[test]
    fn sweep_without_table_metadata_refuses_to_guess() {
        let store = InMemoryStore::new();
        commit_one_segment(&store, "segments/a.bin", 10);

        let err = sweep_orphans(&store, "", DEFAULT_RETENTION_WINDOW_MILLIS, 0, false).unwrap_err();

        assert!(matches!(err, SweepError::NoTableMetadata));
    }

    /// The core scenario: a crashed writer left a segment and a snapshot
    /// object that never got pointed at (the same pattern
    /// `tests/s3_store.rs`'s `orphaned_writer_crash_is_harmless_to_readers`
    /// establishes against real MinIO). A sweep past the retention window
    /// must remove exactly the orphan and nothing a retained snapshot
    /// references.
    ///
    /// This is also the exact scenario that catches the bug this module's
    /// own doc comment describes fixing: the orphan's `version` (1) is
    /// **higher** than the real current snapshot's (`baseline`, version
    /// 0), because nothing ever advanced the pointer past `baseline`. A
    /// sweep that picked "current" as "the listed object with the highest
    /// version" would misidentify this orphan as current and protect
    /// `segments/orphan.bin` forever; this test fails loudly under that
    /// bug (`segments/orphan.bin` would end up in `retained_referenced`
    /// instead of `deleted`).
    #[test]
    fn sweep_removes_an_old_orphan_and_leaves_referenced_files_untouched() {
        let store = InMemoryStore::new();
        write_metadata(
            &store,
            RetentionPolicy {
                min_snapshots_to_keep: Some(1),
                max_snapshot_age_millis: None,
            },
        );
        commit_one_segment(&store, "segments/baseline.bin", 2);

        // Simulate the crash: a segment and a snapshot object written, but
        // `_strand/current` never updated to point at them.
        store
            .put_if_absent("segments/orphan.bin", b"orphan bytes")
            .unwrap();
        let orphan_snapshot = SnapshotMetadata {
            version: 1,
            next_row_id: 5,
            segments: vec![SegmentRef {
                path: "segments/orphan.bin".to_string(),
                row_id_base: 2,
                row_id_count: 3,
                byte_length: 12,
                checksum: 0,
                deletion_vector: None,
            }],
            committed_at_millis: 0,
        };
        store
            .put_if_absent(
                "_strand/snapshots/crash-orphan.json",
                &serde_json::to_vec(&orphan_snapshot).unwrap(),
            )
            .unwrap();

        // `InMemoryStore` stamps `last_modified_millis` from the real
        // wall clock (`store.rs`), so a real "now" must be used here too —
        // a small window plus a short real sleep guarantees every
        // unreferenced object is past it, without the flakiness a
        // millisecond-exact race would risk.
        std::thread::sleep(std::time::Duration::from_millis(5));
        let outcome = sweep_orphans(&store, "", 0, now_millis(), false).unwrap();

        assert!(outcome.deleted.contains(&"segments/orphan.bin".to_string()));
        assert!(
            outcome
                .deleted
                .contains(&"_strand/snapshots/crash-orphan.json".to_string())
        );
        assert!(store.get("segments/orphan.bin").unwrap().is_none());
        assert!(
            store
                .get("_strand/snapshots/crash-orphan.json")
                .unwrap()
                .is_none()
        );

        // The baseline segment and the real current snapshot/pointer
        // survive untouched.
        assert!(store.get("segments/baseline.bin").unwrap().is_some());
        assert!(store.get("_strand/current").unwrap().is_some());
        assert!(store.get("_strand/metadata.json").unwrap().is_some());
        assert!(
            outcome
                .retained_referenced
                .contains(&"segments/baseline.bin".to_string())
        );
    }

    /// The retention-window safety margin: an orphan younger than the
    /// window must survive a sweep even though nothing references it —
    /// the real second safety margin against a race with a commit still
    /// in flight, named explicitly in `spec/manifest.md`'s "Orphan files"
    /// rule.
    #[test]
    fn a_young_unreferenced_orphan_survives_a_sweep() {
        let store = InMemoryStore::new();
        write_metadata(
            &store,
            RetentionPolicy {
                min_snapshots_to_keep: Some(1),
                max_snapshot_age_millis: None,
            },
        );
        commit_one_segment(&store, "segments/baseline.bin", 2);
        store
            .put_if_absent("segments/fresh-orphan.bin", b"in flight")
            .unwrap();

        // A huge retention window: even measured "now" (age ~0) is well
        // inside it.
        let outcome = sweep_orphans(
            &store,
            "",
            DEFAULT_RETENTION_WINDOW_MILLIS,
            now_millis(),
            false,
        )
        .unwrap();

        assert!(
            outcome
                .skipped_within_window
                .contains(&"segments/fresh-orphan.bin".to_string())
        );
        assert!(outcome.deleted.is_empty());
        assert!(store.get("segments/fresh-orphan.bin").unwrap().is_some());
    }

    #[test]
    fn dry_run_reports_deletions_without_performing_them() {
        let store = InMemoryStore::new();
        write_metadata(
            &store,
            RetentionPolicy {
                min_snapshots_to_keep: Some(1),
                max_snapshot_age_millis: None,
            },
        );
        commit_one_segment(&store, "segments/baseline.bin", 2);
        store.put_if_absent("segments/orphan.bin", b"x").unwrap();

        std::thread::sleep(std::time::Duration::from_millis(5));
        let outcome = sweep_orphans(&store, "", 0, now_millis(), true).unwrap();

        assert!(outcome.deleted.contains(&"segments/orphan.bin".to_string()));
        assert!(
            store.get("segments/orphan.bin").unwrap().is_some(),
            "dry_run must not actually delete anything"
        );
    }

    /// A **superseded** deletion-vector object — one a later
    /// `commit_deletion_vector` call has already replaced on the current
    /// segment reference — is exactly as orphan-eligible as a superseded
    /// segment once the snapshot that was current when it was written
    /// falls out of the retention policy. `docs/ledger.md`'s own open item
    /// ("the orphan-sweep tool's handling of superseded deletion-vector
    /// objects") is resolved by this being the same referenced-set
    /// computation, not special-cased: because every snapshot's `segments`
    /// list is the full cumulative set (`manifest.rs`'s `commit`), the
    /// *current* snapshot always carries the newest deletion-vector
    /// reference for a live segment — a superseded one only stops being
    /// referenced once the intermediate snapshot that pointed at it is no
    /// longer retained.
    #[test]
    fn a_superseded_deletion_vector_is_swept_once_its_snapshot_is_no_longer_retained() {
        let store = InMemoryStore::new();
        write_metadata(
            &store,
            RetentionPolicy {
                // Only the current snapshot is retained.
                min_snapshots_to_keep: Some(1),
                max_snapshot_age_millis: None,
            },
        );
        commit_one_segment(&store, "segments/a.bin", 200);
        let seg = {
            let snap = strand_core::manifest::read_snapshot(&store)
                .unwrap()
                .unwrap();
            snap.segments[0].clone()
        };

        let write_dv = |ordinals: &[u32], suffix: u32| {
            let mut bitmap = strand_core::deletion::RoaringBitmap::new();
            for &o in ordinals {
                bitmap.insert(o);
            }
            let bytes =
                strand_core::deletion::build_deletion_vector(&bitmap, seg.row_id_count).unwrap();
            let checksum = strand_core::deletion::checksum(&bytes);
            let path = format!("deletions/a-{suffix}.bin");
            store.put_if_absent(&path, &bytes).unwrap();
            strand_core::deletion::DeletionVectorRef {
                path,
                byte_length: bytes.len() as u64,
                checksum,
            }
        };

        // First tombstone: version 1's snapshot references `deletions/a-0.bin`.
        strand_core::manifest::commit_deletion_vector(&store, &seg.path, |_current| {
            write_dv(&[2], 0)
        })
        .unwrap();
        // Second tombstone supersedes it: version 2 (current) references
        // `deletions/a-1.bin` instead. `deletions/a-0.bin` is now
        // referenced only by version 1's snapshot object.
        strand_core::manifest::commit_deletion_vector(&store, &seg.path, |_current| {
            write_dv(&[2, 5], 1)
        })
        .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(5));
        let outcome = sweep_orphans(&store, "", 0, now_millis(), false).unwrap();

        assert!(
            outcome.deleted.contains(&"deletions/a-0.bin".to_string()),
            "superseded deletion-vector object must be swept: {:?}",
            outcome.deleted
        );
        assert!(
            outcome
                .retained_referenced
                .contains(&"deletions/a-1.bin".to_string()),
            "the current deletion-vector object must survive: {:?}",
            outcome.retained_referenced
        );
        assert!(store.get("deletions/a-0.bin").unwrap().is_none());
        assert!(store.get("deletions/a-1.bin").unwrap().is_some());
    }
}
