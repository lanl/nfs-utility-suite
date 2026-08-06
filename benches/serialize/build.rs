use xdr_codegen::codegen::ser_layout::*;

fn main() {
    xdr_codegen::Compiler::new()
        .file("../../tests/input/optional.x")
        .file("../../tests/input/structs.x")
        .enable_no_alloc()
        .enable_zcopy()
        .add_ser_layout(SerLayout {
            name: "Statx".into(),
            maps_to: "FileAttributes".into(),
            padding: PaddingMode::Total(256), // sizeof(statx) = 256
            members: vec![
                LayoutData::Unused(4), // __u32 stx_mask;        /* Mask of bits indicating filled fields */
                LayoutData::Unused(4), // __u32 stx_blksize;     /* Block size for filesystem I/O */
                LayoutData::Unused(8), // __u64 stx_attributes;  /* Extra file attribute indicators */
                LayoutData::NativeU32(vec![IntInfo {
                    name: "nlink".into(),
                    ops: vec![],
                }]), // __u32 stx_nlink;       /* Number of hard links */
                LayoutData::NativeU32(vec![IntInfo {
                    name: "uid".into(),
                    ops: vec![],
                }]), // __u32 stx_uid;         /* User ID of owner */
                LayoutData::NativeU32(vec![IntInfo {
                    name: "gid".into(),
                    ops: vec![],
                }]), // __u32 stx_gid;         /* Group ID of owner */
                LayoutData::NativeU32(vec![
                    IntInfo {
                        name: "mode".into(),
                        ops: vec![IntOp::And(0xFFF)],
                    },
                    IntInfo {
                        name: "typ".into(),
                        ops: vec![
                            IntOp::RShift(12),
                            IntOp::LUT32([
                                0, 7, // 0o001 => 7,
                                4, // 0o002 => 4,
                                0, 2, // 0o004 => 2,
                                0, 3, // 0o006 => 3
                                0, 1, // 0o010 => 1
                                0, 5, // 0o012 => 5
                                0, 6, // 0o014 => 6
                                0, 0, 0,
                            ]),
                        ],
                    },
                ]), // __u16 stx_mode;        /* File type and mode */
                LayoutData::NativeU64(vec![IntInfo {
                    name: "fileid".into(),
                    ops: vec![],
                }]), // __u64 stx_ino;         /* Inode number */
                LayoutData::NativeU64(vec![IntInfo {
                    name: "size".into(),
                    ops: vec![],
                }]), // __u64 stx_size;        /* Total size in bytes */
                LayoutData::NativeU64(vec![IntInfo {
                    name: "used".into(),
                    ops: vec![IntOp::LShift(9)],
                }]), // __u64 stx_blocks;      /* Number of 512B blocks allocated */
                LayoutData::Unused(8), // __u64 stx_attributes_mask;
                //                        /* Mask to show what's supported
                //                           in stx_attributes */

                // /* The following fields are file timestamps */
                // struct statx_timestamp stx_atime;  /* Last access */
                LayoutData::NativeI64(vec![IntInfo {
                    name: "atime_s".into(),
                    ops: vec![],
                }]),
                LayoutData::NativeU32(vec![IntInfo {
                    name: "atime_ns".into(),
                    ops: vec![],
                }]),
                LayoutData::Unused(4),
                // struct statx_timestamp stx_btime;  /* Creation */
                LayoutData::NativeI64(vec![IntInfo {
                    name: "btime_s".into(),
                    ops: vec![],
                }]),
                LayoutData::NativeU32(vec![IntInfo {
                    name: "btime_ns".into(),
                    ops: vec![],
                }]),
                LayoutData::Unused(4),
                // struct statx_timestamp stx_ctime;  /* Last status change */
                LayoutData::NativeI64(vec![IntInfo {
                    name: "ctime_s".into(),
                    ops: vec![],
                }]),
                LayoutData::NativeU32(vec![IntInfo {
                    name: "ctime_ns".into(),
                    ops: vec![],
                }]),
                LayoutData::Unused(4),
                // struct statx_timestamp stx_mtime;  /* Last modification */
                LayoutData::Unused(16),
                // /* If this file represents a device, then the next two
                //    fields contain the ID of the device */
                // __u32 stx_rdev_major;  /* Major ID */
                LayoutData::NativeU32(vec![IntInfo {
                    name: "rdev_1".into(),
                    ops: vec![],
                }]),
                // __u32 stx_rdev_minor;  /* Minor ID */
                LayoutData::NativeU32(vec![IntInfo {
                    name: "rdev_2".into(),
                    ops: vec![],
                }]),
                // /* The next two fields contain the ID of the device
                //    containing the filesystem where the file resides */
                // __u32 stx_dev_major;   /* Major ID */
                // __u32 stx_dev_minor;   /* Minor ID */
                LayoutData::NativeU32(vec![IntInfo {
                    name: "fsid_major".into(),
                    ops: vec![],
                }]),
                LayoutData::NativeU32(vec![IntInfo {
                    name: "fsid_minor".into(),
                    ops: vec![],
                }]),
                // __u64 stx_mnt_id;      /* Mount ID */

                // /* Direct I/O alignment restrictions */
                // __u32 stx_dio_mem_align;
                // __u32 stx_dio_offset_align;

                // /* Direct I/O atomic write limits */
                // __u32 stx_atomic_write_unit_min;
                // __u32 stx_atomic_write_unit_max;
                // __u32 stx_atomic_write_segments_max;
            ],
        })
        .run()
        .expect("That should have worked. :(");
}
