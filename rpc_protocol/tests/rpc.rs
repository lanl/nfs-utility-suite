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
    let (after, _) = RpcMessage::deserialize(bytes.as_slice()).unwrap();

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
    let (after, _) = RpcMessage::deserialize(bytes.as_slice()).unwrap();

    assert_eq!(reply, after);
}

#[test]
fn decode_call_undefined_message_type() {
    // xid 1, followed by a message type that is neither CALL nor REPLY.
    let buf = vec![0, 0, 0, 1, 0, 0, 0, 99];

    let res = decode_call(&buf);
    assert!(matches!(res, Err(ProtocolError::Decode)), "got {res:?}");
}

#[test]
fn decode_call_credential_longer_than_record() {
    // A CALL whose credential length prefix claims 0xffffffff bytes, with no credential body
    // behind it.
    let buf = vec![
        0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 1, 134, 160, 0, 0, 0, 4, 0, 0, 0, 4, 0, 0, 0, 0,
        //                                                                    v here
        255, 255, 255, 255,
    ];

    let res = decode_call(&buf);
    assert!(matches!(res, Err(ProtocolError::Decode)), "got {res:?}");
}

#[test]
fn call_and_reply_roundtrip() {
    let record = client::encode_call(
        99,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: 7,
            vers: 4,
            proc: 1,
            cred: OpaqueAuth::none(),
            verf: OpaqueAuth::none(),
        },
        &[1, 2, 3, 4],
    );

    let mark = RecordMark::decode(record[..RECORD_MARK_SIZE].try_into().unwrap());
    assert!(mark.last);
    assert_eq!(mark.len as usize, record.len() - RECORD_MARK_SIZE);

    let call = decode_call(&record[RECORD_MARK_SIZE..]).unwrap();
    assert_eq!(call.xid, 99);
    assert_eq!(call.header.prog, 7);
    assert_eq!(call.header.vers, 4);
    assert_eq!(call.header.proc, 1);
    assert_eq!(call.arg, [1, 2, 3, 4]);

    let record = server::encode_succesful_reply(call.xid, &[5, 6, 7, 8]);
    assert_eq!(client::peek_xid(&record[RECORD_MARK_SIZE..]), Some(99));

    let (message, result) = RpcMessage::deserialize(&record[RECORD_MARK_SIZE..]).unwrap();
    assert_eq!(message.xid, 99);
    let RpcMessageBody::Reply(reply) = message.body else {
        panic!("Expected a reply, got {message:?}");
    };
    assert!(reply.is_success());
    assert_eq!(result, [5, 6, 7, 8]);
}

#[test]
fn error_reply_is_not_success() {
    let reply = ReplyBody::accepted_reply(AcceptedReplyBody::ProgUnavail);
    let record = server::encode_reply_no_arg(3, reply.clone());

    let (message, result) = RpcMessage::deserialize(&record[RECORD_MARK_SIZE..]).unwrap();
    assert_eq!(message.xid, 3);
    assert_eq!(message.body, RpcMessageBody::Reply(reply.clone()));
    assert!(!reply.is_success());
    assert!(result.is_empty());
}

#[test]
fn record_mark_encode_and_decode() {
    let mark = RecordMark {
        len: 258,
        last: true,
    };
    assert_eq!(mark.encode(), [128, 0, 1, 2]);
    assert_eq!(RecordMark::decode(&[128, 0, 1, 2]), mark);

    let mark = RecordMark {
        len: 40,
        last: false,
    };
    assert_eq!(mark.encode(), [0, 0, 0, 40]);
    assert_eq!(RecordMark::decode(&[0, 0, 0, 40]), mark);
}

#[test]
fn assemble_fragmented_record() {
    let mut assembler = RecordAssembler::new(16);
    assert!(assembler.is_empty());
    assert_eq!(assembler.take(), None);

    let fragment = assembler
        .next_fragment(RecordMark {
            len: 3,
            last: false,
        })
        .unwrap();
    fragment.copy_from_slice(&[1, 2, 3]);
    assert!(!assembler.is_empty());

    // The record can't be taken before its last fragment:
    assert_eq!(assembler.take(), None);

    // An empty fragment in the middle of a record is legal:
    let fragment = assembler
        .next_fragment(RecordMark {
            len: 0,
            last: false,
        })
        .unwrap();
    assert!(fragment.is_empty());

    let fragment = assembler
        .next_fragment(RecordMark { len: 2, last: true })
        .unwrap();
    fragment.copy_from_slice(&[4, 5]);
    assert!(!assembler.is_empty());

    assert_eq!(assembler.take(), Some(vec![1, 2, 3, 4, 5]));
    assert!(assembler.is_empty());
    assert_eq!(assembler.take(), None);

    // The assembler can be used again for the next record:
    let fragment = assembler
        .next_fragment(RecordMark { len: 1, last: true })
        .unwrap();
    fragment.copy_from_slice(&[6]);
    assert_eq!(assembler.take(), Some(vec![6]));
}

#[test]
fn assemble_empty_record() {
    let mut assembler = RecordAssembler::new(16);

    let fragment = assembler
        .next_fragment(RecordMark { len: 0, last: true })
        .unwrap();
    assert!(fragment.is_empty());

    assert_eq!(assembler.take(), Some(vec![]));
}

#[test]
#[should_panic(expected = "not taken")]
fn assemble_without_taking_previous_record() {
    let mut assembler = RecordAssembler::new(16);

    let mark = RecordMark { len: 1, last: true };
    assembler.next_fragment(mark).unwrap();
    let _ = assembler.next_fragment(mark);
}

#[test]
fn assemble_record_too_large() {
    let mut assembler = RecordAssembler::new(8);

    // A single fragment that is larger than the limit:
    let res = assembler.next_fragment(RecordMark { len: 9, last: true });
    assert!(matches!(res, Err(ProtocolError::RecordTooLarge)));

    // Fragments which are individually small enough, but together exceed the limit:
    assembler
        .next_fragment(RecordMark {
            len: 8,
            last: false,
        })
        .unwrap();
    let res = assembler.next_fragment(RecordMark { len: 1, last: true });
    assert!(matches!(res, Err(ProtocolError::RecordTooLarge)));
}
