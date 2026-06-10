// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

// Deserialization routines for XDR data types.

use super::*;
use crate::symbol_table::ValidatedSymbolTable;

const DESERIALIZE_SIGNATURE: &str =
    "pub fn deserialize(&mut self, input: &mut &[u8]) -> xdr_lib::Result<()>";

impl Array {
    pub(super) fn get_size_inline_zcopy(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        buf.add_line(&self.array_count_extractor(Some("length"), tab, false, true));

        if let Some(elem_width) = self.elem_size(tab) {
            buf.add_line(&format!(
                "let required = xdr_lib::geq_4byte_boundary(length * {}usize) + _array_count_size;",
                elem_width
            ));

            buf.add_line("Ok(required)");
        } else {
            buf.add_line(&format!(
                "let mut it = xdr_lib::ArrayIter::<'a, {}>::new(_input, length, None);",
                self.zcopy_gen_inner_type(tab)
            ));

            buf.add_line("it.by_ref().for_each(drop);");
            buf.add_line("");
            buf.code_block("if it.i != length", |buf| {
                buf.add_line("return Err(xdr_lib::DeserializeError);");
            });
            buf.add_line("");
            buf.add_line("Ok(it.off + _array_count_size)");
        }
    }

    pub(super) fn deserialize_inline_zcopy(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        buf.add_line(&self.array_count_extractor(Some("length"), tab, true, false));

        match &self.kind {
            ArrayKind::Byte => buf.add_line("&_input[..length]"),
            ArrayKind::Ascii => buf.add_line("std::ffi::OsStr::from_bytes(&_input[..length])"),
            ArrayKind::UserType(_) => {
                buf.add_line(&format!(
                    "xdr_lib::ArrayIter::<'a, {}>::new(_input, length, None)",
                    self.zcopy_gen_inner_type(tab)
                ));
            }
        }
    }

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

    pub(super) fn deserialize_inline_zcopy(
        &self,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,
        fallible_parent: bool,
    ) {
        match &self.kind {
            DeclarationKind::Scalar(ty) => {
                ty.deserialize_inline_zcopy(buf, tab, fallible_parent);
            }
            DeclarationKind::Array(a) => {
                a.deserialize_inline_zcopy(buf, tab);
            }
            DeclarationKind::Optional(o) => {
                o.deserialize_optional_inline_zcopy(buf, tab, fallible_parent);
            }
        }
    }

    pub(super) fn get_size_inline_zcopy(
        &self,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,

        fallible_parent: bool,
        member_name: Option<String>,
    ) {
        match &self.kind {
            DeclarationKind::Scalar(ty) => {
                ty.get_size_inline_zcopy(buf, tab, fallible_parent, member_name);
            }
            DeclarationKind::Array(a) => {
                a.get_size_inline_zcopy(buf, tab);
            }
            DeclarationKind::Optional(ty) => {
                ty.get_optional_size_inline_zcopy(buf, tab, fallible_parent, member_name);
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
        let code = off
            .deps
            .iter()
            .map(|v| format!("self.get_{}_width()?", v))
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

    pub fn offset_to_string_infallible(off: &DeclarationOfset) -> String {
        let code = off
            .deps
            .iter()
            .map(|v| format!("self.get_{}_width().unwrap()", v))
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

    pub(super) fn deserialize_definition_zcopy(
        &self,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,
    ) {
        buf.code_block("fn validate(self) -> xdr_lib::Result<Self>", |buf| {
            if let Some((last, last_off)) = self.members.last() {
                let last_size = if self.member_is_self_referential(last, tab) {
                    // We skip last members who evaluate to iterators, which could be a
                    // large performance sink
                    Some(0)
                } else {
                    last.size(tab)
                };

                let mut overall_definition_size = DefinitionSize {
                    known: last_off.known + last_size.unwrap_or(0),
                    deps: last_off.deps.clone(),
                };

                if last_size.is_none() {
                    overall_definition_size.deps.push(last.name.clone());
                }

                buf.add_line(&format!(
                    "let required = {};",
                    &Self::offset_to_string(&overall_definition_size)
                ));
                buf.code_block("if required > self.buf.len()", |buf| {
                    buf.add_line("return Err(xdr_lib::DeserializeError);");
                });
            }

            buf.add_line("Ok(self)");
        });

        let deps = self.get_variable_width_members(tab);

        for dep in deps.iter() {
            buf.code_block(
                &format!("pub fn get_{}_width(&self) -> xdr_lib::Result<usize>", dep),
                |buf| {
                    let (member, member_off) = self
                        .members
                        .iter()
                        .find(|(nd, _)| nd.name == **dep)
                        .unwrap();

                    if self.member_is_self_referential(member, tab) {
                        match &member.kind {
                            DeclarationKind::Optional(xdr_type)
                            | DeclarationKind::Scalar(xdr_type) => {
                                buf.add_line(&format!(
                                    "let off = {};",
                                    Self::offset_to_string(member_off)
                                ));
                                buf.add_line("let _input = &self.buf[off..];");
                                xdr_type.get_optional_size_inline_zcopy(buf, tab, true, None);
                            }
                            _ => unreachable!(),
                        };
                    } else {
                        if member.is_varlen_reader(tab) {
                            buf.add_line(&format!("self.{}.get_width()", member.name));
                        } else {
                            buf.add_line(&format!("Ok(self.{}_width)", member.name));
                        }
                    }
                },
            );
        }

        for (member, member_off) in self.members.iter() {
            buf.code_block(
                &format!(
                    "pub fn get_{}(&self) -> {}",
                    member.name,
                    member.as_zcopy_dser_type_name(tab)
                ),
                |buf| {
                    if member.size(tab).is_none() && member.is_varlen_reader(tab) {
                        buf.add_line(&format!("return self.{}.clone()", member.name));
                        return;
                    }

                    if member_off.is_determinate() {
                        buf.add_line(&format!("let off = {};", member_off.known));
                    } else {
                        buf.add_line(&format!(
                            "let off = {};",
                            Self::offset_to_string_infallible(member_off)
                        ));
                    }

                    buf.add_line("let _input = &self.buf[off..];");

                    // Validation can be here
                    member.deserialize_inline_zcopy(buf, tab, false);
                },
            );
        }
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

    pub(super) fn deserialize_definition_zcopy(
        &self,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,
    ) {
        buf.code_block(
            "pub fn deserialize(_input: &[u8]) -> xdr_lib::Result<Self>",
            |buf| {
                buf.add_line("let val = xdr_lib::get_i32_infallible(_input);");
                buf.code_block("match val", |buf| {
                    for variant in self.variants.iter() {
                        let val = variant.1.as_const(tab);
                        buf.add_line(&format!("{} => Ok({}::{}),", val, self.name, variant.0));
                    }
                    buf.add_line("_ => Err(xdr_lib::DeserializeError),");
                });
            },
        );

        buf.add_line("pub fn get_width(&self) -> usize {4}");
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

    pub(super) fn deserialize_inline_zcopy(
        &self,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,
        fallible_parent: bool,
    ) {
        // Handle typedefs specially by finding their underlying type:
        if let XdrType::Name(name) = self {
            let definition = tab.lookup_definition(name);
            if let ValidatedDefinition::TypeDef(ref tdef) = *definition {
                tdef.decl
                    .deserialize_inline_zcopy(buf, tab, fallible_parent);
                return;
            };
        };

        let unwrap_method = if fallible_parent { "?" } else { ".unwrap()" };

        // typedef case already handled, non-typedefs follow:
        let (method, fallible) = self.deserialize_method_zcopy(tab);
        if !fallible {
            buf.add_line(&format!("{method}(_input)"));
        } else {
            buf.add_line(&format!("{method}(_input){unwrap_method}"));
        }
    }

    pub(super) fn get_size_inline_zcopy(
        &self,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,

        fallible_parent: bool,
        member_name: Option<String>,
    ) {
        // Handle typedefs specially by finding their underlying type:
        if let XdrType::Name(name) = self {
            let definition = tab.lookup_definition(name);
            if let ValidatedDefinition::TypeDef(ref tdef) = *definition {
                tdef.decl
                    .get_size_inline_zcopy(buf, tab, fallible_parent, member_name);
                return;
            };
        };

        let unwrap_method = if fallible_parent { "?" } else { ".unwrap()" };

        if let Some(found_size) = self.size(tab) {
            buf.add_line(&format!("Ok({})", found_size));
        } else {
            // typedef case already handled, non-typedefs follow:
            let (method, fallible) = self.deserialize_method_zcopy(tab);

            if let Some(cache_name) = member_name {
                buf.block_statement(
                    &format!("if let Some(cached) = {}.get()", cache_name),
                    |buf| {
                        buf.add_line("return cached.get_width();");
                    },
                );
                buf.add_line(&format!(
                    "let val = {method}(_input){};",
                    if fallible { unwrap_method } else { "" }
                ));
                buf.add_line(&format!("{cache_name}.get_or_init(|| val).get_width()"));
            } else {
                buf.add_line(&format!(
                    "{method}(_input){}.get_width()",
                    if fallible { unwrap_method } else { "" }
                ));
            }
        }
    }

    pub(super) fn get_optional_size_inline_zcopy(
        &self,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,
        fallible_parent: bool,
        reader_name: Option<String>,
    ) {
        // Handle typedefs specially by finding their underlying type:
        if let XdrType::Name(name) = self {
            let definition = tab.lookup_definition(name);
            if let ValidatedDefinition::TypeDef(ref tdef) = *definition {
                tdef.decl
                    .get_size_inline_zcopy(buf, tab, fallible_parent, reader_name);
                return;
            };
        };

        buf.add_line("let has_optional = xdr_lib::get_i32_infallible(_input);");
        buf.code_block("match has_optional", |buf| {
            buf.add_line("0 => Ok(4),");
            buf.code_block("_ =>", |buf| {
                if self.self_referential_optional(tab) {
                    buf.add_line(&format!(
                        "let mut it = xdr_lib::LinkedListIter::<'a, {}>::new(_input, {});",
                        self.as_zcopy_deser_type_name(tab),
                        self.size(tab)
                            .map(|v| format!("Some({})", v))
                            .unwrap_or("None".to_string())
                    ));

                    buf.add_line("it.by_ref().for_each(drop);");
                    buf.add_line("");
                    buf.add_line("Ok(it.off)");
                } else if let Some(opt_size) = self.size(tab) {
                    buf.add_line(&format!("Ok({})", opt_size + 4));
                } else {
                    buf.add_line("let off = off + 4;");
                    buf.add_line("let _input = &self.buf[off..];");
                    self.get_size_inline_zcopy(buf, tab, fallible_parent, reader_name.clone());
                    buf.add_line("\t.map(|val| val + 4usize)");
                }
            });
        });
    }

    // returns (name, fallible)
    fn deserialize_method_zcopy(&self, tab: &ValidatedSymbolTable) -> (String, bool) {
        match self {
            XdrType::Int => ("xdr_lib::get_i32_infallible".to_string(), false),
            XdrType::UInt => ("xdr_lib::get_u32_infallible".to_string(), false),
            XdrType::Hyper => ("xdr_lib::get_i64_infallible".to_string(), false),
            XdrType::UHyper => ("xdr_lib::get_u64_infallible".to_string(), false),
            XdrType::Float => todo!(),
            XdrType::Double => todo!(),
            XdrType::Quadruple => todo!(),
            XdrType::Bool => ("xdr_lib::get_bool_infallible".to_string(), false),
            XdrType::Name(n) => {
                if let ValidatedDefinition::Enum(_) = tab.lookup_definition(n) {
                    (format!("{n}::deserialize"), true)
                } else {
                    (format!("{n}Reader::from_buf"), true)
                }
            }
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

    pub(super) fn deserialize_optional_inline_zcopy(
        &self,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,
        fallible_parent: bool,
    ) {
        if self.self_referential_optional(tab) {
            buf.add_line(&format!(
                "xdr_lib::LinkedListIter::<'a, {}>::new(_input, {})",
                self.as_zcopy_deser_type_name(tab),
                self.size(tab)
                    .map(|v| format!("Some({})", v))
                    .unwrap_or("None".to_string())
            ));
        } else {
            buf.add_line("let has_val = xdr_lib::get_i32_infallible(_input);");
            buf.code_block("match has_val", |buf| {
                buf.add_line("0 => None,");
                buf.code_block("_ =>", |buf| {
                    buf.block_statement("let val = ", |buf| {
                        buf.add_line("let off = off + 4;");
                        buf.add_line("let _input = &self.buf[off..];");
                        self.deserialize_inline_zcopy(buf, tab, fallible_parent);
                    });
                    buf.add_line("Some(val)");
                    // buf.add_line("val.map(|v| Some(v))");
                })
            });
        }
    }
}
