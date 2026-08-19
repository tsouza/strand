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

//! Tests for `crate::estimate`'s error bound. Two different kinds of
//! property, deliberately tested two different ways:
//!
//! - `lower_bound <= estimate <= upper_bound` is a *structural* guarantee
//!   (it follows directly from `f_error >= 0`, itself already guaranteed
//!   by `quantize.rs`) — a real property test is the right tool.
//! - "the true distance falls within `[lower_bound, upper_bound]`" is
//!   RaBitQ's own *probabilistic* guarantee, tuned by `K_CONST_EPSILON` for
//!   "nearly perfect confidence," not literal 100% (`docs/docs/rabitq/
//!   estimator.md`'s own wording) — asserting it on every one of a
//!   `proptest` run's many random trials would produce a real, expected
//!   occasional failure that is not an implementation bug, so this is
//!   checked statistically instead: a large, fixed-seed sample, asserting
//!   the empirical containment rate clears a stated threshold rather than
//!   demanding 100%.

use proptest::prelude::*;
use strand_vector::estimate::{QueryFactors, estimate_distance};
use strand_vector::quantize::{MetricType, quantize_one_bit};

fn finite_component() -> impl Strategy<Value = f32> {
    prop::num::f32::NORMAL.prop_filter("bounded magnitude", |v| v.abs() < 100.0)
}

proptest! {
    /// The error bound is structurally sound for any finite, non-degenerate
    /// input: the estimate always falls inside its own bound interval.
    #[test]
    fn estimate_always_falls_within_its_own_bounds(
        data in prop::collection::vec(finite_component(), 64),
        centroid in prop::collection::vec(finite_component(), 64),
        query in prop::collection::vec(finite_component(), 64),
        use_ip in any::<bool>(),
    ) {
        prop_assume!(data != centroid);
        let metric = if use_ip { MetricType::InnerProduct } else { MetricType::L2 };

        let quantized = quantize_one_bit(&data, &centroid, metric);
        let qf = QueryFactors::new(&query, 1);
        let est = estimate_distance(&quantized, &query, &centroid, &qf, metric);

        prop_assert!(est.estimate.is_finite());
        prop_assert!(est.lower_bound.is_finite());
        prop_assert!(est.upper_bound.is_finite());
        prop_assert!(est.lower_bound <= est.estimate + 1e-3, "lb={} est={}", est.lower_bound, est.estimate);
        prop_assert!(est.estimate <= est.upper_bound + 1e-3, "est={} ub={}", est.estimate, est.upper_bound);
    }
}

/// A deterministic PRNG (no extra dependency) for a large, reproducible
/// statistical sample.
fn next_f32(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    ((*state % 2001) as f32 - 1000.0) / 200.0 // roughly [-5.0, 5.0]
}

/// RaBitQ's own stated design intent: `K_CONST_EPSILON` gives "nearly
/// perfect confidence," not a hard 100% guarantee. Over a large, fixed
/// sample of random (data, centroid, query) triples, the true distance
/// should fall within `[lb, ub]` for the overwhelming majority of cases —
/// checked statistically, with a threshold well below 100% so this test
/// doesn't become flaky on the algorithm's own expected rare misses.
#[test]
fn true_distance_falls_within_bounds_for_the_large_majority_of_random_cases() {
    let dim = 64;
    let trials = 2000;
    let mut state = 0xE5_71_AA_u64;
    let mut contained_l2 = 0;
    let mut contained_ip = 0;

    for _ in 0..trials {
        let data: Vec<f32> = (0..dim).map(|_| next_f32(&mut state)).collect();
        let centroid: Vec<f32> = (0..dim).map(|_| next_f32(&mut state)).collect();
        let query: Vec<f32> = (0..dim).map(|_| next_f32(&mut state)).collect();
        if data == centroid {
            continue;
        }

        let qf = QueryFactors::new(&query, 1);

        let q_l2 = quantize_one_bit(&data, &centroid, MetricType::L2);
        let est_l2 = estimate_distance(&q_l2, &query, &centroid, &qf, MetricType::L2);
        let true_l2: f32 = data
            .iter()
            .zip(&query)
            .map(|(&d, &qv)| (d - qv) * (d - qv))
            .sum();
        if est_l2.lower_bound <= true_l2 && true_l2 <= est_l2.upper_bound {
            contained_l2 += 1;
        }

        let q_ip = quantize_one_bit(&data, &centroid, MetricType::InnerProduct);
        let est_ip = estimate_distance(&q_ip, &query, &centroid, &qf, MetricType::InnerProduct);
        let true_neg_ip: f32 = -data.iter().zip(&query).map(|(&d, &qv)| d * qv).sum::<f32>();
        if est_ip.lower_bound <= true_neg_ip && true_neg_ip <= est_ip.upper_bound {
            contained_ip += 1;
        }
    }

    let rate_l2 = f64::from(contained_l2) / f64::from(trials);
    let rate_ip = f64::from(contained_ip) / f64::from(trials);
    assert!(
        rate_l2 > 0.90,
        "L2 containment rate too low: {rate_l2} ({contained_l2}/{trials})"
    );
    assert!(
        rate_ip > 0.90,
        "IP containment rate too low: {rate_ip} ({contained_ip}/{trials})"
    );
}
