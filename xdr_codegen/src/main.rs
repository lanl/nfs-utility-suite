// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    /// Whether to generate non-allocating serialization routines.
    #[arg(short, long)]
    no_alloc: bool,
}

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut compiler = xdr_codegen::Compiler::new();
    if args.no_alloc {
        compiler.enable_no_alloc().disable_alloc().run()
    } else {
        compiler.run()
    }
}
