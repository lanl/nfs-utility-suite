// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::{io, net::TcpStream};

use clap::{Parser, Subcommand};

use nfs3::{nfs3_xdr::nfs3::procedures::*, nfs3_xdr::nfs3::*};
use rpc_protocol::client::*;

#[derive(Debug, Parser)]
struct Cli {
    #[arg(long, default_value = "localhost")]
    hostname: String,

    #[arg(long, default_value_t = 2049)]
    port: u16,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Perform a getattr RPC.
    Getattr {
        #[arg(short, long)]
        filehandle: u64,
    },
}

fn main() -> io::Result<()> {
    let args = Cli::parse();
    eprintln!("{args:?}");

    let mut stream = TcpStream::connect(format!("{}:{}", args.hostname, args.port))?;

    match args.command {
        Command::Getattr { filehandle } => do_getattr(&mut stream, filehandle),
    }
}

fn do_getattr(stream: &mut TcpStream, fh: u64) -> io::Result<()> {
    let arg = GetAttrArgs {
        object: FileHandle {
            data: Vec::from(fh.to_be_bytes()),
        },
    };

    let arg = arg.serialize_alloc();

    let res = do_rpc_call(stream, NFS_PROGRAM, NFS_V3::VERSION, NFS_V3::GETATTR, &arg);

    match res {
        Ok(bytes) => {
            let mut res = GetAttrResult::default();
            res.deserialize(&mut bytes.as_slice()).unwrap();
            eprintln!("Success: {res:?}");
        }
        Err(e) => {
            eprintln!("{e:?}");
        }
    };

    Ok(())
}
