# Agner Fog's Instruction Tables — PDEP/PEXT Latency Across Microarchitectures

Vendored excerpt (specific table rows only, not the full 43-page
document), not a full reproduction. Source: Agner Fog, "Instruction
tables: Lists of instruction latencies, throughputs and micro-operation
breakdowns for Intel, AMD, and VIA CPUs," fetched 2026-08-18 from
`https://www.agner.org/optimize/instruction_tables.pdf`. Publicly
distributed reference material, reproduced here as a small excerpt of
specific numeric table entries for citation. Cited to ground a real
hardware-heterogeneity trap this project should remember if a future
session ever considers BMI2's `pext`/`pdep` for a bit-manipulation kernel:
confirmed BP128/FastPFOR implementations checked during this project's
hardware-acceleration investigation do **not** use these instructions
(they use plain shift/AND/OR — see
`references/lemire-boytsov-simd-bp128.md`), but the trap is worth
recording precisely in case a future kernel design considers them for a
different codec.

---

### The instruction-table entries (register-form `PDEP`/`PEXT`, i.e. `r,r,r`)

| Microarchitecture | µops | Latency (cycles) | Reciprocal throughput (cycles) |
| --- | --- | --- | --- |
| AMD Zen 1 | 6 (PDEP) / 7 (PEXT) | 18 | 18 |
| AMD Zen 2 | 6 (PDEP) / 7 (PEXT) | 19 | 19 |
| AMD Zen 3 | 1 | 3 | 1 |
| AMD Zen 4 | 1 | 3 | 1 |
| AMD Zen 5 | 1 | 3 | 0.33 |
| Intel Haswell | 1 | 3 | 1 |

Zen 1 and Zen 2 entries read literally, e.g. Zen 2: `PDEP r,r,r  6  19  19  BMI2` and `PEXT r,r,r  7  19  19  BMI2` — a fully serialized, ~19-cycle-per-op microcoded implementation (latency equals reciprocal throughput, meaning back-to-back independent instances don't pipeline at all). Zen 3 reads `PDEP r,r,r  1  3  1  BMI2` — a genuine single-µop, pipelined implementation. Haswell's own table (a differently-laid-out section, with an explicit `p1` port assignment) reads `PDEP r,r,r  1  1  p1  3  1` under that table's column header order (µops fused, µops unfused, port, latency, reciprocal throughput) — latency 3, throughput 1, per-instruction port p1.

### What this actually shows

Contrary to a loose "Intel Haswell+ is single-cycle, AMD Zen1/2 is ~18
cycles" framing, the real numbers are: **Zen 1 and Zen 2 are the outliers**
(18–19 cycles, non-pipelined, clearly microcoded), while **Haswell and
Zen 3 onward all land in the same fast class** (latency 3, throughput 1
— pipelined, one issued per cycle even though each individual result
takes 3 cycles to be ready). Zen 5 goes further, at 0.33 reciprocal
throughput (three per cycle). The practical implication for any future
BMI2-based kernel is unchanged from what a looser "Zen1/2 slow" framing
would already suggest: plain CPUID/feature-flag detection ("is BMI2
present") is not enough, because Zen 1/2 *have* BMI2 and would silently
run 6–19× slower than every other microarchitecture in this table —
correct dispatch needs microarchitecture-generation awareness, not just
a feature bit.
