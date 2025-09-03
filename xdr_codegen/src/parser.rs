// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use crate::{ast::*, scanner::*};

pub struct Parser<'src> {
    scanner: Scanner<'src>,
    current: Token,
    next: Token,
    /// When the schema contains a string type, the generated code needs to know this in order to
    /// include the right FFI modules.
    schema_contains_string: bool,
}

impl<'src> Parser<'src> {
    pub fn new(scanner: Scanner<'src>) -> Self {
        let mut parser = Parser {
            scanner,
            current: Token {
                kind: TokenKind::Eof,
                line: 0,
            },
            next: Token {
                kind: TokenKind::Eof,
                line: 0,
            },
            schema_contains_string: false,
        };

        parser.next();

        parser
    }

    pub fn parse(&mut self) -> crate::Result<Schema> {
        let mut definitions = Vec::new();
        let mut programs = Vec::new();
        loop {
            match self.peek().kind {
                TokenKind::Program => programs.push(self.program()),
                TokenKind::Eof => break,
                _ => definitions.push(self.definition()),
            }
        }
        Ok(Schema {
            definitions,
            programs,
            contains_string: self.schema_contains_string,
        })
    }

    fn program(&mut self) -> Program {
        let TokenKind::Program = self.next().kind else {
            panic!("BUG: expected 'program'");
        };

        let name = self.expect_identifier("Expected identifier after 'program'");
        self.expect(TokenKind::LeftBrace, "Expected '{' after program name");

        let mut versions = Vec::new();
        loop {
            let tok = self.next();
            match &tok.kind {
                TokenKind::Version => {
                    let name = self.expect_identifier("Expected identifier after 'version'");
                    self.expect(TokenKind::LeftBrace, "Expected '{' after version name");
                    let procedures = self.procedures();
                    self.expect(
                        TokenKind::RightBrace,
                        "Expected '}' after procedure definitions",
                    );
                    self.expect(TokenKind::Equal, "Expected '=' after version definition");
                    let id: u32 = self
                        .expect_number("Expected version number after version definition")
                        .try_into()
                        .unwrap();
                    self.expect(TokenKind::Semicolon, "Expected ';' after version defintion");

                    versions.push(ProgramVersion {
                        name,
                        procedures,
                        id,
                    });
                }
                TokenKind::RightBrace => break,
                _ => Parser::error("Expected 'version' or '}' in program definition", Some(tok)),
            }
        }

        if versions.len() == 0 {
            Parser::error("Program definition must have at least one version.", None);
        }

        self.expect(TokenKind::Equal, "Expected '=' after program definition");
        let id: u32 = self
            .expect_number("Expected program number after program definition")
            .try_into()
            .unwrap();
        self.expect(
            TokenKind::Semicolon,
            "Expected ';' after program definition",
        );

        Program { name, versions, id }
    }

    fn procedures(&mut self) -> Vec<Procedure> {
        let mut procs = Vec::new();

        loop {
            let _ret = match self.peek().kind {
                TokenKind::RightBrace => break,
                _ => self.procedure_type(),
            };
            let name = self.expect_identifier("Expected identifier in procedure definition");
            self.expect(
                TokenKind::LeftParen,
                "Expected '(' to start procedure argument list",
            );
            let _arg = self.procedure_type();
            self.expect(
                TokenKind::RightParen,
                "Expected ')' to end procedure argument list",
            );
            self.expect(
                TokenKind::Equal,
                "Expected '=' after procedure argument list",
            );
            let id: u32 = self
                .expect_number("Expected procedure number after procedure definition")
                .try_into()
                .unwrap();
            self.expect(
                TokenKind::Semicolon,
                "Expected ';' after procedure definition",
            );

            procs.push(Procedure {
                name,
                _arg,
                _ret,
                id,
            });
        }

        if procs.len() == 0 {
            Parser::error("Version definition must have at least one procedure.", None);
        }

        procs
    }

    fn procedure_type(&mut self) -> ProcedureType {
        match self.peek().kind {
            TokenKind::Void => {
                self.next();
                ProcedureType::Void
            }
            _ => ProcedureType::Ty(self.xdr_type()),
        }
    }

    fn definition(&mut self) -> Definition {
        let tok = self.next();
        let def = match &tok.kind {
            TokenKind::Const => self.const_definition(),
            TokenKind::Typedef => Definition::TypeDef(self.type_def()),
            TokenKind::Struct => {
                let name = self.expect_identifier("Expected identifier in struct definition");
                let members = self.xdr_struct_body();
                Definition::Struct(XdrStruct { name, members, self_referential_optional: false })
            }
            TokenKind::Enum => {
                let name = self.expect_identifier("Expected identifier in enum definition");
                let variants = self.xdr_enum_body();
                Definition::Enum(XdrEnum { name, variants })

            }
            TokenKind::Union => {
                Definition::Union(self.xdr_union())
            }
            _ => Parser::error(
                "Expected 'const', 'typedef', 'enum', 'union', or 'struct' to begin a type definition",
                Some(tok),
            ),
        };
        self.expect(TokenKind::Semicolon, "Expected ';' after definition");
        def
    }

    fn type_def(&mut self) -> XdrTypeDef {
        XdrTypeDef {
            decl: self.declaration(),
        }
    }

    fn const_definition(&mut self) -> Definition {
        let name = self.expect_identifier("Expected identifier in const definition");
        self.expect(TokenKind::Equal, "Expected '=' after const name");
        let tok = self.next();
        let value = match &tok.kind {
            TokenKind::Number(n) => Value::Int(*n),
            TokenKind::Identifier(name) => Value::Name(name.to_string()),
            _ => Parser::error(
                "Expected constant or identifier in const definition",
                Some(tok),
            ),
        };
        Definition::Const(ConstDefinition { name, value })
    }

    fn xdr_enum_body(&mut self) -> Vec<(String, Value)> {
        self.expect(TokenKind::LeftBrace, "enum body must start with '{'");
        let mut variants = Vec::new();
        let mut first = true;
        loop {
            if self.peek().kind == TokenKind::RightBrace {
                self.next();
                break;
            }
            if !first {
                self.expect(TokenKind::Comma, "Expected ',' after enum variant");
            }
            first = false;

            let name = self.expect_identifier("Expected enum variant to start with an identifier");
            self.expect(TokenKind::Equal, "Expected '=' after enum variant name");
            let tok = self.next();
            let value = match &tok.kind {
                TokenKind::Number(n) => Value::Int(*n),
                TokenKind::Identifier(name) => Value::Name(name.to_string()),
                _ => Parser::error("Expected number or identifier as enum value", Some(tok)),
            };
            variants.push((name, value));
        }

        if variants.len() == 0 {
            Parser::error("Enum must have at least one variant", None);
        }

        variants
    }

    fn xdr_struct_body(&mut self) -> Vec<Declaration> {
        self.expect(TokenKind::LeftBrace, "struct body must start with '{'");
        let mut members = Vec::new();
        loop {
            if self.peek().kind == TokenKind::RightBrace {
                self.next();
                break;
            }
            members.push(self.declaration());
            self.expect(TokenKind::Semicolon, "Expected ';' following declaration");
        }

        if members.len() == 0 {
            Parser::error("Struct must have at least one member", None);
        }

        members
    }

    fn xdr_union(&mut self) -> XdrUnion {
        let name = self.expect_identifier("Expected identifier in union definition");
        self.expect(TokenKind::Switch, "Expected 'switch' after union name");
        self.expect(TokenKind::LeftParen, "Expected '(' after switch");
        let tok = self.next();
        let body = match &tok.kind {
            TokenKind::Int => todo!("don't support int unions yet"),
            TokenKind::Unsigned => {
                match self.peek().kind {
                    TokenKind::Int => {
                        self.next();
                    }
                    _ => {}
                };
                self.xdr_union_discriminant_remainder();
                let (arms, default_arm) = self.xdr_union_enum_body();
                XdrUnionBody::Enum(XdrUnionEnumBody {
                    discriminant: None,
                    arms,
                    default_arm,
                })
            }
            TokenKind::Identifier(ref discriminant) => {
                let discriminant = discriminant.to_string();
                self.xdr_union_discriminant_remainder();
                let (arms, default_arm) = self.xdr_union_enum_body();
                XdrUnionBody::Enum(XdrUnionEnumBody {
                    discriminant: Some(discriminant),
                    arms,
                    default_arm,
                })
            }
            TokenKind::Bool => {
                self.xdr_union_discriminant_remainder();
                let (true_arm, false_arm) = self.xdr_union_bool_body();
                XdrUnionBody::Bool(XdrUnionBoolBody {
                    true_arm,
                    false_arm,
                })
            }
            // XXX: remove the "Enum" case?
            TokenKind::Enum => {
                panic!("untested, probably unsupported");
            }
            _ => Parser::error(
                "Expected one of 'int', 'unsigned', 'enum', or an identifier to begin union",
                Some(&tok),
            ),
        };

        XdrUnion { name, body }
    }

    fn xdr_union_discriminant_remainder(&mut self) {
        let _ = self.expect_identifier("Expected identifier after union discriminant kind");
        self.expect(
            TokenKind::RightParen,
            "Expected '(' after union discriminant",
        );
    }

    fn xdr_union_bool_body(&mut self) -> (Declaration, Declaration) {
        self.expect(TokenKind::LeftBrace, "Expected '{' at start of union body");
        self.expect(TokenKind::Case, "Expected 'case' to begin a union case");
        // To simplify parsing, only accept bool unions where TRUE is the first case, until a
        // counterexample shows up:
        self.expect(
            TokenKind::True,
            "Expected first case to be 'TRUE' for a bool union",
        );
        self.expect(TokenKind::Colon, "Expected ':' after case in union");
        let true_arm = self.declaration();
        self.expect(TokenKind::Semicolon, "Expected ';' after union arm");

        let tok = self.next();
        match &tok.kind {
            TokenKind::Default => {}
            TokenKind::Case => self.expect(
                TokenKind::False,
                "Expected 'FALSE' for second bool union case",
            ),
            _ => Parser::error(
                "Expected 'FALSE' or 'default' for second bool union case",
                Some(tok),
            ),
        };
        self.expect(TokenKind::Colon, "Expected ':' after case in union");
        let false_arm = self.declaration();
        self.expect(TokenKind::Semicolon, "Expected ';' after union arm");
        self.expect(TokenKind::RightBrace, "Expected '}' at end of union body");

        (true_arm, false_arm)
    }

    fn xdr_union_enum_body(&mut self) -> (Vec<UnionArm>, DefaultUnionArm) {
        self.expect(TokenKind::LeftBrace, "Expected '{' at start of union body");
        let mut cases = Vec::new();
        let mut default = None;
        loop {
            match self.peek().kind {
                TokenKind::RightBrace => {
                    self.next();
                    break;
                }
                TokenKind::Default => {
                    self.next();
                    self.expect(TokenKind::Colon, "Expected ':' after default in union");
                    default = Some(self.declaration());
                    self.expect(
                        TokenKind::Semicolon,
                        "Expected ';' after union arm declaration",
                    );
                    // Default must be the last case:
                    self.expect(TokenKind::RightBrace, "Expected '}' after union body");
                    break;
                }
                _ => {}
            }
            let mut case_names = Vec::new();
            loop {
                let tok = self.peek();
                match tok.kind {
                    TokenKind::Case => {
                        self.next();
                        case_names.push(
                            self.expect_identifier("Expected identifier after 'case' in union"),
                        );
                        self.expect(TokenKind::Colon, "Expected ':' after identifier in union");
                    }
                    _ => break,
                }
            }
            if case_names.len() == 0 {
                Parser::error("union must have at least one case per arm", None);
            }
            let decl = self.declaration();
            for name in case_names.into_iter() {
                cases.push((Value::Name(name), decl.clone()));
            }
            self.expect(
                TokenKind::Semicolon,
                "Expected ';' after union arm declaration",
            );
        }

        if cases.len() == 0 {
            Parser::error("Enum must have at least one variant", None);
        }

        (cases, default)
    }

    fn array(&mut self, name: String, kind: ArrayKind) -> Declaration {
        let tok = self.next();
        let size = match &tok.kind {
            TokenKind::LeftBracket => {
                if kind == ArrayKind::Ascii {
                    Parser::error("Fixed length strings are prohibitied", None)
                } else {
                    let tok = self.next();
                    let val = match &tok.kind {
                        TokenKind::Number(n) => Value::Int(*n),
                        TokenKind::Identifier(name) => Value::Name(name.to_string()),
                        _ => Parser::error("Expected number of identifier after '['", Some(tok)),
                    };
                    self.expect(
                        TokenKind::RightBracket,
                        "Expected ']' after fixed length array",
                    );
                    ArraySize::Fixed(val)
                }
            }
            TokenKind::LessThan => {
                let tok = self.next();
                match &tok.kind {
                    TokenKind::Number(n) => {
                        let n = *n;
                        self.expect(
                            TokenKind::GreaterThan,
                            "Expected '>' after variable length array",
                        );
                        ArraySize::Limited(Value::Int(n))
                    }
                    TokenKind::Identifier(name) => {
                        let name = name.to_string();
                        self.expect(
                            TokenKind::GreaterThan,
                            "Expected '>' after variable length array",
                        );
                        ArraySize::Limited(Value::Name(name))
                    }
                    TokenKind::GreaterThan => ArraySize::Unlimited,
                    _ => Parser::error("Expected '>' after array definition", Some(tok)),
                }
            }
            _ => Parser::error("Expected '[' or '<' after array identifier", Some(tok)),
        };

        Declaration::Named(NamedDeclaration {
            name: name.to_string(),
            kind: DeclarationKind::Array(Array { kind, size }),
        })
    }

    fn xdr_type(&mut self) -> XdrType {
        let tok = self.next();
        match &tok.kind {
            TokenKind::Unsigned => {
                let tok = self.peek();
                match &tok.kind {
                    TokenKind::Int => {
                        self.next();
                        XdrType::UInt
                    }
                    TokenKind::Long => {
                        self.next();
                        XdrType::UInt
                    }
                    TokenKind::Hyper => {
                        self.next();
                        XdrType::UHyper
                    }
                    // The XDR spec doesn't permit 'unsigned' by itself, but in practice it seems to
                    // be used by itself as a synonym for 'unsigned int':
                    _ => XdrType::UInt,
                }
            }
            TokenKind::Int => XdrType::Int,
            TokenKind::Long => XdrType::Int,
            TokenKind::Hyper => XdrType::Hyper,
            TokenKind::Float => XdrType::Float,
            TokenKind::Double => XdrType::Double,
            TokenKind::Quadruple => XdrType::Quadruple,
            TokenKind::Bool => XdrType::Bool,
            TokenKind::Struct => {
                // Don't allow anonymous structs declared within outer structs, but do allow using
                // "struct identifier" as a long form of "identifier":
                let name = self.expect_identifier("Expected identifier after 'struct'");
                XdrType::Name(name.to_string())
            }
            TokenKind::Identifier(name) => XdrType::Name(name.to_string()),
            _ => Parser::error("Expected type specifier to begin declaration", Some(tok)),
        }
    }

    fn declaration(&mut self) -> Declaration {
        match self.peek().kind {
            TokenKind::Void => {
                self.next();
                return Declaration::Void;
            }
            TokenKind::Opaque => {
                self.next();
                let name = self.expect_identifier("Expected identifier after 'opaque'");
                return self.array(name, ArrayKind::Byte);
            }
            TokenKind::String => {
                self.schema_contains_string = true;
                self.next();
                let name = self.expect_identifier("Expected identifier after 'opaque'");
                return self.array(name, ArrayKind::Ascii);
            }
            _ => {}
        };

        let ty = self.xdr_type();

        let tok = self.next();
        match &tok.kind {
            TokenKind::Star => {
                let kind = DeclarationKind::Optional(ty);
                let name = self
                    .expect_identifier("Expected identifier after '*'")
                    .to_string();
                Declaration::Named(NamedDeclaration { name, kind })
            }
            TokenKind::Identifier(name) => {
                let name = name.to_string();
                match self.peek().kind {
                    TokenKind::LeftBracket => self.array(name, ArrayKind::UserType(ty)),
                    TokenKind::LessThan => self.array(name, ArrayKind::UserType(ty)),
                    _ => Declaration::Named(NamedDeclaration {
                        name: name,
                        kind: DeclarationKind::Scalar(ty),
                    }),
                }
            }
            _ => Parser::error("Expected '*' or identifier in declaration", Some(tok)),
        }
    }

    fn next(&mut self) -> &Token {
        self.current = std::mem::replace(&mut self.next, self.scanner.next());
        &self.current
    }

    fn peek(&mut self) -> &Token {
        &self.next
    }

    fn expect(&mut self, tok: TokenKind, msg: &str) {
        let actual = self.next();
        if actual.kind != tok {
            Parser::error(msg, Some(actual));
        }
    }

    fn expect_identifier(&mut self, msg: &str) -> String {
        let actual = self.next();
        let TokenKind::Identifier(ref s) = actual.kind else {
            Parser::error(msg, Some(actual));
        };

        s.to_string()
    }

    fn expect_number(&mut self, msg: &str) -> u64 {
        let actual = self.next();
        let TokenKind::Number(n) = actual.kind else {
            Parser::error(msg, Some(actual));
        };

        n
    }

    fn error(msg: &str, actual: Option<&Token>) -> ! {
        eprintln!("{msg}");
        if let Some(actual) = actual {
            eprintln!("Got: {:?}", actual.kind);
            eprintln!("on line: {}", actual.line);
        }
        // TODO: nicer error handling
        // std::process::exit(1);
        panic!("Parsing error");
    }
}
