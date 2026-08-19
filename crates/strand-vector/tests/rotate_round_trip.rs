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

//! Property-based tests for `crate::rotate` — `CLAUDE.md` §9's "prove"
//! step. `rotate_fht_kac` is, by construction, an orthogonal transform
//! (Hadamard butterflies, sign flips, and Kac's-walk butterflies are all
//! individually orthogonal, up to the final documented 0.25 rescale) —
//! its defining mathematical property, L2-norm preservation, holds for
//! *any* input, which makes it a property test no fixed set of hand-picked
//! cases can substitute for.

use proptest::prelude::*;
use rand::SeedableRng;
use rand::rngs::StdRng;
use strand_vector::descriptor::{self, DistanceMetric, padded_dims_for};
use strand_vector::rotate::rotate_fht_kac;

fn finite_component() -> impl Strategy<Value = f32> {
    prop::num::f32::NORMAL.prop_filter("bounded magnitude", |v| v.abs() < 1e4)
}

proptest! {
    /// `rotate_fht_kac` preserves L2 norm (within a generous f32 rounding
    /// tolerance that grows with dimensionality), for both the general
    /// branch (dims not a power of two) and the simple branch (dims a
    /// power of two, e.g. 64, 128).
    #[test]
    fn rotate_fht_kac_preserves_l2_norm(
        dims in 1usize..300,
        data in prop::collection::vec(finite_component(), 1..300),
        seed in any::<u64>(),
    ) {
        let dims = dims.min(data.len()).max(1);
        let data = &data[..dims];
        let padded_dims = padded_dims_for(dims as u32) as usize;

        let mut rng = StdRng::seed_from_u64(seed);
        let descriptor_bytes = descriptor::build_fht_kac(dims as u32, DistanceMetric::L2, 1, &mut rng);
        let reader = descriptor::DescriptorReader::new(&descriptor_bytes).unwrap();

        let rotated = rotate_fht_kac(data, padded_dims, reader.rotation_payload());

        let norm_in: f64 = data.iter().map(|&v| (v as f64) * (v as f64)).sum();
        let norm_out: f64 = rotated.iter().map(|&v| (v as f64) * (v as f64)).sum();

        // A generous relative tolerance: f32 rounding accumulates over
        // O(log(padded_dims)) FHT stages plus 4 kacs_walk stages.
        let tolerance = (norm_in.max(norm_out) * 1e-3).max(1e-3);
        prop_assert!(
            (norm_in - norm_out).abs() < tolerance,
            "norm not preserved: in={norm_in}, out={norm_out}, dims={dims}, padded_dims={padded_dims}"
        );
    }
}
