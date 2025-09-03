// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use crate::{ast::*, symbol_table::*, XdrError};

pub struct ValidatedSchema {
    /// This owns the definitions of the... definitions.
    pub symbol_table: SymbolTable,

    /// This list exists so that codegen can output code for types in the same order as those types
    /// appear in the source. The `String`s are keys into the `symbol_table`.
    pub definition_list: Vec<String>,

    pub programs: Vec<Program>,
    pub contains_string: bool,
}

impl ValidatedSchema {
    /// Validate a schema, eventually ensuring that it doesn't have any errors that would prevent
    /// succesful code generation.
    ///
    /// (For now, it only checks some errors, so finding errors during codegen is still possible.)
    pub fn validate(schema: Schema) -> crate::Result<ValidatedSchema> {
        let (symbol_table, definition_list) = SymbolTable::new(&schema);

        for (_, definition) in symbol_table.tab.iter() {
            definition.borrow_mut().validate(&symbol_table)?;
        }

        Ok(ValidatedSchema {
            symbol_table,
            definition_list,
            programs: schema.programs,
            contains_string: schema.contains_string,
        })
    }
}

impl Definition {
    fn validate(&mut self, tab: &SymbolTable) -> crate::Result<()> {
        match self {
            Definition::Const(_) => {}
            Definition::TypeDef(_) => {}
            Definition::Struct(s) => s.validate(tab)?,
            Definition::Enum(_) => {}
            Definition::Union(_) => {}
        };

        Ok(())
    }
}

impl XdrStruct {
    fn validate(&mut self, tab: &SymbolTable) -> crate::Result<()> {
        self.self_referential_optional(tab)
    }

    /// Determine if a struct has a "self-referential optional":
    ///
    ///    struct foo {
    ///        /* initial fields */
    ///        ...
    ///        foo *next;       /* recursive */
    ///    };
    ///
    /// To simplify code generation, only allow a self-referential optional as the final field of
    /// the struct. If such a member occurred in the middle of a struct, it would complicate
    /// correct [de]seriailizing, but I've never seen such a struct in an actual protocol
    /// definition, so simply don't allow it.
    fn self_referential_optional(&mut self, tab: &SymbolTable) -> crate::Result<()> {
        for member in self.members.iter() {
            if self.self_referential_optional {
                return Err(XdrError::UnsupportedOptional(self.name.clone()));
            }
            if is_declaration_option_of_name(&self.name, member, tab) {
                self.self_referential_optional = true;
            }
        }

        // For self-referential optional types, the last member, an optional "pointer" to the next
        // item, is serialized by the container type which holds the "linked list" (actually a
        // Vector in the Rust representation).
        //
        if self.self_referential_optional {
            self.members.pop();
        }

        Ok(())
    }
}

/// Determine if the given declaration is an optional field of type `outer_name`.
///
/// This is recursive because a declaration might refer to a typedef, which might in turn refer to
/// an optional `outer_name`.
fn is_declaration_option_of_name(outer_name: &str, decl: &Declaration, tab: &SymbolTable) -> bool {
    match decl {
        Declaration::Named(n) => match &n.kind {
            DeclarationKind::Optional(ty) => {
                let XdrType::Name(member_type_name) = ty else {
                    return false;
                };
                if *member_type_name != outer_name {
                    return false;
                }
                true
            }
            DeclarationKind::Scalar(ty) => {
                let XdrType::Name(name) = ty else {
                    return false;
                };
                let def = tab.lookup_definition(name).expect("Undefined name");
                let Definition::TypeDef(ref typedef) = *def else {
                    return false;
                };
                is_declaration_option_of_name(outer_name, &typedef.decl, tab)
            }
            DeclarationKind::Array(_) => false,
        },
        Declaration::Void => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::{validate, Parser, Scanner, XdrError};

    fn try_validate(src: &str) -> crate::Result<()> {
        let mut parser = Parser::new(Scanner::new(src));
        let schema = parser.parse()?;
        let _ = validate::ValidatedSchema::validate(schema)?;
        Ok(())
    }

    #[test]
    fn invalid_optional() {
        let res = try_validate("struct foo { foo *next; int a; };").unwrap_err();
        assert!(matches!(res, XdrError::UnsupportedOptional(_)));
    }

    #[test]
    fn valid_optional() {
        assert!(try_validate("struct foo { int a; foo *next; };").is_ok());
    }
}
