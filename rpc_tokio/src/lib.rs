// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2026. Triad National Security, LLC.

//! An asynchronous RPC client for tokio, built on top of the I/O-free `rpc_protocol` crate.
//!
//! A `Client` owns one connection and allows many calls to be in flight on it at the same time.
//! A writer task writes the encoded calls to the connection, and a reader task reads the replies
//! and hands each one to the caller that is waiting for it, based on the transaction ID.

use std::fmt;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use log::*;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};
use tokio::task::AbortHandle;

use rpc_protocol::{
    client::*, CallBody, ProtocolError, RecordAssembler, RecordMark, ReplyBody, RpcMessage,
    RpcMessageBody, DEFAULT_MAX_RECORD_SIZE, RECORD_MARK_SIZE,
};

/// How many encoded calls can be queued up for the writer task before `Client::call()` waits.
const WRITE_QUEUE_LENGTH: usize = 64;

/// The possible errors that can arise from an RPC call.
#[derive(Debug)]
pub enum Error {
    /// The reply to the call could not be decoded.
    Protocol(ProtocolError),

    /// The server replied with something other than an accepted, succesful reply.
    Rpc(ReplyBody),

    /// The connection was closed or failed before the reply to the call arrived.
    ConnectionClosed,
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Protocol(e) => write!(f, "Protocol error: {e}"),
            Self::Rpc(e) => write!(f, "RPC error: {e:?}"),
            Self::ConnectionClosed => write!(f, "Connection closed"),
        }
    }
}

/// What a call is waiting on: either the encoded result of the procedure, or an error.
type Waiter = oneshot::Sender<Result<Vec<u8>, Error>>;

struct State {
    pending: PendingCalls<Waiter>,

    /// Set once the connection is no longer usable, so that new calls fail instead of waiting for
    /// a reply that can't arrive.
    closed: bool,
}

struct Shared {
    state: Mutex<State>,
    stale_replies: AtomicU64,
}

impl Shared {
    /// Marks the connection as unusable and fails every call that is waiting for a reply.
    fn close(&self) {
        let mut state = self.state.lock().unwrap();

        state.closed = true;
        for waiter in state.pending.drain() {
            let _ = waiter.send(Err(Error::ConnectionClosed));
        }
    }
}

/// A handle to a connection to an RPC server. Clones share the same connection.
#[derive(Clone)]
pub struct Client {
    writer: mpsc::Sender<Vec<u8>>,
    shared: Arc<Shared>,
}

impl Client {
    /// Takes ownership of a connected stream. This spawns tasks, so it must be called from
    /// within a tokio runtime. The connection is closed once every clone of the Client has been
    /// dropped.
    pub fn new<S>(stream: S) -> Self
    where
        S: AsyncRead + AsyncWrite + Send + 'static,
    {
        Self::with_max_record_size(stream, DEFAULT_MAX_RECORD_SIZE)
    }

    /// Like `new()`, but replies larger than `max_record_size` are treated as a connection
    /// failure.
    pub fn with_max_record_size<S>(stream: S, max_record_size: usize) -> Self
    where
        S: AsyncRead + AsyncWrite + Send + 'static,
    {
        let (read_half, write_half) = tokio::io::split(stream);

        // The standard library seeds each RandomState randomly, which makes it a source of
        // randomness that doesn't need another dependency:
        let first_xid = std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish() as u32;

        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                pending: PendingCalls::new(first_xid),
                closed: false,
            }),
            stale_replies: AtomicU64::new(0),
        });

        let (writer, write_queue) = mpsc::channel(WRITE_QUEUE_LENGTH);

        let reader = tokio::spawn(reader_task(read_half, shared.clone(), max_record_size));
        tokio::spawn(writer_task(
            write_half,
            write_queue,
            shared.clone(),
            reader.abort_handle(),
        ));

        Self { writer, shared }
    }

    /// Do the RPC call described by `call`, and wait for its result. It returns either that
    /// result as a byte vector (which the caller can decode), or an error.
    ///
    /// `arg` must always be passed, but a zero-length slice can be used for a procedure which
    /// takes no arguments.
    ///
    /// This waits for as long as the connection stays open. To give up earlier, wrap the call in
    /// `tokio::time::timeout()`: it is always safe to drop the returned future.
    pub async fn call(&self, call: CallBody, arg: &[u8]) -> Result<Vec<u8>, Error> {
        let (waiter, reply) = oneshot::channel();

        let xid = {
            let mut state = self.shared.state.lock().unwrap();

            if state.closed {
                return Err(Error::ConnectionClosed);
            }
            state.pending.insert(waiter)
        };

        // From here on, the call is forgotten if this future is dropped before the reply arrives:
        let _forget = ForgetOnDrop {
            shared: &self.shared,
            xid,
        };

        let record = encode_call(xid, call, arg);

        if self.writer.send(record).await.is_err() {
            return Err(Error::ConnectionClosed);
        }

        match reply.await {
            Ok(result) => result,
            Err(_) => Err(Error::ConnectionClosed),
        }
    }

    /// The number of replies received that did not match any call that was waiting for a reply.
    /// These are replies that arrived after the caller gave up, duplicates, or replies with a
    /// transaction ID that was never sent.
    pub fn stale_replies(&self) -> u64 {
        self.shared.stale_replies.load(Ordering::Relaxed)
    }
}

/// Removes a call from the pending calls when dropped. This is a no-op if the reply was already
/// received.
struct ForgetOnDrop<'a> {
    shared: &'a Shared,
    xid: u32,
}

impl Drop for ForgetOnDrop<'_> {
    fn drop(&mut self) {
        self.shared.state.lock().unwrap().pending.remove(self.xid);
    }
}

/// Reads one complete record, reassembling it if it is fragmented. Returns None if the stream
/// ended in between two records.
async fn read_record<R: AsyncRead + Unpin>(
    stream: &mut R,
    max_record_size: usize,
) -> std::io::Result<Option<Vec<u8>>> {
    let mut assembler = RecordAssembler::new(max_record_size);

    loop {
        let mut mark = [0; RECORD_MARK_SIZE];

        match stream.read_exact(&mut mark).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof && assembler.is_empty() => {
                return Ok(None);
            }
            Err(e) => return Err(e),
        }

        let mark = RecordMark::decode(&mark);

        let fragment = assembler
            .next_fragment(mark)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        stream.read_exact(fragment).await?;

        if let Some(record) = assembler.take() {
            return Ok(Some(record));
        }
    }
}

async fn reader_task<R: AsyncRead + Unpin>(
    mut stream: R,
    shared: Arc<Shared>,
    max_record_size: usize,
) {
    loop {
        match read_record(&mut stream, max_record_size).await {
            Ok(Some(record)) => dispatch(&shared, record),
            Ok(None) => break,
            Err(e) => {
                warn!("Error reading from stream: {e}");
                break;
            }
        }
    }

    shared.close();
}

/// Hands a received record to the call that is waiting for it.
fn dispatch(shared: &Shared, mut record: Vec<u8>) {
    let (xid, result) = match RpcMessage::deserialize(&record) {
        Ok((message, result)) => match message.body {
            RpcMessageBody::Reply(reply) if reply.is_success() => {
                (message.xid, Ok(record.len() - result.len()))
            }
            RpcMessageBody::Reply(reply) => (message.xid, Err(Error::Rpc(reply))),
            RpcMessageBody::Call(_) => {
                // TODO: this is where calls from the server on a backchannel would be handled.
                debug!("Ignoring a CALL sent by the server");
                return;
            }
        },
        Err(_) => {
            let Some(xid) = peek_xid(&record) else {
                warn!("Dropping a record that is too short to be a reply");
                return;
            };
            (xid, Err(Error::Protocol(ProtocolError::Decode)))
        }
    };

    let Some(waiter) = shared.state.lock().unwrap().pending.remove(xid) else {
        debug!("REPLY with XID {xid} which no call is waiting for");
        shared.stale_replies.fetch_add(1, Ordering::Relaxed);
        return;
    };

    // The entire header has been decoded, so the rest of the record is the return value of the
    // RPC call:
    //
    // TODO: drain() moves the whole result to the front of the buffer. Returning a type that owns
    // the record and remembers where the result starts would avoid that.
    let result = result.map(|header_length| {
        record.drain(..header_length);
        record
    });

    let _ = waiter.send(result);
}

async fn writer_task<W: AsyncWrite + Unpin>(
    mut stream: W,
    mut queue: mpsc::Receiver<Vec<u8>>,
    shared: Arc<Shared>,
    reader: AbortHandle,
) {
    // This loop ends succesfully once every Client has been dropped:
    while let Some(record) = queue.recv().await {
        let res = match stream.write_all(&record).await {
            Ok(()) => stream.flush().await,
            Err(e) => Err(e),
        };

        if let Err(e) = res {
            warn!("Error writing to stream: {e}");
            break;
        }
    }

    // Either way the connection is finished. Stopping the reader task is what closes it, because
    // the reader task owns the other half of the stream.
    shared.close();
    let _ = stream.shutdown().await;
    reader.abort();
}
