// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

include!(concat!(env!("OUT_DIR"), "/enums.rs"));
use enums::*;

#[test]
fn basic_enum() {
    let before = Cases::two;
    let mut bytes = [1; 4];
    assert_eq!(4, before.serialize(&mut bytes));
    let mut after = Cases::default();
    after.deserialize(&mut bytes.as_slice()).unwrap();
    assert_eq!(before, after);
}

#[test]
fn invalid_enum_variant() {
    let bad_bytes = [0, 0, 0, 4];
    let mut after = Cases::default();
    assert!(after.deserialize(&mut bad_bytes.as_slice()).is_err());
}
