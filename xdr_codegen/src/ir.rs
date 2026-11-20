use crate::ast::*;

#[derive(Debug, Clone, PartialEq)]
pub struct DefinitionSize {
    pub known: usize,
    pub deps: Vec<String>,
}

pub type DeclarationOfset = DefinitionSize;

impl DefinitionSize {
    pub fn is_determinate(&self) -> bool {
        self.deps.is_empty()
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

/// "Enumerations have the same representation as signed integers."
#[derive(Debug, PartialEq, Clone)]
pub struct ValidatedEnum {
    pub name: String,
    pub variants: Vec<(String, i32)>,
    pub size: DefinitionSize,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ValidatedStruct {
    // TODO: store snake_case -> CameCase transformed name...
    pub name: String,
    pub members: Vec<(NamedDeclaration, DeclarationOfset)>,
    pub size: DefinitionSize,

    /// Structs that have an optional "pointer" to themselves at the end need special handling
    /// during codegen. This field is filled in during Schema::validate().
    pub self_referential_optional: bool,
    pub contains_self_ref_opt: bool,
    pub eof_tail: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ValidatedUnion {
    pub name: String,
    pub arms: Vec<ValidatedUnionArm>,
    pub default_arm: Option<ValidatedUnionArm>,
    pub size: DefinitionSize,
}

impl ValidatedUnion {
    pub fn arms(&self) -> impl Iterator<Item = &ValidatedUnionArm> {
        self.arms.iter().chain(self.default_arm.iter())
    }

    /// True if this union is equivalent to an Option, i.e., the discriminant is a bool, the true
    /// arm is non-void, and the false arm is void.
    ///
    /// Such unions are represented as Options in Rust and therefore have different encoding and
    /// decoding.
    pub fn is_option(&self) -> bool {
        // The existence of a default arm means this is not an Option.
        if self.default_arm.is_some() {
            return false;
        }

        for arm in &self.arms {
            match arm.val {
                // False arm must be void.
                0 => {
                    if matches!(arm.decl, Declaration::Named(_)) {
                        return false;
                    }
                }

                // True arm must be non-void.
                1 => {
                    if matches!(arm.decl, Declaration::Void) {
                        return false;
                    }
                }

                // Any other discriminant value means this is not an Option.
                _ => return false,
            };
        }

        true
    }

    /// Returns the True arm of an "Option" style union. Panics if such an arm does not exist.
    pub fn true_arm(&self) -> &NamedDeclaration {
        let arm = self.arms.iter().find(|a| a.val == 1).unwrap();

        let Declaration::Named(decl) = &arm.decl else {
            panic!("'True' arm of an Option style union must not be a void declaration.");
        };

        decl
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct ValidatedUnionArm {
    /// The discriminant value for this arm.
    pub val: u32,

    /// Whether this is a default arm, which should match any value not covered by another arm.
    pub default: bool,

    pub name: String,
    pub decl: Declaration,
}
