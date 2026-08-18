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

//! Reproduces RFC 0006's ("blue"/"red", 6-document toy segment) worked
//! example and checks it against the pinned conformance golden files.

use roaring::RoaringBitmap;
use strand_lexical::filter_bitmaps::{
    build_filter_bitmap_store, build_value_dictionary, FilterBitmapStore, ValueDictionary,
};

fn toy_bitmaps() -> Vec<RoaringBitmap> {
    vec![
        RoaringBitmap::from_iter([0u32, 3, 4]), // "blue"
        RoaringBitmap::from_iter([1u32, 2, 5]), // "red"
    ]
}

#[test]
fn worked_example_matches_conformance_golden_files() {
    let values: Vec<&[u8]> = vec![b"blue".as_slice(), b"red".as_slice()];
    let fst_bytes = build_value_dictionary(&values).unwrap();
    let store_bytes = build_filter_bitmap_store(&toy_bitmaps(), 6).unwrap();

    let golden_fst = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/filter-bitmaps/toy-values.fst"
    ))
    .expect("golden file conformance/filter-bitmaps/toy-values.fst must exist");
    let golden_store = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/filter-bitmaps/toy-bitmap-store.bin"
    ))
    .expect("golden file conformance/filter-bitmaps/toy-bitmap-store.bin must exist");

    assert_eq!(fst_bytes, golden_fst, "FST bytes must match RFC 0006's pinned 53 bytes");
    assert_eq!(
        store_bytes, golden_store,
        "filter-bitmap store bytes must match RFC 0006's pinned 68 bytes"
    );
}

#[test]
fn worked_example_resolves_query_predicate_per_the_rfc() {
    let values: Vec<&[u8]> = vec![b"blue".as_slice(), b"red".as_slice()];
    let fst_bytes = build_value_dictionary(&values).unwrap();
    let store_bytes = build_filter_bitmap_store(&toy_bitmaps(), 6).unwrap();

    let dict = ValueDictionary::open(fst_bytes).unwrap();
    let store = FilterBitmapStore::new(&store_bytes);

    // field = "red"
    let ordinal = dict.get(b"red").unwrap();
    assert_eq!(ordinal, 1);
    let entry = store.directory_entry(ordinal).unwrap();
    assert_eq!(entry.bitmap_offset, 46);
    assert_eq!(entry.bitmap_length, 22);

    let bitmap = store.bitmap(ordinal).unwrap();
    assert!(bitmap.contains(1));
    assert!(bitmap.contains(2));
    assert!(bitmap.contains(5));
    assert!(!bitmap.contains(0));

    assert_eq!(dict.get(b"green"), None, "a lookup miss is a normal outcome");
}
