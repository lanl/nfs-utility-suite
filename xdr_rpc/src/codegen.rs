// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use crate::ast::*;
use crate::symbol_table::SymbolTable;
use crate::validate::*;

const HELPERS: &str = r#"
pub fn get_i32(dst: &mut i32, input: &mut &[u8]) -> Result<(), DeserializeError> {
    if input.len() < 4 {
        return Err(DeserializeError);
    }
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<i32>());
    *input = rest;
    *dst = i32::from_be_bytes(int_bytes.try_into().unwrap());
    Ok(())
}

pub fn get_u32(dst: &mut u32, input: &mut &[u8]) -> Result<(), DeserializeError> {
    if input.len() < 4 {
        return Err(DeserializeError);
    }
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<u32>());
    *input = rest;
    *dst = u32::from_be_bytes(int_bytes.try_into().unwrap());
    Ok(())
}

pub fn get_i64(dst: &mut i64, input: &mut &[u8]) -> Result<(), DeserializeError> {
    if input.len() < 4 {
        return Err(DeserializeError);
    }
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<i64>());
    *input = rest;
    *dst = i64::from_be_bytes(int_bytes.try_into().unwrap());
    Ok(())
}

pub fn get_u64(dst: &mut u64, input: &mut &[u8]) -> Result<(), DeserializeError> {
    if input.len() < 4 {
        return Err(DeserializeError);
    }
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<u64>());
    *input = rest;
    *dst = u64::from_be_bytes(int_bytes.try_into().unwrap());
    Ok(())
}

pub fn get_bool(dst: &mut bool, input: &mut &[u8]) -> Result<(), DeserializeError> {
    if input.len() < 4 {
        return Err(DeserializeError);
    }
    let (bool_bytes, rest) = input.split_at(std::mem::size_of::<u32>());
    *input = rest;
    *dst = match u32::from_be_bytes(bool_bytes.try_into().unwrap()) {
        0 => false,
        _ => true,
    };
    Ok(())
}

pub fn serialize_bool(src: &bool) -> [u8; 4] {
    match src {
        true => 1_u32.to_be_bytes(),
        false => 0_u32.to_be_bytes(),
    }
}

#[derive(Debug)]
pub struct DeserializeError;

impl std::error::Error for DeserializeError {}

impl std::fmt::Display for DeserializeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Invalid input to deserialize method")
    }
}
"#;

const USE_FFI_HEADER: &str = r#"
use std::os::unix::ffi::OsStrExt;
"#;

enum FunctionKind {
    Function,
    Method,
}

pub fn codegen(schema: &ValidatedSchema, module_name: &str) -> String {
    let mut buf = CodeBuf::new();

    buf.add_line("#[allow(non_camel_case_types, non_snake_case)]");
    buf.code_block(&format!("pub mod {module_name}"), |buf| {
        if schema.contains_string {
            buf.add_line(USE_FFI_HEADER);
            buf.add_line("");
        }

        for def in schema.definition_list.iter() {
            let def = schema
                .symbol_table
                .lookup_definition(def)
                .expect("Undefined name");
            def.definition(buf, &schema.symbol_table);
        }

        for def in schema.definition_list.iter() {
            let def = schema
                .symbol_table
                .lookup_definition(def)
                .expect("Undefined name");
            def.implementation(buf, &schema.symbol_table);
        }

        for prog in schema.programs.iter() {
            prog.codegen(buf);
        }

        buf.add_line("#[allow(dead_code)]");
        buf.code_block("mod helpers", |buf| {
            for line in HELPERS.lines() {
                buf.add_line(line);
            }
        });
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

impl Definition {
    /// The definition for the type.
    fn definition(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        match self {
            Definition::Const(c) => {
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
            Definition::Enum(e) => {
                e.definition(buf);
            }
            Definition::Struct(s) => {
                s.definition(buf, tab);
            }
            Definition::TypeDef(_) => {}
            Definition::Union(u) => {
                u.definition(buf, tab);
            }
        }
    }

    /// The impl block for the type, including its serialize and deserialize methods.
    fn implementation(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        match self {
            Definition::Enum(e) => {
                e.codegen(buf, tab);
            }
            Definition::Struct(s) => {
                s.codegen(buf, tab);
            }
            Definition::TypeDef(_) => {}
            Definition::Union(u) => {
                u.codegen(buf, tab);
            }
            _ => {}
        }
    }

    /// Given a definition, get its type name in a way suitable for a struct member.
    ///
    /// If the definition is based on an UnresolvedName, then recursively look up that name in the
    /// symbol table.
    ///
    /// For example:
    ///
    ///    Definition                      Result
    ///
    ///    const FOO = 2;                  2
    ///    typedef unsigned long uint32;   u32
    ///    typedef uid3 uint32             u32     (resolves via above typedef)
    ///    struct blah { /* ... */ };      blah
    fn as_type_name(&self, tab: &SymbolTable) -> String {
        match self {
            Definition::Struct(s) => s.name.to_string(),
            Definition::Enum(e) => e.name.to_string(),
            Definition::Union(u) => u.name.to_string(),
            Definition::Const(c) => c.value.as_type_name(tab),
            Definition::TypeDef(t) => match &t.decl {
                Declaration::Named(n) => match &n.kind {
                    DeclarationKind::Scalar(ty) => ty.as_type_name(tab),
                    DeclarationKind::Optional(o) => o.optional_type_name(tab),
                    DeclarationKind::Array(arr) => arr.as_type_name(tab),
                },
                Declaration::Void => panic!("not supporting void in typedef..."),
            },
        }
    }

    fn as_const(&self, tab: &SymbolTable) -> u64 {
        match self {
            Definition::Const(c) => c.value.as_const(tab),
            _ => panic!("not a constant"),
        }
    }
}

impl Value {
    fn as_type_name(&self, tab: &SymbolTable) -> String {
        match self {
            Value::Int(i) => format!("{}", i),
            Value::Name(name) => tab
                .lookup_definition(&name)
                .expect("undefined name")
                .as_type_name(tab),
        }
    }

    fn as_const(&self, tab: &SymbolTable) -> u64 {
        match self {
            Value::Int(i) => *i,
            Value::Name(name) => tab
                .lookup_definition(&name)
                .expect("undefined name")
                .as_const(tab),
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
    fn as_type_name(&self, tab: &SymbolTable) -> String {
        let inner_type = match &self.kind {
            ArrayKind::Ascii => return "std::ffi::OsString".to_string(),
            ArrayKind::Byte => "u8".to_string(),
            ArrayKind::UserType(ty) => ty.as_type_name(tab),
        };

        match &self.size {
            ArraySize::Fixed(v) => {
                let len = &match v {
                    Value::Int(i) => *i,
                    Value::Name(name) => tab
                        .lookup_definition(&name)
                        .expect("undefined name")
                        .as_const(tab),
                };
                format!("[{inner_type}; {len}]")
            }
            // XXX: different representation for upper-bounded array?
            ArraySize::Limited(_) => format!("Vec<{inner_type}>"),
            ArraySize::Unlimited => format!("Vec<{inner_type}>"),
        }
    }

    fn default_value(&self, tab: &SymbolTable) -> String {
        match &self.size {
            ArraySize::Fixed(v) => self.fixed_length_array_initializer(&v, tab),
            _ => match &self.kind {
                ArrayKind::Ascii => "std::ffi::OsString::new()".to_string(),
                _ => "Vec::new()".to_string(),
            },
        }
    }

    fn fixed_length_array_initializer(&self, val: &Value, tab: &SymbolTable) -> String {
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
                &format!(
                    "let arr: [{}; {}] = ::core::array::from_fn(|_|",
                    inner_type, len,
                ),
                ");",
                |buf| {
                    buf.add_line(&inner_default_value);
                },
            );
            buf.add_line("arr");
        });
        buf.contents
    }

    fn serialize_inline(&self, name: &str, context: Context, buf: &mut CodeBuf, tab: &SymbolTable) {
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
                buf.add_line(&format!("buf.extend_from_slice(&vec![0; padding]);"));
            }
        };
    }

    fn deserialize_inline(&self, name: &str, buf: &mut CodeBuf, tab: &SymbolTable) {
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
                buf.add_line(&format!("let padding = (4 - len % 4) % 4;"));
                buf.add_line("let (_, rest) = input.split_at(padding as usize);");
                buf.add_line("*input = rest;");
            }
        }
    }
}

impl NamedDeclaration {
    fn as_type_name(&self, tab: &SymbolTable) -> String {
        match &self.kind {
            DeclarationKind::Scalar(s) => s.as_type_name(tab),
            DeclarationKind::Array(arr) => arr.as_type_name(tab),
            DeclarationKind::Optional(o) => o.optional_type_name(tab),
        }
    }
    fn default_value(&self, tab: &SymbolTable) -> String {
        match &self.kind {
            DeclarationKind::Scalar(s) => s.default_value(tab),
            DeclarationKind::Array(a) => a.default_value(tab),
            DeclarationKind::Optional(o) => o.optional_default_value(tab),
        }
    }
    /// Generate code to serialize a named declaration, inline within the serialization routine for
    /// another container type (struct, union, etc.)
    ///
    /// If `override_name` is `Some(name)`, then this function uses `name` for the field name
    /// instead of assuming it is named `self.member_name` (where `member_name is the name of the
    /// field in the XDR spec).
    fn serialize_inline(
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
    /// Generate code to deserialize a named declaration, inline within the deserialization routine
    /// for another container type (struct, union, etc.)
    ///
    /// If `override_name` is `Some(name)`, then this function uses `name` for the field name
    /// instead of assuming it is named `self.member_name` (where `member_name is the name of the
    /// field in the XDR spec).
    fn deserialize_inline(
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
    fn codegen(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        self.default(buf, tab);
        buf.code_block(&format!("impl {}", self.name), |buf| {
            self.serialize_definition(buf, tab);
            buf.add_line("");
            self.deserialize_definition(buf, tab);
        });
        buf.add_line("");
    }
    fn definition(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        buf.type_header();
        match &self.body {
            XdrUnionBody::Bool(b) => b.definition_bool(&self.name, buf, tab),
            XdrUnionBody::Enum(e) => e.definition_enum(&self.name, buf, tab),
        };
    }
    fn default(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        buf.code_block(&format!("impl Default for {}", self.name), |buf| {
            buf.code_block("fn default() -> Self", |buf| match &self.body {
                XdrUnionBody::Bool(b) => b.default_bool(buf),
                XdrUnionBody::Enum(e) => e.default_enum(buf, tab),
            })
        });
    }
    fn serialize_definition(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        buf.code_block(
            "pub fn serialize_alloc(&self) -> Vec<u8>",
            |buf| match &self.body {
                XdrUnionBody::Bool(b) => b.serialize_bool(buf, tab),
                XdrUnionBody::Enum(e) => e.serialize_enum(buf, tab),
            },
        );
    }
    fn deserialize_definition(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
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
    fn definition_bool(&self, name: &str, buf: &mut CodeBuf, tab: &SymbolTable) {
        // XXX: A Bool union nearly always has Void for the false arm.
        // Until I see an example where this is not the case, express it as an Option.
        let Declaration::Void = self.false_arm else {
            unimplemented!("Bool union with non-Void false arm is not supported");
        };

        let inner_type = match &self.true_arm {
            Declaration::Named(n) => n.as_type_name(tab),
            Declaration::Void => "()".to_string(),
        };

        buf.code_block(&format!("pub struct {}", name), |buf| {
            buf.add_line(&format!("pub inner: Option<{inner_type}>,"));
        });
    }
    fn default_bool(&self, buf: &mut CodeBuf) {
        buf.code_block("Self", |buf| {
            buf.add_line("inner: None,");
        });
    }
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
    fn deserialize_bool(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
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
    /// Given a union case value, which can be either an integer or an identifier, return a name
    /// suitable for a variant in a Rust enum.
    fn arm_name(val: &Value) -> String {
        match val {
            Value::Int(i) => format!("Var{i}"),
            Value::Name(n) => n.to_string(),
        }
    }
    fn definition_enum(&self, name: &str, buf: &mut CodeBuf, tab: &SymbolTable) {
        buf.code_block(&format!("pub enum {}", name), |buf| {
            for arm in self.arms.iter() {
                let name = XdrUnionEnumBody::arm_name(&arm.0);
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
    fn default_enum(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
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

    fn serialize_enum(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        let mut max_disc = 0; // Used to determine the discriminant for a default
                              // arm, when present.
        buf.add_line("let mut buf = Vec::new();");
        buf.code_block("match self", |buf| {
            for arm in self.arms.iter() {
                let arm_name = XdrUnionEnumBody::arm_name(&arm.0);
                match &arm.1 {
                    Declaration::Void => {
                        buf.code_block(&format!("Self::{} => ", arm_name), |buf| {
                            max_disc =
                                self.serialize_discriminant_value(&arm.0, max_disc, buf, tab);
                            buf.add_line("// void");
                        });
                    }
                    Declaration::Named(n) => {
                        buf.code_block(&format!("Self::{}(inner) => ", arm_name), |buf| {
                            max_disc =
                                self.serialize_discriminant_value(&arm.0, max_disc, buf, tab);
                            n.serialize_inline(Some("inner"), Context::InUnion, buf, tab);
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
                            );
                            n.serialize_inline(Some("inner"), Context::InUnion, buf, tab);
                        });
                    }
                };
            }
        });
        buf.add_line("buf");
    }
    /// Get the value of `val` as a u64, and then serialize it.
    ///
    /// Compare it to `max_disc` and return the larger of the two. This is to serialize default
    /// arms: they should use a discriminant value that doesn't get used for another arm.
    fn serialize_discriminant_value(
        &self,
        val: &Value,
        max_disc: u64,
        buf: &mut CodeBuf,
        tab: &SymbolTable,
    ) -> u64 {
        let disc = self.get_discriminant_value(val, tab);
        buf.add_line(&format!(
            "buf.extend_from_slice(&{}_i32.to_be_bytes());",
            disc
        ));

        if disc > max_disc {
            disc
        } else {
            max_disc
        }
    }
    /// Given the value `val`, convert it into its integer value for encoding. If `val` is already
    /// an int, use that, otherwise if it's a string, look it up in the discriminant enum.
    fn get_discriminant_value(&self, val: &Value, tab: &SymbolTable) -> u64 {
        match val {
            Value::Int(i) => *i,
            Value::Name(n) => {
                let Some(ref disc) = self.discriminant else {
                    panic!("BUG: attempt to use enum-style union without a discriminant");
                };
                let Definition::Enum(ref e) = *tab.lookup_definition(&disc).unwrap() else {
                    panic!("Using non-enum {n} as union discriminant is not allowed");
                };
                let val = e.lookup_value(&n, tab).unwrap();
                val
            }
        }
    }
    fn deserialize_enum(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
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
                            buf.add_line(&format!("Self::{}(inner) ", arm_name));
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
                buf.add_line(
                    "_ => panic!(\"invalid discriminant value in union without default case\")",
                );
            }
        });
    }
}

impl XdrStruct {
    fn codegen(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        self.default(buf, tab);
        buf.code_block(&format!("impl {}", self.name), |buf| {
            self.serialize_definition(buf, tab);
            buf.add_line("");
            self.deserialize_definition(buf, tab);
        });
        buf.add_line("");
    }

    fn definition(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        buf.type_header();
        buf.code_block(&format!("pub struct {}", self.name), |buf| {
            for decl in self.members.iter() {
                let Declaration::Named(decl) = decl else {
                    unimplemented!("'void' is not supported as a struct member");
                };
                self.member_declaration(&decl, buf, tab);
            }
        });
        buf.add_line("");
    }

    fn member_declaration(&self, decl: &NamedDeclaration, buf: &mut CodeBuf, tab: &SymbolTable) {
        let type_name = decl.as_type_name(tab);
        buf.add_line(&format!("pub {}: {},", decl.name, type_name));
    }

    fn default(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        buf.code_block(&format!("impl Default for {}", self.name), |buf| {
            buf.code_block("fn default() -> Self", |buf| {
                buf.code_block(&format!("{}", self.name), |buf| {
                    for decl in self.members.iter() {
                        let Declaration::Named(decl) = decl else {
                            unimplemented!("'void' is not supported as a struct member");
                        };
                        buf.add_line(&format!("{}: {},", decl.name, decl.default_value(tab)));
                    }
                });
            });
        });
    }

    fn serialize_definition(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
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

    fn deserialize_definition(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
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
    fn codegen(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        self.default(buf);
        buf.code_block(&format!("impl {}", self.name), |buf| {
            self.serialize_definition(buf, tab);
            buf.add_line("");
            self.deserialize_definition(buf, tab);
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
    fn serialize_definition(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
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
    fn deserialize_definition(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
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
                    buf.add_line("_ => panic!(\"invalid enum value: {}\", val)");
                });
                buf.add_line("Ok(())");
            },
        );
    }
    /// Given the string `name`, look it up in this enum and return its integer value.
    ///
    /// Returns None if `name` does not appear as a variant in this enum, and returns Err(_) if the
    /// value of `name` exists but is unresolvable.
    fn lookup_value(&self, name: &str, tab: &SymbolTable) -> Option<u64> {
        for var in self.variants.iter() {
            if name == &var.0 {
                return match &var.1 {
                    Value::Int(i) => Some(*i),
                    Value::Name(n) => Some(
                        tab.lookup_definition(n)
                            .expect("undefined name")
                            .as_const(tab),
                    ),
                };
            }
        }

        None
    }
}

impl XdrType {
    fn as_type_name(&self, tab: &SymbolTable) -> String {
        match self {
            XdrType::Int => "i32".to_string(),
            XdrType::UInt => "u32".to_string(),
            XdrType::Hyper => "i64".to_string(),
            XdrType::UHyper => "u64".to_string(),
            XdrType::Float => todo!(),
            XdrType::Double => todo!(),
            XdrType::Quadruple => todo!(),
            XdrType::Bool => "bool".to_string(),
            XdrType::Name(s) => tab
                .lookup_definition(s)
                .expect("undefined name")
                .as_type_name(tab),
        }
    }

    fn default_value(&self, tab: &SymbolTable) -> String {
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
                let definition = tab.lookup_definition(n).unwrap();
                match *definition {
                    Definition::TypeDef(ref tdef) => match &tdef.decl {
                        Declaration::Void => panic!("void default value not supported"),
                        Declaration::Named(n) => n.default_value(tab),
                    },
                    _ => format!("{n}::default()"),
                }
            }
        }
    }

    fn serialize_inline(
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
        let (func_name, func_kind) = self.serialize_method(tab);
        match func_kind {
            FunctionKind::Function => {
                buf.add_line(&format!("let bytes = {}(&{});", func_name, var_name));
            }
            FunctionKind::Method => {
                buf.add_line(&format!("let bytes = {}.{};", var_name, func_name));
            }
        };
        buf.add_line("buf.extend_from_slice(&bytes);");
    }

    /// Add the method to serialize an XdrType, assumed to be inline within a function for a
    /// top-level "container" type such as a `struct` or `union`.
    ///
    /// This code is inserted as `method` in:
    ///    `let bytes = decl.method();`
    ///                      ^^^^^^
    ///    `v.extend_from_slice(&bytes);`
    ///
    fn serialize_method(&self, tab: &SymbolTable) -> (String, FunctionKind) {
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
                    "helpers::serialize_bool".to_string(),
                    FunctionKind::Function,
                )
            }
            XdrType::Name(name) => match *tab.lookup_definition(name).unwrap() {
                Definition::TypeDef(_) => unreachable!(
                    "BUG: Typedef should have already been handled in serialize_inline()"
                ),
                _ => "serialize_alloc()",
            },
        }
        .to_string();

        (method, FunctionKind::Method)
    }

    fn deserialize_inline(&self, var_name: &str, buf: &mut CodeBuf, tab: &SymbolTable) {
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
    /// Check if this XdrType is a "self-referential optional" type, that is, something like
    ///    struct foo {
    ///        int data;
    ///        foo *next;
    ///    };
    ///
    /// Such types are represented in Rust as Vectors, rather than linked lists.
    /// Non-self-referential optional types are represented as Rust Options.
    fn self_referential_optional(&self, tab: &SymbolTable) -> bool {
        let XdrType::Name(n) = self else {
            return false;
        };

        let Definition::Struct(ref s) = *tab.lookup_definition(n).expect("undefined name") else {
            return false;
        };

        s.self_referential_optional
    }
    fn optional_type_name(&self, tab: &SymbolTable) -> String {
        let inner_type = self.as_type_name(tab);

        if self.self_referential_optional(tab) {
            format!("Vec<{inner_type}>")
        } else {
            format!("Option<{inner_type}>")
        }
    }
    fn optional_default_value(&self, tab: &SymbolTable) -> String {
        if self.self_referential_optional(tab) {
            "Vec::new()"
        } else {
            "None"
        }
        .to_string()
    }
    fn serialize_optional_inline(
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
    fn deserialize_optional_inline(&self, name: &str, buf: &mut CodeBuf, tab: &SymbolTable) {
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
        self.add_line(&format!("}}{}", trailer,));
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
            self.contents.push_str("\n");
        }
    }

    /// Write standard "derive"s that each type definition should have.
    /// TODO: come up with a mechanism to add "Copy" to types for which it's appropriate?
    pub fn type_header(&mut self) {
        self.add_line("#[derive(Debug, PartialEq, Clone)]");
    }
}
