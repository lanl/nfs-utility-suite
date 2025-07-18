// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

#![allow(non_camel_case_types)]

use rpc_protocol::rpcbind::{self, RpcbindServerAddress};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    rpcbind::server::main(RpcbindServerAddress::Tcp("0.0.0.0:111".to_string()));

    Ok(())
}
