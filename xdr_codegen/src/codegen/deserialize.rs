// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

// Deserialization routines for XDR data types.

use super::*;
use crate::symbol_table::ValidatedSymbolTable;

const DESERIALIZE_SIGNATURE: &str =
    "pub fn deserialize(&mut self, input: &mut &[u8]) -> xdr_lib::Result<()>";

impl Array {
    pub(super) fn deserialize_inline(
        &self,
        name: &str,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,
    ) {
        match &self.size {
            ArraySize::Fixed(_) => {
                buf.add_line(&format!("let len = {name}.len();"));
            }
            _ => {
                buf.add_line("let mut len = 0;");
                buf.add_line("xdr_lib::get_u32(&mut len, input)?;");
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
        tab: &ValidatedSymbolTable,
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

impl ValidatedUnion {
    pub(super) fn deserialize_definition(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        buf.code_block(DESERIALIZE_SIGNATURE, |buf| {
            match &self.body {
                ValidatedUnionBody::Bool(b) => b.deserialize_bool(buf, tab),
                ValidatedUnionBody::Enum(e) => e.deserialize_enum(buf, tab),
            };
            buf.add_line("Ok(())");
        });
    }
}

impl ValidatedUnionBoolBody {
    pub(super) fn deserialize_bool(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        buf.add_line("let mut discriminant: u32 = 0;");
        buf.add_line("xdr_lib::get_u32(&mut discriminant, input)?;");
        buf.block_statement("match discriminant", |buf| {
            buf.add_line("0 => (*self).inner = None,");
            buf.code_block("_ => ", |buf| {
                buf.add_line(&format!(
                    "let mut val = {};",
                    self.true_arm.default_value(tab)
                ));
                self.true_arm.deserialize_inline(Some("val"), buf, tab);
                buf.add_line("(*self).inner = Some(val)");
            });
        });
    }
}

impl ValidatedUnionEnumBody {
    pub(super) fn deserialize_enum(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        buf.add_line("let mut discriminant = 0;");
        buf.add_line("xdr_lib::get_i32(&mut discriminant, input)?;");
        buf.block_statement("*self = match discriminant", |buf| {
            for arm in self.arms.iter() {
                let discriminant_value = self.get_discriminant_value(&arm.0, tab);
                buf.code_block(&format!("{discriminant_value} => "), |buf| {
                    let arm_name = ValidatedUnionEnumBody::arm_name(&arm.0);
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
                buf.add_line("_ => return Err(xdr_lib::DeserializeError),");
            }
        });
    }
}

impl ValidatedStruct {
    pub fn offset_to_string(off: &DeclarationOfset) -> String {
        Self::offset_to_string_with_unwrapper(off, "?")
    }

    pub fn offset_to_string_infallible(off: &DeclarationOfset) -> String {
        Self::offset_to_string_with_unwrapper(off, ".unwrap()")
    }

    pub fn offset_to_string_with_unwrapper(off: &DeclarationOfset, unwrapper: &str) -> String {
        let code = off
            .deps
            .iter()
            .map(|v| format!("self.get_{}_width(){}", v, unwrapper))
            .chain(
                vec![format!("{}", off.known)]
                    .into_iter()
                    .filter(|v| v != "0"),
            )
            .collect::<Vec<String>>()
            .join(" + ");

        if code.is_empty() {
            "0".to_string()
        } else {
            code.clone()
        }
    }

    pub fn offset_to_string_localvars(off: &DeclarationOfset) -> String {
        let code = off
            .deps
            .iter()
            .map(|v| format!("{}_width", v))
            .chain(
                vec![format!("{}", off.known)]
                    .into_iter()
                    .filter(|v| v != "0"),
            )
            .collect::<Vec<String>>()
            .join(" + ");

        if code.is_empty() {
            "0".to_string()
        } else {
            code.clone()
        }
    }

    pub(super) fn deserialize_definition(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        buf.code_block(DESERIALIZE_SIGNATURE, |buf| {
            for (decl, _) in self.members.iter() {
                buf.add_line(&format!("// {}:", decl.name));
                decl.deserialize_inline(None, buf, tab);
            }
            buf.add_line("Ok(())");
        });
    }
}

impl ValidatedEnum {
    pub(super) fn deserialize_definition(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        buf.code_block(DESERIALIZE_SIGNATURE, |buf| {
            buf.add_line("let mut val = 0;");
            buf.add_line("xdr_lib::get_i32(&mut val, input)?;");
            buf.block_statement("*self = match val", |buf| {
                for variant in self.variants.iter() {
                    let val = variant.1.as_const(tab);
                    buf.add_line(&format!("{} => {}::{},", val, self.name, variant.0));
                }
                buf.add_line("_ => return Err(xdr_lib::DeserializeError),");
            });
            buf.add_line("Ok(())");
        });
    }
}

impl XdrType {
    pub(super) fn deserialize_inline(
        &self,
        var_name: &str,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,
    ) {
        // Handle typedefs specially by finding their underlying type:
        if let XdrType::Name(name) = self {
            let definition = tab.lookup_definition(name);
            if let ValidatedDefinition::TypeDef(ref tdef) = *definition {
                tdef.decl.deserialize_inline(Some(var_name), buf, tab);
                return;
            };
        };

        // typedef case already handled, non-typedefs follow:
        let method = self.deserialize_method();
        buf.add_line(&format!("{method}(&mut {var_name}, input)?;"));
    }

    fn deserialize_method(&self) -> String {
        match self {
            XdrType::Int => "xdr_lib::get_i32".to_string(),
            XdrType::UInt => "xdr_lib::get_u32".to_string(),
            XdrType::Hyper => "xdr_lib::get_i64".to_string(),
            XdrType::UHyper => "xdr_lib::get_u64".to_string(),
            XdrType::Float => todo!(),
            XdrType::Double => todo!(),
            XdrType::Quadruple => todo!(),
            XdrType::Bool => "xdr_lib::get_bool".to_string(),
            XdrType::Name(n) => format!("{n}::deserialize"),
        }
    }

    pub(super) fn deserialize_optional_inline(
        &self,
        name: &str,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,
    ) {
        if self.self_referential_optional(tab) {
            buf.code_block("loop", |buf| {
                buf.add_line("let mut item_follows = 0;");
                buf.add_line("xdr_lib::get_i32(&mut item_follows, input)?;");
                buf.add_line("if item_follows == 0 { break; }");
                buf.add_line(&format!("let mut new = {};", self.default_value(tab)));
                self.deserialize_inline("new", buf, tab);
                buf.add_line(&format!("{name}.push(new)"));
            });
        } else {
            buf.add_line("let mut optional_follows = 0;");
            buf.add_line("xdr_lib::get_i32(&mut optional_follows, input)?;");
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
