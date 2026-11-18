// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

include!(concat!(env!("OUT_DIR"), "/unions.rs"));
use unions::*;

#[test]
fn bool_union() {
    // True case:
    let before = AnOption { inner: Some(7) };
    assert_eq!(before.get_width(), 8);

    let mut bytes = vec![1; 8];
    assert_eq!(8, before.serialize(&mut bytes));

    let (after, _) = AnOption::deserialize(bytes.as_slice()).unwrap();
    assert_eq!(after.get_width(), 8);
    assert_eq!(before, after);

    // False case:
    let before = AnOption { inner: None };

    assert_eq!(4, before.get_width());
    assert_eq!(4, before.serialize(&mut bytes));

    let (after, _) = AnOption::deserialize(bytes.as_slice()).unwrap();
    assert_eq!(4, after.get_width());
    assert_eq!(before, after);
}

#[test]
fn basic_enum() {
    let before = Cases::two;
    let mut bytes = [1; 4];
    assert_eq!(4, before.serialize(&mut bytes));
    let (after, _) = Cases::deserialize(bytes.as_slice()).unwrap();
    assert_eq!(before, after);
}

#[test]
fn invalid_enum_variant() {
    let bad_bytes = [0, 0, 0, 4];
    assert!(Cases::deserialize(bad_bytes.as_slice()).is_err());
}

#[test]
fn enum_union_no_default() {
    let before = NoDefault::one(99);
    assert_eq!(before.get_width(), 8);
    let mut bytes = [1; 100];
    assert_eq!(8, before.serialize(&mut bytes));
    let (after, _) = NoDefault::deserialize(bytes.as_slice()).unwrap();
    assert_eq!(after.get_width(), 8);
    assert_eq!(before, after);

    let before = NoDefault::two(AnOption {
        inner: Some(i32::MAX),
    });
    assert_eq!(before.get_width(), 12);
    assert_eq!(12, before.serialize(&mut bytes));
    let (after, _) = NoDefault::deserialize(bytes.as_slice()).unwrap();
    assert_eq!(after.get_width(), 12);
    assert_eq!(before, after);

    let before = NoDefault::three;
    assert_eq!(before.get_width(), 4);
    assert_eq!(4, before.serialize(&mut bytes));
    let (after, _) = NoDefault::deserialize(bytes.as_slice()).unwrap();
    assert_eq!(after.get_width(), 4);
    assert_eq!(before, after);
}

#[test]
fn enum_union_default_void() {
    let before = VoidDefault::Default;
    let mut bytes = [1; 4];
    assert_eq!(before.get_width(), 4);
    assert_eq!(4, before.serialize(&mut bytes));
    let (after, _) = VoidDefault::deserialize(bytes.as_slice()).unwrap();
    assert_eq!(after.get_width(), 4);
    assert_eq!(before, after);
}

#[test]
fn enum_union_default_non_void() {
    let before = IntDefault::Default(7);
    let mut bytes = [1; 8];
    assert_eq!(before.get_width(), 8);
    assert_eq!(8, before.serialize(&mut bytes));
    let (after, _) = IntDefault::deserialize(bytes.as_slice()).unwrap();
    assert_eq!(after.get_width(), 8);
    assert_eq!(before, after);
}

#[test]
fn invalid_union_variant_no_default() {
    let bad_bytes = [0, 0, 0, 4];
    assert!(NoDefault::deserialize(bad_bytes.as_slice()).is_err());
}

#[test]
fn unexpected_union_variant_with_default() {
    // When a union has a default arm, any discriminant value which is NOT one of the enum variants
    // is treated as the default arm:
    let bad_bytes = [0, 0, 0, 100];
    let (after, _) = VoidDefault::deserialize(bad_bytes.as_slice()).unwrap();
    assert_eq!(VoidDefault::Default, after);
}
