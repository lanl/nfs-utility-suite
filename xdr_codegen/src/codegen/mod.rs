// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

// This file does code generation for allocating serialization routines which return a Vec<u8>,
// and de-serialization routines.

use std::collections::HashSet;

use crate::ast::*;
use crate::ir::*;
use crate::symbol_table::ValidatedSymbolTable;
use crate::validate::*;

mod alloc;
mod deserialize;
mod no_alloc;

/// Parameters for code generation.
pub struct Params {
    /// Whether to include non-allocating serialization routines.
    pub no_alloc: bool,

    /// Whether to include allocating serialization routines.
    pub alloc: bool,

    /// Whether to include zero-copy serdes routines
    pub zcopy: bool,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            no_alloc: false,
            alloc: true,
            zcopy: false,
        }
    }
}

const USE_FFI_HEADER: &str = r#"
use std::os::unix::ffi::OsStrExt;
"#;

enum FunctionKind {
    Function,
    Method,
}

pub fn codegen(schema: &ValidatedSchema, module_name: &str, params: &Params) -> String {
    let mut buf = CodeBuf::new();

    buf.add_line("#[allow(non_camel_case_types, non_snake_case, clippy::all)]");
    buf.code_block(&format!("pub mod {module_name}"), |buf| {
        if schema.contains_string {
            buf.add_line(USE_FFI_HEADER);
            buf.add_line("");
        }

        if params.zcopy {
            buf.add_line("#[allow(unused_imports)]");
            buf.add_line("use xdr_lib::Reader;");
            buf.add_line("");
        }

        for def in schema.definition_list.iter() {
            let def = schema.symbol_table.lookup_definition(def);
            def.definition(buf, &schema.symbol_table, params);
        }

        for def in schema.definition_list.iter() {
            let def = schema.symbol_table.lookup_definition(def);
            def.implementation(buf, &schema.symbol_table, params);
        }

        for prog in schema.programs.iter() {
            prog.codegen(buf);
        }
    });

    buf.contents
}

impl Program {
    fn codegen(&self, buf: &mut CodeBuf) {
        buf.code_block("pub mod procedures", |buf| {
            buf.add_line(&format!("pub const {}: u32 = {};", self.name, self.id));
            for version in self.versions.iter() {
                buf.code_block(&format!("pub mod {}", version.name), |buf| {
                    buf.add_line(&format!("pub const VERSION: u32 = {};", version.id));
                    for procedure in version.procedures.iter() {
                        buf.add_line(&format!(
                            "pub const {}: u32 = {};",
                            procedure.name, procedure.id
                        ));
                        buf.add_line("");
                    }
                });
            }
        });
    }
}

impl ValidatedDefinition {
    /// The definition for the type.
    fn definition(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable, params: &Params) {
        if params.zcopy {
            self.definition_zcopy(buf, tab);
        } else {
            self.definition_copy(buf, tab);
        }
    }

    fn definition_zcopy(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        match self {
            ValidatedDefinition::Const(c) => {
                match &c.value {
                    Value::Int(n) => {
                        buf.add_line(&format!(
                            "pub const {}: u64 = {};",
                            c.name.to_uppercase(),
                            n
                        ));
                    }
                    Value::Name(name) => {
                        todo!("{name}");
                    }
                };
            }
            ValidatedDefinition::Enum(e) => {
                e.definition(buf);
            }
            ValidatedDefinition::Struct(s) => {
                s.definition_zcopy(buf, tab);
            }
            ValidatedDefinition::TypeDef(_) => {}
            ValidatedDefinition::Union(_u) => {
                unimplemented!();
            }
        }
    }

    fn definition_copy(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        match self {
            ValidatedDefinition::Const(c) => {
                match &c.value {
                    Value::Int(n) => {
                        buf.add_line(&format!(
                            "pub const {}: u64 = {};",
                            c.name.to_uppercase(),
                            n
                        ));
                    }
                    Value::Name(name) => {
                        todo!("{name}");
                    }
                };
            }
            ValidatedDefinition::Enum(e) => {
                e.definition(buf);
            }
            ValidatedDefinition::Struct(s) => {
                s.definition(buf, tab);
            }
            ValidatedDefinition::TypeDef(_) => {}
            ValidatedDefinition::Union(u) => {
                u.definition(buf, tab);
            }
        }
    }

    /// The impl block for the type, including its serialize and deserialize methods.
    fn implementation(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable, params: &Params) {
        match self {
            ValidatedDefinition::Enum(e) => {
                e.codegen(buf, tab, params);
            }
            ValidatedDefinition::Struct(s) => {
                s.codegen(buf, tab, params);
            }
            ValidatedDefinition::Union(u) => {
                u.codegen(buf, tab, params);
            }
            ValidatedDefinition::TypeDef(_) | ValidatedDefinition::Const(_) => {}
        }
    }

    /// Given a definition, get its type name in a way suitable for a struct member.
    ///
    /// If the definition is based on an UnresolvedName, then recursively look up that name in the
    /// symbol table.
    ///
    /// For example:
    ///
    ///    ValidatedDefinition                      Result
    ///
    ///    const FOO = 2;                  2
    ///    typedef unsigned long uint32;   u32
    ///    typedef uid3 uint32             u32     (resolves via above typedef)
    ///    struct blah { /* ... */ };      blah
    fn as_type_name(&self, tab: &ValidatedSymbolTable) -> String {
        match self {
            ValidatedDefinition::Struct(s) => s.name.to_string(),
            ValidatedDefinition::Enum(e) => e.name.to_string(),
            ValidatedDefinition::Union(u) => u.name.to_string(),
            ValidatedDefinition::Const(c) => c.value.as_type_name(tab),
            ValidatedDefinition::TypeDef(t) => match &t.decl.kind {
                DeclarationKind::Scalar(ty) => ty.as_type_name(tab),
                DeclarationKind::Optional(o) => o.optional_type_name(tab),
                DeclarationKind::Array(arr) => arr.as_type_name(tab),
            },
        }
    }

    fn is_reader(&self, tab: &ValidatedSymbolTable) -> bool {
        match self {
            ValidatedDefinition::Struct(_) | ValidatedDefinition::Union(_) => true,
            ValidatedDefinition::TypeDef(t) => match &t.decl.kind {
                DeclarationKind::Scalar(ty) | DeclarationKind::Optional(ty) => ty.is_reader(tab),
                DeclarationKind::Array(_) => false,
            },
            _ => false,
        }
    }

    fn as_zcopy_deser_type_name(&self, tab: &ValidatedSymbolTable) -> String {
        match self {
            ValidatedDefinition::Struct(s) => format!("{}Reader<'a>", s.name),
            ValidatedDefinition::Enum(e) => e.name.to_string(),
            ValidatedDefinition::Union(_u) => unimplemented!(),
            ValidatedDefinition::Const(c) => c.value.as_type_name(tab),
            ValidatedDefinition::TypeDef(t) => match &t.decl.kind {
                DeclarationKind::Scalar(ty) => ty.as_zcopy_deser_type_name(tab),
                DeclarationKind::Optional(_o) => unimplemented!(),
                DeclarationKind::Array(_arr) => unimplemented!(),
            },
        }
    }

    fn as_const(&self, tab: &ValidatedSymbolTable) -> u64 {
        match self {
            ValidatedDefinition::Const(c) => c.value.as_const(tab),
            _ => panic!("not a constant"),
        }
    }
}

impl Value {
    fn as_type_name(&self, tab: &ValidatedSymbolTable) -> String {
        match self {
            Value::Int(i) => format!("{i}"),
            Value::Name(name) => tab.lookup_definition(name).as_type_name(tab),
        }
    }

    fn as_const(&self, tab: &ValidatedSymbolTable) -> u64 {
        match self {
            Value::Int(i) => *i,
            Value::Name(name) => tab.lookup_definition(name).as_const(tab),
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum Context {
    InUnion,
    NotInUnion,
}

impl Array {
    // XXX: represent arrays as slices instead of as vectors?
    fn as_type_name(&self, tab: &ValidatedSymbolTable) -> String {
        let inner_type = match &self.kind {
            ArrayKind::Ascii => {
                return match &self.size {
                    ArraySize::Limited(lim) => {
                        let lim = lim.as_const(tab);
                        format!("std::ffi::OsString /* max length: {lim} */")
                    }
                    _ => "std::ffi::OsString".to_string(),
                };
            }
            ArrayKind::Byte => "u8".to_string(),
            ArrayKind::UserType(ty) => ty.as_type_name(tab),
        };

        match &self.size {
            ArraySize::Fixed(v) => {
                let len = &match v {
                    Value::Int(i) => *i,
                    Value::Name(name) => tab.lookup_definition(name).as_const(tab),
                };
                format!("[{inner_type}; {len}]")
            }
            // XXX: different representation for upper-bounded array?
            ArraySize::Limited(lim) => {
                let lim = lim.as_const(tab);
                format!("Vec<{inner_type}> /* max length: {lim} */")
            }
            ArraySize::Unlimited => format!("Vec<{inner_type}>"),
        }
    }

    fn default_value(&self, tab: &ValidatedSymbolTable) -> String {
        match &self.size {
            ArraySize::Fixed(v) => self.fixed_length_array_initializer(v, tab),
            _ => match &self.kind {
                ArrayKind::Ascii => "std::ffi::OsString::new()".to_string(),
                _ => "Vec::new()".to_string(),
            },
        }
    }

    fn fixed_length_array_initializer(&self, val: &Value, tab: &ValidatedSymbolTable) -> String {
        let inner_type = match &self.kind {
            ArrayKind::Ascii => "std::ffi::OsString".to_string(),
            ArrayKind::Byte => "u8".to_string(),
            ArrayKind::UserType(ty) => ty.as_type_name(tab),
        };

        let inner_default_value = match &self.kind {
            ArrayKind::Ascii => "std::ffi::OsString::new()".to_string(),
            ArrayKind::Byte => "0_u8".to_string(),
            ArrayKind::UserType(ty) => ty.default_value(tab),
        };
        let mut buf = CodeBuf::new();
        let len = val.as_const(tab);
        buf.code_block("", |buf| {
            buf.block_with_trailer(
                &format!("let arr: [{inner_type}; {len}] = ::core::array::from_fn(|_|",),
                ");",
                |buf| {
                    buf.add_line(&inner_default_value);
                },
            );
            buf.add_line("arr");
        });
        buf.contents
    }
}

impl NamedDeclaration {
    fn as_type_name(&self, tab: &ValidatedSymbolTable) -> String {
        match &self.kind {
            DeclarationKind::Scalar(s) => s.as_type_name(tab),
            DeclarationKind::Array(arr) => arr.as_type_name(tab),
            DeclarationKind::Optional(o) => o.optional_type_name(tab),
        }
    }

    fn as_zcopy_dser_type_name(&self, tab: &ValidatedSymbolTable) -> String {
        match &self.kind {
            DeclarationKind::Scalar(s) => s.as_zcopy_deser_type_name(tab),
            DeclarationKind::Optional(_o) => unimplemented!(),
            DeclarationKind::Array(_arr) => unimplemented!(),
        }
    }

    fn default_value(&self, tab: &ValidatedSymbolTable) -> String {
        match &self.kind {
            DeclarationKind::Scalar(s) => s.default_value(tab),
            DeclarationKind::Array(a) => a.default_value(tab),
            DeclarationKind::Optional(o) => o.optional_default_value(tab),
        }
    }

    fn is_varlen_reader(&self, tab: &ValidatedSymbolTable) -> bool {
        match &self.kind {
            DeclarationKind::Scalar(ty) | DeclarationKind::Optional(ty) => {
                ty.is_reader(tab) && ty.size(tab).is_none()
            }
            _ => false,
        }
    }
}

impl ValidatedUnion {
    fn codegen(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable, params: &Params) {
        if !params.zcopy {
            self.default(buf, tab);
            buf.code_block(&format!("impl {}", self.name), |buf| {
                if params.alloc {
                    self.serialize_definition(buf, tab);
                }
                if params.no_alloc {
                    self.serialize_no_alloc(buf, tab);
                }
                buf.add_line("");
                self.deserialize_definition(buf, tab);
            });
        } else {
            unimplemented!();
        }
        buf.add_line("");
    }
    fn definition(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        buf.type_header();
        match &self.body {
            ValidatedUnionBody::Bool(b) => b.definition_bool(&self.name, buf, tab),
            ValidatedUnionBody::Enum(e) => e.definition_enum(&self.name, buf, tab),
        };
    }
    fn default(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        buf.code_block(&format!("impl Default for {}", self.name), |buf| {
            buf.code_block("fn default() -> Self", |buf| match &self.body {
                ValidatedUnionBody::Bool(b) => b.default_bool(buf),
                ValidatedUnionBody::Enum(e) => e.default_enum(buf, tab),
            })
        });
    }
}

impl ValidatedUnionBoolBody {
    fn definition_bool(&self, name: &str, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        // XXX: A Bool union nearly always has Void for the false arm.
        // Until I see an example where this is not the case, express it as an Option.
        let inner_type = self.true_arm.as_type_name(tab);

        buf.code_block(&format!("pub struct {name}"), |buf| {
            buf.add_line(&format!("pub inner: Option<{inner_type}>,"));
        });
    }
    fn default_bool(&self, buf: &mut CodeBuf) {
        buf.code_block("Self", |buf| {
            buf.add_line("inner: None,");
        });
    }
}

impl ValidatedUnionEnumBody {
    /// Given a union case value, which can be either an integer or an identifier, return a name
    /// suitable for a variant in a Rust enum.
    fn arm_name(val: &Value) -> String {
        match val {
            Value::Int(i) => format!("Var{i}"),
            Value::Name(n) => n.to_string(),
        }
    }
    fn definition_enum(&self, name: &str, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        buf.code_block(&format!("pub enum {name}"), |buf| {
            for arm in self.arms.iter() {
                let name = ValidatedUnionEnumBody::arm_name(&arm.0);
                match &arm.1 {
                    Declaration::Void => buf.add_line(&format!("{name},")),
                    Declaration::Named(n) => {
                        let inner_type = n.as_type_name(tab);
                        buf.add_line(&format!("{name}({inner_type}),"));
                    }
                };
            }

            match &self.default_arm {
                Some(Declaration::Void) => buf.add_line("Default,"),
                Some(Declaration::Named(n)) => {
                    let inner_type = n.as_type_name(tab);
                    buf.add_line(&format!("Default({inner_type}),"));
                }
                None => {} // Don't generate anything for absent default arm.
            }
        })
    }

    /// Serialize an Enum union, either using allocating or non-allocating code depending on the
    /// value of `alloc`.
    fn serialize_enum(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable, alloc: bool) {
        let mut max_disc = 0; // Used to determine the discriminant for a default
                              // arm, when present.
        if alloc {
            buf.add_line("let mut buf = Vec::new();");
        }
        buf.code_block("match self", |buf| {
            for arm in &self.arms {
                let arm_name = ValidatedUnionEnumBody::arm_name(&arm.0);
                match &arm.1 {
                    Declaration::Void => {
                        buf.code_block(&format!("Self::{arm_name} => "), |buf| {
                            max_disc = self
                                .serialize_discriminant_value(&arm.0, max_disc, buf, tab, alloc);
                            buf.add_line("// void");
                        });
                    }
                    Declaration::Named(n) => {
                        buf.code_block(&format!("Self::{arm_name}(inner) => "), |buf| {
                            max_disc = self
                                .serialize_discriminant_value(&arm.0, max_disc, buf, tab, alloc);
                            if alloc {
                                n.serialize_inline(Some("inner"), Context::InUnion, buf, tab);
                            } else {
                                n.serialize_no_alloc_inline(Some("inner"), buf, tab);
                            }
                        });
                    }
                };
            }
            if let Some(default_arm) = &self.default_arm {
                match default_arm {
                    Declaration::Void => {
                        buf.code_block("Self::Default => ", |buf| {
                            let _ = self.serialize_discriminant_value(
                                &Value::Int(max_disc + 1),
                                0,
                                buf,
                                tab,
                                alloc,
                            );
                            buf.add_line("// void");
                        });
                    }
                    Declaration::Named(n) => {
                        buf.code_block("Self::Default(inner) => ", |buf| {
                            let _ = self.serialize_discriminant_value(
                                &Value::Int(max_disc + 1),
                                0,
                                buf,
                                tab,
                                alloc,
                            );
                            if alloc {
                                n.serialize_inline(Some("inner"), Context::InUnion, buf, tab);
                            } else {
                                n.serialize_no_alloc_inline(Some("inner"), buf, tab);
                            }
                        });
                    }
                };
            }
        });
        if alloc {
            buf.add_line("buf");
        }
    }

    /// Get the value of `val` as a u64, and then write the code to serialize it according to
    /// `alloc`.
    ///
    /// Compare it to `max_disc` and return the larger of the two. This is to serialize default
    /// arms: they should use a discriminant value that doesn't get used for another arm.
    ///
    /// Panics if `val` won't fit into an `i32` -- it is an error to try to use such a large value
    /// as an Enum variant because the XDR spec requires Enum variants be encoded as signed ints.
    fn serialize_discriminant_value(
        &self,
        val: &Value,
        max_disc: u64,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,
        alloc: bool,
    ) -> u64 {
        let disc = self.get_discriminant_value(val, tab);
        if alloc {
            let disc: i32 = disc.try_into().unwrap();
            buf.add_line(&format!(
                "buf.extend_from_slice(&{disc}_i32.to_be_bytes());"
            ));
        } else {
            buf.serialize_int(disc.try_into().unwrap());
        }

        if disc > max_disc {
            disc
        } else {
            max_disc
        }
    }
    fn default_enum(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        let (value, declaration) = &self.arms[0];
        let name = match &value {
            Value::Int(i) => format!("Var{i}"),
            Value::Name(n) => n.to_string(),
        };
        match declaration {
            Declaration::Void => buf.add_line(&format!("Self::{name}")),
            Declaration::Named(d) => {
                let inner_default = d.default_value(tab);
                buf.add_line(&format!("Self::{name}({inner_default})"));
            }
        };
    }

    /// Given the value `val`, convert it into its integer value for encoding. If `val` is already
    /// an int, use that, otherwise if it's a string, look it up in the discriminant enum.
    fn get_discriminant_value(&self, val: &Value, tab: &ValidatedSymbolTable) -> u64 {
        match val {
            Value::Int(i) => *i,
            Value::Name(n) => {
                let Some(ref disc) = self.discriminant else {
                    panic!("BUG: attempt to use enum-style union without a discriminant");
                };
                let ValidatedDefinition::Enum(ref e) = *tab.lookup_definition(disc) else {
                    panic!("Using non-enum {n} as union discriminant is not allowed");
                };
                e.lookup_value(n, tab).unwrap()
            }
        }
    }
}

impl ValidatedStruct {
    fn get_variable_width_members(&self, tab: &ValidatedSymbolTable) -> HashSet<&String> {
        let mut deps: HashSet<&String> = HashSet::new();
        for (_, size) in self.members.iter() {
            deps.extend(size.deps.iter());
        }

        if let Some((last, _)) = self.members.last() {
            if last.size(tab).is_none() {
                deps.insert(&last.name);
            }
        }

        deps
    }

    fn codegen(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable, params: &Params) {
        if !params.zcopy {
            self.default(buf, tab);
            buf.code_block(&format!("impl {}", self.name), |buf| {
                if params.alloc {
                    self.serialize_definition(buf, tab);
                }
                if params.no_alloc {
                    self.serialize_no_alloc(buf, tab);
                }
                buf.add_line("");
                self.deserialize_definition(buf, tab);
            });
        } else {
            buf.code_block(&format!("impl<'a> {}Reader<'a>", self.name), |buf| {
                buf.code_block(
                    &format!("pub fn new(buf: &'a [u8]) -> xdr_lib::Result<Self>"),
                    |buf| {
                        buf.add_line("Self::from_buf(buf)");
                    },
                );

                self.deserialize_definition_zcopy(buf, tab);
            });
        }
        buf.add_line("");
    }

    fn definition(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        buf.type_header();
        buf.code_block(&format!("pub struct {}", self.name), |buf| {
            for (decl, _) in self.members.iter() {
                self.member_declaration(decl, buf, tab);
            }
        });
        buf.add_line("");
    }

    fn member_is_self_referential(
        &self,
        decl: &NamedDeclaration,
        tab: &ValidatedSymbolTable,
    ) -> bool {
        match &decl.kind {
            DeclarationKind::Scalar(xdr_type) => xdr_type.self_referential_optional(tab),
            _ => false,
        }
    }

    fn definition_zcopy(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        let deps = self.get_variable_width_members(tab);

        buf.add_line("#[derive(Debug, PartialEq, Clone, Default)]");
        buf.code_block(&format!("pub struct {}Reader <'a>", self.name), |buf| {
            buf.add_line("buf: &'a [u8],");
            for dep in deps.iter() {
                buf.add_line(&format!("{}_width: usize,", dep));

                let (member, _) = self.members.iter().find(|v| v.0.name == **dep).unwrap();
                if member.is_varlen_reader(tab) {
                    let typename = member.as_zcopy_dser_type_name(tab);

                    buf.add_line(&format!("{}: {},", dep, typename));
                }
            }
        });

        buf.add_line("");
        buf.code_block(
            &format!("impl<'a> xdr_lib::Reader<'a> for {}Reader <'a>", &self.name),
            |buf| {
                buf.code_block(
                    "fn from_buf(buf: &'a [u8]) -> xdr_lib::Result<Self>",
                    |buf| {
                        buf.add_line("let me = Self{ buf,..Default::default() };");
                        buf.add_line("me.validate()");
                    },
                );

                buf.code_block("fn get_width(&self) -> xdr_lib::Result<usize>", |buf| {
                    if let Some((last, last_off)) = self.members.last() {
                        let last_size = last.size(tab);
                        let mut overall_definition_size = DefinitionSize {
                            known: last_off.known + last_size.unwrap_or(0),
                            deps: last_off.deps.clone(),
                        };

                        if last_size.is_none() {
                            overall_definition_size.deps.push(last.name.clone());
                        }

                        buf.add_line(&format!(
                            "Ok({})",
                            &Self::offset_to_string(&overall_definition_size)
                        ));
                    } else {
                        buf.add_line("Ok(0)");
                    }
                });
            },
        );
    }

    fn member_declaration(
        &self,
        decl: &NamedDeclaration,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,
    ) {
        let type_name = decl.as_type_name(tab);
        buf.add_line(&format!("pub {}: {},", decl.name, type_name));
    }

    fn default(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        buf.code_block(&format!("impl Default for {}", self.name), |buf| {
            buf.code_block("fn default() -> Self", |buf| {
                buf.code_block(&self.name, |buf| {
                    for (decl, _) in self.members.iter() {
                        buf.add_line(&format!("{}: {},", decl.name, decl.default_value(tab)));
                    }
                });
            });
        });
    }
}

impl ValidatedEnum {
    fn codegen(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable, params: &Params) {
        self.default(buf);
        buf.code_block(&format!("impl {}", self.name), |buf| {
            if params.alloc {
                self.serialize_definition(buf, tab);
            }
            if params.no_alloc {
                self.serialize_no_alloc(buf, tab);
            }
            buf.add_line("");

            if params.zcopy {
                self.deserialize_definition_zcopy(buf, tab);
            } else {
                self.deserialize_definition(buf, tab);
            }
        });
        buf.add_line("");
    }
    fn default(&self, buf: &mut CodeBuf) {
        buf.code_block(&format!("impl Default for {}", self.name), |buf| {
            buf.code_block("fn default() -> Self", |buf| {
                // XXX: enum default should be "uninitialized", rather than just picking
                // the first variant... doing this for now because it's "safe"
                let default_variant = &self.variants[0];
                buf.add_line(&format!("{}::{}", self.name, default_variant.0));
            });
        });
    }
    fn definition(&self, buf: &mut CodeBuf) {
        buf.type_header();
        buf.code_block(&format!("pub enum {}", self.name), |buf| {
            for var in self.variants.iter() {
                buf.add_line(&format!("{},", var.0));
            }
        });
    }
    /// Given the string `name`, look it up in this enum and return its integer value.
    ///
    /// Returns None if `name` does not appear as a variant in this enum, and returns Err(_) if the
    /// value of `name` exists but is unresolvable.
    fn lookup_value(&self, name: &str, tab: &ValidatedSymbolTable) -> Option<u64> {
        for var in self.variants.iter() {
            if name == var.0 {
                return match &var.1 {
                    Value::Int(i) => Some(*i),
                    Value::Name(n) => Some(tab.lookup_definition(n).as_const(tab)),
                };
            }
        }

        None
    }
}

impl XdrType {
    fn as_type_name(&self, tab: &ValidatedSymbolTable) -> String {
        match self {
            XdrType::Int => "i32".to_string(),
            XdrType::UInt => "u32".to_string(),
            XdrType::Hyper => "i64".to_string(),
            XdrType::UHyper => "u64".to_string(),
            XdrType::Float => todo!(),
            XdrType::Double => todo!(),
            XdrType::Quadruple => todo!(),
            XdrType::Bool => "bool".to_string(),
            XdrType::Name(s) => tab.lookup_definition(s).as_type_name(tab),
        }
    }
    fn as_zcopy_deser_type_name(&self, tab: &ValidatedSymbolTable) -> String {
        match self {
            XdrType::Int => "i32".to_string(),
            XdrType::UInt => "u32".to_string(),
            XdrType::Hyper => "i64".to_string(),
            XdrType::UHyper => "u64".to_string(),
            XdrType::Float => todo!(),
            XdrType::Double => todo!(),
            XdrType::Quadruple => todo!(),
            XdrType::Bool => "bool".to_string(),
            XdrType::Name(s) => tab.lookup_definition(s).as_zcopy_deser_type_name(tab),
        }
    }

    fn default_value(&self, tab: &ValidatedSymbolTable) -> String {
        match self {
            XdrType::Int => "0".to_string(),
            XdrType::UInt => "0".to_string(),
            XdrType::Hyper => "0".to_string(),
            XdrType::UHyper => "0".to_string(),
            XdrType::Float => "0.0".to_string(),
            XdrType::Double => "0.0".to_string(),
            XdrType::Quadruple => "0.0".to_string(),
            XdrType::Bool => "false".to_string(),
            XdrType::Name(n) => {
                let definition = tab.lookup_definition(n);
                match *definition {
                    ValidatedDefinition::TypeDef(ref tdef) => tdef.decl.default_value(tab),
                    _ => format!("{n}::default()"),
                }
            }
        }
    }

    /// Given a variable named `var_name`, generate the appropriate code to serialize it based on
    /// its type. Note that this assumes the serializer is allocating (non-allocating serializer
    /// doesn't use this method.)
    ///
    /// For example, given an XdrType::Int named `foo`, returns:
    ///
    ///     "foo.to_be_bytes()"
    ///
    /// or given an XdrType::Name("bar"):
    ///
    ///     "bar.serialize_alloc()"
    fn serialize_method_string(&self, var_name: &str, tab: &ValidatedSymbolTable) -> String {
        let (func_name, func_kind) = self.serialize_method(tab);
        match func_kind {
            FunctionKind::Function => {
                format!("{func_name}(&{var_name})")
            }
            FunctionKind::Method => {
                format!("{var_name}.{func_name}")
            }
        }
    }

    /// Add the method to serialize an XdrType, assumed to be inline within a function for a
    /// top-level "container" type such as a `struct` or `union`.
    ///
    /// This code is inserted as `method` in:
    ///    `let bytes = decl.method();`
    ///                      ^^^^^^
    ///    `v.extend_from_slice(&bytes);`
    ///
    fn serialize_method(&self, tab: &ValidatedSymbolTable) -> (String, FunctionKind) {
        let method = match self {
            XdrType::Int => "to_be_bytes()",
            XdrType::UInt => "to_be_bytes()",
            XdrType::Hyper => "to_be_bytes()",
            XdrType::UHyper => "to_be_bytes()",
            XdrType::Float => todo!(),
            XdrType::Double => todo!(),
            XdrType::Quadruple => todo!(),
            XdrType::Bool => {
                return (
                    "xdr_lib::serialize_bool".to_string(),
                    FunctionKind::Function,
                )
            }
            XdrType::Name(name) => match *tab.lookup_definition(name) {
                ValidatedDefinition::TypeDef(_) => unreachable!(
                    "BUG: Typedef should have already been handled in serialize_inline()"
                ),
                _ => "serialize_alloc()",
            },
        }
        .to_string();

        (method, FunctionKind::Method)
    }

    /// Check if this XdrType is a "self-referential optional" type, that is, something like
    ///    struct foo {
    ///        int data;
    ///        foo *next;
    ///    };
    ///
    /// Such types are represented in Rust as Vectors, rather than linked lists.
    /// Non-self-referential optional types are represented as Rust Options.
    fn self_referential_optional(&self, tab: &ValidatedSymbolTable) -> bool {
        let XdrType::Name(n) = self else {
            return false;
        };

        let ValidatedDefinition::Struct(ref s) = *tab.lookup_definition(n) else {
            return false;
        };

        s.self_referential_optional
    }
    fn optional_type_name(&self, tab: &ValidatedSymbolTable) -> String {
        let inner_type = self.as_type_name(tab);

        if self.self_referential_optional(tab) {
            format!("Vec<{inner_type}>")
        } else {
            format!("Option<{inner_type}>")
        }
    }
    fn optional_default_value(&self, tab: &ValidatedSymbolTable) -> String {
        if self.self_referential_optional(tab) {
            "Vec::new()"
        } else {
            "None"
        }
        .to_string()
    }

    fn is_reader(&self, tab: &ValidatedSymbolTable) -> bool {
        match self {
            XdrType::Name(n) => tab.lookup_definition(n).is_reader(tab),
            _ => false,
        }
    }
}

struct CodeBuf {
    contents: String,
    indent_level: usize,
}

impl CodeBuf {
    pub fn new() -> Self {
        CodeBuf {
            contents: String::new(),
            indent_level: 0,
        }
    }

    /// Add one level of indentation.
    pub fn indent(&mut self) {
        self.indent_level += 1;
    }

    /// Remove one level of indentation.
    pub fn outdent(&mut self) {
        self.indent_level -= 1;
    }

    /// Format a code block, which is `start` followed by '{', then call the provided closure
    /// to format the block contents with an additional level of indentation, then format a closing
    /// '}'.
    pub fn code_block<F>(&mut self, start: &str, f: F)
    where
        F: FnMut(&mut CodeBuf),
    {
        self.block_with_trailer(start, "", f)
    }

    /// Same as `code_block()`, but terminate the block with a semicolon to make it a statement.
    pub fn block_statement<F>(&mut self, start: &str, f: F)
    where
        F: FnMut(&mut CodeBuf),
    {
        self.block_with_trailer(start, ";", f)
    }

    pub fn block_with_trailer<F>(&mut self, start: &str, trailer: &str, mut f: F)
    where
        F: FnMut(&mut CodeBuf),
    {
        self.add_contents(start);
        self.contents.push_str(" {\n");
        self.indent();
        f(self);
        self.outdent();
        self.add_line(&format!("}}{trailer}"));
    }

    /// Append the given `contents` to the buffer.
    fn add_contents(&mut self, contents: &str) {
        self.contents.push_str(&"    ".repeat(self.indent_level));
        self.contents.push_str(contents);
    }

    /// Append the given `line` to the buffer, and then append a newline character.
    ///
    /// If the user actually passes multiple lines, split those up so that each line gets the right
    /// amount of indentation.
    pub fn add_line(&mut self, lines: &str) {
        for line in lines.lines() {
            self.add_contents(line);
            self.contents.push('\n');
        }
    }

    /// Write standard "derive"s that each type definition should have.
    /// TODO: come up with a mechanism to add "Copy" to types for which it's appropriate?
    pub fn type_header(&mut self) {
        self.add_line("#[derive(Debug, PartialEq, Clone)]");
    }
}
