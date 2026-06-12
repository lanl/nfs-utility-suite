use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

include!(concat!(env!("OUT_DIR"), "/unions.rs"));

use crate::unions::*;
use xdr_lib::{DeserializeError, Reader};

fn to_be_bytes_i32(val: i32) -> [u8; 4] {
    val.to_be_bytes()
}
fn to_be_bytes_u32(val: u32) -> [u8; 4] {
    val.to_be_bytes()
}
fn to_be_bytes_u64(val: u64) -> [u8; 8] {
    val.to_be_bytes()
}

#[test]
fn test_plant_kind_deserialize() {
    assert_eq!(
        PlantKind::deserialize(&to_be_bytes_i32(0)),
        Ok(PlantKind::Tree)
    );
    assert_eq!(
        PlantKind::deserialize(&to_be_bytes_i32(1)),
        Ok(PlantKind::Grass)
    );
    assert_eq!(
        PlantKind::deserialize(&to_be_bytes_i32(2)),
        Ok(PlantKind::Flower)
    );
    assert_eq!(
        PlantKind::deserialize(&to_be_bytes_i32(3)),
        Err(DeserializeError)
    );
}

#[test]
fn test_plant_reader() {
    let mut buf = Vec::new();
    buf.extend_from_slice(&to_be_bytes_i32(0));
    buf.extend_from_slice(&to_be_bytes_i32(15));
    let reader = PlantReader::new(&buf).unwrap();

    assert_eq!(reader.deserialize(), Plant::Tree(15));
    assert_eq!(reader.get_width().unwrap(), 8);

    // let buf = to_be_bytes_i32(99);
    // let reader = PlantReader::new(&buf).unwrap();
    // assert_eq!(reader.deserialize(), Err(DeserializeError));
}

#[test]
fn test_num_leaves_reader() {
    let buf = to_be_bytes_i32(0);
    let reader = NumLeavesReader::new(&buf).unwrap();
    assert_eq!(reader.deserialize(), None);
    assert_eq!(reader.get_width().unwrap(), 4);

    let mut buf = Vec::new();
    buf.extend_from_slice(&to_be_bytes_i32(1));
    buf.extend_from_slice(&to_be_bytes_u32(100));
    let reader = NumLeavesReader::new(&buf).unwrap();
    assert_eq!(reader.deserialize(), Some(100));
    assert_eq!(reader.get_width().unwrap(), 8);
}

#[test]
fn test_maybe_a_plant_kind_reader() {
    let buf = to_be_bytes_i32(0);
    let reader = MaybeAPlantKindReader::new(&buf).unwrap();
    assert_eq!(reader.deserialize(), None);
    assert_eq!(reader.get_width().unwrap(), 4);

    let mut buf = Vec::new();
    buf.extend_from_slice(&to_be_bytes_i32(1));
    buf.extend_from_slice(&to_be_bytes_i32(1));
    let reader = MaybeAPlantKindReader::new(&buf).unwrap();
    assert_eq!(reader.deserialize(), Some(PlantKind::Grass));
    assert_eq!(reader.get_width().unwrap(), 8);
}

#[test]
fn test_stuff_reader() {
    let mut buf = Vec::new();
    buf.extend_from_slice(&to_be_bytes_i32(-5));
    buf.extend_from_slice(&to_be_bytes_u64(999999999));

    let reader = StuffReader::new(&buf).unwrap();
    assert_eq!(reader.get_a(), -5);
    assert_eq!(reader.get_b(), 999999999);
    assert_eq!(reader.get_width().unwrap(), 12);
}

#[test]
fn test_maybe_stuff_reader() {
    let buf = to_be_bytes_i32(0);
    let reader = MaybeStuffReader::new(&buf).unwrap();
    assert_eq!(reader.deserialize(), None);
    assert_eq!(reader.get_width().unwrap(), 4);

    let mut buf = Vec::new();
    buf.extend_from_slice(&to_be_bytes_i32(1));
    buf.extend_from_slice(&to_be_bytes_i32(10));
    buf.extend_from_slice(&to_be_bytes_u64(20));

    let reader = MaybeStuffReader::new(&buf).unwrap();
    let result = reader.deserialize().unwrap();
    assert_eq!(result.get_a(), 10);
    assert_eq!(result.get_b(), 20);
    assert_eq!(reader.get_width().unwrap(), 16);
}

#[test]
fn test_has_string_reader() {
    let buf = to_be_bytes_i32(0);
    let reader = HasStringReader::new(&buf).unwrap();
    assert_eq!(reader.deserialize(), None);
    assert_eq!(reader.get_width().unwrap(), 4);

    let mut buf = Vec::new();
    buf.extend_from_slice(&to_be_bytes_i32(1));
    buf.extend_from_slice(&to_be_bytes_u32(4));
    buf.extend_from_slice(b"Rust");

    let reader = HasStringReader::new(&buf).unwrap();
    assert_eq!(reader.deserialize(), Some(OsStr::from_bytes(b"Rust")));
    assert_eq!(reader.get_width().unwrap(), 12);
}

#[test]
fn test_cases_deserialize() {
    assert_eq!(Cases::deserialize(&to_be_bytes_i32(1)), Ok(Cases::one));
    assert_eq!(Cases::deserialize(&to_be_bytes_i32(2)), Ok(Cases::two));
    assert_eq!(Cases::deserialize(&to_be_bytes_i32(3)), Ok(Cases::three));
}

#[test]
fn test_stuff_or_plant_reader() {
    let mut buf = Vec::new();
    buf.extend_from_slice(&to_be_bytes_i32(1));
    buf.extend_from_slice(&to_be_bytes_i32(50));
    buf.extend_from_slice(&to_be_bytes_u64(100));
    let reader = StuffOrPlantReader::new(&buf).unwrap();
    assert!(matches!(reader.deserialize(), StuffOrPlant::one(_)));
    assert_eq!(reader.get_width().unwrap(), 16);

    let mut buf = Vec::new();
    buf.extend_from_slice(&to_be_bytes_i32(2));
    buf.extend_from_slice(&to_be_bytes_i32(2));
    let reader = StuffOrPlantReader::new(&buf).unwrap();
    assert_eq!(reader.deserialize(), StuffOrPlant::two(PlantKind::Flower));
    assert_eq!(reader.get_width().unwrap(), 8);

    let mut buf = Vec::new();
    buf.extend_from_slice(&to_be_bytes_i32(3));
    buf.extend_from_slice(&to_be_bytes_i32(1));
    buf.extend_from_slice(&to_be_bytes_i32(500));
    let reader = StuffOrPlantReader::new(&buf).unwrap();
    assert!(matches!(reader.deserialize(), StuffOrPlant::three(_)));
    assert_eq!(reader.get_width().unwrap(), 12);
}

#[test]
fn test_same_width_different_stuff_reader() {
    let mut buf = Vec::new();
    buf.extend_from_slice(&to_be_bytes_i32(10));
    buf.extend_from_slice(&to_be_bytes_i32(20));
    buf.extend_from_slice(&to_be_bytes_i32(30));

    let reader = SameWidthDifferentStuffReader::new(&buf).unwrap();
    assert_eq!(reader.get_width().unwrap(), 12);
}

#[test]
fn test_stuff_or_plant2_reader() {
    let mut buf = Vec::new();
    buf.extend_from_slice(&to_be_bytes_i32(2));
    buf.extend_from_slice(&to_be_bytes_i32(100));
    buf.extend_from_slice(&to_be_bytes_i32(200));
    buf.extend_from_slice(&to_be_bytes_i32(300));
    let reader = StuffOrPlant2Reader::new(&buf).unwrap();
    assert!(matches!(reader.deserialize(), StuffOrPlant2::two(_)));
    assert_eq!(reader.get_width().unwrap(), 16);

    let mut buf = Vec::new();
    buf.extend_from_slice(&to_be_bytes_i32(99));
    buf.extend_from_slice(&to_be_bytes_i32(3));
    let reader = StuffOrPlant2Reader::new(&buf).unwrap();
    assert_eq!(reader.deserialize(), StuffOrPlant2::Default(Cases::three));
    assert_eq!(reader.get_width().unwrap(), 8);
}
