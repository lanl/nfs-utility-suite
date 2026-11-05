// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

// Tests for the non-allocating serialization APIs.

include!(concat!(env!("OUT_DIR"), "/optional.rs"));
use optional::*;

#[test]
fn recursive_optional() {
    let mut before = ListBegin::default();
    for i in 0..5 {
        let node = ListNode { data: i };
        before.list.push(node);
    }
    assert_eq!(before.get_width(), 48);

    let mut bytes = vec![1; 48];
    assert_eq!(48, before.serialize(&mut bytes));

    let (after, _) = ListBegin::deserialize(bytes.as_slice()).unwrap();
    assert_eq!(48, after.get_width());

    assert_eq!(before, after);
}

#[test]
fn non_recursive_optional_none() {
    let before = JustAnOption { maybe: None };
    assert_eq!(before.get_width(), 4);

    let mut bytes = vec![1; 4];
    assert_eq!(4, before.serialize(&mut bytes));

    let (after, _) = JustAnOption::deserialize(bytes.as_slice()).unwrap();
    assert_eq!(before, after);
}

#[test]
fn non_recursive_optional_some() {
    let before = JustAnOption {
        maybe: Some(NonRecursive {
            stuff: 49,
            str: "Hello, world!".into(),
        }),
    };
    assert_eq!(before.get_width(), 28);

    let mut bytes = vec![1; 28];
    assert_eq!(28, before.serialize(&mut bytes));

    let (after, _) = JustAnOption::deserialize(bytes.as_slice()).unwrap();
    assert_eq!(after.get_width(), 28);

    assert_eq!(before, after);
}

#[test]
fn mount_proto_export_list() {
    let mut before = exports::default();
    for i in 0..5 {
        let mut export = exportnode {
            ex_dir: format!("test_{i}").into(),
            ex_groups: Vec::new(),
        };
        for j in 0..5 {
            let group = groupnode {
                gr_name: format!("group_{j}").into(),
            };
            export.ex_groups.push(group);
        }
        before.inner.push(export);
    }
    assert_eq!(before.get_width(), 504);

    let mut bytes = vec![1; 1024];
    assert_eq!(504, before.serialize(&mut bytes));

    let (after, _) = exports::deserialize(bytes.as_slice()).unwrap();
    assert_eq!(after.get_width(), 504);

    assert_eq!(before, after);
}
