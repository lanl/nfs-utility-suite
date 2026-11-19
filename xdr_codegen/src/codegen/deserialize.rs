// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

// Deserialization routines for XDR data types.

use super::*;
use crate::symbol_table::ValidatedSymbolTable;

const DESERIALIZE_SIGNATURE: &str =
    "pub fn deserialize(input: &[u8]) -> xdr_lib::Result<(Self, &[u8])>";

impl Array {
    pub(super) fn deserialize_inline(
        &self,
        name: &str,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,
    ) {
        match &self.size {
            ArraySize::Fixed(len) => {
                let len = len.as_const(tab);
                buf.add_line(&format!("let len = {len};"));
            }
            ArraySize::Limited(len) => {
                let len = len.as_const(tab);
                buf.add_line("let (len, input) = xdr_lib::get_size(input)?;");
                buf.add_line(&format!(
                    "if len > {len} {{ return Err(xdr_lib::DeserializeError); }}"
                ));
            }
            ArraySize::Unlimited => {
                buf.add_line("let (len, input) = xdr_lib::get_size(input)?;");
            }
        };
        match &self.kind {
            ArrayKind::UserType(ty) => {
                buf.add_line(&format!("let mut {name} = Vec::new();"));
                buf.add_line("let mut input2 = input;");
                buf.block_statement("for _i in 0..len", |buf| {
                    buf.add_line("let input = input2;");
                    ty.deserialize_inline("item", buf, tab);
                    buf.add_line("input2 = input;");
                    buf.add_line(&format!("{name}.push(item);"));
                });
                buf.add_line("let input = input2;");
                if let ArraySize::Fixed(len) = &self.size {
                    let len = len.as_const(tab);
                    buf.add_line(&format!(
                        "let {name}: [_; {len}] = {name}.try_into().unwrap();"
                    ));
                }
            }
            byte_or_ascii => {
                // `len` is read from the input, so it has to be checked against what is left of
                // the input before splitting on it.
                buf.add_line("let padded_len = xdr_lib::padded(len)?;");
                buf.add_line(
                    "if input.len() < padded_len { return Err(xdr_lib::DeserializeError); }",
                );
                buf.add_line("let (xdr_bytes, input) = input.split_at(len);");

                match &self.size {
                    ArraySize::Fixed(len) => {
                        buf.add_line(&format!("let mut {name} = [0; {}];", len.as_const(tab)));
                        buf.add_line(&format!("{name}.clone_from_slice(xdr_bytes);"))
                    }
                    _ => match byte_or_ascii {
                        ArrayKind::Byte => {
                            buf.add_line(&format!("let mut {name} = Vec::new();"));
                            buf.add_line(&format!("{name}.extend_from_slice(xdr_bytes);"))
                        }
                        ArrayKind::Ascii => {
                            buf.add_line(&format!("let mut {name} = std::ffi::OsString::new();"));
                            buf.add_line(&format!(
                                "{name}.push(std::ffi::OsStr::from_bytes(xdr_bytes));"
                            ))
                        }
                        ArrayKind::UserType(_) => unreachable!(),
                    },
                };
                buf.add_line("let (_, input) = input.split_at(padded_len - len);");
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
        let var_name = override_name.unwrap_or_else(|| &self.name);
        match &self.kind {
            DeclarationKind::Scalar(ty) => {
                ty.deserialize_inline(var_name, buf, tab);
            }
            DeclarationKind::Array(a) => {
                a.deserialize_inline(var_name, buf, tab);
            }
            DeclarationKind::Optional(o) => {
                o.deserialize_optional_inline(var_name, buf, tab);
            }
        };
    }
}

impl ValidatedUnion {
    pub(super) fn deserialize_definition(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        buf.code_block(DESERIALIZE_SIGNATURE, |buf| {
            match &self.body {
                ValidatedUnionBody::Bool(b) => b.deserialize_bool(buf, tab),
                ValidatedUnionBody::Enum(e) => e.deserialize_enum(buf, tab),
            };
        });
    }
}

impl ValidatedUnionBoolBody {
    pub(super) fn deserialize_bool(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        buf.add_line("let (discriminant, input) = xdr_lib::get_bool(input)?;");
        buf.code_block("match discriminant", |buf| {
            buf.code_block("false => ", |buf| {
                buf.block_statement("let new = Self", |buf| {
                    buf.add_line("inner: None,");
                });
                buf.add_line("Ok((new, input))");
            });
            buf.code_block("true => ", |buf| {
                self.true_arm.deserialize_inline(Some("val"), buf, tab);
                buf.block_statement("let new = Self", |buf| {
                    buf.add_line("inner: Some(val),");
                });

                buf.add_line("Ok((new, input))");
            });
        });
    }
}

impl ValidatedUnionEnumBody {
    pub(super) fn deserialize_enum(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        // While enums are represented as i32s, in practice this implementation requires them to be
        // negative. At the same time, integer-discriminated unions may have an "unsigned int"
        // discriminant which can in theory hold values > i32::MAX. Using get_u32() supports both
        // of these.
        buf.add_line("let (discriminant, input) = xdr_lib::get_u32(input)?;");
        buf.code_block("match discriminant", |buf| {
            for arm in self.arms.iter() {
                buf.code_block(&format!("{} => ", arm.val), |buf| {
                    match &arm.decl {
                        Declaration::Void => {
                            buf.add_line(&format!("Ok((Self::{}, input))", arm.name));
                        }
                        Declaration::Named(n) => {
                            n.deserialize_inline(Some("inner"), buf, tab);
                            buf.add_line(&format!("Ok((Self::{}(inner), input))", arm.name));
                        }
                    };
                });
            }
            if let Some(default_arm) = &self.default_arm {
                match default_arm {
                    Declaration::Void => {
                        buf.add_line("_ => Ok((Self::Default, input))");
                    }
                    Declaration::Named(n) => {
                        buf.code_block("_ => ", |buf| {
                            n.deserialize_inline(Some("inner"), buf, tab);
                            buf.add_line("Ok((Self::Default(inner), input))");
                        });
                    }
                };
            } else {
                buf.add_line("_ => Err(xdr_lib::DeserializeError)");
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
            .map(|v| format!("self.get_{}_width(){}", v.replace("r#", ""), unwrapper))
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
                decl.deserialize_inline(Some(&decl.name_as_local_var()), buf, tab);
                buf.add_line("");
            }

            buf.block_statement("let new =  Self", |buf| {
                for (decl, _) in self.members.iter() {
                    buf.add_line(&format!("{}: {},", decl.name, decl.name_as_local_var()));
                }
            });
            buf.add_line("");
            buf.add_line("Ok((new, input))");
        });
    }
}

impl ValidatedEnum {
    pub(super) fn deserialize_definition(&self, buf: &mut CodeBuf) {
        buf.code_block(DESERIALIZE_SIGNATURE, |buf| {
            buf.add_line("let (val, input) = xdr_lib::get_i32(input)?;");
            buf.block_statement("let new = match val", |buf| {
                for variant in self.variants.iter() {
                    buf.add_line(&format!("{} => {}::{},", variant.1, self.name, variant.0));
                }
                buf.add_line("_ => return Err(xdr_lib::DeserializeError),");
            });
            buf.add_line("Ok((new, input))");
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
        buf.add_line(&format!("let ({var_name}, input) = {method}(input)?;"));
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
            buf.add_line(&format!("let mut {name} = Vec::new();"));
            buf.add_line("let mut input2 = input;");
            buf.block_statement("loop", |buf| {
                buf.add_line("let input = input2;");
                buf.add_line("let (item_follows, input) = xdr_lib::get_bool(input)?;");
                buf.add_line("input2 = input;");
                buf.add_line("if !item_follows { break; }");
                self.deserialize_inline("new", buf, tab);
                buf.add_line("input2 = input;");
                buf.add_line(&format!("{name}.push(new)"));
            });
            buf.add_line("let input = input2;");
        } else {
            buf.add_line("let (optional_follows, input) = xdr_lib::get_bool(input)?;");
            buf.block_statement(
                &format!("let ({name}, input) = match optional_follows"),
                |buf| {
                    buf.add_line("false => (None, input),");
                    buf.code_block("true =>", |buf| {
                        self.deserialize_inline("new", buf, tab);
                        buf.add_line("(Some(new), input)");
                    });
                },
            );
        }
    }
}
