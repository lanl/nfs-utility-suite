# NFS v3

This directory holds programs related to the NFSv3 implementation.

The NFSv3 server relies on `io_uring` and thus must be run on a new enough Linux kernel.

## `mountd`

An daemon that implements the server side of the mount protocol.

## `showmount`

A program that implements the client side of the mount protocol.

## `nfs_server`

The server side implementation of the NFS v3 protocol.

## `nfs_cli`

A command-line client of the NFS v3 protocol.
