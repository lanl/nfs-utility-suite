// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

// Allocating serialization routines for XDR data types.

use super::*;
use crate::symbol_table::SymbolTable;

impl Array {
    pub(super) fn serialize_inline(
        &self,
        name: &str,
        context: Context,
        buf: &mut CodeBuf,
        tab: &SymbolTable,
    ) {
        match &self.size {
            ArraySize::Fixed(_) => {} // Fixed-size array does not need length encoded
            _ => {
                buf.add_line(&format!(
                    "buf.extend_from_slice(&({name}.len() as u32).to_be_bytes());"
                ));
            }
        };
        match &self.kind {
            ArrayKind::Ascii => buf.add_line(&format!("buf.extend_from_slice({name}.as_bytes());")),
            ArrayKind::Byte => buf.add_line(&format!(
                "buf.extend_from_slice({}{name});",
                match &self.size {
                    // When appending a byte array to a vector, depending on the context it may or
                    // may not be necessary to append '&' to make it a reference:
                    ArraySize::Fixed(_) => match context {
                        Context::InUnion => "",
                        Context::NotInUnion => "&",
                    },
                    _ => "&",
                }
            )),
            ArrayKind::UserType(ty) => {
                buf.block_statement(&format!("for item in {name}.iter()"), |buf| {
                    ty.serialize_inline("item", context, buf, tab);
                });
            }
        };
        // Byte arrays and strings need to be padded to a multiple of 4:
        match &self.kind {
            ArrayKind::UserType(_) => {}
            _ => {
                buf.add_line(&format!("let padding = (4 - {name}.len() % 4) % 4;"));
                buf.add_line("buf.extend_from_slice(&vec![0; padding]);");
            }
        };
    }
}

impl NamedDeclaration {
    /// Generate code to serialize a named declaration, inline within the serialization routine for
    /// another container type (struct, union, etc.)
    ///
    /// If `override_name` is `Some(name)`, then this function uses `name` for the field name
    /// instead of assuming it is named `self.member_name` (where `member_name is the name of the
    /// field in the XDR spec).
    pub(super) fn serialize_inline(
        &self,
        override_name: Option<&str>,
        context: Context,
        buf: &mut CodeBuf,
        tab: &SymbolTable,
    ) {
        let var_name = match override_name {
            Some(over) => over.to_string(),
            None => format!("self.{}", self.name),
        };
        match &self.kind {
            DeclarationKind::Scalar(ty) => {
                ty.serialize_inline(&var_name, context, buf, tab);
            }
            DeclarationKind::Array(a) => {
                a.serialize_inline(&var_name, context, buf, tab);
            }
            DeclarationKind::Optional(o) => {
                o.serialize_optional_inline(&var_name, context, buf, tab);
            }
        };
    }
}

impl XdrUnion {
    pub(super) fn serialize_definition(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        buf.code_block(
            "pub fn serialize_alloc(&self) -> Vec<u8>",
            |buf| match &self.body {
                XdrUnionBody::Bool(b) => b.serialize_bool(buf, tab),
                XdrUnionBody::Enum(e) => e.serialize_enum(buf, tab, true),
            },
        );
    }
}

impl XdrUnionBoolBody {
    fn serialize_bool(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        buf.code_block("match &self.inner", |buf| {
            buf.code_block("Some(val) => ", |buf| {
                buf.add_line("let mut buf = 1_u32.to_be_bytes().to_vec();");
                match &self.true_arm {
                    Declaration::Void => {
                        buf.add_line("// void");
                    }
                    Declaration::Named(n) => {
                        n.serialize_inline(Some("val"), Context::InUnion, buf, tab)
                    }
                };
                buf.add_line("buf");
            });
            buf.add_line("None => 0_u32.to_be_bytes().to_vec(),");
        });
    }
}

impl XdrStruct {
    pub(super) fn serialize_definition(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        buf.code_block("pub fn serialize_alloc(&self) -> Vec<u8>", |buf| {
            buf.add_line("let mut buf = Vec::new();");
            for decl in self.members.iter() {
                let Declaration::Named(decl) = decl else {
                    buf.add_line("// void");
                    continue;
                };
                buf.add_line(&format!("// {}:", decl.name));
                decl.serialize_inline(None, Context::NotInUnion, buf, tab);
            }
            buf.add_line("buf");
        });
    }
}

impl XdrEnum {
    pub(super) fn serialize_definition(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        buf.code_block("pub fn serialize_alloc(&self) -> Vec<u8>", |buf| {
            buf.block_statement("let val: i32 = match self", |buf| {
                for variant in self.variants.iter() {
                    let val = variant.1.as_const(tab);
                    buf.add_line(&format!("{}::{} => {},", self.name, variant.0, val));
                }
            });
            buf.add_line("val.to_be_bytes().to_vec()");
        });
    }
}

impl XdrType {
    pub(super) fn serialize_inline(
        &self,
        var_name: &str,
        context: Context,
        buf: &mut CodeBuf,
        tab: &SymbolTable,
    ) {
        // Handle typedefs specially by finding their underlying type:
        if let XdrType::Name(name) = self {
            let definition = tab.lookup_definition(name).unwrap();
            if let Definition::TypeDef(ref tdef) = *definition {
                match &tdef.decl {
                    Declaration::Void => panic!("Void typedefs are not currently supported"),
                    Declaration::Named(n) => n.serialize_inline(Some(var_name), context, buf, tab),
                };
                return;
            };
        };

        // The typedef case was already handled, non-typedefs follow:
        let serialize_method = self.serialize_method_string(var_name, tab);
        buf.add_line(&format!("let bytes = {serialize_method};"));
        buf.add_line("buf.extend_from_slice(&bytes);");
    }

    pub(super) fn serialize_optional_inline(
        &self,
        name: &str,
        context: Context,
        buf: &mut CodeBuf,
        tab: &SymbolTable,
    ) {
        if self.self_referential_optional(tab) {
            buf.code_block(&format!("for item in {name}.iter()"), |buf| {
                buf.add_line("buf.extend_from_slice(&1_i32.to_be_bytes());");
                self.serialize_inline("item", context, buf, tab);
            });
            buf.add_line("buf.extend_from_slice(&0_i32.to_be_bytes());");
        } else {
            buf.block_statement(&format!("match &{name}"), |buf| {
                buf.code_block("Some(inner) => ", |buf| {
                    buf.add_line("buf.extend_from_slice(&1_i32.to_be_bytes());");
                    self.serialize_inline("inner", context, buf, tab);
                });
                buf.add_line("None => buf.extend_from_slice(&0_i32.to_be_bytes()),");
            });
        }
    }
}
