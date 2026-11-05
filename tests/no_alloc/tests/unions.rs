// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

include!(concat!(env!("OUT_DIR"), "/unions.rs"));
use unions::*;

#[test]
fn bool_union() {
    // True case:
    let before = MyOption { inner: Some(7) };
    assert_eq!(before.get_width(), 8);

    let mut bytes = vec![1; 8];
    assert_eq!(8, before.serialize(&mut bytes));

    let (after, _) = MyOption::deserialize(bytes.as_slice()).unwrap();
    assert_eq!(after.get_width(), 8);
    assert_eq!(before, after);

    // False case:
    let before = MyOption { inner: None };

    assert_eq!(4, before.get_width());
    assert_eq!(4, before.serialize(&mut bytes));

    let (after, _) = MyOption::deserialize(bytes.as_slice()).unwrap();
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
    let before = Stuff::one(99);
    assert_eq!(before.get_width(), 8);
    let mut bytes = [1; 100];
    assert_eq!(8, before.serialize(&mut bytes));
    let (after, _) = Stuff::deserialize(bytes.as_slice()).unwrap();
    assert_eq!(after.get_width(), 8);
    assert_eq!(before, after);

    let before = Stuff::two(MyOption {
        inner: Some(i32::MAX),
    });
    assert_eq!(before.get_width(), 12);
    assert_eq!(12, before.serialize(&mut bytes));
    let (after, _) = Stuff::deserialize(bytes.as_slice()).unwrap();
    assert_eq!(after.get_width(), 12);
    assert_eq!(before, after);

    let before = Stuff::three;
    assert_eq!(before.get_width(), 4);
    assert_eq!(4, before.serialize(&mut bytes));
    let (after, _) = Stuff::deserialize(bytes.as_slice()).unwrap();
    assert_eq!(after.get_width(), 4);
    assert_eq!(before, after);
}

#[test]
fn enum_union_default_void() {
    let before = Things::Default;
    let mut bytes = [1; 4];
    assert_eq!(before.get_width(), 4);
    assert_eq!(4, before.serialize(&mut bytes));
    let (after, _) = Things::deserialize(bytes.as_slice()).unwrap();
    assert_eq!(after.get_width(), 4);
    assert_eq!(before, after);
}

#[test]
fn enum_union_default_non_void() {
    let before = MoreThings::Default(7);
    let mut bytes = [1; 8];
    assert_eq!(before.get_width(), 8);
    assert_eq!(8, before.serialize(&mut bytes));
    let (after, _) = MoreThings::deserialize(bytes.as_slice()).unwrap();
    assert_eq!(after.get_width(), 8);
    assert_eq!(before, after);
}

#[test]
fn invalid_union_variant_no_default() {
    let bad_bytes = [0, 0, 0, 4];
    assert!(Stuff::deserialize(bad_bytes.as_slice()).is_err());
}

#[test]
fn unexpected_union_variant_with_default() {
    // When a union has a default arm, any discriminant value which is NOT one of the enum variants
    // is treated as the default arm:
    let bad_bytes = [0, 0, 0, 100];
    let (after, _) = Things::deserialize(bad_bytes.as_slice()).unwrap();
    assert_eq!(Things::Default, after);
}
