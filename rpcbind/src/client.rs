// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use log::*;

use std::{
    io::{Read, Write},
    net::TcpStream,
    os::unix::net::UnixStream,
};

use crate::{procedures::*, RpcbindServerAddress, *};
use rpc_blocking::{client::do_rpc_call, Error};
use rpc_protocol::*;

/// Try to call the SET RPC for the RPCBIND server listening at `address`, to add `new_service` to
/// its service list.
pub fn set(
    new_service: rpcbind::RpcService,
    server_address: RpcbindServerAddress,
) -> Result<bool, rpc_blocking::Error> {
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
) -> Result<bool, rpc_blocking::Error> {
    let arg = new_service.serialize_alloc();

    let res = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: RPCBPROG,
            vers: RPCBVERS::VERSION,
            proc: RPCBVERS::RPCBPROC_SET,
            cred: OpaqueAuth::none(),
            verf: OpaqueAuth::none(),
        },
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
) -> Result<std::ffi::OsString, rpc_blocking::Error> {
    let arg = service.serialize_alloc();

    let res = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: RPCBPROG,
            vers: RPCBVERS::VERSION,
            proc: RPCBVERS::RPCBPROC_GETADDR,
            cred: OpaqueAuth::none(),
            verf: OpaqueAuth::none(),
        },
        arg.as_slice(),
    )?;

    let Ok((addr, _)) = rpcbind::RpcbString::deserialize(res.as_slice()) else {
        return Err(Error::Protocol(ProtocolError::Decode));
    };

    Ok(addr.contents)
}
