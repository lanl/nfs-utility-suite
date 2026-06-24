// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::{io, net::TcpStream};

use clap::{Parser, Subcommand};

use ::nfs3::{nfs3_xdr::procedures::*, nfs3_xdr::*};
use nfs3::mount_proto::{
    procedures::{MOUNT_PROGRAM, MOUNT_V3},
    MountProc3Args, MountResultReader, MountResultRet,
};
use rpc_protocol::client::*;
use std::ffi::OsString;

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
    Mount {
        #[arg(short, long)]
        filename: String,
    },
}

fn main() -> io::Result<()> {
    let args = Cli::parse();
    eprintln!("{args:?}");

    let mut stream = TcpStream::connect(format!("{}:{}", args.hostname, args.port))?;

    match args.command {
        Command::Mount { filename } => do_mount(&mut stream, filename),
    }
}

fn do_mount(stream: &mut TcpStream, filename: String) -> io::Result<()> {
    let arg = MountProc3Args {
        dirpath: OsString::from(filename),
    };

    let width = arg.get_width();
    let mut buf = vec![0u8; width];
    let written = arg.serialize(buf.as_mut_slice());
    assert_eq!(written, width);

    let res = do_rpc_call(
        stream,
        MOUNT_PROGRAM,
        MOUNT_V3::VERSION,
        MOUNT_V3::MOUNTPROC3_MNT,
        buf.as_slice(),
    );

    match res {
        Ok(bytes) => {
            let reader = MountResultReader::new(bytes.as_slice()).unwrap();

            match reader.deserialize() {
                nfs3::mount_proto::MountResultRet::Ok(reader) => {
                    let data = reader.get_fhandle();
                    let (int_bytes, _) = data.split_at(8);
                    eprintln!(
                        "Success: {:?}",
                        u64::from_be_bytes(int_bytes.try_into().unwrap())
                    );
                }
                MountResultRet::Default => eprintln!("Error: mount failed"),
            }
        }
        Err(e) => {
            eprintln!("{e:?}");
        }
    };

    Ok(())
}
