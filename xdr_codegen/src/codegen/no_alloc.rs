// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

// Non-allocating serialization routines for XDR data types.

use super::*;
use crate::symbol_table::SymbolTable;

impl XdrStruct {
    /// Output a non-allocating serialization routine for this XdrStruct.
    ///
    /// Given:
    ///     struct Foo {
    ///         ...
    ///     };
    ///
    /// Produces:
    ///     pub fn serialize(&self, buf: &mut [u8]) -> usize {
    ///         ...
    ///     }
    pub(super) fn serialize_no_alloc(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        buf.code_block("pub fn serialize(&self, buf: &mut [u8]) -> usize", |buf| {
            buf.add_line("let mut offset = 0;");
            for decl in &self.members {
                let Declaration::Named(decl) = decl else {
                    buf.add_line("// void");
                    continue;
                };
                buf.add_line(&format!("// {}:", decl.name));
                decl.serialize_no_alloc_inline(None, buf, tab);
            }
            buf.add_line("offset");
        });
    }
}

impl XdrUnion {
    pub(super) fn serialize_no_alloc(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        buf.code_block("pub fn serialize(&self, buf: &mut [u8]) -> usize", |buf| {
            buf.add_line("let mut offset = 0;");
            match &self.body {
                XdrUnionBody::Bool(b) => b.serialize_no_alloc(buf, tab),
                XdrUnionBody::Enum(b) => b.serialize_enum(buf, tab, false),
            };
            buf.add_line("offset");
        });
    }
}

impl XdrUnionBoolBody {
    fn serialize_no_alloc(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        buf.code_block("match &self.inner", |buf| {
            buf.code_block("Some(val) => ", |buf| {
                buf.serialize_int(1);
                match &self.true_arm {
                    Declaration::Void => {
                        buf.add_line("// void");
                    }
                    Declaration::Named(n) => n.serialize_no_alloc_inline(Some("val"), buf, tab),
                };
            });
            buf.code_block("None => ", |buf| buf.serialize_int(0));
        });
    }
}

impl XdrEnum {
    pub(super) fn serialize_no_alloc(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        buf.code_block("pub fn serialize(&self, buf: &mut [u8]) -> usize", |buf| {
            buf.add_line("let mut offset = 0;");
            buf.block_statement("let val: i32 = match self", |buf| {
                for variant in self.variants.iter() {
                    let val = variant.1.as_const(tab);
                    buf.add_line(&format!("{}::{} => {},", self.name, variant.0, val));
                }
            });
            buf.add_line("buf[offset..offset + 4].copy_from_slice(&val.to_be_bytes());");
            buf.add_line("offset += 4;");
            buf.add_line("offset");
        });
    }
}

impl NamedDeclaration {
    /// Generate code to serialize a named declaration without allocating, inline within the
    /// serialization routine of a container type.
    ///
    /// When `override_name` is `None`, then this uses `self.member_name` as the name of the field.
    /// However, this does not work for typedefs, as the name of the typedef would be
    /// `member_name`, instead of the name of the field in its container type.
    /// Thus, for typedefs, the caller will pass in `Some(name)` for `override_name`, since only
    /// the caller knows the actual field name.
    pub(super) fn serialize_no_alloc_inline(
        &self,
        override_name: Option<&str>,
        buf: &mut CodeBuf,
        tab: &SymbolTable,
    ) {
        let var_name = match override_name {
            Some(name) => name.to_string(),
            None => format!("self.{}", self.name),
        };
        match &self.kind {
            DeclarationKind::Scalar(ty) => ty.serialize_no_alloc_inline(&var_name, buf, tab),
            DeclarationKind::Array(a) => a.serialize_no_alloc_inline(&var_name, buf, tab),
            DeclarationKind::Optional(ty) => {
                ty.serialize_optional_no_alloc_inline(&var_name, buf, tab)
            }
        }
    }
}

impl Array {
    fn serialize_no_alloc_inline(&self, var_name: &str, buf: &mut CodeBuf, tab: &SymbolTable) {
        self.encode_size(var_name, buf, tab);

        if let ArrayKind::UserType(ty) = &self.kind {
            buf.block_statement(&format!("for item in {var_name}.iter()"), |buf| {
                ty.serialize_no_alloc_inline("item", buf, tab);
            });

            return;
        };

        match &self.kind {
            ArrayKind::Byte => {
                buf.add_line(&format!(
                    "buf[offset..offset + {var_name}.len()].copy_from_slice(&{var_name});"
                ));
            }
            ArrayKind::Ascii => {
                buf.add_line(&format!(
                    "buf[offset..offset + {var_name}.len()].copy_from_slice(&{var_name}.as_bytes());"
                ));
            }
            ArrayKind::UserType(_) => unreachable!(), // already handled above
        };

        buf.add_line(&format!("offset += {var_name}.len();"));
        buf.add_line("offset += helpers::encode_padding(offset, buf);");
    }

    /// Generate the code that encodes the size of a variable length array into the message.
    ///
    /// For limited-size arrays, this adds an assert that the user does not try to encode an array
    /// exceeding the limit.
    fn encode_size(&self, var_name: &str, buf: &mut CodeBuf, tab: &SymbolTable) {
        match &self.size {
            // The length of a fixed-length array does not need to be encoded.
            ArraySize::Fixed(_) => return,
            ArraySize::Limited(lim) => {
                let lim = lim.as_const(tab);
                // It is a bug to try to encode a too-large variable length array.
                buf.add_line(&format!("assert!({var_name}.len() <= {lim});"));
            }
            ArraySize::Unlimited => {}
        };

        buf.add_line(&format!(
            "buf[offset..offset + 4].copy_from_slice(&({var_name}.len() as u32).to_be_bytes());"
        ));
        buf.add_line("offset += 4;");
    }
}

impl XdrType {
    fn serialize_no_alloc_inline(&self, var_name: &str, buf: &mut CodeBuf, tab: &SymbolTable) {
        match self {
            XdrType::Name(name) => {
                let definition = tab.lookup_definition(name).unwrap();
                if let Definition::TypeDef(ref tdef) = *definition {
                    match &tdef.decl {
                        Declaration::Void => panic!("Void typedefs are not currently supported"),
                        Declaration::Named(n) => {
                            n.serialize_no_alloc_inline(Some(var_name), buf, tab)
                        }
                    };
                    return;
                };

                buf.add_line(&format!(
                    "offset += {var_name}.serialize(&mut buf[offset..]);"
                ));
            }
            _ => {
                let width = self.width();
                let serialize_method = self.serialize_method_string(var_name, tab);

                buf.add_line(&format!(
                    "buf[offset..offset + {width}].copy_from_slice(&{serialize_method});"
                ));

                buf.add_line(&format!("offset += {width};"));
            }
        };
    }

    fn serialize_optional_no_alloc_inline(&self, name: &str, buf: &mut CodeBuf, tab: &SymbolTable) {
        if self.self_referential_optional(tab) {
            buf.code_block(&format!("for item in {name}.iter()"), |buf| {
                buf.serialize_int(1);
                self.serialize_no_alloc_inline("item", buf, tab);
            });
            buf.serialize_int(0);
        } else {
            buf.block_statement(&format!("match &{name}"), |buf| {
                buf.code_block("Some(inner) => ", |buf| {
                    buf.serialize_int(1);
                    self.serialize_no_alloc_inline("inner", buf, tab);
                });
                buf.code_block("None => ", |buf| {
                    buf.serialize_int(0);
                });
            });
        }
    }

    /// Returns the width of a primitive scalar type. E.g., int is 4.
    ///
    /// Panics if the type isn't a primitive type (i.e., it is the name of another type which could
    /// be a typedef, struct, union, etc.). Type names should be fully resolved to an underlying
    /// primitive type before calling this method.
    fn width(&self) -> u32 {
        match self {
            XdrType::Int => 4,
            XdrType::UInt => 4,
            XdrType::Hyper => 8,
            XdrType::UHyper => 8,
            XdrType::Float => 4,
            XdrType::Double => 8,
            XdrType::Quadruple => 16,
            XdrType::Bool => 4,
            XdrType::Name(n) => panic!("Name {n}: Getting the width of a named type isn't supported. Resolve the type name first."),
        }
    }
}

impl CodeBuf {
    /// Write into `self` the code to serialize a signed integer `val`.
    pub(super) fn serialize_int(&mut self, val: i32) {
        self.add_line(&format!(
            "buf[offset..offset + 4].copy_from_slice(&{val}_i32.to_be_bytes());"
        ));
        self.add_line("offset += 4;");
    }
}
