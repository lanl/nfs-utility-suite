// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2026. Triad National Security, LLC.

use std::hash::{BuildHasher, Hasher};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

use log::*;

use rpc_protocol::{client::*, CallBody, ProtocolError, RpcMessage, RpcMessageBody};

use crate::{read_record, Error};

/// Do the RPC call described by `call`, using the given `stream`.
///
/// `arg` must always be passed, but a zero-length slice can be used for a procedure which takes no
/// arguments.
///
/// This blocks the calling thread until the procedure returns a result. It returns either that
/// result as a byte vector (which the caller can decode), or an error.
///
/// Anything received which is not the reply to this call is skipped. That includes replies to
/// earlier calls which were abandoned, duplicate replies, and calls made by the server.
///
/// If this returns an I/O error or a `RecordTooLarge` error, then `stream` may have been left in
/// the middle of a record, in either direction. There is no way to find the start of the next
/// record again, so the stream must not be used for any more calls.
pub fn do_rpc_call<S: Read + Write>(
    stream: &mut S,
    call: CallBody,
    arg: &[u8],
) -> Result<Vec<u8>, Error> {
    let xid = next_xid();

    let buf = encode_call(xid, call, arg);
    stream.write_all(&buf)?;

    loop {
        let record = read_record(stream)?;

        let Ok((message, result)) = RpcMessage::deserialize(&record) else {
            // A malformed record is only an error for this call if it claims to be its reply:
            if peek_xid(&record).is_some_and(|other| other != xid) {
                debug!("Skipping a malformed record which is not the reply to this call");
                continue;
            }
            return Err(Error::Protocol(ProtocolError::Decode));
        };

        // The stream may have been used for other calls before this one, so this is not
        // necessarily the reply to this call:
        if message.xid != xid {
            debug!(
                "Skipping a message with XID {}, expected {xid}",
                message.xid
            );
            continue;
        }

        let RpcMessageBody::Reply(reply) = message.body else {
            debug!("Skipping a CALL sent by the server");
            continue;
        };

        // Only continue for accepted succesful replies: anything else is returned as an error:
        if !reply.is_success() {
            return Err(Error::Rpc(reply));
        }

        // The entire header has been decoded, so the rest of the message is the return value of
        // the RPC call:
        //
        // TODO: this copies the result. Returning a type that owns `record` and remembers where
        // the result starts would avoid that.
        return Ok(result.to_vec());
    }
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
