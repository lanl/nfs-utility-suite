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
    let mut after = OpaqueArrays::default();
    OpaqueArrays::deserialize(&mut after, &mut bytes.as_slice()).unwrap();

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
    let mut after = IntArrays::default();
    IntArrays::deserialize(&mut after, &mut bytes.as_slice()).unwrap();

    assert_eq!(arr, after);
}

#[test]
fn strings() {
    let mut before = Strings::default();
    before.str = "hello!!".into();
    before.str_2 = "world".into();
    let bytes = before.serialize_alloc();
    let mut after = Strings::default();
    Strings::deserialize(&mut after, &mut bytes.as_slice()).unwrap();

    assert_eq!(before, after);
}

#[test]
fn many_strings() {
    let mut before = ManyStrings::default();
    for i in 0..4 {
        let mut s = Strings::default();
        s.str = format!("str {i}.1").into();
        s.str_2 = format!("str {i}.2").into();
        before.many[i] = s;
    }
    let bytes = before.serialize_alloc();
    let mut after = ManyStrings::default();
    ManyStrings::deserialize(&mut after, &mut bytes.as_slice()).unwrap();

    assert_eq!(before, after);
}

#[test]
fn identifier_array() {
    let mut before = IdentifierArray::default();
    for i in 0..AMOUNT {
        before.bytes[i as usize] = i as u8;
        before.ints.push(std::i32::MAX - i as i32);
    }
    before.str = "hello".into();
    let bytes = before.serialize_alloc();
    let mut after = IdentifierArray::default();
    IdentifierArray::deserialize(&mut after, &mut bytes.as_slice()).unwrap();

    assert_eq!(before, after);
}

#[test]
fn many_ints() {
    let mut before = ManyInts::default();
    before.first[0] = std::u64::MAX - 1;
    before.first[1] = 1;
    for i in 0..7 {
        before.second.push(i as i32);
        before.third.push(std::i64::MAX - i as i64);
    }
    let bytes = before.serialize_alloc();
    let mut after = ManyInts::default();
    ManyInts::deserialize(&mut after, &mut bytes.as_slice()).unwrap();

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
    let mut after = hello::Hello::default();
    hello::Hello::deserialize(&mut after, &mut bytes.as_slice()).unwrap();

    assert_eq!(before.abc, after.abc);
    assert_eq!(before.def, after.def);
    assert_eq!(before.favorite_fruit, after.favorite_fruit);

    assert_eq!(A_CONSTANT, 12345);
    assert_eq!(ANOTHER, 15);
}

include!(concat!(env!("OUT_DIR"), "/optional.rs"));
use optional::*;

#[test]
fn optional() {
    let mut head = ListBegin::default();
    for i in 0..5 {
        let mut node = ListNode::default();
        node.data = i;
        head.list.push(node);
    }

    let bytes = head.serialize_alloc();
    let mut after = ListBegin::default();
    ListBegin::deserialize(&mut after, &mut bytes.as_slice()).unwrap();
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
                x: -2199023255535,
                y: 70368744177664,
            },
        },
        b: 9897654,
        no: false,
        yes: true,
    };

    let bytes = before.serialize_alloc();

    let mut after = structs::Foo::default();
    structs::Foo::deserialize(&mut after, &mut bytes.as_slice()).unwrap();

    assert_eq!(before.a, after.a);
    assert_eq!(before.b, after.b);
    assert_eq!(before.blah.a, after.blah.a);
    assert_eq!(before.blah.b, after.blah.b);
    assert_eq!(before.blah.one.x, after.blah.one.x);
    assert_eq!(before.blah.one.y, after.blah.one.y);
    assert_eq!(before.yes, after.yes);
    assert_eq!(before.no, after.no);
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
    let mut after = File::default();
    File::deserialize(&mut after, &mut bytes.as_slice()).unwrap();

    assert_eq!(before, after);
}

include!(concat!(env!("OUT_DIR"), "/unions.rs"));
use unions::*;

#[test]
fn test_simple_union() {
    let plant: PlantKind = PlantKind::Flower;

    let plant_bytes = plant.serialize_alloc();

    let mut plant_after = PlantKind::Tree;
    plant_after
        .deserialize(&mut plant_bytes.as_slice())
        .unwrap();

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

    let mut not_a_plant_after = NumLeaves { inner: Some(17) };
    let mut is_a_plant_after = NumLeaves { inner: None };

    not_a_plant_after
        .deserialize(&mut not_a_plant_bytes.as_slice())
        .unwrap();
    is_a_plant_after
        .deserialize(&mut is_a_plant_bytes.as_slice())
        .unwrap();

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

    let mut after_some = MaybeAPlantKind { inner: None };
    let mut after_none = MaybeAPlantKind {
        inner: Some(PlantKind::Tree),
    };

    after_some
        .deserialize(&mut before_some_bytes.as_slice())
        .unwrap();
    after_none
        .deserialize(&mut before_none_bytes.as_slice())
        .unwrap();

    assert_eq!(before_some.inner, after_some.inner);
    assert_eq!(before_none.inner, after_none.inner);
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

    let mut after_some = MaybeStuff { inner: None };
    let mut after_none = MaybeStuff {
        inner: Some(Stuff { a: 3, b: 4 }),
    };

    after_some
        .deserialize(&mut before_some_bytes.as_slice())
        .unwrap();
    after_none
        .deserialize(&mut before_none_bytes.as_slice())
        .unwrap();

    assert_eq!(before_some.inner, after_some.inner);
    assert_eq!(before_none.inner, after_none.inner);
}

#[test]
fn test_enum_union() {
    let plants = vec![Plant::Tree(1), Plant::Grass(2147483647), Plant::Flower(0)];

    let plants_bytes = plants.iter().map(|plant| plant.serialize_alloc());

    let plants_after = plants_bytes.map(|bytes| {
        let mut after = Plant::Tree(7);
        Plant::deserialize(&mut after, &mut bytes.as_slice()).unwrap();
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
        let mut after = StuffOrPlant::default();
        StuffOrPlant::deserialize(&mut after, &mut b.as_slice()).unwrap();
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

    let mut after = exports::default();

    exports::deserialize(&mut after, &mut bytes.as_slice()).unwrap();
    assert_eq!(exports, after);
}
