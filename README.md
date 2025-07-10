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

### How to use `xdr_codegen`

You can run `xdr_codegen` via the command line:
```bash
$ echo "struct foo { int bar; };" | cargo run --bin xdr_codegen
#[allow(non_camel_case_types, non_snake_case)]
pub mod XdrInterface {
    #[derive(Debug, PartialEq, Clone)]
    pub struct foo {
        pub bar: i32,
    }

    ...
}
```

or in `build.rs`:
```Rust
fn main() {
    xdr_codegen::Compiler::new()
        .file("protocol_spec.x")
        .run()
        .expect("Generating code failed");
}
```

### XDR Data Types

#### Scalar Types

The scalar types translate from XDR to Rust in the predictable way, e.g., `int => i32`
`unsigned hyper => u64`, etc.

#### Arrays

XDR Arrays are encoded as Rust arrays (for fixed-length arrays) or Vectors (for variable length
arrays, both limited and unlimited). Opaque arrays are encoded as `u8`s.

XDR Strings are represented as `ffi::OsString`s.

<table>
<tr>
<th>XDR</th>
<th>Rust</th>
</tr>
<tr>
<td>

```XDR
struct Arrays {
        /* Strings: */
        string lim<10>;
        string unlim<>;

        /* Byte arrays: */
        opaque fixed[4];
        opaque byte_lim<5>;
        opaque byte_unlim<>;
};
```

</td>
<td>

```Rust
pub struct Arrays {
    pub lim: std::ffi::OsString /* max length: 10 */,
    pub unlim: std::ffi::OsString,
    pub fixed: [u8; 4],
    pub byte_lim: Vec<u8> /* max length: 5 */,
    pub byte_unlim: Vec<u8>,
}
```

</td>
</tr>
</table>

#### Optionals

XDR uses "optional" types to implement a "linked list"-like structure. XDR's "pointer" syntax suggests
that this type of structure be implemented as a literal linked list. This might be natural in C, but
it certainly is not in Rust. Therefore, this library represents optionals using a Rust Vector.

The implementation requires that the self-referential structure has a container type, and that container
type--NOT the self-referential type itself--is the one that contains the vector. For example:

<table>
<tr>
<th>XDR</th>
<th>Rust</th>
</tr>
<tr>
<td>

```XDR
struct Node {
        string name<>;
        Node *next;
};

struct NodeList {
        Node *list;
};
```

</td>
<td>

```Rust
pub struct Node {
    pub name: std::ffi::OsString,
}
pub struct NodeList {
    pub list: Vec<Node>,
}
```

</td>
</tr>
</table>

If the self-referential type does not have a container type, then this library will not serialize it
properly! In practice, most/all of the real protocol definitions I have looked at do use container
types for lists. For example, in the following types from the NFSv3 spec:

```XDR
struct entry3 {
   fileid3      fileid;
   filename3    name;
   cookie3      cookie;
   entry3       *nextentry;
};

struct dirlist3 {
   entry3       *entries;
   bool         eof;
};
```
`dirlist3` is the needed container type.

#### Naming Conventions

XDR normally uses snake_case for type names, while Rust uses CamelCase. This library makes no
attempt to rename types to conform to the Rust style. I recommend using CamelCase names in the
XDR specs that you write, to that the generatecd Rust code will be idiomatic.


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
