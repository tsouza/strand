# tantivy — field-length (fieldnorm) accounting has no `discountOverlaps` equivalent

Vendored excerpts, fetched against live source (`git clone`, not the summarizing
WebFetch model — this file's contents are load-bearing for byte-level conformance
work), not memory, per `CLAUDE.md` §3.

**Source repo:** `github.com/quickwit-oss/tantivy`. **Pinned:** tag `0.26.1` — the
same tag `references/r11a-tantivy-reader-surface-and-lucene-codec-spi.md` already
pinned this session, for consistency. The tag is an annotated tag object,
`0093923d94157d9f1f63a292bb504bb8db401f2a` (confirmed via
`GET /repos/quickwit-oss/tantivy/git/refs/tags/0.26.1`), which peels to commit
`d8f4c0b703120ed98f06297724dc1522df6019b9` (confirmed via
`GET /repos/quickwit-oss/tantivy/git/tags/0093923d94157d9f1f63a292bb504bb8db401f2a`)
— the actual commit checked out by `git clone --depth 1 --branch 0.26.1`. Fetched
2026-08-19.

Cited by: `docs/ledger.md` R4, `rfcs/0004-analyzer-descriptors.md` §6
(`counts_overlaps_in_length`), `CLAUDE.md` invariant 5 ("parity within Lucene's
one-byte norm quantization") and invariant 6 (per-document length definition) —
this file grounds the tantivy half of the question RFC 0004 left explicitly open in
its Non-goals section, answering the exact same question
`references/lucene-bm25similarity-and-smallfloat.md` already grounded for Lucene:
does the engine's per-document field length, as recorded for norm/BM25 purposes,
discount tokens that share a position with a preceding token (Lucene calls these
"overlaps," e.g. synonym-expansion output), or does it count every token
unconditionally?

## Answer, up front

**tantivy counts every token unconditionally. There is no discounting mechanism at
all — not enabled by default, not available as an option.** A repo-wide search
confirms the concept does not exist in tantivy's source:

```
$ grep -rn "discount" --include="*.rs" .
(no matches)
```

This contrasts with Lucene, where `discountOverlaps` is a named boolean field on
`BM25Similarity`, defaulting to `true`
(`references/lucene-bm25similarity-and-smallfloat.md`). tantivy has no analogous
field, method, or code path anywhere in its tree — the question "does tantivy
default this on or off" is malformed, because there is nothing to default: the
behavior is unconditional counting, full stop, with no toggle.

## Where the count happens

Field length for a `Str` field is `IndexingPosition::num_tokens`, accumulated while
tokenizing and passed straight into `FieldNormsWriter::record` as the raw
`fieldnorm` value — no `- num_overlap` term of any kind, unlike Lucene's
`computeNorm`.

`src/indexer/segment_writer.rs:194-221` (tag `0.26.1`, commit `d8f4c0b7`):

```rust
FieldType::Str(_) => {
    let mut indexing_position = IndexingPosition::default();
    for value in values {
        let value = value.as_value();

        let mut token_stream = if let Some(text) = value.as_str() {
            let text_analyzer =
                &mut self.per_field_text_analyzers[field.field_id() as usize];
            text_analyzer.token_stream(text)
        } else if let Some(tok_str) = value.into_pre_tokenized_text() {
            BoxTokenStream::new(PreTokenizedStream::from(*tok_str.clone()))
        } else {
            continue;
        };

        assert!(term_buffer.is_empty());
        postings_writer.index_text(
            doc_id,
            &mut *token_stream,
            term_buffer,
            ctx,
            &mut indexing_position,
        );
    }
    if field_entry.has_fieldnorms() {
        self.fieldnorms_writer
            .record(doc_id, field, indexing_position.num_tokens);
    }
}
```

`indexing_position.num_tokens` is produced by `PostingsWriter::index_text`,
`src/postings/postings_writer.rs:97-161`:

```rust
#[derive(Default, Debug)]
pub(crate) struct IndexingPosition {
    pub num_tokens: u32,
    pub end_position: u32,
}

...

fn index_text(
    &mut self,
    doc_id: DocId,
    token_stream: &mut dyn TokenStream,
    term_buffer: &mut IndexingTerm,
    ctx: &mut IndexingContext,
    indexing_position: &mut IndexingPosition,
) {
    let end_of_path_idx = term_buffer.len_bytes();
    let mut num_tokens = 0;
    let mut end_position = indexing_position.end_position;
    token_stream.process(&mut |token: &Token| {
        // We skip all tokens with a len greater than u16.
        if token.text.len() > MAX_TOKEN_LEN {
            warn!(...);
            return;
        }
        term_buffer.truncate_value_bytes(end_of_path_idx);
        term_buffer.append_bytes(token.text.as_bytes());
        let start_position = indexing_position.end_position + token.position as u32;
        end_position = end_position.max(start_position + token.position_length as u32);
        self.subscribe(doc_id, start_position, term_buffer, ctx);
        num_tokens += 1;
    });

    indexing_position.end_position = end_position + POSITION_GAP;
    indexing_position.num_tokens += num_tokens;
    term_buffer.truncate_value_bytes(end_of_path_idx);
}
```

Every `Token` the stream yields — with the sole exception of tokens longer than
`MAX_TOKEN_LEN`, dropped as a size limit unrelated to position — increments
`num_tokens` by exactly 1, unconditionally. There is no branch here that inspects
`token.position` (whether this token's start position collides with the previous
token's, i.e. an overlap) or `token.position_length` (whether this token spans more
than one nominal position, tantivy's rough equivalent of a Lucene multi-word
synonym) to decide whether the token counts toward length. Both fields affect only
`end_position` — the running position cursor used to place the *next* field value's
tokens (relevant for multi-valued `Str` fields, separated by `POSITION_GAP`) — never
whether the current token is included in `num_tokens`.

`FieldNormsWriter::record`, `src/fieldnorm/writer.rs:73`, takes this `fieldnorm: u32`
and stores it (through `fieldnorm_to_id`, tantivy's own lossy byte encoding,
`src/fieldnorm/code.rs`) with no further adjustment:

```rust
/// * fieldnorm - the number of terms present in document `doc` in field `field`
pub fn record(&mut self, doc: DocId, field: Field, fieldnorm: u32) {
```

The doc comment itself says it plainly: "the number of terms present" — a raw
count, not a discounted one.

## Confirmation from tantivy's own tests: `position_length` never changes the count

Three of tantivy's own unit tests in `src/indexer/segment_writer.rs` exercise
`position_length` directly and confirm it is purely a position-gap mechanism, not a
length-discount mechanism:

- `test_multiple_field_value_and_long_tokens` (`segment_writer.rs:970-1002`) feeds a
  single token with `position_length: 2` into a field twice; the assertion checks
  the *positions* recorded for the second occurrence (`&[0, 3]`, "as opposed to
  `0, 2` if we had a position length of 1") — proving `position_length` shifts where
  the next value's tokens land, and saying nothing about a token count being
  reduced.
- `test_last_token_not_ending_last` (`segment_writer.rs:1006-1039`) is the same
  pattern with `position_length: 3` on one token followed by a `position_length: 1`
  token, again asserting only positions.
- Neither test, nor any other test in the file, asserts a `fieldnorm` or `num_tokens`
  value that differs from a plain per-token count.

## What this means for the invariant-6 `counts_overlaps_in_length` field and M4

`rfcs/0004-analyzer-descriptors.md` §6 defines `counts_overlaps_in_length` as a
boolean the descriptor declares, and requires `lucene-parity` scoring to set it
`false` (matching Lucene's `discountOverlaps = true`, which *excludes* overlaps).
Mapped onto tantivy's real behavior just grounded above: **tantivy's native
field-length computation is equivalent to `counts_overlaps_in_length = true`** —
every token counts, unconditionally, including any that would occupy an
overlapping position (a synonym-style expansion, were one applied upstream of
indexing; tantivy ships no synonym filter in its own tokenizer set, but nothing in
`index_text` would discount such tokens if a caller-supplied token stream produced
them).

Consequence for M4's tantivy-fork parity claim (`docs/milestones.md`,
`docs/ledger.md` R11): a STRAND-compatible tantivy fork targeting `lucene-parity`
scoring cannot rely on tantivy's stock field-length accounting for documents whose
analysis chain produces same-position (overlapping) tokens — it differs from
Lucene's default by exactly `num_overlap` tokens for such documents, matching
Lucene's *non-default* `discountOverlaps = false` behavior instead. The fork must
patch `PostingsWriter::index_text` (or the `FieldType::Str` call site in
`segment_writer.rs`) to subtract same-position tokens from `num_tokens` before
calling `fieldnorms_writer.record`, the same place this file identifies as the
unconditional-count site. For STRAND's own v0.1 tokenizer profile, which has no
synonym-expansion step (`rfcs/0004-analyzer-descriptors.md` §Non-goals), this gap is
inert today — it only bites once a synonym-style filter enters either engine's
chain — which is exactly why RFC 0004 correctly scoped it as gating M4, not M1.

## Version scope

This grounding targets tag `0.26.1` specifically (matching the version this session
already pinned for R11(a)), not tantivy's `main` branch. No `discountOverlaps`-like
feature request or in-progress branch was observed during this fetch; a future
session revisiting this for a newer tantivy release should re-run the same
`grep -rn "discount"` repo-wide check before assuming this finding still holds.
