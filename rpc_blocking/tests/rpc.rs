// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::io::{Read, Write};

use rpc_blocking::{client::do_rpc_call, pipe, server::handle_connection, Error};
use rpc_protocol::{server::*, *};

#[test]
fn call_invalid_program() {
    let mut client_endpoint = launch_example_server();

    // invalid CALL: wrong program number
    let res = do_rpc_call(
        &mut client_endpoint,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: 8,
            vers: 4,
            proc: 1,
            cred: OpaqueAuth::none(),
            verf: OpaqueAuth::none(),
        },
        &[0; 0],
    );

    expected_error(res, AcceptedReplyBody::ProgUnavail);
}

#[test]
fn call_invalid_version() {
    let mut client_endpoint = launch_example_server();

    // invalid CALL: too low version number
    let res = do_rpc_call(
        &mut client_endpoint,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: 7,
            vers: 1,
            proc: 1,
            cred: OpaqueAuth::none(),
            verf: OpaqueAuth::none(),
        },
        &[0; 0],
    );

    expected_error(
        res,
        AcceptedReplyBody::ProgMismatch(ProgMismatchBody { low: 2, high: 4 }),
    );

    let mut client_endpoint = launch_example_server();

    // invalid CALL: too high version number
    let res = do_rpc_call(
        &mut client_endpoint,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: 7,
            vers: 5,
            proc: 1,
            cred: OpaqueAuth::none(),
            verf: OpaqueAuth::none(),
        },
        &[0; 0],
    );

    expected_error(
        res,
        AcceptedReplyBody::ProgMismatch(ProgMismatchBody { low: 2, high: 4 }),
    );
}

#[test]
fn call_invalid_procedure() {
    let mut client_endpoint = launch_example_server();

    // invalid CALL: wrong procedure number
    let res = do_rpc_call(
        &mut client_endpoint,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: 7,
            vers: 4,
            proc: 2,
            cred: OpaqueAuth::none(),
            verf: OpaqueAuth::none(),
        },
        &[0; 0],
    );

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

#[test]
fn call_with_fragmented_reply() {
    let (mut client_endpoint, mut server_endpoint) = pipe::pipe().unwrap();

    std::thread::spawn(move || {
        let mut mark = [0; RECORD_MARK_SIZE];
        server_endpoint.read_exact(&mut mark).unwrap();
        let mut call = vec![0; RecordMark::decode(&mark).len as usize];
        server_endpoint.read_exact(&mut call).unwrap();
        let xid = decode_call(&call).unwrap().xid;

        // Send the reply (minus its own record mark) as fragments of at most 5 bytes:
        let reply = encode_succesful_reply(xid, &[1, 2, 3, 4]);
        let fragments: Vec<&[u8]> = reply[RECORD_MARK_SIZE..].chunks(5).collect();
        for (i, fragment) in fragments.iter().enumerate() {
            let mark = RecordMark {
                len: fragment.len() as u32,
                last: i == fragments.len() - 1,
            };
            server_endpoint.write_all(&mark.encode()).unwrap();
            server_endpoint.write_all(fragment).unwrap();
        }
    });

    let res = do_rpc_call(
        &mut client_endpoint,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: 7,
            vers: 4,
            proc: 1,
            cred: OpaqueAuth::none(),
            verf: OpaqueAuth::none(),
        },
        &[0; 0],
    );

    assert_eq!(res.unwrap(), [1, 2, 3, 4]);
}

#[test]
fn connection_stays_open_after_error_reply() {
    let mut client_endpoint = launch_example_server();

    // invalid CALL: wrong procedure number
    let res = do_rpc_call(
        &mut client_endpoint,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: 7,
            vers: 4,
            proc: 2,
            cred: OpaqueAuth::none(),
            verf: OpaqueAuth::none(),
        },
        &[0; 0],
    );
    expected_error(res, AcceptedReplyBody::ProcUnavail);

    // invalid CALL: wrong program number
    let res = do_rpc_call(
        &mut client_endpoint,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: 8,
            vers: 4,
            proc: 1,
            cred: OpaqueAuth::none(),
            verf: OpaqueAuth::none(),
        },
        &[0; 0],
    );
    expected_error(res, AcceptedReplyBody::ProgUnavail);

    // A valid call on the same connection still works:
    let res = do_rpc_call(
        &mut client_endpoint,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: 7,
            vers: 4,
            proc: 1,
            cred: OpaqueAuth::none(),
            verf: OpaqueAuth::none(),
        },
        &[0; 0],
    );
    assert_eq!(res.unwrap(), []);
}

fn garbage_args_procedure(_call: &Call, _state: &mut ()) -> RpcResult {
    RpcResult::GarbageArgs
}

fn system_err_procedure(_call: &Call, _state: &mut ()) -> RpcResult {
    RpcResult::SystemErr
}

#[test]
fn procedure_errors_are_sent_as_replies() {
    let (mut client_endpoint, mut server_endpoint) = pipe::pipe().unwrap();

    let procedures: Vec<Option<RpcProcedure<()>>> = vec![
        None,
        Some(garbage_args_procedure),
        Some(system_err_procedure),
    ];
    let mut server = RpcProgram::new(7, 2, 4, procedures, ());

    std::thread::spawn(move || {
        let _ = handle_connection(&mut server, &mut server_endpoint);
    });

    let res = do_rpc_call(
        &mut client_endpoint,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: 7,
            vers: 4,
            proc: 1,
            cred: OpaqueAuth::none(),
            verf: OpaqueAuth::none(),
        },
        &[0; 0],
    );
    expected_error(res, AcceptedReplyBody::GarbageArgs);

    let res = do_rpc_call(
        &mut client_endpoint,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: 7,
            vers: 4,
            proc: 2,
            cred: OpaqueAuth::none(),
            verf: OpaqueAuth::none(),
        },
        &[0; 0],
    );
    expected_error(res, AcceptedReplyBody::SystemErr);
}

/// A stream which holds one valid call to be read, and on which every write fails.
struct WriteFails {
    call: std::io::Cursor<Vec<u8>>,
}

impl Read for WriteFails {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.call.read(buf)
    }
}

impl Write for WriteFails {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Err(std::io::ErrorKind::BrokenPipe.into())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn server_gives_up_on_connection_when_reply_cant_be_written() {
    let call = client::encode_call(
        1,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: 7,
            vers: 4,
            proc: 1,
            cred: OpaqueAuth::none(),
            verf: OpaqueAuth::none(),
        },
        &[],
    );
    let stream = WriteFails {
        call: std::io::Cursor::new(call),
    };

    let mut server = RpcProgram::new(7, 2, 4, vec![None, Some(null_procedure)], ());

    let res = handle_connection(&mut server, stream);
    let Err(Error::Io(e)) = res else {
        panic!("Expected an I/O error, got {res:?}");
    };
    assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe);
}

/// Launches an RpcProgram with program number 7, version range 2-4, and one procedure defined (in
/// addition to procedure 0 which is always defined.)
///
/// Returns a client endpoint for comunicating with the service.
fn launch_example_server() -> pipe::Endpoint {
    let (client_endpoint, mut server_endpoint) = pipe::pipe().unwrap();

    let mut server = RpcProgram::new(7, 2, 4, vec![None, Some(null_procedure)], ());

    std::thread::spawn(move || {
        handle_connection(&mut server, &mut server_endpoint).unwrap();
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
