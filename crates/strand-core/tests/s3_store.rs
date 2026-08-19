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

//! Exercises `S3Store` against a real MinIO instance, started fresh per test
//! via testcontainers — self-contained and reproducible, per this project's
//! own rule that object-storage behavior is tested against local MinIO, not
//! assumed from documentation. `#[cfg(feature = "s3")]`-gated: `cargo test`
//! without `--features s3` skips this file entirely.
#![cfg(feature = "s3")]

use aws_config::BehaviorVersion;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::Credentials;
use strand_core::container::{ChunkCodec, StorageClass, Tier};
use strand_core::manifest::{SegmentRef, SnapshotMetadata, commit, read_snapshot};
use strand_core::s3_store::S3Store;
use strand_core::segment::{BlobSpec, SegmentBuilder, write_segment};
use strand_core::store::{ConditionalStore, RangeGetStore, StoreError};
use testcontainers_modules::minio::MinIO;
use testcontainers_modules::testcontainers::runners::AsyncRunner;

fn toy_builder(row_id_count: u64, tag: u8) -> SegmentBuilder {
    let mut builder = SegmentBuilder::new(row_id_count);
    builder.add_blob(BlobSpec {
        family_id: 0,
        field_id: 0,
        blob_type_id: 0,
        storage_class: StorageClass::RawMappable,
        tier: Tier::NotApplicable,
        alignment: 8,
        chunk_codec: ChunkCodec::None,
        chunk_codec_level: 0,
        data: vec![tag, 0x00, 0x00, 0x00],
    });
    builder
}

const BUCKET: &str = "strand-test";

async fn start_store() -> (
    S3Store,
    testcontainers_modules::testcontainers::ContainerAsync<MinIO>,
) {
    let container = MinIO::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(9000).await.unwrap();
    let endpoint = format!("http://127.0.0.1:{port}");

    let config = aws_config::defaults(BehaviorVersion::latest())
        .endpoint_url(&endpoint)
        .region("us-east-1")
        .credentials_provider(Credentials::new(
            "minioadmin",
            "minioadmin",
            None,
            None,
            "static",
        ))
        .load()
        .await;
    let s3_config = aws_sdk_s3::config::Builder::from(&config)
        .force_path_style(true)
        .build();
    let client = Client::from_conf(s3_config);
    client.create_bucket().bucket(BUCKET).send().await.unwrap();

    (S3Store::new(client, BUCKET), container)
}

/// Starts a fresh MinIO container, runs `test` against an `S3Store` backed
/// by it, and always tears the container down inside an active runtime
/// afterward — `ContainerAsync::drop` panics without one (verified
/// empirically), and a plain scope-exit drop on a test panic would run with
/// no runtime entered, turning a clean assertion failure into a process
/// abort instead of a readable test failure.
fn with_store(test: impl FnOnce(&S3Store) + std::panic::UnwindSafe) {
    let setup_runtime = tokio::runtime::Runtime::new().unwrap();
    let (store, container) = setup_runtime.block_on(start_store());

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| test(&store)));

    setup_runtime.block_on(async { drop(container) });

    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

#[test]
fn conditional_write_semantics_match_the_in_memory_store() {
    with_store(|store| {
        assert_eq!(store.get("absent").unwrap(), None);

        let etag = store.put_if_absent("key", b"first").unwrap();
        let (bytes, got_etag) = store.get("key").unwrap().unwrap();
        assert_eq!(bytes, b"first");
        assert_eq!(got_etag, etag);

        assert_eq!(
            store.put_if_absent("key", b"second"),
            Err(StoreError::PreconditionFailed),
            "a second create at the same key must be rejected, not overwrite"
        );

        let new_etag = store.put_if_match("key", b"updated", &etag).unwrap();
        assert_ne!(new_etag, etag);
        assert_eq!(store.get("key").unwrap().unwrap().0, b"updated");

        assert_eq!(
            store.put_if_match("key", b"stale write", &etag),
            Err(StoreError::PreconditionFailed),
            "a write against a stale ETag must be rejected"
        );
    });
}

#[test]
fn full_commit_and_read_round_trip_against_real_object_storage() {
    with_store(|store| {
        let committed = commit(store, |next_row_id| {
            let mut builder = SegmentBuilder::new(2);
            builder.add_blob(BlobSpec {
                family_id: 0,
                field_id: 0,
                blob_type_id: 0,
                storage_class: strand_core::container::StorageClass::RawMappable,
                tier: strand_core::container::Tier::NotApplicable,
                alignment: 8,
                chunk_codec: strand_core::container::ChunkCodec::None,
                chunk_codec_level: 0,
                data: vec![0x2A, 0x00, 0x00, 0x00, 0x2B, 0x00, 0x00, 0x00],
            });
            vec![write_segment(
                store,
                "segments/one.bin",
                &builder,
                next_row_id,
            )]
        })
        .unwrap();

        assert_eq!(committed.segments.len(), 1);
        assert_eq!(committed.segments[0].row_id_base, 0);

        let read_back = read_snapshot(store).unwrap().unwrap();
        assert_eq!(read_back, committed);

        let (segment_bytes, _) = store.get("segments/one.bin").unwrap().unwrap();
        assert_eq!(segment_bytes.len(), 110);
    });
}

/// Crash test per `docs/milestones.md` M0: a writer that dies after writing
/// its segment and snapshot metadata (RFC 0001 §3 steps 1–2) but before
/// advancing the pointer (step 3) leaves an orphan. `CLAUDE.md` §6 says this
/// must be harmless to readers and cost only storage — proved here against
/// real object storage, not simulated.
#[test]
fn orphaned_writer_crash_is_harmless_to_readers() {
    with_store(|store| {
        let baseline = commit(store, |next_row_id| {
            vec![write_segment(
                store,
                "segments/baseline.bin",
                &toy_builder(2, 0xA0),
                next_row_id,
            )]
        })
        .unwrap();

        // Simulate the crash: write a segment and a snapshot object that
        // together would be the *next* legitimate commit, but never touch
        // `_strand/current` — step 3 of RFC 0001 §3 never happens.
        let orphan_segment = write_segment(
            store,
            "segments/orphan.bin",
            &toy_builder(3, 0xB0),
            baseline.next_row_id,
        );
        let mut orphan_segments = baseline.segments.clone();
        orphan_segments.push(orphan_segment);
        let orphan_snapshot = SnapshotMetadata {
            version: baseline.version + 1,
            next_row_id: baseline.next_row_id + 3,
            segments: orphan_segments,
        };
        store
            .put_if_absent(
                "_strand/snapshots/orphan-crash-test.json",
                &serde_json::to_vec(&orphan_snapshot).unwrap(),
            )
            .unwrap();

        // A reader must see only the baseline — the orphan is invisible.
        let read_back = read_snapshot(store).unwrap().unwrap();
        assert_eq!(read_back, baseline);

        // A subsequent legitimate commit must proceed normally, oblivious
        // to the orphan: it builds on the baseline, not the crashed
        // writer's unreferenced snapshot.
        let next = commit(store, |next_row_id| {
            vec![write_segment(
                store,
                "segments/after-crash.bin",
                &toy_builder(1, 0xC0),
                next_row_id,
            )]
        })
        .unwrap();

        assert_eq!(next.version, baseline.version + 1);
        assert_eq!(
            next.segments.len(),
            2,
            "baseline's segment plus this one, not the orphan's"
        );
        assert!(
            next.segments
                .iter()
                .all(|s: &SegmentRef| s.path != "segments/orphan.bin"),
            "the orphan must never be referenced by a real commit"
        );
        assert_eq!(next.segments[1].row_id_base, baseline.next_row_id);
    });
}

/// A `ConditionalStore` wrapper that runs a hook before delegating each
/// `get` call to a real backend — the same seam `manifest.rs`'s own
/// in-memory tests use, generalized over any inner store so it can wrap
/// `S3Store`.
struct FaultInjectingStore<'a, S: ConditionalStore, F: Fn(&str)> {
    inner: &'a S,
    on_get: F,
}

impl<S: ConditionalStore, F: Fn(&str)> ConditionalStore for FaultInjectingStore<'_, S, F> {
    fn get(&self, key: &str) -> Result<Option<(Vec<u8>, strand_core::store::ETag)>, StoreError> {
        (self.on_get)(key);
        self.inner.get(key)
    }

    fn put_if_absent(
        &self,
        key: &str,
        bytes: &[u8],
    ) -> Result<strand_core::store::ETag, StoreError> {
        self.inner.put_if_absent(key, bytes)
    }

    fn put_if_match(
        &self,
        key: &str,
        bytes: &[u8],
        etag: &strand_core::store::ETag,
    ) -> Result<strand_core::store::ETag, StoreError> {
        self.inner.put_if_match(key, bytes, etag)
    }
}

/// Crash test per `docs/milestones.md` M0: a reader whose pointer read named
/// a snapshot that compaction has since removed must refresh and retry
/// rather than report corruption (RFC 0001 §3, `CLAUDE.md` §6) — proved here
/// against real object storage's actual 404 behavior, not the in-memory
/// simulation `manifest.rs`'s own tests use. The fault is injected at the
/// same point a genuinely interleaved reader would hit it: right as the
/// reader fetches the snapshot object its (now-stale) pointer read named.
#[test]
fn reader_on_a_compacted_snapshot_recovers_against_real_object_storage() {
    with_store(|store| {
        let first = commit(store, |next_row_id| {
            vec![write_segment(
                store,
                "segments/a.bin",
                &toy_builder(2, 0xA0),
                next_row_id,
            )]
        })
        .unwrap();

        let (pointer_bytes, _) = store.get("_strand/current").unwrap().unwrap();
        let first_snapshot_path = String::from_utf8(pointer_bytes).unwrap();
        let fired = std::cell::Cell::new(false);

        let faulting = FaultInjectingStore {
            inner: store,
            on_get: |key: &str| {
                if key == first_snapshot_path && !fired.get() {
                    fired.set(true);
                    // A newer commit lands, moving the pointer past
                    // `first` — only then is deleting `first`'s snapshot
                    // object safe under deletion-safety (`CLAUDE.md` §6).
                    commit(store, |next_row_id| {
                        vec![write_segment(
                            store,
                            "segments/b.bin",
                            &toy_builder(3, 0xB0),
                            next_row_id,
                        )]
                    })
                    .unwrap();
                    store.delete(&first_snapshot_path).unwrap();
                }
            },
        };

        let read_back = read_snapshot(&faulting).unwrap().unwrap();

        assert_eq!(
            read_back.version,
            first.version + 1,
            "recovered onto the newer snapshot"
        );
        assert_eq!(read_back.segments.len(), 2);
    });
}

/// RFC 0001 §1 / Open questions item 3: does MinIO's `GetObject` honor the
/// HTTP suffix-range request form (`Range: bytes=-N`, RFC 9110 §14.1.2,
/// `references/rfc9110-range-requests.txt`), as opposed to only the
/// explicit-end form (`bytes=start-end`) the container's open protocol
/// actually uses? The protocol question was already settled by reading the
/// RFC text directly; this is the server-support question, closed here by
/// issuing a real suffix-range request against a real MinIO instance rather
/// than inferring an answer from AWS's documentation, which — per
/// `references/aws-s3-getobject-range-parameter.md` — demonstrates only the
/// explicit-end form and is silent on the suffix form either way. This test
/// bypasses `S3Store`/`ConditionalStore` deliberately: suffix-range support
/// is not something the open protocol depends on (RFC 0001 §1 chose the
/// explicit-end form specifically to avoid depending on it), so there is no
/// production code path to exercise — only the raw SDK client, used the same
/// way `start_store` builds one.
#[test]
fn suffix_range_get_is_honored_by_minio() {
    let setup_runtime = tokio::runtime::Runtime::new().unwrap();
    let result = setup_runtime.block_on(async {
        let container = MinIO::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(9000).await.unwrap();
        let endpoint = format!("http://127.0.0.1:{port}");

        let config = aws_config::defaults(BehaviorVersion::latest())
            .endpoint_url(&endpoint)
            .region("us-east-1")
            .credentials_provider(Credentials::new(
                "minioadmin",
                "minioadmin",
                None,
                None,
                "static",
            ))
            .load()
            .await;
        let s3_config = aws_sdk_s3::config::Builder::from(&config)
            .force_path_style(true)
            .build();
        let client = Client::from_conf(s3_config);
        client
            .create_bucket()
            .bucket("suffix-range-test")
            .send()
            .await
            .unwrap();

        // A 26-byte object, so the last-10-bytes suffix range is
        // unambiguous and independently checkable by eye: bytes 16..26 of
        // the lowercase alphabet, "qrstuvwxyz".
        let body: Vec<u8> = (b'a'..=b'z').collect();
        client
            .put_object()
            .bucket("suffix-range-test")
            .key("alphabet.bin")
            .body(body.clone().into())
            .send()
            .await
            .unwrap();

        // The suffix form itself, RFC 9110 §14.1.2: "suffix-length gives the
        // number of octets from the end of the representation" — no
        // explicit start offset, unlike the explicit-end form RFC 0001 §1's
        // open protocol actually uses.
        let response = client
            .get_object()
            .bucket("suffix-range-test")
            .key("alphabet.bin")
            .range("bytes=-10")
            .send()
            .await;

        let outcome = match response {
            Ok(output) => {
                let content_range = output.content_range().map(str::to_string);
                let bytes = output.body.collect().await.unwrap().into_bytes().to_vec();
                Some((content_range, bytes))
            }
            Err(_) => None,
        };

        drop(container);
        outcome
    });

    let (content_range, bytes) =
        result.expect("MinIO rejected the suffix-range GET outright (see test doc comment)");

    assert_eq!(
        bytes, b"qrstuvwxyz",
        "suffix range 'bytes=-10' against a 26-byte object must return exactly its last 10 bytes"
    );
    assert_eq!(
        content_range.as_deref(),
        Some("bytes 16-25/26"),
        "MinIO must report the resolved absolute range in Content-Range, per RFC 9110 §14.4"
    );
}

/// `S3Store::get_range` (X-5: the trait extension the parallel-range-fetch
/// benchmark depends on) against real MinIO: a sub-range GET must return
/// exactly the requested bytes, not the whole object, and must honor S3's
/// clamp-at-end-of-object semantics for an over-long range.
#[test]
fn get_range_returns_exactly_the_requested_bytes_against_real_object_storage() {
    with_store(|store| {
        let payload: Vec<u8> = (0u32..10_000).map(|i| (i % 256) as u8).collect();
        store.put_if_absent("range-target", &payload).unwrap();

        let middle = store.get_range("range-target", 100, 200).unwrap();
        assert_eq!(middle, payload[100..200]);

        let clamped = store.get_range("range-target", 9_990, 1_000_000).unwrap();
        assert_eq!(clamped, payload[9_990..]);

        let whole = store
            .get_range("range-target", 0, payload.len() as u64)
            .unwrap();
        assert_eq!(whole, payload);
    });
}
