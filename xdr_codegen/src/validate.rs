// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::collections::{HashMap, HashSet};

use crate::{ast::*, ir::*, symbol_table::*, XdrError};

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

        let mut size_tab: HashMap<String, DefinitionSize> = HashMap::new();
        println!("processing order:");
        for definition_name in definition_list.iter() {
            println!("\t{}", definition_name);
        }
        println!();

        for definition_name in definition_list.iter() {
            println!("processing {}", definition_name);
            if let Some(definition) = symbol_table.tab.get(definition_name) {
                let res = definition.borrow_mut().validate(&symbol_table, &size_tab)?;
                let size = match &res {
                    ValidatedDefinition::Const(_) => DefinitionSize {
                        known: 4,
                        deps: Vec::new(),
                    },
                    ValidatedDefinition::TypeDef(xdr_type_def) => match &xdr_type_def.decl {
                        Declaration::Named(named_declaration) => match &named_declaration.kind {
                            DeclarationKind::Scalar(xdr_type) => {
                                if let Some(size) = xdr_type.size(&size_tab) {
                                    DefinitionSize {
                                        known: size,
                                        deps: Vec::new(),
                                    }
                                } else {
                                    DefinitionSize {
                                        known: 0,
                                        deps: vec![named_declaration.name.clone()],
                                    }
                                }
                            }
                            DeclarationKind::Array(array) => {
                                let arr_size = array.size(&symbol_table, &size_tab);
                                if let Some(arr_size) = arr_size {
                                    DefinitionSize {
                                        known: arr_size,
                                        deps: Vec::new(),
                                    }
                                } else {
                                    DefinitionSize {
                                        known: 0,
                                        deps: vec![named_declaration.name.clone()],
                                    }
                                }
                            }
                            DeclarationKind::Optional(xdr_type) => DefinitionSize {
                                known: 0,
                                deps: vec![named_declaration.name.clone()],
                            },
                        },
                        Declaration::Void => panic!("encountered null typedef"),
                    },
                    ValidatedDefinition::Struct(validated_struct) => validated_struct.size.clone(),
                    ValidatedDefinition::Enum(validated_enum) => DefinitionSize {
                        known: 4,
                        deps: Vec::new(),
                    },
                    ValidatedDefinition::Union(validated_union) => validated_union.size.clone(),
                };

                println!(
                    "sizeof({}) = {}:{}",
                    definition_name,
                    size.known,
                    size.is_determinate()
                );

                size_tab.insert(definition_name.to_string(), size.clone());
            }
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
    fn validate(
        &mut self,
        tab: &SymbolTable,
        size_tab: &HashMap<String, DefinitionSize>,
    ) -> crate::Result<ValidatedDefinition> {
        let ret = match self {
            Definition::Const(cdef) => match cdef.value {
                Value::Int(_) => ValidatedDefinition::Const(ValidatedConstDefinition {
                    name: cdef.name.clone(),
                    value: cdef.value.clone(),
                }),
                Value::Name(_) => {
                    panic!(
                        "constant \"{}\" is invalid: constants must be integers",
                        cdef.name
                    )
                }
            },
            Definition::TypeDef(td) => {
                let td_size = td.decl.size(tab, size_tab);

                ValidatedDefinition::TypeDef(ValidatedTypeDef {
                    decl: td.decl.clone(),
                })
            }
            Definition::Struct(s) => ValidatedDefinition::Struct(s.validate(tab, &size_tab)?),
            Definition::Enum(e) => ValidatedDefinition::Enum(ValidatedEnum {
                name: e.name.clone(),
                variants: e.variants.clone(),
                size: DefinitionSize {
                    known: 4,
                    deps: Vec::new(),
                },
            }),
            Definition::Union(u) => {
                match &u.body {
                    XdrUnionBody::Bool(body) => {
                        let true_size = body.true_arm.size(tab, size_tab);
                        let false_size = body.false_arm.size(tab, size_tab);

                        let (known, deps) = if true_size != None && true_size == false_size {
                            (true_size.unwrap(), Vec::new())
                        } else {
                            let arm_names: Vec<String> =
                                [body.true_arm.name(), body.false_arm.name()]
                                    .iter()
                                    .filter_map(|val| val.clone())
                                    .map(|val| val.to_string())
                                    .collect();

                            if arm_names.is_empty() {
                                panic!("both boolean arms for {} are unamed, which is weird, because the two arm sizes should have evaluated to equal eachother and never reached this case", u.name)
                            }

                            (0, arm_names)
                        };

                        ValidatedDefinition::Union(ValidatedUnion {
                            name: u.name.clone(),
                            body: ValidatedUnionBody::Bool(ValidatedUnionBoolBody {
                                true_arm: body.true_arm.clone(),
                                false_arm: body.false_arm.clone(),
                                size: DefinitionSize {
                                    known: known.clone(),
                                    deps: deps.clone(),
                                },
                            }),
                            size: DefinitionSize {
                                known: 4 + known.clone(),
                                deps: deps.clone(),
                            },
                        })
                    }
                    XdrUnionBody::Enum(body) => {
                        let mut arms_iter = body.arms.iter();

                        let Some(discriminant_name) = &body.discriminant else {
                            todo!("we do not currently support void discriminant unions")
                        };

                        let Ok(discriminant) = tab.lookup_definition(discriminant_name) else {
                            panic!("could not find discriminant \"{}\"", discriminant_name)
                        };

                        let all_possible: HashSet<String> = match &*discriminant {
                            Definition::Enum(xdr_enum) => xdr_enum
                                .variants
                                .iter()
                                .map(|(var_name, _)| var_name.clone())
                                .collect(),
                            _ => {
                                todo!("we currently do not support discriminant types outside of enum for our enum descriminant")
                            }
                        };
                        let mut left = all_possible.clone();

                        for (val, decl) in body.arms.iter() {
                            match val {
                                Value::Int(_) => {
                                    todo!("{}: we currently do not support integer values in enum unions", u.name)
                                }
                                Value::Name(value_name) => {
                                    if !all_possible.contains(value_name) {
                                        panic!(
                                            "{}: unknown enum type for {}: {}",
                                            u.name, discriminant_name, value_name
                                        )
                                    }

                                    if !left.remove(value_name) {
                                        panic!(
                                            "{}: enum variant {}::{} seems to be a duplicate case",
                                            u.name, discriminant_name, value_name
                                        )
                                    }
                                }
                            }
                        }

                        // if all the enum cases are covered by the match arms, we can elide the
                        // default case
                        let size = if body.default_arm.is_some() && !left.is_empty() {
                            let default_arm = (&body.default_arm).as_ref().unwrap();
                            let default_size = default_arm.size(tab, size_tab);
                            if default_size.is_none()
                                || arms_iter.all(|(_, d)| d.size(tab, size_tab) == default_size)
                            {
                                default_size
                            } else {
                                None
                            }
                        } else {
                            match arms_iter.next() {
                                Some((_, first)) => {
                                    let first_size = first.size(tab, size_tab);

                                    if first_size.is_none()
                                        || arms_iter
                                            .all(|(_, d)| d.size(tab, size_tab) == first_size)
                                    {
                                        first_size
                                    } else {
                                        None
                                    }
                                }
                                None => {
                                    panic!("union {} does not have any arms!", u.name)
                                }
                            }
                        };

                        let (known, deps) = if let Some(size) = size {
                            (size, Vec::new())
                        } else {
                            (
                                0,
                                body.arms
                                    .iter()
                                    .map(|(_, arm)| arm)
                                    .filter_map(|arm| arm.name())
                                    .map(|name| name.to_string())
                                    .collect(),
                            )
                        };

                        ValidatedDefinition::Union(ValidatedUnion {
                            name: u.name.clone(),
                            body: ValidatedUnionBody::Enum(ValidatedUnionEnumBody {
                                discriminant: body.discriminant.clone(),
                                arms: body.arms.clone(),
                                default_arm: body.default_arm.clone(),
                                size: DefinitionSize {
                                    known: known.clone(),
                                    deps: deps.clone(),
                                },
                            }),
                            size: DefinitionSize {
                                known: known.clone() + 4,
                                deps,
                            },
                        })
                    }
                }
            }
        };

        Ok(ret)
    }
}

impl XdrType {
    fn size(&self, size_tab: &HashMap<String, DefinitionSize>) -> Option<usize> {
        match self {
            XdrType::Int | XdrType::UInt | XdrType::Float | XdrType::Bool => Some(4),
            XdrType::Hyper | XdrType::UHyper | XdrType::Double => Some(8),
            XdrType::Quadruple => Some(16),
            XdrType::Name(tn) => {
                if let Some(decl_size) = size_tab.get(tn) {
                    if decl_size.is_determinate() {
                        Some(decl_size.known)
                    } else {
                        None
                    }
                } else {
                    panic!("could not find size information for type \"{tn}\"");
                }
            }
        }
    }
}

impl Array {
    fn size(&self, tab: &SymbolTable, size_tab: &HashMap<String, DefinitionSize>) -> Option<usize> {
        match &self.size {
            ArraySize::Fixed(value) => {
                let count = match value {
                    Value::Int(val) => val.clone() as usize,
                    Value::Name(name) => {
                        if let Ok(constval) = tab.lookup_definition(name) {
                            if let Definition::Const(constval) = &*constval {
                                if let Value::Int(intval) = constval.value {
                                    intval as usize
                                } else {
                                    panic!("constant \"{name}\" passed to array is not immediately an integer");
                                }
                            } else {
                                panic!("definition for value passed as array length specifier \"{name}\" is not a constant");
                            }
                        } else {
                            panic!("could not find definition for constant of name \"{name}\"");
                        }
                    }
                };

                let single_width = match &self.kind {
                    ArrayKind::Byte | ArrayKind::Ascii => Some(1 as usize),
                    ArrayKind::UserType(xdr_type) => xdr_type.size(size_tab),
                };

                if let Some(single_width) = single_width {
                    // Align to closest >= 4 byte multiple
                    Some((single_width * count + 3) & !(0b11 as usize))
                } else {
                    None
                }
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

    fn size(&self, tab: &SymbolTable, size_tab: &HashMap<String, DefinitionSize>) -> Option<usize> {
        if let Declaration::Named(m) = self {
            match &m.kind {
                DeclarationKind::Scalar(xdr_type) => xdr_type.size(size_tab),
                DeclarationKind::Array(array) => array.size(tab, size_tab),
                DeclarationKind::Optional(_) => None,
            }
        } else {
            Some(0)
        }
    }
}

impl XdrStruct {
    fn validate(
        &mut self,
        tab: &SymbolTable,
        size_tab: &HashMap<String, DefinitionSize>,
    ) -> crate::Result<ValidatedStruct> {
        match self.self_referential_optional(tab) {
            Err(e) => return Err(e),
            _ => {}
        }

        let mut s = DefinitionSize {
            known: 0,
            deps: Vec::new(),
        };

        let members: Vec<(Declaration, DefinitionOffset)> = self
            .members
            .iter()
            .map(|m| {
                let name: String = if let Declaration::Named(m) = m {
                    m.name.clone()
                } else {
                    panic!("Unamed declaration found!")
                };

                let m_size: Option<usize> = m.size(tab, size_tab);

                let ret = (m.clone(), s.clone());

                if let Some(m_size) = m_size {
                    s.known += m_size;
                } else {
                    s.deps.push(name);
                }

                ret
            })
            .collect();

        Ok(ValidatedStruct {
            name: self.name.clone(),
            members,
            size: s.clone(),
            self_referential_optional: self.self_referential_optional,
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
fn is_declaration_option_of_name(
    outer_name: &str,
    n: &NamedDeclaration,
    tab: &SymbolTable,
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
            let def = tab.lookup_definition(name).expect("Undefined name");
            let Definition::TypeDef(ref typedef) = *def else {
                return false;
            };
            let Declaration::Named(ref n) = typedef.decl else {
                return false;
            };
            is_declaration_option_of_name(outer_name, n, tab)
        }
        DeclarationKind::Array(_) => false,
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
