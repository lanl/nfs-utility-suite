// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use xdr_rpc;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    xdr_rpc::Compiler::new().run()
}
