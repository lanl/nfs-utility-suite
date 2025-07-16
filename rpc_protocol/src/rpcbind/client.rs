// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use log::*;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::unix::net::UnixStream;

use crate::client::*;
use crate::rpcbind::{self, procedures::*, RpcbindServerAddress};
use crate::*;

/// Try to call the SET RPC for the RPCBIND server listening at `address`, to add `new_service` to
/// its service list.
pub fn set(
    new_service: rpcbind::RpcService,
    server_address: RpcbindServerAddress,
) -> Result<bool, crate::Error> {
    debug!("performing RPCBIND Set call");

    match server_address {
        RpcbindServerAddress::Unix(addr) => {
            let mut stream = UnixStream::connect(addr)?;
            set_using_stream(new_service, &mut stream)
        }
        RpcbindServerAddress::Tcp(addr) => {
            let mut stream = TcpStream::connect(addr)?;
            set_using_stream(new_service, &mut stream)
        }
    }
}

pub fn set_using_stream<S: Read + Write>(
    new_service: rpcbind::RpcService,
    stream: &mut S,
) -> Result<bool, crate::Error> {
    let arg = new_service.serialize_alloc();

    let res = do_rpc_call(
        stream,
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

pub fn getaddr_using_stream<S: Read + Write>(
    service: rpcbind::RpcService,
    stream: &mut S,
) -> Result<std::ffi::OsString, crate::Error> {
    let arg = service.serialize_alloc();

    let res = do_rpc_call(
        stream,
        RPCBPROG,
        RPCBVERS::VERSION,
        RPCBVERS::RPCBPROC_GETADDR,
        arg.as_slice(),
    )?;

    let mut addr = rpcbind::RpcbString::default();
    match addr.deserialize(&mut res.as_slice()) {
        Ok(_) => Ok(addr.contents),
        Err(_) => Err(Error::Protocol(ProtocolError::Decode)),
    }
}
