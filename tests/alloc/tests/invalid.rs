// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

include!(concat!(env!("OUT_DIR"), "/arrays.rs"));
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

#[test]
fn length_past_end_of_input() {
    use arrays::*;

    // A length prefix of 0xffffffff with no data behind it.
    let msg = [255u8, 255u8, 255u8, 255u8];

    let mut opaque = UnlimitedOpaqueArray::default();
    assert!(opaque.deserialize(&mut msg.as_slice()).is_err());

    // A string claiming eight bytes when only two follow.
    let msg = [0u8, 0u8, 0u8, 8u8, b'h', b'i'];

    let mut string = LimitedOpaqueArray::default();
    assert!(string.deserialize(&mut msg.as_slice()).is_err());
}

#[test]
fn missing_padding() {
    use arrays::*;

    // Three bytes of data, so one byte of padding is expected but not present.
    let msg = [0u8, 0u8, 0u8, 3u8, 1u8, 2u8, 3u8];

    let mut opaque = UnlimitedOpaqueArray::default();
    assert!(opaque.deserialize(&mut msg.as_slice()).is_err());
}
