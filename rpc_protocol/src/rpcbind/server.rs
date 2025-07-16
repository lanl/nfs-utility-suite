// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

#![allow(non_camel_case_types)]

use log::*;

use std::ffi::OsString;
use std::net::TcpListener;
use std::os::unix::net::UnixListener;

use crate::rpcbind::{self, procedures::*, RpcbindServerAddress};
use crate::server::*;
use crate::*;

pub fn main(addr: RpcbindServerAddress) {
    let service_list = default_service_list();

    let procedures: Vec<Option<RpcProcedure<rpcbind::RpcbindList>>> =
        vec![None, Some(set), Some(unset), Some(getaddr), Some(dump)];
    let mut server = RpcService::new(RPCBPROG, RPCBVERS::VERSION, procedures, service_list);

    match addr {
        RpcbindServerAddress::Tcp(addr) => {
            let listener = TcpListener::bind(addr).unwrap();
            server.run_blocking_tcp_server(listener);
        }
        RpcbindServerAddress::Unix(addr) => {
            // Not necessary to check for errors in remove_file() because ENOENT is expected, and
            // a failure to remove the file (while it already exists) will result in an error in
            // bind().
            let _ = std::fs::remove_file(&addr);
            let listener = UnixListener::bind(addr).unwrap();
            server.run_blocking_tcp_server(listener);
        }
    }
}

/// Implementation of the getaddr RPC. This loops over the `service_list` to see if the service
/// requested in the `arg` is in the list, and returns its address if so. Otherwise, it returns an
/// empty string.
fn getaddr(_call: &CallBody, mut arg: &[u8], service_list: &mut rpcbind::RpcbindList) -> RpcResult {
    let mut requested = rpcbind::RpcService::default();
    rpcbind::RpcService::deserialize(&mut requested, &mut arg).unwrap();
    debug!("GETADDR Call: {requested:?}");

    if let Some(service) = get_service(requested.prog, requested.vers, service_list) {
        let address = rpcbind::RpcbString {
            contents: service.addr.clone(),
        };

        return RpcResult::Success(rpcbind::RpcbString::serialize_alloc(&address));
    }

    let empty = rpcbind::RpcbString {
        contents: std::ffi::OsString::from(""),
    };

    RpcResult::Success(empty.serialize_alloc())
}

/// Implementation of the set RPC. This adds a service to the list.
fn set(_call: &CallBody, arg: &[u8], service_list: &mut rpcbind::RpcbindList) -> RpcResult {
    let mut new_service = rpcbind::RpcService::default();
    let mut arg = arg;
    if let Err(_) = new_service.deserialize(&mut arg) {
        return RpcResult::GarbageArgs;
    }

    // Make sure that this service is not already registered:
    if get_service(new_service.prog, new_service.vers, service_list).is_some() {
        // If it is, return False to the caller:
        return RpcResult::Success(vec![0, 0, 0, 0]);
    }

    if new_service.netid.is_empty() || new_service.addr.is_empty() {
        // According to the RFC, empty netid and address are not allowed.
        return RpcResult::Success(vec![0, 0, 0, 0]);
    }

    service_list.items.push(rpcbind::RpcbindItem {
        rpcb_map: new_service,
    });

    RpcResult::Success(vec![0, 0, 0, 1])
}

/// Implementation of the unset RPC. This removes a service from the list.
fn unset(_call: &CallBody, _arg: &[u8], _service_list: &mut rpcbind::RpcbindList) -> RpcResult {
    todo!()
}

/// Implementation of the dump RPC. This returns the entire known `service_list`.
fn dump(_call: &CallBody, _arg: &[u8], service_list: &mut rpcbind::RpcbindList) -> RpcResult {
    let data = service_list.serialize_alloc();

    RpcResult::Success(data)
}

fn default_service_list() -> rpcbind::RpcbindList {
    let item = rpcbind::RpcbindItem {
        rpcb_map: rpcbind::RpcService {
            prog: 100000,
            vers: 3,
            netid: OsString::from("tcp"),
            addr: OsString::from("0.0.0.0.111"),
            owner: OsString::from("superuser"),
        },
    };

    rpcbind::RpcbindList { items: vec![item] }
}

/// Returns the service specified by `program` and `version` from the `service_list`, or none if
/// there is no match.
fn get_service(
    program: u32,
    version: u32,
    service_list: &rpcbind::RpcbindList,
) -> Option<&rpcbind::RpcService> {
    for service in &service_list.items {
        let service = &service.rpcb_map;

        if program != service.prog {
            continue;
        }

        if version != service.vers {
            continue;
        }

        return Some(service);
    }

    return None;
}
