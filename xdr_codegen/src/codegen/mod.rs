// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

// This file does code generation for allocating serialization routines which return a Vec<u8>,
// and de-serialization routines.

use crate::ast::*;
use crate::symbol_table::SymbolTable;
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
}

impl Default for Params {
    fn default() -> Self {
        Self {
            no_alloc: false,
            alloc: true,
        }
    }
}

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

/// Serialization method kind: either allocating, or non-allocating.
enum SerializeKind {
    Alloc,
    NoAlloc,
}

pub fn codegen(schema: &ValidatedSchema, module_name: &str, params: &Params) -> String {
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
            def.implementation(buf, &schema.symbol_table, params);
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
    fn implementation(&self, buf: &mut CodeBuf, tab: &SymbolTable, params: &Params) {
        match self {
            Definition::Enum(e) => {
                e.codegen(buf, tab, params);
            }
            Definition::Struct(s) => {
                s.codegen(buf, tab, params);
            }
            Definition::Union(u) => {
                u.codegen(buf, tab, params);
            }
            Definition::TypeDef(_) | Definition::Const(_) => {}
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
            Value::Int(i) => format!("{i}"),
            Value::Name(name) => tab
                .lookup_definition(name)
                .expect("undefined name")
                .as_type_name(tab),
        }
    }

    fn as_const(&self, tab: &SymbolTable) -> u64 {
        match self {
            Value::Int(i) => *i,
            Value::Name(name) => tab
                .lookup_definition(name)
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
                        .lookup_definition(name)
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
            ArraySize::Fixed(v) => self.fixed_length_array_initializer(v, tab),
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
}

impl XdrUnion {
    fn codegen(&self, buf: &mut CodeBuf, tab: &SymbolTable, params: &Params) {
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
        buf.code_block(&format!("pub enum {name}"), |buf| {
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

    /// Given the value `val`, convert it into its integer value for encoding. If `val` is already
    /// an int, use that, otherwise if it's a string, look it up in the discriminant enum.
    fn get_discriminant_value(&self, val: &Value, tab: &SymbolTable) -> u64 {
        match val {
            Value::Int(i) => *i,
            Value::Name(n) => {
                let Some(ref disc) = self.discriminant else {
                    panic!("BUG: attempt to use enum-style union without a discriminant");
                };
                let Definition::Enum(ref e) = *tab.lookup_definition(disc).unwrap() else {
                    panic!("Using non-enum {n} as union discriminant is not allowed");
                };
                e.lookup_value(n, tab).unwrap()
            }
        }
    }
}

impl XdrStruct {
    fn codegen(&self, buf: &mut CodeBuf, tab: &SymbolTable, params: &Params) {
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
        buf.add_line("");
    }

    fn definition(&self, buf: &mut CodeBuf, tab: &SymbolTable) {
        buf.type_header();
        buf.code_block(&format!("pub struct {}", self.name), |buf| {
            for decl in self.members.iter() {
                let Declaration::Named(decl) = decl else {
                    unimplemented!("'void' is not supported as a struct member");
                };
                self.member_declaration(decl, buf, tab);
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
                buf.code_block(&self.name, |buf| {
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
}

impl XdrEnum {
    fn codegen(&self, buf: &mut CodeBuf, tab: &SymbolTable, params: &Params) {
        self.default(buf);
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
    fn lookup_value(&self, name: &str, tab: &SymbolTable) -> Option<u64> {
        for var in self.variants.iter() {
            if name == var.0 {
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

    /// Given a variable named `var_name`, generate the appropriate code to serialize it based on
    /// its type and whether the `kind` of serializer is allocating or non-allocating.
    ///
    /// For example, given an XdrType::Int named `foo`, returns:
    ///
    ///     "foo.to_be_bytes()"
    ///
    /// or given an XdrType::Name("bar"), and an allocating serializer, returns:
    ///
    ///     "bar.serialize_alloc()"
    fn serialize_method_string(
        &self,
        var_name: &str,
        kind: SerializeKind,
        tab: &SymbolTable,
    ) -> String {
        let (func_name, func_kind) = self.serialize_method(kind, tab);
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
    fn serialize_method(&self, kind: SerializeKind, tab: &SymbolTable) -> (String, FunctionKind) {
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
                _ => match kind {
                    SerializeKind::Alloc => "serialize_alloc()",
                    SerializeKind::NoAlloc => "serialize()",
                },
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
