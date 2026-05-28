// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::{
    cell::{Ref, RefCell},
    collections::HashMap,
};

use crate::{ast::*, XdrError};

pub struct SymbolTable {
    pub tab: HashMap<UnresolvedName, RefCell<Definition>>,
}

pub type DefinitionList = Vec<String>;

impl SymbolTable {
    pub fn new(schema: &Schema) -> (Self, DefinitionList) {
        let mut tab = HashMap::new();
        let mut definitions = Vec::new();

        for def in schema.definitions.iter() {
            let name = def.get_name();
            if let Some(name) = name {
                tab.insert(name.to_string(), RefCell::new(def.clone()));
                definitions.push(name.to_string());
            }
        }

        (SymbolTable { tab }, definitions)
    }

    /// Tries to resolve a name to its underlying type.
    pub fn lookup_definition(&self, name: &str) -> Result<Ref<'_, Definition>, XdrError> {
        match self.tab.get(name) {
            Some(ent) => Ok(ent.borrow()),
            None => Err(XdrError::UndefinedName(name.to_string())),
        }
    }
}
