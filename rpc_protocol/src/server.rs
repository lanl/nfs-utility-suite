// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use log::*;

use crate::*;

/// An RPC Procedure implementation takes a reference to the call body for the request (mainly
/// useful in case it needs to inspect the credential, for example) as well as a reference to the
/// encoded argument to the procedure. It returns a result which may be either succesful, and
/// contains the encoded response, or unsuccesful.
pub type RpcProcedure<T> = fn(&CallBody, &[u8], &mut T) -> RpcResult;

/// An RPC procedure implementation is permitted to return these results.
pub enum RpcResult {
    /// A succesful result includes the encoded value of the reply.
    Success(Vec<u8>),

    /// The procedure implementation determined that the arguments were invalid.
    GarbageArgs,

    /// The procedure implementation had an internal error (e.g., out of memory).
    SystemErr,
}

/// An RPC Service is defined by its program and version numbers, and a map from procedure numbers
/// to the actual procedures which implement them. The private state is shared by each procedure
/// implementation in the service.
///
/// TODO: allow multiple prog/vers/procedure sets to coexist in one RpcService (sharing a single
/// state).
pub struct RpcService<T> {
    /// The program number of this RPC service.
    program: u32,

    /// The version number of this RPC service.
    version: u32,

    /// The mapping of procedure numbers to functions that implement the procedures.
    /// The 0th element of this array is ignored because it is always mapped to the NULL procedure.
    procedures: Vec<Option<RpcProcedure<T>>>,

    /// The RPC service implementation can use this field to store state that must be maintained
    /// across RPC calls.
    private_state: T,
}

impl<T> RpcService<T> {
    pub fn new(
        program: u32,
        version: u32,
        procedures: Vec<Option<RpcProcedure<T>>>,
        private_state: T,
    ) -> Self {
        Self {
            program,
            version,
            procedures,
            private_state,
        }
    }

    /// Run a blocking TCP server for this RPC service using the given TcpListener.
    pub fn run_blocking_tcp_server(&mut self, listener: std::net::TcpListener) {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let _ = self.handle_connection(stream);
                }
                Err(e) => eprintln!("Error accepting connection: {e}"),
            }
        }
    }

    /// Tries to handle a given stream by reading a series of RPC Call messages from it, and
    /// passing those calls off to the appropriate implementation function to handle. If any errors
    /// are encountered, the function returns and the stream is dropped.
    ///
    /// TODO: this function can be enhanced to send the appropriate kind of reply message to
    /// respond to some error conditions, rather than dropping the connection.
    fn handle_connection(&mut self, mut stream: std::net::TcpStream) -> Result<(), crate::Error> {
        loop {
            let message_length = decode_record_mark(&mut stream)?;
            trace!("got message with record mark: {message_length}");

            let mut buf = vec![0; message_length as usize];
            stream
                .read_exact(&mut buf)
                .inspect_err(|e| warn!("Error reading message from stream: {e}"))?;

            let mut message = RpcMessage::default();
            let mut rest = buf.as_slice();
            if let Err(e) = RpcMessage::deserialize(&mut message, &mut rest) {
                eprintln!("Error deserializing message: {e}");
                return Err(Error::Protocol(ProtocolError::Decode));
            }

            // The client better have sent a "call" message:
            let RpcMessageBody::Call(call) = message.body else {
                return Err(Error::Protocol(ProtocolError::Decode));
            };

            // The RPC version must always be 2:
            if call.rpcvers != RPC_VERSION {
                // This could reply with a "RpcMismatch" reply instead...
                return Err(Error::Protocol(ProtocolError::WrongRpcVersion));
            }

            // This implementation currently only supports auth styles "None" and "Sys":
            match call.cred.flavor {
                AuthFlavor::None => {}
                AuthFlavor::Sys => {}
                // This could reply with an "AuthError" reply instead...
                _ => return Err(Error::Protocol(ProtocolError::UnsupportedAuth)),
            };

            debug!(
                "recieved CALL for program {}, version {}, procedure {}",
                call.prog, call.vers, call.proc
            );

            if call.prog != self.program {
                // should reply PROG_UNAVAIL
                todo!();
            }

            if call.vers != self.version {
                // should reply PROG_MISMATCH
                todo!();
            }

            if call.proc as usize > self.procedures.len() - 1 {
                // should reply PROC_UNAVAIL
                todo!();
            }

            if call.proc == 0 {
                self.null_procedure();
            }

            // Get the appropriate implementation from the procedures array, or if there is no
            // procedure for the requested proc number, then TODO: return an error:
            let Some(procedure) = self.procedures[call.proc as usize] else {
                // should return PROC_UNAVAIL
                todo!();
            };

            let res = procedure(&call, rest, &mut self.private_state);

            let _ = match res {
                RpcResult::Success(data) => send_accepted_reply(
                    &mut stream,
                    message.xid,
                    AcceptedReplyBody::Success([0u8; 0]),
                    Some(&data),
                ),
                // can reply with either GARBAGE_ARGS, SYSTEM_ERR, or SUCCESS
                _ => todo!(),
            };
        }
    }

    fn null_procedure(&self) {
        todo!();
    }
}

/// Given the reply body, encode it and send it on the given TcpStream.
///
/// XXX: can the protocol definition be adjusted so that AcceptedReplyBody::Success(_) holds
/// arg instead of needing to split out arg into a separate Option?
///
/// TODO: currently hard-coded to use auth "None"--this will have to be updated to use the
/// correct kind of auth based on the call.
fn send_accepted_reply(
    stream: &mut TcpStream,
    xid: u32,
    reply_data: AcceptedReplyBody,
    arg: Option<&[u8]>,
) -> Result<(), Box<dyn std::error::Error>> {
    let body = RpcMessageBody::Reply(ReplyBody::Accepted(AcceptedReply {
        verf: OpaqueAuth::none(),
        reply_data,
    }));

    let message = RpcMessage { xid, body };

    let mut buf = buf_with_dummy_record_mark();
    buf.append(&mut message.serialize_alloc());

    if let Some(arg) = arg {
        let RpcMessageBody::Reply(ReplyBody::Accepted(acc)) = message.body else {
            panic!("Must be accepted reply if an argument is passed");
        };

        let AcceptedReplyBody::Success(_) = acc.reply_data else {
            panic!("Must be accepted succesful reply if an argument is passed");
        };

        // It is illegal to pass an argument that is not padded to a multiple of 4 bytes:
        assert_eq!(0, arg.len() % 4);

        buf.extend_from_slice(arg);
    }

    crate::update_record_mark(&mut buf);

    stream.write_all(&buf)?;

    Ok(())
}
