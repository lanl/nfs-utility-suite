// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

// Tests for the non-allocating serialization APIs.

include!(concat!(env!("OUT_DIR"), "/arrays.rs"));
use arrays::*;

#[test]
fn struct_with_primitive_types() {
    let before = OpaqueArrays {
        a: [7u8],
        b: [1u8, 2u8],
        c: [255u8, 120u8, 0u8],
        d: [1u8, 0u8, 3u8, 4u8],
    };

    let mut bytes = vec![0; 16];

    assert_eq!(16, before.serialize(&mut bytes));

    let mut after = OpaqueArrays::default();

    OpaqueArrays::deserialize(&mut after, &mut bytes.as_slice()).unwrap();

    assert_eq!(before, after);
}
