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

use rust_stemmers::{Algorithm, Stemmer};
use std::collections::HashSet;
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
    fn lower_case_folding_leaves_sharp_s_unchanged() {
        // references/unicode-casefolding-sharp-s.md: ß has no simple/common
        // lowercase mapping, only a full-case-fold one this profile doesn't use.
        assert_eq!(lowercase("straße"), "straße");
    }
}
