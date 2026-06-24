// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

#[cfg(target_os = "linux")]
use {
    clap::Parser,
    io_uring::{opcode, types},
    nfs3::{
        mount_proto::{
            procedures::{MOUNT_PROGRAM, MOUNT_V3},
            MountProc3ArgsReader, MountResult, MountResultOk,
        },
        nfs3_xdr::{procedures::*, *},
    },
    rpc_protocol::{server::RpcResult, Call},
    std::{collections::HashMap, ffi::CString, os::unix::ffi::OsStrExt},
};

#[cfg(target_os = "linux")]
mod ring;

#[cfg(target_os = "linux")]
use crate::ring::*;

#[cfg(target_os = "linux")]
#[derive(Parser)]
struct Cli {
    #[arg(long, default_value_t = 2049)]
    port: u16,
}

#[cfg(target_os = "linux")]
struct ServerState {
    fh_map: HashMap<Box<u8>, String>,
}

#[cfg(target_os = "linux")]
fn main() {
    env_logger::init();

    let args = Cli::parse();
    let address = format!("127.0.0.1:{}", args.port);

    let state = ServerState {
        fh_map: HashMap::new(),
    };

    let procedures: Vec<Option<RingProcedure<ServerState>>> = vec![None, Some(mount)];
    let procedure_map = ProcedureMap::new(
        MOUNT_PROGRAM,
        MOUNT_V3::VERSION,
        MOUNT_V3::VERSION,
        procedures,
    );

    let mut server = RpcServer::new(&address, procedure_map, state).unwrap();

    server.main_loop().unwrap();
}

#[cfg(target_os = "linux")]
fn getattr(call: &Call, _state: &mut ServerState, _connfd: i32) -> RingResult {
    let arg = call.arg;
    eprintln!("in getattr impl: {arg:?}");

    let obj_attributes = FileAttributes::default();

    let result = GetAttrResult::Ok(GetAttrSuccess { obj_attributes });

    let width = result.get_width();
    let mut buf = vec![0u8; width];
    let written = result.serialize(buf.as_mut_slice());
    assert_eq!(written, width);

    RingResult::Done(RpcResult::Success(buf))
}

#[cfg(target_os = "linux")]
fn mount(call: &Call, _state: &mut ServerState, connfd: i32, xid: u32) -> RingResult {
    let arg = call.arg;
    let Ok(mount_params) = MountProc3ArgsReader::new(arg) else {
        todo!("handle errors");
    };

    let path_cstring =
        CString::new(mount_params.get_dirpath().as_bytes()).expect("invalid mount string");

    let mut user_data = Box::new(Operation::MountStatx(Statx {
        data: unsafe { std::mem::zeroed() },
        path: path_cstring,
        connfd,
        xid,
        cb: mount_response,
    }));

    let op = match &mut *user_data {
        Operation::MountStatx(s) => opcode::Statx::new(
            types::Fd(libc::AT_FDCWD),
            s.path.as_ptr(),
            &mut s.data as *mut libc::statx as *mut _,
        ),
        _ => unreachable!(),
    };

    let op = op.build().user_data(user_data.to_u64());

    RingResult::_MoreIo(op)
}

#[cfg(target_os = "linux")]
fn mount_response(res: libc::statx, path: CString, _connfd: i32) -> RingResult {
    let res = MountResult::Ok(MountResultOk {
        fhandle: res.stx_ino.to_be_bytes().to_vec(),
        auth_flavors: vec![],
    });

    let mut data = vec![0u8; res.get_width()];
    res.serialize(data.as_mut_slice());

    RingResult::Done(RpcResult::Success(data))
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("nfs server only supported on linux.");
}
