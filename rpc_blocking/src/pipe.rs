// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

//! An "pipe", constructed using socketpair(2), that can be used for testing client and
//! server behavior.

use nix::sys::socket::{socketpair, AddressFamily, SockFlag, SockType};

pub struct Endpoint {
    fd: std::os::fd::OwnedFd,
}

pub fn pipe() -> std::io::Result<(Endpoint, Endpoint)> {
    let (a, b) = socketpair(
        AddressFamily::Unix,
        SockType::Stream,
        None,
        SockFlag::empty(),
    )?;

    Ok((Endpoint { fd: a }, Endpoint { fd: b }))
}

impl std::io::Read for Endpoint {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        Ok(nix::unistd::read(&self.fd, buf)?)
    }
}

impl std::io::Write for Endpoint {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(nix::unistd::write(&self.fd, buf)?)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
