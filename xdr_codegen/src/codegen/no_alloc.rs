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
    /// Generate code to serialize a named declaration without allocating, inline within the
    /// serialization routine of a container type.
    ///
    /// When `override_name` is `None`, then this uses `self.member_name` as the name of the field.
    /// However, this does not work for typedefs, as the name of the typedef would be
    /// `member_name`, instead of the name of the field in its container type.
    /// Thus, for typedefs, the caller will pass in `Some(name)` for `override_name`, since only
    /// the caller knows the actual field name.
    fn serialize_no_alloc_inline(
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
            DeclarationKind::Optional(_) => todo!(),
        }
    }
}

impl Array {
    fn serialize_no_alloc_inline(&self, var_name: &str, buf: &mut CodeBuf, _tab: &SymbolTable) {
        match &self.size {
            ArraySize::Fixed(_) => {}
            _ => todo!(),
        };
        match &self.kind {
            ArrayKind::Byte => {
                buf.add_line(&format!(
                    "buf[offset..offset + {var_name}.len()].copy_from_slice(&{var_name});"
                ));
                buf.add_line(&format!("offset += {var_name}.len();"));
                buf.add_line(&format!("let padding = (4 - {var_name}.len() % 4) % 4;"));
                buf.add_line("buf[offset..offset + padding].copy_from_slice(&vec![0; padding]);");
                buf.add_line("offset += padding;");
            }
            _ => todo!(),
        };
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
