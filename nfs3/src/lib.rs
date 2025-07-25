// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

pub mod mount {
    include!(concat!(env!("OUT_DIR"), "/mount_proto.rs"));
}

pub mod nfs3 {
    include!(concat!(env!("OUT_DIR"), "/nfs3.rs"));
}
