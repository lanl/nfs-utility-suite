// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    xdr_codegen::Compiler::new().run()
}
