// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::collections::hash_map::{Entry, HashMap};

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

/// The set of calls that have been sent on a connection and not yet replied to.
///
/// This allocates the transaction ID for each call, and maps the transaction ID of a reply back
/// to whatever the caller stored for that call (`W`), which is typically something that can wake
/// up the task or thread waiting for the reply.
pub struct PendingCalls<W> {
    next_xid: u32,
    calls: HashMap<u32, W>,
}

impl<W> PendingCalls<W> {
    /// `first_xid` is the transaction ID given to the first call. Callers should choose it
    /// randomly so that transaction IDs are not repeated across connections.
    pub fn new(first_xid: u32) -> Self {
        Self {
            next_xid: first_xid,
            calls: HashMap::new(),
        }
    }

    /// Registers a new call and returns the transaction ID to send it with.
    pub fn insert(&mut self, waiter: W) -> u32 {
        loop {
            let xid = self.next_xid;
            self.next_xid = self.next_xid.wrapping_add(1);

            // Only relevant once the counter has wrapped around while a call is still pending:
            if let Entry::Vacant(entry) = self.calls.entry(xid) {
                entry.insert(waiter);
                return xid;
            }
        }
    }

    /// Removes the call with the given transaction ID. Returns None if there is no such call,
    /// for example because the reply is a duplicate or arrived after the caller gave up.
    pub fn remove(&mut self, xid: u32) -> Option<W> {
        self.calls.remove(&xid)
    }

    /// Removes every call, for example because the connection was lost.
    pub fn drain(&mut self) -> impl Iterator<Item = W> + '_ {
        self.calls.drain().map(|(_, waiter)| waiter)
    }

    pub fn len(&self) -> usize {
        self.calls.len()
    }

    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xid_in_use_is_skipped() {
        let mut pending = PendingCalls::new(10);
        assert_eq!(pending.insert("old"), 10);

        // Pretend that the counter went all the way around while the call was still pending:
        pending.next_xid = 10;

        assert_eq!(pending.insert("new"), 11);
        assert_eq!(pending.remove(10), Some("old"));
        assert_eq!(pending.remove(11), Some("new"));
    }
}
