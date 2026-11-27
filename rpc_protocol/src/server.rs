// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use log::*;

use crate::*;

/// An RPC Procedure implementation takes a reference to the RPC call information for the request
/// which allows it to inspect the credential, and also contains the encoded argument to the
/// procedure. It returns a result which may be either succesful, and contains the encoded response,
/// or unsuccesful.
pub type RpcProcedure<T> = fn(&Call, &mut T) -> RpcResult;

/// The NULL Procedure is defined for every service and does nothing, succesfully.
pub fn null_procedure<T>(_call: &Call, _state: &mut T) -> RpcResult {
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
pub struct RpcProgram<T> {
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

impl<T> RpcProgram<T> {
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

    /// Given a complete record (without the record mark) that holds an RPC call, passes the call
    /// off to the appropriate implementation function and returns the reply to send back, encoded
    /// as a record that begins with a record mark.
    ///
    /// A call which is not valid for this service, or which the procedure implementation could
    /// not handle, still results in a reply, which tells the caller what was wrong.
    ///
    /// An error means that the record could not be understood as a call at all, in which case
    /// there is nothing to reply to and the connection should be closed.
    pub fn handle_call(&mut self, record: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let call = decode_call(record)?;

        let procedure = match self.validate_call(&call) {
            Ok(proc) => proc,
            Err(reply) => return Ok(encode_reply_no_arg(call.xid, reply)),
        };

        let reply = match procedure(&call, &mut self.private_state) {
            RpcResult::Success(data) => encode_succesful_reply(call.xid, &data),
            RpcResult::GarbageArgs => encode_reply_no_arg(
                call.xid,
                ReplyBody::accepted_reply(AcceptedReplyBody::GarbageArgs),
            ),
            RpcResult::SystemErr => encode_reply_no_arg(
                call.xid,
                ReplyBody::accepted_reply(AcceptedReplyBody::SystemErr),
            ),
        };

        Ok(reply)
    }

    /// Given an RPC call, checks if it is a valid call for this service. If so returns the
    /// procedure which implements that call.
    ///
    /// Otherwise, returns the error reply to send back.
    fn validate_call(&self, call: &Call) -> Result<RpcProcedure<T>, ReplyBody> {
        validate_program_and_version(call, self.program, self.version_min, self.version_max)?;

        let procedure_number = call.header.proc;

        if procedure_number == 0 {
            return Ok(null_procedure);
        }

        if procedure_number as usize > self.procedures.len() - 1 {
            debug!("CALL for unknown procedure {procedure_number}");
            return Err(ReplyBody::accepted_reply(AcceptedReplyBody::ProcUnavail));
        }

        let Some(procedure) = self.procedures[procedure_number as usize] else {
            debug!("CALL for unimplemented procedure {procedure_number}");
            return Err(ReplyBody::accepted_reply(AcceptedReplyBody::ProcUnavail));
        };

        Ok(procedure)
    }
}

/// Checks that the call uses a supported kind of authentication and is for the given program
/// and range of versions. If not, returns the error reply to send back.
pub fn validate_program_and_version(
    call: &Call,
    program: u32,
    version_min: u32,
    version_max: u32,
) -> Result<(), ReplyBody> {
    // This implementation currently only supports auth styles "None" and "Sys":
    let credential = &call.header.cred;

    match credential.flavor {
        AuthFlavor::None => {}
        AuthFlavor::Sys => {}
        _ => {
            debug!("CALL with unsupported auth: {credential:?}");
            return Err(ReplyBody::Denied(RejectedReply::AuthError(
                AuthStat::RejectedCred,
            )));
        }
    };

    let call_prog = call.header.prog;
    if call_prog != program {
        debug!("CALL for unknown program {call_prog}");
        return Err(ReplyBody::accepted_reply(AcceptedReplyBody::ProgUnavail));
    }

    let version = call.header.vers;
    if version < version_min || version > version_max {
        debug!("CALL for unknown version {version}");
        return Err(ReplyBody::accepted_reply(AcceptedReplyBody::ProgMismatch(
            ProgMismatchBody {
                low: version_min,
                high: version_max,
            },
        )));
    }
    Ok(())
}

/// Encode a reply without any procedure result (for example, an error reply).
pub fn encode_reply_no_arg(xid: u32, reply_data: ReplyBody) -> Vec<u8> {
    let message = RpcMessage {
        xid,
        body: RpcMessageBody::Reply(reply_data),
    };

    let mut buf = buf_with_dummy_record_mark();
    buf.append(&mut message.serialize_alloc());
    crate::update_record_mark(&mut buf);

    buf
}

impl ReplyBody {
    pub fn accepted_reply(reply_data: AcceptedReplyBody) -> Self {
        ReplyBody::Accepted(AcceptedReply {
            verf: OpaqueAuth::none(),
            reply_data,
        })
    }
}

/// Encode a succesful reply which carries `arg`, the encoded result of the procedure.
///
/// XXX: can the protocol definition be adjusted so that AcceptedReplyBody::Success(_) holds
/// arg instead of needing to split out arg into a separate Option?
///
/// TODO: currently hard-coded to use auth "None"--this will have to be updated to use the
/// correct kind of auth based on the call.
pub fn encode_succesful_reply(xid: u32, arg: &[u8]) -> Vec<u8> {
    let body = RpcMessageBody::Reply(ReplyBody::accepted_reply(AcceptedReplyBody::Success(
        [0u8; 0],
    )));

    let message = RpcMessage { xid, body };

    let mut buf = buf_with_dummy_record_mark();
    buf.append(&mut message.serialize_alloc());

    // It is illegal to pass an argument that is not padded to a multiple of 4 bytes:
    assert_eq!(0, arg.len() % 4);

    buf.extend_from_slice(arg);

    crate::update_record_mark(&mut buf);

    buf
}
