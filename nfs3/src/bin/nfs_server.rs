// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use rpc_protocol::server::ring::*;

fn main() {
    env_logger::init();

    let mut server = RpcServer::new("127.0.0.1:2049").unwrap();

    server.main_loop().unwrap();
}
