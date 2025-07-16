// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::net::TcpListener;

use rpc_protocol::rpcbind;
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

    let handle = std::thread::spawn(|| {
        let state = MountState::new();
        let mut server = RpcService::new(
            MOUNT_PROGRAM,
            MOUNT_V3::VERSION,
            MOUNT_V3::VERSION,
            procedures,
            state,
        );

        let listener = TcpListener::bind("0.0.0.0:20048").unwrap();
        server.run_blocking_tcp_server(listener);
    });

    if let Err(e) = announce_self() {
        eprintln!("Could not set mountd address in RPCBIND server: {e}");
        return;
    }

    let _ = handle.join();
}

fn export(_call: &CallBody, _arg: &[u8], state: &mut MountState) -> RpcResult {
    RpcResult::Success(state.exports.serialize_alloc())
}

/// Tell the RPCBIND server that the mount service is now running:
fn announce_self() -> Result<(), rpc_protocol::Error> {
    let service = rpcbind::RpcService {
        prog: MOUNT_PROGRAM,
        vers: MOUNT_V3::VERSION,
        netid: "tcp".into(),
        addr: "0.0.0.0.78.80".into(),
        owner: "superuser".into(),
    };

    rpcbind::client::set(
        service,
        rpcbind::RpcbindServerAddress::Tcp("0.0.0.0:111".to_string()),
    )?;

    Ok(())
}
