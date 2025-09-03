// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use rpc_protocol::server::ring::*;
use rpc_protocol::server::RpcResult;
use rpc_protocol::CallBody;

use nfs3::nfs3_xdr::nfs3::{procedures::*, *};

struct ServerState {}

fn main() {
    env_logger::init();

    let procedures: Vec<Option<RingProcedure<ServerState>>> = vec![None, Some(getattr)];

    let state = ServerState {};

    let procedure_map =
        ProcedureMap::new(NFS_PROGRAM, NFS_V3::VERSION, NFS_V3::VERSION, procedures);

    let mut server = RpcServer::new("127.0.0.1:2049", procedure_map, state).unwrap();

    server.main_loop().unwrap();
}

fn getattr(_call: &CallBody, arg: &[u8], _state: &mut ServerState) -> RingResult {
    eprintln!("in getattr impl: {arg:?}");

    let obj_attributes = FileAttributes::default();

    let result = GetAttrResult::Ok(GetAttrSuccess { obj_attributes });

    RingResult::Done(RpcResult::Success(result.serialize_alloc()))
}
