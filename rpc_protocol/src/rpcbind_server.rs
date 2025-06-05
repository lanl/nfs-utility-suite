// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

#![allow(non_camel_case_types)]

use std::ffi::OsString;
use std::net::TcpListener;

use crate::server::*;
use crate::*;

include!(concat!(env!("OUT_DIR"), "/rpcbind.rs"));

use rpcbind::procedures::*;

pub fn main() {
    let service_list = default_service_list();

    let procedures: Vec<Option<RpcProcedure<rpcbind::RpcbindList>>> = vec![
        None,
        None, // set()
        None, // unset()
        Some(getaddr),
        Some(dump),
    ];
    let mut server = RpcService::new(RPCBPROG, RPCBVERS::VERSION, procedures, service_list);

    let listener = TcpListener::bind("0.0.0.0:111").unwrap();
    server.run_blocking_tcp_server(listener);
}

/// Implementation of the getaddr RPC. This loops over the `service_list` to see if the service
/// requested in the `arg` is in the list, and returns its address if so. Otherwise, it returns an
/// empty string.
fn getaddr(_call: &CallBody, mut arg: &[u8], service_list: &mut rpcbind::RpcbindList) -> RpcResult {
    let mut requested = rpcbind::RpcService::default();
    rpcbind::RpcService::deserialize(&mut requested, &mut arg).unwrap();
    eprintln!("{:?}", requested);

    for service in service_list.items.iter() {
        let service = &service.rpcb_map;

        if requested.prog != service.prog {
            continue;
        }

        if requested.vers != service.vers {
            continue;
        }

        let address = rpcbind::RpcbString {
            contents: service.addr.clone(),
        };

        let bytes = rpcbind::RpcbString::serialize_alloc(&address);

        return RpcResult::Success(bytes);
    }

    let empty = rpcbind::RpcbString {
        contents: std::ffi::OsString::from(""),
    };

    RpcResult::Success(empty.serialize_alloc())
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
