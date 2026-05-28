use std::fmt::Display;

use crate::ast::{
    ConstDefinition, Declaration, DefaultUnionArm, Definition, UnionArm, Value, XdrStruct,
    XdrTypeDef, XdrUnion,
};

#[derive(Debug, Clone, PartialEq)]
pub struct DefinitionSize {
    pub known: usize,
    pub deps: Vec<String>,
}

pub type DefinitionOffset = DefinitionSize;

impl DefinitionSize {
    pub fn is_determinate(&self) -> bool {
        self.deps.len() == 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValidatedDefinition {
    Const(ConstDefinition),
    TypeDef(XdrTypeDef),
    Struct(ValidatedStruct),
    Enum(ValidatedEnum),
    Union(ValidatedUnion),
}

/// For strings that are not used for their own value, but to resolve to another type.
pub type UnresolvedName = String;

/// "Enumerations have the same representation as signed integers."
#[derive(Debug, PartialEq, Clone)]
pub struct ValidatedEnum {
    pub name: String,
    pub variants: Vec<(String, Value)>,
    pub size: DefinitionSize,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ValidatedStruct {
    // TODO: store snake_case -> CameCase transformed name...
    pub name: String,
    pub members: Vec<(Declaration, DefinitionOffset)>,
    pub size: DefinitionSize,

    /// Structs that have an optional "pointer" to themselves at the end need special handling
    /// during codegen. This field is filled in during Schema::validate().
    pub self_referential_optional: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ValidatedUnion {
    pub name: String,
    pub body: ValidatedUnionBody,
    pub size: DefinitionSize,
}

/// An XDR Union can be discriminated by either an [un]signed int, bool, or an enum.
/// (int-discriminated unions are represented in the enum case.)
#[derive(Debug, PartialEq, Clone)]
pub enum ValidatedUnionBody {
    Bool(ValidatedUnionBoolBody),
    Enum(ValidatedUnionEnumBody),
}

#[derive(Debug, PartialEq, Clone)]
pub struct ValidatedUnionBoolBody {
    pub true_arm: Declaration,
    /// False arm and default arm are equivalent for a bool union.
    /// False arm should always appear but is typically 'void'.
    // XXX: get rid of false_arm entirely and force this to be basically an Option<>?
    pub false_arm: Declaration,

    pub size: DefinitionSize,
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
pub struct ValidatedUnionEnumBody {
    pub discriminant: Option<UnresolvedName>,
    pub arms: Vec<UnionArm>,
    pub default_arm: DefaultUnionArm,
    pub size: DefinitionSize,
}
