use std::{net::IpAddr, path::PathBuf};

use cidr::Ipv4Cidr;

/// An NFS export.
pub struct Export {
    pub path: PathBuf,
    pub clients: Vec<ExportClient>,
}

/// A set of clients that can access an export, together with the options applied to those clients.
pub struct ExportClient {
    pub client: ClientId,
    pub options: ExportOptions,
}

pub enum ClientId {
    Name(String),
    Addr(IpAddr),
    Netgroup(String),
    Network(Ipv4Cidr),
}

pub struct ExportOptions {
    /// Whether to deny write access to the export.
    pub read_only: bool,
    /// If true, map the root user to the anonymous user.
    pub root_squash: bool,
}
