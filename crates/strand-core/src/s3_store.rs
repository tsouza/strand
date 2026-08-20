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

//! A `ConditionalStore` backed by S3-compatible object storage (S3 itself,
//! or MinIO, verified empirically against a real MinIO instance before this
//! was written — MinIO honors `If-None-Match: *` and `If-Match: <etag>` with
//! the same 412 semantics S3 documents). Bridges the async AWS SDK to the
//! synchronous `ConditionalStore` trait with a dedicated Tokio runtime per
//! store instance: the manifest protocol issues a handful of requests per
//! commit or read, not a hot loop, so a blocking facade is the right
//! complexity trade for now. Revisit if a future async read path needs to
//! share a runtime instead.

use crate::store::{
    ConditionalStore, DeletableStore, ETag, ListableStore, ListedObject, RangeGetStore, StoreError,
};
use aws_sdk_s3::Client;
use aws_sdk_s3::primitives::ByteStream;

pub struct S3Store {
    client: Client,
    bucket: String,
    runtime: tokio::runtime::Runtime,
}

impl S3Store {
    pub fn new(client: Client, bucket: impl Into<String>) -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to start the Tokio runtime backing S3Store");
        S3Store {
            client,
            bucket: bucket.into(),
            runtime,
        }
    }

    /// Issues the explicit-end range GET RFC 0001 §1's open protocol
    /// specifies — `Range: bytes={start}-{end_inclusive}` — and returns the
    /// bytes actually served. Not part of `ConditionalStore`, same reasoning
    /// as `delete`: the manifest-driven query path this trait serves opens a
    /// segment by fetching it wholesale today (`bench/src/cold_open.rs`,
    /// `bench/src/vector_cold_open.rs`). This exists so a benchmark can
    /// measure RFC 0001 §1's actual two-phase open protocol against real
    /// object storage (`bench/src/hotcache_tail_read.rs`), which is exactly
    /// what the RFC's own Open Questions section calls for before `N` is
    /// pinned to more than a provisional value.
    ///
    /// Named distinctly from `RangeGetStore::get_range` (below) rather than
    /// overloading that name on `S3Store`: this method's inclusive-end,
    /// `Option`-returning signature is shaped around RFC 0001 §1's specific
    /// tail-read protocol, while `RangeGetStore` is the general-purpose,
    /// half-open, non-optional abstraction X-5's parallel-fetch benchmark
    /// depends on — Rust's inherent-method priority would otherwise make an
    /// `S3Store::get_range` of this shape silently shadow the trait method
    /// for every caller holding a concrete `S3Store`, a real correctness
    /// trap this rename avoids.
    ///
    /// Returns `Ok(None)` for a key that does not exist, matching `get`'s
    /// convention. `start` is clamped to the tail-window arithmetic RFC
    /// 0001 §1 itself specifies (`max(0, byte_length - N)`) by the caller,
    /// not here — this method issues exactly the range it's given.
    pub fn get_tail_range(
        &self,
        key: &str,
        start: u64,
        end_inclusive: u64,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        self.runtime.block_on(async {
            let result = self
                .client
                .get_object()
                .bucket(&self.bucket)
                .key(key)
                .range(format!("bytes={start}-{end_inclusive}"))
                .send()
                .await;
            let output = match result {
                Ok(output) => output,
                Err(err) => {
                    if err.as_service_error().is_some_and(|e| e.is_no_such_key()) {
                        return Ok(None);
                    }
                    return Err(StoreError::Io(format!(
                        "{:#}",
                        aws_smithy_types::error::display::DisplayErrorContext(&err)
                    )));
                }
            };
            let body = output.body.collect().await.map_err(|e| {
                StoreError::Io(format!(
                    "{:#}",
                    aws_smithy_types::error::display::DisplayErrorContext(&e)
                ))
            })?;
            Ok(Some(body.into_bytes().to_vec()))
        })
    }

    /// Unconditionally removes `key`. Not part of `ConditionalStore`, same
    /// reasoning as `InMemoryStore::delete`: no reader or writer in the
    /// commit protocol itself ever deletes an object. This exists for
    /// crash/compaction tests and, later, the M3 orphan-sweep tool.
    pub fn delete(&self, key: &str) -> Result<(), StoreError> {
        self.runtime.block_on(async {
            self.client
                .delete_object()
                .bucket(&self.bucket)
                .key(key)
                .send()
                .await
                .map_err(|e| {
                    StoreError::Io(format!(
                        "{:#}",
                        aws_smithy_types::error::display::DisplayErrorContext(&e)
                    ))
                })?;
            Ok(())
        })
    }
}

/// Was this PUT rejected by an `If-None-Match`/`If-Match` precondition?
/// Neither condition has a modeled error variant in the SDK (S3's
/// conditional-write support postdates most of the typed error surface),
/// so this checks the raw HTTP status, per aws-smithy's documented escape
/// hatch (`SdkError::raw_response`) for exactly this situation.
fn is_precondition_failed<E>(err: &aws_sdk_s3::error::SdkError<E>) -> bool {
    err.raw_response()
        .is_some_and(|r| r.status().as_u16() == 412)
}

/// Did this write fail in a way that leaves its outcome genuinely unknown,
/// as opposed to a definite non-application? Per
/// `aws_smithy_runtime_api::client::result::SdkError`'s own variant
/// documentation (vendored by inspecting the crate source directly, not
/// from memory — CLAUDE.md §3): `TimeoutError` and `DispatchFailure` are
/// each documented "the request MAY have been sent"; `ResponseError` means
/// a response started arriving and then stopped short of being parseable
/// (its own doc example: "the server hung up without sending a complete
/// response") — the server may already have committed the write before the
/// connection dropped. `ConstructionFailure` never left the client, and
/// `ServiceError` (including the 412 case handled separately above) is a
/// complete, well-formed answer from the service — both are definite, and
/// fall through to `StoreError::Io`.
fn is_ambiguous_outcome<E, R>(err: &aws_sdk_s3::error::SdkError<E, R>) -> bool {
    use aws_sdk_s3::error::SdkError;
    matches!(
        err,
        SdkError::TimeoutError(_) | SdkError::DispatchFailure(_) | SdkError::ResponseError(_)
    )
}

fn classify_write_error<E>(err: aws_sdk_s3::error::SdkError<E>) -> StoreError
where
    E: std::error::Error + Send + Sync + 'static,
{
    if is_precondition_failed(&err) {
        return StoreError::PreconditionFailed;
    }
    let formatted = format!(
        "{:#}",
        aws_smithy_types::error::display::DisplayErrorContext(&err)
    );
    if is_ambiguous_outcome(&err) {
        StoreError::Ambiguous(formatted)
    } else {
        StoreError::Io(formatted)
    }
}

impl ConditionalStore for S3Store {
    fn get(&self, key: &str) -> Result<Option<(Vec<u8>, ETag)>, StoreError> {
        self.runtime.block_on(async {
            let result = self
                .client
                .get_object()
                .bucket(&self.bucket)
                .key(key)
                .send()
                .await;
            let output = match result {
                Ok(output) => output,
                Err(err) => {
                    if err.as_service_error().is_some_and(|e| e.is_no_such_key()) {
                        return Ok(None);
                    }
                    return Err(StoreError::Io(format!(
                        "{:#}",
                        aws_smithy_types::error::display::DisplayErrorContext(&err)
                    )));
                }
            };
            let etag = output.e_tag().unwrap_or_default().to_string();
            let body = output.body.collect().await.map_err(|e| {
                StoreError::Io(format!(
                    "{:#}",
                    aws_smithy_types::error::display::DisplayErrorContext(&e)
                ))
            })?;
            Ok(Some((body.into_bytes().to_vec(), etag)))
        })
    }

    fn put_if_absent(&self, key: &str, bytes: &[u8]) -> Result<ETag, StoreError> {
        self.runtime.block_on(async {
            let result = self
                .client
                .put_object()
                .bucket(&self.bucket)
                .key(key)
                .if_none_match("*")
                .body(ByteStream::from(bytes.to_vec()))
                .send()
                .await;
            result
                .map(|output| output.e_tag().unwrap_or_default().to_string())
                .map_err(classify_write_error)
        })
    }

    fn put_if_match(&self, key: &str, bytes: &[u8], etag: &ETag) -> Result<ETag, StoreError> {
        self.runtime.block_on(async {
            let result = self
                .client
                .put_object()
                .bucket(&self.bucket)
                .key(key)
                .if_match(etag)
                .body(ByteStream::from(bytes.to_vec()))
                .send()
                .await;
            result
                .map(|output| output.e_tag().unwrap_or_default().to_string())
                .map_err(classify_write_error)
        })
    }
}

impl RangeGetStore for S3Store {
    fn get_range(&self, key: &str, start: u64, end: u64) -> Result<Vec<u8>, StoreError> {
        assert!(start < end, "empty or inverted range: {start}..{end}");
        // HTTP `Range` is inclusive on both ends; our own `get_range`
        // contract is the Rust-style half-open `[start, end)`, so the wire
        // header's end byte is `end - 1`.
        let range_header = format!("bytes={start}-{}", end - 1);
        self.runtime.block_on(async {
            let result = self
                .client
                .get_object()
                .bucket(&self.bucket)
                .key(key)
                .range(range_header)
                .send()
                .await;
            let output = result.map_err(|err| {
                StoreError::Io(format!(
                    "{:#}",
                    aws_smithy_types::error::display::DisplayErrorContext(&err)
                ))
            })?;
            let body = output.body.collect().await.map_err(|e| {
                StoreError::Io(format!(
                    "{:#}",
                    aws_smithy_types::error::display::DisplayErrorContext(&e)
                ))
            })?;
            Ok(body.into_bytes().to_vec())
        })
    }
}

impl ListableStore for S3Store {
    /// Lists every object under `prefix` via real `ListObjectsV2`,
    /// following its continuation token internally so a caller sees one
    /// flat, complete listing regardless of how many 1000-key pages S3 (or
    /// MinIO) actually returned — the enumeration primitive the M3-5
    /// orphan sweep's "list the prefix" step (`spec/manifest.md`, "Orphan
    /// files") needs. Each entry's `last_modified_millis` comes straight
    /// from the service's own `LastModified` field
    /// (`aws_smithy_types::DateTime::to_millis`), the sweep's staleness
    /// signal for its retention-window safety margin.
    fn list(&self, prefix: &str) -> Result<Vec<ListedObject>, StoreError> {
        self.runtime.block_on(async {
            let mut result = Vec::new();
            let mut continuation_token: Option<String> = None;
            loop {
                let mut request = self
                    .client
                    .list_objects_v2()
                    .bucket(&self.bucket)
                    .prefix(prefix);
                if let Some(token) = continuation_token.take() {
                    request = request.continuation_token(token);
                }
                let output = request.send().await.map_err(|err| {
                    StoreError::Io(format!(
                        "{:#}",
                        aws_smithy_types::error::display::DisplayErrorContext(&err)
                    ))
                })?;
                for object in output.contents() {
                    let key = object.key().ok_or_else(|| {
                        StoreError::Io("ListObjectsV2 returned an object with no Key".to_string())
                    })?;
                    let last_modified = object.last_modified().ok_or_else(|| {
                        StoreError::Io(format!(
                            "ListObjectsV2 returned no LastModified for key {key}"
                        ))
                    })?;
                    let millis = last_modified.to_millis().map_err(|e| {
                        StoreError::Io(format!(
                            "could not convert LastModified for key {key} to milliseconds: {e}"
                        ))
                    })?;
                    result.push(ListedObject {
                        key: key.to_string(),
                        // A `DateTime` before the Unix epoch would produce a
                        // negative value; no conforming backend reports an
                        // object written before 1970, so clamping to 0
                        // rather than propagating an error keeps the common
                        // case simple without silently misreporting a real
                        // negative age anywhere reachable in practice.
                        last_modified_millis: millis.max(0) as u64,
                    });
                }
                match output.next_continuation_token() {
                    Some(token) => continuation_token = Some(token.to_string()),
                    None => break,
                }
            }
            result.sort_unstable_by(|a: &ListedObject, b: &ListedObject| a.key.cmp(&b.key));
            Ok(result)
        })
    }
}

impl DeletableStore for S3Store {
    fn delete_object(&self, key: &str) -> Result<(), StoreError> {
        self.delete(key)
    }
}
