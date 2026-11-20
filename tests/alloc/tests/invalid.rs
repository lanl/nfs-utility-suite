// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

include!(concat!(env!("OUT_DIR"), "/arrays.rs"));
include!(concat!(env!("OUT_DIR"), "/optional.rs"));
include!(concat!(env!("OUT_DIR"), "/structs.rs"));
include!(concat!(env!("OUT_DIR"), "/unions.rs"));

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

/// A bool-discriminated union should only accept 0 and 1 as the discriminant values.
#[test]
fn bool_union_invalid_discriminant() {
    let union = unions::AnOption { inner: Some(7) };

    let mut bytes = union.serialize_alloc();

    bytes[0..4].copy_from_slice(&2u32.to_be_bytes());
    assert!(unions::AnOption::deserialize(&bytes).is_err());

    bytes[0..4].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(unions::AnOption::deserialize(&bytes).is_err());
}

/// An optional is equivalent to a bool-discriminated union with a void arm, so its
/// discriminant must only be 0 and 1.
#[test]
fn optional_invalid_discriminant() {
    for invalid in [2u32, u32::MAX] {
        let nonrecursive = optional::JustAnOption {
            maybe: Some(optional::NonRecursive {
                stuff: 7,
                str: "hello".into(),
            }),
        };
        let mut bytes = nonrecursive.serialize_alloc();
        bytes[0..4].copy_from_slice(&invalid.to_be_bytes());
        assert!(optional::JustAnOption::deserialize(&bytes).is_err());

        let recursive = optional::ListBegin {
            list: vec![optional::ListNode { data: 7 }],
            eof: true,
        };
        let mut bytes = recursive.serialize_alloc();
        bytes[0..4].copy_from_slice(&invalid.to_be_bytes());
        assert!(optional::ListBegin::deserialize(&bytes).is_err());
    }
}

/// An int-discriminated union without a default arm should not allow discriminant values other
/// than the specified arms.
#[test]
fn int_union_invalid_disc() {
    for invalid in [0u32, 3u32] {
        let u = unions::IntDiscriminant::VAL1(7);
        let mut bytes = u.serialize_alloc();
        bytes[0..4].copy_from_slice(&invalid.to_be_bytes());
        assert!(unions::IntDiscriminant::deserialize(&bytes).is_err());
    }
}
