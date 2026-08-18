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

use crate::store::{ConditionalStore, ETag, StoreError};
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
                    return Err(StoreError::Io(err.to_string()));
                }
            };
            let etag = output.e_tag().unwrap_or_default().to_string();
            let body = output
                .body
                .collect()
                .await
                .map_err(|e| StoreError::Io(e.to_string()))?;
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
            match result {
                Ok(output) => Ok(output.e_tag().unwrap_or_default().to_string()),
                Err(err) if is_precondition_failed(&err) => Err(StoreError::PreconditionFailed),
                Err(err) => Err(StoreError::Io(err.to_string())),
            }
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
            match result {
                Ok(output) => Ok(output.e_tag().unwrap_or_default().to_string()),
                Err(err) if is_precondition_failed(&err) => Err(StoreError::PreconditionFailed),
                Err(err) => Err(StoreError::Io(err.to_string())),
            }
        })
    }
}
