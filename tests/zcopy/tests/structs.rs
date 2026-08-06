// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

include!(concat!(env!("OUT_DIR"), "/structs.rs"));

use crate::structs::*;
use xdr_lib::Reader;

#[test]
fn test_structs_basic() {
    #[rustfmt::skip]
    let data: Vec<u8> = vec![
        0xC0, 0xFF, 0xEE, 0x11, // int Foo.a
        0x11, 0xEE, 0xFF, 0xC0, // unsigned int Foo.blah.a
        0x00, 0x00, 0x00, 0x01, // Val Foo.blah.one.val
        0xBA, 0xDD, 0xF0, 0x00,
        0xDD, 0xC0, 0xFF, 0xEE, // hyper foo.blah.one.x
        0xDD, 0xC0, 0xFF, 0xEE,
        0xBA, 0xDD, 0xF0, 0x00, // unsigned hyper foo.blah.one.y
        0xFA, 0xDF, 0xFD, 0xAF, // int Foo.blah.b
        0x01, 0x10, 0x01, 0x10, // unsigned int Foo.b
        0x00, 0x00, 0x00, 0x00, // bool Foo.no = false
        0x00, 0x80, 0x00, 0x00  // bool Foo.yes = true
    ];

    let reader = structs::FooReader::new(data.as_slice()).unwrap();
    assert_eq!(reader.get_a(), i32::from_be_bytes([0xC0, 0xFF, 0xEE, 0x11]));

    let bar_reader = reader.get_blah();
    {
        assert_eq!(bar_reader.get_a(), 0x11EEFFC0);

        let another_reader = bar_reader.get_one();
        {
            assert_eq!(another_reader.get_val(), Val::one);
            assert_eq!(
                another_reader.get_x(),
                i64::from_be_bytes([0xBA, 0xDD, 0xF0, 0x00, 0xDD, 0xC0, 0xFF, 0xEE])
            );
            assert_eq!(another_reader.get_y(), 0xDDC0FFEEBADDF000);
            assert_eq!(another_reader.get_width().unwrap(), 20);
        }

        assert_eq!(
            bar_reader.get_b(),
            i32::from_be_bytes([0xFA, 0xDF, 0xFD, 0xAF])
        );
        assert_eq!(bar_reader.get_width().unwrap(), 28);
    }

    assert_eq!(reader.get_no(), false);
    assert_eq!(reader.get_yes(), true);
    assert_eq!(reader.get_width().unwrap(), 44);
}

#[test]
fn test_structs_one_byte_short() {
    #[rustfmt::skip]
    let data: Vec<u8> = vec![
        0xC0, 0xFF, 0xEE, 0x11, // int Foo.a
        0x11, 0xEE, 0xFF, 0xC0, // unsigned int Foo.blah.a
        0x00, 0x00, 0x00, 0x01, // Val Foo.blah.one.val
        0xBA, 0xDD, 0xF0, 0x00,
        0xDD, 0xC0, 0xFF, 0xEE, // hyper foo.blah.one.x
        0xDD, 0xC0, 0xFF, 0xEE,
        0xBA, 0xDD, 0xF0, 0x00, // unsigned hyper foo.blah.one.y
        0xFA, 0xDF, 0xFD, 0xAF, // int Foo.blah.b
        0x01, 0x10, 0x01, 0x10, // unsigned int Foo.b
        0x00, 0x00, 0x00, 0x00, // bool Foo.no = false
        0x00, 0x80, 0x00,       // bool Foo.yes = invalid
    ];

    assert!(structs::FooReader::new(data.as_slice()).is_err());

    if std::hint::black_box(false) {
        let a = vec![0u8; 4];
        let mut b = vec![0u8; 4];
        unsafe {
            structs::FileAttributes::serialize_vectorized(
                std::hint::black_box(&a),
                std::hint::black_box(&mut b),
            );
        }
    }
}

#[test]
fn test_structs_statx_vectorized() {
    if std::is_x86_feature_detected!("avx512f") && std::is_x86_feature_detected!("avx512bw") {
        unsafe {
            let mut st: libc::statx = std::mem::zeroed();

            st.stx_nlink = 0x1111_1111;
            st.stx_uid = 0x2222_2222;
            st.stx_gid = 0x3333_3333;
            st.stx_mode = 0x4444; // u16
            st.stx_ino = 0x5555_5555_6666_6666; // u64
            st.stx_size = 0x7777_7777_8888_8888; // u64
            st.stx_blocks = 0x9999_9999_AAAA_AAAA; // u64

            st.stx_atime.tv_sec = 0x1234_5678_9ABC_DEF0_i64;
            st.stx_atime.tv_nsec = 0xAAAA_BBBB;
            st.stx_ctime.tv_sec = 0x0F1E_2D3C_4B5A_6978_i64;
            st.stx_ctime.tv_nsec = 0xCCCC_DDDD;

            st.stx_rdev_major = 0xDEAD_0001;
            st.stx_rdev_minor = 0xDEAD_0002;
            st.stx_dev_major = 0xDEAD_0003;
            st.stx_dev_minor = 0xDEAD_0004;

            let in_buf: &[u8] = std::slice::from_raw_parts(
                (&st as *const libc::statx) as *const u8,
                std::mem::size_of::<libc::statx>(),
            );

            let mut out_buf: Vec<u8> = vec![0u8; 128];

            FileAttributes::serialize_vectorized(in_buf, &mut out_buf);

            let mut fattr = FileAttributes::default();
            fattr.deserialize(&mut out_buf.as_slice()).unwrap();

            assert_eq!(fattr.typ, FileType::Dir);

            assert_eq!(fattr.mode, 0x0444);

            assert_eq!(fattr.nlink, 0x1111_1111);
            assert_eq!(fattr.uid, 0x2222_2222);
            assert_eq!(fattr.gid, 0x3333_3333);
            assert_eq!(fattr.size, 0x7777_7777_8888_8888);

            assert_eq!(fattr.used, 0x9999_9999_AAAA_AAAA << 9);

            assert_eq!(fattr.rdev_1, 0xDEAD_0001);
            assert_eq!(fattr.rdev_2, 0xDEAD_0002);
            assert_eq!(fattr.fsid_major, 0xDEAD_0003);
            assert_eq!(fattr.fsid_minor, 0xDEAD_0004);
            assert_eq!(fattr.fileid, 0x5555_5555_6666_6666);

            assert_eq!(fattr.atime_s, 0x9ABC_DEF0);
            assert_eq!(fattr.atime_ns, 0xAAAA_BBBB);

            assert_eq!(fattr.mtime_s, 0);
            assert_eq!(fattr.mtime_ns, 0);

            assert_eq!(fattr.ctime_s, 0x4B5A_6978);
            assert_eq!(fattr.ctime_ns, 0xCCCC_DDDD);
        }
    }
}
