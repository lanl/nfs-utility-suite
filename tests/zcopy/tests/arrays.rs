use rand::distr::{Alphanumeric, SampleString};

include!(concat!(env!("OUT_DIR"), "/arrays.rs"));

use crate::arrays::*;
use xdr_lib::Reader;

#[test]
fn test_opaque_arrays_minsize() {
    let data: Vec<u8> = vec![0x8, 0x8, 0x8, 0xF, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0];
    let reader = OpaqueArraysReader::from_buf(data.as_slice()).unwrap();

    assert_eq!(reader.get_bytes(), vec![0x8, 0x8, 0x8]);
    assert_eq!(reader.get_bytes_2(), vec![]);
    assert_eq!(reader.get_bytes_3(), vec![]);
}

#[test]
fn test_opaque_arrays() {
    let mut data: Vec<u8> = vec![0x8, 0x9, 0x1];
    let bytes2: Vec<u8> = Alphanumeric.sample_string(&mut rand::rng(), 2).into_bytes();
    let bytes3: Vec<u8> = Alphanumeric
        .sample_string(&mut rand::rng(), 1023)
        .into_bytes();

    data.extend([0xF]);
    data.extend(bytes2.as_slice());
    data.extend([0xF]);
    data.extend(bytes3.as_slice())
}

#[test]
fn test_opaque_arrays_reader() {
    let mut data = Vec::new();
    data.extend_from_slice(&[1, 2, 3, 0]);
    data.extend_from_slice(&2_i32.to_be_bytes());
    data.extend_from_slice(&[4, 5, 0, 0]);
    data.extend_from_slice(&4_i32.to_be_bytes());
    data.extend_from_slice(&[6, 7, 8, 9]);

    let reader = OpaqueArraysReader::from_buf(&data).unwrap();

    assert_eq!(reader.get_bytes(), &[1, 2, 3]);
    assert_eq!(reader.get_bytes_2(), &[4, 5]);
    assert_eq!(reader.get_bytes_3(), &[6, 7, 8, 9]);

    assert_eq!(reader.get_bytes_2_width(), Ok(8));
    assert_eq!(reader.get_bytes_3_width(), Ok(8));
    assert_eq!(reader.get_width(), Ok(8 + 8 + 4));
}

#[test]
fn test_an_int_reader() {
    let mut data = Vec::new();
    data.extend_from_slice(&42_i32.to_be_bytes());

    let reader = AnIntReader::from_buf(&data).unwrap();
    assert_eq!(reader.get_a(), 42);
    assert_eq!(reader.get_width(), Ok(4));
}

#[test]
fn test_int_arrays_reader() {
    let mut data = Vec::new();
    for i in 1_i32..=4 {
        data.extend_from_slice(&i.to_be_bytes());
    }
    data.extend_from_slice(&2_u32.to_be_bytes());
    data.extend_from_slice(&10_u32.to_be_bytes());
    data.extend_from_slice(&20_u32.to_be_bytes());
    data.extend_from_slice(&1_u32.to_be_bytes());
    data.extend_from_slice(&100_u32.to_be_bytes());

    let reader = IntArraysReader::from_buf(&data).unwrap();

    let mut fixed_iter = reader.get_fixed();
    assert_eq!(fixed_iter.next().unwrap().get_a(), 1);
    assert_eq!(fixed_iter.next().unwrap().get_a(), 2);
    assert_eq!(fixed_iter.next().unwrap().get_a(), 3);
    assert_eq!(fixed_iter.next().unwrap().get_a(), 4);
    assert!(fixed_iter.next().is_none());

    let mut limited_iter = reader.get_limited();
    assert_eq!(limited_iter.next().unwrap().get_a(), 10);
    assert_eq!(limited_iter.next().unwrap().get_a(), 20);
    assert!(limited_iter.next().is_none());

    assert_eq!(reader.get_limited_width(), Ok(12));
    assert_eq!(reader.get_unlimited_width(), Ok(8));
    assert_eq!(reader.get_width(), Ok(12 + 8 + 16));
}

#[test]
fn test_strings_reader() {
    let mut data = Vec::new();

    data.extend_from_slice(&5_i32.to_be_bytes());
    data.extend_from_slice(b"Hello\0\0\0");

    data.extend_from_slice(&3_i32.to_be_bytes());
    data.extend_from_slice(b"XDR\0");

    let reader = StringsReader::from_buf(&data).unwrap();

    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    assert_eq!(reader.get_str(), OsStr::from_bytes(b"Hello"));
    assert_eq!(reader.get_str_2(), OsStr::from_bytes(b"XDR"));

    assert_eq!(reader.get_str_width(), Ok(12));
    assert_eq!(reader.get_str_2_width(), Ok(8));
    assert_eq!(reader.get_width(), Ok(20));
}

#[test]
fn test_const_size_array_reader() {
    let mut data = Vec::new();
    data.extend_from_slice(&[10, 20, 30, 40, 50, 0, 0, 0]);
    for i in 1_i32..=5 {
        data.extend_from_slice(&(i * 10_i32).to_be_bytes());
    }

    let reader = ConstSizeArrayReader::from_buf(&data).unwrap();
    assert_eq!(reader.get_bytes(), &[10, 20, 30, 40, 50]);

    let mut ints_iter = reader.get_ints();
    assert_eq!(ints_iter.next().unwrap(), 10);
    assert_eq!(ints_iter.next().unwrap(), 20);
    assert_eq!(ints_iter.next().unwrap(), 30);
    assert_eq!(ints_iter.next().unwrap(), 40);
    assert_eq!(ints_iter.next().unwrap(), 50);
    assert!(ints_iter.next().is_none());
    assert_eq!(reader.get_width(), Ok(28));
}

#[test]
fn test_many_ints_reader() {
    let mut data = Vec::new();
    data.extend_from_slice(&0x1111222233334444_u64.to_be_bytes());
    data.extend_from_slice(&0x5555666677778888_u64.to_be_bytes());

    data.extend_from_slice(&2_u32.to_be_bytes());
    data.extend_from_slice(&500_u32.to_be_bytes());
    data.extend_from_slice(&600_u32.to_be_bytes());

    data.extend_from_slice(&1_u32.to_be_bytes());
    data.extend_from_slice(&0x9999999999999999_u64.to_be_bytes());

    let reader = ManyIntsReader::from_buf(&data).unwrap();

    let mut first_iter = reader.get_first();
    assert_eq!(first_iter.next().unwrap(), 0x1111222233334444);
    assert_eq!(first_iter.next().unwrap(), 0x5555666677778888);
    assert!(first_iter.next().is_none());

    let mut second_iter = reader.get_second();
    assert_eq!(second_iter.next().unwrap(), 500);
    assert_eq!(second_iter.next().unwrap(), 600);
    assert!(second_iter.next().is_none());

    let mut third_iter = reader.get_third();
    assert_eq!(third_iter.next().unwrap(), 0x9999999999999999_u64 as i64);
    assert!(third_iter.next().is_none());

    assert_eq!(reader.get_second_width(), Ok(12));
    assert_eq!(reader.get_third_width(), Ok(12));
    assert_eq!(reader.get_width(), Ok(12 + 12 + 16));
}
