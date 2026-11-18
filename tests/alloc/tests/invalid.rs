// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

include!(concat!(env!("OUT_DIR"), "/arrays.rs"));
include!(concat!(env!("OUT_DIR"), "/structs.rs"));

#[test]
fn too_short_32bit() {
    use structs::*;

    let msg = [1u8, 2u8, 3u8];

    assert!(Int::deserialize(msg.as_slice()).is_err());

    assert!(Uint::deserialize(msg.as_slice()).is_err());

    assert!(Bool::deserialize(msg.as_slice()).is_err());
}

#[test]
fn too_short_64bit() {
    use structs::*;

    let msg = [1u8, 2u8, 3u8, 4u8];
    assert!(Hyper::deserialize(msg.as_slice()).is_err());

    let msg = [1u8, 2u8, 3u8, 4u8, 5u8, 6u8, 7u8];
    assert!(Uhyper::deserialize(msg.as_slice()).is_err());
}

#[test]
fn length_past_end_of_input() {
    use arrays::*;

    // A length prefix of 0xffffffff with no data behind it.
    let msg = [255u8, 255u8, 255u8, 255u8];

    assert!(UnlimitedOpaqueArray::deserialize(msg.as_slice()).is_err());

    // A string claiming eight bytes when only two follow.
    let msg = [0u8, 0u8, 0u8, 8u8, b'h', b'i'];

    assert!(LimitedOpaqueArray::deserialize(msg.as_slice()).is_err());
}

#[test]
fn missing_padding() {
    use arrays::*;

    // Three bytes of data, so one byte of padding is expected but not present.
    let msg = [0u8, 0u8, 0u8, 3u8, 1u8, 2u8, 3u8];

    assert!(UnlimitedOpaqueArray::deserialize(msg.as_slice()).is_err());
}

#[test]
fn too_long_array() {
    use arrays::*;

    let arr = OpaqueArrays {
        bytes: [0; 3],
        bytes_2: vec![0; 4], // Too long: limit is 3
        bytes_3: vec![0; 1],
    };
    let bytes = arr.serialize_alloc();

    assert!(OpaqueArrays::deserialize(&bytes).is_err());
}

/// On 32-bit systems, tests that overflow is detected when an array length prefix is too close to
/// u32::MAX.
///
/// On 64-bit systems, UnlimitedOpaqueArray::deserialize() still returns an error, although it's
/// actually because the length prefix is longer than the buffer and not because of overflow.
/// Still a useful test though.
#[test]
fn array_length_prefix_overflows() {
    use arrays::*;

    let arr = UnlimitedOpaqueArray { data: vec![0; 10] };
    let mut bytes = arr.serialize_alloc();

    for i in 0..3 {
        let overflowing_length = u32::MAX - i;
        bytes[0..4].copy_from_slice(&overflowing_length.to_be_bytes());
        assert!(UnlimitedOpaqueArray::deserialize(&bytes).is_err());
    }
}
