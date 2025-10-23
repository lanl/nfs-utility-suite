// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

pub mod client;
pub mod rpcbind;
pub mod server;

use log::*;

use std::{
    fmt,
    io::{Read, Write},
};

include!(concat!(env!("OUT_DIR"), "/rpc_prot.rs"));

pub use rpc_prot::{
    AcceptedReply, AcceptedReplyBody, AuthFlavor, AuthStat, CallBody, OpaqueAuth, ProgMismatchBody,
    RejectedReply, ReplyBody, RpcMessage, RpcMessageBody,
};

/// Only supported version of the RPC Protocol
const RPC_VERSION: u32 = 2;

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
                Self::MessageFragment => "Received a fragmented message",
                Self::UnsupportedAuth => "Unsupported authorization mechanism",
                Self::WrongRpcVersion => "Only RPC Protocol version 2 is supported",
            }
        )
    }
}

/// A `call` holds the data needed to respond to an RPC call.
#[derive(Debug)]
pub struct Call<'a> {
    xid: u32,
    inner: CallBody,

    /// The call's encoded argument.
    pub arg: &'a [u8],
}

impl Call<'_> {
    /// Transaction ID of this call.
    pub fn get_xid(&self) -> u32 {
        self.xid
    }

    /// Program number, e.g., 10005 for NFS v3.
    pub fn get_program(&self) -> u32 {
        self.inner.prog
    }

    /// Version number, e.g., 3 for NFS v3.
    pub fn get_version(&self) -> u32 {
        self.inner.vers
    }

    /// Procedure number, e.g., 1 for GETATTR in NFS v3.
    pub fn get_procedure(&self) -> u32 {
        self.inner.proc
    }

    /// Credential
    pub fn get_credential(&self) -> &OpaqueAuth {
        &self.inner.cred
    }
}

/// Given an encoded RPC call in `data` (including both the call header and the encoded arguments),
/// tries to decode the call and returns either:
///
///   - Ok(_): the succesfully decoded call and a slice containing the argument
///   - Err(_): an error that occurred while trying to decode the call
///
/// The caller is expected to provide a complete RPC call record without the record mark prefix (if
/// present). If the caller is using a transport layer that uses record marking, like TCP, the
/// caller must handle decoding the record mark and reading a cmplete record. Passing a record that
/// is too short is returned as a decoding error.
pub fn decode_call(data: &[u8]) -> Result<Call<'_>, ProtocolError> {
    let mut message = RpcMessage::default();
    let mut rest = data;

    if let Err(e) = message.deserialize(&mut rest) {
        warn!("Error deserializing message: {e}");
        todo!();
    }

    let RpcMessageBody::Call(call) = message.body else {
        return Err(ProtocolError::Decode);
    };

    debug!(
        "recieved CALL for program {}, version {}, procedure {}, argument length {} bytes",
        call.prog,
        call.vers,
        call.proc,
        rest.len(),
    );

    if call.rpcvers != RPC_VERSION {
        debug!("CALL with unexpected RPC version: {}", call.rpcvers);
        return Err(ProtocolError::WrongRpcVersion);
    };

    Ok(Call {
        xid: message.xid,
        inner: call,
        arg: rest,
    })
}

/// Given a buffer that contains an encoded message, prefaced by a dummy record mark, update that
/// record mark based on the actual length of the message.
fn update_record_mark(buf: &mut [u8]) {
    // size of message, not including the 4 bytes for the record mark itself:
    let message_size = u32::try_from(buf.len() - 4).unwrap();
    let record_mark: u32 = message_size | (1 << 31);
    buf[..4].copy_from_slice(&record_mark.to_be_bytes());
}

/// Reads 4 bytes from the given stream, and interprets them as a record mark.
fn stream_record_mark<S: Read>(stream: &mut S) -> Result<u32, crate::Error> {
    let mut record_mark_bytes: [u8; 4] = [0; 4];

    stream.read_exact(&mut record_mark_bytes).inspect_err(|e| {
        if e.kind() != std::io::ErrorKind::UnexpectedEof {
            eprintln!("Error getting record mark: error reading from stream: {e}");
        }
    })?;

    decode_record_mark(&record_mark_bytes)
}

/// Returns the length indicated by the record mark.
///
/// If the record mark indicates that the record is fragmented, returns an error as this
/// implementation does not yet support record fragments.
///
/// Unlike the `stream_` variant, this can't return an I/O error.
fn decode_record_mark(mark: &[u8; 4]) -> Result<u32, crate::Error> {
    let record_mark = u32::from_be_bytes(*mark);

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

/// An "pipe", constructed using socketpair(2), that can be used for testing client and
/// server behavior.
pub mod pipe {
    use nix::sys::socket::{socketpair, AddressFamily, SockFlag, SockType};

    pub struct Endpoint {
        fd: std::os::fd::OwnedFd,
    }

    pub fn pipe() -> std::io::Result<(Endpoint, Endpoint)> {
        let (a, b) = socketpair(
            AddressFamily::Unix,
            SockType::Stream,
            None,
            SockFlag::empty(),
        )?;

        Ok((Endpoint { fd: a }, Endpoint { fd: b }))
    }

    impl std::io::Read for Endpoint {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            Ok(nix::unistd::read(&self.fd, buf)?)
        }
    }

    impl std::io::Write for Endpoint {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            Ok(nix::unistd::write(&self.fd, buf)?)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
}
