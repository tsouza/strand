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
//! (`rfcs/0001-container-rowid-manifest.md`).

use crate::store::ConditionalStore;
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

fn read_current<S: ConditionalStore>(store: &S) -> CurrentState {
    let Some((pointer_bytes, pointer_etag)) = store.get(CURRENT_POINTER_KEY) else {
        return CurrentState {
            version: None,
            next_row_id: 0,
            segments: Vec::new(),
            pointer_etag: None,
        };
    };
    let snapshot_path = String::from_utf8(pointer_bytes).expect("pointer content is UTF-8");
    let (snapshot_bytes, _) = store
        .get(&snapshot_path)
        .expect("pointer target must exist");
    let snapshot: SnapshotMetadata =
        serde_json::from_slice(&snapshot_bytes).expect("snapshot content is valid JSON");
    CurrentState {
        version: Some(snapshot.version),
        next_row_id: snapshot.next_row_id,
        segments: snapshot.segments,
        pointer_etag: Some(pointer_etag),
    }
}

/// Commits `build_segments`'s output as a new snapshot, retrying per RFC 0001
/// §3 if another writer's commit wins the pointer CAS first. `build_segments`
/// is called fresh on every attempt, including retries, with the row-ID base
/// this attempt should build against — it is never reused across attempts,
/// which is what makes a stale, overlapping row-ID range structurally
/// impossible rather than a discipline the caller must maintain.
pub fn commit<S: ConditionalStore>(
    store: &S,
    build_segments: impl Fn(u64) -> Vec<SegmentRef>,
) -> SnapshotMetadata {
    loop {
        let current = read_current(store);
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

        let nonce = format!("{:016x}", rand_nonce());
        let snapshot_bytes = serde_json::to_vec(&snapshot).unwrap();
        let path = snapshot_key(version, &nonce);
        store
            .put_if_absent(&path, &snapshot_bytes)
            .expect("snapshot path is unique per attempt and must not collide");

        let pointer_result = match &current.pointer_etag {
            Some(etag) => store.put_if_match(CURRENT_POINTER_KEY, path.as_bytes(), etag),
            None => store.put_if_absent(CURRENT_POINTER_KEY, path.as_bytes()),
        };

        if pointer_result.is_ok() {
            return snapshot;
        }
        // Lost the pointer CAS: another writer committed first. Loop back to
        // re-read the fresh current state and recompute — not retry — the
        // version and row-ID range.
    }
}

/// The reader protocol, RFC 0001 §3 steps 1–2: resolve the current pointer,
/// then read the snapshot it names. Returns `None` for a table with no
/// commits yet.
pub fn read_snapshot<S: ConditionalStore>(store: &S) -> Option<SnapshotMetadata> {
    let current = read_current(store);
    let version = current.version?;
    Some(SnapshotMetadata {
        version,
        next_row_id: current.next_row_id,
        segments: current.segments,
    })
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
            }]
        });

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
            }]
        });

        let committed = commit(&store, |next_row_id| {
            vec![SegmentRef {
                path: "segments/b.bin".to_string(),
                row_id_base: next_row_id,
                row_id_count: 3,
                byte_length: 200,
                checksum: 0xf00d,
            }]
        });

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
                    }]
                });
            }
            vec![SegmentRef {
                path: "segments/mine.bin".to_string(),
                row_id_base: next_row_id,
                row_id_count: 2,
                byte_length: 102,
                checksum: 0xdead_beef,
            }]
        });

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

    #[test]
    fn read_snapshot_returns_none_for_a_table_with_no_commits() {
        let store = InMemoryStore::new();

        assert_eq!(read_snapshot(&store), None);
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
            }]
        });
        let second = commit(&store, |next_row_id| {
            vec![SegmentRef {
                path: "segments/b.bin".to_string(),
                row_id_base: next_row_id,
                row_id_count: 3,
                byte_length: 200,
                checksum: 0xf00d,
            }]
        });

        assert_eq!(read_snapshot(&store), Some(second));
    }
}
