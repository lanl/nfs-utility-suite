// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::net::TcpStream;

use clap::Parser;

include!(concat!(env!("OUT_DIR"), "/rpcbind.rs"));

use rpc_blocking::client::*;
use rpc_protocol::{CallBody, OpaqueAuth, RPC_VERSION};

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "localhost")]
    hostname: String,

    #[arg(long, default_value_t = 111)]
    port: u16,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();
    let server_address = format!("{}:{}", args.hostname, args.port);
    let mut stream = TcpStream::connect(&server_address)?;

    let res = do_rpc_call(
        &mut stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: rpcbind::procedures::RPCBPROG,
            vers: rpcbind::procedures::RPCBVERS::VERSION,
            proc: rpcbind::procedures::RPCBVERS::RPCBPROC_DUMP,
            cred: OpaqueAuth::none(),
            verf: OpaqueAuth::none(),
        },
        &[0u8; 0],
    )?;

    let (list, _) = rpcbind::RpcbindList::deserialize(res.as_slice())?;

    print_rpcblist(list);

    Ok(())
}

fn print_rpcblist(list: rpcbind::RpcbindList) {
    for map in list.items.iter() {
        let map = &map.rpcb_map;
        println!(
            "{} {} {:?} {:?} {:?}",
            map.prog, map.vers, map.netid, map.addr, map.owner
        );
    }
}
