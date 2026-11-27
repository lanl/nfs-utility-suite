// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::{
    net::{Ipv4Addr, SocketAddr, TcpStream},
    str::FromStr,
};

use clap::Parser;

use nfs3::mount_proto::*;
use rpc_blocking::client::*;
use rpc_protocol::{CallBody, OpaqueAuth, RPC_VERSION};
use socket2::{Domain, Protocol, Socket, Type};

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
    let server_address = SocketAddr::new(
        std::net::IpAddr::V4(Ipv4Addr::from_str(&args.hostname)?),
        args.port,
    );
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;

    socket.set_reuse_address(true)?;

    let local_addr: SocketAddr = format!("192.168.59.11:{}", 1023).parse()?;
    socket.bind(&local_addr.into())?;

    socket.connect(&server_address.into())?;

    let mut stream: TcpStream = socket.into();

    let res = do_rpc_call(
        &mut stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: procedures::MOUNT_PROGRAM,
            vers: procedures::MOUNT_V3::VERSION,
            proc: procedures::MOUNT_V3::MOUNTPROC3_EXPORT,
            cred: OpaqueAuth::none(),
            verf: OpaqueAuth::none(),
        },
        &[0u8; 0],
    )?;

    let (export_list, _) = Exports::deserialize(res.as_slice())?;

    print_exports(&args.hostname, &export_list);

    for export in export_list.inner {
        let mount_arg = MountArg {
            path: export.dir.clone(),
        };

        let res = do_rpc_call(
            &mut stream,
            CallBody {
                rpcvers: RPC_VERSION,
                prog: procedures::MOUNT_PROGRAM,
                vers: procedures::MOUNT_V3::VERSION,
                proc: procedures::MOUNT_V3::MOUNTPROC3_MNT,
                cred: OpaqueAuth::none(),
                verf: OpaqueAuth::none(),
            },
            &mount_arg.serialize_alloc(),
        )?;

        let (mount_res, _) = MountResult::deserialize(res.as_slice())?;

        match &mount_res {
            MountResult::Ok(mount_result_ok) => {
                println!("mount_ok: {:?}", mount_result_ok.fhandle)
            }
            _ => {
                println!("mount_failed: {mount_res:?}")
            }
        }
        println!("mount_res: {mount_res:?}");
    }

    Ok(())
}

fn print_exports(hostname: &str, list: &Exports) {
    println!("Export list for {hostname}:");
    for export in &list.inner {
        print!("{} ", export.dir.display());
        for group in &export.groups.inner {
            print!("{} ", group.name.display());
        }
        println!();
    }
}
