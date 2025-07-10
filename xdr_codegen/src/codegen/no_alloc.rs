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
                decl.serialize_no_alloc_inline(buf, tab);
            }
            buf.add_line("offset");
        });
    }
}

impl XdrUnion {
    pub(super) fn serialize_no_alloc(&self, buf: &mut CodeBuf, _tab: &SymbolTable) {
        buf.code_block("pub fn serialize(&self, buf: &mut [u8]) -> usize>", |buf| {
            buf.add_line("todo!()");
        });
    }
}

impl XdrEnum {
    pub(super) fn serialize_no_alloc(&self, buf: &mut CodeBuf, _tab: &SymbolTable) {
        buf.code_block("pub fn serialize(&self, buf: &mut [u8]) -> usize", |buf| {
            buf.add_line("todo!()");
        });
    }
}

impl NamedDeclaration {
    fn serialize_no_alloc_inline(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        let var_name = format!("self.{}", self.name);
        match &self.kind {
            DeclarationKind::Scalar(ty) => ty.serialize_no_alloc_inline(&var_name, buf, tab),
            _ => todo!(),
        }
    }
}

impl XdrType {
    fn serialize_no_alloc_inline(&self, var_name: &str, buf: &mut CodeBuf, tab: &SymbolTable) {
        if let XdrType::Name(_name) = self {
            // TODO: add this...
            return;
        }
        let width = self.width();
        let serialize_method = self.serialize_method_string(var_name, SerializeKind::NoAlloc, tab);

        buf.add_line(&format!(
            "buf[offset..offset + {width}].copy_from_slice(&{serialize_method});"
        ));

        buf.add_line(&format!("offset += {width};"));
    }

    /// Returns the width of a primitive scalar type. E.g., int is 4.
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
