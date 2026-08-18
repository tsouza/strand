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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreError {
    /// The `If-None-Match: *` or `If-Match` precondition was not met.
    PreconditionFailed,
}

pub trait ConditionalStore {
    /// Returns the object's bytes and current ETag, or `None` if absent.
    fn get(&self, key: &str) -> Option<(Vec<u8>, ETag)>;

    /// Creates `key` with `bytes`, only if `key` does not already exist.
    fn put_if_absent(&self, key: &str, bytes: &[u8]) -> Result<ETag, StoreError>;

    /// Overwrites `key` with `bytes`, only if its current ETag is `etag`.
    fn put_if_match(&self, key: &str, bytes: &[u8], etag: &ETag) -> Result<ETag, StoreError>;
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
}

impl ConditionalStore for InMemoryStore {
    fn get(&self, key: &str) -> Option<(Vec<u8>, ETag)> {
        let objects = self.objects.lock().unwrap();
        objects
            .get(key)
            .map(|(bytes, rev)| (bytes.clone(), rev.to_string()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_returns_none_for_absent_key() {
        let store = InMemoryStore::new();
        assert_eq!(store.get("missing"), None);
    }

    #[test]
    fn put_if_absent_then_get_round_trips() {
        let store = InMemoryStore::new();

        let etag = store.put_if_absent("key", b"hello").unwrap();
        let (bytes, got_etag) = store.get("key").unwrap();

        assert_eq!(bytes, b"hello");
        assert_eq!(got_etag, etag);
    }

    #[test]
    fn put_if_absent_fails_when_key_already_exists() {
        let store = InMemoryStore::new();
        store.put_if_absent("key", b"first").unwrap();

        let result = store.put_if_absent("key", b"second");

        assert_eq!(result, Err(StoreError::PreconditionFailed));
        assert_eq!(store.get("key").unwrap().0, b"first");
    }

    #[test]
    fn put_if_match_succeeds_with_current_etag() {
        let store = InMemoryStore::new();
        let etag = store.put_if_absent("key", b"first").unwrap();

        let new_etag = store.put_if_match("key", b"second", &etag).unwrap();

        assert_ne!(new_etag, etag);
        assert_eq!(store.get("key").unwrap().0, b"second");
    }

    #[test]
    fn put_if_match_fails_with_stale_etag() {
        let store = InMemoryStore::new();
        let stale_etag = store.put_if_absent("key", b"first").unwrap();
        store.put_if_match("key", b"second", &stale_etag).unwrap();

        let result = store.put_if_match("key", b"third", &stale_etag);

        assert_eq!(result, Err(StoreError::PreconditionFailed));
        assert_eq!(store.get("key").unwrap().0, b"second");
    }
}
