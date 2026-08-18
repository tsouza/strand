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

//! Shared harness for the end-to-end benchmarks CLAUDE.md §7 and
//! `docs/milestones.md`'s M0 entry call for: real MinIO (via testcontainers,
//! self-contained and reproducible, not a pre-existing manual container),
//! a GET/PUT-counting `ConditionalStore` wrapper, and result reporting
//! pinned with the date and commit hash per CLAUDE.md §7's rule that no
//! performance claim ships without a reproducible, machine-readable result.

use serde::Serialize;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use strand_core::s3_store::S3Store;
use strand_core::store::{ConditionalStore, ETag, StoreError};
use testcontainers_modules::minio::MinIO;
use testcontainers_modules::testcontainers::ContainerAsync;
use testcontainers_modules::testcontainers::runners::AsyncRunner;

const BUCKET: &str = "strand-bench";

/// For use from a context that is already inside a Tokio runtime (e.g.
/// `start_minio`, itself driven by `setup_runtime.block_on`) — `.await`s
/// directly rather than spinning up a nested runtime, which would hit
/// "cannot start a runtime from within a runtime" (found by doing exactly
/// that: this async version exists because the sync one below, called from
/// here, panicked with precisely that error).
async fn client_config_async(endpoint: &str) -> aws_sdk_s3::Config {
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .endpoint_url(endpoint)
        .region("us-east-1")
        .credentials_provider(aws_sdk_s3::config::Credentials::new(
            "minioadmin",
            "minioadmin",
            None,
            None,
            "static",
        ))
        .load()
        .await;
    aws_sdk_s3::config::Builder::from(&config)
        .force_path_style(true)
        .build()
}

/// For use from a plain sync context (an arbitrary caller thread in
/// `store_for`) that is not already inside a Tokio runtime: spins up a
/// short-lived runtime just for the config load, then drops it — safe here
/// specifically because config loading with static credentials makes no
/// lasting connections, unlike the `S3Store` built from its result, which
/// gets its own separate, long-lived runtime.
fn client_config(endpoint: &str) -> aws_sdk_s3::Config {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(client_config_async(endpoint))
}

/// Starts a fresh MinIO container and creates the bucket benchmarks run
/// against, returning the endpoint URL and bucket name — not a pre-built
/// client. A `Client`'s underlying HTTP connector ties its background
/// dispatch tasks to whatever Tokio runtime first drives it; sharing one
/// `Client` (even cloned) across the independent per-store runtimes
/// `store_for` builds fails with "runtime dropped the dispatch task" —
/// found by hitting exactly that error the first time a multi-writer
/// benchmark used a shared client. `store_for` therefore builds a fully
/// independent client per call, safe to call from any thread. The
/// container must outlive every store built from its endpoint; callers
/// hold it and drop it inside an active Tokio runtime context
/// (`ContainerAsync::drop` panics without one, verified empirically while
/// building the M0 crash tests this harness reuses the pattern from).
pub async fn start_minio() -> (String, String, ContainerAsync<MinIO>) {
    let container = MinIO::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(9000).await.unwrap();
    let endpoint = format!("http://127.0.0.1:{port}");

    let s3_config = client_config_async(&endpoint).await;
    let client = aws_sdk_s3::Client::from_conf(s3_config);
    client.create_bucket().bucket(BUCKET).send().await.unwrap();

    (endpoint, BUCKET.to_string(), container)
}

/// Builds a fully independent `S3Store` — its own client, its own Tokio
/// runtime — against `endpoint`/`bucket` (from `start_minio`). Safe to call
/// once per thread in a concurrency benchmark; see `start_minio`'s docs for
/// why a shared client is not.
pub fn store_for(endpoint: &str, bucket: &str) -> S3Store {
    let client = aws_sdk_s3::Client::from_conf(client_config(endpoint));
    S3Store::new(client, bucket.to_string())
}

/// Starts a fresh MinIO container, runs `body` against its endpoint and
/// bucket, and always tears the container down inside an active runtime
/// afterward, panic or not. `ContainerAsync::drop` panics without an active
/// Tokio runtime (verified empirically); a plain scope-exit drop on a `body`
/// panic would run with no runtime entered, turning one clean assertion
/// failure into a second panic during unwind and a process abort instead of
/// a readable error — the exact failure mode this guards against, found by
/// actually hitting it while first writing these benchmarks.
pub fn with_minio(body: impl FnOnce(&str, &str) + std::panic::UnwindSafe) {
    let setup_runtime = tokio::runtime::Runtime::new().unwrap();
    let (endpoint, bucket, container) = setup_runtime.block_on(start_minio());

    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(&endpoint, &bucket)));

    setup_runtime.block_on(async { drop(container) });

    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

/// A `ConditionalStore` wrapper that counts GET and PUT calls, so a
/// benchmark can report request counts alongside latency — CLAUDE.md §7
/// requires every cold-path number be justified in GETs and bytes, not
/// only wall-clock time.
pub struct CountingStore<'a, S: ConditionalStore> {
    inner: &'a S,
    gets: AtomicU64,
    puts: AtomicU64,
}

impl<'a, S: ConditionalStore> CountingStore<'a, S> {
    pub fn new(inner: &'a S) -> Self {
        CountingStore {
            inner,
            gets: AtomicU64::new(0),
            puts: AtomicU64::new(0),
        }
    }

    pub fn get_count(&self) -> u64 {
        self.gets.load(Ordering::SeqCst)
    }

    pub fn put_count(&self) -> u64 {
        self.puts.load(Ordering::SeqCst)
    }

    pub fn reset(&self) {
        self.gets.store(0, Ordering::SeqCst);
        self.puts.store(0, Ordering::SeqCst);
    }
}

impl<S: ConditionalStore> ConditionalStore for CountingStore<'_, S> {
    fn get(&self, key: &str) -> Result<Option<(Vec<u8>, ETag)>, StoreError> {
        self.gets.fetch_add(1, Ordering::SeqCst);
        self.inner.get(key)
    }

    fn put_if_absent(&self, key: &str, bytes: &[u8]) -> Result<ETag, StoreError> {
        self.puts.fetch_add(1, Ordering::SeqCst);
        self.inner.put_if_absent(key, bytes)
    }

    fn put_if_match(&self, key: &str, bytes: &[u8], etag: &ETag) -> Result<ETag, StoreError> {
        self.puts.fetch_add(1, Ordering::SeqCst);
        self.inner.put_if_match(key, bytes, etag)
    }
}

/// Times `f`, returning its result and the elapsed wall time in
/// milliseconds.
pub fn timed<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let start = Instant::now();
    let result = f();
    (result, start.elapsed().as_secs_f64() * 1000.0)
}

/// The `p`th percentile (0.0..=1.0) of `samples`, nearest-rank, on a
/// pre-sorted-ascending slice. Panics on an empty slice — a benchmark with
/// zero samples is a bug, not a zero result.
pub fn percentile(sorted_ascending: &[f64], p: f64) -> f64 {
    assert!(
        !sorted_ascending.is_empty(),
        "no samples to compute a percentile from"
    );
    let rank =
        ((p * sorted_ascending.len() as f64).ceil() as usize).clamp(1, sorted_ascending.len());
    sorted_ascending[rank - 1]
}

#[derive(Serialize)]
pub struct BenchReport<T: Serialize> {
    pub name: String,
    pub date: String,
    pub commit: String,
    pub result: T,
}

/// Writes `report` as pretty JSON to `bench/results/{name}.json`, per
/// CLAUDE.md §7: results are committed with date and commit hash,
/// machine-readable. Date and commit are captured here, not by the caller,
/// so every report is stamped the same way.
pub fn write_report<T: Serialize>(name: &str, result: T) {
    let date = String::from_utf8(
        Command::new("date")
            .arg("-u")
            .arg("+%Y-%m-%dT%H:%M:%SZ")
            .output()
            .expect("the `date` command must be available")
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    let commit = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("git must be available to stamp a commit hash")
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();

    let report = BenchReport {
        name: name.to_string(),
        date,
        commit,
        result,
    };

    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/results");
    std::fs::create_dir_all(dir).unwrap();
    let path = format!("{dir}/{name}.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("wrote {path}");
}
