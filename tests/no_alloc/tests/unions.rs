// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

include!(concat!(env!("OUT_DIR"), "/unions.rs"));
use unions::*;

#[test]
fn bool_union() {
    // True case:
    let before = MyOption { inner: Some(7) };

    let mut bytes = vec![1; 8];
    assert_eq!(8, before.serialize(&mut bytes));

    let mut after = MyOption::default();

    MyOption::deserialize(&mut after, &mut bytes.as_slice()).unwrap();
    assert_eq!(before, after);

    // False case:
    let before = MyOption { inner: None };

    assert_eq!(4, before.serialize(&mut bytes));

    MyOption::deserialize(&mut after, &mut bytes.as_slice()).unwrap();
    assert_eq!(before, after);
}
