// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2026. Triad National Security, LLC.

use std::io::{Read, Write};

use log::*;

use rpc_protocol::server::*;

use crate::{read_record, Error};

/// A trait that allows functions to be generic over both TcpListener and UnixListener.
pub trait Listener<S> {
    fn accept(&self) -> std::io::Result<S>;
}

impl Listener<std::net::TcpStream> for std::net::TcpListener {
    fn accept(&self) -> std::io::Result<std::net::TcpStream> {
        Ok(self.accept()?.0)
    }
}

impl Listener<std::os::unix::net::UnixStream> for std::os::unix::net::UnixListener {
    fn accept(&self) -> std::io::Result<std::os::unix::net::UnixStream> {
        Ok(self.accept()?.0)
    }
}

/// Run a blocking TCP server for the given RPC service using the given Listener.
pub fn run_blocking_tcp_server<T, S: Read + Write>(
    program: &mut RpcProgram<T>,
    listener: impl Listener<S>,
) {
    loop {
        match listener.accept() {
            Ok(stream) => {
                let _ = handle_connection(program, stream);
            }
            Err(e) => warn!("Error accepting connection: {e}"),
        }
    }
}

/// Tries to handle a given stream by reading a series of RPC Call messages from it, and
/// passing those calls off to the given RPC service to handle. A call that the service rejects
/// gets an error reply, and the connection stays open. If a message can't be decoded as a call, or
/// if an I/O error is encountered, the function returns and the stream is dropped.
pub fn handle_connection<T, S: Read + Write>(
    program: &mut RpcProgram<T>,
    mut stream: S,
) -> Result<(), Error> {
    loop {
        let record = read_record(&mut stream)?;
        trace!("got message of length: {}", record.len());

        let reply = program.handle_call(&record).map_err(Error::Protocol)?;

        // If only part of the reply could be written, then the client can no longer tell where the
        // next reply starts, so give up on the connection:
        stream.write_all(&reply)?;
    }
}
