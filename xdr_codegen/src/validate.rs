// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::collections::HashSet;

use crate::{ast::*, ir::*, symbol_table::*, XdrError};

#[derive(Debug)]
pub struct ValidatedSchema {
    /// This owns the definitions of the... definitions.
    pub symbol_table: ValidatedSymbolTable,

    /// This list exists so that codegen can output code for types in the same order as those types
    /// appear in the source. The `String`s are keys into the `symbol_table`.
    pub definition_list: Vec<String>,

    pub programs: Vec<Program>,
    pub contains_string: bool,
}

impl ValidatedDefinition {
    pub fn size(&self, validated_symbol_table: &ValidatedSymbolTable) -> DefinitionSize {
        match self {
            ValidatedDefinition::Const(_) => DefinitionSize {
                known: 4, // When used as an enum discriminant, the size is 4. Otherwise, this could
                // be arbitrarily wide. That being said, the processing of the usage of constants
                // happens at compile time and thus this width is irrelevant.
                deps: Vec::new(),
            },
            ValidatedDefinition::TypeDef(type_def) => match &type_def.decl.kind {
                DeclarationKind::Scalar(xdr_type) => {
                    if let Some(size) = xdr_type.size(validated_symbol_table) {
                        DefinitionSize {
                            known: size,
                            deps: Vec::new(),
                        }
                    } else {
                        DefinitionSize {
                            known: 0,
                            deps: vec![type_def.decl.name.clone()],
                        }
                    }
                }
                DeclarationKind::Array(array) => {
                    let arr_size = array.size(validated_symbol_table);
                    if let Some(arr_size) = arr_size {
                        DefinitionSize {
                            known: arr_size,
                            deps: Vec::new(),
                        }
                    } else {
                        DefinitionSize {
                            known: 0,
                            deps: vec![type_def.decl.name.clone()],
                        }
                    }
                }
                DeclarationKind::Optional(_) => DefinitionSize {
                    known: 0,
                    deps: vec![type_def.decl.name.clone()],
                },
            },
            ValidatedDefinition::Struct(validated_struct) => validated_struct.size.clone(),
            ValidatedDefinition::Enum(_) => DefinitionSize {
                known: 4,
                deps: Vec::new(),
            },
            ValidatedDefinition::Union(validated_union) => validated_union.size.clone(),
        }
    }
}

impl ValidatedSchema {
    /// Validate a schema, eventually ensuring that it doesn't have any errors that would prevent
    /// succesful code generation.
    ///
    /// (For now, it only checks some errors, so finding errors during codegen is still possible.)
    pub fn validate(mut schema: Schema) -> crate::Result<ValidatedSchema> {
        let mut validated_symbol_table = ValidatedSymbolTable::new_empty();
        let mut definition_list = Vec::new();
        for definition in schema.definitions.drain(..) {
            let definition_name = definition.get_name().to_string();
            let validated_definition = definition.validate(&validated_symbol_table)?;

            let size = validated_definition.size(&validated_symbol_table);

            validated_symbol_table
                .tab
                .insert(definition_name.clone(), validated_definition);
            definition_list.push(definition_name.clone());
            validated_symbol_table
                .size_tab
                .insert(definition_name, size);
        }

        Ok(ValidatedSchema {
            symbol_table: validated_symbol_table,
            definition_list,
            programs: schema.programs,
            contains_string: schema.contains_string,
        })
    }
}

impl Definition {
    fn validate(self, tab: &ValidatedSymbolTable) -> crate::Result<ValidatedDefinition> {
        let ret = match self {
            Definition::Const(cdef) => match cdef.value {
                Value::Int(_) => ValidatedDefinition::Const(ConstDefinition {
                    name: cdef.name,
                    value: cdef.value,
                }),
                Value::Name(_) => {
                    return Err(XdrError::InvalidConstantDefinition(cdef.name));
                }
            },
            Definition::TypeDef(td) => ValidatedDefinition::TypeDef(td),
            Definition::Struct(s) => ValidatedDefinition::Struct(s.validate(tab)?),
            Definition::Enum(e) => ValidatedDefinition::Enum(ValidatedEnum {
                name: e.name,
                variants: e.variants,
                size: DefinitionSize {
                    known: 4,
                    deps: Vec::new(),
                },
            }),
            Definition::Union(u) => {
                let name = u.name;
                let body = u.body;
                match body {
                    XdrUnionBody::Bool(body) => body.validate(name, tab),
                    XdrUnionBody::Enum(body) => body.validate(name, tab),
                }
            }
        };

        Ok(ret)
    }
}

impl XdrType {
    pub fn size(&self, tab: &ValidatedSymbolTable) -> Option<usize> {
        match self {
            XdrType::Int | XdrType::UInt | XdrType::Float | XdrType::Bool => Some(4),
            XdrType::Hyper | XdrType::UHyper | XdrType::Double => Some(8),
            XdrType::Quadruple => Some(16),
            XdrType::Name(tn) => {
                let decl_size = tab.lookup_size(tn);
                if decl_size.is_determinate() {
                    Some(decl_size.known)
                } else {
                    None
                }
            }
        }
    }
}

impl Array {
    fn size(&self, tab: &ValidatedSymbolTable) -> Option<usize> {
        match &self.size {
            ArraySize::Fixed(value) => {
                let count = match value {
                    Value::Int(val) => *val as usize,
                    Value::Name(name) => {
                        let constval = tab.lookup_definition(name);
                        if let ValidatedDefinition::Const(constval) = constval {
                            if let Value::Int(intval) = constval.value {
                                intval as usize
                            } else {
                                panic!("constant \"{name}\" passed to array is not immediately an integer");
                            }
                        } else {
                            panic!("definition for value passed as array length specifier \"{name}\" is not a constant");
                        }
                    }
                };

                let single_width = match &self.kind {
                    ArrayKind::Byte | ArrayKind::Ascii => Some(1_usize),
                    ArrayKind::UserType(xdr_type) => xdr_type.size(tab),
                };

                single_width.map(|single_width| (single_width * count + 3) & !0b11_usize)
            }
            _ => None,
        }
    }
}

impl Declaration {
    fn name(&self) -> Option<&str> {
        match self {
            Declaration::Named(named_declaration) => Some(named_declaration.name.as_str()),
            Declaration::Void => None,
        }
    }

    fn size(&self, tab: &ValidatedSymbolTable) -> Option<usize> {
        if let Declaration::Named(m) = self {
            m.size(tab)
        } else {
            Some(0)
        }
    }
}

impl NamedDeclaration {
    pub fn size(&self, tab: &ValidatedSymbolTable) -> Option<usize> {
        match &self.kind {
            DeclarationKind::Scalar(xdr_type) => xdr_type.size(tab),
            DeclarationKind::Array(array) => array.size(tab),
            DeclarationKind::Optional(_) => None,
        }
    }
}

impl XdrStruct {
    fn validate(mut self, tab: &ValidatedSymbolTable) -> crate::Result<ValidatedStruct> {
        // if the last member is a self referential optional, we can remove it
        let has_self_reference = self.self_referential_optional(tab)?;
        if has_self_reference {
            self.members.pop();
        }

        let mut s = DefinitionSize {
            known: 0,
            deps: Vec::new(),
        };

        let members: Vec<(NamedDeclaration, DeclarationOfset)> = self
            .members
            .drain(..)
            .map(|m| {
                let m_size: Option<usize> = m.size(tab);
                let ret = (m, s.clone());

                if let Some(m_size) = m_size {
                    s.known += m_size;
                } else {
                    s.deps.push(ret.0.name.clone());
                }

                ret
            })
            .collect();

        Ok(ValidatedStruct {
            name: self.name,
            members,
            size: s,
            self_referential_optional: has_self_reference,
        })
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
    fn self_referential_optional(&self, tab: &ValidatedSymbolTable) -> crate::Result<bool> {
        let mut self_referential_optional = false;
        for member in self.members.iter() {
            if self_referential_optional {
                return Err(XdrError::UnsupportedOptional(self.name.clone()));
            }
            if is_declaration_option_of_name(&self.name, member, tab) {
                self_referential_optional = true;
            }
        }

        Ok(self_referential_optional)
    }
}

impl XdrUnionBoolBody {
    fn validate(self, u_name: String, _tab: &ValidatedSymbolTable) -> ValidatedDefinition {
        let arm_names = vec![self.true_arm.name.clone()];

        // bool union body size is never known as there is always a void member and a non void
        // member
        ValidatedDefinition::Union(ValidatedUnion {
            name: u_name,
            body: ValidatedUnionBody::Bool(ValidatedUnionBoolBody {
                true_arm: self.true_arm,
                size: DefinitionSize {
                    known: 0,
                    deps: arm_names.clone(),
                },
            }),
            size: DefinitionSize {
                known: 4,
                deps: arm_names,
            },
        })
    }
}

impl XdrUnionEnumBody {
    fn validate(mut self, u_name: String, tab: &ValidatedSymbolTable) -> ValidatedDefinition {
        let Some(discriminant_name) = &self.discriminant else {
            todo!("we do not currently support void discriminant unions")
        };

        let discriminant = tab.lookup_definition(discriminant_name);
        let all_possible: HashSet<String> = match discriminant {
            ValidatedDefinition::Enum(xdr_enum) => xdr_enum
                .variants
                .iter()
                .map(|(var_name, _)| var_name.clone())
                .collect(),
            _ => {
                todo!("we currently do not support discriminant types outside of enum for our enum discriminant")
            }
        };
        let mut left = all_possible.clone();

        for (val, _decl) in self.arms.iter() {
            match val {
                Value::Int(_) => {
                    todo!(
                        "{}: we currently do not support integer values in enum unions",
                        u_name
                    )
                }
                Value::Name(value_name) => {
                    if !all_possible.contains(value_name) {
                        panic!(
                            "{}: unknown enum type for {}: {}",
                            u_name, discriminant_name, value_name
                        )
                    }

                    if !left.remove(value_name) {
                        panic!(
                            "{}: enum variant {}::{} seems to be a duplicate case",
                            u_name, discriminant_name, value_name
                        )
                    }
                }
            }
        }

        if let Some(default_arm) = self.default_arm.as_ref() {
            self.arms
                .extend(left.drain().map(|s| (Value::Name(s), default_arm.clone())));
        }

        let mut arms_iter = self.arms.iter();
        let size = if let Some(default_arm) = self.default_arm.as_ref() {
            let default_size = default_arm.size(tab);
            if default_size.is_none() || arms_iter.all(|(_, d)| d.size(tab) == default_size) {
                default_size
            } else {
                None
            }
        } else {
            match arms_iter.next() {
                Some((_, first)) => {
                    let first_size = first.size(tab);

                    if first_size.is_none() || arms_iter.all(|(_, d)| d.size(tab) == first_size) {
                        first_size
                    } else {
                        None
                    }
                }
                None => {
                    panic!("union {} does not have any arms!", u_name)
                }
            }
        };

        let (known, deps) = if let Some(size) = size {
            (size, Vec::new())
        } else {
            (
                0,
                self.arms
                    .iter()
                    .map(|(_, arm)| arm)
                    .filter_map(|arm| arm.name())
                    .map(|name| name.to_string())
                    .collect(),
            )
        };

        ValidatedDefinition::Union(ValidatedUnion {
            name: u_name,
            body: ValidatedUnionBody::Enum(ValidatedUnionEnumBody {
                discriminant: self.discriminant,
                arms: self.arms,
                default_arm: self.default_arm,
                size: DefinitionSize {
                    known,
                    deps: deps.clone(),
                },
            }),
            size: DefinitionSize {
                known: known + 4,
                deps,
            },
        })
    }
}

/// Determine if the given declaration is an optional field of type `outer_name`.
///
/// This is recursive because a declaration might refer to a typedef, which might in turn refer to
/// an optional `outer_name`.
fn is_declaration_option_of_name(
    outer_name: &str,
    n: &NamedDeclaration,
    tab: &ValidatedSymbolTable,
) -> bool {
    match &n.kind {
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
            let def = tab.lookup_definition(name);
            let ValidatedDefinition::TypeDef(ref typedef) = *def else {
                return false;
            };
            is_declaration_option_of_name(outer_name, &typedef.decl, tab)
        }
        DeclarationKind::Array(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ast::{Array, ArrayKind, ArraySize, DeclarationKind, NamedDeclaration, Value, XdrType},
        ir::{DefinitionSize, ValidatedDefinition, ValidatedStruct},
        validate::{self, ValidatedSchema},
        Parser, Scanner, XdrError,
    };

    fn try_validate(src: &str) -> crate::Result<ValidatedSchema> {
        let mut parser = Parser::new(Scanner::new(src));
        let schema = parser.parse()?;
        validate::ValidatedSchema::validate(schema)
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

    #[test]
    fn deterministic_struct() {
        let xdr = r#"
            struct Foo {
                int a;
                opaque b[5];
                hyper c;
            };

            struct Bar {
                Foo foos[3];
                double baz;
            };
        "#;

        let res = try_validate(xdr);
        assert!(res.is_ok());

        let schema = res.unwrap();

        let foo_def = schema.symbol_table.lookup_definition("Foo");
        let ValidatedDefinition::Struct(foo_def) = foo_def else {
            panic!("Foo should be a struct");
        };

        assert!(
            foo_def.size
                == DefinitionSize {
                    known: 4 + 8 + 8,
                    deps: Vec::new(),
                }
        );

        let bar_def = schema.symbol_table.lookup_definition("Bar");
        let ValidatedDefinition::Struct(bar_def) = bar_def else {
            panic!("Bar should be a struct");
        };

        assert!(foo_def.members.len() == 3);
        let (foo_a, foo_a_offset) = &foo_def.members[0];
        assert_eq!(foo_a.name, "a");
        assert!(matches!(foo_a.kind, DeclarationKind::Scalar(XdrType::Int)));
        assert!(foo_a_offset.known == 0);
        assert!(foo_a_offset.deps.is_empty());
        assert!(foo_a.size(&schema.symbol_table) == Some(4));

        let (foo_b, foo_b_offset) = &foo_def.members[1];
        assert!(foo_b.name == "b");
        assert!(matches!(
            foo_b.kind,
            DeclarationKind::Array(Array {
                kind: ArrayKind::Byte,
                size: ArraySize::Fixed(Value::Int(5)),
            })
        ));
        assert!(foo_b_offset.known == 4);
        assert!(foo_b_offset.deps.is_empty());
        assert!(foo_b.size(&schema.symbol_table) == Some(8));

        let (foo_c, foo_c_offset) = &foo_def.members[2];
        assert!(foo_c.name == "c");
        assert!(matches!(
            foo_c.kind,
            DeclarationKind::Scalar(XdrType::Hyper)
        ));
        assert!(foo_c_offset.known == 12);
        assert!(foo_c_offset.deps.is_empty());
        assert!(foo_c.size(&schema.symbol_table) == Some(8));

        assert_eq!(bar_def.size.known, foo_def.size.known * 3 + 8);
        assert!(bar_def.size.deps.is_empty());
        assert!(bar_def.size.is_determinate());

        let (bar_foos, bar_foos_offset) = &bar_def.members[0];
        assert_eq!(bar_foos.name, "foos");
        assert_eq!(
            bar_foos_offset,
            &DefinitionSize {
                known: 0,
                deps: Vec::new(),
            }
        );

        let (bar_baz, bar_baz_offset) = &bar_def.members[1];
        assert_eq!(bar_baz.name, "baz");
        assert_eq!(
            bar_baz_offset,
            &DefinitionSize {
                known: foo_def.size.known * 3,
                deps: Vec::new(),
            }
        );
    }

    #[test]
    fn non_deterministic_struct() {
        let xdr = r#"
            struct Foo {
                int a;
                opaque b[5];
                hyper c;
            };

            struct Bar {
                opaque a[3];
                opaque b<5>;
                Foo foo;
                double baz;
            };

            struct Baz {
                int beforeBar;
                Bar bar;
                unsigned int drunk;
            };
        "#;

        let res = try_validate(xdr);
        assert!(res.is_ok());

        let schema = res.unwrap();

        let foo_def = schema.symbol_table.lookup_definition("Foo");
        let ValidatedDefinition::Struct(foo_def) = foo_def else {
            panic!("Foo should be a struct");
        };

        assert!(
            foo_def.size
                == DefinitionSize {
                    known: 4 + 8 + 8,
                    deps: Vec::new(),
                }
        );

        let bar_def = schema.symbol_table.lookup_definition("Bar");
        let ValidatedDefinition::Struct(bar_def) = bar_def else {
            panic!("Bar should be a struct");
        };
        assert_eq!(
            bar_def,
            &ValidatedStruct {
                name: "Bar".to_string(),
                members: vec![
                    (
                        NamedDeclaration {
                            name: "a".to_string(),
                            kind: DeclarationKind::Array(Array {
                                kind: ArrayKind::Byte,
                                size: ArraySize::Fixed(Value::Int(3))
                            })
                        },
                        DefinitionSize {
                            known: 0,
                            deps: Vec::new(),
                        }
                    ),
                    (
                        NamedDeclaration {
                            name: "b".to_string(),
                            kind: DeclarationKind::Array(Array {
                                kind: ArrayKind::Byte,
                                size: ArraySize::Limited(Value::Int(5))
                            })
                        },
                        DefinitionSize {
                            known: 4,
                            deps: Vec::new(),
                        }
                    ),
                    (
                        NamedDeclaration {
                            name: "foo".to_string(),
                            kind: DeclarationKind::Scalar(XdrType::Name("Foo".to_string())),
                        },
                        DefinitionSize {
                            known: 4,
                            deps: vec!["b".to_string()],
                        }
                    ),
                    (
                        NamedDeclaration {
                            name: "baz".to_string(),
                            kind: DeclarationKind::Scalar(XdrType::Double),
                        },
                        DefinitionSize {
                            known: 24,
                            deps: vec!["b".to_string()],
                        }
                    ),
                ],
                size: DefinitionSize {
                    known: 32,
                    deps: vec!["b".to_string()],
                },
                self_referential_optional: false,
            }
        );

        let baz_def = schema.symbol_table.lookup_definition("Baz");
        let ValidatedDefinition::Struct(baz_def) = baz_def else {
            panic!("Baz should be a struct");
        };
        assert_eq!(
            baz_def,
            &ValidatedStruct {
                name: "Baz".to_string(),
                members: vec![
                    (
                        NamedDeclaration {
                            name: "beforeBar".to_string(),
                            kind: DeclarationKind::Scalar(XdrType::Int),
                        },
                        DefinitionSize {
                            known: 0,
                            deps: Vec::new(),
                        }
                    ),
                    (
                        NamedDeclaration {
                            name: "bar".to_string(),
                            kind: DeclarationKind::Scalar(XdrType::Name("Bar".to_string())),
                        },
                        DefinitionSize {
                            known: 4,
                            deps: Vec::new(),
                        }
                    ),
                    (
                        NamedDeclaration {
                            name: "drunk".to_string(),
                            kind: DeclarationKind::Scalar(XdrType::UInt),
                        },
                        DefinitionSize {
                            known: 4,
                            deps: vec!["bar".to_string()],
                        }
                    ),
                ],
                size: DefinitionSize {
                    known: 8,
                    deps: vec!["bar".to_string()],
                },
                self_referential_optional: false,
            }
        );
    }

    #[test]
    fn equal_arm_union() -> Result<(), Box<dyn std::error::Error>> {
        let xdr = r#"
            enum PlantKind {
                Tree = 0,
                Grass = 1,
                Flower = 2
            };

            struct PlantKindButWrapped {
                PlantKind kind;
            };

            union Plant switch (PlantKind kind) {
            case Tree:
                int a;
            case Grass:
                PlantKindButWrapped b;
            case Flower:
                PlantKind c;
            };
        "#;

        let schema = try_validate(xdr)?;

        assert_eq!(
            schema.definition_list,
            vec!["PlantKind", "PlantKindButWrapped", "Plant"]
        );

        assert_eq!(
            schema.symbol_table.lookup_size("PlantKind"),
            &DefinitionSize {
                known: 4,
                deps: Vec::new(),
            }
        );

        assert_eq!(
            schema.symbol_table.lookup_size("PlantKindButWrapped"),
            &DefinitionSize {
                known: 4,
                deps: Vec::new(),
            }
        );

        assert_eq!(
            schema.symbol_table.lookup_size("Plant"),
            &DefinitionSize {
                known: 4 + 4,
                deps: Vec::new(),
            }
        );

        Ok(())
    }

    // TODO: missing enum case
    // TODO: duplicate enum case
    // TODO: non-void false arm
    // TODO: void true arm
    // TODO: unreachable, non-void default arm (warning?)
    // TODO: fallthrough arms?
    // TODO: duplicate enum entry (name/value)
    // TODO: void fields in a struct
    // TODO: usage before definition
    // TODO: typedef sizing
    // TODO: constants used as array sizing
    // TODO: variable length struct, partially known structs, dependencies
    // TODO: self-referential-optionals out of place
    // TODO: variable length, upper bounded length array, sizing
}
