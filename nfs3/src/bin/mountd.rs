// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::net::TcpListener;

use rpc_protocol::server::*;
use rpc_protocol::CallBody;

use nfs3::mount::mount_proto::procedures::*;
use nfs3::mount::mount_proto::*;

struct MountState {
    exports: Exports,
}

impl MountState {
    fn new() -> Self {
        Self {
            exports: Exports {
                inner: vec![ExportNode {
                    dir: "/test/nfs/export".into(),
                    groups: Groups {
                        inner: vec![GroupNode {
                            name: "localhost".into(),
                        }],
                    },
                }],
            },
        }
    }
}

fn main() {
    let procedures: Vec<Option<RpcProcedure<MountState>>> = vec![
        None,
        None, // mount
        None, // dump
        None, // umount
        None, // umountall
        Some(export),
    ];

    let state = MountState::new();
    let mut server = RpcService::new(MOUNT_PROGRAM, MOUNT_V3::VERSION, procedures, state);

    let listener = TcpListener::bind("0.0.0.0:20048").unwrap();
    server.run_blocking_tcp_server(listener);
}

fn export(_call: &CallBody, _arg: &[u8], state: &mut MountState) -> RpcResult {
    RpcResult::Success(state.exports.serialize_alloc())
}
