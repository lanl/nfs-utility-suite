// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

pub mod client;
pub mod server;

use log::*;

use std::fmt;

include!(concat!(env!("OUT_DIR"), "/rpc_prot.rs"));

pub use rpc_prot::{
    AcceptedReply, AcceptedReplyBody, AuthFlavor, AuthStat, CallBody, OpaqueAuth, ProgMismatchBody,
    RejectedReply, ReplyBody, RpcMessage, RpcMessageBody,
};

use crate::rpc_prot::authsys_parms;

/// Only supported version of the RPC Protocol
pub const RPC_VERSION: u32 = 2;

/// The possible errors that can arise from trying to decode an RPC call or reply.
#[derive(Debug)]
pub enum ProtocolError {
    /// Generic decoding error:
    Decode,

    /// A record's fragments add up to more than the receiver is willing to buffer:
    RecordTooLarge,

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
                Self::RecordTooLarge => "Record exceeds the maximum record size",
                Self::UnsupportedAuth => "Unsupported authorization mechanism",
                Self::WrongRpcVersion => "Only RPC Protocol version 2 is supported",
            }
        )
    }
}

/// A `call` holds the data needed to respond to an RPC call.
#[derive(Debug)]
pub struct Call<'a> {
    /// Transaction ID of this call.
    pub xid: u32,

    /// The call's header, which holds the program, version and procedure numbers (e.g., 100003,
    /// 3, and 1 for GETATTR in NFS v3), and the credential and verifier.
    pub header: CallBody,

    /// The call's encoded argument.
    pub arg: &'a [u8],
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
    let (message, rest) = match RpcMessage::deserialize(data) {
        Ok(m) => m,
        Err(e) => {
            warn!("Error deserializing message: {e}");
            return Err(ProtocolError::Decode);
        }
    };

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
        header: call,
        arg: rest,
    })
}

/// Given a buffer that contains an encoded message, prefaced by a dummy record mark, update that
/// record mark based on the actual length of the message.
fn update_record_mark(buf: &mut [u8]) {
    // size of message, not including the bytes for the record mark itself:
    let message_size = u32::try_from(buf.len() - RECORD_MARK_SIZE).unwrap();
    let record_mark = RecordMark {
        len: message_size,
        last: true,
    };
    buf[..RECORD_MARK_SIZE].copy_from_slice(&record_mark.encode());
}

/// A default limit on the size of a reassembled record, for callers that have no better number.
pub const DEFAULT_MAX_RECORD_SIZE: usize = 16 * 1024 * 1024;

/// Size in bytes of an encoded record mark.
pub const RECORD_MARK_SIZE: usize = 4;

/// The bit of a record mark which is set if the fragment is the last one of its record. The
/// remaining bits hold the length of the fragment.
const LAST_FRAGMENT: u32 = 1 << 31;

/// The header that precedes every record fragment on a stream transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordMark {
    /// Length of the fragment that follows the mark.
    pub len: u32,

    /// Whether this is the last fragment of its record.
    pub last: bool,
}

impl RecordMark {
    pub fn decode(mark: &[u8; RECORD_MARK_SIZE]) -> Self {
        let mark = u32::from_be_bytes(*mark);

        Self {
            len: mark & !LAST_FRAGMENT,
            last: mark & LAST_FRAGMENT != 0,
        }
    }

    /// `len` must fit in 31 bits, since the remaining bit is the last fragment flag.
    ///
    // TODO: this is not checked, so a larger `len` is silently encoded as a shorter fragment with
    // the last fragment flag set. Assert that `len` fits, and have `update_record_mark()` deal
    // with messages that are too large for one fragment.
    pub fn encode(self) -> [u8; RECORD_MARK_SIZE] {
        let mut mark = self.len;
        if self.last {
            mark |= LAST_FRAGMENT;
        }

        mark.to_be_bytes()
    }
}

/// Reassembles a record out of its fragments.
///
/// The caller does the reading. For each fragment it decodes the record mark, passes it to
/// `next_fragment()`, and fills the returned slice with the fragment's bytes. Once that has been
/// done for the last fragment of the record, `take()` returns the complete record:
///
/// ```ignore
/// loop {
///     let mark = RecordMark::decode(&read_mark()?);
///     read_exact(assembler.next_fragment(mark)?)?;
///     if let Some(record) = assembler.take() {
///         return Ok(record);
///     }
/// }
/// ```
pub struct RecordAssembler {
    max_size: usize,
    record: Vec<u8>,
    state: AssemblerState,
}

#[derive(PartialEq)]
enum AssemblerState {
    /// No fragment of the next record has been seen yet.
    Empty,

    /// Some fragments of the record have been seen, but not the last one.
    Partial,

    /// The last fragment of the record has been seen.
    Complete,
}

impl RecordAssembler {
    /// `max_size` bounds the total size of a record, so that a peer can't make the caller
    /// allocate an arbitrary amount of memory.
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            record: Vec::new(),
            state: AssemblerState::Empty,
        }
    }

    /// Makes room for the fragment described by `mark` and returns the slice that the caller must
    /// fill with that fragment's bytes.
    ///
    /// A complete record must be removed with `take()` before starting on the next one. It is a
    /// bug in the caller to not do so, and this panics in that case.
    pub fn next_fragment(&mut self, mark: RecordMark) -> Result<&mut [u8], ProtocolError> {
        assert!(
            self.state != AssemblerState::Complete,
            "the previous record was not taken"
        );

        let start = self.record.len();
        let len = mark.len as usize;

        if len > self.max_size - start {
            return Err(ProtocolError::RecordTooLarge);
        }

        self.state = if mark.last {
            AssemblerState::Complete
        } else {
            AssemblerState::Partial
        };
        self.record.resize(start + len, 0);

        Ok(&mut self.record[start..])
    }

    /// If the last fragment of the record has been passed to `next_fragment()`, returns the
    /// record and resets the assembler for the next one. Otherwise, returns None.
    pub fn take(&mut self) -> Option<Vec<u8>> {
        if self.state != AssemblerState::Complete {
            return None;
        }

        self.state = AssemblerState::Empty;
        Some(std::mem::take(&mut self.record))
    }

    /// True if no fragment has been passed to `next_fragment()` since the last record was taken.
    /// If the stream ends when this is false, then a record was truncated.
    pub fn is_empty(&self) -> bool {
        self.state == AssemblerState::Empty
    }
}

impl ReplyBody {
    /// Whether this is an accepted, succesful reply, which is the only kind of reply that is
    /// followed by the result of the procedure.
    pub fn is_success(&self) -> bool {
        matches!(
            self,
            ReplyBody::Accepted(AcceptedReply {
                reply_data: AcceptedReplyBody::Success(_),
                ..
            })
        )
    }
}

impl OpaqueAuth {
    pub fn none() -> Self {
        OpaqueAuth {
            flavor: AuthFlavor::None,
            body: Vec::new(),
        }
    }

    pub fn sys(parms: authsys_parms) -> Self {
        OpaqueAuth {
            flavor: AuthFlavor::Sys,
            body: parms.serialize_alloc(),
        }
    }
}

/// Returns a buffer with space for a record mark already allocated, but a dummy value (0) encoded
/// since the length of the message isn't known yet.
fn buf_with_dummy_record_mark() -> Vec<u8> {
    vec![0; RECORD_MARK_SIZE]
}
