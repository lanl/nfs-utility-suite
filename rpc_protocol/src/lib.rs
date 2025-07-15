// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

pub mod client;
pub mod rpcbind;
pub mod server;

use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;

include!(concat!(env!("OUT_DIR"), "/rpc_prot.rs"));

pub use rpc_prot::{
    AcceptedReply, AcceptedReplyBody, AuthFlavor, CallBody, OpaqueAuth, ReplyBody, RpcMessage,
    RpcMessageBody,
};

/// Only supported version of the RPC Protocol
const RPC_VERSION: u32 = 2;

/// The possible errors that can arise from trying to read or write an RPC call or reply.
#[derive(Debug)]
pub enum Error {
    Protocol(ProtocolError),
    Rpc(ReplyBody),
    Io(std::io::Error),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Protocol(e) => write!(f, "Protocol error: {e}"),
            Self::Rpc(e) => write!(f, "RPC error: {:?}", e),
            Self::Io(e) => write!(f, "IO error: {e}"),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

#[derive(Debug)]
pub enum ProtocolError {
    /// Generic decoding error:
    Decode,

    /// Received a fragmented message. TODO: once support for message fragments is included, this
    /// variant can be removed.
    MessageFragment,

    /// Message auth type is not supported by this library:
    UnsupportedAuth,

    /// Message's RPC Version was not 2 (only support version):
    WrongRpcVersion,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Decode => "Error decoding",
                Self::MessageFragment => "Recieved a fragmented message",
                Self::UnsupportedAuth => "Unsupported authorization mechanism",
                Self::WrongRpcVersion => "Only RPC Protocol version 2 is supported",
            }
        )
    }
}

/// Given a buffer that contains an encoded message, prefaced by a dummy record mark, update that
/// record mark based on the actual length of the message.
fn update_record_mark(buf: &mut Vec<u8>) {
    // size of message, not including the 4 bytes for the record mark itself:
    let message_size = u32::try_from(buf.len() - 4).unwrap();
    let record_mark: u32 = message_size | (1 << 31);
    buf[..4].copy_from_slice(&record_mark.to_be_bytes());
}

/// Reads 4 bytes from the given stream, and interprets them as a record mark.
///
/// If the record mark indicates that the record is fragmented, returns an error as this
/// implementation does not yet support record fragments.
///
/// Otherwise, returns the length of the message.
fn decode_record_mark(stream: &mut std::net::TcpStream) -> Result<u32, crate::Error> {
    let mut record_mark_bytes: [u8; 4] = [0; 4];

    stream.read_exact(&mut record_mark_bytes).inspect_err(|e| {
        if e.kind() != std::io::ErrorKind::UnexpectedEof {
            eprintln!("Error getting record mark: error reading from stream: {e}");
        }
    })?;

    let record_mark = u32::from_be_bytes(record_mark_bytes);

    if (record_mark & (1 << 31)) == 0 {
        return Err(Error::Protocol(ProtocolError::MessageFragment));
    }

    Ok(record_mark & (!(1 << 31)))
}

impl OpaqueAuth {
    fn none() -> Self {
        OpaqueAuth {
            flavor: AuthFlavor::None,
            body: Vec::new(),
        }
    }
}

/// Get a "unique" XID. TODO: make a real implementation for this function...
fn get_xid() -> u32 {
    17
}

/// Returns a buffer with space for a record mark already allocated, but a dummy value (0) encoded
/// since the length of the message isn't known yet.
fn buf_with_dummy_record_mark() -> Vec<u8> {
    vec![0, 0, 0, 0]
}
