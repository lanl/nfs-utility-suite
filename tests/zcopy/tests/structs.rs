// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

include!(concat!(env!("OUT_DIR"), "/structs.rs"));

use crate::structs::*;
use xdr_lib::Reader;

#[test]
fn test_structs_basic() {
    #[rustfmt::skip]
    let data: Vec<u8> = vec![
        0xC0, 0xFF, 0xEE, 0x11, // int Foo.a
        0x11, 0xEE, 0xFF, 0xC0, // unsigned int Foo.blah.a
        0x00, 0x00, 0x00, 0x01, // Val Foo.blah.one.val
        0xBA, 0xDD, 0xF0, 0x00,
        0xDD, 0xC0, 0xFF, 0xEE, // hyper foo.blah.one.x
        0xDD, 0xC0, 0xFF, 0xEE,
        0xBA, 0xDD, 0xF0, 0x00, // unsigned hyper foo.blah.one.y
        0xFA, 0xDF, 0xFD, 0xAF, // int Foo.blah.b
        0x01, 0x10, 0x01, 0x10, // unsigned int Foo.b
        0x00, 0x00, 0x00, 0x00, // bool Foo.no = false
        0x00, 0x80, 0x00, 0x00  // bool Foo.yes = true
    ];

    let reader = structs::FooReader::from_buf(data.as_slice()).unwrap();
    assert_eq!(reader.get_a(), i32::from_be_bytes([0xC0, 0xFF, 0xEE, 0x11]));

    let bar_reader = reader.get_blah();
    {
        assert_eq!(bar_reader.get_a(), 0x11EEFFC0);

        let another_reader = bar_reader.get_one();
        {
            assert_eq!(another_reader.get_val(), Val::one);
            assert_eq!(
                another_reader.get_x(),
                i64::from_be_bytes([0xBA, 0xDD, 0xF0, 0x00, 0xDD, 0xC0, 0xFF, 0xEE])
            );
            assert_eq!(another_reader.get_y(), 0xDDC0FFEEBADDF000);
            assert_eq!(another_reader.get_width().unwrap(), 20);
        }

        assert_eq!(
            bar_reader.get_b(),
            i32::from_be_bytes([0xFA, 0xDF, 0xFD, 0xAF])
        );
        assert_eq!(bar_reader.get_width().unwrap(), 28);
    }

    assert_eq!(reader.get_no(), false);
    assert_eq!(reader.get_yes(), true);
    assert_eq!(reader.get_width().unwrap(), 44);
}
