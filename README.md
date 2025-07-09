# NFS and RPC Utilities

This repo houses a collection of utilities for XDR / RPC programs.

LANL software release number: O4912. See the LICENSE file for copyright and license.

## Organization

- `xdr_codegen/` -- this directory houses the library used to compile an XDR language specification
  into a Rust library that implements the [de]serialization of the defined data types.
- `test_xdr/` -- this directory holds tests for the `xdr_codegen` library
- `rpc_protocol` -- this directory holds a library that implements the RPC protocol, as well as
  binaries that implement the rpcbind protocol, both client and server side. These are effectively
  (currently incomplete) clones of the standard `rpcbind` and `rpcinfo` binaries.

## Protocol Definitions

Here are some relevant RFCs:

### RPC Related:
- [RFC 4506](https://datatracker.ietf.org/doc/html/rfc4506) - XDR Serialization Protocol
- [RFC 5531](https://datatracker.ietf.org/doc/html/rfc5531) - RPC Protocol
- [RFC 1813](https://datatracker.ietf.org/doc/html/rfc1833) - RPCBIND and PortMapper Protocols

### NFS Related:
- [RFC 1813](https://datatracker.ietf.org/doc/html/rfc1813) - NFS v3 Protocol
- [RFC 5661](https://datatracker.ietf.org/doc/html/rfc5661) - NFS v4.1 Protocol
  - [RFC 5662](https://datatracker.ietf.org/doc/html/rfc5662) - NFS v4.1 XDR/RPC Specification
- [RFC 7862](https://datatracker.ietf.org/doc/html/rfc7862)  - NFS v4.2 Protocol
  - [RFC 7863](https://datatracker.ietf.org/doc/html/rfc7863) - NFS v4.2 XDR/RPC Specification

- [RFC 8434](https://datatracker.ietf.org/doc/html/rfc8434) - Requirements for PNFS Layout Types
- [RFC 8435](https://datatracker.ietf.org/doc/html/rfc8435) - PNFS Flex Files Layout Specification

## `xdr_codegen`

The `xdr_codegen` library works by taking in a specification in the XDR language (defined in RFC 4506)
and the RPC language extension (defined in RFC 5531) and outputs Rust code that implements
serialization and deserialization routines for the data types defined in the XDR spec.

For example, given the following XDR code:
```xdr
struct File {
    unsigned int size;
    string name<>;
};
```

`xdr_codegen` will generate a Rust module that defines the following struct and methods:

```Rust
#[derive(Debug, PartialEq, Clone)]
pub struct File {
    pub size: u32,
    pub name: std::ffi::OsString,
}
impl File {
    pub fn serialize_alloc(&self) -> Vec<u8>;
    pub fn deserialize(&mut self, mut input: &mut &[u8]) -> Result<(), helpers::DeserializeError>;
}
```
