include!(concat!(env!("OUT_DIR"), "/structs.rs"));

use std::mem;

use crate::structs::*;
use criterion::*;
use rand::Rng;
use rand::RngExt;
use xdr_lib::Reader;

use std::alloc::{alloc, dealloc, Layout};
use std::slice;
fn generate_random_statx() -> libc::statx {
    let mut st: libc::statx = unsafe { mem::zeroed() };

    let mut rng = rand::rng();

    let valid_mode_bits = [0o004];

    let mut rand_byte = [0u8; 1];
    rng.fill(&mut rand_byte);
    let rand_type_idx = (rand_byte[0] as usize) % valid_mode_bits.len();

    let type_shifted = valid_mode_bits[rand_type_idx] << 12;

    let mut perm_bytes = [0u8; 2];
    rng.fill(&mut perm_bytes);
    let permissions = u16::from_ne_bytes(perm_bytes) & 0x0FFF;

    st.stx_mode = type_shifted | permissions;

    st
}

fn bench_statx_serialization(c: &mut Criterion) {
    let st = generate_random_statx();

    let in_buf: &[u8] = unsafe {
        std::slice::from_raw_parts(
            (&st as *const libc::statx) as *const u8,
            std::mem::size_of::<libc::statx>(),
        )
    };

    let in_buf = in_buf.to_vec();
    let in_buf = in_buf.as_slice();

    let mut temp_out = vec![0u8; 128];
    unsafe { FileAttributes::serialize_vectorized(in_buf, &mut temp_out) };

    let mut fattr = FileAttributes::default();
    fattr.deserialize(&mut temp_out.as_slice()).unwrap();

    let mut group = c.benchmark_group("Statx_Serialization");

    use std::hint::black_box;
    group.bench_function("serialize_vectorized_with_fixup", |b| {
        let len = 1024;
        let align = 4096;

        let layout = Layout::from_size_align(len, align).unwrap();
        let ptr = unsafe { alloc(layout) };

        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }

        unsafe {
            std::ptr::write_bytes(ptr, 0, len);
        }

        let mut out_buf: Vec<u8> = unsafe { Vec::from_raw_parts(ptr, len, len) };

        b.iter(|| {
            let input = black_box(in_buf);
            let out = black_box(&mut out_buf);

            unsafe {
                FileAttributes::serialize_vectorized(input, out);
            }

            black_box(out);
        })
    });

    group.bench_function("serialize_alloc", |b| {
        let mut out_buf: Vec<u8> = vec![0u8; 128];
        b.iter(|| {
            fattr.typ = match st.stx_mode >> 12 {
                0o010 => FileType::Reg,
                0o004 => FileType::Dir,
                0o012 => FileType::Lnk,
                0o002 => FileType::Chr,
                0o006 => FileType::Blk,
                0o001 => FileType::Fifo,
                0o014 => FileType::Sock,
                _ => panic!(
                    "Setup failed: invalid mode bits generated: {}",
                    st.stx_mode >> 12
                ),
            };
            fattr.mode = (st.stx_mode & 0xFFF) as u32;
            fattr.nlink = st.stx_nlink;
            fattr.uid = st.stx_uid;
            fattr.gid = st.stx_gid;
            fattr.size = st.stx_size;
            fattr.used = st.stx_blocks * 512;
            fattr.rdev_1 = st.stx_rdev_major;
            fattr.rdev_2 = st.stx_rdev_minor;
            fattr.fileid = st.stx_ino;
            fattr.atime_s = st.stx_atime.tv_sec as u32;
            fattr.atime_ns = st.stx_atime.tv_nsec;
            fattr.mtime_s = st.stx_mtime.tv_sec as u32;
            fattr.mtime_ns = st.stx_mtime.tv_nsec;
            fattr.ctime_s = st.stx_ctime.tv_sec as u32;
            fattr.ctime_ns = st.stx_ctime.tv_nsec;

            black_box(fattr.serialize(out_buf.as_mut_slice()));
        })
    });

    group.finish();
}

criterion_group!(benches, bench_statx_serialization);
criterion_main!(benches);
