# RFC 0004: Analyzer descriptors and per-document length

- **Status:** Approved — passed adversarial review (independent re-fetch and
  re-derivation of every primary-source claim and the worked example; citation-
  hygiene, undeclared-processing-step, and ledger-consistency passes). One Critical
  finding (three broken internal/cross-document section citations — the same defect
  class RFC 0003 was sent back for, found recurring here despite this RFC's own
  stated intent to apply that lesson) and two Important findings (an undeclared
  `"word-only"` retention criterion with no normative source, structurally identical
  to the case-folding gap this RFC caught in itself; and an overstated claim to
  fully resolve `docs/ledger.md` R4 when only its Lucene half was grounded) fixed
  and grounded. All Minor findings fixed. No blocking findings remain.
- **Milestone:** M1 — Lexical (`docs/milestones.md`)
- **Spec chapters produced:** `spec/analyzer-descriptors.md`
- **Invariants exercised:** 6 (`CLAUDE.md` §5), which invariant 5 depends on;
  resolves the per-document-length dependency RFC 0003's Non-goals section named as
  open, for Lucene parity specifically. The tantivy half of `docs/ledger.md` R4,
  left open by this RFC (see Non-goals below), was resolved by a later session —
  2026-08-19, `references/tantivy-fieldnorm-overlap-accounting.md` — not by this RFC
  itself; see the addendum at the end of Non-goals.

## Summary

Defines the analyzer-descriptor schema invariant 6 requires — a structured
description of a lexical blob's analysis chain, pinning Unicode version, ICU/CLDR
version, tokenizer profile with any UAX #29 deviations, stopword list identity,
stemmer name and version, and a schema slot (not yet a resolved choice) for
dictionary-segmented scripts — and per-document length, precisely: which tokens
count, at which point in the chain. A single worked example carries raw text through
every stage of a concrete descriptor to an exact token stream and length, grounded
entirely in fetched, checkable data: Unicode 17.0.0's own word-boundary rules,
Lucene 10.5.1's real default English stopword list, and the Snowball project's own
English-stemmer test vectors — not predicted from memory at any step.

## Motivation

Invariant 6 is explicit about why version pinning alone is insufficient: UAX #29 is
not stable across Unicode versions (the annex itself is versioned per release,
`references/unicode-wordbreaktest-and-cldr-version.md`), and U+202F NARROW NO-BREAK
SPACE's `Word_Break` class changed in Unicode 9.0 — documented in the PRI #308
background material vendored during the original R4 grounding pass
(`references/r4-analyzer-conformance-sources.md`). Two engines pinning "Unicode
9.0+" without a shared descriptor can
still tokenize the same text differently across versions past that floor. Hiemstra,
Hendriksen, Kamphuis & de Vries measured the real cost of shipping index exchange
without this: importing a CIFF index built with one tokenizer and querying it with
another dropped MAP from 0.234 to 0.081 on a TREC Robust04 subset — recovered
entirely by forcing both sides through a shared, declared tokenizer
(`references/r4-analyzer-conformance-sources.md`). This RFC exists so STRAND never
has that failure mode: an index either declares its analysis chain precisely enough
to reproduce, or it is not conforming, full stop.

## Non-goals

**Choosing a specific CJK/Thai/Lao segmentation dictionary** was not resolved by
this RFC at Approval. Invariant 6 requires the descriptor carry a dictionary
identity and version for dictionary-segmented scripts; this RFC pins the *schema
slot* those fields occupy (Design §5), not which dictionary STRAND ships as a
default. The default is now resolved — Discussion — post-approval amendments,
below — but resolving *which identity a conforming default names* is still not the
same as *implementing* it: `crates/strand-lexical/src/analyzer.rs` populates
`segmentation_dictionary` for no script yet, and no dictionary-segmented
conformance vector exists in `conformance/analyzers/`. Both remain real,
un-started M1 execution work.

**tantivy's own doc-length accounting** is not grounded here, though `docs/ledger.md`
R4 lists "precise Lucene-vs-tantivy doc-length accounting for the invariant-6 length
definition" as one open item. This RFC grounds the Lucene half fully (§6:
`counts_overlaps_in_length`, mapped directly to Lucene's real `discountOverlaps`
behavior, `references/lucene-bm25similarity-and-smallfloat.md`) and leaves the
tantivy half of R4 genuinely open — found during this RFC's own adversarial review,
not resolved by it. A future session must vendor tantivy's actual length-accounting
source before M4's tantivy-fork parity work can rely on this chapter being complete
for that engine; `docs/ledger.md` R4 is updated to reflect this narrower scope
(Lucene resolved, tantivy still owed) rather than left implying the whole item is
closed.

**Addendum, 2026-08-19 (tantivy half resolved, M1-5):** a later session vendored
tantivy's real indexing-path source (tag `0.26.1`,
`references/tantivy-fieldnorm-overlap-accounting.md`) and found tantivy has **no
`discountOverlaps`-equivalent mechanism at all** — its field-length count
(`IndexingPosition::num_tokens`) increments unconditionally per token, with no
concept of position overlap ever discounting the count. Mapped onto this RFC's own
§6 vocabulary, tantivy's native behavior is equivalent to
`counts_overlaps_in_length = true`, the opposite of what `lucene-parity` scoring
requires — meaning a STRAND-compatible tantivy fork cannot use tantivy's stock
fieldnorm computation unmodified if it claims `lucene-parity` for documents whose
analysis chain can produce same-position tokens. `docs/ledger.md` R4 is updated to
record both halves resolved; this paragraph is left in place, unedited above, as the
honest record of what this RFC itself did and did not ground.

**A catalog of every possible UAX #29 deviation** is not enumerated. The descriptor
carries a `deviations` list (§2 below) as an open-ended, self-describing mechanism;
this RFC does not attempt to pre-list every deviation a future tokenizer might
declare, the same way invariant 4 pins the raw-statistics *principle* for block-max
bounds without this RFC needing to pin every field STRAND's own postings layer will
eventually carry.

**Scoring formulas** are RFC 0003's job, not restated here. This RFC's connection to
RFC 0003 is narrow and explicit: it resolves the one dependency RFC 0003's own
Non-goals section named as open (per-document length, and specifically what
`lucene-parity`'s inherited `discountOverlaps` behavior means for STRAND) and goes no
further.

**Where the descriptor physically lives inside a segment** is deferred to the R2
RFC, for the same reason RFC 0003 deferred its own descriptor's placement: the
lexical blob's byte layout doesn't exist yet, and pinning an offset into a
not-yet-designed structure would be premature. The placement constraint
`spec/scoring-profiles.md` §4 states (zero added round trips beyond invariant 3's
≤2-RTT budget) applies identically here; Napkin math, below, restates it for this
RFC's own descriptor.

## Design

### 1. The analyzer descriptor

JSON, matching the manifest's and RFC 0003's own binary-for-wire/JSON-for-metadata
split:

| field                     | type              | notes                                                    |
| ------------------------- | ----------------- | --------------------------------------------------------- |
| `unicode_version`         | string             | e.g. `"17.0.0"`                                           |
| `icu_version`              | string             | e.g. `"78.3"`                                             |
| `cldr_version`             | string             | e.g. `"48.2"` — ICU and CLDR version independently even when released together (`references/unicode-wordbreaktest-and-cldr-version.md`) |
| `tokenizer_profile`        | object             | §2 below                                                  |
| `stopword_list_id`         | string or `null`   | §3 below; `null` means no stopword filtering              |
| `stemmer`                  | object or `null`   | §4 below; `null` means no stemming                        |
| `segmentation_dictionary`  | object or `null`   | §5 below; MUST be non-null if the field's content uses a dictionary-segmented script (CJK, Thai, Lao) |
| `counts_overlaps_in_length`| boolean            | §6 below — resolves RFC 0003's deferred `discountOverlaps` question |

### 2. `tokenizer_profile`

```
{
  "algorithm": "UAX29-word",
  "unicode_version": <matches the descriptor's own unicode_version>,
  "token_retention": "word-only" | "all-segments",
  "case_folding": "none" | "lower" | "full-case-fold",
  "deviations": [ <string, human-readable, empty if none> ]
}
```

`algorithm: "UAX29-word"` names the Unicode word-boundary algorithm
(`references/unicode-wordbreaktest-and-cldr-version.md`) at the declared
`unicode_version`, applied with **no property overrides** unless `deviations` names
one explicitly — an empty `deviations` list is itself a normative claim (this
descriptor's tokenizer follows stock UAX #29 exactly at this Unicode version), not
merely the absence of a claim. `token_retention` states whether every boundary
segment UAX #29 produces becomes an indexed token (`"all-segments"`) or only
"word-like" ones do (`"word-only"`) — a real behavioral fork invariant 6 does not
itself resolve, so this RFC makes it an explicit, declared field rather than an
unstated default. **UAX #29 itself defines only break/no-break positions between
characters — it has no normative concept of a segment being a "word" versus not**
(confirmed by reading the annex directly: no rule-status or word/non-word
classification exists in its own text). `"word-only"` therefore MUST use a pinned,
external criterion rather than an appeal to "UAX #29's own classification," which
does not exist: a segment is retained if and only if it contains at least one
character with the Unicode `Alphabetic` property or `General_Category = Number` —
`unicode_words()`'s own documented filter
(`references/unicode-segmentation-word-filter-criterion.md`), adopted here as the
normative rule so two conformant descriptors cannot disagree on an edge case (an
isolated combining-mark run, a standalone symbol) with no criterion to appeal to.
`case_folding` states
whether tokens are left as-is (`"none"`), simply lowercased (`"lower"`, ordinary
`String::to_lowercase`-style mapping), or run through Unicode's full case-folding
algorithm (`"full-case-fold"`, which additionally normalizes cases simple
lowercasing does not, such as German ß) — a distinction this RFC's own drafting
initially missed (an earlier draft applied lowercasing in its worked example with no
corresponding schema field, caught while writing "How this could be wrong" below)
and fixes here rather than carrying forward as a known gap.

### 3. `stopword_list_id`

An opaque identifier resolving to an exact, versioned word list — this RFC does not
mandate a registry mechanism for resolving identifiers to lists (that is
implementation/M1-execution scope), only that the identifier MUST be specific enough
that two engines holding the same identifier hold the same list, byte-for-byte. The
worked example below uses `"lucene-en-10.5.1"`, naming the exact vendored list
(`references/lucene-english-stopwords.md`) unambiguously.

### 4. `stemmer`

```
{ "name": <string>, "version": <string> }
```

`version` MUST anchor to something byte-reproducible — a specific released version or
commit hash of the stemmer's own algorithm definition, not a vague "latest." The
worked example uses `{"name": "snowball-porter2-en", "version": "snowballstem/
snowball, confirmed against snowball-data test vectors 2026-08-18"}` (identical,
byte-for-byte, to the JSON in the worked example below — this description and that
literal value MUST NOT drift, since both are meant to describe the same conformance
vector) — Snowball's algorithm is a stable specification with periodic
refinements, so a real conformance harness needs a commit-pinned reference the same
way this RFC's own worked example does
(`references/snowball-porter2-english-stemmer.md`).

### 5. `segmentation_dictionary`

```
{ "script": <string, a Unicode Script value>, "identity": <string>, "version": <string> } | null
```

`script` is a single Unicode Script property value (e.g. `"Han"`, `"Thai"`, `"Lao"`,
`"Hiragana"`, `"Katakana"`, `"Hangul"`) — "CJK" is invariant 6's own umbrella term
for the whole dictionary-segmentation problem, not one Script value; content mixing
scripts (Japanese text using Han, Hiragana, and Katakana together, for instance)
would need per-script handling this RFC does not design. Schema only — §Non-goals
above states this RFC does not choose a default dictionary for any of them.

### 6. `counts_overlaps_in_length` and per-document length, precisely

**Per-document length**, for a field, is defined as: the count of tokens surviving
the field's full declared analysis chain — tokenization, then every filter in
`deviations`-adjusted tokenization, stopword removal, and stemming, in that declared
order — landing as an indexed posting. This is exactly `dl := Σ_{i∈V} tf_i`,
Robertson & Zaragoza's own definition (`references/robertson-zaragoza-bm25-and-
beyond.md`), computed over the tokens this descriptor's chain actually produces, not
over raw pre-analysis text.

`counts_overlaps_in_length` resolves whether tokens sharing a zero position
increment with the token before them (synonym-expansion-style overlaps — a
mechanism this RFC does not otherwise specify, since STRAND's v0.1 tokenizer profile
above has no synonym-expansion step) count toward that length. **`bm25`** (RFC 0003)
uses whatever this field declares, honestly, for whichever segment it scores.
**`lucene-parity`** (RFC 0003) additionally requires `counts_overlaps_in_length =
false` — matching Lucene's own `discountOverlaps = true` default, which *excludes*
overlaps from the counted length (`references/lucene-bm25similarity-and-
smallfloat.md`); a segment declaring `counts_overlaps_in_length = true` while
claiming `lucene-parity` scoring is non-conforming. This is the resolution RFC 0003's
own Non-goals section deferred to this RFC.

## Worked example

Descriptor:

```json
{
  "unicode_version": "17.0.0",
  "icu_version": "78.3",
  "cldr_version": "48.2",
  "tokenizer_profile": {
    "algorithm": "UAX29-word",
    "unicode_version": "17.0.0",
    "token_retention": "word-only",
    "case_folding": "lower",
    "deviations": []
  },
  "stopword_list_id": "lucene-en-10.5.1",
  "stemmer": {
    "name": "snowball-porter2-en",
    "version": "snowballstem/snowball, confirmed against snowball-data test vectors 2026-08-18"
  },
  "segmentation_dictionary": null,
  "counts_overlaps_in_length": false
}
```

Raw text: `"The whales swim quickly."`

| stage | output |
| ----- | ------ |
| UAX #29 word tokenization, `token_retention: "word-only"` | `["The", "whales", "swim", "quickly"]` — the trailing `.` is a boundary segment with no `Alphabetic`/`Number` character, so it fails `"word-only"`'s retention criterion (§2) and is discarded |
| `case_folding: "lower"` | `["the", "whales", "swim", "quickly"]` |
| Stopword removal, `lucene-en-10.5.1` (`references/lucene-english-stopwords.md`, confirmed to contain `"the"`) | `["whales", "swim", "quickly"]` |
| Stemming, `snowball-porter2-en` (`references/snowball-porter2-english-stemmer.md`: `whales → whale`, `swim → swim`, `quickly → quick`, all confirmed against real test vectors, not predicted) | `["whale", "swim", "quick"]` |

Final indexed token stream: `["whale", "swim", "quick"]`. Per-document length for
this field, this document: **`dl = 3`** (§6's definition — three tokens survived the
full declared chain). This is the first real, checkable analyzer-conformance vector
this project has; it becomes a `conformance/analyzers/` golden file once implemented
(`spec/analyzer-descriptors.md` §7, "Conformance status"), the same status RFC
0003's worked example holds for scoring.

## Napkin math (`CLAUDE.md` §7)

Same conclusion as RFC 0003 §Napkin math, for the same reason: an analyzer
descriptor is segment-open metadata, not a per-posting cold-path structure. The
binding constraint on whichever RFC places its bytes is identical — zero added round
trips beyond invariant 3's ≤2-RTT budget. A descriptor's realistic size (a handful of
short strings and one small nested object) is well under any budget this format has
needed a calculation to justify.

## Invariant-11 checklist

- **Endianness:** not applicable — JSON, not a binary wire structure.
- **Term sort order:** not applicable at this layer.
- **Chunk codec / checksums / codec-variant / stochastic-transform provenance:** not
  applicable, identical reasoning to RFC 0003's own checklist — whichever blob
  eventually carries these bytes inherits its own registry entry's checksum scope;
  this RFC introduces no new codec or stochastic transform.
- **Golden files:** the worked example above is the first `conformance/analyzers/`
  vector — raw text in (`"The whales swim quickly."`), the descriptor JSON, and the
  exact token stream out (`["whale", "swim", "quick"]`), per invariant 6's own
  requirement that these vectors be normative.

## How this could be wrong

**Nearest grave (`docs/lineage.md`): CIFF, "no analyzer metadata."** `docs/lineage.md`
names this as one of CIFF's explicit, named gaps — "conversion required, no
positions, no pruning bounds, no analyzer metadata, lossy doc lengths. Every gap is a
MUST here." This RFC is the direct fix for that specific gap, not a generic
improvement: CIFF has no mechanism at all for a consumer to know how the producer
tokenized, so cross-engine reuse silently corrupts results exactly the way Hiemstra
et al.'s own measurement shows (Motivation, above). The risk this RFC could still fall
into the same grave is real: a descriptor schema loose enough to be satisfied
trivially (e.g., a `tokenizer_profile` with no way to express what actually changed)
would be "analyzer metadata" in name only, reproducing CIFF's gap under a different
field name. The `deviations` list and the requirement that an empty list be a
positive claim, not silence, is this RFC's answer to that risk — but it is only as
good as the discipline of whoever writes future descriptors, which no schema alone
enforces.

**An implicit, undeclared case-folding step — caught by this RFC's own worked
example, fixed in place.** An earlier draft of §2's `tokenizer_profile` had no
`case_folding` field, yet the worked example applied lowercasing anyway. That is
exactly the kind of undeclared-analysis non-portability invariant 6 exists to
prevent: two descriptors with identical JSON could have disagreed on case handling.
Writing the worked example — forcing every step of a concrete chain onto the page —
is what surfaced the gap; `case_folding` (§2) is the fix, applied to this version of
the RFC rather than carried forward as a known hole for a later revision to close.
This is worth naming as a process point, not just a content fix: a worked example
that never gets written is a worked example that never catches this kind of gap.

**Grounding the stemmer's "version" in a fetch-date rather than a commit hash.** The
worked example's `stemmer.version` field cites "confirmed against snowball-data test
vectors 2026-08-18" rather than a specific git commit of the algorithm's own `.sbl`
definition. This is honest about what was actually checked (the test vectors, at a
known date) but is weaker byte-determinism grounding than this project's own
invariant-11 discipline asks for elsewhere (e.g. RFC 0001 pins its own dependencies
to commit hashes and release tags precisely). A future session implementing this
descriptor for real should pin an actual commit hash of `snowballstem/snowball`'s
English `.sbl` source, not repeat this RFC's own date-based shortcut.

## Alternatives considered

**A single free-text `analyzer_description` string** instead of a structured schema.
Rejected: invariant 6 requires the *fields* invariant 6 itself lists, not a
human-readable summary a machine cannot check — the whole point of normative
conformance vectors is that a descriptor's claims are checkable, and a free-text
field is not.

**Registering a fixed enum of stopword lists and stemmers** instead of opaque,
versioned identifiers. Rejected: STRAND does not want to be in the business of
registering every language's every stopword list as a spec amendment; an opaque,
sufficiently-specific identifier (§3, §4) achieves the same determinism without
coupling the format's own versioning to every analyzer component's release cadence —
the same reasoning invariant 8 already applies to codecs ("don't invent encodings,"
here: don't invent a closed registry where an open, checkable identifier suffices).

## Open questions / follow-on RFCs

- ~~Which CJK/Thai/Lao segmentation dictionary STRAND adopts as a default~~ —
  resolved below (Discussion — post-approval amendments). Populating
  `segmentation_dictionary` in the reference implementation (`crates/
  strand-lexical/src/analyzer.rs`) and adding a real dictionary-segmented
  conformance vector to `conformance/analyzers/` remain genuine, separate M1
  execution work this amendment does not do.
- The stemmer version-pinning mechanism (How this could be wrong, above) should be
  tightened to a commit hash before implementation, not left at this RFC's own
  date-based placeholder.
- The descriptor's exact placement inside a segment is the R2 RFC's job, identical
  in shape to RFC 0003's own deferred placement question — the two descriptors
  (scoring-profile, analyzer) may end up sharing one carrying mechanism once R2
  designs it; this RFC does not assume they will.
- A real segmentation-accuracy bake-off (the ICU4X dictionary path vs. Lindera+
  IPADIC for Japanese, vs. Jieba for Chinese, vs. PyThaiNLP for Thai), the same
  discipline R2 applied to postings codecs, has not been run — the recommendation
  below is grounded in license and dependency-shape terms, not measured accuracy
  (Discussion, below).
- The exact byte-level pinning mechanism for `segmentation_dictionary.version` (a
  crate semver string, a compiled-data content hash, or both) is not decided here
  — Discussion, below, states the requirement and leaves the mechanism to the
  implementation session, the same way this RFC's own "How this could be wrong"
  already left the stemmer's commit-hash pinning to a future session.

## Discussion — post-approval amendments

**2026-08-19 — CJK/Thai/Lao default `segmentation_dictionary` resolved.** Prompted
by `docs/roadmap.md` M1-1, sourced directly from this RFC's own Non-goals and Open
questions sections: the schema slot for `segmentation_dictionary` (Design §5) was
pinned at Approval, but which dictionary STRAND recommends as its default was left
unresolved, "gates real conformance for those scripts, not an edge case."

Five live candidates were fetched and license-audited in this session, none taken
from memory (`CLAUDE.md` §3): MeCab, the classic C++ Japanese tokenizer; Lindera, a
pure-Rust MeCab-shaped reimplementation; Jieba/`jieba-rs`, Chinese-specific;
ICU4C's classic dictionary-based break iterator, reachable from Rust via the
`rust_icu` binding; and ICU4X's `icu_segmenter` crate. Thai-specific PyThaiNLP was
also checked and set aside for lack of a Rust path (`references/
pythainlp-license-and-rust-gap.md`).

| Candidate | Code license | Rust path | Dictionary license | Script coverage |
| --- | --- | --- | --- | --- |
| MeCab | GPL/LGPL/BSD, recipient's choice (`references/mecab-triple-license.md`) | none native; C++ FFI only | IPADIC: NAIST + ICOT Free Software, permissive | Japanese only |
| Lindera | MIT (`references/lindera-rust-morphological-analyzer.md`) | native, pure Rust, actively maintained (v5.3.0, published 2026-08-16) | IPADIC (permissive) for Japanese; **CC-CEDICT for Chinese is CC BY-SA 4.0**, share-alike (`references/cc-cedict-and-lindera-cc-cedict-license.md`) | Japanese clean; Chinese license-blocked; no Thai/Lao |
| Jieba / `jieba-rs` | MIT (`references/jieba-and-jieba-rs-license.md`) | native, pure Rust, actively maintained | bundled `dict.txt`, MIT | Chinese only |
| ICU4C classic break iterator (`rust_icu`) | Apache-2.0 binding; ICU4C data Unicode-3.0 + BSD-style notices (`references/rust-icu-icu4c-binding-crate.md`) | FFI to a native, system-linked C library | `cjdict.txt` (Libtabe BSD + IPADIC/ICOT) for Han/Kana, `laodict.txt` (BSD-2-clause) for Lao, Thai covered by the primary Unicode-3.0 grant directly | Chinese, Japanese, Thai, Lao, Khmer, Burmese — automatic dictionary lookup, no LSTM fork |
| ICU4X `icu_segmenter` | Unicode-3.0 (`references/icu4x-icu-segmenter-crate.md`) | native, pure Rust, no C dependency, actively maintained (v2.3.0, published 2026-08-13) | same upstream word lists as ICU4C (`references/icu-license-word-break-dictionaries.md`), Unicode-3.0 | Chinese, Japanese, Thai, Lao, Khmer, Myanmar via explicit `try_new_dictionary()`; `try_new_auto()` instead prefers LSTM for Thai/Lao/Khmer/Myanmar |

**Recommendation: ICU4X's `icu_segmenter` crate, called via `WordSegmenter::
try_new_dictionary()`, is STRAND's default `segmentation_dictionary` family for
CJK, Thai, and Lao.** Three reasons, in order of weight:

1. **It is the only candidate that is simultaneously license-clean, covers all
   three required script families from one dependency, and needs no native C
   binding.** Lindera is the closest runner-up but its Chinese dictionary
   (CC-CEDICT) carries a share-alike condition this project's Apache-2.0 policy
   cannot absorb (`references/cc-cedict-and-lindera-cc-cedict-license.md`), and it
   has no Thai or Lao coverage at all. Jieba is Chinese-only. MeCab's own triple
   license includes a usable BSD option, but only for Japanese, and only via a
   native C++ dependency Lindera already makes unnecessary. ICU4C via `rust_icu`
   covers the same scripts as ICU4X with the same underlying dictionary licenses,
   but requires linking a system ICU4C build — a real byte-determinism
   (invariant 11) and dependency-shape (`CLAUDE.md` §8) cost the pure-Rust ICU4X
   path avoids.
2. **Its dictionary data is license-clean straight through**, confirmed by
   directly fetching ICU's own third-party LICENSE notices rather than trusting a
   summary: `cjdict.txt` combines Libtabe (BSD-style) and IPADIC (NAIST/ICOT,
   permissive, no share-alike); `laodict.txt` is BSD-2-clause; Thai's dictionary
   data carries no separate third-party notice at all and is covered by ICU's own
   primary Unicode License V3 grant (`references/
   icu-license-word-break-dictionaries.md`). None of the three carries a copyleft
   or share-alike condition, unlike CC-CEDICT.
3. **A version-pinning identifier is available without inventing one.** The
   descriptor's own `identity`/`version` fields (Design §5) can name the `icu`
   crate's semver (e.g. `"icu_segmenter 2.3.0"`, the version confirmed live on
   crates.io at time of writing) the same way `stopword_list_id` and
   `stemmer.version` already use an opaque, sufficiently-specific string rather
   than a closed registry (Alternatives considered, above) — no new versioning
   mechanism is invented for this field either.

**Unicode-3.0 is determined Apache-2.0-compatible here, for the first time in this
project.** Every other license this project has accepted so far is MIT (tantivy,
FAISS, `cwida/FastLanes`, Lindera, Jieba) or Apache-2.0 itself. Unicode License V3
is a different family: permissive, non-copyleft, OSI-approved, with copyright- and
permission-notice preservation as its only condition — the same shape of grant MIT
makes, just from a different steward. This determination is made on that basis, not
on precedent alone, and is recorded here so a future session does not have to
re-derive it.

**How this could be wrong.**

*Accuracy is unverified, and the recommendation is not accuracy-driven.* This
choice was made on license and dependency-shape grounds, not a measured
segmentation-quality bake-off — no comparison was run between ICU4X's dictionary
output and Lindera+IPADIC for Japanese, or Jieba for Chinese, or PyThaiNLP for
Thai, all of which have reputations (unverified in this pass) for higher accuracy
than ICU's general-purpose word lists on their respective scripts. `CLAUDE.md` §7's
own napkin-math discipline exists to keep sizing claims honest; the same honesty
applies here: this is a defensible default, not a benchmarked winner, and a real
bake-off — the same discipline R2 applied to postings codecs
(`docs/ledger.md` R2) — is named as still-open work above rather than skipped
silently.

*The `try_new_dictionary()` vs. `try_new_auto()` selection trap is real and
structurally familiar.* ICU4X's own default-shaped constructor, `new_auto()`,
silently substitutes an LSTM model for Thai, Lao, Khmer, and Myanmar rather than
using the dictionary path (`references/icu4x-icu-segmenter-crate.md`). A future
implementation session that reaches for the naturally-named "automatic" constructor
would produce output that does not match what a `segmentation_dictionary` field
claims, without any error — an undeclared-processing-step failure mode this RFC's
own Design section already caught once, in the same document, for case folding
("An earlier draft... applied lowercasing anyway," How this could be wrong, above).
This is also this RFC's nearest grave, restated for this specific decision rather
than a new one invented for it: `docs/lineage.md` names CIFF's "no analyzer
metadata" as the gap this whole RFC exists to close, and the risk named there
("a descriptor schema loose enough to be satisfied trivially... would be 'analyzer
metadata' in name only") is exactly what silently picking `new_auto()` over
`new_dictionary()` would produce — a descriptor that looks precise but describes a
different algorithm than the one that actually ran. The mitigation is the same as
the original risk's: name the exact constructor normatively in the implementation
task this amendment defers, not just the crate.

*The descriptor's existing `icu_version`/`cldr_version` fields do not automatically
cover this dependency's version.* The worked example's `icu_version: "78.3"` names
a classic ICU4C release. ICU4X versions independently (`icu` crate `2.3.0` at time
of writing) and this session did not find a stated CLDR-version correspondence for
that release on its own documentation page. Adopting ICU4X does not let
`segmentation_dictionary.version` piggyback on the descriptor's existing
`icu_version` field for free, the way this amendment's Design-coherence framing
might suggest at a glance — they are two different projects' version numbers that
happen to share a name. `segmentation_dictionary.version` MUST therefore carry its
own value (the `icu`/`icu_segmenter` crate semver, per the recommendation above),
independent of whatever `icu_version` the rest of the descriptor names; a future
implementation session should also decide whether to additionally pin a content
hash of the compiled dictionary data, matching invariant 11's general preference
for checksums over a bare version string alone (Open questions, above) — this RFC
amendment states the requirement, not the mechanism.

Sections updated: Non-goals (states the default is resolved, implementation is
not), Design §5 (unchanged — the schema slot already accommodates any identity
string), Open questions (struck the resolved item, added the bake-off and
byte-pinning follow-ons this amendment's own adversarial pass surfaced), and this
Discussion section. `spec/analyzer-descriptors.md` §5 is updated in the same
session to name the default. `docs/ledger.md` and `docs/roadmap.md` M1-1 are
updated to reflect a resolved format-design decision with implementation still
open, not a fully closed item. No wire format changes: `segmentation_dictionary`'s
shape (`{script, identity, version}`) is unchanged from Approval.

**2026-08-19 — M1-6 implemented; one constructor name corrected against the real
crate.** `crates/strand-lexical/src/analyzer.rs` now populates
`segmentation_dictionary` for Han-script content, wired to `icu_segmenter` 2.3.0
(added to `crates/strand-lexical/Cargo.toml` with `default-features = false,
features = ["compiled_data"]` — deliberately excluding the crate's own `auto`/
`lstm` features so `new_auto`/`new_lstm` are not even reachable from this crate, a
compile-time guardrail on top of the code-review one this amendment already named).
One real dictionary-segmented conformance vector was added,
`conformance/analyzers/icu4x-dictionary-zh-01.json`, built from real ICU4X output
on a real Simplified Chinese sentence, not predicted (`crates/strand-lexical/src/
analyzer.rs`'s test module records the same real output as a unit test).

Fetching the real `icu_segmenter` 2.3.0 source directly (`CLAUDE.md` §3, the same
discipline this RFC's own Motivation section describes) found this amendment's own
paragraph above named the wrong constructor: the infallible, `compiled_data`-only
dictionary constructor is `WordSegmenter::new_dictionary()`, not
`try_new_dictionary()`. `try_new_dictionary()` exists in the real API but is the
fallible variant `gen_buffer_data_constructors!` generates for callers supplying a
custom `BufferProvider` — not the zero-argument, compiled-data path this
amendment's own recommendation assumed. This is a small, textbook instance of the
exact failure mode §3 warns against (implementing a detail from a plausible-sounding
remembered name rather than the fetched source) and is corrected here rather than
silently fixed in code with the RFC left wrong: the load-bearing distinction this
amendment cared about — dictionary lookup, not LSTM — is unaffected, since
`new_dictionary()` is exactly as infallible-with-compiled-data and
dictionary-lookup-based as the mistakenly-named `try_new_dictionary()` was assumed
to be. `spec/analyzer-descriptors.md` §5 is updated with the corrected name.

`segmentation_dictionary.version`'s byte-level pinning mechanism (left open above)
is resolved as: the `icu_segmenter` crate's semver paired with the
`icu_segmenter_data` crate's semver (e.g. `"icu_segmenter 2.3.0 (icu_segmenter_data
2.3.0)"`), not a content hash of the compiled dictionary data. `Cargo.lock` already
pins both crate versions deterministically for any given build; hashing the
dictionary data itself would require new build-time tooling against
`icu_segmenter_data`'s generated Rust source (it `include!()`s `.rs.data` files
rather than shipping one hashable binary blob), which this task judged unnecessary
for a first implementation. `spec/analyzer-descriptors.md` §5 states this choice
and leaves a content-hash upgrade as still-open follow-on work, the same way this
RFC's own "How this could be wrong" section already left the stemmer's
commit-hash pinning open for a later session.

Not done here, and not claimed: the Thai/Lao/Hiragana/Katakana scripts this
default also covers remain unvectored (one Han-script vector only), and M1-7's
segmentation-accuracy bake-off is unstarted — both already named as open in this
amendment's own Open questions and "How this could be wrong," unchanged by this
implementation pass. `docs/roadmap.md` M1-6 is updated to closed; M1-7 remains
open.
