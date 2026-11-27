// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::{
    io::{self, Read},
    net::TcpStream,
};

use clap::{builder::PossibleValue, Args, Parser, Subcommand, ValueEnum};

use ::nfs3::{nfs3_xdr::procedures::*, nfs3_xdr::*};
use nfs3::{nfs3_xdr::Sattr, *};
use rpc_blocking::client::*;
use rpc_protocol::{rpc_prot::authsys_parms, CallBody, OpaqueAuth, RPC_VERSION};

#[derive(ValueEnum, Clone, Debug)]
enum Sec {
    Sys,
    None,
}

#[derive(Args, Debug)]
struct SysSecArgs {
    #[arg(long, default_value_t = 1000)]
    uid: u32,

    #[arg(long, default_value_t = 1000)]
    gid: u32,

    #[arg(long)]
    groups: Vec<u32>,

    #[arg(long, default_value_t = 42)]
    stamp: u32,

    #[arg(long, default_value = "nfs_cli")]
    machine_name: String,
}

use socket2::{Domain, Protocol, Socket, Type};

use std::{
    net::{Ipv4Addr, SocketAddr},
    str::FromStr,
};

#[derive(Debug, Parser)]
struct Cli {
    #[arg(long, default_value = "127.0.0.1")]
    hostname: String,

    #[arg(long, default_value_t = 2049)]
    port: u16,

    #[arg(long)]
    sec: Sec,

    #[command(flatten)]
    syssec: SysSecArgs,

    #[arg(long, default_value_t = 1023)]
    client_port: u16,

    #[arg(long, default_value = "127.0.0.1")]
    client_addr: String,

    #[arg(long)]
    mount_target: String,

    #[arg(long)]
    pretty: bool,

    #[clap(subcommand)]
    command: Command,
}

fn parse_hex(s: &str) -> Result<Vec<u8>, hex::FromHexError> {
    hex::decode(s)
}

fn parse_octal(s: &str) -> Result<u32, std::num::ParseIntError> {
    u32::from_str_radix(s, 8)
}

type Bytes = Vec<u8>;

#[derive(Debug, Clone, Copy)]
enum SetTime {
    Set(u32, u32),
    Server,
}

impl FromStr for SetTime {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lower = s.to_lowercase();
        match lower.as_str() {
            "server" => Ok(SetTime::Server),
            _ => {
                // Expecting a format like "10,20" or "set:10,20"
                // Clean up prefixes if present (e.g., "set:10,20" -> "10,20")
                let clean_s = lower.strip_prefix("set:").unwrap_or(&lower);

                let parts: Vec<&str> = clean_s.split(',').collect();
                if parts.len() == 2 {
                    let a = parts[0]
                        .trim()
                        .parse::<u32>()
                        .map_err(|_| "First integer invalid")?;
                    let b = parts[1]
                        .trim()
                        .parse::<u32>()
                        .map_err(|_| "Second integer invalid")?;
                    return Ok(SetTime::Set(a, b));
                }

                Err("Expected 'server', 'none', or two integers separated by a comma (e.g. '10,20')".to_string())
            }
        }
    }
}

impl ValueEnum for SetTime {
    fn value_variants<'a>() -> &'a [Self] {
        &[SetTime::Server, SetTime::Set(0, 0)]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        match self {
            SetTime::Server => Some(PossibleValue::new("server").help("Sync time with the server")),
            SetTime::Set(_, _) => Some(
                PossibleValue::new("<INT,INT>")
                    .help("Provide two comma-separated numbers (e.g., '10,20')"),
            ),
        }
    }
}

#[derive(Args, Debug, Copy, Clone)]
struct SattrWrapper {
    #[arg(long, value_parser = parse_octal)]
    mode: Option<u32>,

    #[arg(long)]
    uid: Option<u32>,

    #[arg(long)]
    gid: Option<u32>,

    #[arg(long)]
    size: Option<u64>,

    #[arg(long)]
    atime: Option<SetTime>,

    #[arg(long)]
    mtime: Option<SetTime>,
}

impl SattrWrapper {
    fn to_xdr_sattr(self) -> nfs3_xdr::Sattr {
        Sattr {
            mode: SetMode { inner: self.mode },
            uid: SetUid { inner: self.uid },
            gid: SetGid { inner: self.gid },
            size: SetSize { inner: self.size },
            atime: match self.atime {
                Some(SetTime::Set(s, n)) => SetAtime::SET_TO_CLIENT_TIME(NfsTime {
                    seconds: s,
                    nseconds: n,
                }),
                Some(SetTime::Server) => SetAtime::SET_TO_SERVER_TIME,
                None => SetAtime::DONT_CHANGE,
            },
            mtime: match self.mtime {
                Some(SetTime::Set(s, n)) => SetMtime::SET_TO_CLIENT_TIME(NfsTime {
                    seconds: s,
                    nseconds: n,
                }),
                Some(SetTime::Server) => SetMtime::SET_TO_SERVER_TIME,
                None => SetMtime::DONT_CHANGE,
            },
        }
    }
}

#[derive(Args, Debug)]
struct Access {
    #[arg(long)]
    read: bool,

    #[arg(long)]
    lookup: bool,

    #[arg(long)]
    modify: bool,

    #[arg(long)]
    extend: bool,

    #[arg(long)]
    delete: bool,

    #[arg(long)]
    execute: bool,
}

#[derive(Args, Debug)]
struct SpecDataWrapper {
    #[arg(long)]
    specdata1: Option<u32>,

    #[arg(long)]
    specdata2: Option<u32>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Perform a getattr RPC.
    Getattr {
        #[arg(short, long, value_parser = parse_hex)]
        filehandle: Bytes,
    },
    Setattr {
        #[arg(short, long, value_parser = parse_hex)]
        filehandle: Bytes,

        #[command(flatten)]
        sattr: SattrWrapper,

        #[arg(long)]
        guard_ctime_s: Option<u32>,

        #[arg(long)]
        guard_ctime_ns: Option<u32>,
    },
    Lookup {
        #[arg(short, long, value_parser = parse_hex)]
        filehandle: Bytes,

        #[arg(short, long)]
        name: String,
    },
    Access {
        #[arg(short, long, value_parser = parse_hex)]
        filehandle: Bytes,

        #[command(flatten)]
        access: Access,
    },
    Readlink {
        #[arg(short, long, value_parser = parse_hex)]
        filehandle: Bytes,
    },
    Read {
        #[arg(short, long, value_parser = parse_hex)]
        filehandle: Bytes,

        #[arg(long)]
        off: u64,

        #[arg(long)]
        count: u32,
    },
    Write {
        #[arg(short, long, value_parser = parse_hex)]
        filehandle: Bytes,

        #[arg(long)]
        off: u64,

        #[arg(long)]
        how: StableHow,
    },
    CreateUnchecked {
        #[arg(short, long, value_parser = parse_hex)]
        filehandle: Bytes,

        #[arg(long)]
        filename: String,

        #[command(flatten)]
        sattr: SattrWrapper,
    },
    CreateGuarded {
        #[arg(short, long, value_parser = parse_hex)]
        filehandle: Bytes,

        #[arg(long)]
        filename: String,

        #[command(flatten)]
        sattr: SattrWrapper,
    },
    CreateExclusive {
        #[arg(short, long, value_parser = parse_hex)]
        filehandle: Bytes,

        #[arg(long)]
        filename: String,

        #[arg(long, value_parser = parse_hex)]
        verf: Bytes,
    },
    Mkdir {
        #[arg(short, long, value_parser = parse_hex)]
        filehandle: Bytes,

        #[arg(long)]
        filename: String,

        #[command(flatten)]
        sattr: SattrWrapper,
    },
    Symlink {
        #[arg(short, long, value_parser = parse_hex)]
        filehandle: Bytes,

        #[arg(long)]
        filename: String,

        #[arg(long)]
        data: String,

        #[command(flatten)]
        sattr: SattrWrapper,
    },
    Mknod {
        #[arg(long)]
        typ: FileType,

        #[arg(short, long, value_parser = parse_hex)]
        filehandle: Bytes,

        #[arg(long)]
        filename: String,

        #[command(flatten)]
        sattr: SattrWrapper,

        #[command(flatten)]
        specdata: SpecDataWrapper,
    },
    Remove {
        #[arg(short, long, value_parser = parse_hex)]
        filehandle: Bytes,

        #[arg(long)]
        filename: String,
    },
    Rmdir {
        #[arg(short, long, value_parser = parse_hex)]
        filehandle: Bytes,

        #[arg(long)]
        filename: String,
    },
    Rename {
        #[arg(short, long, value_parser = parse_hex)]
        src_filehandle: Bytes,

        #[arg(long)]
        src_filename: String,

        #[arg(short, long, value_parser = parse_hex)]
        dst_filehandle: Bytes,

        #[arg(long)]
        dst_filename: String,
    },
    Link {
        #[arg(short, long, value_parser = parse_hex)]
        src_filehandle: Bytes,

        #[arg(short, long, value_parser = parse_hex)]
        dst_filehandle: Bytes,

        #[arg(long)]
        dst_filename: String,
    },
    Readdir {
        #[arg(short, long, value_parser = parse_hex)]
        dirhandle: Bytes,

        #[arg(short, long, default_value_t = 0)]
        cookie: u64,

        #[arg(long, value_parser = parse_hex, default_value = "0000000000000000")]
        cookieverf: Bytes,

        #[arg(long)]
        count: u32,
    },
    ReaddirPlus {
        #[arg(short, long, value_parser = parse_hex)]
        dirhandle: Bytes,

        #[arg(short, long, default_value_t = 0)]
        cookie: u64,

        #[arg(long, value_parser = parse_hex, default_value = "0000000000000000")]
        cookieverf: Bytes,

        #[arg(long)]
        dircount: u32,

        #[arg(long)]
        maxcount: u32,
    },
    FsStat {
        #[arg(short, long, value_parser = parse_hex)]
        filehandle: Bytes,
    },
    FsInfo {
        #[arg(short, long, value_parser = parse_hex)]
        filehandle: Bytes,
    },
    Pathconf {
        #[arg(short, long, value_parser = parse_hex)]
        filehandle: Bytes,
    },
    Commit {
        #[arg(short, long, value_parser = parse_hex)]
        filehandle: Bytes,

        #[arg(long)]
        off: u64,

        #[arg(long)]
        count: u32,
    },
}

fn main() -> io::Result<()> {
    let args = Cli::parse();

    let server_address = SocketAddr::new(
        std::net::IpAddr::V4(Ipv4Addr::from_str(&args.hostname).unwrap()),
        args.port,
    );
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;

    socket.set_reuse_address(true)?;

    let local_addr: SocketAddr = format!("{}:{}", args.client_addr, args.client_port)
        .parse()
        .expect("failed to parse client socket address");
    socket.bind(&local_addr.into())?;

    socket.connect(&server_address.into())?;

    let mut stream: TcpStream = socket.into();

    let sec_cred = match args.sec {
        Sec::Sys => OpaqueAuth::sys(authsys_parms {
            stamp: args.syssec.stamp,
            machinename: args.syssec.machine_name.into(),
            uid: args.syssec.uid,
            gid: args.syssec.gid,
            gids: vec![args.syssec.gid],
        }),
        Sec::None => OpaqueAuth::none(),
    };

    let sec_verf = OpaqueAuth::none();

    {
        let server_address = SocketAddr::new(
            std::net::IpAddr::V4(Ipv4Addr::from_str(&args.hostname).unwrap()),
            20048,
        );
        let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;

        socket.set_reuse_address(true)?;

        let local_addr: SocketAddr = format!("{}:{}", args.client_addr, args.client_port)
            .parse()
            .expect("failed to parse client socket address");
        socket.bind(&local_addr.into())?;

        socket.connect(&server_address.into())?;

        let mut stream: TcpStream = socket.into();

        let mount_arg = mount_proto::MountArg {
            path: args.mount_target.clone().into(),
        };

        let res = do_rpc_call(
            &mut stream,
            CallBody {
                rpcvers: RPC_VERSION,
                prog: mount_proto::procedures::MOUNT_PROGRAM,
                vers: mount_proto::procedures::MOUNT_V3::VERSION,
                proc: mount_proto::procedures::MOUNT_V3::MOUNTPROC3_MNT,
                cred: sec_cred.clone(),
                verf: sec_verf.clone(),
            },
            &mount_arg.serialize_alloc(),
        )
        .expect("mount RPC error");

        let (mount_res, _) = mount_proto::MountResult::deserialize(res.as_slice())
            .expect("mount result deserialization error");

        match &mount_res {
            mount_proto::MountResult::Ok(_mount_result_ok) => {}
            _ => {
                panic!("mount_failed: {mount_res:#?}")
            }
        }
    }

    use serde::Serialize;
    use std::fmt::Debug;

    trait DebugAndSerialize: Debug + erased_serde::Serialize {}
    impl<T: Debug + Serialize> DebugAndSerialize for T {}

    erased_serde::serialize_trait_object!(DebugAndSerialize);

    let res: &dyn DebugAndSerialize = match args.command {
        Command::Getattr { filehandle } => {
            &do_getattr(&mut stream, filehandle, sec_cred.clone(), sec_verf.clone())
        }
        Command::Lookup { filehandle, name } => &do_lookup(
            &mut stream,
            filehandle,
            name,
            sec_cred.clone(),
            sec_verf.clone(),
        ),
        Command::Access { filehandle, access } => &do_access(
            &mut stream,
            filehandle,
            access,
            sec_cred.clone(),
            sec_verf.clone(),
        ),
        Command::Readlink { filehandle } => {
            &do_readlink(&mut stream, filehandle, sec_cred.clone(), sec_verf.clone())
        }
        Command::Read {
            filehandle,
            off,
            count,
        } => &do_read(
            &mut stream,
            filehandle,
            off,
            count,
            sec_cred.clone(),
            sec_verf.clone(),
        ),
        Command::Write {
            filehandle,
            off,
            how,
        } => &do_write(
            &mut stream,
            filehandle,
            off,
            how,
            sec_cred.clone(),
            sec_verf.clone(),
        ),
        Command::Readdir {
            dirhandle,
            cookie,
            cookieverf,
            count,
        } => &do_readdir(
            &mut stream,
            dirhandle,
            cookie,
            cookieverf,
            count,
            sec_cred.clone(),
            sec_verf.clone(),
        ),
        Command::Setattr {
            filehandle,
            sattr,
            guard_ctime_s,
            guard_ctime_ns,
        } => &&do_sattr(
            &mut stream,
            filehandle,
            sattr,
            guard_ctime_s,
            guard_ctime_ns,
            sec_cred.clone(),
            sec_verf.clone(),
        ),
        Command::CreateUnchecked {
            filehandle,
            filename,
            sattr,
        } => &do_create_unchecked(
            &mut stream,
            filehandle,
            filename,
            sattr,
            sec_cred.clone(),
            sec_verf.clone(),
        ),
        Command::CreateGuarded {
            filehandle,
            filename,
            sattr,
        } => &do_create_guarded(
            &mut stream,
            filehandle,
            filename,
            sattr,
            sec_cred.clone(),
            sec_verf.clone(),
        ),
        Command::CreateExclusive {
            filehandle,
            filename,
            verf,
        } => &do_create_exclusive(
            &mut stream,
            filehandle,
            filename,
            verf,
            sec_cred.clone(),
            sec_verf.clone(),
        ),
        Command::Mkdir {
            filehandle,
            filename,
            sattr,
        } => &do_mkdir(
            &mut stream,
            filehandle,
            filename,
            sattr,
            sec_cred.clone(),
            sec_verf.clone(),
        ),
        Command::Symlink {
            filehandle,
            filename,
            data,
            sattr,
        } => &do_symlink(
            &mut stream,
            filehandle,
            filename,
            data,
            sattr,
            sec_cred.clone(),
            sec_verf.clone(),
        ),
        Command::Mknod {
            typ,
            filehandle,
            filename,
            sattr,
            specdata,
        } => &do_mknod(
            &mut stream,
            typ,
            filehandle,
            filename,
            sattr,
            specdata,
            sec_cred.clone(),
            sec_verf.clone(),
        ),
        Command::Remove {
            filehandle,
            filename,
        } => &do_remove(
            &mut stream,
            filehandle,
            filename,
            sec_cred.clone(),
            sec_verf.clone(),
        ),
        Command::Rmdir {
            filehandle,
            filename,
        } => &do_rmdir(
            &mut stream,
            filehandle,
            filename,
            sec_cred.clone(),
            sec_verf.clone(),
        ),
        Command::Rename {
            src_filehandle,
            src_filename,
            dst_filehandle,
            dst_filename,
        } => &&do_rename(
            &mut stream,
            src_filehandle,
            src_filename,
            dst_filehandle,
            dst_filename,
            sec_cred.clone(),
            sec_verf.clone(),
        ),
        Command::Link {
            src_filehandle,
            dst_filehandle,
            dst_filename,
        } => &do_link(
            &mut stream,
            src_filehandle,
            dst_filehandle,
            dst_filename,
            sec_cred.clone(),
            sec_verf.clone(),
        ),
        Command::ReaddirPlus {
            dirhandle,
            cookie,
            cookieverf,
            dircount,
            maxcount,
        } => &&do_readdirplus(
            &mut stream,
            dirhandle,
            cookie,
            cookieverf,
            dircount,
            maxcount,
            sec_cred.clone(),
            sec_verf.clone(),
        ),
        Command::FsStat { filehandle } => {
            &do_fsstat(&mut stream, filehandle, sec_cred.clone(), sec_verf.clone())
        }
        Command::FsInfo { filehandle } => {
            &do_fsinfo(&mut stream, filehandle, sec_cred.clone(), sec_verf.clone())
        }
        Command::Pathconf { filehandle } => {
            &do_pathconf(&mut stream, filehandle, sec_cred.clone(), sec_verf.clone())
        }
        Command::Commit {
            filehandle,
            off,
            count,
        } => &do_commit(
            &mut stream,
            filehandle,
            off,
            count,
            sec_cred.clone(),
            sec_verf.clone(),
        ),
    };

    if args.pretty {
        println!("{}", serde_json::to_string_pretty(&res).unwrap());
    } else {
        println!("{}", serde_json::to_string(&res).unwrap());
    }

    Ok(())
}

fn do_getattr(
    stream: &mut TcpStream,
    fh: Vec<u8>,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> GetAttrResult {
    let arg = GetAttrArgs {
        object: FileHandle { data: fh },
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::GETATTR,
            cred,
            verf,
        },
        &arg,
    )
    .expect("getattr rpc failure");

    let (res, _) =
        GetAttrResult::deserialize(bytes.as_slice()).expect("getattr result deserialization error");

    res
}

fn do_lookup(
    stream: &mut TcpStream,
    fh: Vec<u8>,
    name: String,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> LookupResult {
    let arg = LookupArgs {
        what: DiropArgs {
            dir: FileHandle { data: fh },
            name: name.into(),
        },
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::LOOKUP,
            cred,
            verf,
        },
        &arg,
    )
    .expect("lookup rpc failure");

    let (res, _) =
        LookupResult::deserialize(bytes.as_slice()).expect("lookup result deserialization error");

    res
}

fn do_readlink(
    stream: &mut TcpStream,
    fh: Vec<u8>,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> ReadlinkResult {
    let arg = ReadlinkArgs {
        symlink: FileHandle { data: fh },
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::READLINK,
            cred,
            verf,
        },
        &arg,
    )
    .expect("readlink rpc failure");

    let (res, _) = ReadlinkResult::deserialize(bytes.as_slice())
        .expect("readlink result deserialization error");

    res
}

fn do_access(
    stream: &mut TcpStream,
    fh: Vec<u8>,
    access: Access,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> AccessResult {
    let mut access_bits: u32 = 0;

    if access.read {
        access_bits |= 0x1;
    }

    if access.lookup {
        access_bits |= 0x2;
    }

    if access.modify {
        access_bits |= 0x4;
    }

    if access.extend {
        access_bits |= 0x8;
    }

    if access.delete {
        access_bits |= 0x10;
    }

    if access.execute {
        access_bits |= 0x20;
    }

    let arg = AccessArgs {
        object: FileHandle { data: fh },
        access: access_bits,
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::ACCESS,
            cred,
            verf,
        },
        &arg,
    )
    .expect("access rpc failure");

    let (res, _) =
        AccessResult::deserialize(bytes.as_slice()).expect("access result deserialization error");

    res
}

fn do_read(
    stream: &mut TcpStream,
    fh: Vec<u8>,
    off: u64,
    count: u32,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> ReadResult {
    let arg = ReadArgs {
        file: FileHandle { data: fh },
        offset: off,
        count,
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::READ,
            cred,
            verf,
        },
        &arg,
    )
    .expect("read rpc failure");

    let (res, _) =
        ReadResult::deserialize(bytes.as_slice()).expect("read result deserialization error");

    res
}

fn do_write(
    stream: &mut TcpStream,
    fh: Vec<u8>,
    off: u64,
    how: StableHow,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> WriteResult {
    let mut stdin = io::stdin().lock();
    let mut buffer = Vec::new();
    stdin
        .read_to_end(&mut buffer)
        .expect("failed to read from stdin");

    let arg = WriteArgs {
        file: FileHandle { data: fh },
        count: buffer.len() as u32,
        data: buffer,
        stable: how,
        offset: off,
    };
    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::WRITE,
            cred,
            verf,
        },
        &arg,
    )
    .expect("write rpc failure");

    let (res, _) =
        WriteResult::deserialize(bytes.as_slice()).expect("write result deserialization error");

    res
}

fn do_sattr(
    stream: &mut TcpStream,
    fh: Vec<u8>,
    sattr: SattrWrapper,
    guard_ctime_s: Option<u32>,
    guard_ctime_ns: Option<u32>,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> SattrResult {
    let sattr_guard = guard_ctime_s.or(guard_ctime_s).map(|_| NfsTime {
        seconds: guard_ctime_s.unwrap_or(0),
        nseconds: guard_ctime_ns.unwrap_or(0),
    });

    let arg = SattrArgs {
        object: FileHandle { data: fh },
        new_attributes: sattr.to_xdr_sattr(),
        guard: SattrGuard { inner: sattr_guard },
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::SETATTR,
            cred,
            verf,
        },
        &arg,
    )
    .expect("sattr rpc failure");

    let (res, _) =
        SattrResult::deserialize(bytes.as_slice()).expect("sattr result deserialization error");

    res
}

fn do_create_unchecked(
    stream: &mut TcpStream,
    fh: Vec<u8>,
    filename: String,
    sattr: SattrWrapper,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> CreateResult {
    let arg = CreateArgs {
        r#where: DirOpArgs {
            dir: FileHandle { data: fh },
            name: filename.into(),
        },
        how: CreateHow::UNCHECKED(sattr.to_xdr_sattr()),
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::CREATE,
            cred,
            verf,
        },
        &arg,
    )
    .expect("create rpc failure");

    let (res, _) =
        CreateResult::deserialize(bytes.as_slice()).expect("create result deserialization error");

    res
}

fn do_create_guarded(
    stream: &mut TcpStream,
    fh: Vec<u8>,
    filename: String,
    sattr: SattrWrapper,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> CreateResult {
    let arg = CreateArgs {
        r#where: DirOpArgs {
            dir: FileHandle { data: fh },
            name: filename.into(),
        },
        how: CreateHow::GUARDED(sattr.to_xdr_sattr()),
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::CREATE,
            cred,
            verf,
        },
        &arg,
    )
    .expect("create rpc failure");

    let (res, _) =
        CreateResult::deserialize(bytes.as_slice()).expect("create result deserialization error");

    res
}

fn do_create_exclusive(
    stream: &mut TcpStream,
    fh: Vec<u8>,
    filename: String,
    verf: Bytes,
    cred: OpaqueAuth,
    sec_verf: OpaqueAuth,
) -> CreateResult {
    let arg = CreateArgs {
        r#where: DirOpArgs {
            dir: FileHandle { data: fh },
            name: filename.into(),
        },
        how: CreateHow::EXCLUSIVE(CreateVerf {
            data: verf
                .try_into()
                .expect("failed to parse user provided CreateVerf"),
        }),
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::CREATE,
            cred,
            verf: sec_verf,
        },
        &arg,
    )
    .expect("create rpc failure");

    let (res, _) =
        CreateResult::deserialize(bytes.as_slice()).expect("create result deserialization error");

    res
}

fn do_mkdir(
    stream: &mut TcpStream,
    fh: Vec<u8>,
    filename: String,
    sattr: SattrWrapper,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> MkdirResult {
    let arg = MkdirArgs {
        r#where: DirOpArgs {
            dir: FileHandle { data: fh },
            name: filename.into(),
        },
        attributes: sattr.to_xdr_sattr(),
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::MKDIR,
            cred,
            verf,
        },
        &arg,
    )
    .expect("mkdir rpc failure");

    let (res, _) =
        MkdirResult::deserialize(bytes.as_slice()).expect("mkdir result deserialization error");

    res
}

fn do_symlink(
    stream: &mut TcpStream,
    fh: Vec<u8>,
    filename: String,
    data: String,
    sattr: SattrWrapper,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> SymlinkResult {
    let arg = SymlinkArgs {
        r#where: DirOpArgs {
            dir: FileHandle { data: fh },
            name: filename.into(),
        },
        symlink: SymlinkData {
            symlink_attributes: sattr.to_xdr_sattr(),
            symlink_data: data.into(),
        },
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::SYMLINK,
            cred,
            verf,
        },
        &arg,
    )
    .expect("symlink rpc failure");

    let (res, _) =
        SymlinkResult::deserialize(bytes.as_slice()).expect("symlink result deserialization error");

    res
}

#[allow(clippy::too_many_arguments)]
fn do_mknod(
    stream: &mut TcpStream,
    typ: FileType,
    fh: Vec<u8>,
    filename: String,
    sattr: SattrWrapper,
    spec: SpecDataWrapper,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> MknodResult {
    let mknod_data = match typ {
        FileType::Reg => MknodData::Reg,
        FileType::Dir => MknodData::Dir,
        FileType::Blk => MknodData::Blk(DeviceData {
            dev_attributes: sattr.to_xdr_sattr(),
            spec: SpecData {
                specdata1: spec
                    .specdata1
                    .expect("to create a block device, you must specifiy specdata"),
                specdata2: spec
                    .specdata2
                    .expect("to create a block device, you must specifiy specdata"),
            },
        }),
        FileType::Chr => MknodData::Chr(DeviceData {
            dev_attributes: sattr.to_xdr_sattr(),
            spec: SpecData {
                specdata1: spec
                    .specdata1
                    .expect("to create a character device, you must specifiy specdata"),
                specdata2: spec
                    .specdata2
                    .expect("to create a character device, you must specifiy specdata"),
            },
        }),
        FileType::Lnk => MknodData::Lnk,
        FileType::Sock => MknodData::Sock(sattr.to_xdr_sattr()),
        FileType::Fifo => MknodData::Fifo(sattr.to_xdr_sattr()),
    };

    let arg = MknodArgs {
        r#where: DirOpArgs {
            dir: FileHandle { data: fh },
            name: filename.into(),
        },
        what: mknod_data,
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::MKNOD,
            cred,
            verf,
        },
        &arg,
    )
    .expect("mknod rpc failure");

    let (res, _) =
        MknodResult::deserialize(bytes.as_slice()).expect("mknod result deserialization error");

    res
}

fn do_remove(
    stream: &mut TcpStream,
    fh: Vec<u8>,
    filename: String,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> RemoveResult {
    let arg = RemoveArgs {
        object: DirOpArgs {
            dir: FileHandle { data: fh },
            name: filename.into(),
        },
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::REMOVE,
            cred,
            verf,
        },
        &arg,
    )
    .expect("remove rpc failure");

    let (res, _) =
        RemoveResult::deserialize(bytes.as_slice()).expect("remove result deserialization error");

    res
}

fn do_rmdir(
    stream: &mut TcpStream,
    fh: Vec<u8>,
    filename: String,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> RmdirResult {
    let arg = RmdirArgs {
        object: DirOpArgs {
            dir: FileHandle { data: fh },
            name: filename.into(),
        },
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::RMDIR,
            cred,
            verf,
        },
        &arg,
    )
    .expect("rmdir rpc failure");

    let (res, _) =
        RmdirResult::deserialize(bytes.as_slice()).expect("rmdir result deserialization error");

    res
}

fn do_rename(
    stream: &mut TcpStream,
    src_fh: Vec<u8>,
    src_filename: String,
    dst_fh: Vec<u8>,
    dst_filename: String,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> RenameResult {
    let arg = RenameArgs {
        from: DirOpArgs {
            dir: FileHandle { data: src_fh },
            name: src_filename.into(),
        },
        to: DirOpArgs {
            dir: FileHandle { data: dst_fh },
            name: dst_filename.into(),
        },
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::RENAME,
            cred,
            verf,
        },
        &arg,
    )
    .expect("rename rpc failure");

    let (res, _) =
        RenameResult::deserialize(bytes.as_slice()).expect("rename result deserialization error");

    res
}

fn do_link(
    stream: &mut TcpStream,
    src_fh: Vec<u8>,
    dst_fh: Vec<u8>,
    dst_filename: String,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> LinkResult {
    let arg = LinkArgs {
        file: FileHandle { data: src_fh },
        link: DirOpArgs {
            dir: FileHandle { data: dst_fh },
            name: dst_filename.into(),
        },
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::LINK,
            cred,
            verf,
        },
        &arg,
    )
    .expect("link rpc failure");

    let (res, _) =
        LinkResult::deserialize(bytes.as_slice()).expect("link result deserialization error");

    res
}

fn do_readdir(
    stream: &mut TcpStream,
    dirhandle: Vec<u8>,
    cookie: u64,
    cookieverf: Vec<u8>,
    count: u32,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> ReaddirResult {
    let arg = ReaddirArgs {
        dir: FileHandle { data: dirhandle },
        cookie,
        cookieverf: CookieVerf {
            data: cookieverf.try_into().expect("expected length 8 cookieverf"),
        },
        count,
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::READDIR,
            cred,
            verf,
        },
        &arg,
    )
    .expect("readdir rpc failure");

    let (res, _) =
        ReaddirResult::deserialize(bytes.as_slice()).expect("readdir result deserialization error");

    res
}

#[allow(clippy::too_many_arguments)]
fn do_readdirplus(
    stream: &mut TcpStream,
    dirhandle: Vec<u8>,
    cookie: u64,
    cookieverf: Vec<u8>,
    dircount: u32,
    maxcount: u32,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> ReaddirPlusResult {
    let arg = ReaddirPlusArgs {
        dir: FileHandle { data: dirhandle },
        cookie,
        cookieverf: CookieVerf {
            data: cookieverf.try_into().expect("expected length 8 cookieverf"),
        },
        dircount,
        maxcount,
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::READDIRPLUS,
            cred,
            verf,
        },
        &arg,
    )
    .expect("readdirplus rpc failure");

    let (res, _) = ReaddirPlusResult::deserialize(bytes.as_slice())
        .expect("readdirplus result deserialization error");

    res
}

fn do_fsstat(
    stream: &mut TcpStream,
    filehandle: Vec<u8>,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> FsStatResult {
    let arg = FsStatArgs {
        fsroot: FileHandle { data: filehandle },
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::FSSTAT,
            cred,
            verf,
        },
        &arg,
    )
    .expect("fsstat rpc failure");

    let (res, _) =
        FsStatResult::deserialize(bytes.as_slice()).expect("fsstat result deserialization error");

    res
}

fn do_fsinfo(
    stream: &mut TcpStream,
    filehandle: Vec<u8>,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> FsInfoResult {
    let arg = FsInfoArgs {
        fsroot: FileHandle { data: filehandle },
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::FSINFO,
            cred,
            verf,
        },
        &arg,
    )
    .expect("fsinfo rpc failure");

    let (res, _) =
        FsInfoResult::deserialize(bytes.as_slice()).expect("fsinfo result deserialization error");

    res
}

fn do_pathconf(
    stream: &mut TcpStream,
    filehandle: Vec<u8>,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> PathconfResult {
    let arg = PathconfArgs {
        object: FileHandle { data: filehandle },
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::PATHCONF,
            cred,
            verf,
        },
        &arg,
    )
    .expect("pathconf rpc failure");

    let (res, _) = PathconfResult::deserialize(bytes.as_slice())
        .expect("pathconf result deserialization error");

    res
}

fn do_commit(
    stream: &mut TcpStream,
    filehandle: Vec<u8>,
    offset: u64,
    count: u32,
    cred: OpaqueAuth,
    verf: OpaqueAuth,
) -> CommitResult {
    let arg = CommitArgs {
        file: FileHandle { data: filehandle },
        offset,
        count,
    };

    let arg = arg.serialize_alloc();

    let bytes = do_rpc_call(
        stream,
        CallBody {
            rpcvers: RPC_VERSION,
            prog: NFS_PROGRAM,
            vers: NFS_V3::VERSION,
            proc: NFS_V3::COMMIT,
            cred,
            verf,
        },
        &arg,
    )
    .expect("commit rpc failure");

    let (res, _) =
        CommitResult::deserialize(bytes.as_slice()).expect("commit result deserialization error");

    res
}
