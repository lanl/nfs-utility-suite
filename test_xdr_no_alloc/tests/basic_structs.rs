// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

// Tests for the non-allocating serialization APIs.

include!(concat!(env!("OUT_DIR"), "/structs.rs"));
use structs::*;

#[test]
fn struct_with_primitive_types() {
    let before = Foo {
        a: 7,
        b: u32::MAX - 7,
        c: 0,
        d: (u32::MAX as u64) + 1,
    };

    let mut bytes = vec![0; 24];

    assert_eq!(24, before.serialize(&mut bytes));

    let mut after = Foo::default();

    Foo::deserialize(&mut after, &mut bytes.as_slice()).unwrap();

    assert_eq!(before, after);
}
