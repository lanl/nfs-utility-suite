// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::collections::HashMap;

use crate::{ast::*, ir::ValidatedDefinition, XdrError};

#[derive(Debug)]
pub struct ValidatedSymbolTable {
    pub tab: HashMap<UnresolvedName, ValidatedDefinition>,
}

impl ValidatedSymbolTable {
    pub fn new_empty() -> ValidatedSymbolTable {
        ValidatedSymbolTable {
            tab: HashMap::<String, ValidatedDefinition>::new(),
        }
    }

    /// Tries to resolve a name to its underlying type.
    pub fn lookup_definition(&self, name: &str) -> Result<&ValidatedDefinition, XdrError> {
        match self.tab.get(name) {
            Some(ent) => Ok(ent),
            None => Err(XdrError::UndefinedName(name.to_string())),
        }
    }
}
