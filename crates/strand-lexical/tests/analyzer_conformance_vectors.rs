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

//! Runs every normative token-stream vector in `conformance/analyzers/`
//! (invariant 6, `CLAUDE.md` §5) against `strand_lexical::analyzer`. Each
//! vector is raw text in, an analyzer descriptor, and the exact token
//! stream out — an implementation that fails a vector does not conform,
//! whatever its descriptor claims.

use serde_json::Value;
use strand_lexical::analyzer::analyze_lucene_en_word_only;

fn conformance_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/analyzers"
    ))
}

#[test]
fn lucene_en_word_only_01_matches_rfc_0004s_worked_example() {
    let path = conformance_dir().join("lucene-en-word-only-01.json");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let vector: Value = serde_json::from_str(&raw).unwrap();

    let input = vector["input"].as_str().unwrap();
    let expected_tokens: Vec<String> = vector["expected_tokens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    let expected_document_length = vector["expected_document_length"].as_u64().unwrap();

    // This vector's descriptor (embedded in the JSON for a future
    // language-neutral reader) is exactly the chain
    // analyze_lucene_en_word_only implements: UAX29-word tokenization with
    // "word-only" retention, "lower" case folding, lucene-en-10.5.1
    // stopwords, snowball-porter2-en stemming.
    let descriptor = &vector["descriptor"];
    assert_eq!(
        descriptor["tokenizer_profile"]["token_retention"],
        "word-only"
    );
    assert_eq!(descriptor["tokenizer_profile"]["case_folding"], "lower");
    assert_eq!(descriptor["stopword_list_id"], "lucene-en-10.5.1");
    assert_eq!(descriptor["stemmer"]["name"], "snowball-porter2-en");
    assert_eq!(descriptor["counts_overlaps_in_length"], false);

    let actual_tokens = analyze_lucene_en_word_only(input);
    assert_eq!(actual_tokens, expected_tokens);
    assert_eq!(actual_tokens.len() as u64, expected_document_length);
}
