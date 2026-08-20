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

//! Exercises `strand_tools::orphan_sweep::sweep_orphans` against a real
//! MinIO instance, started fresh per test via testcontainers — the same
//! pattern `crates/strand-core/tests/s3_store.rs` uses for the CAS
//! protocol itself, applied here to the M3-5 orphan sweep (`docs/
//! roadmap.md`). Real object storage, not simulated: `S3Store::list`
//! issues a real `ListObjectsV2` and `delete_object` a real
//! `DeleteObject`.

use aws_config::BehaviorVersion;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::Credentials;
use strand_core::container::{ChunkCodec, StorageClass, Tier};
use strand_core::manifest::{SnapshotMetadata, commit};
use strand_core::s3_store::S3Store;
use strand_core::segment::{BlobSpec, SegmentBuilder, write_segment};
use strand_core::store::ConditionalStore;
use strand_core::table_metadata::{CasHost, RetentionPolicy, TableMetadata, write_table_metadata};
use strand_tools::orphan_sweep::{now_millis, sweep_orphans};
use testcontainers_modules::minio::MinIO;
use testcontainers_modules::testcontainers::runners::AsyncRunner;

const BUCKET: &str = "strand-sweep-test";

/// A minimal, real segment builder — mirrors `tests/s3_store.rs`'s own
/// `toy_builder`: this test needs a real segment object at a real path
/// (not just a `SegmentRef` entry in the manifest), since it checks real
/// object presence/absence after a sweep.
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

/// Mirrors `tests/s3_store.rs`'s own `with_store`: starts a fresh MinIO
/// container, runs `test` against a real `S3Store`, and always tears the
/// container down inside an active runtime afterward.
fn with_store(test: impl FnOnce(&S3Store) + std::panic::UnwindSafe) {
    let setup_runtime = tokio::runtime::Runtime::new().unwrap();
    let (store, container) = setup_runtime.block_on(start_store());

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| test(&store)));

    setup_runtime.block_on(async { drop(container) });

    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

fn write_metadata(store: &S3Store) {
    write_table_metadata(
        store,
        &TableMetadata {
            format_version: 0,
            cas_host: CasHost::Native {
                store: "s3".to_string(),
            },
            retention: RetentionPolicy {
                min_snapshots_to_keep: Some(1),
                max_snapshot_age_millis: None,
            },
        },
    )
    .unwrap();
}

/// The crash test per `spec/manifest.md`'s "Orphan files" rule, against
/// real object storage: reproduces `tests/s3_store.rs`'s own
/// `orphaned_writer_crash_is_harmless_to_readers` crash pattern — a
/// segment and a snapshot object written, but `_strand/current` never
/// updated — then runs a real sweep and confirms the orphan is gone while
/// the real, pointed-at baseline segment and snapshot survive untouched.
#[test]
fn orphan_sweep_removes_a_crashed_writers_orphan_against_real_minio() {
    with_store(|store| {
        write_metadata(store);

        let baseline = commit(store, |next_row_id| {
            vec![write_segment(
                store,
                "segments/baseline.bin",
                &toy_builder(2, 0xA0),
                next_row_id,
            )]
        })
        .unwrap();

        // Simulate the crash: a segment and a snapshot object that
        // together would be the *next* legitimate commit, but the pointer
        // is never advanced (RFC 0001 §2 step 3 never happens).
        let orphan_segment = write_segment(
            store,
            "segments/orphan.bin",
            &toy_builder(3, 0xB0),
            baseline.next_row_id,
        );
        let orphan_snapshot = SnapshotMetadata {
            version: baseline.version + 1,
            next_row_id: baseline.next_row_id + 3,
            segments: {
                let mut segments = baseline.segments.clone();
                segments.push(orphan_segment);
                segments
            },
            committed_at_millis: 0,
        };
        store
            .put_if_absent(
                "_strand/snapshots/00000000000000000001-crash.json",
                &serde_json::to_vec(&orphan_snapshot).unwrap(),
            )
            .unwrap();

        // A retention window of 0ms against a real "now": both orphans
        // are unambiguously past it (real wall-clock time only moves
        // forward from the moment they were written, a moment ago).
        let outcome = sweep_orphans(store, "", 0, now_millis(), false).unwrap();

        assert!(
            outcome.deleted.contains(&"segments/orphan.bin".to_string()),
            "deleted: {:?}",
            outcome.deleted
        );
        assert!(
            outcome
                .deleted
                .contains(&"_strand/snapshots/00000000000000000001-crash.json".to_string()),
            "deleted: {:?}",
            outcome.deleted
        );
        assert!(store.get("segments/orphan.bin").unwrap().is_none());
        assert!(
            store
                .get("_strand/snapshots/00000000000000000001-crash.json")
                .unwrap()
                .is_none()
        );

        // The real, pointed-at baseline segment and snapshot survive.
        assert!(store.get("segments/baseline.bin").unwrap().is_some());
        assert!(store.get("_strand/current").unwrap().is_some());
        let read_back = strand_core::manifest::read_snapshot(store)
            .unwrap()
            .unwrap();
        assert_eq!(read_back, baseline, "the real commit is untouched");
    });
}

/// The retention-window safety margin, against real object storage: an
/// orphan written moments ago must survive a sweep even though nothing
/// references it, because it is younger than the retention window — the
/// real second safety margin `spec/manifest.md`'s "Orphan files" rule
/// names against a race with a commit still in flight.
#[test]
fn orphan_younger_than_the_retention_window_survives_a_sweep_against_real_minio() {
    with_store(|store| {
        write_metadata(store);

        commit(store, |next_row_id| {
            vec![write_segment(
                store,
                "segments/baseline.bin",
                &toy_builder(2, 0xA0),
                next_row_id,
            )]
        })
        .unwrap();

        store
            .put_if_absent("segments/fresh-orphan.bin", b"in flight")
            .unwrap();

        // A one-hour retention window comfortably exceeds the real
        // wall-clock time this test takes to run.
        let one_hour_millis = 60 * 60 * 1000;
        let outcome = sweep_orphans(store, "", one_hour_millis, now_millis(), false).unwrap();

        assert!(
            outcome
                .skipped_within_window
                .contains(&"segments/fresh-orphan.bin".to_string()),
            "skipped_within_window: {:?}",
            outcome.skipped_within_window
        );
        assert!(outcome.deleted.is_empty(), "deleted: {:?}", outcome.deleted);
        assert!(
            store.get("segments/fresh-orphan.bin").unwrap().is_some(),
            "a young orphan must survive the sweep"
        );
    });
}
