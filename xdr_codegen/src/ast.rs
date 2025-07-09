// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

#[derive(Debug)]
pub struct Schema {
    pub definitions: Vec<Definition>,
    pub programs: Vec<Program>,
    /// If the schema has any string type within it -- need to know during code generation
    pub contains_string: bool,
}

#[derive(Debug)]
pub struct Program {
    pub name: String,
    pub versions: Vec<ProgramVersion>,
    pub id: u32,
}

#[derive(Debug)]
pub struct ProgramVersion {
    pub name: String,
    pub procedures: Vec<Procedure>,
    pub id: u32,
}

#[derive(Debug)]
pub struct Procedure {
    pub name: String,
    pub _arg: ProcedureType,
    pub _ret: ProcedureType,
    pub id: u32,
}

/// Represents both the argument and return value type of a procedure.
#[derive(Debug)]
#[allow(dead_code)]
pub enum ProcedureType {
    Ty(XdrType),
    Void,
}

#[derive(Debug, Clone)]
pub enum Definition {
    Const(ConstDefinition),
    TypeDef(XdrTypeDef),
    Struct(XdrStruct),
    Enum(XdrEnum),
    Union(XdrUnion),
}

#[derive(Debug, Clone)]
pub struct ConstDefinition {
    pub name: String,
    pub value: Value,
}

#[derive(Debug, Clone)]
pub struct XdrTypeDef {
    pub decl: Declaration,
}

/// For strings that are not used for their own value, but to resolve to another type.
pub type UnresolvedName = String;

#[derive(Debug, PartialEq, Clone)]
pub enum XdrType {
    // XXX: encode the number of bits in the int type name here?
    /// a 32-bit quantity
    Int,
    UInt,
    /// a 64-bit quantity
    Hyper,
    UHyper,
    Float,
    Double,
    Quadruple,
    Bool,
    Name(UnresolvedName),
}

/// "Enumerations have the same representation as signed integers."
#[derive(Debug, PartialEq, Clone)]
pub struct XdrEnum {
    pub name: String,
    pub variants: Vec<(String, Value)>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct XdrStruct {
    // TODO: store snake_case -> CameCase transformed name...
    pub name: String,
    pub members: Vec<Declaration>,

    /// Structs that have an optional "pointer" to themselves at the end need special handling
    /// during codegen. This field is filled in during Schema::validate().
    pub self_referential_optional: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub struct XdrUnion {
    pub name: String,
    pub body: XdrUnionBody,
}

/// An XDR Union can be discriminated by either an [un]signed int, bool, or an enum.
/// (int-discriminated unions are represented in the enum case.)
#[derive(Debug, PartialEq, Clone)]
pub enum XdrUnionBody {
    Bool(XdrUnionBoolBody),
    Enum(XdrUnionEnumBody),
}

#[derive(Debug, PartialEq, Clone)]
pub struct XdrUnionBoolBody {
    pub true_arm: Declaration,
    /// False arm and default arm are equivalent for a bool union.
    /// False arm should always appear but is typically 'void'.
    // XXX: get rid of false_arm entirely and force this to be basically an Option<>?
    pub false_arm: Declaration,
}

/// An "enum" style union (as opposed to a bool style union) is used for enum-discriminated as well
/// as [unsigned] int-discriminated unions.
///
/// In the int case, only the Value -> Declaration mappings are needed (so the discriminant is
/// None).
///
/// In the enum case, the name of the enum is needed in order to resolve the enum variant to the
/// right integer value.
#[derive(Debug, PartialEq, Clone)]
pub struct XdrUnionEnumBody {
    pub discriminant: Option<UnresolvedName>,
    pub arms: Vec<UnionArm>,
    pub default_arm: DefaultUnionArm,
}

pub type UnionArm = (Value, Declaration);
pub type DefaultUnionArm = Option<Declaration>;

/// An XDR array may hold opaque data (= bytes), a string (= ASCII), or elements of any other
/// type with a given name.
///
/// Its length may be fixed, or variable with an optional limit.
///
/// Technically, the standard doesn't allow for fixed length strings, but it should be harmless to
/// have a representation of that.
#[derive(Debug, PartialEq, Clone)]
pub struct Array {
    pub kind: ArrayKind,
    pub size: ArraySize,
}

#[derive(Debug, PartialEq, Clone)]
pub enum ArrayKind {
    Byte,
    Ascii,
    UserType(XdrType),
}

#[derive(Debug, PartialEq, Clone)]
pub enum ArraySize {
    Fixed(Value),
    Limited(Value),
    Unlimited,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    Int(u64),
    Name(UnresolvedName),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Declaration {
    Named(NamedDeclaration),
    Void,
}

#[derive(Debug, PartialEq, Clone)]
pub struct NamedDeclaration {
    pub name: String,
    pub kind: DeclarationKind,
}

#[derive(Debug, PartialEq, Clone)]
pub enum DeclarationKind {
    Scalar(XdrType),
    Array(Array),
    Optional(XdrType),
}
