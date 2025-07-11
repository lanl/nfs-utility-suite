// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

// Deserialization routines for XDR data types.

use super::*;
use crate::symbol_table::SymbolTable;

impl Array {
    pub(super) fn deserialize_inline(&self, name: &str, buf: &mut CodeBuf, tab: &SymbolTable) {
        match &self.size {
            ArraySize::Fixed(_) => {
                buf.add_line(&format!("let len = {name}.len();"));
            }
            _ => {
                buf.add_line("let mut len = 0;");
                buf.add_line("helpers::get_u32(&mut len, &mut input)?;");
            }
        };
        match &self.kind {
            ArrayKind::UserType(ty) => {
                buf.block_statement("for _i in 0..len", |buf| {
                    buf.add_line(&format!("let mut new = {};", ty.default_value(tab)));
                    ty.deserialize_inline("new", buf, tab);
                    match &self.size {
                        ArraySize::Fixed(_) => buf.add_line(&format!("{name}[_i] = new;")),
                        _ => buf.add_line(&format!("{name}.push(new);")),
                    }
                });
            }
            _ => {
                buf.add_line("let (bytes, rest) = input.split_at(len as usize);");
                buf.add_line("*input = rest;");
                match &self.size {
                    ArraySize::Fixed(_) => {
                        buf.add_line(&format!("{name}.clone_from_slice(bytes);"))
                    }
                    _ => match &self.kind {
                        ArrayKind::Byte => {
                            buf.add_line(&format!("{name}.extend_from_slice(bytes);"))
                        }
                        ArrayKind::Ascii => buf
                            .add_line(&format!("{name}.push(std::ffi::OsStr::from_bytes(bytes));")),
                        ArrayKind::UserType(_) => unreachable!(),
                    },
                };
                buf.add_line("let padding = (4 - len % 4) % 4;");
                buf.add_line("let (_, rest) = input.split_at(padding as usize);");
                buf.add_line("*input = rest;");
            }
        }
    }
}

impl NamedDeclaration {
    /// Generate code to deserialize a named declaration, inline within the deserialization routine
    /// for another container type (struct, union, etc.)
    ///
    /// If `override_name` is `Some(name)`, then this function uses `name` for the field name
    /// instead of assuming it is named `self.member_name` (where `member_name is the name of the
    /// field in the XDR spec).
    pub(super) fn deserialize_inline(
        &self,
        override_name: Option<&str>,
        buf: &mut CodeBuf,
        tab: &SymbolTable,
    ) {
        let var_name = match override_name {
            Some(over) => over.to_string(),
            None => format!("self.{}", self.name),
        };
        match &self.kind {
            DeclarationKind::Scalar(ty) => {
                ty.deserialize_inline(&var_name, buf, tab);
            }
            DeclarationKind::Array(a) => {
                a.deserialize_inline(&var_name, buf, tab);
            }
            DeclarationKind::Optional(o) => {
                o.deserialize_optional_inline(&var_name, buf, tab);
            }
        }
    }
}

impl XdrUnion {
    pub(super) fn deserialize_definition(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        buf.code_block(
            "pub fn deserialize(&mut self, mut input: &mut &[u8]) -> Result<(), helpers::DeserializeError>",
            |buf| {
                match &self.body {
                    XdrUnionBody::Bool(b) => b.deserialize_bool(buf, tab),
                    XdrUnionBody::Enum(e) => e.deserialize_enum(buf, tab),
                };
                buf.add_line("Ok(())");
            }
        );
    }
}

impl XdrUnionBoolBody {
    pub(super) fn deserialize_bool(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        buf.add_line("let mut discriminant: u32 = 0;");
        buf.add_line("helpers::get_u32(&mut discriminant, &mut input)?;");
        buf.block_statement("match discriminant", |buf| {
            buf.add_line("0 => (*self).inner = None,");
            match &self.true_arm {
                Declaration::Void => buf.add_line("_ => {}, // void"),
                Declaration::Named(n) => {
                    buf.code_block("_ => ", |buf| {
                        buf.add_line(&format!("let mut val = {};", n.default_value(tab)));
                        n.deserialize_inline(Some("val"), buf, tab);
                        buf.add_line("(*self).inner = Some(val)");
                    });
                }
            };
        });
    }
}

impl XdrUnionEnumBody {
    pub(super) fn deserialize_enum(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        buf.add_line("let mut discriminant = 0;");
        buf.add_line("helpers::get_i32(&mut discriminant, &mut input)?;");
        buf.block_statement("*self = match discriminant", |buf| {
            for arm in self.arms.iter() {
                let discriminant_value = self.get_discriminant_value(&arm.0, tab);
                buf.code_block(&format!("{discriminant_value} => "), |buf| {
                    let arm_name = XdrUnionEnumBody::arm_name(&arm.0);
                    match &arm.1 {
                        Declaration::Void => {
                            buf.add_line(&format!("Self::{arm_name}"));
                        }
                        Declaration::Named(n) => {
                            buf.add_line(&format!("let mut inner = {};", n.default_value(tab)));
                            n.deserialize_inline(Some("inner"), buf, tab);
                            buf.add_line(&format!("Self::{arm_name}(inner) "));
                        }
                    };
                });
            }
            if let Some(default_arm) = &self.default_arm {
                match default_arm {
                    Declaration::Void => {
                        buf.add_line("_ => Self::Default,");
                    }
                    Declaration::Named(n) => {
                        buf.code_block("_ => ", |buf| {
                            buf.add_line(&format!("let mut inner = {};", n.default_value(tab)));
                            n.deserialize_inline(Some("inner"), buf, tab);
                            buf.add_line("Self::Default(inner) ");
                        });
                    }
                };
            } else {
                buf.add_line("_ => return Err(helpers::DeserializeError),");
            }
        });
    }
}

impl XdrStruct {
    pub(super) fn deserialize_definition(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        buf.code_block(
            "pub fn deserialize(&mut self, mut input: &mut &[u8]) -> Result<(), helpers::DeserializeError>",
            |buf| {
                for decl in self.members.iter() {
                    let Declaration::Named(decl) = decl else {
                        buf.add_line("// void");
                        continue;
                    };
                    buf.add_line(&format!("// {}:", decl.name));
                    decl.deserialize_inline(None, buf, tab);
                }
                buf.add_line("Ok(())");
            },
        );
    }
}

impl XdrEnum {
    pub(super) fn deserialize_definition(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        buf.code_block(
            "pub fn deserialize(&mut self, mut input: &mut &[u8]) -> Result<(), helpers::DeserializeError>",
            |buf| {
                buf.add_line("let mut val = 0;");
                buf.add_line("helpers::get_i32(&mut val, &mut input)?;");
                buf.block_statement("*self = match val", |buf| {
                    for variant in self.variants.iter() {
                        let val = variant.1.as_const(tab);
                        buf.add_line(&format!("{} => {}::{},", val, self.name, variant.0));
                    }
                    buf.add_line("_ => return Err(helpers::DeserializeError),");
                });
                buf.add_line("Ok(())");
            },
        );
    }
}

impl XdrType {
    pub(super) fn deserialize_inline(&self, var_name: &str, buf: &mut CodeBuf, tab: &SymbolTable) {
        // Handle typedefs specially by finding their underlying type:
        if let XdrType::Name(name) = self {
            let definition = tab.lookup_definition(name).unwrap();
            if let Definition::TypeDef(ref tdef) = *definition {
                match &tdef.decl {
                    Declaration::Void => panic!("Void typedefs are not currently supported"),
                    Declaration::Named(n) => n.deserialize_inline(Some(var_name), buf, tab),
                };
                return;
            };
        };

        // typedef case already handled, non-typedefs follow:
        let method = self.deserialize_method();
        buf.add_line(&format!("{method}(&mut {var_name}, &mut input)?;"));
    }

    fn deserialize_method(&self) -> String {
        match self {
            XdrType::Int => "helpers::get_i32".to_string(),
            XdrType::UInt => "helpers::get_u32".to_string(),
            XdrType::Hyper => "helpers::get_i64".to_string(),
            XdrType::UHyper => "helpers::get_u64".to_string(),
            XdrType::Float => todo!(),
            XdrType::Double => todo!(),
            XdrType::Quadruple => todo!(),
            XdrType::Bool => "helpers::get_bool".to_string(),
            XdrType::Name(n) => format!("{n}::deserialize"),
        }
    }

    pub(super) fn deserialize_optional_inline(
        &self,
        name: &str,
        buf: &mut CodeBuf,
        tab: &SymbolTable,
    ) {
        if self.self_referential_optional(tab) {
            buf.code_block("loop", |buf| {
                buf.add_line("let mut item_follows = 0;");
                buf.add_line("helpers::get_i32(&mut item_follows, input)?;");
                buf.add_line("if item_follows == 0 { break; }");
                buf.add_line(&format!("let mut new = {};", self.default_value(tab)));
                self.deserialize_inline("new", buf, tab);
                buf.add_line(&format!("{name}.push(new)"));
            });
        } else {
            buf.add_line("let mut optional_follows = 0;");
            buf.add_line("helpers::get_i32(&mut optional_follows, input)?;");
            buf.block_statement(&format!("{name} = match optional_follows"), |buf| {
                buf.add_line("0 => None,");
                buf.code_block("_ =>", |buf| {
                    buf.add_line(&format!("let mut new = {};", self.default_value(tab)));
                    self.deserialize_inline("new", buf, tab);
                    buf.add_line("Some(new)");
                })
            });
        }
    }
}
