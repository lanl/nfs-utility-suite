// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

include!(concat!(env!("OUT_DIR"), "/structs.rs"));

#[test]
fn too_short_32bit() {
    use structs::*;

    let msg = [1u8, 2u8, 3u8];

    let mut int = Int::default();
    assert!(int.deserialize(&mut msg.as_slice()).is_err());

    let mut uint = Uint::default();
    assert!(uint.deserialize(&mut msg.as_slice()).is_err());

    let mut bool = Bool::default();
    assert!(bool.deserialize(&mut msg.as_slice()).is_err());
}

#[test]
fn too_short_64bit() {
    use structs::*;

    let msg = [1u8, 2u8, 3u8, 4u8];

    let mut hyper = Hyper::default();
    assert!(hyper.deserialize(&mut msg.as_slice()).is_err());

    let msg = [1u8, 2u8, 3u8, 4u8, 5u8, 6u8, 7u8];
    let mut uhyper = Uhyper::default();
    assert!(uhyper.deserialize(&mut msg.as_slice()).is_err());
}
