# LICO — SIMD-Aware Learned Inverted Index Compression: real numbers, real verdict

Vendored 2026-08-18, resolved same day once the user obtained a legitimate copy of
the full paper (ACM DL's own page and PDF both returned an HTTP 403 Cloudflare
bot-challenge to every automated fetch attempted from this session — WebFetch and
direct `curl` alike — despite Unpaywall reporting the paper CC-BY hybrid open
access while DBLP's own record for the same DOI reports it closed; neither claim
was resolved, and no bypass of the Cloudflare challenge was attempted). This entry
records what `docs/research/r2-hybrid-codec-methodology.md` Phase 0 actually found,
against the bar fixed before searching: full-scan decode throughput within 15% of
BP128-class speed **and** compressed-domain search at least 25% faster than
decode-then-search, both required simultaneously.

**Citation.** Xianyu Zhu, Qiyu Liu, Guangyi Zhang, Zhibing Sha, Jianwei Liao, Sha Hu,
Lei Chen, "LICO: An SIMD-Aware High-Performance Learned Inverted Index Compression
Framework," *Proc. ACM Manag. Data* 4(3), article 202, 27 pages, May 2026 (SIGMOD/
PACMMOD 2026). DOI `10.1145/3802079`. Code: `github.com/xianyuzhuruc/LICO`.

## The mechanism is real

`nextgeq()` is a genuine compressed-domain skip operator: LICO's per-list encoding
is already segmented into error-bounded piecewise-linear-model (PLA) pieces, each
with a first-key bound, so a query can binary-search directly over segment
boundaries and, within a segment, over the model's own predicted values plus a
compact residual/correction array — never fully decoding a block of plain integers
first. Table 2 (the paper's own compatibility matrix) shows LICO and LICO++ are
the only two of fourteen compared methods with `√` in all four rows (Decode,
Intersection, Union, SIMD); most non-learned codecs support decode but not native
compressed-domain intersection (e.g. FastPFor, OptPFor, QMX are marked `×` for
Intersection/Union/SIMD). §6.2's own explanation: "traditional compressors usually
employ a NextGeq... operator [that requires] skip pointers to the maximum key of
each block... In contrast, LICO natively supports NextGeq without any additional
metadata... each [segment already contains] sufficient information to perform the
skip operations." This is a real, structurally different mechanism from BP128/
FastPFOR, independently confirmed by reading the actual `nextgeq()` source in
`include/lico_enumerate.hpp` before the full paper was available.

## The two axes, checked against the fixed bar, with real numbers

**Compressed-domain search: clears the 25% bar, by a wide margin.** §7.5, Table 5
(intersection query latency, ms/query, TREC 2005/2006 Efficiency Track topics on
CW12B/CCNews/WITD): "LICO and LICO++ achieve the best intersection performance in
most cases, outperforming the fastest non-learned SIMD-based methods by **up to
2.64×**." The paper's own headline conclusion states LICO "improves query
performance by up to **5.52×** compared with both highly optimized conventional
codecs and recent learned compressors" (§9, aggregate across intersection and
union). Both numbers are cross-codec comparisons (LICO's NextGeq-based pruning vs.
other codecs' own intersection implementations), not literally "LICO's own
NextGeq vs. LICO's own decode-then-scan on the same binary" — Phase 3's later,
stricter gate asks for that same-binary ablation specifically; this paper doesn't
report it. But the cross-codec numbers are strong, real, and consistent with the
structural mechanism above: NextGeq genuinely helps.

**Full-scan decode throughput: fails the 15% bar, by roughly an order of
magnitude.** Table 7 reports LICO's own best configuration
(`LICO_OptimalPLA`, the SIMD-aware AVX-512 pipeline, averaged over CW12B/CCNews/
WITD): **0.61 ns/int**. The paper's own text calls this "the highest decoding
throughput" — true, but only *among the fourteen methods this paper itself
compares* (VByte, OptVByte, BIC, Delta, Rice, PEF, DINT, OptPFor, FastPFor,
Simple16, QMX, LA-vector, LeCo-var, plus LICO/LICO++). **None of those is a pure
SIMD-BP128-style bit-packer** (`BitPacker8x`/`BitPacker4x`) — OptPFor is the
nearest, and it is an exception-based PFOR variant, not patch-free bit-packing
(the same family-precision distinction `references/ottaviano-venturini-partitioned-elias-fano.md`
already flags for OptPFD). Comparing LICO's 0.61 ns/int against this project's own
real, measured `BitPacker8x` throughput (`bench/results/codec-decode-throughput.json`,
this session, ~14.2–19.17B values/sec depending on distribution) gives 0.052–0.070
ns/int for `BitPacker8x` — **LICO is 8.7×–11.7× slower per integer**, computed
directly from real numbers on both sides, not estimated. This comparison crosses
machines (LICO: Xeon Gold 6430 server, AVX-512; this project's own benchmark: Core
i7-10510U mobile, AVX2 only) and that is a real caveat — but the gap is an order
of magnitude, and the more capable server hardware should if anything favor LICO,
not work against it, so a hardware artifact explaining away the whole gap is
implausible. It is also the expected result algorithmically, not a surprising
one: PLA decode evaluates a linear function (multiply, divide-or-shift, add a
correction) per value, where plain bit-packing only shifts and masks — more
per-value arithmetic work is exactly what a heavier, structurally richer decode
path should cost.

## Verdict

LICO does not satisfy Phase 0's bar. It is a real, working, peer-reviewed
technique that gets a genuine compressed-domain search win — but it buys that win
with roughly an order-of-magnitude decode-speed cost against pure bit-packing,
not a free combination of both properties. This is Phase 0's second bucket per
its own verdict-mapping — **candidate found, checked with real numbers, fails** —
not the inconclusive-for-access-reasons status this entry originally recorded
before the full paper became available. It is a real negative data point, and a
structurally unsurprising one: it confirms, with a concrete measured number this
time, the same conclusion this project's ledger already recorded from the
Lemire/Boytsov literature before this investigation started — no established
technique fuses BP128-class decode speed with compressed-domain searchability;
you buy one by spending the other.
