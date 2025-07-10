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

#[test]
#[should_panic]
fn buf_too_small() {
    let before = Foo {
        a: 7,
        b: u32::MAX - 7,
        c: 0,
        d: (u32::MAX as u64) + 1,
    };

    let mut bytes = vec![0; 23];

    let _ = before.serialize(&mut bytes);
}

#[test]
fn struct_with_inner_struct() {
    let before = Container {
        first: Foo {
            a: 7,
            b: u32::MAX - 7,
            c: 0,
            d: (u32::MAX as u64) + 1,
        },
        middle: true,
        last: Foo {
            a: 1,
            b: 2,
            c: 3,
            d: 4,
        },
    };

    let mut bytes = vec![0; 52];

    assert_eq!(52, before.serialize(&mut bytes));

    let mut after = Container::default();

    Container::deserialize(&mut after, &mut bytes.as_slice()).unwrap();

    assert_eq!(before, after);
}

#[test]
fn struct_with_typedef() {
    let before = HasTypedef { blah: -12345 };

    let mut bytes = vec![0; 4];

    assert_eq!(4, before.serialize(&mut bytes));

    let mut after = HasTypedef::default();

    HasTypedef::deserialize(&mut after, &mut bytes.as_slice()).unwrap();

    assert_eq!(before, after);
}
