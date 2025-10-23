// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use clap::Parser;

use rpc_protocol::{server::ring::*, server::RpcResult, CallBody};

use ::nfs3::nfs3_xdr::{procedures::*, *};

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value_t = 2049)]
    port: u16,
}

struct ServerState {}

fn main() {
    env_logger::init();

    let args = Cli::parse();
    let address = format!("127.0.0.1:{}", args.port);

    let state = ServerState {};

    let procedures: Vec<Option<RingProcedure<ServerState>>> = vec![None, Some(getattr)];
    let procedure_map =
        ProcedureMap::new(NFS_PROGRAM, NFS_V3::VERSION, NFS_V3::VERSION, procedures);

    let mut server = RpcServer::new(&address, procedure_map, state).unwrap();

    server.main_loop().unwrap();
}

fn getattr(_call: &CallBody, arg: &[u8], _state: &mut ServerState) -> RingResult {
    eprintln!("in getattr impl: {arg:?}");

    let obj_attributes = FileAttributes::default();

    let result = GetAttrResult::Ok(GetAttrSuccess { obj_attributes });

    RingResult::Done(RpcResult::Success(result.serialize_alloc()))
}
