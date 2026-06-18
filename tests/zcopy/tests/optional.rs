// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::os::unix::ffi::OsStrExt;

include!(concat!(env!("OUT_DIR"), "/optional.rs"));

use crate::optional::*;
use xdr_lib::{DeserializeError, Reader};

#[test]
fn test_optionals_nonrecursive() {
    let mut data: Vec<u8> = vec![
        0x0, 0x0, 0x0, 0x0, 0xC0, 0xFF, 0xEE, 0x11, 0x0, 0x0, 0x0, 0x5,
    ];
    data.extend("hello".as_bytes());
    data.extend([0x1u8, 0x2, 0x3]); // padding

    let reader = JustAnOptionReader::new(data.as_slice()).unwrap();
    assert_eq!(reader.get_maybe(), None);

    data[3] = 0x1;
    let reader = JustAnOptionReader::new(data.as_slice()).unwrap();
    let nonrecursive_reader = reader.get_maybe().unwrap();

    assert_eq!(
        nonrecursive_reader.get_stuff(),
        i32::from_be_bytes([0xC0, 0xFF, 0xEE, 0x11])
    );

    assert_eq!(nonrecursive_reader.get_str().as_bytes(), "hello".as_bytes());

    assert_eq!(nonrecursive_reader.get_width(), Ok(16));
    assert_eq!(reader.get_width(), Ok(20));
    assert_eq!(reader.get_maybe_width(), Ok(20));
}

#[test]
fn test_optionals_nonrecursive_missing_discriminant() {
    let data: Vec<u8> = vec![];
    assert!(JustAnOptionReader::new(data.as_slice()).is_err());
}

#[test]
fn test_optionals_nonrecursive_missing_data() {
    let data: Vec<u8> = vec![0x0, 0x0, 0x0, 0x1];
    assert!(JustAnOptionReader::new(data.as_slice()).is_err());
}

#[test]
fn test_optionals_recursive() {
    let mut data: Vec<u8> = vec![];
    let mut integers: Vec<i32> = vec![];

    for i in 0i32..100 {
        data.extend([0x0, 0x0, 0x0, 0x1]);
        data.extend(i.to_be_bytes());
        integers.push(i);
    }

    for i in 0i32..110 {
        data.extend([0x0, 0x0, 0x0, 0x0]);
        data.extend(i.to_be_bytes());
    }

    let reader = ListBeginReader::new(data.as_slice()).unwrap();
    let actual_ints: Vec<i32> = reader.get_list().map(|v| v.unwrap().get_data()).collect();

    assert_eq!(integers, actual_ints);
    assert_eq!(reader.get_width(), Ok(8 * 100 + 4));
}

#[test]
fn test_optionals_recursive_missing_first_discriminant() {
    let data: Vec<u8> = vec![];
    let reader = ListBeginReader::new(data.as_slice()).unwrap();
    assert_eq!(
        reader
            .get_list()
            .collect::<Vec<Result<ListNodeReader, DeserializeError>>>(),
        vec![Err(xdr_lib::DeserializeError)]
    );
}

#[test]
fn test_optionals_recursive_missing_last_discriminant() {
    let mut data: Vec<u8> = vec![];
    let mut integers: Vec<xdr_lib::Result<i32>> = vec![];

    for i in 0i32..100 {
        data.extend([0x0, 0x0, 0x0, 0x1]);
        data.extend(i.to_be_bytes());
        integers.push(Ok(i));
    }
    integers.push(Err(DeserializeError));

    let reader = ListBeginReader::new(&data.as_slice()[..data.len()]).unwrap();
    let actual_integers: Vec<xdr_lib::Result<i32>> = reader
        .get_list()
        .map(|v| v.map(|reader| reader.get_data()))
        .collect();

    assert_eq!(actual_integers, integers);
}

struct Groupnode {
    name: String,
}

struct Exportnode {
    dirpath: String,
    groups: Vec<Groupnode>,
}

#[test]
fn test_optionals_recursive_varlen_interiors() {
    let mut data: Vec<u8> = vec![];

    let mut export_groups: Vec<Exportnode> = Vec::new();
    for i in 0..5 {
        let mut export = Exportnode {
            dirpath: format!("test_{i}").into(),
            groups: Vec::new(),
        };
        for j in 0..5 {
            let group = Groupnode {
                name: format!("group_{j}").into(),
            };
            export.groups.push(group);
        }
        export_groups.push(export);
    }

    for en in export_groups.iter() {
        data.extend([0x0, 0x0, 0x0, 0x1u8]);
        data.extend((en.dirpath.len() as u32).to_be_bytes());
        data.extend(en.dirpath.as_bytes());
        let padding = (en.dirpath.len() + 3) & !(0b11usize);
        data.extend(vec![0u8; padding]);
        for gn in en.groups.iter() {
            data.extend([0x0, 0x0, 0x0, 0x1u8]);
            data.extend((gn.name.len() as u32).to_be_bytes());
            data.extend(gn.name.as_bytes());
            let padding = (gn.name.len() + 3) & !(0b11usize);
            data.extend(vec![0u8; padding]);
        }
        data.extend([0x0, 0x0, 0x0, 0x0]);
    }
    data.extend([0x0, 0x0, 0x0, 0x0]);

    let reader = exportsReader::new(data.as_slice()).unwrap();
    for (i, en) in reader.get_inner().enumerate() {
        assert_eq!(
            export_groups.get(i).unwrap().dirpath.as_str().as_bytes(),
            en.as_ref().unwrap().get_ex_dir().as_bytes()
        );

        for (j, gn) in en.unwrap().get_ex_groups().enumerate() {
            let first_string = export_groups
                .get(i)
                .unwrap()
                .groups
                .get(j)
                .unwrap()
                .name
                .as_bytes();
            let second_string = gn.unwrap().get_gr_name().as_bytes();
            assert_eq!(first_string, second_string);
        }
    }
}

#[test]
fn test_enum_chain() {
    let mut enums = vec![];
    let mut data: Vec<u8> = vec![];

    for i in 0..100 {
        data.extend(1u32.to_be_bytes());
        if i % 2 == 0 {
            enums.push(MyEnum::ZERO);
            data.extend(0u32.to_be_bytes());
        } else {
            enums.push(MyEnum::ONE);
            data.extend(1u32.to_be_bytes());
        }
    }

    data.extend(0u32.to_be_bytes());

    let reader = EnumChainStartReader::new(data.as_slice()).unwrap();
    let actual_enums = reader
        .get_first()
        .map(|v| v.unwrap().get_a())
        .collect::<Vec<_>>();

    assert_eq!(actual_enums, enums);
}

#[test]
fn test_enum_chain_middle_error() {
    let mut enums = vec![];
    let mut data: Vec<u8> = vec![];

    for i in 0..100 {
        data.extend(1u32.to_be_bytes());
        if i == 49 {
            enums.push(Err(DeserializeError));
            data.extend(2u32.to_be_bytes());
        } else if i % 2 == 0 {
            enums.push(Ok(MyEnum::ZERO));
            data.extend(0u32.to_be_bytes());
        } else {
            enums.push(Ok(MyEnum::ONE));
            data.extend(1u32.to_be_bytes());
        }
    }

    data.extend(0u32.to_be_bytes());

    let reader = EnumChainStartReader::new(data.as_slice()).unwrap();
    let actual_enums = reader
        .get_first()
        .map(|v| v.map(|res| res.get_a()))
        .collect::<Vec<_>>();

    assert_eq!(actual_enums, enums[0..50]);
}

#[test]
fn test_enum_chain_end_missing_data() {
    let mut enums = vec![];
    let mut data: Vec<u8> = vec![];

    for i in 0..100 {
        data.extend(1u32.to_be_bytes());
        if i % 2 == 0 {
            enums.push(Ok(MyEnum::ZERO));
            data.extend(0u32.to_be_bytes());
        } else {
            enums.push(Ok(MyEnum::ONE));
            data.extend(1u32.to_be_bytes());
        }
    }

    data.extend(1u32.to_be_bytes());
    enums.push(Err(DeserializeError));

    let reader = EnumChainStartReader::new(data.as_slice()).unwrap();
    let actual_enums = reader
        .get_first()
        .map(|v| v.map(|res| res.get_a()))
        .collect::<Vec<_>>();

    assert_eq!(actual_enums, enums);
}

#[test]
fn test_enum_chain_end_missing_discriminant() {
    let mut enums = vec![];
    let mut data: Vec<u8> = vec![];

    for i in 0..100 {
        data.extend(1u32.to_be_bytes());
        if i % 2 == 0 {
            enums.push(Ok(MyEnum::ZERO));
            data.extend(0u32.to_be_bytes());
        } else {
            enums.push(Ok(MyEnum::ONE));
            data.extend(1u32.to_be_bytes());
        }
    }
    enums.push(Err(DeserializeError));

    let reader = EnumChainStartReader::new(data.as_slice()).unwrap();
    let actual_enums = reader
        .get_first()
        .map(|v| v.map(|res| res.get_a()))
        .collect::<Vec<_>>();

    assert_eq!(actual_enums, enums);
}
