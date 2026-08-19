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

//! A concrete analysis chain matching one analyzer descriptor from
//! `spec/analyzer-descriptors.md` (RFC 0004): UAX #29 `"word-only"` word
//! tokenization, `"lower"` case folding, Lucene English stopword removal,
//! and Snowball Porter2 English stemming — the exact chain RFC 0004's
//! worked example describes and `conformance/analyzers/` pins as a golden
//! vector (invariant 6).
//!
//! Built from real, licensed implementations rather than hand-rolled
//! algorithms (`CLAUDE.md` §3): `unicode-segmentation`
//! (`references/unicode-segmentation-crate-license.md`) for tokenization,
//! and `rust-stemmers` (a Rust port of the Snowball project's own generated
//! code, `references/snowball-porter2-english-stemmer.md`) for stemming.
//!
//! Also implements `segmentation_dictionary` (`spec/analyzer-descriptors.md`
//! §5) for CJK/Thai/Lao content: ICU4X's `icu_segmenter` crate
//! (`references/icu4x-icu-segmenter-crate.md`,
//! `references/icu-license-word-break-dictionaries.md`), the default RFC
//! 0004's Discussion — post-approval amendments recommends.

use icu_segmenter::options::WordBreakInvariantOptions;
use icu_segmenter::{WordSegmenter, WordSegmenterBorrowed};
use rust_stemmers::{Algorithm, Stemmer};
use std::collections::HashSet;
use std::sync::LazyLock;
use unicode_segmentation::UnicodeSegmentation;

/// Apache Lucene 10.5.1's default English stopword list, exactly as declared
/// in `EnglishAnalyzer` (`references/lucene-english-stopwords.md`) — the
/// `lucene-en-10.5.1` `stopword_list_id`.
pub const LUCENE_EN_10_5_1_STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "but", "by", "for", "if", "in", "into", "is", "it",
    "no", "not", "of", "on", "or", "such", "that", "the", "their", "then", "there", "these",
    "they", "this", "to", "was", "will", "with",
];

/// Tokenizes per `token_retention: "word-only"`: UAX #29 word-boundary
/// segmentation, retaining only segments containing at least one character
/// with the `Alphabetic` property or `General_Category = Number`
/// (`references/unicode-segmentation-word-filter-criterion.md`) — exactly
/// `UnicodeSegmentation::unicode_words`, which implements this criterion
/// directly.
pub fn tokenize_word_only(text: &str) -> Vec<&str> {
    text.unicode_words().collect()
}

/// `case_folding: "lower"` — ordinary Unicode lowercasing, distinct from
/// full case folding (German ß has no simple lowercase mapping and is left
/// unchanged here; `references/unicode-casefolding-sharp-s.md`).
pub fn lowercase(token: &str) -> String {
    token.to_lowercase()
}

/// Removes any token present in `stopwords`, preserving order.
pub fn remove_stopwords(tokens: Vec<String>, stopwords: &HashSet<&str>) -> Vec<String> {
    tokens
        .into_iter()
        .filter(|t| !stopwords.contains(t.as_str()))
        .collect()
}

/// Snowball Porter2 English stemming (`stemmer.name = "snowball-porter2-en"`).
pub fn stem_en(tokens: &[String]) -> Vec<String> {
    let stemmer = Stemmer::create(Algorithm::English);
    tokens
        .iter()
        .map(|t| stemmer.stem(t).into_owned())
        .collect()
}

/// The full chain for the descriptor RFC 0004's worked example names:
/// `token_retention: "word-only"` → `case_folding: "lower"` → stopword
/// removal against `lucene-en-10.5.1` → `snowball-porter2-en` stemming.
/// Per-document length (`spec/analyzer-descriptors.md` §6) is the returned
/// vector's length, since this descriptor's `counts_overlaps_in_length` is
/// `false` and this chain produces no overlaps to begin with.
pub fn analyze_lucene_en_word_only(text: &str) -> Vec<String> {
    let stopwords: HashSet<&str> = LUCENE_EN_10_5_1_STOPWORDS.iter().copied().collect();
    let lowered: Vec<String> = tokenize_word_only(text)
        .into_iter()
        .map(lowercase)
        .collect();
    let filtered = remove_stopwords(lowered, &stopwords);
    stem_en(&filtered)
}

/// The `identity` this crate names in a `segmentation_dictionary` descriptor
/// (`spec/analyzer-descriptors.md` §5) when it segments with
/// [`tokenize_dictionary_segmented`].
pub const ICU4X_DICTIONARY_IDENTITY: &str = "icu4x-dictionary";

/// The `version` this crate names in a `segmentation_dictionary` descriptor.
/// Pins both the `icu_segmenter` crate's own semver and the
/// `icu_segmenter_data` crate's semver that actually carries the compiled
/// dictionary bytes `WordSegmenter::new_dictionary` bakes in at compile
/// time — the two version independently in principle even though they have
/// matched at every release fetched for this implementation
/// (`references/icu4x-icu-segmenter-crate.md`). A crate-semver pin is a
/// coarser byte-determinism anchor than a content hash of the compiled
/// dictionary data itself would be (invariant 11, `CLAUDE.md` §5, prefers
/// checksums where practical) — `spec/analyzer-descriptors.md` §5 records
/// this as the chosen mechanism and names the content-hash upgrade as
/// still-open follow-on work, the same way RFC 0004 itself left the
/// stemmer's commit-hash pinning to a later session.
pub const ICU4X_DICTIONARY_VERSION: &str = "icu_segmenter 2.3.0 (icu_segmenter_data 2.3.0)";

/// ICU4X's dictionary-based `WordSegmenter`, constructed once and reused.
///
/// This calls `WordSegmenter::new_dictionary` — the real, infallible,
/// `compiled_data`-feature constructor for the dictionary-lookup path
/// (Chinese, Japanese, Khmer, Lao, Myanmar, Thai) that this crate's
/// `segmentation_dictionary` field names. It deliberately does **not** call
/// `new_auto`/`try_new_auto`, which silently substitutes an LSTM model for
/// Thai, Lao, Khmer, and Myanmar instead of dictionary lookup — the trap
/// RFC 0004's Discussion — post-approval amendments names explicitly
/// (`references/icu4x-icu-segmenter-crate.md`). This crate additionally
/// disables the `icu_segmenter` crate's `auto`/`lstm` Cargo features
/// entirely (`Cargo.toml`), so `new_auto`/`new_lstm` are not even reachable
/// from this file — a compile-time guardrail on top of the code-level one.
static DICTIONARY_WORD_SEGMENTER: LazyLock<WordSegmenterBorrowed<'static>> =
    LazyLock::new(|| WordSegmenter::new_dictionary(WordBreakInvariantOptions::default()));

/// Tokenizes a single dictionary-segmented script's content (CJK, Thai, Lao)
/// per `token_retention: "word-only"`: ICU4X dictionary word-boundary
/// segmentation, retaining only segments containing at least one character
/// with the Unicode `Alphabetic` property or `General_Category = Number`
/// (`references/unicode-segmentation-word-filter-criterion.md`) — the same
/// retention criterion [`tokenize_word_only`] applies via
/// `unicode_words()`, reimplemented here directly against `char::
/// is_alphabetic`/`char::is_numeric` because ICU4X's segmenter, unlike
/// `unicode-segmentation`, does not bundle this filter itself.
pub fn tokenize_dictionary_segmented(text: &str) -> Vec<&str> {
    let breaks: Vec<usize> = DICTIONARY_WORD_SEGMENTER.segment_str(text).collect();
    breaks
        .windows(2)
        .map(|w| &text[w[0]..w[1]])
        .filter(|segment| segment.chars().any(|c| c.is_alphabetic() || c.is_numeric()))
        .collect()
}

/// The full chain for a `segmentation_dictionary`-bearing descriptor with no
/// case folding, stopword removal, or stemming declared (`spec/
/// analyzer-descriptors.md` §5) — the shape of the conformance vector this
/// function backs (`conformance/analyzers/icu4x-dictionary-zh-01.json`):
/// `token_retention: "word-only"` dictionary segmentation only. Per-document
/// length is the returned vector's length.
pub fn analyze_dictionary_segmented_word_only(text: &str) -> Vec<String> {
    tokenize_dictionary_segmented(text)
        .into_iter()
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stems_match_the_vendored_snowball_data_test_vectors() {
        // references/snowball-porter2-english-stemmer.md's table, from the real
        // snowballstem/snowball-data voc.txt/output.txt corpus, not predicted.
        let cases = [
            ("whales", "whale"),
            ("whale", "whale"),
            ("swim", "swim"),
            ("swimming", "swim"),
            ("quickly", "quick"),
            ("quick", "quick"),
            ("running", "run"),
            ("run", "run"),
            ("runs", "run"),
        ];
        let inputs: Vec<String> = cases.iter().map(|(input, _)| input.to_string()).collect();
        let stems = stem_en(&inputs);
        for ((input, expected), actual) in cases.iter().zip(stems.iter()) {
            assert_eq!(actual, expected, "stem({input})");
        }
    }

    #[test]
    fn word_only_retention_drops_pure_punctuation_segments() {
        // "quickly." splits into "quickly" and "." under UAX #29; "." has no
        // Alphabetic/Number character and fails word-only retention
        // (references/unicode-segmentation-word-filter-criterion.md).
        let words = tokenize_word_only("quickly.");
        assert_eq!(words, vec!["quickly"]);
    }

    #[test]
    fn dictionary_segmentation_splits_real_chinese_sentence_and_drops_punctuation() {
        // Real ICU4X dictionary output for "这是一个中文测试句子。" ("This is a
        // Chinese test sentence."), captured by running the real segmenter
        // (not predicted): ["这", "是", "一个", "中文", "测试", "句子", "。"].
        // "。" (IDEOGRAPHIC FULL STOP) has no Alphabetic/Number character and
        // fails word-only retention, the same as "." does for the English
        // chain above.
        let words = tokenize_dictionary_segmented("这是一个中文测试句子。");
        assert_eq!(words, vec!["这", "是", "一个", "中文", "测试", "句子"]);
    }

    #[test]
    fn dictionary_segmentation_uses_dictionary_lookup_not_lstm_for_thai() {
        // Real ICU4X dictionary-path output for a Thai sentence containing
        // no punctuation, so every segment survives word-only retention.
        // Captured by running the real segmenter, not predicted.
        let words = tokenize_dictionary_segmented("ฉันรักการเขียนโปรแกรม");
        assert_eq!(words, vec!["ฉัน", "รัก", "การ", "เขียน", "โปรแกรม"]);
    }

    #[test]
    fn lower_case_folding_leaves_sharp_s_unchanged() {
        // references/unicode-casefolding-sharp-s.md: ß has no simple/common
        // lowercase mapping, only a full-case-fold one this profile doesn't use.
        assert_eq!(lowercase("straße"), "straße");
    }
}
