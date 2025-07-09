// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::cell::{Ref, RefCell};
use std::collections::HashMap;

use crate::ast::*;
use crate::XdrError;

pub struct SymbolTable {
    pub tab: HashMap<UnresolvedName, RefCell<Definition>>,
}

pub type DefinitionList = Vec<String>;

impl SymbolTable {
    pub fn new(schema: &Schema) -> (Self, DefinitionList) {
        let mut tab = HashMap::new();
        let mut definitions = Vec::new();

        for def in schema.definitions.iter() {
            let name = match def {
                Definition::Const(d) => &d.name,
                Definition::TypeDef(d) => match &d.decl {
                    Declaration::Named(n) => &n.name,
                    Declaration::Void => {
                        continue;
                    }
                },
                Definition::Struct(d) => &d.name,
                Definition::Enum(d) => &d.name,
                Definition::Union(d) => &d.name,
            };
            tab.insert(name.clone(), RefCell::new(def.clone()));
            definitions.push(name.clone());
        }

        (SymbolTable { tab }, definitions)
    }

    /// Tries to resolve a name to its underlying type.
    pub fn lookup_definition(&self, name: &str) -> Result<Ref<Definition>, XdrError> {
        match self.tab.get(name) {
            Some(ent) => Ok(ent.borrow()),
            None => Err(XdrError::UndefinedName(name.to_string())),
        }
    }
}
