// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

// Tests for the non-allocating serialization APIs.

include!(concat!(env!("OUT_DIR"), "/arrays.rs"));
use arrays::*;

#[test]
fn fixed_length_byte_arrays() {
    let before = OpaqueArrays {
        a: [7u8],
        b: [1u8, 2u8],
        c: [255u8, 120u8, 0u8],
        d: [1u8, 0u8, 3u8, 4u8],
    };

    let mut bytes = vec![1; 16];

    assert_eq!(16, before.serialize(&mut bytes));

    let mut after = OpaqueArrays::default();

    OpaqueArrays::deserialize(&mut after, &mut bytes.as_slice()).unwrap();

    assert_eq!(before, after);
}

#[test]
fn limited_length_byte_arrays() {
    let before = LimitedOpaqueArrays {
        a: vec![],
        b: vec![1u8],
        c: vec![2u8, 3u8, 4u8],
        d: vec![5u8, 6u8],
        e: vec![6u8, 7u8, 8u8, 9u8],
    };

    let mut bytes = vec![1; 36];

    assert_eq!(36, before.serialize(&mut bytes));

    let mut after = LimitedOpaqueArrays::default();

    LimitedOpaqueArrays::deserialize(&mut after, &mut bytes.as_slice()).unwrap();

    assert_eq!(before, after);
}

#[test]
#[should_panic]
fn limited_length_array_exceeded() {
    let before = LimitedOpaqueArrays {
        a: vec![],
        b: vec![1u8, 2u8, 3u8],
        c: vec![],
        d: vec![],
        e: vec![],
    };

    let mut bytes = vec![0; 36];

    let _ = before.serialize(&mut bytes);
}

#[test]
fn unlimited_byte_array() {
    let before = UnlimitedOpaqueArray { data: vec![7; 499] };

    let mut bytes = vec![1u8; 504];

    assert_eq!(504, before.serialize(&mut bytes));

    let mut after = UnlimitedOpaqueArray::default();
    UnlimitedOpaqueArray::deserialize(&mut after, &mut bytes.as_slice()).unwrap();

    assert_eq!(before, after);
}

#[test]
fn strings() {
    let before = Strings {
        lim: "hello".into(),
        unlim: "world!!!".into(),
    };

    let mut bytes = vec![1u8; 24];
    assert_eq!(24, before.serialize(&mut bytes));

    let mut after = Strings::default();
    Strings::deserialize(&mut after, &mut bytes.as_slice()).unwrap();

    assert_eq!(before, after);
}

#[test]
#[should_panic]
fn limited_length_string_exceeded() {
    let before = Strings {
        lim: "hello, world!".into(),
        unlim: "".into(),
    };

    let mut bytes = vec![0; 36];

    let _ = before.serialize(&mut bytes);
}

#[test]
fn arrays_of_user_defined_type() {
    let mut before = IntArrays::default();
    for i in 0..4 {
        before.fixed[i] = AnInt { a: i as u32 };
    }
    for i in 0..7 {
        before.limited.push(AnInt {
            a: u32::MAX - i as u32,
        });
    }
    for i in 0..512 {
        before.unlimited.push(AnInt {
            a: u32::MAX - i as u32,
        });
    }

    let mut bytes = [1; 2100];

    assert_eq!(2100, before.serialize(&mut bytes));

    let mut after = IntArrays::default();

    IntArrays::deserialize(&mut after, &mut bytes.as_slice()).unwrap();

    assert_eq!(before, after);
}

#[test]
#[should_panic]
fn too_long_array_of_user_defined_type() {
    let mut before = IntArrays::default();
    for i in 0..8 {
        before.limited.push(AnInt { a: i });
    }

    let mut bytes = [1; 2100];

    let _ = before.serialize(&mut bytes);
}
