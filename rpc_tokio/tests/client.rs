// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2026. Triad National Security, LLC.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

use rpc_protocol::{server::*, *};
use rpc_tokio::{Client, Error};

/// Returns a client and the server's end of the client's connection.
fn connect() -> (Client, DuplexStream) {
    let (client_end, server_end) = tokio::io::duplex(4096);

    (Client::new(client_end), server_end)
}

async fn call(client: &Client, arg: &[u8]) -> Result<Vec<u8>, Error> {
    client
        .call(
            CallBody {
                rpcvers: RPC_VERSION,
                prog: 7,
                vers: 4,
                proc: 1,
                cred: OpaqueAuth::none(),
                verf: OpaqueAuth::none(),
            },
            arg,
        )
        .await
}

/// Reads one unfragmented record from the client and returns it without its record mark.
async fn read_call(server: &mut DuplexStream) -> Vec<u8> {
    let mut mark = [0; RECORD_MARK_SIZE];
    server.read_exact(&mut mark).await.unwrap();

    let mark = RecordMark::decode(&mark);
    assert!(mark.last);

    let mut record = vec![0; mark.len as usize];
    server.read_exact(&mut record).await.unwrap();

    record
}

/// Reads one call from the client and returns the reply that echoes the call's argument back.
async fn read_call_and_encode_echo(server: &mut DuplexStream) -> Vec<u8> {
    let record = read_call(server).await;
    let call = decode_call(&record).unwrap();

    assert_eq!(call.header.prog, 7);
    assert_eq!(call.header.vers, 4);
    assert_eq!(call.header.proc, 1);

    encode_succesful_reply(call.xid, call.arg)
}

#[tokio::test]
async fn call_returns_result() {
    let (client, mut server) = connect();

    tokio::spawn(async move {
        loop {
            let reply = read_call_and_encode_echo(&mut server).await;
            server.write_all(&reply).await.unwrap();
        }
    });

    assert_eq!(call(&client, &[1, 2, 3, 4]).await.unwrap(), [1, 2, 3, 4]);
    assert_eq!(call(&client, &[]).await.unwrap(), []);
    assert_eq!(client.stale_replies(), 0);
}

#[tokio::test]
async fn concurrent_calls_get_their_own_replies() {
    const CALLS: u32 = 8;

    let (client, mut server) = connect();

    // The server waits until every call has arrived, and then replies in the opposite order:
    tokio::spawn(async move {
        let mut replies = Vec::new();
        for _ in 0..CALLS {
            replies.push(read_call_and_encode_echo(&mut server).await);
        }
        for reply in replies.iter().rev() {
            server.write_all(reply).await.unwrap();
        }
        server
    });

    let mut calls = Vec::new();
    for i in 0..CALLS {
        let client = client.clone();
        calls.push(tokio::spawn(async move {
            call(&client, &i.to_be_bytes()).await
        }));
    }

    for (i, res) in calls.into_iter().enumerate() {
        assert_eq!(res.await.unwrap().unwrap(), (i as u32).to_be_bytes());
    }
    assert_eq!(client.stale_replies(), 0);
}

#[tokio::test]
async fn fragmented_reply_is_reassembled() {
    let (client, mut server) = connect();

    tokio::spawn(async move {
        let reply = read_call_and_encode_echo(&mut server).await;

        // Split the reply (minus its record mark) into fragments of at most 5 bytes:
        let fragments: Vec<&[u8]> = reply[RECORD_MARK_SIZE..].chunks(5).collect();
        for (i, fragment) in fragments.iter().enumerate() {
            let mark = RecordMark {
                len: fragment.len() as u32,
                last: i == fragments.len() - 1,
            };
            server.write_all(&mark.encode()).await.unwrap();
            server.write_all(fragment).await.unwrap();
        }
        server
    });

    let arg = [9, 8, 7, 6, 5, 4, 3, 2];
    assert_eq!(call(&client, &arg).await.unwrap(), arg);
}

#[tokio::test]
async fn error_reply_is_returned_as_error() {
    let (client, mut server) = connect();

    tokio::spawn(async move {
        let record = read_call(&mut server).await;
        let xid = decode_call(&record).unwrap().xid;

        let reply = ReplyBody::accepted_reply(AcceptedReplyBody::ProcUnavail);
        server
            .write_all(&encode_reply_no_arg(xid, reply))
            .await
            .unwrap();
        server
    });

    let res = call(&client, &[]).await;
    let Err(Error::Rpc(ReplyBody::Accepted(reply))) = res else {
        panic!("Expected RPC error reply, got {res:?}");
    };
    assert_eq!(reply.reply_data, AcceptedReplyBody::ProcUnavail);
}

#[tokio::test]
async fn malformed_reply_fails_only_its_own_call() {
    let (client, mut server) = connect();

    tokio::spawn(async move {
        let record = read_call(&mut server).await;
        let xid = decode_call(&record).unwrap().xid;

        // The right XID, followed by a message type that is neither CALL nor REPLY:
        let mark = RecordMark { len: 8, last: true };
        let mut reply = mark.encode().to_vec();
        reply.extend_from_slice(&xid.to_be_bytes());
        reply.extend_from_slice(&[0, 0, 0, 99]);
        server.write_all(&reply).await.unwrap();

        let reply = read_call_and_encode_echo(&mut server).await;
        server.write_all(&reply).await.unwrap();
        server
    });

    let res = call(&client, &[]).await;
    assert!(matches!(res, Err(Error::Protocol(_))), "got {res:?}");

    assert_eq!(call(&client, &[0, 0, 0, 1]).await.unwrap(), [0, 0, 0, 1]);
}

#[tokio::test]
async fn calls_fail_when_connection_closes() {
    let (client, mut server) = connect();

    tokio::spawn(async move {
        // Read the call, then drop the connection without replying.
        read_call(&mut server).await;
    });

    let res = call(&client, &[]).await;
    assert!(matches!(res, Err(Error::ConnectionClosed)), "got {res:?}");

    // Later calls fail too, rather than waiting for a reply that can't arrive:
    let res = call(&client, &[]).await;
    assert!(matches!(res, Err(Error::ConnectionClosed)), "got {res:?}");
}

#[tokio::test]
async fn oversized_reply_closes_connection() {
    let (client_end, mut server) = tokio::io::duplex(4096);
    let client = Client::with_max_record_size(client_end, 64);

    tokio::spawn(async move {
        read_call(&mut server).await;

        // Two fragments that are each within the limit, but together exceed it:
        let mut mark = RecordMark {
            len: 40,
            last: false,
        };
        server.write_all(&mark.encode()).await.unwrap();
        server.write_all(&[0; 40]).await.unwrap();

        mark.last = true;
        server.write_all(&mark.encode()).await.unwrap();
        let _ = server.write_all(&[0; 40]).await;
        server
    });

    let res = call(&client, &[]).await;
    assert!(matches!(res, Err(Error::ConnectionClosed)), "got {res:?}");
}

#[tokio::test]
async fn abandoned_call_does_not_disturb_later_calls() {
    let (client, mut server) = connect();

    let (late_reply_sent, wait_for_late_reply) = tokio::sync::oneshot::channel();

    tokio::spawn(async move {
        // Hold on to the reply to the first call until the second call has arrived:
        let first = read_call_and_encode_echo(&mut server).await;
        let second = read_call_and_encode_echo(&mut server).await;

        server.write_all(&first).await.unwrap();
        server.write_all(&second).await.unwrap();
        late_reply_sent.send(()).unwrap();
        server
    });

    // The caller gives up on the first call by dropping it:
    let res = tokio::time::timeout(Duration::from_millis(50), call(&client, &[0, 0, 0, 1])).await;
    assert!(res.is_err());

    assert_eq!(call(&client, &[0, 0, 0, 2]).await.unwrap(), [0, 0, 0, 2]);

    wait_for_late_reply.await.unwrap();
    assert_eq!(client.stale_replies(), 1);
}

#[tokio::test]
async fn duplicate_reply_is_dropped() {
    let (client, mut server) = connect();

    tokio::spawn(async move {
        // Reply to the first call twice:
        let reply = read_call_and_encode_echo(&mut server).await;
        server.write_all(&reply).await.unwrap();
        server.write_all(&reply).await.unwrap();

        let reply = read_call_and_encode_echo(&mut server).await;
        server.write_all(&reply).await.unwrap();
        server
    });

    assert_eq!(call(&client, &[0, 0, 0, 1]).await.unwrap(), [0, 0, 0, 1]);

    // The second call gets its own reply, not the duplicate of the first one:
    assert_eq!(call(&client, &[0, 0, 0, 2]).await.unwrap(), [0, 0, 0, 2]);
    assert_eq!(client.stale_replies(), 1);
}

#[tokio::test]
async fn call_from_server_is_not_taken_as_a_reply() {
    let (client, mut server) = connect();

    tokio::spawn(async move {
        let record = read_call(&mut server).await;
        let call = decode_call(&record).unwrap();

        // Before replying, the server sends a CALL of its own which has the same XID as the call
        // that the client is waiting on:
        let server_call = client::encode_call(
            call.xid,
            CallBody {
                rpcvers: RPC_VERSION,
                prog: 7,
                vers: 4,
                proc: 1,
                cred: OpaqueAuth::none(),
                verf: OpaqueAuth::none(),
            },
            &[9, 9, 9, 9],
        );
        server.write_all(&server_call).await.unwrap();

        let reply = encode_succesful_reply(call.xid, call.arg);
        server.write_all(&reply).await.unwrap();
        server
    });

    assert_eq!(call(&client, &[0, 0, 0, 1]).await.unwrap(), [0, 0, 0, 1]);
    assert_eq!(client.stale_replies(), 0);
}

#[tokio::test]
async fn connection_closes_when_client_is_dropped() {
    let (client, mut server) = connect();

    drop(client);

    // The server sees the end of the stream:
    let mut buf = [0; 4];
    let res = server.read_exact(&mut buf).await.unwrap_err();
    assert_eq!(res.kind(), std::io::ErrorKind::UnexpectedEof);
}
