// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use crate::*;

/// Encodes the RPC call described by `call` into a record that is ready to be written to a
/// stream transport (that is, it begins with a record mark).
///
/// `arg` must always be passed, but a zero-length slice can be used for a procedure which takes no
/// arguments.
pub fn encode_call(xid: u32, call: CallBody, arg: &[u8]) -> Vec<u8> {
    let body = RpcMessageBody::Call(call);

    let message = RpcMessage { xid, body };

    let mut buf = buf_with_dummy_record_mark();

    // TODO: this copies `arg`, and lets the buffer grow as needed rather than allocating the
    // right size up front. Avoiding the copy means either letting the caller serialize the
    // argument directly into this buffer, or returning the header on its own so that the header
    // and the argument can be written with one vectored write.
    buf.append(&mut message.serialize_alloc());
    buf.extend_from_slice(arg);

    update_record_mark(&mut buf);

    buf
}

/// Returns the transaction ID of a record without decoding the rest of it. This makes it possible
/// to tell which call a reply was meant for even when the reply is malformed.
pub fn peek_xid(record: &[u8]) -> Option<u32> {
    let (xid, _) = xdr_lib::get_u32(record).ok()?;

    Some(xid)
}
