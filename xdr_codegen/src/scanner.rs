// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

#[derive(Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
}

#[derive(Debug, PartialEq)]
pub enum TokenKind {
    Struct,
    Union,
    Switch,
    Case,
    Default,
    Typedef,
    Enum,
    Const,

    // RPC:
    Program,
    Version,

    Float,
    Double,
    Quadruple,
    Bool,
    True,
    False,
    Unsigned,
    Int,
    // Based on the appearance of the following in the NFS v3 spec (RFC 1318):
    //      > typedef unsigned long uint32;
    // I assume long is a 32-bit int.
    Long,
    Hyper,
    Opaque,
    String,
    Void,

    Identifier(String),
    // XXX: Not allowing negative constants...
    Number(u64),

    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    LeftParen,
    RightParen,
    LessThan,
    GreaterThan,
    Colon,
    Semicolon,
    Star,
    Equal,
    Comma,

    Eof,
}

pub struct Scanner<'src> {
    source: &'src str,
    chars: std::iter::Peekable<std::str::CharIndices<'src>>,
    start: usize,
    current: usize,
    line: usize,
}

impl<'src> Scanner<'src> {
    pub fn new(source: &str) -> Scanner<'_> {
        Scanner {
            source,
            chars: source.char_indices().peekable(),
            start: 0,
            current: 0,
            line: 1,
        }
    }

    pub fn next(&mut self) -> Token {
        self.skip_whitespace_and_comments();

        let kind = match self.chars.next() {
            Some((i, ch)) => match ch {
                '{' => TokenKind::LeftBrace,
                '}' => TokenKind::RightBrace,
                '[' => TokenKind::LeftBracket,
                ']' => TokenKind::RightBracket,
                '(' => TokenKind::LeftParen,
                ')' => TokenKind::RightParen,
                '<' => TokenKind::LessThan,
                '>' => TokenKind::GreaterThan,
                ';' => TokenKind::Semicolon,
                ':' => TokenKind::Colon,
                '*' => TokenKind::Star,
                '=' => TokenKind::Equal,
                ',' => TokenKind::Comma,
                '-' => {
                    unimplemented!("Negative numbers not currently supported")
                }
                // Octal or Hex number:
                '0' => match self.chars.peek() {
                    Some((i, 'x')) => {
                        let i = *i;
                        self.chars.next();
                        self.chars.next();
                        self.start = i + 1;
                        self.number(16)
                    }
                    Some((i, ch)) if ch.is_numeric() => {
                        let i = *i;
                        self.chars.next();
                        self.start = i;
                        self.number(8)
                    }
                    _ => TokenKind::Number(0),
                },
                // Positive decimal number:
                ch if ch.is_numeric() => {
                    self.start = i;
                    let num = self.number(10);
                    num
                }
                ch if ch.is_alphabetic() => {
                    self.start = i;
                    self.keyword_or_identifier()
                }
                ch => todo!("Unhandled character: {ch}"),
            },
            None => TokenKind::Eof,
        };

        Token {
            kind,
            line: self.line,
        }
    }

    fn keyword_or_identifier(&mut self) -> TokenKind {
        self.current = self.start;
        loop {
            match self.chars.peek() {
                Some((_, ch)) => {
                    match ch {
                        '_' => {}
                        ch if ch.is_alphanumeric() => {}
                        _ => break,
                    };
                    self.current += 1;
                    self.chars.next();
                }
                _ => break,
            }
        }
        self.current += 1;
        let id = &self.source[self.start..self.current];
        match id {
            "EOF" => TokenKind::Eof,
            "struct" => TokenKind::Struct,
            "union" => TokenKind::Union,
            "switch" => TokenKind::Switch,
            "case" => TokenKind::Case,
            "default" => TokenKind::Default,
            "typedef" => TokenKind::Typedef,
            "enum" => TokenKind::Enum,
            "const" => TokenKind::Const,
            "float" => TokenKind::Float,
            "double" => TokenKind::Double,
            "quadruple" => TokenKind::Quadruple,
            "bool" => TokenKind::Bool,
            "TRUE" => TokenKind::True,
            "FALSE" => TokenKind::False,
            "unsigned" => TokenKind::Unsigned,
            "int" => TokenKind::Int,
            "long" => TokenKind::Long,
            "hyper" => TokenKind::Hyper,
            "opaque" => TokenKind::Opaque,
            "string" => TokenKind::String,
            "void" => TokenKind::Void,
            "program" => TokenKind::Program,
            "version" => TokenKind::Version,
            _ => TokenKind::Identifier(Scanner::maybe_escape(id)),
        }
    }

    /// If the input is a keyword in rust, like `type`, or `where`, then escape it.
    fn maybe_escape(s: &str) -> String {
        match s {
            "where" => "r#where".to_string(),
            "type" => "r#type".to_string(),
            _ => s.to_string(),
        }
    }

    fn number(&mut self, radix: u32) -> TokenKind {
        self.current = self.start;
        loop {
            match self.chars.peek() {
                Some((_, ch)) if ch.is_alphanumeric() => {
                    self.current += 1;
                    self.chars.next();
                }
                _ => {
                    self.current += 1;
                    break;
                }
            }
        }
        let num = &self.source[self.start..self.current];
        let num: u64 = u64::from_str_radix(num, radix)
            .expect(&format!("Should be able to parse {num} as a number"));

        TokenKind::Number(num)
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.chars.peek() {
                Some((_, '\n')) => {
                    self.line += 1;
                    self.chars.next();
                }
                Some((_, ch)) if ch.is_whitespace() => {
                    self.chars.next();
                }
                Some((_, '/')) => {
                    self.chars.next();
                    self.multiline_comment();
                }
                Some((_, '%')) => {
                    self.chars.next();
                    self.singleline_comment();
                }
                _ => break,
            };
        }
    }

    /// While I don't see single line comments explained in the XDR spec anywhere, the NFS v4.1 and
    /// 4.2 specs appear to treat lines starting with '%' as comments.
    fn singleline_comment(&mut self) {
        loop {
            if let Some((_, '\n')) = self.chars.next() {
                break;
            }
        }
        self.line += 1;
    }

    /// Multiline comments: /* ... */
    fn multiline_comment(&mut self) {
        self.expect('*', "Expected '*' after '/'");
        loop {
            match self.chars.next() {
                Some((_, '\n')) => self.line += 1,
                Some((_, '*')) => match self.chars.peek() {
                    Some((_, '/')) => {
                        self.chars.next();
                        return;
                    }
                    _ => continue,
                },
                _ => continue,
            }
        }
    }

    fn expect(&mut self, ch: char, msg: &str) {
        match self.chars.next() {
            Some((_, next)) if next == ch => {}
            _ => {
                eprintln!("{msg}");
                // TODO: nicer error handling
                std::process::exit(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn characters() {
        let mut scanner = Scanner::new(" { }[]<>*= ;:, ()");
        assert_eq!(scanner.next().kind, TokenKind::LeftBrace);
        assert_eq!(scanner.next().kind, TokenKind::RightBrace);
        assert_eq!(scanner.next().kind, TokenKind::LeftBracket);
        assert_eq!(scanner.next().kind, TokenKind::RightBracket);
        assert_eq!(scanner.next().kind, TokenKind::LessThan);
        assert_eq!(scanner.next().kind, TokenKind::GreaterThan);
        assert_eq!(scanner.next().kind, TokenKind::Star);
        assert_eq!(scanner.next().kind, TokenKind::Equal);
        assert_eq!(scanner.next().kind, TokenKind::Semicolon);
        assert_eq!(scanner.next().kind, TokenKind::Colon);
        assert_eq!(scanner.next().kind, TokenKind::Comma);
        assert_eq!(scanner.next().kind, TokenKind::LeftParen);
        assert_eq!(scanner.next().kind, TokenKind::RightParen);
        assert_eq!(scanner.next().kind, TokenKind::Eof);
    }

    #[test]
    fn comments() {
        let mut scanner = Scanner::new("/* */ { /* } */ = /* * * / */ *");
        assert_eq!(scanner.next().kind, TokenKind::LeftBrace);
        assert_eq!(scanner.next().kind, TokenKind::Equal);
        assert_eq!(scanner.next().kind, TokenKind::Star);
        assert_eq!(scanner.next().kind, TokenKind::Eof);
    }

    #[test]
    fn numbers() {
        let mut scanner = Scanner::new(
            "123 456 7{8}9
            0xa 0xA 0x01 0x1 0x20 01 010 0,1",
        );
        assert_eq!(scanner.next().kind, TokenKind::Number(123));
        assert_eq!(scanner.next().kind, TokenKind::Number(456));
        assert_eq!(scanner.next().kind, TokenKind::Number(7));
        assert_eq!(scanner.next().kind, TokenKind::LeftBrace);
        assert_eq!(scanner.next().kind, TokenKind::Number(8));
        assert_eq!(scanner.next().kind, TokenKind::RightBrace);
        assert_eq!(scanner.next().kind, TokenKind::Number(9));
        assert_eq!(scanner.next().kind, TokenKind::Number(10));
        assert_eq!(scanner.next().kind, TokenKind::Number(10));
        assert_eq!(scanner.next().kind, TokenKind::Number(1));
        assert_eq!(scanner.next().kind, TokenKind::Number(1));
        assert_eq!(scanner.next().kind, TokenKind::Number(32));
        assert_eq!(scanner.next().kind, TokenKind::Number(1));
        assert_eq!(scanner.next().kind, TokenKind::Number(8));
        assert_eq!(scanner.next().kind, TokenKind::Number(0));
        assert_eq!(scanner.next().kind, TokenKind::Comma);
        assert_eq!(scanner.next().kind, TokenKind::Number(1));
        assert_eq!(scanner.next().kind, TokenKind::Eof);
    }

    #[test]
    fn keywords() {
        let mut scanner = Scanner::new(
            "struct union an_identifier123 switch case default typedef enum program version
            const const_ float double quadruple bool TRUE FALSE 
            unsigned int long hyper opaque string void ",
        );
        assert_eq!(scanner.next().kind, TokenKind::Struct);
        assert_eq!(scanner.next().kind, TokenKind::Union);
        assert_eq!(
            scanner.next().kind,
            TokenKind::Identifier("an_identifier123".to_string())
        );
        assert_eq!(scanner.next().kind, TokenKind::Switch);
        assert_eq!(scanner.next().kind, TokenKind::Case);
        assert_eq!(scanner.next().kind, TokenKind::Default);
        assert_eq!(scanner.next().kind, TokenKind::Typedef);
        assert_eq!(scanner.next().kind, TokenKind::Enum);
        assert_eq!(scanner.next().kind, TokenKind::Program);
        assert_eq!(scanner.next().kind, TokenKind::Version);
        assert_eq!(scanner.next().kind, TokenKind::Const);
        assert_eq!(
            scanner.next().kind,
            TokenKind::Identifier("const_".to_string())
        );
        assert_eq!(scanner.next().kind, TokenKind::Float);
        assert_eq!(scanner.next().kind, TokenKind::Double);
        assert_eq!(scanner.next().kind, TokenKind::Quadruple);
        assert_eq!(scanner.next().kind, TokenKind::Bool);
        assert_eq!(scanner.next().kind, TokenKind::True);
        assert_eq!(scanner.next().kind, TokenKind::False);
        assert_eq!(scanner.next().kind, TokenKind::Unsigned);
        assert_eq!(scanner.next().kind, TokenKind::Int);
        assert_eq!(scanner.next().kind, TokenKind::Long);
        assert_eq!(scanner.next().kind, TokenKind::Hyper);
        assert_eq!(scanner.next().kind, TokenKind::Opaque);
        assert_eq!(scanner.next().kind, TokenKind::String);
        assert_eq!(scanner.next().kind, TokenKind::Void);
        assert_eq!(scanner.next().kind, TokenKind::Eof);
    }
}
