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

//! Property-based tests for `crate::kmeans` across random inputs —
//! `CLAUDE.md` §9's "prove" step, complementing `kmeans.rs`'s own
//! fixed-blob tests with broad coverage over shape and content no small
//! set of hand-picked cases can give.

use proptest::prelude::*;
use rand::SeedableRng;
use rand::rngs::StdRng;
use strand_vector::kmeans::{inertia, kmeans};

proptest! {
    /// For any well-formed input, `kmeans` returns exactly `n` assignments,
    /// each a valid cluster index, every cluster non-empty, and a finite,
    /// non-negative inertia — structural guarantees that must hold
    /// regardless of the random data's actual geometry.
    #[test]
    fn kmeans_returns_well_formed_output(
        n in 3usize..60,
        dims in 1usize..12,
        k_frac in 0.1f64..0.9,
        seed in any::<u64>(),
        values in prop::collection::vec(prop::num::f32::NORMAL.prop_filter("bounded", |v| v.abs() < 1e3), 3 * 12),
    ) {
        let k = ((n as f64) * k_frac).max(1.0) as usize;
        let k = k.min(n);
        let vectors: Vec<f32> = (0..n * dims).map(|i| values[i % values.len()] + i as f32 * 0.001).collect();

        let mut rng = StdRng::seed_from_u64(seed);
        let result = kmeans(&vectors, n, dims, k, 50, &mut rng);

        prop_assert_eq!(result.assignments.len(), n);
        prop_assert_eq!(result.centroids.len(), k * dims);
        prop_assert!(result.assignments.iter().all(|&a| a < k));

        let mut counts = vec![0usize; k];
        for &a in &result.assignments {
            counts[a] += 1;
        }
        prop_assert!(counts.iter().all(|&c| c > 0), "every cluster must be non-empty: {counts:?}");

        let obj = inertia(&vectors, n, dims, &result);
        prop_assert!(obj.is_finite() && obj >= 0.0, "inertia={obj}");
    }
}
