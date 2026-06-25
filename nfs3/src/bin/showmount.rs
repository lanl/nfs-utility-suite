// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::net::TcpStream;

use clap::Parser;

use nfs3::mount_proto::*;
use rpc_protocol::client::*;

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "localhost")]
    hostname: String,

    #[arg(long, default_value_t = 20048)]
    port: u16,

    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();
    let server_address = format!("{}:{}", args.hostname, args.port);
    let mut stream = TcpStream::connect(&server_address)?;

    let res = do_rpc_call(
        &mut stream,
        procedures::MOUNT_PROGRAM,
        procedures::MOUNT_V3::VERSION,
        procedures::MOUNT_V3::MOUNTPROC3_EXPORT,
        &[0u8; 0],
    )?;

    let exports = ExportsReader::new(res.as_slice())?;

    print_exports(&args.hostname, exports.get_inner());

    Ok(())
}

fn print_exports<'a>(
    hostname: &str,
    list: impl Iterator<Item = xdr_lib::Result<ExportNodeReader<'a>>>,
) {
    println!("Export list for {hostname}:");
    for export in list {
        let export = export.expect("Failed to deserialize node reader");
        print!("{} ", export.get_dir().display());
        for group in export.get_groups().get_inner() {
            print!("{} ", group.unwrap().get_name().display());
        }
        println!();
    }
}
