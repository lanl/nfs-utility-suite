// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2026. Triad National Security, LLC.

//! A blocking RPC client and server that work on anything that implements `Read + Write`, built
//! on top of the I/O-free `rpc_protocol` crate.

pub mod client;
pub mod pipe;
pub mod server;

use std::fmt;
use std::io::Read;

use log::*;

use rpc_protocol::{
    ProtocolError, RecordAssembler, RecordMark, ReplyBody, DEFAULT_MAX_RECORD_SIZE,
    RECORD_MARK_SIZE,
};

/// The possible errors that can arise from trying to read or write an RPC call or reply.
#[derive(Debug)]
pub enum Error {
    /// Protocol errors are always returned by the RPC server implementation before
    /// even invoking procedure-specific code.
    Protocol(ProtocolError),

    /// Some RPC errors are returned by the server implementation (for example, unknown procedure),
    /// and some are returned by the procedure implementation (for example garbage args, or
    /// internal error like ENOMEM).
    ///
    // XXX: would it make sense to separate out the library-generated and user-generated errors
    // into separate variants?
    Rpc(ReplyBody),

    /// Errors returned by I/O failures.
    Io(std::io::Error),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Protocol(e) => write!(f, "Protocol error: {e}"),
            Self::Rpc(e) => write!(f, "RPC error: {e:?}"),
            Self::Io(e) => write!(f, "IO error: {e}"),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Reads one complete record from the given stream, reassembling it if it is fragmented.
fn read_record<S: Read>(stream: &mut S) -> Result<Vec<u8>, Error> {
    let mut assembler = RecordAssembler::new(DEFAULT_MAX_RECORD_SIZE);

    loop {
        let mut mark = [0; RECORD_MARK_SIZE];

        stream.read_exact(&mut mark).inspect_err(|e| {
            if e.kind() != std::io::ErrorKind::UnexpectedEof {
                eprintln!("Error getting record mark: error reading from stream: {e}");
            }
        })?;

        let mark = RecordMark::decode(&mark);

        let fragment = assembler.next_fragment(mark).map_err(Error::Protocol)?;
        stream
            .read_exact(fragment)
            .inspect_err(|e| warn!("Error reading message from stream: {e}"))?;

        if let Some(record) = assembler.take() {
            return Ok(record);
        }
    }
}
