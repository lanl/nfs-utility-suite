// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use crate::*;

/// Do an RPC call indicated by the `prog`, `vers`, and `proc`, arguments, using the given
/// `stream`.
///
/// `arg` must always be passed, but a zero-length slice can be used for a procedure which takes no
/// arguments.
///
/// This blocks the calling thread until the procedure returns a result. It returns either that
/// result as a byte vector (which the caller can decode), or an error.
pub fn do_rpc_call<S: Read + Write>(
    stream: &mut S,
    prog: u32,
    vers: u32,
    proc: u32,
    arg: &[u8],
) -> Result<Vec<u8>, Error> {
    let body = RpcMessageBody::Call(CallBody {
        rpcvers: RPC_VERSION,
        prog,
        vers,
        proc,
        cred: OpaqueAuth::none(),
        verf: OpaqueAuth::none(),
    });

    let xid = get_xid();

    let message = RpcMessage { xid, body };

    let mut buf = buf_with_dummy_record_mark();

    buf.append(&mut message.serialize_alloc());
    buf.extend_from_slice(arg);

    crate::update_record_mark(&mut buf);

    if let Err(e) = stream.write_all(&buf) {
        return Err(Error::Io(e));
    };

    read_reply_from_stream(xid, stream)
}

fn read_reply_from_stream<S: Read + Write>(
    xid: u32,
    stream: &mut S,
) -> Result<Vec<u8>, crate::Error> {
    let message_length = decode_record_mark(stream)?;

    let mut buf = vec![0; message_length as usize];
    if let Err(e) = stream.read_exact(&mut buf) {
        return Err(Error::Io(e));
    }

    let mut message = RpcMessage::default();
    let mut rest = buf.as_slice();
    if RpcMessage::deserialize(&mut message, &mut rest).is_err() {
        return Err(Error::Protocol(ProtocolError::Decode));
    }

    // Assuming that the stream was just used for sending the message indicated by the arg `xid`, it
    // is unexpected to get a different XID back in the reply:
    if message.xid != xid {
        return Err(Error::Protocol(ProtocolError::Decode));
    };

    // It is unexpected to receive a Call message after sending a Call message:
    let RpcMessageBody::Reply(reply) = message.body else {
        return Err(Error::Protocol(ProtocolError::Decode));
    };

    // Only continue for accepted succesful replies: anything else is returned as an error:
    let ReplyBody::Accepted(ref arep) = reply else {
        return Err(Error::Rpc(reply));
    };
    let AcceptedReplyBody::Success(_) = arep.reply_data else {
        return Err(Error::Rpc(reply));
    };

    // The entire header was already been decoded, so the rest of the message is the return value
    // of the RPC Call:
    Ok(rest.to_vec())
}
