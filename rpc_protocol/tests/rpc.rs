// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use rpc_protocol::*;

#[test]
fn rpc_protocol_call() {
    let msg = RpcMessage {
        xid: 1,
        body: RpcMessageBody::Call(CallBody {
            rpcvers: 2,
            prog: 3,
            vers: 2,
            proc: 17,
            cred: OpaqueAuth {
                flavor: AuthFlavor::None,
                body: Vec::new(),
            },
            verf: OpaqueAuth {
                flavor: AuthFlavor::None,
                body: Vec::new(),
            },
        }),
    };

    let bytes = msg.serialize_alloc();
    let mut after = RpcMessage::default();
    RpcMessage::deserialize(&mut after, &mut bytes.as_slice()).unwrap();

    assert_eq!(msg, after);
}

#[test]
fn rpc_protocol_reply() {
    let reply = RpcMessage {
        xid: 2,
        body: RpcMessageBody::Reply(ReplyBody::Accepted(AcceptedReply {
            verf: OpaqueAuth {
                flavor: AuthFlavor::None,
                body: Vec::new(),
            },
            reply_data: AcceptedReplyBody::Success([0; 0]),
        })),
    };

    let bytes = reply.serialize_alloc();
    let mut after = RpcMessage::default();
    RpcMessage::deserialize(&mut after, &mut bytes.as_slice()).unwrap();

    assert_eq!(reply, after);
}
