// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2026. Triad National Security, LLC.

use std::{cmp::min, collections::BTreeMap};

use crate::codegen::ser_layout::IntOp;

use super::*;

#[derive(Clone, Debug, PartialEq)]
struct CopySide {
    name: String,
    size: usize,
    block: usize,
    block_off: usize,
    ops: Vec<IntOp>,
}

#[derive(Clone, Debug, PartialEq)]
struct StaticBlockInfo {
    elems: Vec<CopySide>,
    off_info: OffInfo,
}

#[derive(Clone, Debug, PartialEq)]
struct OffInfo {
    known: usize,
    deps: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct CStringInfo {
    name: String,
    block: usize,
    off_info: OffInfo,
}

#[derive(Clone, Debug, PartialEq)]
enum BlockInfo {
    Static(StaticBlockInfo),
    CString(CStringInfo),
}

impl ValidatedStruct {
    fn src_deps(layout: &ser_layout::SerLayout) -> (Vec<String>, BTreeMap<usize, BlockInfo>) {
        let mut total_off = 0usize;

        let mut src_block = 0usize;
        let mut src_block_off = 0usize;

        let mut src_block_map = std::collections::BTreeMap::<usize, BlockInfo>::new();
        let mut src_dep_names: Vec<String> = vec![];

        for val in layout.members.iter() {
            let (v, src_width) = match val {
                ser_layout::LayoutData::NativeU32(i) => (i, 4usize),
                ser_layout::LayoutData::NativeI32(i) => (i, 4usize),
                ser_layout::LayoutData::NativeU64(i) => (i, 8usize),
                ser_layout::LayoutData::NativeI64(i) => (i, 8usize),
                ser_layout::LayoutData::CString(name) => {
                    src_block += 1;
                    src_block_off = 0;

                    src_block_map.entry(src_block).or_insert_with(|| {
                        let ret = BlockInfo::CString(CStringInfo {
                            name: name.clone(),
                            block: src_block,
                            off_info: OffInfo {
                                known: total_off,
                                deps: src_dep_names.clone(),
                            },
                        });

                        src_dep_names.push(name.clone());

                        ret
                    });

                    src_block += 1;
                    continue;
                }
                ser_layout::LayoutData::Unused(s) => {
                    let inc = src_block_off + s;

                    src_block += inc / 64;
                    src_block_off = inc % 64;

                    continue;
                }
            };

            let inc = src_block_off + src_width;

            for i in v {
                if !src_block_off.is_multiple_of(src_width) {
                    panic!("we do not support non-type-aligned types ({})!", i.name);
                }

                let copy_side = CopySide {
                    name: i.name.clone(),
                    size: src_width,
                    block: src_block,
                    block_off: src_block_off,
                    ops: i.ops.clone(),
                };

                let entry = src_block_map.entry(src_block).or_insert_with(|| {
                    let ret = BlockInfo::Static(StaticBlockInfo {
                        elems: Vec::new(),
                        off_info: OffInfo {
                            known: total_off,
                            deps: src_dep_names.clone(),
                        },
                    });
                    total_off += 64;
                    ret
                });

                let BlockInfo::Static(s) = entry else {
                    panic!("expected static block");
                };

                s.elems.push(copy_side);
            }

            src_block += inc / 64;
            src_block_off = inc % 64;
        }

        (src_dep_names, src_block_map)
    }

    fn dst_deps(
        &self,
        tab: &ValidatedSymbolTable,
        src_dep_names: &[String],
    ) -> BTreeMap<usize, BlockInfo> {
        let mut dst_block_map = std::collections::BTreeMap::<usize, BlockInfo>::new();
        let mut dst_block = 0usize;
        let mut dst_block_off = 0usize;
        let mut total_off = 0;
        for (member, _) in self.members.iter() {
            let Some(size) = member.size(tab) else {
                todo!("we currently do not support variable length destination variables");
            };

            let copy_side = CopySide {
                name: member.name.clone(),
                size,
                block: dst_block,
                block_off: dst_block_off,
                ops: vec![],
            };

            let entry = dst_block_map.entry(dst_block).or_insert_with(|| {
                let ret = BlockInfo::Static(StaticBlockInfo {
                    elems: Vec::new(),
                    off_info: OffInfo {
                        known: total_off,
                        deps: src_dep_names.to_vec(),
                    },
                });
                total_off += 64;
                ret
            });

            let BlockInfo::Static(s) = entry else {
                panic!("expected static block");
            };

            s.elems.push(copy_side);

            let inc = dst_block_off + size;
            dst_block += inc / 64;
            dst_block_off = inc % 64;
        }

        dst_block_map
    }

    pub(super) fn serialize_vectorized_definition(
        &self,
        buf: &mut CodeBuf,
        layout: &ser_layout::SerLayout,
        tab: &ValidatedSymbolTable,
    ) {
        buf.add_line("#[target_feature(enable = \"avx512f,avx512bw\")]");
        buf.code_block(
            "pub unsafe fn serialize_vectorized(input: &[u8], out: &mut Vec<u8>)",
            |buf| {
                let mut total = 0usize;
                let mut unused = 0usize;
                for val in layout.members.iter() {
                    match val {
                        ser_layout::LayoutData::NativeU32(_) => total += 4,
                        ser_layout::LayoutData::NativeI32(_) => total += 4,
                        ser_layout::LayoutData::NativeU64(_) => total += 8,
                        ser_layout::LayoutData::NativeI64(_) => total += 8,
                        ser_layout::LayoutData::CString(_) => todo!(),
                        ser_layout::LayoutData::Unused(size) => {
                            total += size;
                            unused += size
                        }
                    }
                }

                buf.add_line(&format!("// min read: {}", total));
                buf.add_line(&format!("// unused read: {}", unused));

                let (src_dep_names, src_block_map) = ValidatedStruct::src_deps(layout);
                let dst_block_map = self.dst_deps(tab, &src_dep_names);


                buf.add_line("use core::arch::x86_64::*;");
                buf.add_line("let in_ptr: *const i32 = input.as_ptr().cast::<i32>();");
                buf.add_line("let out_ptr: *mut i32 = out.as_mut_ptr().cast::<i32>();");

                for (block_number, block_info) in dst_block_map.iter() {
                    match block_info {
                        BlockInfo::Static(_) => {
                            buf.add_line(&format!("let out_block_{block_number} = _mm512_set1_epi64(0);"));
                            buf.add_line(&format!("let out_k_{block_number} = 0u16;"));
                        }
                        BlockInfo::CString(_) => todo!(),
                    }
                }

                for (block_number, block_info) in src_block_map.iter() {
                    let mut copy_mappings = Vec::new();
                    match block_info {
                        BlockInfo::Static(info) => {
                            for elem in info.elems.iter() {
                                let val = dst_block_map
                                    .values()
                                    .flat_map(|v| match v {
                                        BlockInfo::Static(s) => Some(s.elems.iter()),
                                        BlockInfo::CString(_) => None,
                                    })
                                    .flatten()
                                    .find(|v2| v2.name == elem.name);

                                if let Some(val) = val {
                                    copy_mappings.push((elem.clone(), val.clone()));
                                } else {
                                    buf.add_line(&format!("// warning: {} not found", elem.name));
                                }
                            }

                            buf.add_line(&format!(
                                "/*block {}: {:#?}*/",
                                block_number, copy_mappings
                            ));

                            if copy_mappings.is_empty() {
                                continue;
                            }

                            // INITIAL LOAD
                            let bytes_to_read = copy_mappings
                                .iter()
                                .max_by(|(src_a, _), (src_b, _)| {
                                    (src_a.block_off + src_a.size).cmp(&(src_b.block_off + src_b.size))
                                })
                                .map(|(src, _)| src.block_off + src.size)
                                .unwrap();

                            assert!(bytes_to_read.is_multiple_of(4));

                            let bytes_to_read = bytes_to_read / 4;

                            buf.add_line(&format!(
                                "let block_mask_{} = ((1usize << {}) - 1) as u16;",
                                block_number, bytes_to_read
                            ));

                            buf.add_line(&format!("let off = {}{};", info.off_info.known / 4, info.off_info.deps.iter().map(|dep| format!(" + {dep}_width")).collect::<Vec<String>>().join("")));
                            buf.add_line(&format!("let block_{block_number} = _mm512_maskz_loadu_epi32(block_mask_{block_number}, in_ptr.offset(off));"));

                            let mut pre_op_srcs: std::collections::HashMap<usize, (usize, Vec<IntOp>)> = std::collections::HashMap::new();
                            for (src, dst) in copy_mappings.iter() {
                                let needs_pre = dst.size == 8 && !dst.block_off.is_multiple_of(8) && !src.ops.is_empty();
                                if !needs_pre {
                                    continue;
                                }

                                let sharers = copy_mappings.iter().filter(|(s, _)| s.block_off == src.block_off).count();
                                assert_eq!(
                                    sharers, 1,
                                    "source field '{}' at block_off {} in block {} has ops and an unaligned 8-byte destination ('{}'), but is copied to multiple destinations -- not supported",
                                    src.name, src.block_off, block_number, dst.name
                                );

                                if let Some((_, existing_ops)) = pre_op_srcs.get(&src.block_off) {
                                    assert_eq!(existing_ops, &src.ops, "conflicting ops recorded for the same source lane {}", src.block_off);
                                } else {
                                    pre_op_srcs.insert(src.block_off, (src.size, src.ops.clone()));
                                }
                            }

                            for (src_off, (src_size, ops)) in pre_op_srcs.iter() {
                                assert_eq!(*src_size, 8, "pre-permute op path is only expected for 8-byte source fields");
                                let item = src_off / 4;
                                assert!(item % 2 == 0, "internal error: unaligned 8-byte source lane at dword {item}");
                                let lane = item / 2;

                                for op in ops.iter() {
                                    match op {
                                        IntOp::LShift(cnt) => {
                                            let mut shifts = [0i64; 8];
                                            shifts[lane] = *cnt as i64;
                                            buf.add_line(&format!("let lshift_mask = _mm512_set_epi64({});", shifts.iter().rev().map(|v| v.to_string()).collect::<Vec<String>>().join(", ")));
                                            buf.add_line(&format!("let block_{block_number} = _mm512_sllv_epi64(block_{block_number}, lshift_mask);"));
                                        },
                                        IntOp::RShift(cnt) => {
                                            let mut shifts = [0i64; 8];
                                            shifts[lane] = *cnt as i64;
                                            buf.add_line(&format!("let rshift_mask = _mm512_set_epi64({});", shifts.iter().rev().map(|v| v.to_string()).collect::<Vec<String>>().join(", ")));
                                            buf.add_line(&format!("let block_{block_number} = _mm512_srlv_epi64(block_{block_number}, rshift_mask);"));
                                        },
                                        IntOp::LUT32(lut) => {
                                            buf.add_line(&format!("let lut_table = _mm512_set_epi32({});", lut.iter().rev().map(|v| v.to_string()).collect::<Vec<String>>().join(", ")));
                                            buf.add_line(&format!("let lut_mask = {}u16;", 1 << item));
                                            buf.add_line(&format!("let block_{block_number} = _mm512_mask_permutexvar(block_{block_number}, lut_mask, block_{block_number}, lut_table);"));
                                        },
                                        IntOp::And(mask) => {
                                            let mut and_mask = [-1i64; 8];
                                            and_mask[lane] = *mask as i64;
                                            buf.add_line(&format!("let and_mask = _mm512_set_epi64({});", and_mask.iter().rev().map(|v| v.to_string()).collect::<Vec<String>>().join(", ")));
                                            buf.add_line(&format!("let block_{block_number} = _mm512_and_si512(block_{block_number}, and_mask);"));
                                        },
                                    }
                                }
                            }

                            let unique_destinations: HashSet<usize> = copy_mappings.iter().map(|(_, v)| v.block).collect();

                            for destination in unique_destinations {
                                let relevant_mappings: Vec<(CopySide, CopySide)> = copy_mappings.iter().filter(|(_, v)| v.block == destination).map(|(src, dst)| (src.clone(), dst.clone())).collect();

                                // sanity check: no two mappings into this destination
                                // block should target the same destination lane.
                                {
                                    let mut seen_dst_offs = std::collections::HashSet::new();
                                    for (_, dst) in relevant_mappings.iter() {
                                        assert!(
                                            seen_dst_offs.insert(dst.block_off),
                                            "multiple mappings write to the same destination offset {} in block {}",
                                            dst.block_off, destination
                                        );
                                    }
                                }

                                // PERMUTE dwords
                                let mut perm_mask = [0u32; 16];
                                let mut k = 0u16;
                                for (src, dst) in relevant_mappings.iter() {
                                    let dst_idx = dst.block_off / 4;
                                    let src_idx = src.block_off / 4;
                                    buf.add_line(&format!("// perm mapping {} -> {}", src_idx, dst_idx));
                                    let n = min(src.size / 4, dst.size / 4);
                                    assert!(n == 1 || n == 2);

                                    perm_mask[dst_idx] = src_idx as u32;
                                    k |= 1u16 << dst_idx;
                                    if n == 2 {
                                        perm_mask[dst_idx + 1] = (src_idx + 1) as u32;
                                        k |= 1u16 << (dst_idx + 1);
                                    }
                                }

                                buf.add_line(&format!("let k_{}_to_{} = {}u16;", block_number, destination, k));
                                buf.add_line(&format!("let buf_swap_{}_to_{} = _mm512_set_epi32({});", block_number, destination, perm_mask.iter().rev().map(|v| v.to_string()).collect::<Vec<String>>().join(", ")));
                                buf.add_line(&format!("let buf_{block_number}_to_{destination} = _mm512_maskz_permutexvar_epi32(k_{block_number}_to_{destination}, buf_swap_{block_number}_to_{destination}, block_{block_number});"));

                                // OPS 
                                for (src, dst) in relevant_mappings.iter() {
                                    let handled_pre_permute = dst.size == 8 && !dst.block_off.is_multiple_of(8) && !src.ops.is_empty();
                                    if handled_pre_permute {
                                        continue;
                                    }

                                    let item = dst.block_off / 4;
                                    let words = dst.size / 4;
                                    assert!(words == 1 || words == 2);
                                    for op in src.ops.iter() {
                                        match op {
                                            IntOp::LShift(cnt) => {
                                                if words == 1 {
                                                    let mut shifts = [0i32; 16];
                                                    shifts[item] = *cnt as i32;

                                                    buf.add_line(&format!("let lshift_mask = _mm512_set_epi32({});", shifts.iter().rev().map(|v| v.to_string()).collect::<Vec<String>>().join(", ")));
                                                    buf.add_line(&format!("let buf_{block_number}_to_{destination} = _mm512_sllv_epi32(buf_{block_number}_to_{destination}, lshift_mask);"));
                                                } else {
                                                    assert!(item % 2 == 0, "internal error: unaligned 64-bit destination lane at dword {item} reached post-permute op path");
                                                    let lane = item / 2;
                                                    let mut shifts = [0i64; 8];
                                                    shifts[lane] = *cnt as i64;

                                                    buf.add_line(&format!("let lshift_mask = _mm512_set_epi64({});", shifts.iter().rev().map(|v| v.to_string()).collect::<Vec<String>>().join(", ")));
                                                    buf.add_line(&format!("let buf_{block_number}_to_{destination} = _mm512_sllv_epi64(buf_{block_number}_to_{destination}, lshift_mask);"));
                                                }
                                            },
                                            IntOp::RShift(cnt) => {
                                                if words == 1 {
                                                    let mut shifts = [0i32; 16];
                                                    shifts[item] = *cnt as i32;

                                                    buf.add_line(&format!("let rshift_mask = _mm512_set_epi32({});", shifts.iter().rev().map(|v| v.to_string()).collect::<Vec<String>>().join(", ")));
                                                    buf.add_line(&format!("let buf_{block_number}_to_{destination} = _mm512_srlv_epi32(buf_{block_number}_to_{destination}, rshift_mask);"));
                                                } else {
                                                    assert!(item % 2 == 0, "internal error: unaligned 64-bit destination lane at dword {item} reached post-permute op path");
                                                    let lane = item / 2;
                                                    let mut shifts = [0i64; 8];
                                                    shifts[lane] = *cnt as i64;

                                                    buf.add_line(&format!("let rshift_mask = _mm512_set_epi64({});", shifts.iter().rev().map(|v| v.to_string()).collect::<Vec<String>>().join(", ")));
                                                    buf.add_line(&format!("let buf_{block_number}_to_{destination} = _mm512_srlv_epi64(buf_{block_number}_to_{destination}, rshift_mask);"));
                                                }
                                            },
                                            IntOp::LUT32(lut) => {
                                                buf.add_line(&format!("let lut_table = _mm512_set_epi32({});", lut.iter().rev().map(|v| v.to_string()).collect::<Vec<String>>().join(", ")));
                                                buf.add_line(&format!("let lut_mask = {}u16;", 1 << item));
                                                buf.add_line(&format!("let buf_{block_number}_to_{destination} = _mm512_mask_permutexvar_epi32(buf_{block_number}_to_{destination}, lut_mask, buf_{block_number}_to_{destination}, lut_table);"));
                                            },
                                            IntOp::And(mask) => {
                                                if words == 2 {
                                                    assert!(item % 2 == 0, "internal error: unaligned 64-bit destination lane at dword {item} reached post-permute op path");
                                                    let lane = item / 2;
                                                    let mut and_mask = [-1i64; 8];
                                                    and_mask[lane] = *mask as i64;

                                                    buf.add_line(&format!("let and_mask = _mm512_set_epi64({});", and_mask.iter().rev().map(|v| v.to_string()).collect::<Vec<String>>().join(", ")));
                                                } else {
                                                    let mut and_mask = [-1i32; 16];
                                                    and_mask[item] = *mask as i32;

                                                    buf.add_line(&format!("let and_mask = _mm512_set_epi32({});", and_mask.iter().rev().map(|v| v.to_string()).collect::<Vec<String>>().join(", ")));
                                                }
                                                buf.add_line(&format!("let buf_{block_number}_to_{destination} = _mm512_and_si512(buf_{block_number}_to_{destination}, and_mask);"));
                                            },
                                        }
                                    }
                                }

                                // swap dword order
                                let mut needs_dword_swap = false;
                                let mut swap_idx = [0u32; 16];
                                for (i, item) in swap_idx.iter_mut().enumerate() { *item = i as u32; }
                                for (_, dst) in relevant_mappings.iter() {
                                    if dst.size / 4 == 2 {
                                        let dst_idx = dst.block_off / 4;
                                        swap_idx[dst_idx] = (dst_idx + 1) as u32;
                                        swap_idx[dst_idx + 1] = dst_idx as u32;
                                        needs_dword_swap = true;
                                    }
                                }
                                if needs_dword_swap {
                                    buf.add_line(&format!("let dword_swap_{}_to_{} = _mm512_set_epi32({});", block_number, destination, swap_idx.iter().rev().map(|v| v.to_string()).collect::<Vec<String>>().join(", ")));
                                    buf.add_line(&format!("let buf_{block_number}_to_{destination} = _mm512_permutexvar_epi32(dword_swap_{block_number}_to_{destination}, buf_{block_number}_to_{destination});"));
                                }

                                // BYTESWAP
                                let mut shuf_mask = [-1i8; 64];
                                for (_, dst) in relevant_mappings.iter() {
                                    let base = dst.block_off;
                                    let nwords = dst.size / 4;
                                    for w in 0..nwords {
                                        let chunk_base = base + w * 4;
                                        for i in 0..4 {
                                            let src_pos = chunk_base + 4 - 1 - i;
                                            shuf_mask[chunk_base + i] = (src_pos % 16) as i8;
                                        }
                                    }
                                }
                                buf.add_line(&format!("let buf_bswap_{block_number}_to_{destination} = _mm512_set_epi8("));
                                for i in 0..4 {
                                    let i = 3 - i;
                                    let line = shuf_mask[(i * 16)..(i + 1) * 16].iter().rev().map(|v| format!("{}", v)).collect::<Vec<String>>().join(", ");
                                    buf.add_line(&format!("    {},", line));
                                }
                                buf.add_line(");");
                                buf.add_line(&format!("let buf_{block_number}_to_{destination} = _mm512_shuffle_epi8(buf_{block_number}_to_{destination}, buf_bswap_{block_number}_to_{destination});"));

                                buf.add_line(&format!("let out_block_{destination} = _mm512_or_si512(out_block_{destination}, buf_{block_number}_to_{destination});"));
                                buf.add_line(&format!("let out_k_{destination} = out_k_{destination} | k_{block_number}_to_{destination};"));
                            }
                        }
                        BlockInfo::CString(_) => todo!(),
                    }
                }


                for (block_number, block_info) in dst_block_map.iter() {
                    match block_info {
                        BlockInfo::Static(_) => {
                            buf.add_line(&format!("_mm512_mask_storeu_epi32(out_ptr.offset({}), out_k_{block_number}, out_block_{block_number});", block_number * 16));
                        }
                        BlockInfo::CString(_) => todo!(),
                    }
                }
            },
        );
    }
}
