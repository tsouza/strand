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

//! Real, measured Chinese word-segmentation bake-off between ICU4X's
//! `icu_segmenter` dictionary path (`WordSegmenter::new_dictionary`,
//! `references/icu4x-icu-segmenter-crate.md`) — RFC 0004's recommended
//! `segmentation_dictionary` default, adopted on license and
//! dependency-shape grounds, explicitly *not* on measured accuracy — and
//! `jieba-rs`, a real, actively maintained, MIT-licensed Chinese-specific
//! segmenter (`references/jieba-and-jieba-rs-license.md`).
//!
//! `docs/roadmap.md` M1-7. RFC 0004's Discussion — post-approval amendments
//! ("How this could be wrong") names this gap explicitly: "no comparison
//! was run between ICU4X's dictionary output and Lindera+IPADIC for
//! Japanese, or Jieba for Chinese, or PyThaiNLP for Thai... a real
//! bake-off... is named as still-open work." This binary closes the
//! **Chinese** slice of that gap with a real measurement. Japanese
//! (Lindera+IPADIC) and Thai (PyThaiNLP) are explicitly NOT covered here —
//! see the module-level scope note in the JSON output and the RFC 0004
//! amendment this run adds, for why (Japanese needs a real IPADIC
//! download and dictionary-license audit beyond `references/
//! lindera-rust-morphological-analyzer.md`'s existing code-license check;
//! Thai has no maintained Rust binding at all per `references/
//! pythainlp-license-and-rust-gap.md`), rather than silently omitted.
//!
//! ## What this measures, and what it does not
//!
//! This is an **inter-segmenter agreement** measurement, not accuracy
//! against a gold standard. No SIGHAN-bakeoff-style gold-segmented Chinese
//! test set was found in this pass with a citation this session could
//! verify directly (the standard SIGHAN 2005 bakeoff corpora are
//! distributed via a registration-gated academic mirror, not a plain
//! fetchable URL — `CLAUDE.md` §3 forbids citing a dataset from memory
//! without verifying the actual fetch), so this run measures how often two
//! real, independent segmenters agree on where word boundaries fall in the
//! same real text, not which one is "more correct." Where they disagree,
//! that is reported as disagreement, not attributed to either segmenter
//! being wrong.
//!
//! ## Inputs
//!
//! Eight real sentences from the plain-text (`explaintext=1`) lead-section
//! extract of four Chinese Wikipedia articles, fetched live via the
//! official MediaWiki API on 2026-08-19 (not typed from memory, not
//! translated, not modified — verbatim substrings of the fetched
//! `extract` field):
//! - `https://zh.wikipedia.org/wiki/人工智能` ("Artificial intelligence")
//! - `https://zh.wikipedia.org/wiki/北京市` ("Beijing")
//! - `https://zh.wikipedia.org/wiki/计算机科学` ("Computer science")
//! - `https://zh.wikipedia.org/wiki/长城` ("Great Wall")
//!
//! Fetched via:
//! `https://zh.wikipedia.org/w/api.php?action=query&prop=extracts&exintro=1&explaintext=1&format=json&titles=<title>`
//!
//! ## Method
//!
//! For each sentence, both segmenters run over the identical `&str`:
//! - ICU4X: `WordSegmenter::new_dictionary(WordBreakInvariantOptions::default())`
//!   (the RFC-0004-recommended, non-LSTM, dictionary-path constructor,
//!   version pinned in `bench/Cargo.toml` — `icu_segmenter 2.3.0` at time
//!   of writing) `.segment_str(sentence)`, which yields UTF-8 byte-offset
//!   boundaries (confirmed against the crate's real vendored source,
//!   `icu_segmenter-2.3.0/src/word.rs`: `WordBreakIterator<..., Utf8>`
//!   built from `input.char_indices()`).
//! - `jieba-rs`: `Jieba::new()` (the bundled MIT-licensed default
//!   dictionary) `.cut(sentence, true)` — `hmm=true`, matching upstream
//!   Jieba's own default (`jieba.cut()`'s Python default is `HMM=True`).
//!   Each returned `Token` carries real `byte_start`/`byte_end` fields
//!   (confirmed against `jieba-rs-0.10.3/src/lib.rs`), the same UTF-8 byte
//!   unit ICU4X uses, so boundaries compare directly with no unit
//!   conversion.
//!
//! Both segmenters always agree on the sentence's start (0) and end (byte
//! length) — `icu_segmenter`'s own doc comment states this explicitly
//! ("There are always breakpoints at 0 and the string length"). The
//! candidate universe for agreement is therefore every *interior* UTF-8
//! char boundary (every position a `char_indices()` walk could stop at,
//! excluding 0 and the final length) — not just positions either segmenter
//! actually proposed. For each candidate position, both segmenters either
//! agree (both cut, or both do not cut) or disagree. The reported
//! **agreement rate** is agreeing positions divided by total candidate
//! positions, aggregated two ways: macro (mean of per-sentence rates) and
//! micro (total agreeing positions over total candidate positions across
//! all sentences). A secondary Jaccard figure (intersection over union of
//! the two segmenters' *proposed* interior boundary sets) is reported
//! alongside for readers who want the "how much do the actual cut points
//! overlap" framing instead.

use icu_segmenter::WordSegmenter;
use icu_segmenter::options::WordBreakInvariantOptions;
use jieba_rs::Jieba;
use serde::Serialize;
use std::collections::BTreeSet;

/// (article title, article URL, one real verbatim sentence from its lead
/// extract). Fetch date for all: 2026-08-19, via the MediaWiki API call
/// documented in the module doc comment above — not the rendered article
/// page, to get byte-exact plain text with no HTML/markdown artifacts.
const SENTENCES: &[(&str, &str, &str)] = &[
    (
        "人工智能 (Artificial intelligence)",
        "https://zh.wikipedia.org/wiki/人工智能",
        "人工智能（英語：artificial intelligence，缩写为AI），是指计算机系统执行通常与人类智慧相关任务的能力，例如学习、推理、解决问题、感知和决策。",
    ),
    (
        "人工智能 (Artificial intelligence)",
        "https://zh.wikipedia.org/wiki/人工智能",
        "自2020年代以来，生成式人工智能已被广泛用于根据文本提示生成图像、音频和视频。",
    ),
    (
        "人工智能 (Artificial intelligence)",
        "https://zh.wikipedia.org/wiki/人工智能",
        "人工智能作为一门学科于1956年成立，该领域在其历史中经历了多次乐观的循环，随后是失望和资金流失的时期，即所谓的AI寒冬。",
    ),
    (
        "北京市 (Beijing)",
        "https://zh.wikipedia.org/wiki/北京市",
        "北京市，简称“京”，旧称“北平”，中华人民共和国的首都及直辖市，是中国的政治、文化、科技、教育、军事和国际交往中心。",
    ),
    (
        "北京市 (Beijing)",
        "https://zh.wikipedia.org/wiki/北京市",
        "北京是一座全球城市，是世界人口第三多的城市和人口最多的首都。",
    ),
    (
        "计算机科学 (Computer science)",
        "https://zh.wikipedia.org/wiki/计算机科学",
        "计算机科学（英語：computer science，缩写为CS）是系统性研究信息与计算的理论基础以及它们在计算机系统中如何实现与应用的实用技术的学科。",
    ),
    (
        "计算机科学 (Computer science)",
        "https://zh.wikipedia.org/wiki/计算机科学",
        "有时公众会误以为计算机科学就是解决计算机问题的事业（比如信息技术），或者只是与使用计算机的经验有关，如玩游戏、上网或者文字处理。",
    ),
    (
        "长城 (Great Wall)",
        "https://zh.wikipedia.org/wiki/长城",
        "长城是在中國大陸華北一帶歷朝修筑的大規模軍用隔離牆的统称，旨在抵御来自欧亚草原的游牧民族入侵。",
    ),
];

#[derive(Serialize)]
struct SentenceResult {
    source_article: String,
    source_url: String,
    sentence: String,
    byte_len: usize,
    candidate_interior_boundaries: usize,
    icu_tokens: Vec<String>,
    jieba_tokens: Vec<String>,
    exact_token_match: bool,
    agreeing_interior_boundaries: usize,
    agreement_rate: f64,
    icu_only_boundaries: usize,
    jieba_only_boundaries: usize,
    jaccard_on_proposed_boundaries: f64,
}

#[derive(Serialize)]
struct BakeoffResult {
    methodology: String,
    scope_note: String,
    icu_segmenter_crate_version: String,
    icu_constructor: String,
    jieba_rs_crate_version: String,
    jieba_cut_mode: String,
    sentence_count: usize,
    sentences: Vec<SentenceResult>,
    macro_average_agreement_rate: f64,
    micro_average_agreement_rate: f64,
    exact_full_sentence_match_count: usize,
}

/// Every interior UTF-8 char-boundary position in `s`: every position a
/// `char_indices()` walk visits, excluding the trivial always-agreed
/// endpoints 0 and `s.len()`.
fn interior_char_boundaries(s: &str) -> BTreeSet<usize> {
    s.char_indices()
        .map(|(i, _)| i)
        .filter(|&i| i != 0)
        .collect()
}

fn run_sentence(
    segmenter: &icu_segmenter::WordSegmenterBorrowed<'static>,
    jieba: &Jieba,
    source_article: &str,
    source_url: &str,
    sentence: &str,
) -> SentenceResult {
    let byte_len = sentence.len();
    let candidates = interior_char_boundaries(sentence);

    let icu_boundaries: BTreeSet<usize> = segmenter
        .segment_str(sentence)
        .filter(|&b| b != 0 && b != byte_len)
        .collect();
    let icu_tokens: Vec<String> = {
        let mut bounds: Vec<usize> = std::iter::once(0)
            .chain(icu_boundaries.iter().copied())
            .chain(std::iter::once(byte_len))
            .collect();
        bounds.dedup();
        bounds
            .windows(2)
            .map(|w| sentence[w[0]..w[1]].to_string())
            .collect()
    };

    let jieba_tokens_raw = jieba.cut(sentence, true);
    let jieba_boundaries: BTreeSet<usize> = jieba_tokens_raw
        .iter()
        .flat_map(|t| [t.byte_start, t.byte_end])
        .filter(|&b| b != 0 && b != byte_len)
        .collect();
    let jieba_tokens: Vec<String> = jieba_tokens_raw
        .iter()
        .map(|t| t.word.to_string())
        .collect();

    let mut agreeing = 0usize;
    for &pos in &candidates {
        let icu_cuts = icu_boundaries.contains(&pos);
        let jieba_cuts = jieba_boundaries.contains(&pos);
        if icu_cuts == jieba_cuts {
            agreeing += 1;
        }
    }
    let total = candidates.len();
    let agreement_rate = if total == 0 {
        1.0
    } else {
        agreeing as f64 / total as f64
    };

    let intersection = icu_boundaries.intersection(&jieba_boundaries).count();
    let union = icu_boundaries.union(&jieba_boundaries).count();
    let jaccard = if union == 0 {
        1.0
    } else {
        intersection as f64 / union as f64
    };

    SentenceResult {
        source_article: source_article.to_string(),
        source_url: source_url.to_string(),
        sentence: sentence.to_string(),
        byte_len,
        candidate_interior_boundaries: total,
        exact_token_match: icu_tokens == jieba_tokens,
        icu_only_boundaries: icu_boundaries.difference(&jieba_boundaries).count(),
        jieba_only_boundaries: jieba_boundaries.difference(&icu_boundaries).count(),
        icu_tokens,
        jieba_tokens,
        agreeing_interior_boundaries: agreeing,
        agreement_rate,
        jaccard_on_proposed_boundaries: jaccard,
    }
}

fn main() {
    let segmenter = WordSegmenter::new_dictionary(WordBreakInvariantOptions::default());
    let jieba = Jieba::new();

    let sentences: Vec<SentenceResult> = SENTENCES
        .iter()
        .map(|(article, url, text)| run_sentence(&segmenter, &jieba, article, url, text))
        .collect();

    for s in &sentences {
        eprintln!(
            "[{}] agreement={:.3} ({}/{}) jaccard={:.3} exact_match={}",
            s.source_article,
            s.agreement_rate,
            s.agreeing_interior_boundaries,
            s.candidate_interior_boundaries,
            s.jaccard_on_proposed_boundaries,
            s.exact_token_match
        );
        eprintln!("  icu:   {:?}", s.icu_tokens);
        eprintln!("  jieba: {:?}", s.jieba_tokens);
    }

    let macro_avg =
        sentences.iter().map(|s| s.agreement_rate).sum::<f64>() / sentences.len() as f64;
    let total_agree: usize = sentences
        .iter()
        .map(|s| s.agreeing_interior_boundaries)
        .sum();
    let total_candidates: usize = sentences
        .iter()
        .map(|s| s.candidate_interior_boundaries)
        .sum();
    let micro_avg = total_agree as f64 / total_candidates as f64;
    let exact_matches = sentences.iter().filter(|s| s.exact_token_match).count();

    eprintln!(
        "\nmacro-average agreement rate: {macro_avg:.4}\nmicro-average agreement rate: {micro_avg:.4}\nexact full-sentence token-list matches: {exact_matches}/{}",
        sentences.len()
    );

    let output = BakeoffResult {
        methodology: "Inter-segmenter boundary agreement over real Chinese Wikipedia \
            sentences, NOT accuracy against a gold-standard segmentation (no verifiably \
            fetchable gold-standard Chinese segmentation corpus was used in this pass, \
            see module doc comment). For each sentence, every interior UTF-8 char-boundary \
            position is a candidate; agreement = both segmenters place a boundary there or \
            both do not, divided by candidate count. Jaccard is a secondary figure over \
            each segmenter's actually-proposed interior boundary set."
            .to_string(),
        scope_note: "Chinese only. Japanese (Lindera+IPADIC) and Thai (PyThaiNLP) are \
            explicitly out of scope for this run: PyThaiNLP has no maintained Rust binding \
            (references/pythainlp-license-and-rust-gap.md), and a Japanese comparison would \
            require downloading and license-auditing IPADIC beyond what references/ \
            lindera-rust-morphological-analyzer.md already checked (code license only) — \
            both remain real, named, still-open follow-on work (docs/roadmap.md M1-7)."
            .to_string(),
        icu_segmenter_crate_version: "2.3.0".to_string(),
        icu_constructor: "WordSegmenter::new_dictionary(WordBreakInvariantOptions::default())"
            .to_string(),
        jieba_rs_crate_version: "0.10.3".to_string(),
        jieba_cut_mode: "Jieba::new().cut(sentence, /* hmm = */ true)".to_string(),
        sentence_count: sentences.len(),
        sentences,
        macro_average_agreement_rate: macro_avg,
        micro_average_agreement_rate: micro_avg,
        exact_full_sentence_match_count: exact_matches,
    };

    let out_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/results/cjk-segmentation-bakeoff.json"
    );
    let json = serde_json::to_string_pretty(&output).unwrap();
    std::fs::write(out_path, &json).unwrap_or_else(|e| panic!("write {out_path}: {e}"));
    eprintln!("Wrote {out_path} ({} bytes)", json.len());
}
