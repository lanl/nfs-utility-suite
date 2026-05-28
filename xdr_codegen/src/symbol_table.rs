// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::{
    cell::{Ref, RefCell},
    collections::HashMap,
};

use crate::{ast::*, ir::ValidatedDefinition, XdrError};

pub struct GenericSymbolTable<T> {
    pub tab: HashMap<UnresolvedName, RefCell<T>>,
}
pub type DefinitionList = Vec<String>;

pub trait HasName {
    fn get_name(&self) -> Option<&str>;
}

impl<T> GenericSymbolTable<T>
where
    T: HasName,
    T: Clone,
{
    pub fn new(input_definitions: &[T]) -> (Self, DefinitionList) {
        let mut tab = HashMap::new();
        let mut definitions = Vec::new();

        for def in input_definitions.iter() {
            let name = def.get_name();
            if let Some(name) = name {
                tab.insert(name.to_string(), RefCell::new(def.clone()));
                definitions.push(name.to_string());
            }
        }

        (GenericSymbolTable::<T> { tab }, definitions)
    }

    /// Tries to resolve a name to its underlying type.
    pub fn lookup_definition(&self, name: &str) -> Result<Ref<'_, T>, XdrError> {
        match self.tab.get(name) {
            Some(ent) => Ok(ent.borrow()),
            None => Err(XdrError::UndefinedName(name.to_string())),
        }
    }
}

pub type SymbolTable = GenericSymbolTable<Definition>;
pub type ValidatedSymbolTable = GenericSymbolTable<ValidatedDefinition>;
