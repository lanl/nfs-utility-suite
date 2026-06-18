// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

include!(concat!(env!("OUT_DIR"), "/hello.rs"));

use crate::hello::*;
use xdr_lib::Reader;

#[test]
fn test_hello_basic() {
    #[rustfmt::skip]
    let data: Vec<u8> = vec![
        0xC0, 0xFF, 0xEE, 0x11, // unsigned int abc
        0x00, 0xBA, 0xB1, 0x0C, // int def
        0x00, 0x00, 0x00, 0x02, // favorite_fruit = Banana
        0xDE, 0xAD, 0xBE, 0xAF, // should be ignored
    ];

    let reader = HelloReader::new(data.as_slice()).unwrap();

    assert_eq!(reader.get_abc(), 0xC0FFEE11);
    assert_eq!(reader.get_def(), 0x00BAB10C);
    assert_eq!(reader.get_favorite_fruit(), Fruit::Banana);
    assert_eq!(reader.get_width().unwrap(), 12);
}

#[test]
fn test_hello_basic_size_ooe() {
    #[rustfmt::skip]
    let data: Vec<u8> = vec![
        0xC0, 0xFF, 0xEE, 0x11, // unsigned int abc
        0x00, 0xBA, 0xB1, 0x0C, // int def
        0x00, 0x00, 0x00, 0x02, // favorite_fruit = Banana
    ];

    let reader = HelloReader::new(data.as_slice()).unwrap();

    // this should all just work if we access out of order as well
    assert_eq!(reader.get_width().unwrap(), 12);
    assert_eq!(reader.get_def(), 0x00BAB10C);
    assert_eq!(reader.get_favorite_fruit(), Fruit::Banana);
    assert_eq!(reader.get_abc(), 0xC0FFEE11);
}

#[test]
fn test_hello_invalid_enum() {
    #[rustfmt::skip]
    let data: Vec<u8> = vec![
        0xC0, 0xFF, 0xEE, 0x11, // unsigned int abc
        0x00, 0xBA, 0xB1, 0x0C, // int def
        0x80, 0x00, 0x00, 0x02, // favorite_fruit = invalid
        0xDE, 0xAD, 0xBE, 0xAF, // should be ignored
    ];

    assert!(HelloReader::new(data.as_slice()).is_err());
}

#[test]
fn test_hello_one_byte_short() {
    #[rustfmt::skip]
    let data: Vec<u8> = vec![
        0xC0, 0xFF, 0xEE, 0x11, // unsigned int abc
        0x00, 0xBA, 0xB1, 0x0C, // int def
        0x80, 0x00, 0x00, // favorite_fruit = invalid
    ];

    assert!(HelloReader::new(data.as_slice()).is_err());
}
