use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

use rand::distr::{Alphanumeric, SampleString};

include!(concat!(env!("OUT_DIR"), "/typedef.rs"));

use crate::typedef::*;

#[test]
fn test_typedef() {
    let mut data: Vec<u8> = vec![0xC0, 0xFF, 0xEE, 0x11];
    let filename_data: Vec<u8> = Alphanumeric
        .sample_string(&mut rand::rng(), 819)
        .into_bytes();
    let contents_data: Vec<u8> = Alphanumeric
        .sample_string(&mut rand::rng(), 1023)
        .into_bytes();

    data.extend_from_slice(&819_i32.to_be_bytes());
    data.extend_from_slice(filename_data.as_slice());
    data.extend_from_slice(&[0x0]); // pad to 4 byte alignment
    data.extend_from_slice(&1023_i32.to_be_bytes());
    data.extend_from_slice(contents_data.as_slice());
    data.extend_from_slice(&[0x0]); // pad to 4 byte alignment

    data.extend([
        0xDD, 0xFF, 0xEE, 0xAA, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
    ]);

    let reader = FileReader::new(data.as_slice()).unwrap();

    assert_eq!(
        reader.get_owner(),
        i32::from_be_bytes([0xC0, 0xFF, 0xEE, 0x11])
    );
    let timestamp_reader = reader.get_t();

    assert_eq!(
        reader.get_name(),
        <OsStr as OsStrExt>::from_bytes(filename_data.as_slice())
    );

    assert_eq!(reader.get_contents(), contents_data.as_slice());

    assert_eq!(
        timestamp_reader.get_atime(),
        i32::from_be_bytes([0xDD, 0xFF, 0xEE, 0xAA])
    );
    assert_eq!(
        timestamp_reader.get_ctime(),
        i32::from_be_bytes([0x11, 0x22, 0x33, 0x44])
    );
    assert_eq!(
        timestamp_reader.get_mtime(),
        i32::from_be_bytes([0x55, 0x66, 0x77, 0x88])
    );
}

#[test]
fn test_typedef_limited_lencheck() {
    let mut data: Vec<u8> = vec![0xC0, 0xFF, 0xEE, 0x11];
    let filename_data: Vec<u8> = Alphanumeric
        .sample_string(&mut rand::rng(), 819)
        .into_bytes();
    let contents_data: Vec<u8> = Alphanumeric
        .sample_string(&mut rand::rng(), 1027)
        .into_bytes();

    data.extend_from_slice(&819_i32.to_be_bytes());
    data.extend_from_slice(filename_data.as_slice());
    data.extend_from_slice(&[0x0]); // pad to 4 byte alignment
    data.extend_from_slice(&1027_i32.to_be_bytes());
    data.extend_from_slice(contents_data.as_slice());
    data.extend_from_slice(&[0x0]); // pad to 4 byte alignment

    data.extend([
        0xDD, 0xFF, 0xEE, 0xAA, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
    ]);

    assert!(FileReader::new(data.as_slice()).is_err());
}

#[test]
fn test_typedef_filename_lencheck() {
    let mut data: Vec<u8> = vec![0xC0, 0xFF, 0xEE, 0x11];
    let filename_data: Vec<u8> = Alphanumeric
        .sample_string(&mut rand::rng(), 818)
        .into_bytes();
    let contents_data: Vec<u8> = Alphanumeric
        .sample_string(&mut rand::rng(), 1023)
        .into_bytes();

    data.extend_from_slice(&819_i32.to_be_bytes());
    data.extend_from_slice(filename_data.as_slice());
    data.extend_from_slice(&[0x0]); // pad to 4 byte alignment
    data.extend_from_slice(&1023_i32.to_be_bytes());
    data.extend_from_slice(contents_data.as_slice());
    data.extend_from_slice(&[0x0]); // pad to 4 byte alignment

    data.extend([
        0xDD, 0xFF, 0xEE, 0xAA, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
    ]);

    assert!(FileReader::new(data.as_slice()).is_err());
}
