// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

pub mod client;
pub mod server;

include!(concat!(env!("OUT_DIR"), "/rpcbind.rs"));
pub use self::rpcbind::*;

/// An RPCBIND Server tends to listen both on a Unix socket and a TCP socket.
pub enum RpcbindServerAddress {
    Unix(String),
    Tcp(String),
}
