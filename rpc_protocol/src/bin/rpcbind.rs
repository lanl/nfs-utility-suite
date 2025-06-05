// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

#![allow(non_camel_case_types)]

use rpc_protocol::rpcbind_server;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    rpcbind_server::main();

    Ok(())
}
