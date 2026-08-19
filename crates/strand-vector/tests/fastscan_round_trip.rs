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

//! Property-based round-trip tests for the FastScan pack/unpack codec
//! (`crate::fastscan`, grounding: `references/rabitq-library-fastscan-
//! pack-codes-source.md`) across many random inputs and vector counts —
//! `CLAUDE.md` §9's "prove" step for any reader.

use proptest::prelude::*;
use strand_vector::fastscan::{pack_codes, unpack_codes};

proptest! {
    #[test]
    fn pack_then_unpack_recovers_the_original_compact_codes(
        cols in 1usize..17,
        num in 1usize..130,
        seed in any::<u64>(),
    ) {
        // A simple deterministic PRNG (no extra dependency) seeded per case.
        let mut state = seed ^ 0x9E3779B97F4A7C15;
        let mut next_byte = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state & 0xFF) as u8
        };
        let compact: Vec<u8> = (0..(num * cols)).map(|_| next_byte()).collect();

        let packed = pack_codes(&compact, num, cols);
        let recovered = unpack_codes(&packed, num, cols);

        prop_assert_eq!(recovered, compact);
    }

    #[test]
    fn packed_length_only_depends_on_num_and_cols_not_content(
        cols in 1usize..17,
        num in 1usize..130,
    ) {
        let zeros = vec![0u8; num * cols];
        let ones = vec![0xFFu8; num * cols];
        prop_assert_eq!(pack_codes(&zeros, num, cols).len(), pack_codes(&ones, num, cols).len());
    }
}
