// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

mod ast;
mod codegen;
mod parser;
mod scanner;
mod symbol_table;
mod validate;

use std::error::Error;
use std::fmt;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use parser::Parser;
use scanner::{Scanner, Token};

type Result<T> = std::result::Result<T, XdrError>;

#[derive(Debug)]
enum XdrError {
    /// For unexpected characters and the like
    _Scan((char, Token)),

    /// For parsing issues
    _Parse((Option<Token>, Option<Token>)),

    /// For unsupported optional types, contains the name of the type with the unsupported optional
    UnsupportedOptional(String),

    /// For attempting to use a name that isn't defined anywhere
    UndefinedName(String),

    /// For attempting to use a name that should resolve to a constant, when the name isn't a
    /// constant
    _NotAConstant(String),
}

impl std::error::Error for XdrError {}

impl fmt::Display for XdrError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            XdrError::_Scan((ch, tok)) => write!(
                f,
                "Unexpected character {ch} found after token {:?} on line {}",
                tok.kind, tok.line
            ),
            XdrError::_Parse(_) => todo!(),
            XdrError::UnsupportedOptional(n) => write!(f, "Unsupported optional in: {n}"),
            XdrError::UndefinedName(n) => write!(f, "Undefined name: {n}"),
            XdrError::_NotAConstant(n) => write!(f, "Not a constant: {n}"),
        }
    }
}

enum InputSource {
    StdIo,
    Files(Vec<PathBuf>),
}

pub struct Compiler {
    source: InputSource,
}

impl Compiler {
    pub fn new() -> Self {
        Compiler {
            source: InputSource::StdIo,
        }
    }

    pub fn file<P>(&mut self, path: P) -> &mut Self
    where
        P: AsRef<Path>,
    {
        match &mut self.source {
            InputSource::StdIo => {
                let source = vec![path.as_ref().to_path_buf()];
                self.source = InputSource::Files(source);
            }
            InputSource::Files(ref mut list) => {
                list.push(path.as_ref().to_path_buf());
            }
        }

        self
    }

    pub fn run(&mut self) -> std::result::Result<(), Box<dyn Error>> {
        match &self.source {
            InputSource::StdIo => {
                let mut source = Vec::new();
                io::stdin().read_to_end(&mut source)?;
                let source = String::from_utf8(source).expect("Input should be valid UTF-8");

                print!("{}", Compiler::codegen(&source, "XdrInterface")?)
            }
            InputSource::Files(list) => {
                for infile in list.iter() {
                    eprintln!("Handling file {:?}", infile.display());
                    let source = std::fs::read_to_string(infile)?;
                    let module_name = infile
                        .file_stem()
                        .unwrap_or(std::ffi::OsStr::new("XdrInterface"));
                    let code = Compiler::codegen(&source, module_name.to_str().unwrap())?;

                    let mut out_name = module_name.to_owned();
                    out_name.push(".rs");
                    let out_file = std::env::var("OUT_DIR").expect("OUT_DIR should be defined");
                    let mut out_file = PathBuf::from(out_file);
                    out_file.push(out_name);
                    std::fs::write(out_file, code)?;
                }
            }
        };

        Ok(())
    }

    fn codegen(source: &str, module_name: &str) -> Result<String> {
        let mut parser = Parser::new(Scanner::new(&source));
        let schema = parser.parse()?;
        let validated_schema = validate::ValidatedSchema::validate(schema)?;
        Ok(codegen::codegen(&validated_schema, module_name))
    }
}
