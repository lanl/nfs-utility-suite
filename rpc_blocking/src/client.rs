// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2026. Triad National Security, LLC.

use std::hash::{BuildHasher, Hasher};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

use rpc_protocol::{client::*, CallBody, ProtocolError, RpcMessage, RpcMessageBody};

use crate::{read_record, Error};

/// Do the RPC call described by `call`, using the given `stream`.
///
/// `arg` must always be passed, but a zero-length slice can be used for a procedure which takes no
/// arguments.
///
/// This blocks the calling thread until the procedure returns a result. It returns either that
/// result as a byte vector (which the caller can decode), or an error.
pub fn do_rpc_call<S: Read + Write>(
    stream: &mut S,
    call: CallBody,
    arg: &[u8],
) -> Result<Vec<u8>, Error> {
    let xid = next_xid();

    let buf = encode_call(xid, call, arg);
    stream.write_all(&buf)?;

    let record = read_record(stream)?;

    let Ok((message, result)) = RpcMessage::deserialize(&record) else {
        return Err(Error::Protocol(ProtocolError::Decode));
    };

    // Assuming that the stream was just used for sending the message indicated by `xid`, it is
    // unexpected to get a different XID back in the reply:
    if message.xid != xid {
        return Err(Error::Protocol(ProtocolError::Decode));
    };

    // It is unexpected to receive a Call message after sending a Call message:
    let RpcMessageBody::Reply(reply) = message.body else {
        return Err(Error::Protocol(ProtocolError::Decode));
    };

    // Only continue for accepted succesful replies: anything else is returned as an error:
    if !reply.is_success() {
        return Err(Error::Rpc(reply));
    }

    // The entire header has been decoded, so the rest of the message is the return value of the
    // RPC call:
    //
    // TODO: this copies the result. Returning a type that owns `record` and remembers where the
    // result starts would avoid that.
    Ok(result.to_vec())
}

/// Returns a transaction ID that has not been used by this process before. The first one is
/// chosen randomly.
fn next_xid() -> u32 {
    static NEXT_XID: OnceLock<AtomicU32> = OnceLock::new();

    NEXT_XID
        .get_or_init(|| {
            // The standard library seeds each RandomState randomly, which makes it a source of
            // randomness that doesn't need another dependency:
            let seed = std::collections::hash_map::RandomState::new()
                .build_hasher()
                .finish();
            AtomicU32::new(seed as u32)
        })
        .fetch_add(1, Ordering::Relaxed)
}
