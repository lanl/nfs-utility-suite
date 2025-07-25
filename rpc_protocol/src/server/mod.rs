// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use log::*;

use crate::*;

pub mod ring;

/// An RPC Procedure implementation takes a reference to the call body for the request (mainly
/// useful in case it needs to inspect the credential, for example) as well as a reference to the
/// encoded argument to the procedure. It returns a result which may be either succesful, and
/// contains the encoded response, or unsuccesful.
pub type RpcProcedure<T> = fn(&CallBody, &[u8], &mut T) -> RpcResult;

/// The NULL Procedure is defined for every service and does nothing, succesfully.
pub fn null_procedure<T>(_call: &CallBody, _arg: &[u8], _state: &mut T) -> RpcResult {
    RpcResult::Success(vec![])
}

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
pub struct RpcService<T> {
    /// The program number of this RPC service.
    program: u32,

    /// The min version number of this RPC service.
    version_min: u32,

    /// The max version number of this RPC service.
    version_max: u32,

    /// The mapping of procedure numbers to functions that implement the procedures.
    /// The 0th element of this array is ignored because it is always mapped to the NULL procedure.
    /// This structure assumes that al the versions between version_min and version_max share the
    /// same procedures. If that assumption should turn false in the future, this structure will
    /// have to be modified.
    procedures: Vec<Option<RpcProcedure<T>>>,

    /// The RPC service implementation can use this field to store state that must be maintained
    /// across RPC calls.
    private_state: T,
}

/// A trait that allows functions to be generic over both TcpListener and UnixListener.
pub trait Listener<S> {
    fn accept(&self) -> std::io::Result<S>;
}

impl Listener<std::net::TcpStream> for std::net::TcpListener {
    fn accept(&self) -> std::io::Result<std::net::TcpStream> {
        Ok(self.accept()?.0)
    }
}

impl Listener<std::os::unix::net::UnixStream> for std::os::unix::net::UnixListener {
    fn accept(&self) -> std::io::Result<std::os::unix::net::UnixStream> {
        Ok(self.accept()?.0)
    }
}

impl<T> RpcService<T> {
    pub fn new(
        program: u32,
        version_min: u32,
        version_max: u32,
        procedures: Vec<Option<RpcProcedure<T>>>,
        private_state: T,
    ) -> Self {
        Self {
            program,
            version_min,
            version_max,
            procedures,
            private_state,
        }
    }

    /// Run a blocking TCP server for this RPC service using the given Listener.
    pub fn run_blocking_tcp_server<S: Read + Write>(&mut self, listener: impl Listener<S>) {
        loop {
            match listener.accept() {
                Ok(stream) => {
                    let _ = self.handle_connection(stream);
                }
                Err(e) => warn!("Error accepting connection: {e}"),
            }
        }
    }

    /// Tries to handle a given stream by reading a series of RPC Call messages from it, and
    /// passing those calls off to the appropriate implementation function to handle. If any errors
    /// are encountered, the function returns and the stream is dropped.
    pub fn handle_connection<S: Read + Write>(
        &mut self,
        mut stream: S,
    ) -> Result<(), crate::Error> {
        loop {
            let message_length = stream_record_mark(&mut stream)?;
            trace!("got message with record mark: {message_length}");

            let mut buf = vec![0; message_length as usize];
            stream
                .read_exact(&mut buf)
                .inspect_err(|e| warn!("Error reading message from stream: {e}"))?;

            let mut message = RpcMessage::default();
            let mut rest = buf.as_slice();
            if let Err(e) = message.deserialize(&mut rest) {
                warn!("Error deserializing message: {e}");
                return Err(Error::Protocol(ProtocolError::Decode));
            }

            // The client better have sent a "call" message:
            let RpcMessageBody::Call(call) = message.body else {
                return Err(Error::Protocol(ProtocolError::Decode));
            };

            debug!(
                "recieved CALL for program {}, version {}, procedure {}, argument length {} bytes",
                call.prog,
                call.vers,
                call.proc,
                rest.len(),
            );

            let procedure = match self.validate_call(&call) {
                Ok(proc) => proc,
                Err(e) => {
                    if let Error::Rpc(reply) = e {
                        send_reply_no_arg(&mut stream, message.xid, reply)?;
                    }

                    return Ok(());
                }
            };

            let res = procedure(&call, rest, &mut self.private_state);

            let _ = match res {
                RpcResult::Success(data) => {
                    send_succesful_reply(&mut stream, message.xid, Some(&data))
                }
                // can reply with either GARBAGE_ARGS, SYSTEM_ERR, or SUCCESS
                _ => todo!(),
            };
        }
    }

    /// Given an RPC call, checks if it is a valid call for this service. If so returns the
    /// procedure which implements that call.
    ///
    /// Otherwise, returns the appropiate kind of error.
    fn validate_call(&self, call: &CallBody) -> Result<RpcProcedure<T>, Error> {
        // The RPC version must always be 2:
        if call.rpcvers != RPC_VERSION {
            warn!("CALL with unexpected RPC version {}", call.rpcvers);
            // This could reply with a "RpcMismatch" reply instead...
            return Err(Error::Protocol(ProtocolError::WrongRpcVersion));
        }

        // This implementation currently only supports auth styles "None" and "Sys":
        match call.cred.flavor {
            AuthFlavor::None => {}
            AuthFlavor::Sys => {}
            _ => {
                debug!("CALL with unsupported auth: {:?}", call.cred);
                let reply = ReplyBody::Denied(RejectedReply::AuthError(AuthStat::RejectedCred));
                return Err(crate::Error::Rpc(reply));
            }
        };

        if call.prog != self.program {
            debug!("CALL for unknown program {}", call.prog);
            let reply = ReplyBody::accepted_reply(AcceptedReplyBody::ProgUnavail);
            return Err(crate::Error::Rpc(reply));
        }

        if call.vers < self.version_min || call.vers > self.version_max {
            debug!("CALL for unknown version {}", call.vers);
            let reply =
                ReplyBody::accepted_reply(AcceptedReplyBody::ProgMismatch(ProgMismatchBody {
                    low: self.version_min,
                    high: self.version_max,
                }));
            return Err(crate::Error::Rpc(reply));
        }

        if call.proc == 0 {
            return Ok(null_procedure);
        }

        if call.proc as usize > self.procedures.len() - 1 {
            debug!("CALL for unknown procedure {}", call.proc);
            let reply = ReplyBody::accepted_reply(AcceptedReplyBody::ProcUnavail);
            return Err(crate::Error::Rpc(reply));
        }

        let Some(procedure) = self.procedures[call.proc as usize] else {
            debug!("CALL for unimplemented procedure {}", call.proc);
            let reply = ReplyBody::accepted_reply(AcceptedReplyBody::ProcUnavail);
            return Err(crate::Error::Rpc(reply));
        };

        Ok(procedure)
    }
}

/// Write a reply to the stream without encoding any procedure result (for example, an error reply).
fn send_reply_no_arg<S: Read + Write>(
    stream: &mut S,
    xid: u32,
    reply_data: ReplyBody,
) -> Result<(), crate::Error> {
    let message = RpcMessage {
        xid,
        body: RpcMessageBody::Reply(reply_data),
    };

    let mut buf = buf_with_dummy_record_mark();
    buf.append(&mut message.serialize_alloc());
    crate::update_record_mark(&mut buf);

    stream.write_all(&buf)?;

    Ok(())
}

impl ReplyBody {
    pub fn accepted_reply(reply_data: AcceptedReplyBody) -> Self {
        ReplyBody::Accepted(AcceptedReply {
            verf: OpaqueAuth::none(),
            reply_data,
        })
    }
}

/// Given the reply body, encode it and send it on the given TcpStream.
///
/// XXX: can the protocol definition be adjusted so that AcceptedReplyBody::Success(_) holds
/// arg instead of needing to split out arg into a separate Option?
///
/// TODO: currently hard-coded to use auth "None"--this will have to be updated to use the
/// correct kind of auth based on the call.
fn send_succesful_reply<S: Read + Write>(
    stream: &mut S,
    xid: u32,
    arg: Option<&[u8]>,
) -> Result<(), crate::Error> {
    let body = RpcMessageBody::Reply(ReplyBody::accepted_reply(AcceptedReplyBody::Success(
        [0u8; 0],
    )));

    let message = RpcMessage { xid, body };

    let mut buf = buf_with_dummy_record_mark();
    buf.append(&mut message.serialize_alloc());

    if let Some(arg) = arg {
        // It is illegal to pass an argument that is not padded to a multiple of 4 bytes:
        assert_eq!(0, arg.len() % 4);

        buf.extend_from_slice(arg);
    }

    crate::update_record_mark(&mut buf);

    stream.write_all(&buf)?;

    Ok(())
}
