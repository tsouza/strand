# LICO — SIMD-Aware Learned Inverted Index Compression (appendix only; main paper inaccessible)

Vendored 2026-08-18. Found while chasing R9's open question of whether a single codec
could combine BP128-class decode speed with Elias-Fano-class compressed-domain
searchability (`docs/research/r2-hybrid-codec-methodology.md` Phase 0). This entry
records exactly what was and was not verified — the qualitative/structural finding
is real and grounded; the quantitative one is explicitly not.

**Citation.** Xianyu Zhu, Qiyu Liu, Guangyi Zhang, Zhibing Sha, Jianwei Liao, Sha Hu,
Lei Chen, "LICO: An SIMD-Aware High-Performance Learned Inverted Index Compression
Framework," *Proc. ACM Manag. Data* 4(3), article 202, 27 pages, May 2026 (SIGMOD/
PACMMOD 2026). DOI `10.1145/3802079`.

**Access status — genuinely contradictory metadata, not resolved here.** Unpaywall
(`api.unpaywall.org/v2/10.1145/3802079`) reports `is_oa: true`, `license: cc-by`,
`oa_status: hybrid`. DBLP's own record for the same DOI
(`dblp.org/rec/journals/pacmmod/ZhuLZSLHC26`) reports `"access": "closed"`. Neither
was taken on faith: the ACM DL page and PDF (`dl.acm.org/doi/10.1145/3802079`,
`dl.acm.org/doi/pdf/10.1145/3802079`) both returned an HTTP 403 with a Cloudflare
bot-challenge (`cf-mitigated: challenge`) to every automated fetch attempted —
WebFetch and direct `curl` alike, with a normal browser User-Agent. No open-access
repository copy was found: no arXiv preprint exists (checked via the arXiv API,
zero results for `ti:"LICO" AND ti:"inverted"`), no author-page mirror (the
co-author's own publication list page links back to the same blocked ACM DL PDF
URL), and no ResearchGate upload was found via search. This reference does not
attempt to bypass the Cloudflare challenge — that crosses from "accessing licensed
open content" into "circumventing a publisher's bot defenses," a line this project
does not cross even for content that may be legitimately open. The main paper's
actual Experiments section — the section with the numbers that matter — was never
read.

**What was independently obtained and read: the appendix, legitimately, via the
authors' own GitHub repository.** `github.com/xianyuzhuruc/LICO` links its own
"Technical Report" PDF directly (`LICO_An SIMD-Aware High-Performance Learned
Inverted Index Compression Framework (Technical Report).pdf`), fetched via a plain
`curl` from `raw.githubusercontent.com` — no bot-challenge, no ambiguity about
access. That PDF is titled "(Appendix)" and contains only Appendix A–F: notation
table, two theorem proofs (space-cost optimality of the error-bound `ε` choice, and
an `O(1)`-approximation bound for a greedy partitioning heuristic), the DP-based
partitioning pseudocode, a generalization experiment on a 2.6B-integer DNA dataset
(gap mean 12.19, variance 1.78×10⁷), and worked diagrams of the SIMD-based list
intersection and union algorithms. Its content is summarized above rather than
checked into this repo as a binary — no other `references/` entry vendors a raw
PDF, all instead transcribe quotations into a citation `.md` file (this one), kept
consistent here too. The source remains directly fetchable:
`raw.githubusercontent.com/xianyuzhuruc/LICO/main/LICO_An%20SIMD-Aware%20High-Performance%20Learned%20Inverted%20Index%20Compression%20Framework%20(Technical%20Report).pdf`.
This is not the paper's own Experiments section, which lives in the inaccessible
main body.

**The repository's source code was also read directly (`include/lico_enumerate.hpp`,
fetched via the same `raw.githubusercontent.com` route).** This confirms a real,
structural property, independent of any number in the paper: `nextgeq()`
(`lico_enumerate.hpp:372`) is a genuine compressed-domain skip operation — it
predicts a candidate value from the learned piecewise-linear model's segment
parameters and a per-position residual/correction array, using binary search
*within* the model's own encoded representation, never decoding a full block of
plain integers first. The same file also defines `nextgeq_naive()`
(`lico_enumerate.hpp:543`), an explicit decode-then-scan baseline living in the
same codebase — exactly the apples-to-apples comparison Phase 0's operational bar
calls for, if it could be run. `README.md` documents `simd_decode_512i` (AVX-512)
and a `normal` (scalar) decode path, and SIMD-based list intersection/union
reusing `vp2intersect` and `SimSIMD`. **This machine cannot run any of it**:
`CMakeLists.txt` hardcodes `-mavx512f -mavx512vl -mavx512cd -mavx512dq` on every
executable target (`lico_build`, `lico_decode`, `lico_query`), not gated behind a
build option, and this session's own hardware (Intel Core i7-10510U, confirmed via
`/proc/cpuinfo`) has no AVX-512 flags at all — so even the nominally-scalar
`nextgeq()`/`nextgeq_naive()` pair could not be compiled into a runnable binary
without first patching the build system, and the repository carries **no detected
license** (`license: None` via the GitHub API) — the standard implied license to
read and study public code does not extend to confidently redistributing a locally
modified build, so this was not attempted.

**Net finding.** LICO is a real, very recent (May 2026), peer-reviewed construction
that is architecturally the right shape to answer R9's Phase 0 question — a learned
PLA-based codec with SIMD decode *and* a genuine compressed-domain `NextGeq`,
plus SIMD list intersection/union, evaluated against PEF/OptPFor/Simple16/QMX/DINT
baselines per the appendix's own generalization figure. Whether it actually clears
Phase 0's operational bar — decode throughput within 15% of BP128-class speed and
compressed-domain search at least 25% faster than decode-then-search, the same
yardstick `docs/research/r2-hybrid-codec-methodology.md` Phase 3 later applies —
is **not established here**. This is not a "candidate found but fails" verdict
(Phase 0's second bucket) — no number was ever checked — and it is not "nothing
found" either. It is a third, genuine outcome the plan's own verdict-mapping did
not anticipate: a real candidate, structurally on-point, empirically unverified
because of an access barrier, not a design or measurement failure.
