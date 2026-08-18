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
use strand_core::manifest::{commit, read_snapshot};
use strand_core::s3_store::S3Store;
use strand_core::segment::{BlobSpec, SegmentBuilder, write_segment};
use strand_core::store::{ConditionalStore, StoreError};
use testcontainers_modules::minio::MinIO;
use testcontainers_modules::testcontainers::runners::AsyncRunner;

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
        assert_eq!(segment_bytes.len(), 102);
    });
}
