// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::collections::HashMap;

use crate::{ast::*, ir::ValidatedDefinition, XdrError};

pub struct GenericSymbolTable<T> {
    pub tab: HashMap<UnresolvedName, T>,
}

pub trait HasName {
    fn get_name(&self) -> Option<&str>;
}

impl<T> GenericSymbolTable<T>
where
    T: HasName,
    T: Clone,
{
    pub fn new_empty() -> GenericSymbolTable<T> {
        GenericSymbolTable {
            tab: HashMap::<String, T>::new(),
        }
    }

    /// Tries to resolve a name to its underlying type.
    pub fn lookup_definition(&self, name: &str) -> Result<&T, XdrError> {
        match self.tab.get(name) {
            Some(ent) => Ok(ent),
            None => Err(XdrError::UndefinedName(name.to_string())),
        }
    }
}

pub type ValidatedSymbolTable = GenericSymbolTable<ValidatedDefinition>;
