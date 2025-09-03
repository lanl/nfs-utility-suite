// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::io::{Read, Write};

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

#[test]
fn call_invalid_program() {
    let mut client_endpoint = launch_example_server();

    // invalid CALL: wrong program number
    let res = client::do_rpc_call(&mut client_endpoint, 8, 4, 1, &[0; 0]);

    expected_error(res, AcceptedReplyBody::ProgUnavail);
}

#[test]
fn call_invalid_version() {
    let mut client_endpoint = launch_example_server();

    // invalid CALL: too low version number
    let res = client::do_rpc_call(&mut client_endpoint, 7, 1, 1, &[0; 0]);

    expected_error(
        res,
        AcceptedReplyBody::ProgMismatch(ProgMismatchBody { low: 2, high: 4 }),
    );

    let mut client_endpoint = launch_example_server();

    // invalid CALL: too high version number
    let res = client::do_rpc_call(&mut client_endpoint, 7, 5, 1, &[0; 0]);

    expected_error(
        res,
        AcceptedReplyBody::ProgMismatch(ProgMismatchBody { low: 2, high: 4 }),
    );
}

#[test]
fn call_invalid_procedure() {
    let mut client_endpoint = launch_example_server();

    // invalid CALL: wrong procedure number
    let res = client::do_rpc_call(&mut client_endpoint, 7, 4, 2, &[0; 0]);

    expected_error(res, AcceptedReplyBody::ProcUnavail);
}

#[test]
fn call_invalid_rpc_version() {
    let mut client_endpoint = launch_example_server();

    // This is a byte sequence that is an RPC call manipulated to have an RPC version of 3 instead
    // of 2.
    let buf = vec![
        //                                               v here
        128, 0, 0, 40, 0, 0, 0, 17, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 7, 0, 0, 0, 4, 0, 0, 0, 2, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    client_endpoint.write_all(&buf).unwrap();

    let mut output = vec![0u8; 4];
    let res = client_endpoint.read_exact(&mut output).unwrap_err();
    // It is expected that the server simply drops the connection, which leads to being unable to
    // read from the pipe endpoint:
    assert_eq!(res.kind(), std::io::ErrorKind::UnexpectedEof);
}

/// Launches an RpcProgram with program number 7, version range 2-4, and one procedure defined (in
/// addition to procedure 0 which is always defined.)
///
/// Returns a client endpoint for comunicating with the service.
fn launch_example_server() -> pipe::Endpoint {
    let (client_endpoint, mut server_endpoint) = pipe::pipe().unwrap();

    let mut server = server::RpcProgram::new(7, 2, 4, vec![None, Some(server::null_procedure)], ());

    std::thread::spawn(move || {
        server.handle_connection(&mut server_endpoint).unwrap();
    });

    client_endpoint
}

fn expected_error(res: Result<Vec<u8>, Error>, expected: AcceptedReplyBody) {
    let Err(Error::Rpc(reply)) = res else {
        panic!("Expected RPC error reply, got {res:?}");
    };

    let ReplyBody::Accepted(arep) = reply else {
        panic!("Expected Accepted reply, got {reply:?}");
    };

    if arep.reply_data != expected {
        panic!("Expected {expected:?}, got {:?}", arep.reply_data);
    }
}
