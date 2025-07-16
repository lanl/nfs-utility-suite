// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::os::unix::net::UnixStream;

use rpc_protocol::*;

use rpcbind::RpcbindServerAddress;

#[test]
fn set_and_getaddr() {
    std::thread::spawn(|| {
        rpcbind::server::main(RpcbindServerAddress::Unix("rpcbind.socket".to_string()));
    });

    let mut stream = wait_for_server("rpcbind.socket");

    let new_service = rpcbind::RpcService {
        prog: 12345,
        vers: 7,
        netid: "example_netid".into(),
        addr: "example_addr".into(),
        owner: "example_owner".into(),
    };

    let res = rpcbind::client::set_using_stream(new_service, &mut stream).unwrap();
    assert!(res);

    let target_service = rpcbind::RpcService {
        prog: 12345,
        vers: 7,
        netid: "".into(),
        addr: "".into(),
        owner: "".into(),
    };

    let res = rpcbind::client::getaddr_using_stream(target_service, &mut stream).unwrap();

    assert_eq!(res, std::ffi::OsString::from("example_addr"));
}

fn wait_for_server(addr: &str) -> UnixStream {
    let mut counter = 20;
    while counter > 0 {
        match UnixStream::connect(addr) {
            Ok(stream) => return stream,
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(10)),
        };
        counter -= 1;
    }

    panic!("Timeout trying to connect to unix domain socket at {addr}");
}
