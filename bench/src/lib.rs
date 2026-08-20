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

/// Creates one additional bucket against an already-running MinIO container
/// (from `start_minio`/`with_minio`). A manifest's pointer and snapshot
/// objects live at fixed keys within a bucket (`_strand/current`,
/// `crates/strand-core/src/manifest.rs`), so two independent tables need two
/// buckets, not two key prefixes within one. Used by
/// `bench/src/multi_segment_query.rs`, which needs one fresh table per
/// segment-count configuration without paying for a second container per
/// configuration.
pub fn create_bucket(endpoint: &str, bucket: &str) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let client = aws_sdk_s3::Client::from_conf(client_config_async(endpoint).await);
        client.create_bucket().bucket(bucket).send().await.unwrap();
    });
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

/// Injects a one-way `netem` delay onto the running container's network
/// interface (roadmap X-4: `bench/src/cold_open_injected_latency.rs`, the
/// real-network-tail-latency counterpart to `bench/src/cold_open.rs`'s
/// localhost-only measurement).
///
/// The container under test (MinIO, started by `start_minio`) ships no
/// package manager and no `tc` binary of its own — confirmed empirically:
/// its image is UBI-based (`cat /etc/os-release` inside it reports
/// `ID="rhel"`), and it has neither `apk`/`dnf`/`microdnf` nor even
/// `which`/`grep`, so nothing can be installed inside it. The mechanism
/// used instead: a throwaway `alpine` sidecar container started with
/// `--net container:<id>` (joining the *same* network namespace as the
/// target, so its `eth0` is literally the target's `eth0`, not a separate
/// veth) and `--cap-add NET_ADMIN`, which installs `iproute2` and runs
/// `tc qdisc add dev eth0 root netem delay`. This was verified directly in
/// this environment before being relied on here (`docker run --rm
/// --cap-add=NET_ADMIN alpine sh -c "apk add -q iproute2 && tc qdisc add
/// dev eth0 root netem delay 100ms"` succeeds), and separately confirmed to
/// actually change measured latency against a real MinIO container over its
/// mapped host port (a `curl` round trip against `/minio/health/live` went
/// from sub-millisecond to ~100ms with a 100ms netem delay applied this
/// way).
///
/// This delays only the target's *egress* (`tc qdisc` binds to one
/// interface's outbound direction) — traffic leaving the container, i.e.
/// server responses — not ingress (client requests arriving). Stated
/// honestly because it is a real asymmetry, not hidden in the number this
/// produces: on a *fresh* TCP connection (a real round trip: SYN, the
/// delayed SYN-ACK, ACK, request, delayed response) this yields roughly
/// `2 * delay_ms` wall time for the first request; on a *reused*
/// (keep-alive) connection — measured directly against a real MinIO
/// container's `/minio/health/live` endpoint, three repeat `curl` calls
/// over one kept-alive connection with a 50ms delay applied measured
/// 0.0508s/0.0507s/0.0507s each, i.e. one delayed leg only — it costs
/// approximately `delay_ms` per request, since only the response leg is
/// delayed. `delay_ms` is therefore chosen to equal the *round-trip*
/// figure this benchmark targets, not a true one-way figure split
/// symmetrically across both directions; a fully symmetric injection would
/// additionally shape the host-side veth peer's egress from the real
/// Docker host's root network namespace, which is a materially more
/// fragile mechanism (it requires resolving the specific veth peer
/// interface and real root privileges on the host outside any container)
/// and was not needed to get a real, honest round-trip number here.
pub fn inject_netem_delay(container_id: &str, delay_ms: u64) {
    let output = Command::new("docker")
        .args([
            "run",
            "--rm",
            "--net",
            &format!("container:{container_id}"),
            "--cap-add",
            "NET_ADMIN",
            "alpine",
            "sh",
            "-c",
            &format!("apk add -q iproute2 && tc qdisc add dev eth0 root netem delay {delay_ms}ms"),
        ])
        .output()
        .expect(
            "docker must be available to run the netem-injection sidecar \
             (docker run --net container:<id> --cap-add NET_ADMIN alpine ...)",
        );
    assert!(
        output.status.success(),
        "netem delay injection failed (docker exit status {:?}); this is a \
         genuine environment blocker, not a benchmark bug — see stdout/stderr \
         below.\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Like `with_minio`, but injects a `delay_ms` one-way `netem` delay (see
/// `inject_netem_delay`) onto the MinIO container's network interface
/// before running `body`, simulating S3-like round-trip latency instead of
/// bare localhost. Used by `bench/src/cold_open_injected_latency.rs`
/// (roadmap X-4).
pub fn with_minio_latency(delay_ms: u64, body: impl FnOnce(&str, &str) + std::panic::UnwindSafe) {
    let setup_runtime = tokio::runtime::Runtime::new().unwrap();
    let (endpoint, bucket, container) = setup_runtime.block_on(start_minio());

    inject_netem_delay(container.id(), delay_ms);

    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(&endpoint, &bucket)));

    setup_runtime.block_on(async { drop(container) });

    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

/// A `ConditionalStore` wrapper that counts GET and PUT calls (and the
/// total bytes returned by GETs), so a benchmark can report request counts
/// and byte volume alongside latency — CLAUDE.md §7 requires every
/// cold-path number be justified in GETs and bytes, not only wall-clock
/// time. Byte tracking (`get_bytes`) was added for
/// `bench/src/multi_segment_query.rs`, the first benchmark here that needs
/// to report "total bytes fetched" for a query spanning a variable,
/// caller-controlled number of GETs (one per segment plus the manifest
/// reads) rather than one fixed-shape open sequence.
pub struct CountingStore<'a, S: ConditionalStore> {
    inner: &'a S,
    gets: AtomicU64,
    puts: AtomicU64,
    get_bytes: AtomicU64,
}

impl<'a, S: ConditionalStore> CountingStore<'a, S> {
    pub fn new(inner: &'a S) -> Self {
        CountingStore {
            inner,
            gets: AtomicU64::new(0),
            puts: AtomicU64::new(0),
            get_bytes: AtomicU64::new(0),
        }
    }

    pub fn get_count(&self) -> u64 {
        self.gets.load(Ordering::SeqCst)
    }

    pub fn put_count(&self) -> u64 {
        self.puts.load(Ordering::SeqCst)
    }

    /// Sum of the byte lengths of every value returned by a `get` call
    /// (misses contribute nothing) since the last `reset`.
    pub fn get_bytes(&self) -> u64 {
        self.get_bytes.load(Ordering::SeqCst)
    }

    pub fn reset(&self) {
        self.gets.store(0, Ordering::SeqCst);
        self.puts.store(0, Ordering::SeqCst);
        self.get_bytes.store(0, Ordering::SeqCst);
    }
}

impl<S: ConditionalStore> ConditionalStore for CountingStore<'_, S> {
    fn get(&self, key: &str) -> Result<Option<(Vec<u8>, ETag)>, StoreError> {
        self.gets.fetch_add(1, Ordering::SeqCst);
        let result = self.inner.get(key)?;
        if let Some((bytes, _)) = &result {
            self.get_bytes
                .fetch_add(bytes.len() as u64, Ordering::SeqCst);
        }
        Ok(result)
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
