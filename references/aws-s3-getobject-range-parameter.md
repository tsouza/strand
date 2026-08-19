# AWS `GetObject` API reference — the `Range` request header

Source: <https://docs.aws.amazon.com/AmazonS3/latest/API/API_GetObject.html>,
fetched 2026-08-19. This is the primary source RFC 0001 §1 already cites for the
claim that AWS's documentation "demonstrates only the explicit-end form in its
examples"; this file vendors the exact passages so that claim is checkable rather
than remembered, per `CLAUDE.md` §3.

## The `Range` parameter's own description

> Downloads the specified byte range of an object. For more information about the
> HTTP Range header, see <https://www.rfc-editor.org/rfc/rfc9110.html#name-range>.
>
> Amazon S3 doesn't support retrieving multiple ranges of data per `GET` request.

This points a reader at RFC 9110 §14.2 "Range" generally — not specifically at
§14.1.2, the subsection that defines the suffix-length form (`bytes=-500`). It
neither confirms nor rules out the suffix form; it simply defers to the RFC's
general `Range` header syntax without restricting or expanding on it. The only
concrete restriction stated is multi-range requests, which is unrelated to this
question.

## The worked example: explicit-end form only

The only concrete `Range` example anywhere on the page uses the explicit-end
form, retrieving the first 10 bytes of a 443-byte object:

**Sample request:**

```
GET /example-object HTTP/1.1
Host: amzn-s3-demo-bucket.s3.<Region>.amazonaws.com
x-amz-date: Fri, 28 Jan 2011 21:32:02 GMT
Range: bytes=0-9
Authorization: AWS AKIAIOSFODNN7EXAMPLE:Yxg83MZaEgh3OZ3l0rLo5RTX11o=
```

**Sample response:**

```
HTTP/1.1 206 Partial Content
...
Accept-Ranges: bytes
Content-Range: bytes 0-9/443
Content-Type: text/plain
Content-Length: 10
Server: AmazonS3

[10 bytes of object data]
```

No suffix-range (`bytes=-N`) example appears anywhere on the `GetObject` page.
This confirms RFC 0001's own framing precisely: AWS's documentation is silent on
the suffix form, not a "no" — an absent example is not evidence either way, which
is why this project resolves the question empirically against MinIO instead of
inferring an answer from the documentation's silence
(`rfcs/0001-container-rowid-manifest.md` Open questions, item 3; the empirical
MinIO result is recorded directly in `crates/strand-core/tests/s3_store.rs`'s
`suffix_range_get_is_honored_by_minio` test and in RFC 0001's Discussion section).

Real S3 itself was not tested directly — this session has no AWS account
credentials — so the suffix-range question against actual S3 (as opposed to
MinIO's server-side implementation) remains open, consistent with RFC 0001's own
Open Questions framing; only the MinIO half of "confirming empirically" could be
closed here.
