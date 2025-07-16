// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use log::*;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::unix::net::UnixStream;

use crate::client::*;

use super::rpcbind;
use super::rpcbind::procedures::*;

/// An RPCBIND Server tends to listen both on a Unix socket and a TCP socket.
pub enum RpcbindServerAddress {
    Unix(String),
    Tcp(String),
}

/// Try to call the SET RPC for the RPCBIND server listening at `address`, to add `new_service` to
/// its service list.
pub fn set(
    new_service: rpcbind::RpcService,
    server_address: RpcbindServerAddress,
) -> Result<bool, crate::Error> {
    debug!("performing RPCBIND Set call");

    match server_address {
        RpcbindServerAddress::Unix(addr) => {
            let stream = UnixStream::connect(addr)?;
            set_using_stream(new_service, stream)
        }
        RpcbindServerAddress::Tcp(addr) => {
            let stream = TcpStream::connect(addr)?;
            set_using_stream(new_service, stream)
        }
    }
}

fn set_using_stream<S: Read + Write>(
    new_service: rpcbind::RpcService,
    mut stream: S,
) -> Result<bool, crate::Error> {
    let arg = new_service.serialize_alloc();

    let res = do_rpc_call(
        &mut stream,
        RPCBPROG,
        RPCBVERS::VERSION,
        RPCBVERS::RPCBPROC_SET,
        arg.as_slice(),
    )?;

    match res.as_slice() {
        &[0, 0, 0, 0] => Ok(false),
        _ => Ok(true),
    }
}
