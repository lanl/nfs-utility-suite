// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2026. Triad National Security, LLC.

include!(concat!(env!("OUT_DIR"), "/optional.rs"));

use std::time::Duration;

use crate::optional::*;
use criterion::{criterion_group, criterion_main, Criterion};

struct Groupnode {
    name: String,
}

struct Exportnode {
    dirpath: String,
    groups: Vec<Groupnode>,
}

// Setup logic moved here to keep it out of the measurement loop
fn setup_test_data() -> Vec<u8> {
    let mut data: Vec<u8> = vec![];

    let mut export_groups: Vec<Exportnode> = Vec::new();
    for i in 0..128 {
        let mut export = Exportnode {
            dirpath: format!("test_{i}").into(),
            groups: Vec::new(),
        };
        for j in 0..128 {
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
        let padding = (4 - en.dirpath.len() % 4) % 4;
        data.extend(vec![0u8; padding]);
        for gn in en.groups.iter() {
            data.extend([0x0, 0x0, 0x0, 0x1u8]);
            data.extend((gn.name.len() as u32).to_be_bytes());
            data.extend(gn.name.as_bytes());
            let padding = (4 - gn.name.len() % 4) % 4;
            data.extend(vec![0u8; padding]);
        }
        data.extend([0x0, 0x0, 0x0, 0x0]);
    }
    data.extend([0x0, 0x0, 0x0, 0x0]);
    data
}

fn bench_exports_reader(_c: &mut Criterion) {
    // let data = setup_test_data();

    let mut c = Criterion::default()
        .warm_up_time(std::time::Duration::new(10, 0)) // 2 seconds warm-up
        .measurement_time(std::time::Duration::new(20, 0)); // 5 seconds measurement

    let mut g = c.benchmark_group("exports_ll_bench");

    g.bench_function("exports_reader_deserialization_old", move |b| {
        b.iter_batched(
            setup_test_data,
            move |data| {
                let mut en = exports::default();
                let _ = std::hint::black_box(en.deserialize(&mut data.as_slice()));
            },
            criterion::BatchSize::LargeInput,
        );
    });

    g.bench_function("exports_reader_deserialization_zcopy", move |b| {
        b.iter_batched(
            setup_test_data,
            move |data| {
                let reader = exportsReader::new(std::hint::black_box(data.as_slice())).unwrap();

                for en in reader.get_inner() {
                    let en = en.unwrap();
                    let _ = std::hint::black_box(en.get_ex_dir());

                    let groups_reader = en.get_ex_groups();
                    for gn in groups_reader {
                        let _ = std::hint::black_box(gn.unwrap().get_gr_name());
                    }
                }
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

criterion_group!(benches, bench_exports_reader);
criterion_main!(benches);
