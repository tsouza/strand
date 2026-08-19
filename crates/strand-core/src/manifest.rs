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

//! The snapshot manifest and its CAS commit protocol, per RFC 0001 §3
//! (`rfcs/0001-container-rowid-manifest.md`), extended by RFC 0012
//! (`rfcs/0012-deletion-vectors.md`, `spec/deletion.md` §4) with a second
//! commit path, `commit_deletion_vector`, sharing the same CAS mechanics
//! via [`propose_snapshot`].

use crate::deletion::DeletionVectorRef;
use crate::store::{ConditionalStore, StoreError};
use serde::{Deserialize, Serialize};

const CURRENT_POINTER_KEY: &str = "_strand/current";

fn snapshot_key(version: u64, nonce: &str) -> String {
    format!("_strand/snapshots/{version:020}-{nonce}.json")
}

/// The current snapshot's state, as read from `_strand/current` and the
/// snapshot it points to — or the defaults for a table with no commits yet.
struct CurrentState {
    version: Option<u64>,
    next_row_id: u64,
    segments: Vec<SegmentRef>,
    pointer_etag: Option<crate::store::ETag>,
}

/// The result of one attempt at resolving the current snapshot: either the
/// table has never been committed to, the pointer's target was found, or the
/// pointer named a snapshot object that is no longer there — the 404 race
/// RFC 0001 §3 describes (compaction removed it between the pointer read and
/// the follow-up fetch).
enum ReadAttempt {
    NoCommitsYet,
    Found(CurrentState),
    Expired,
}

fn try_read_current<S: ConditionalStore>(store: &S) -> Result<ReadAttempt, StoreError> {
    let Some((pointer_bytes, pointer_etag)) = store.get(CURRENT_POINTER_KEY)? else {
        return Ok(ReadAttempt::NoCommitsYet);
    };
    let snapshot_path = String::from_utf8(pointer_bytes).expect("pointer content is UTF-8");
    let Some((snapshot_bytes, _)) = store.get(&snapshot_path)? else {
        return Ok(ReadAttempt::Expired);
    };
    let snapshot: SnapshotMetadata =
        serde_json::from_slice(&snapshot_bytes).expect("snapshot content is valid JSON");
    Ok(ReadAttempt::Found(CurrentState {
        version: Some(snapshot.version),
        next_row_id: snapshot.next_row_id,
        segments: snapshot.segments,
        pointer_etag: Some(pointer_etag),
    }))
}

/// Used by the writer path (`commit`), which already has its own outer retry
/// discipline on the pointer CAS: an expired read here is retried
/// unboundedly, since the writer's real bound is the number of times it's
/// willing to lose the CAS, not this read. A genuine backend failure is not
/// retried at all — it propagates immediately.
fn read_current<S: ConditionalStore>(store: &S) -> Result<CurrentState, StoreError> {
    loop {
        match try_read_current(store)? {
            ReadAttempt::NoCommitsYet => {
                return Ok(CurrentState {
                    version: None,
                    next_row_id: 0,
                    segments: Vec::new(),
                    pointer_etag: None,
                });
            }
            ReadAttempt::Found(state) => return Ok(state),
            ReadAttempt::Expired => continue,
        }
    }
}

/// RFC 0001 §3: the retry count is a reader parameter the RFC deliberately
/// does not pin, for the same reason the speculative tail size in the
/// container's open protocol is unpinned — but the bound itself, unlike its
/// exact value, is not optional. This default is provisional pending the
/// M0 crash-test data the RFC calls for.
const READER_REFRESH_RETRY_LIMIT: u32 = 5;

/// Commits `build_segments`'s output as a new snapshot, retrying per RFC 0001
/// §3 if another writer's commit wins the pointer CAS first. `build_segments`
/// is called fresh on every attempt, including retries, with the row-ID base
/// this attempt should build against — it is never reused across attempts,
/// which is what makes a stale, overlapping row-ID range structurally
/// impossible rather than a discipline the caller must maintain.
///
/// Because `build_segments` can run more than once, any I/O it performs
/// must be attempt-safe: writing a segment (via `segment::write_segment`,
/// say) to a **fixed** path derived only from caller-side state (a loop
/// index, a writer ID) will collide with that same path's first attempt on
/// retry, since the first attempt's write already landed even though its
/// commit lost the race — `write_segment`'s `put_if_absent` then correctly
/// rejects the second write to the same path. Include something
/// attempt-unique in the path (a fresh random nonce, a counter bumped each
/// call) the same way `commit`'s own snapshot-path nonce does.
pub fn commit<S: ConditionalStore>(
    store: &S,
    build_segments: impl Fn(u64) -> Vec<SegmentRef>,
) -> Result<SnapshotMetadata, CommitError> {
    loop {
        let current = read_current(store).map_err(CommitError::from_store_error)?;
        let new_segments = build_segments(current.next_row_id);
        let added_row_ids: u64 = new_segments.iter().map(|s| s.row_id_count).sum();

        let version = current.version.map_or(0, |v| v + 1);
        let mut segments = current.segments.clone();
        segments.extend(new_segments);
        let snapshot = SnapshotMetadata {
            version,
            next_row_id: current.next_row_id + added_row_ids,
            segments,
        };

        if let Some(committed) = propose_snapshot(store, &current, snapshot)? {
            return Ok(committed);
        }
    }
}

/// Updates one existing segment's `deletion_vector` reference — the
/// `spec/deletion.md` §4 commit path (RFC 0012), sharing `commit`'s exact
/// CAS mechanics via [`propose_snapshot`] but substituting a different
/// transform: revise one entry in place rather than append a new one.
/// `next_row_id` is unchanged; no segment is appended or removed.
///
/// `build_deletion_vector` is called fresh on every CAS retry, receiving
/// that retry's current `SegmentRef` for `segment_path` — carrying
/// `row_id_base`, `row_id_count`, and the segment's current
/// `deletion_vector` (if any) in one parameter, the same role `commit`'s
/// own `build_segments` gives `next_row_id`. This is what makes concurrent
/// deletes against the same segment safe: a caller's closure MUST read the
/// `deletion_vector` it is handed (not one captured once outside this
/// call), union in its own new tombstone(s), and write that union as a
/// fresh object under an attempt-unique path — so an attempt that loses
/// the pointer CAS re-reads the winner's current state on its next
/// iteration and recomputes against it, rather than clobbering the
/// winner's write with a stale union (`spec/deletion.md` §4,
/// "Superseding, not accumulating").
///
/// # Errors
///
/// Returns [`CommitError::SegmentNotFound`] if `segment_path` does not
/// name any segment in the current snapshot (never deleted mid-retry-loop
/// — the segment either exists at read time or it doesn't; a segment
/// disappearing between retries would itself be a `SegmentNotFound` on the
/// next iteration).
pub fn commit_deletion_vector<S: ConditionalStore>(
    store: &S,
    segment_path: &str,
    build_deletion_vector: impl Fn(&SegmentRef) -> DeletionVectorRef,
) -> Result<SnapshotMetadata, CommitError> {
    loop {
        let current = read_current(store).map_err(CommitError::from_store_error)?;
        let Some(idx) = current.segments.iter().position(|s| s.path == segment_path) else {
            return Err(CommitError::SegmentNotFound(segment_path.to_string()));
        };

        let new_deletion_vector = build_deletion_vector(&current.segments[idx]);
        let version = current.version.map_or(0, |v| v + 1);
        let mut segments = current.segments.clone();
        segments[idx].deletion_vector = Some(new_deletion_vector);
        let snapshot = SnapshotMetadata {
            version,
            next_row_id: current.next_row_id,
            segments,
        };

        if let Some(committed) = propose_snapshot(store, &current, snapshot)? {
            return Ok(committed);
        }
    }
}

/// Writes `snapshot` as a new snapshot object and races the pointer CAS to
/// make it current — the shared write-and-race mechanics both `commit`
/// and `commit_deletion_vector` use (`spec/manifest.md` §2, "A second
/// commit path, same protocol, different transform"). Returns `Ok(Some
/// (snapshot))` on success, `Ok(None)` to signal "re-read current state
/// and retry" (the pointer CAS was lost, or an ambiguous pointer write
/// resolved to "did not land"), or `Err` for a definite, non-retryable
/// failure.
fn propose_snapshot<S: ConditionalStore>(
    store: &S,
    current: &CurrentState,
    snapshot: SnapshotMetadata,
) -> Result<Option<SnapshotMetadata>, CommitError> {
    let nonce = format!("{:016x}", rand_nonce());
    let snapshot_bytes = serde_json::to_vec(&snapshot).unwrap();
    let path = snapshot_key(snapshot.version, &nonce);
    match store.put_if_absent(&path, &snapshot_bytes) {
        Ok(_) => {}
        Err(StoreError::Io(msg)) => return Err(CommitError::Io(msg)),
        // No disambiguation needed here, unlike the pointer CAS below:
        // `path` is attempt-unique, so whether this write actually
        // landed or not, nothing will ever reference it under a wrong
        // assumption. Landed-but-unacked just leaves a harmless orphan
        // object (CLAUDE.md §6) once this attempt is abandoned.
        Err(StoreError::Ambiguous(msg)) => return Err(CommitError::Io(msg)),
        Err(StoreError::PreconditionFailed) => panic!(
            "snapshot path {path} collided despite the per-attempt nonce — \
             this should be statistically impossible"
        ),
    }

    let pointer_result = match &current.pointer_etag {
        Some(etag) => store.put_if_match(CURRENT_POINTER_KEY, path.as_bytes(), etag),
        None => store.put_if_absent(CURRENT_POINTER_KEY, path.as_bytes()),
    };

    match pointer_result {
        Ok(_) => Ok(Some(snapshot)),
        Err(StoreError::PreconditionFailed) => {
            // Lost the pointer CAS: another writer committed first. Caller
            // loops back to re-read the fresh current state and recompute —
            // not retry — the version and row-ID range.
            Ok(None)
        }
        Err(StoreError::Io(msg)) => {
            // Not a race — the backend itself failed. Retrying forever
            // on the assumption a rival will eventually stop contending
            // would turn a permanent outage into an infinite loop.
            Err(CommitError::Io(msg))
        }
        Err(StoreError::Ambiguous(msg)) => {
            // We don't know whether this pointer write applied before
            // the failure — the CAS itself is atomic on the backend, so
            // a plain read now resolves it completely: either our path
            // is current (the write landed, the ack was lost) or it
            // isn't (it didn't land, or a rival won first).
            match store.get(CURRENT_POINTER_KEY) {
                Ok(Some((pointer_bytes, _))) if pointer_bytes == path.as_bytes() => {
                    Ok(Some(snapshot))
                }
                Ok(_) => {
                    // Did not land. Caller loops back and recomputes against
                    // fresh state, same as losing the CAS outright.
                    Ok(None)
                }
                Err(StoreError::Io(follow_up) | StoreError::Ambiguous(follow_up)) => {
                    Err(CommitError::Io(format!(
                        "pointer write was ambiguous ({msg}) and the follow-up \
                         read to resolve it also failed: {follow_up}"
                    )))
                }
                Err(StoreError::PreconditionFailed) => {
                    unreachable!("ConditionalStore::get never returns PreconditionFailed")
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    /// The refresh-and-retry bound (RFC 0001 §3) was reached without ever
    /// landing on a snapshot the manifest could actually read.
    RetriesExhausted,
    /// The backend itself failed — not a 404 race, an actual I/O error.
    Io(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitError {
    /// The backend failed for a reason other than losing the pointer CAS —
    /// `commit`'s retry loop never treats this as "a rival writer won" and
    /// keeps looping, because a permanent outage is not a race that
    /// eventually resolves.
    Io(String),
    /// `commit_deletion_vector`'s `segment_path` named no segment in the
    /// current snapshot (`spec/deletion.md` §4) — a caller error, never a
    /// race outcome the retry loop should absorb.
    SegmentNotFound(String),
}

impl CommitError {
    fn from_store_error(e: StoreError) -> CommitError {
        match e {
            // A `get` has no side effect to reconcile, so per `StoreError`'s
            // own doc comment a conforming `get` treats ambiguity the same
            // as a definite failure here.
            StoreError::Io(msg) | StoreError::Ambiguous(msg) => CommitError::Io(msg),
            StoreError::PreconditionFailed => {
                unreachable!("ConditionalStore::get never returns PreconditionFailed")
            }
        }
    }
}

/// The reader protocol, RFC 0001 §3 steps 1–2: resolve the current pointer,
/// then read the snapshot it names. Returns `Ok(None)` for a table with no
/// commits yet. On a 404 race (the pointer named a snapshot compaction has
/// since removed), refreshes and retries, bounded — an unbounded retry loop
/// is not a conforming reader, so past the limit this returns
/// `Err(ReadError::RetriesExhausted)` rather than looping forever.
pub fn read_snapshot<S: ConditionalStore>(
    store: &S,
) -> Result<Option<SnapshotMetadata>, ReadError> {
    for _ in 0..=READER_REFRESH_RETRY_LIMIT {
        match try_read_current(store) {
            Ok(ReadAttempt::NoCommitsYet) => return Ok(None),
            Ok(ReadAttempt::Found(state)) => {
                return Ok(Some(SnapshotMetadata {
                    version: state.version.expect("Found always carries a version"),
                    next_row_id: state.next_row_id,
                    segments: state.segments,
                }));
            }
            Ok(ReadAttempt::Expired) => continue,
            Err(StoreError::Io(msg) | StoreError::Ambiguous(msg)) => {
                return Err(ReadError::Io(msg));
            }
            Err(StoreError::PreconditionFailed) => {
                unreachable!("ConditionalStore::get never returns PreconditionFailed")
            }
        }
    }
    Err(ReadError::RetriesExhausted)
}

fn rand_nonce() -> u64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    RandomState::new().build_hasher().finish()
}

/// One segment referenced by a snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentRef {
    pub path: String,
    pub row_id_base: u64,
    pub row_id_count: u64,
    pub byte_length: u64,
    pub checksum: u64,
    /// Absent iff no row in this segment has ever been deleted (`spec/
    /// deletion.md` §3, RFC 0012).
    pub deletion_vector: Option<DeletionVectorRef>,
}

/// The immutable, versioned body of a proposed or committed snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub version: u64,
    pub next_row_id: u64,
    pub segments: Vec<SegmentRef>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::InMemoryStore;

    #[test]
    fn snapshot_metadata_round_trips_through_json() {
        let snapshot = SnapshotMetadata {
            version: 0,
            next_row_id: 2,
            segments: vec![SegmentRef {
                path: "segments/a.bin".to_string(),
                row_id_base: 0,
                row_id_count: 2,
                byte_length: 102,
                checksum: 0xdead_beef,
                deletion_vector: None,
            }],
        };

        let json = serde_json::to_vec(&snapshot).unwrap();
        let decoded: SnapshotMetadata = serde_json::from_slice(&json).unwrap();

        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn commit_against_fresh_table_creates_first_snapshot() {
        let store = InMemoryStore::new();

        let committed = commit(&store, |next_row_id| {
            vec![SegmentRef {
                path: "segments/a.bin".to_string(),
                row_id_base: next_row_id,
                row_id_count: 2,
                byte_length: 102,
                checksum: 0xdead_beef,
                deletion_vector: None,
            }]
        })
        .unwrap();

        assert_eq!(committed.version, 0);
        assert_eq!(committed.next_row_id, 2);
        assert_eq!(committed.segments.len(), 1);
        assert_eq!(committed.segments[0].row_id_base, 0);
    }

    #[test]
    fn second_commit_advances_version_and_appends_to_prior_segments() {
        let store = InMemoryStore::new();
        commit(&store, |next_row_id| {
            vec![SegmentRef {
                path: "segments/a.bin".to_string(),
                row_id_base: next_row_id,
                row_id_count: 2,
                byte_length: 102,
                checksum: 0xdead_beef,
                deletion_vector: None,
            }]
        })
        .unwrap();

        let committed = commit(&store, |next_row_id| {
            vec![SegmentRef {
                path: "segments/b.bin".to_string(),
                row_id_base: next_row_id,
                row_id_count: 3,
                byte_length: 200,
                checksum: 0xf00d,
                deletion_vector: None,
            }]
        })
        .unwrap();

        assert_eq!(committed.version, 1);
        assert_eq!(committed.next_row_id, 5);
        assert_eq!(committed.segments.len(), 2);
        assert_eq!(committed.segments[0].path, "segments/a.bin");
        assert_eq!(committed.segments[1].path, "segments/b.bin");
        assert_eq!(committed.segments[1].row_id_base, 2);
    }

    /// The scenario the adversarial review of RFC 0001 actually found broken:
    /// a writer's `build_segments` is called once, computes a row-ID range
    /// against a base that is about to go stale, and — before this writer's
    /// pointer CAS lands — a second writer commits first. A buggy `commit`
    /// that reused the first, now-stale range instead of recomputing it would
    /// commit two segments claiming overlapping row-IDs.
    #[test]
    fn commit_recomputes_row_id_range_when_a_rival_commits_first() {
        let store = InMemoryStore::new();
        let rival_injected = std::cell::Cell::new(false);

        let committed = commit(&store, |next_row_id| {
            if !rival_injected.get() {
                rival_injected.set(true);
                // Simulate a second writer's whole commit landing between
                // this attempt's read and its own pointer CAS.
                commit(&store, |rival_next_row_id| {
                    vec![SegmentRef {
                        path: "segments/rival.bin".to_string(),
                        row_id_base: rival_next_row_id,
                        row_id_count: 5,
                        byte_length: 500,
                        checksum: 0xbad,
                        deletion_vector: None,
                    }]
                })
                .unwrap();
            }
            vec![SegmentRef {
                path: "segments/mine.bin".to_string(),
                row_id_base: next_row_id,
                row_id_count: 2,
                byte_length: 102,
                checksum: 0xdead_beef,
                deletion_vector: None,
            }]
        })
        .unwrap();

        assert_eq!(committed.version, 1, "the rival took version 0");
        assert_eq!(committed.segments.len(), 2);
        let rival = &committed.segments[0];
        let mine = &committed.segments[1];
        assert_eq!(rival.path, "segments/rival.bin");
        assert_eq!(mine.path, "segments/mine.bin");

        let rival_range = rival.row_id_base..(rival.row_id_base + rival.row_id_count);
        let mine_range = mine.row_id_base..(mine.row_id_base + mine.row_id_count);
        assert!(
            rival_range.end <= mine_range.start || mine_range.end <= rival_range.start,
            "row-ID ranges must not overlap: rival {rival_range:?}, mine {mine_range:?}"
        );
        assert_eq!(
            mine.row_id_base, 5,
            "recomputed past the rival's range, not reused"
        );
    }

    fn commit_one_segment(store: &InMemoryStore, path: &str, row_id_count: u64) -> SegmentRef {
        let path = path.to_string();
        let committed = commit(store, |next_row_id| {
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
        committed.segments.into_iter().next().unwrap()
    }

    #[test]
    fn commit_deletion_vector_updates_an_existing_segments_reference() {
        let store = InMemoryStore::new();
        let seg = commit_one_segment(&store, "segments/a.bin", 200);

        let committed = commit_deletion_vector(&store, &seg.path, |current| {
            assert_eq!(current.path, seg.path);
            assert!(
                current.deletion_vector.is_none(),
                "no deletes yet against this segment"
            );
            let mut bitmap = roaring::RoaringBitmap::new();
            bitmap.insert(2);
            let bytes =
                crate::deletion::build_deletion_vector(&bitmap, current.row_id_count).unwrap();
            let checksum = crate::deletion::checksum(&bytes);
            store.put_if_absent("deletions/a-0.bin", &bytes).unwrap();
            crate::deletion::DeletionVectorRef {
                path: "deletions/a-0.bin".to_string(),
                byte_length: bytes.len() as u64,
                checksum,
            }
        })
        .unwrap();

        assert_eq!(committed.version, 1, "seg 0 took version 0");
        assert_eq!(committed.next_row_id, 200, "unchanged — no new rows");
        assert_eq!(
            committed.segments.len(),
            1,
            "no segment appended or removed"
        );
        let dv_ref = committed.segments[0]
            .deletion_vector
            .as_ref()
            .expect("deletion_vector must now be set");
        assert_eq!(dv_ref.path, "deletions/a-0.bin");

        let dv = crate::deletion::read(&store, dv_ref).unwrap();
        assert!(dv.is_deleted(seg.row_id_base + 2, seg.row_id_base));
        assert!(!dv.is_deleted(seg.row_id_base + 3, seg.row_id_base));
    }

    #[test]
    fn commit_deletion_vector_reports_segment_not_found_for_an_unknown_path() {
        let store = InMemoryStore::new();
        commit_one_segment(&store, "segments/a.bin", 10);

        let err = commit_deletion_vector(&store, "segments/does-not-exist.bin", |_| {
            panic!("must not be called: segment_path doesn't exist")
        })
        .unwrap_err();
        assert_eq!(
            err,
            CommitError::SegmentNotFound("segments/does-not-exist.bin".to_string())
        );
    }

    #[test]
    fn commit_deletion_vector_recomputes_against_a_concurrent_rivals_write_without_losing_a_tombstone()
     {
        // The exact race RFC 0012's own adversarial review named: two
        // concurrent deletes against the same segment must not let the
        // loser's write clobber the winner's tombstone. Mirrors
        // `commit_recomputes_row_id_range_when_a_rival_commits_first`'s own
        // rival-injection pattern.
        let store = InMemoryStore::new();
        let seg = commit_one_segment(&store, "segments/a.bin", 200);
        let rival_injected = std::cell::Cell::new(false);
        let rival_bin_counter = std::cell::Cell::new(0u32);

        let write_dv = |store: &InMemoryStore, ordinals: &[u32], suffix: u32| {
            let mut bitmap = roaring::RoaringBitmap::new();
            for &o in ordinals {
                bitmap.insert(o);
            }
            let bytes = crate::deletion::build_deletion_vector(&bitmap, 200).unwrap();
            let checksum = crate::deletion::checksum(&bytes);
            let path = format!("deletions/a-{suffix}.bin");
            store.put_if_absent(&path, &bytes).unwrap();
            crate::deletion::DeletionVectorRef {
                path,
                byte_length: bytes.len() as u64,
                checksum,
            }
        };

        let committed = commit_deletion_vector(&store, &seg.path, |current| {
            if !rival_injected.get() {
                rival_injected.set(true);
                // Simulate a second writer's whole commit_deletion_vector
                // call landing between this attempt's read and its own
                // pointer CAS — it tombstones ordinal 5.
                commit_deletion_vector(&store, &seg.path, |rival_current| {
                    assert!(rival_current.deletion_vector.is_none());
                    write_dv(&store, &[5], 900)
                })
                .unwrap();
            }
            // This closure re-runs on retry with the now-current state,
            // which (after the rival landed) already tombstones 5 — a
            // correct implementation unions its own new tombstone (7) into
            // that, not into stale pre-race state.
            let mut existing = match &current.deletion_vector {
                Some(dv_ref) => crate::deletion::read(&store, dv_ref)
                    .unwrap()
                    .clone_bitmap(),
                None => roaring::RoaringBitmap::new(),
            };
            existing.insert(7);
            rival_bin_counter.set(rival_bin_counter.get() + 1);
            write_dv(
                &store,
                &existing.iter().collect::<Vec<_>>(),
                rival_bin_counter.get(),
            )
        })
        .unwrap();

        let dv_ref = committed.segments[0].deletion_vector.as_ref().unwrap();
        let dv = crate::deletion::read(&store, dv_ref).unwrap();
        assert!(
            dv.is_deleted(seg.row_id_base + 5, seg.row_id_base),
            "the rival's tombstone must survive"
        );
        assert!(
            dv.is_deleted(seg.row_id_base + 7, seg.row_id_base),
            "this attempt's own tombstone must also be present"
        );
    }

    #[test]
    fn read_snapshot_returns_none_for_a_table_with_no_commits() {
        let store = InMemoryStore::new();

        assert_eq!(read_snapshot(&store), Ok(None));
    }

    #[test]
    fn read_snapshot_returns_the_latest_committed_snapshot() {
        let store = InMemoryStore::new();
        commit(&store, |next_row_id| {
            vec![SegmentRef {
                path: "segments/a.bin".to_string(),
                row_id_base: next_row_id,
                row_id_count: 2,
                byte_length: 102,
                checksum: 0xdead_beef,
                deletion_vector: None,
            }]
        })
        .unwrap();
        let second = commit(&store, |next_row_id| {
            vec![SegmentRef {
                path: "segments/b.bin".to_string(),
                row_id_base: next_row_id,
                row_id_count: 3,
                byte_length: 200,
                checksum: 0xf00d,
                deletion_vector: None,
            }]
        })
        .unwrap();

        assert_eq!(read_snapshot(&store), Ok(Some(second)));
    }

    /// A `ConditionalStore` wrapper that runs a hook before delegating each
    /// `get` call, letting a test inject a store mutation at the exact point
    /// a reader is mid-sequence — modeling the 404 race RFC 0001 §3
    /// describes: compaction removes a snapshot object between a reader's
    /// pointer read and its follow-up fetch of what that pointer named.
    struct FaultInjectingStore<'a, F: Fn(&str)> {
        inner: &'a InMemoryStore,
        on_get: F,
    }

    impl<F: Fn(&str)> ConditionalStore for FaultInjectingStore<'_, F> {
        fn get(
            &self,
            key: &str,
        ) -> Result<Option<(Vec<u8>, crate::store::ETag)>, crate::store::StoreError> {
            (self.on_get)(key);
            self.inner.get(key)
        }

        fn put_if_absent(
            &self,
            key: &str,
            bytes: &[u8],
        ) -> Result<crate::store::ETag, crate::store::StoreError> {
            self.inner.put_if_absent(key, bytes)
        }

        fn put_if_match(
            &self,
            key: &str,
            bytes: &[u8],
            etag: &crate::store::ETag,
        ) -> Result<crate::store::ETag, crate::store::StoreError> {
            self.inner.put_if_match(key, bytes, etag)
        }
    }

    #[test]
    fn read_snapshot_refreshes_and_retries_past_a_compacted_snapshot() {
        let inner = InMemoryStore::new();
        let first = commit(&inner, |next_row_id| {
            vec![SegmentRef {
                path: "segments/a.bin".to_string(),
                row_id_base: next_row_id,
                row_id_count: 2,
                byte_length: 102,
                checksum: 0xdead_beef,
                deletion_vector: None,
            }]
        })
        .unwrap();
        let first_snapshot_key = snapshot_key_for_test(&inner, first.version);
        let already_fired = std::cell::Cell::new(false);

        let store = FaultInjectingStore {
            inner: &inner,
            on_get: |key: &str| {
                if key == first_snapshot_key && !already_fired.get() {
                    already_fired.set(true);
                    // Simulate: by the time the reader tries to fetch the
                    // snapshot its pointer read just named, compaction has
                    // removed it — because a newer commit already landed and
                    // moved the pointer past it. Deletion-safety (CLAUDE.md
                    // §6) never removes a file the current pointer still
                    // references, so the new commit must land first.
                    commit(&inner, |next_row_id| {
                        vec![SegmentRef {
                            path: "segments/b.bin".to_string(),
                            row_id_base: next_row_id,
                            row_id_count: 3,
                            byte_length: 200,
                            checksum: 0xf00d,
                            deletion_vector: None,
                        }]
                    })
                    .unwrap();
                    inner.delete(&first_snapshot_key);
                }
            },
        };

        let result = read_snapshot(&store).unwrap().unwrap();

        assert_eq!(result.version, 1, "recovered onto the newer snapshot");
        assert_eq!(result.segments.len(), 2);
    }

    #[test]
    fn read_snapshot_gives_up_after_the_retry_limit_instead_of_looping_forever() {
        let inner = InMemoryStore::new();
        commit(&inner, |next_row_id| {
            vec![SegmentRef {
                path: "segments/a.bin".to_string(),
                row_id_base: next_row_id,
                row_id_count: 2,
                byte_length: 102,
                checksum: 0xdead_beef,
                deletion_vector: None,
            }]
        })
        .unwrap();

        let store = FaultInjectingStore {
            inner: &inner,
            on_get: |key: &str| {
                // Every snapshot fetch finds the object already compacted,
                // forever — the pathological case a bound must survive.
                if key.starts_with("_strand/snapshots/") {
                    inner.delete(key);
                }
            },
        };

        assert_eq!(read_snapshot(&store), Err(ReadError::RetriesExhausted));
    }

    /// Property-based coverage extending the hand-picked scenarios above:
    /// randomized sequences of concurrent-writer rounds, checked against the
    /// protocol's real safety invariants rather than one scenario's expected
    /// values. This is the alternative RFC 0002's adversarial review named as
    /// the thing to build and evaluate before a TLA+/DST effort is justified
    /// (`rfcs/0002-manifest-formal-verification.md`).
    mod property {
        use super::*;
        use proptest::prelude::*;
        use std::sync::atomic::{AtomicU64, Ordering};

        fn unique_path() -> String {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            format!("segments/{:x}.bin", COUNTER.fetch_add(1, Ordering::Relaxed))
        }

        fn toy_segment(row_id_base: u64, row_id_count: u64) -> SegmentRef {
            SegmentRef {
                path: unique_path(),
                row_id_base,
                row_id_count,
                byte_length: 1,
                checksum: 0,
                deletion_vector: None,
            }
        }

        /// One round's shape: how many real, top-level `commit()` calls it
        /// produces, and what happens to the round's *last* writer's pointer
        /// CAS.
        #[derive(Debug, Clone, Copy)]
        enum RoundPlan {
            /// No contention, no fault: an ordinary successful commit.
            Solo,
            /// A second writer's whole commit actually lands, for real,
            /// before this round's writer attempts its own pointer CAS —
            /// the genuine `PreconditionFailed` race.
            RivalWins,
            /// This round's pointer write is reported `Ambiguous`, but it
            /// actually landed (the ack was lost).
            AmbiguousLanded,
            /// This round's pointer write is reported `Ambiguous` and did
            /// not land (the request never reached the server).
            AmbiguousNotLanded,
        }

        fn round_plan() -> impl Strategy<Value = RoundPlan> {
            prop_oneof![
                Just(RoundPlan::Solo),
                Just(RoundPlan::RivalWins),
                Just(RoundPlan::AmbiguousLanded),
                Just(RoundPlan::AmbiguousNotLanded),
            ]
        }

        /// Runs one round against the shared `inner` store and returns how
        /// many top-level, successful `commit()` calls it performed (1,
        /// except `RivalWins`, which performs 2).
        fn run_round(inner: &InMemoryStore, plan: RoundPlan, row_id_count: u64) -> u32 {
            match plan {
                RoundPlan::Solo => {
                    commit(inner, |next_row_id| {
                        vec![toy_segment(next_row_id, row_id_count)]
                    })
                    .unwrap();
                    1
                }
                RoundPlan::RivalWins => {
                    let rival_fired = std::cell::Cell::new(false);
                    commit(inner, |next_row_id| {
                        if !rival_fired.get() {
                            rival_fired.set(true);
                            commit(inner, |rival_next_row_id| {
                                vec![toy_segment(rival_next_row_id, 1)]
                            })
                            .unwrap();
                        }
                        vec![toy_segment(next_row_id, row_id_count)]
                    })
                    .unwrap();
                    2
                }
                RoundPlan::AmbiguousLanded => {
                    let store = AmbiguousThenNormalPointerWrite {
                        inner,
                        already_fired: std::cell::Cell::new(false),
                    };
                    commit(&store, |next_row_id| {
                        vec![toy_segment(next_row_id, row_id_count)]
                    })
                    .unwrap();
                    1
                }
                RoundPlan::AmbiguousNotLanded => {
                    let store = AmbiguousAndNeverAppliedPointerWrite {
                        inner,
                        already_fired: std::cell::Cell::new(false),
                    };
                    commit(&store, |next_row_id| {
                        vec![toy_segment(next_row_id, row_id_count)]
                    })
                    .unwrap();
                    1
                }
            }
        }

        proptest! {
            #[test]
            fn commit_invariants_hold_across_randomized_concurrent_rounds(
                rounds in prop::collection::vec((round_plan(), 1u64..5), 1..8),
            ) {
                let inner = InMemoryStore::new();
                let mut expected_commit_count: u32 = 0;
                for (plan, row_id_count) in rounds {
                    expected_commit_count += run_round(&inner, plan, row_id_count);
                }

                let final_snapshot = read_snapshot(&inner).unwrap().unwrap();

                prop_assert_eq!(
                    final_snapshot.version,
                    u64::from(expected_commit_count) - 1,
                    "version must count exactly the successful commits, no more, no fewer"
                );
                prop_assert_eq!(
                    final_snapshot.segments.len(),
                    expected_commit_count as usize
                );

                let mut ranges: Vec<_> = final_snapshot
                    .segments
                    .iter()
                    .map(|s| s.row_id_base..(s.row_id_base + s.row_id_count))
                    .collect();
                ranges.sort_by_key(|r| r.start);
                for pair in ranges.windows(2) {
                    prop_assert!(
                        pair[0].end <= pair[1].start,
                        "row-ID ranges must not overlap: {:?} vs {:?}",
                        pair[0],
                        pair[1]
                    );
                }

                let total_row_ids: u64 = final_snapshot.segments.iter().map(|s| s.row_id_count).sum();
                prop_assert_eq!(final_snapshot.next_row_id, total_row_ids);
            }
        }
    }

    fn snapshot_key_for_test(store: &InMemoryStore, version: u64) -> String {
        let (pointer_bytes, _) = store.get(CURRENT_POINTER_KEY).unwrap().unwrap();
        let path = String::from_utf8(pointer_bytes).unwrap();
        assert!(path.starts_with(&format!("_strand/snapshots/{version:020}-")));
        path
    }

    /// A `ConditionalStore` every one of whose operations fails with
    /// `StoreError::Io` — the persistent-outage case, as opposed to the
    /// transient, self-resolving races the other test doubles model.
    struct AlwaysFailingStore;

    impl ConditionalStore for AlwaysFailingStore {
        fn get(
            &self,
            _key: &str,
        ) -> Result<Option<(Vec<u8>, crate::store::ETag)>, crate::store::StoreError> {
            Err(crate::store::StoreError::Io(
                "simulated network failure".into(),
            ))
        }

        fn put_if_absent(
            &self,
            _key: &str,
            _bytes: &[u8],
        ) -> Result<crate::store::ETag, crate::store::StoreError> {
            Err(crate::store::StoreError::Io(
                "simulated network failure".into(),
            ))
        }

        fn put_if_match(
            &self,
            _key: &str,
            _bytes: &[u8],
            _etag: &crate::store::ETag,
        ) -> Result<crate::store::ETag, crate::store::StoreError> {
            Err(crate::store::StoreError::Io(
                "simulated network failure".into(),
            ))
        }
    }

    #[test]
    fn read_snapshot_surfaces_io_errors_instead_of_panicking() {
        let store = AlwaysFailingStore;

        let result = read_snapshot(&store);

        assert!(matches!(result, Err(ReadError::Io(_))), "{result:?}");
    }

    #[test]
    fn commit_surfaces_io_errors_from_the_initial_read_instead_of_panicking() {
        let store = AlwaysFailingStore;

        let result = commit(&store, |next_row_id| {
            vec![SegmentRef {
                path: "segments/a.bin".to_string(),
                row_id_base: next_row_id,
                row_id_count: 2,
                byte_length: 102,
                checksum: 0xdead_beef,
                deletion_vector: None,
            }]
        });

        assert!(matches!(result, Err(CommitError::Io(_))), "{result:?}");
    }

    /// A store that behaves normally except that writes to the current
    /// pointer always fail with `StoreError::Io` — modeling a persistent
    /// backend outage discovered exactly at the CAS step, as opposed to
    /// `StoreError::PreconditionFailed`, which means a rival writer won.
    /// `commit`'s retry loop must tell these apart: looping forever on a
    /// permanent Io failure, mistaking it for a rival that will eventually
    /// stop racing, is a real bug, not a hypothetical one — this is the
    /// same class of gap the reader's 404-refresh bound closed for reads.
    struct FailingPointerWrites<'a> {
        inner: &'a InMemoryStore,
    }

    impl ConditionalStore for FailingPointerWrites<'_> {
        fn get(
            &self,
            key: &str,
        ) -> Result<Option<(Vec<u8>, crate::store::ETag)>, crate::store::StoreError> {
            self.inner.get(key)
        }

        fn put_if_absent(
            &self,
            key: &str,
            bytes: &[u8],
        ) -> Result<crate::store::ETag, crate::store::StoreError> {
            if key == CURRENT_POINTER_KEY {
                return Err(crate::store::StoreError::Io("simulated outage".into()));
            }
            self.inner.put_if_absent(key, bytes)
        }

        fn put_if_match(
            &self,
            key: &str,
            bytes: &[u8],
            etag: &crate::store::ETag,
        ) -> Result<crate::store::ETag, crate::store::StoreError> {
            if key == CURRENT_POINTER_KEY {
                return Err(crate::store::StoreError::Io("simulated outage".into()));
            }
            self.inner.put_if_match(key, bytes, etag)
        }
    }

    /// A store that behaves normally but reports the pointer CAS as
    /// `StoreError::Ambiguous` on its first attempt — the write is still
    /// forwarded to (and actually applied against) the inner store, exactly
    /// like a request whose response was lost after the server processed
    /// it. Models the case `commit` must resolve rather than blindly retry.
    struct AmbiguousThenNormalPointerWrite<'a> {
        inner: &'a InMemoryStore,
        already_fired: std::cell::Cell<bool>,
    }

    impl ConditionalStore for AmbiguousThenNormalPointerWrite<'_> {
        fn get(
            &self,
            key: &str,
        ) -> Result<Option<(Vec<u8>, crate::store::ETag)>, crate::store::StoreError> {
            self.inner.get(key)
        }

        fn put_if_absent(
            &self,
            key: &str,
            bytes: &[u8],
        ) -> Result<crate::store::ETag, crate::store::StoreError> {
            let result = self.inner.put_if_absent(key, bytes);
            if key == CURRENT_POINTER_KEY && !self.already_fired.get() {
                self.already_fired.set(true);
                return Err(crate::store::StoreError::Ambiguous(
                    "simulated: response lost after the write applied".into(),
                ));
            }
            result
        }

        fn put_if_match(
            &self,
            key: &str,
            bytes: &[u8],
            etag: &crate::store::ETag,
        ) -> Result<crate::store::ETag, crate::store::StoreError> {
            let result = self.inner.put_if_match(key, bytes, etag);
            if key == CURRENT_POINTER_KEY && !self.already_fired.get() {
                self.already_fired.set(true);
                return Err(crate::store::StoreError::Ambiguous(
                    "simulated: response lost after the write applied".into(),
                ));
            }
            result
        }
    }

    #[test]
    fn commit_resolves_an_ambiguous_pointer_write_that_actually_landed_as_success() {
        let inner = InMemoryStore::new();
        let store = AmbiguousThenNormalPointerWrite {
            inner: &inner,
            already_fired: std::cell::Cell::new(false),
        };

        let committed = commit(&store, |next_row_id| {
            vec![SegmentRef {
                path: "segments/a.bin".to_string(),
                row_id_base: next_row_id,
                row_id_count: 2,
                byte_length: 102,
                checksum: 0xdead_beef,
                deletion_vector: None,
            }]
        })
        .unwrap();

        assert_eq!(
            committed.version, 0,
            "the ambiguous write was the real commit"
        );
        assert_eq!(committed.segments.len(), 1);
        // No duplicate: a second, unnecessary commit built on top of the
        // (actually-successful) first one would show up as version 1 here.
        assert_eq!(read_snapshot(&inner), Ok(Some(committed)));
    }

    /// The other resolution of the same ambiguity: the pointer write is
    /// reported `Ambiguous` but never actually reaches the inner store, the
    /// same way a request that failed before the server ever saw it would.
    /// `commit` must not mistake the earlier `Ambiguous` for success.
    struct AmbiguousAndNeverAppliedPointerWrite<'a> {
        inner: &'a InMemoryStore,
        already_fired: std::cell::Cell<bool>,
    }

    impl ConditionalStore for AmbiguousAndNeverAppliedPointerWrite<'_> {
        fn get(
            &self,
            key: &str,
        ) -> Result<Option<(Vec<u8>, crate::store::ETag)>, crate::store::StoreError> {
            self.inner.get(key)
        }

        fn put_if_absent(
            &self,
            key: &str,
            bytes: &[u8],
        ) -> Result<crate::store::ETag, crate::store::StoreError> {
            if key == CURRENT_POINTER_KEY && !self.already_fired.get() {
                self.already_fired.set(true);
                return Err(crate::store::StoreError::Ambiguous(
                    "simulated: request never reached the server".into(),
                ));
            }
            self.inner.put_if_absent(key, bytes)
        }

        fn put_if_match(
            &self,
            key: &str,
            bytes: &[u8],
            etag: &crate::store::ETag,
        ) -> Result<crate::store::ETag, crate::store::StoreError> {
            if key == CURRENT_POINTER_KEY && !self.already_fired.get() {
                self.already_fired.set(true);
                return Err(crate::store::StoreError::Ambiguous(
                    "simulated: request never reached the server".into(),
                ));
            }
            self.inner.put_if_match(key, bytes, etag)
        }
    }

    #[test]
    fn commit_retries_an_ambiguous_pointer_write_that_never_landed() {
        let inner = InMemoryStore::new();
        let store = AmbiguousAndNeverAppliedPointerWrite {
            inner: &inner,
            already_fired: std::cell::Cell::new(false),
        };

        let committed = commit(&store, |next_row_id| {
            vec![SegmentRef {
                path: "segments/a.bin".to_string(),
                row_id_base: next_row_id,
                row_id_count: 2,
                byte_length: 102,
                checksum: 0xdead_beef,
                deletion_vector: None,
            }]
        })
        .unwrap();

        assert_eq!(committed.version, 0);
        assert_eq!(read_snapshot(&inner), Ok(Some(committed)));
    }

    #[test]
    fn commit_surfaces_io_errors_from_a_failing_pointer_cas_instead_of_looping_forever() {
        let inner = InMemoryStore::new();
        let store = FailingPointerWrites { inner: &inner };

        let result = commit(&store, |next_row_id| {
            vec![SegmentRef {
                path: "segments/a.bin".to_string(),
                row_id_base: next_row_id,
                row_id_count: 2,
                byte_length: 102,
                checksum: 0xdead_beef,
                deletion_vector: None,
            }]
        });

        assert!(matches!(result, Err(CommitError::Io(_))), "{result:?}");
    }
}
