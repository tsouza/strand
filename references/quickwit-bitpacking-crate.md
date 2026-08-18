# The `bitpacking` Crate (quickwit-oss) — A Ready SIMD-BP128 Implementation on Stable Rust

Vendored excerpt, not the full README. Source: `quickwit-oss/bitpacking`,
`README.md`, fetched 2026-08-18 from
`https://raw.githubusercontent.com/quickwit-oss/bitpacking/master/README.md`.
License: MIT (per the crate's published metadata; not independently
re-verified byte-level here). Cited as a concrete, already-battle-tested
candidate for this project's R2 postings-codec bake-off
(`docs/ledger.md`): it is a Rust port of Daniel Lemire's `simdcomp` C
library, used by tantivy for its own postings compression, runs on stable
Rust, and does genuine runtime CPU-feature dispatch — satisfying
invariant 9's "SIMD paths... with runtime multiversion dispatch"
requirement as shipped, not as something this project would need to build
from scratch.

---

### What it is

> "This crate is a Rust port of Daniel Lemire's simdcomp C library."
>
> "It makes it possible to compress/decompress: sequence of small
> integers, sequences of increasing integers."
>
> ":star: It is fast. Expect > 4 billions integers per seconds."

### Stable Rust, runtime dispatch, safe by construction

> "`bitpacking` compiles on stable rust but require rust > 1.27 to
> compile."
>
> "For some bitpacking flavor and for some platform, the bitpacking crate
> may benefit from some specific simd instruction set. In this case, it
> will always ship an alternative scalar implementation and will fall
> back to the scalar implementation at runtime. In other words, your do
> not need to configure anything. Your program will run correctly, and at
> the fastest speed available for your CPU."

### Three incompatible formats, one per SIMD tier

> "`BitPacker1x`, `BitPacker4x`, and `BitPacker8x` produce different
> formats, and are incompatible one with another. `BitPacker4x` and
> `BitPacker8x` are designed specifically to leverage `SSE3` and `AVX2`
> instructions respectively. It will safely fall back at runtime to a
> scalar implementation of these format if these instruction sets are not
> available on the running CPU."
>
> "`BitPacker4x` bits ordering works in layers of 4 integers... One block
> must contain `128 integers`" — literally BP128. `BitPacker8x` uses
> 256-integer blocks; `BitPacker1x` is the pure-scalar 32-integer-block
> reference.

### Benchmarks (crate's own README, one specific machine)

> "The following benchmarks have been run on one thread on my laptop's
> CPU: Intel(R) Core(TM) i5-8250U CPU @ 1.60GHz."

| scheme | operation | throughput |
| --- | --- | --- |
| BitPacker1x (scalar) | decompress | 1.8 billion int/s |
| BitPacker4x (SSE3) | decompress | 5.5 billion int/s |
| BitPacker8x (AVX2) | decompress | 6.5 billion int/s |

The README cites its own reference: "SIMD Compression and the
Intersection of Sorted Integers" (arxiv.org/abs/1401.6399) — a distinct,
later Lemire/Boytsov/Kurz paper about intersection of *already-decoded*
integers, not about this crate's own bit-unpacking (see
`references/lemire-boytsov-simd-bp128.md` for the packing/layout paper
this crate actually implements).

### Provenance note on independence of the numbers above

Tantivy's author, Paul Masurel, separately published a from-scratch
Rust benchmark of the same technique ("Of bitpacking with or without
SSE3," fulmicoton.com/posts/bitpacking/, fetched 2026-08-18) on the exact
same CPU model as this README's own numbers: "running on my laptop which
is powered by Intel(R) Core(TM) i5-8250U CPU @ 1.60GHz... Implementation
Unpack throughput: scalar 1.48 billions integers/s, fake SIMD using u64
2.71 billions integers/s, sse3 6 billions integers/s" (bit width 15).
Masurel is very likely this crate's original author (the repository was
previously hosted under `tantivy-search/bitpacking`, tantivy being his
project) — **these two write-ups are not independent measurements**, they
are the same author's work on the same hardware at two points in time,
and should not be cited as two separately-corroborating benchmarks. The
figures are useful as a real, in-Rust, plausible order of magnitude, not
as independent confirmation.

---

## aarch64 NEON path (source-level, fetched 2026-08-18)

The convergence audit found `docs/ledger.md` claiming an "aarch64 NEON
path" for this crate with no support in this file. Confirmed directly
against the crate's own source, `src/bitpacker4x.rs`, fetched 2026-08-18
from `raw.githubusercontent.com/quickwit-oss/bitpacking/master/src/bitpacker4x.rs`:

A native NEON module exists, gated on aarch64 (line 81):

    #[cfg(all(target_arch = "aarch64", target_endian = "little"))]
    mod neon {
        ...
        use std::arch::aarch64::{ ... };
        ...
        declare_bitpacker!(target_feature(enable = "neon"));

        impl Available for UnsafeBitPackerImpl {
            fn available() -> bool {
                std::arch::is_aarch64_feature_detected!("neon")
            }
        }
    }

and `BitPacker4x::new()` dispatches to it at runtime alongside the x86
SSE3 path, with the scalar fallback last (lines 376–386):

    #[cfg(all(target_arch = "aarch64", target_endian = "little"))]
    {
        if neon::UnsafeBitPackerImpl::available() {
            return BitPacker4x(InstructionSet::NEON);
        }
    }
    BitPacker4x(InstructionSet::Scalar)

So the crate's runtime dispatch covers x86 SSE3, aarch64 NEON, and
scalar. No ARM benchmark numbers are published in the README; the NEON
claim is source-confirmed for existence and dispatch only, not for
throughput.
