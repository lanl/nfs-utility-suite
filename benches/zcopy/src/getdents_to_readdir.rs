// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2026. Triad National Security, LLC.

include!(concat!(env!("OUT_DIR"), "/optional.rs"));

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

use rand::RngExt;

use crate::optional::Dirlist;

/// Generate synthetic getdents64-like buffer exactly as returned by the syscall.
///
/// The linux_dirent64 structure has this memory layout:
/// ```c
/// struct linux_dirent64 {
///     __u64        d_ino;      /* 64-bit inode number */
///     __s64        d_off;      /* 64-bit offset to next dirent */
///     unsigned short d_reclen; /* Size of this dirent (including name + padding) */
///     unsigned char  d_type;   /* File type */
///     char           d_name[]; /* Null-terminated filename, 0-padded to alignment */
/// };
/// ```
///
/// The buffer is tightly packed with no gaps between entries. Each entry's d_reclen
/// tells you the distance to the next entry in bytes. The name is null-terminated
/// and then zero-padded to align the next entry to 8 bytes (on x86_64).
fn generate_getdents64_buffer(num_entries: usize, avg_name_len: usize) -> Vec<u8> {
    let mut rng = rand::rng();
    let mut buffer = Vec::new();

    for i in 0..num_entries {
        // Generate filename
        let name_len = (avg_name_len / 2) + rng.random_range(0..avg_name_len);
        let name: Vec<u8> = (0..name_len)
            .map(|_| {
                let charset = b"abcdefghijklmnopqrstuvwxyz0123456789_";
                charset[rng.random_range(0..charset.len())]
            })
            .collect();

        // Build entry: ino (8) + off (8) + reclen (2) + d_type (1) + name + null + padding
        let mut entry = Vec::new();

        // d_ino (8 bytes, u64 little-endian on x86_64)
        let ino = rng.random::<u64>();
        entry.extend_from_slice(&ino.to_le_bytes());

        // d_off (8 bytes, i64 little-endian)
        let off = (i as i64) * 512;
        entry.extend_from_slice(&off.to_le_bytes());

        // d_reclen placeholder (2 bytes) - will fill in after calculating size
        let reclen_offset = entry.len();
        entry.extend_from_slice(&[0u8; 2]);

        // d_type (1 byte)
        entry.push(if rng.random_bool(0.8) { 8 } else { 4 }); // 8=REG, 4=DIR

        // d_name (null-terminated)
        entry.extend_from_slice(&name);
        entry.push(0u8); // null terminator

        // Align to 8-byte boundary (standard on x86_64)
        let reclen = entry.len().div_ceil(8) * 8;
        entry.resize(reclen, 0u8);

        // Write the actual d_reclen value (little-endian)
        let reclen_u16 = reclen as u16;
        entry[reclen_offset] = (reclen_u16 & 0xFF) as u8;
        entry[reclen_offset + 1] = ((reclen_u16 >> 8) & 0xFF) as u8;

        buffer.extend_from_slice(&entry);
    }

    buffer
}

/// Parse getdents64 buffer and convert to XDR readdir entries.
///
/// This simulates what an NFS server would do: receive getdents64 output and
/// convert it to XDR entry3 structures for transmission. Uses d_reclen to
/// iterate through the tightly-packed buffer.
#[inline(never)]
fn getdents64_to_xdr_readdir(buffer: &[u8]) -> Vec<optional::Entry> {
    let mut entries = Vec::new();
    let mut offset = 0;
    let mut cookie = 1u64;

    while offset < buffer.len() {
        if offset + 26 > buffer.len() {
            break; // Not enough space for minimal entry
        }

        // Parse fixed fields (little-endian on x86_64)
        let ino = u64::from_le_bytes(buffer[offset..offset + 8].try_into().unwrap());

        let _off = i64::from_le_bytes(buffer[offset + 8..offset + 16].try_into().unwrap());

        let reclen = u16::from_le_bytes([buffer[offset + 16], buffer[offset + 17]]) as usize;
        let _d_type = buffer[offset + 18];

        // Extract null-terminated name starting after d_type
        let name_start = offset + 19;
        let mut name_end = name_start;
        while name_end < buffer.len() && buffer[name_end] != 0 {
            name_end += 1;
        }

        let name = String::from_utf8_lossy(&buffer[name_start..name_end]).to_string();

        entries.push(optional::Entry {
            fileid: ino,
            name: name.into(),
            cookie,
        });
        cookie += 1;

        offset += reclen;
    }

    entries
}

/// Serialize XDR readdir entries to XDR format.
/// Each entry becomes: [discriminator=1][fileid][name_len][name][padding][cookie]
#[inline(always)]
fn entries_to_xdr_bytes(entries: Vec<optional::Entry>) -> Vec<u8> {
    let dirlist = Dirlist { entries, eof: true };

    let mut buf = vec![0; dirlist.get_width()];

    let written = dirlist.serialize(&mut buf);

    assert_eq!(written, dirlist.get_width());

    buf
}

fn bench_small_readdir(c: &mut Criterion) {
    c.bench_function("getdents64_to_xdr_small_32_entries", |b| {
        b.iter_batched(
            || generate_getdents64_buffer(32, 16),
            |buffer| {
                let entries = getdents64_to_xdr_readdir(&buffer);
                let _ = black_box(entries_to_xdr_bytes(entries));
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

#[inline(never)]
fn bench_medium_readdir(c: &mut Criterion) {
    c.bench_function("getdents64_to_xdr_medium_256_entries", |b| {
        b.iter_batched(
            || generate_getdents64_buffer(256, 32),
            |buffer| {
                let entries = getdents64_to_xdr_readdir(&buffer);
                let _ = black_box(entries_to_xdr_bytes(entries));
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

fn bench_large_readdir(c: &mut Criterion) {
    c.bench_function("getdents64_to_xdr_large_1024_entries", |b| {
        b.iter_batched(
            || generate_getdents64_buffer(1024, 64),
            |buffer| {
                let entries = getdents64_to_xdr_readdir(&buffer);
                let _ = black_box(entries_to_xdr_bytes(entries));
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

criterion_group!(
    benches,
    bench_small_readdir,
    bench_medium_readdir,
    bench_large_readdir,
);

criterion_main!(benches);
