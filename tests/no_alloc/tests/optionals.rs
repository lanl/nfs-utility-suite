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

    let mut bytes = vec![1; 44];
    assert_eq!(44, before.serialize(&mut bytes));
    let mut after = ListBegin::default();
    ListBegin::deserialize(&mut after, &mut bytes.as_slice()).unwrap();
    assert_eq!(before, after);
}

#[test]
fn non_recursive_optional_none() {
    let before = JustAnOption { maybe: None };

    let mut bytes = vec![1; 4];
    assert_eq!(4, before.serialize(&mut bytes));
    let mut after = JustAnOption::default();
    JustAnOption::deserialize(&mut after, &mut bytes.as_slice()).unwrap();
    assert_eq!(before, after);
}

#[test]
fn non_recursive_optional_some() {
    let before = JustAnOption {
        maybe: Some(NonRecursive { stuff: 49 }),
    };

    let mut bytes = vec![1; 8];
    assert_eq!(8, before.serialize(&mut bytes));
    let mut after = JustAnOption::default();
    JustAnOption::deserialize(&mut after, &mut bytes.as_slice()).unwrap();
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

    let mut bytes = vec![1; 1024];
    assert_eq!(504, before.serialize(&mut bytes));
    let mut after = exports::default();
    exports::deserialize(&mut after, &mut bytes.as_slice()).unwrap();
    assert_eq!(before, after);
}
