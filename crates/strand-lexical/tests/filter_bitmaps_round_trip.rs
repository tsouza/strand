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

//! Property-based round-trip tests for the filter-bitmap store, plus a
//! direct mechanical check of `spec/filter-bitmaps.md` §3's no-run-containers
//! MUST: two writers building the identical logical bitmap through different
//! `roaring` insertion APIs must serialize to identical bytes.

use proptest::prelude::*;
use roaring::RoaringBitmap;
use strand_lexical::filter_bitmaps::{
    FilterBitmapError, FilterBitmapStore, MAX_ROW_ID_COUNT, build_filter_bitmap_store,
};

#[test]
fn run_container_promotion_does_not_change_serialized_bytes() {
    // Same logical bitmap, contiguous ordinals 100..=200, built two ways.
    let mut via_individual_inserts = RoaringBitmap::new();
    for ordinal in 100..=200u32 {
        via_individual_inserts.insert(ordinal);
    }

    let mut via_range_insert = RoaringBitmap::new();
    via_range_insert.insert_range(100..=200u32);

    // Confirm the two in-memory bitmaps are logically equal but were built
    // through APIs that can select different internal container types —
    // the exact non-determinism spec/filter-bitmaps.md §3 closes.
    assert_eq!(via_individual_inserts, via_range_insert);

    let store_a = build_filter_bitmap_store(&[via_individual_inserts], 1000).unwrap();
    let store_b = build_filter_bitmap_store(&[via_range_insert], 1000).unwrap();

    assert_eq!(
        store_a, store_b,
        "two conformant writers of the identical logical bitmap must produce identical bytes"
    );
}

#[test]
fn row_id_count_above_the_roaring_cap_is_rejected() {
    let result = build_filter_bitmap_store(&[RoaringBitmap::new()], MAX_ROW_ID_COUNT + 1);
    assert_eq!(
        result,
        Err(FilterBitmapError::RowIdCountExceedsRoaringCapacity)
    );
}

#[test]
fn row_id_count_at_the_roaring_cap_is_accepted() {
    let result = build_filter_bitmap_store(&[RoaringBitmap::new()], MAX_ROW_ID_COUNT);
    assert!(result.is_ok());
}

proptest! {
    #[test]
    fn every_bitmap_resolves_through_its_directory_entry(
        ordinal_sets in prop::collection::vec(
            prop::collection::vec(0u32..1000, 0..30),
            1..10,
        ),
    ) {
        let bitmaps: Vec<RoaringBitmap> = ordinal_sets
            .iter()
            .map(|ords| RoaringBitmap::from_iter(ords.iter().copied()))
            .collect();

        let store_bytes = build_filter_bitmap_store(&bitmaps, 1000).unwrap();
        let store = FilterBitmapStore::new(&store_bytes);

        for (ordinal, expected) in bitmaps.iter().enumerate() {
            let actual = store.bitmap(ordinal as u64).unwrap();
            prop_assert_eq!(&actual, expected);
        }
    }
}
