// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

// Non-allocating serialization routines for XDR data types.

use super::CodeBuf;
use crate::ast::*;
use crate::symbol_table::SymbolTable;

impl XdrUnion {
    pub(super) fn serialize_no_alloc(&self, _buf: &mut CodeBuf, _tab: &SymbolTable) {
        todo!()
    }
}

impl XdrStruct {
    pub(super) fn serialize_no_alloc(&self, _buf: &mut CodeBuf, _tab: &SymbolTable) {
        todo!()
    }
}

impl XdrEnum {
    pub(super) fn serialize_no_alloc(&self, _buf: &mut CodeBuf, _tab: &SymbolTable) {
        todo!()
    }
}
