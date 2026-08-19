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

//! An object store abstraction carrying exactly the conditional-write
//! semantics RFC 0001 §3's commit protocol depends on: `If-None-Match: *`
//! (create-if-absent) and `If-Match: <etag>` (compare-and-swap). Implemented
//! here as an in-memory double for protocol-logic tests; a real backend
//! (S3, MinIO) implements the same trait.

use std::collections::HashMap;
use std::sync::Mutex;

/// An opaque, store-assigned version tag for an object, used for CAS.
pub type ETag = String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// The `If-None-Match: *` or `If-Match` precondition was not met. A
    /// definite outcome: the backend told us, plainly, that the write did
    /// not apply.
    PreconditionFailed,
    /// The backend failed in a way that is definite: either the request was
    /// never dispatched (so it certainly did not apply), or a well-formed
    /// error response came back from the service. Kept distinct from
    /// `Ok(None)` deliberately: conflating the two would mean a transient
    /// failure gets treated as an expired-snapshot 404 by the reader retry
    /// logic in `manifest.rs`, silently masking a real error as a routine
    /// compaction race.
    Io(String),
    /// The backend failed in a way that leaves the write's outcome unknown:
    /// a timeout, a dropped connection, a response that stopped arriving
    /// mid-stream. The request may have been sent and processed before the
    /// failure occurred — there is no guarantee it did not apply. Kept
    /// distinct from `Io` because the two demand different caller behavior:
    /// `Io` means "this definitely did not happen, retry is safe"; a caller
    /// treating `Ambiguous` the same way risks silently duplicating a write
    /// that actually landed. A caller mutating state through a conditional
    /// write (`put_if_absent`/`put_if_match`) MUST resolve this by reading
    /// the key back and checking whether its own write is the one present,
    /// rather than assuming failure and retrying blind. `get` and `delete`
    /// have no side effect to reconcile and MAY treat this the same as
    /// `Io`.
    Ambiguous(String),
}

pub trait ConditionalStore {
    /// Returns the object's bytes and current ETag, or `Ok(None)` if the
    /// key is genuinely absent. A backend failure is `Err`, not `Ok(None)`.
    fn get(&self, key: &str) -> Result<Option<(Vec<u8>, ETag)>, StoreError>;

    /// Creates `key` with `bytes`, only if `key` does not already exist.
    fn put_if_absent(&self, key: &str, bytes: &[u8]) -> Result<ETag, StoreError>;

    /// Overwrites `key` with `bytes`, only if its current ETag is `etag`.
    fn put_if_match(&self, key: &str, bytes: &[u8], etag: &ETag) -> Result<ETag, StoreError>;
}

/// A store capable of fetching a byte sub-range of an object without
/// downloading the whole thing — the capability a conforming cold-open
/// reader needs for invariant 3's one-wave rule (`CLAUDE.md` §7: "every
/// byte range a cold query may need MUST be addressable... issue each
/// fetch stage as one parallel wave"). Kept as a separate trait rather
/// than a new required method on `ConditionalStore`, deliberately: the
/// manifest CAS protocol (RFC 0001 §3) never performs a partial read, so
/// adding it to `ConditionalStore` would force every fault-injection test
/// double in `manifest.rs` and `tests/s3_store.rs` to grow a range-GET stub
/// it would never exercise, for no protocol reason.
pub trait RangeGetStore {
    /// Fetches bytes `[start, end)` of `key` — a half-open range, matching
    /// Rust's `Range` convention rather than HTTP's inclusive-end `Range`
    /// header; implementations translate at the boundary. `start == end` is
    /// a caller bug (an empty range is never a real fetch stage) and MAY
    /// panic; `end` past the object's length behaves per the backend's own
    /// short-read semantics (S3/MinIO clamp to the object's actual end).
    fn get_range(&self, key: &str, start: u64, end: u64) -> Result<Vec<u8>, StoreError>;
}

/// An in-memory `ConditionalStore`. ETags are a monotonic counter per key,
/// which is sufficient to detect staleness — the real property CAS depends
/// on — without imitating any particular backend's ETag format.
#[derive(Default)]
pub struct InMemoryStore {
    objects: Mutex<HashMap<String, (Vec<u8>, u64)>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Unconditionally removes `key`. Not part of `ConditionalStore`: no
    /// reader or writer in the commit protocol ever deletes an object: this
    /// exists for tests that simulate compaction, and later for the M3
    /// orphan-sweep tool.
    pub fn delete(&self, key: &str) {
        self.objects.lock().unwrap().remove(key);
    }
}

impl ConditionalStore for InMemoryStore {
    fn get(&self, key: &str) -> Result<Option<(Vec<u8>, ETag)>, StoreError> {
        let objects = self.objects.lock().unwrap();
        Ok(objects
            .get(key)
            .map(|(bytes, rev)| (bytes.clone(), rev.to_string())))
    }

    fn put_if_absent(&self, key: &str, bytes: &[u8]) -> Result<ETag, StoreError> {
        let mut objects = self.objects.lock().unwrap();
        if objects.contains_key(key) {
            return Err(StoreError::PreconditionFailed);
        }
        objects.insert(key.to_string(), (bytes.to_vec(), 0));
        Ok(0.to_string())
    }

    fn put_if_match(&self, key: &str, bytes: &[u8], etag: &ETag) -> Result<ETag, StoreError> {
        let mut objects = self.objects.lock().unwrap();
        let current_rev = objects.get(key).map(|(_, rev)| *rev);
        if current_rev.map(|rev| rev.to_string()) != Some(etag.clone()) {
            return Err(StoreError::PreconditionFailed);
        }
        let new_rev = current_rev.unwrap() + 1;
        objects.insert(key.to_string(), (bytes.to_vec(), new_rev));
        Ok(new_rev.to_string())
    }
}

impl RangeGetStore for InMemoryStore {
    fn get_range(&self, key: &str, start: u64, end: u64) -> Result<Vec<u8>, StoreError> {
        assert!(start < end, "empty or inverted range: {start}..{end}");
        let objects = self.objects.lock().unwrap();
        let (bytes, _) = objects
            .get(key)
            .ok_or_else(|| StoreError::Io(format!("no such key: {key}")))?;
        let start = start as usize;
        let end = (end as usize).min(bytes.len());
        Ok(bytes[start..end].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_returns_none_for_absent_key() {
        let store = InMemoryStore::new();
        assert_eq!(store.get("missing").unwrap(), None);
    }

    #[test]
    fn delete_removes_a_present_key() {
        let store = InMemoryStore::new();
        store.put_if_absent("key", b"hello").unwrap();

        store.delete("key");

        assert_eq!(store.get("key").unwrap(), None);
    }

    #[test]
    fn delete_of_an_absent_key_is_a_no_op() {
        let store = InMemoryStore::new();

        store.delete("missing");

        assert_eq!(store.get("missing").unwrap(), None);
    }

    #[test]
    fn put_if_absent_then_get_round_trips() {
        let store = InMemoryStore::new();

        let etag = store.put_if_absent("key", b"hello").unwrap();
        let (bytes, got_etag) = store.get("key").unwrap().unwrap();

        assert_eq!(bytes, b"hello");
        assert_eq!(got_etag, etag);
    }

    #[test]
    fn put_if_absent_fails_when_key_already_exists() {
        let store = InMemoryStore::new();
        store.put_if_absent("key", b"first").unwrap();

        let result = store.put_if_absent("key", b"second");

        assert_eq!(result, Err(StoreError::PreconditionFailed));
        assert_eq!(store.get("key").unwrap().unwrap().0, b"first");
    }

    #[test]
    fn put_if_match_succeeds_with_current_etag() {
        let store = InMemoryStore::new();
        let etag = store.put_if_absent("key", b"first").unwrap();

        let new_etag = store.put_if_match("key", b"second", &etag).unwrap();

        assert_ne!(new_etag, etag);
        assert_eq!(store.get("key").unwrap().unwrap().0, b"second");
    }

    #[test]
    fn get_range_returns_the_requested_byte_slice() {
        let store = InMemoryStore::new();
        store.put_if_absent("key", b"0123456789").unwrap();

        assert_eq!(store.get_range("key", 2, 5).unwrap(), b"234");
        assert_eq!(store.get_range("key", 0, 1).unwrap(), b"0");
        assert_eq!(store.get_range("key", 9, 10).unwrap(), b"9");
    }

    #[test]
    fn get_range_clamps_an_end_past_the_object_length() {
        let store = InMemoryStore::new();
        store.put_if_absent("key", b"hello").unwrap();

        assert_eq!(store.get_range("key", 3, 1_000).unwrap(), b"lo");
    }

    #[test]
    fn get_range_of_an_absent_key_is_an_error() {
        let store = InMemoryStore::new();

        assert!(store.get_range("missing", 0, 1).is_err());
    }

    #[test]
    fn put_if_match_fails_with_stale_etag() {
        let store = InMemoryStore::new();
        let stale_etag = store.put_if_absent("key", b"first").unwrap();
        store.put_if_match("key", b"second", &stale_etag).unwrap();

        let result = store.put_if_match("key", b"third", &stale_etag);

        assert_eq!(result, Err(StoreError::PreconditionFailed));
        assert_eq!(store.get("key").unwrap().unwrap().0, b"second");
    }
}
