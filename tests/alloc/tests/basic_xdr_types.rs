// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

include!(concat!(env!("OUT_DIR"), "/arrays.rs"));
use arrays::*;

#[test]
fn opaque_arrays() {
    let mut arr = OpaqueArrays::default();
    for i in 0..3 {
        arr.bytes[i] = 7 + (i as u8);
        arr.bytes_2.push(255 - (i as u8));
        arr.bytes_3.push(i as u8);
    }
    let bytes = arr.serialize_alloc();
    let (after, _) = OpaqueArrays::deserialize(bytes.as_slice()).unwrap();

    assert_eq!(arr, after);
}

#[test]
fn arrays_of_struct() {
    let mut arr = IntArrays::default();
    for i in 0..4 {
        arr.fixed[i] = AnInt { a: i as u32 };
    }
    for i in 0..7 {
        arr.limited.push(AnInt {
            a: u32::MAX - i as u32,
        });
    }
    for i in 0..512 {
        arr.unlimited.push(AnInt {
            a: u32::MAX - i as u32,
        });
    }
    let bytes = arr.serialize_alloc();
    let (after, _) = IntArrays::deserialize(bytes.as_slice()).unwrap();

    assert_eq!(arr, after);
}

#[test]
fn strings() {
    let before = Strings {
        str: "hello!!".into(),
        str_2: "world".into(),
    };
    let bytes = before.serialize_alloc();
    let (after, _) = Strings::deserialize(bytes.as_slice()).unwrap();

    assert_eq!(before, after);
}

#[test]
fn many_strings() {
    let mut before = ManyStrings::default();
    for i in 0..4 {
        before.many[i] = Strings {
            str: format!("str {i}.1").into(),
            str_2: format!("str {i}.2").into(),
        };
    }
    let bytes = before.serialize_alloc();
    let (after, _) = ManyStrings::deserialize(bytes.as_slice()).unwrap();

    assert_eq!(before, after);
}

#[test]
fn identifier_array() {
    let mut before = IdentifierArray::default();
    for i in 0..AMOUNT {
        before.bytes[i as usize] = i as u8;
        before.ints.push(i32::MAX - i as i32);
    }
    before.str = "hello".into();
    let bytes = before.serialize_alloc();
    let (after, _) = IdentifierArray::deserialize(bytes.as_slice()).unwrap();

    assert_eq!(before, after);
}

#[test]
fn many_ints() {
    let mut before = ManyInts::default();
    before.first[0] = u64::MAX - 1;
    before.first[1] = 1;
    for i in 0..7 {
        before.second.push(i);
        before.third.push(i64::MAX - i as i64);
    }
    let bytes = before.serialize_alloc();
    let (after, _) = ManyInts::deserialize(bytes.as_slice()).unwrap();

    assert_eq!(before, after);
}
include!(concat!(env!("OUT_DIR"), "/hello.rs"));

#[test]
fn test_hello() {
    use hello::*;

    let before = hello::Hello {
        abc: 32,
        def: -798,
        favorite_fruit: hello::Fruit::StarFruit,
    };
    let bytes = before.serialize_alloc();
    let (after, _) = hello::Hello::deserialize(bytes.as_slice()).unwrap();

    assert_eq!(before, after);

    assert_eq!(A_CONSTANT, 12345);
    assert_eq!(ANOTHER, 15);
}

include!(concat!(env!("OUT_DIR"), "/optional.rs"));
use optional::*;

#[test]
fn optional() {
    let mut head = ListBegin::default();
    for i in 0..5 {
        head.list.push(ListNode { data: i });
    }

    let bytes = head.serialize_alloc();
    let (after, _) = ListBegin::deserialize(bytes.as_slice()).unwrap();
    assert_eq!(head, after);
}

include!(concat!(env!("OUT_DIR"), "/structs.rs"));
#[test]
fn test_struct() {
    let before = structs::Foo {
        a: -1234567,
        blah: structs::Bar {
            a: 17,
            b: -489,
            one: structs::Another {
                val: structs::Val::two,
                x: -2199023255535,
                y: 70368744177664,
            },
        },
        b: 9897654,
        no: false,
        yes: true,
    };

    let bytes = before.serialize_alloc();

    let (after, _) = structs::Foo::deserialize(bytes.as_slice()).unwrap();

    assert_eq!(before, after);
}

include!(concat!(env!("OUT_DIR"), "/typedef.rs"));
use typedef::*;

#[test]
fn typedef() {
    let before = File {
        owner: 10,
        name: "my_file".into(),
        contents: vec![1, 2, 3],
        t: TimestampsData {
            atime: 123,
            ctime: 234,
            mtime: 345,
        },
    };

    let bytes = before.serialize_alloc();
    let (after, _) = File::deserialize(bytes.as_slice()).unwrap();

    assert_eq!(before, after);
}

include!(concat!(env!("OUT_DIR"), "/unions.rs"));
use unions::*;

#[test]
fn test_simple_union() {
    let plant: PlantKind = PlantKind::Flower;

    let plant_bytes = plant.serialize_alloc();

    let (plant_after, _) = PlantKind::deserialize(plant_bytes.as_slice()).unwrap();

    assert_eq!(plant, plant_after);
}

#[test]
fn test_bool_union_contains_int() {
    let not_a_plant: NumLeaves = NumLeaves { inner: None };
    let is_a_plant: NumLeaves = NumLeaves {
        inner: Some(1234985940),
    };

    let not_a_plant_bytes = not_a_plant.serialize_alloc();
    let is_a_plant_bytes = is_a_plant.serialize_alloc();

    let (not_a_plant_after, _) = NumLeaves::deserialize(not_a_plant_bytes.as_slice()).unwrap();
    let (is_a_plant_after, _) = NumLeaves::deserialize(is_a_plant_bytes.as_slice()).unwrap();

    assert_eq!(not_a_plant.inner, not_a_plant_after.inner);
    assert_eq!(is_a_plant.inner, is_a_plant_after.inner);
}

#[test]
fn test_bool_union_contains_enum() {
    let before_some = MaybeAPlantKind {
        inner: Some(PlantKind::Grass),
    };
    let before_none = MaybeAPlantKind { inner: None };

    let before_some_bytes = before_some.serialize_alloc();
    let before_none_bytes = before_none.serialize_alloc();

    let (after_some, _) = MaybeAPlantKind::deserialize(before_some_bytes.as_slice()).unwrap();

    let (after_none, _) = MaybeAPlantKind::deserialize(before_none_bytes.as_slice()).unwrap();

    assert_eq!(before_some, after_some);
    assert_eq!(before_none, after_none);
}

#[test]
fn test_bool_union_contains_struct() {
    let before_some = MaybeStuff {
        inner: Some(Stuff {
            a: 234734589,
            b: 21334782345794,
        }),
    };
    let before_none = MaybeStuff { inner: None };
    let before_some_bytes = before_some.serialize_alloc();
    let before_none_bytes = before_none.serialize_alloc();

    let (after_some, _) = MaybeStuff::deserialize(before_some_bytes.as_slice()).unwrap();

    let (after_none, _) = MaybeStuff::deserialize(before_none_bytes.as_slice()).unwrap();

    assert_eq!(before_some.inner, after_some.inner);
    assert_eq!(before_none.inner, after_none.inner);
}

#[test]
fn test_enum_union() {
    let plants = vec![Plant::Tree(1), Plant::Grass(2147483647), Plant::Flower(0)];

    let plants_bytes = plants.iter().map(|plant| plant.serialize_alloc());

    let plants_after = plants_bytes.map(|bytes| {
        let (after, _) = Plant::deserialize(bytes.as_slice()).unwrap();
        after
    });

    std::iter::zip(&plants, plants_after).for_each(|(before, after)| {
        assert_eq!(*before, after);
    });
}

#[test]
fn test_enum_union_with_compound_arms() {
    let inputs = vec![
        StuffOrPlant::one(Stuff { a: 23, b: 49 }),
        StuffOrPlant::two(PlantKind::Grass),
        StuffOrPlant::three(Plant::Flower(2938483)),
    ];

    let bytes = inputs.iter().map(|i| i.serialize_alloc());
    let outputs = bytes.map(|b| {
        let (after, _) = StuffOrPlant::deserialize(b.as_slice()).unwrap();
        after
    });

    std::iter::zip(&inputs, outputs).for_each(|(before, after)| {
        assert_eq!(*before, after);
    });
}

#[test]
fn mount_proto_multiple_optionals() {
    use std::ffi::OsString;

    let group = groupnode {
        gr_name: OsString::from("0.0.0.0/0"),
    };

    let export = exportnode {
        ex_dir: OsString::from("/test"),
        ex_groups: vec![group],
    };

    let exports = exports {
        inner: vec![export],
    };

    let bytes = exports.serialize_alloc();

    let (after, _) = exports::deserialize(bytes.as_slice()).unwrap();
    assert_eq!(exports, after);
}
