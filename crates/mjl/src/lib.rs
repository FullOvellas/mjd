use lazy_static::lazy_static;
use regex::Regex;
use std::error::Error;
use std::fmt::Display;
use std::str::Chars;

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    True,
    False,
    Number(String),
    String(String),
    Null,
}

pub struct JsonLexer<'a> {
    pub input: &'a str,
    pub byte_offset: usize,
}

lazy_static! {
    static ref NULL_REGEX: Regex = Regex::new(r"^null\b").unwrap();
    static ref NUM_REGEX: Regex =
        Regex::new(r"^(-?(?:0|[1-9]\d*)(?:\.\d+)?(?:(?:e|E)[+-]?\d+)?)\b").unwrap();
    static ref TRUE_REGEX: Regex = Regex::new(r"^true\b").unwrap();
    static ref FALSE_REGEX: Regex = Regex::new(r"^false\b").unwrap();
}

impl<'a> JsonLexer<'a> {
    pub fn next_token(&mut self) -> Result<Option<Token>, LexError> {
        let mut chars = self.input[self.byte_offset..].chars();
        let mut c;
        loop {
            if let Some(ch) = chars.next() {
                c = ch;
            } else {
                return Ok(None);
            }

            if !c.is_whitespace() {
                break;
            }
            self.byte_offset += 1;
        }

        match c {
            '{' => {
                self.byte_offset += 1;
                Ok(Some(Token::LBrace))
            }
            '}' => {
                self.byte_offset += 1;
                Ok(Some(Token::RBrace))
            }
            '[' => {
                self.byte_offset += 1;
                Ok(Some(Token::LBracket))
            }
            ']' => {
                self.byte_offset += 1;
                Ok(Some(Token::RBracket))
            }
            ',' => {
                self.byte_offset += 1;
                Ok(Some(Token::Comma))
            }
            ':' => {
                self.byte_offset += 1;
                Ok(Some(Token::Colon))
            }
            't' => self.lex_match(4, |s| {
                if TRUE_REGEX.is_match(s) {
                    Some(Token::True)
                } else {
                    None
                }
            }),
            'f' => self.lex_match(5, |s| {
                if FALSE_REGEX.is_match(s) {
                    Some(Token::False)
                } else {
                    None
                }
            }),
            'n' => self.lex_match(4, |s| {
                if NULL_REGEX.is_match(s) {
                    Some(Token::Null)
                } else {
                    None
                }
            }),
            '"' => self.lex_string(chars),
            n @ '-' | n if n.is_ascii_digit() => {
                if let Some(m) = NUM_REGEX.captures(&self.input[self.byte_offset..]) {
                    m.get(1)
                        .map(|n| {
                            self.byte_offset += n.len();
                            Some(Token::Number(n.as_str().to_string()))
                        })
                        .ok_or(LexError("no number match found".to_string()))
                } else {
                    Err(LexError("unexpected character found".to_string()))
                }
            }
            c => Err(LexError(format!("unable to parse token from char {c}"))),
        }
    }

    fn lex_match<T: FnOnce(&str) -> Option<Token>>(
        &mut self,
        len: usize,
        factory: T,
    ) -> Result<Option<Token>, LexError> {
        let end = (self.byte_offset + len).min(self.input.len());
        let slice = &self.input[self.byte_offset..end];
        match factory(slice) {
            Some(token) => {
                self.byte_offset += len;
                Ok(Some(token))
            }
            None => Err(LexError("unexpected token".to_string())),
        }
    }

    fn lex_string(&mut self, mut chars: Chars<'_>) -> Result<Option<Token>, LexError> {
        let mut s = String::new();
        while let Some(c) = chars.next() {
            if c == '"' {
                self.byte_offset += s.len() + 2;
                let result = Ok(Some(Token::String(s)));
                return result;
            }

            if c.is_control() {
                return Err(LexError("invalid control char in string".to_string()));
            }

            if c == '\\' {
                match chars.next() {
                    None => break,
                    Some(e) => match e {
                        '"' | '\\' | '/' | 'f' | 'n' | 'r' | 't' => {
                            s.push(c);
                            s.push(e);
                            continue;
                        }
                        'u' => {
                            s.push(c);
                            s.push(e);
                            for _ in 0..4 {
                                if let Some(h) = chars.next() {
                                    if !h.is_ascii_hexdigit() {
                                        return Err(LexError(
                                            "invalid unicode escape sequence".to_string(),
                                        ));
                                    }
                                    s.push(h);
                                } else {
                                    break;
                                }
                            }
                            continue;
                        }
                        _ => return Err(LexError("invalid escape sequence".to_string())),
                    },
                }
            }
            s.push(c);
        }

        Err(LexError("unclosed string literal".to_string()))
    }
}

#[derive(Debug, PartialEq)]
pub struct LexError(String);

impl Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for LexError {}

#[cfg(test)]
mod test {
    use crate::{JsonLexer, Token};

    #[test]
    fn lex_token_sequence() {
        let mut lexer = JsonLexer {
            input: "{ \"asdf\": 1, \"🗻∈🌏\": true, \"🗻\": 42 }",
            byte_offset: 0,
        };

        assert_eq!(Ok(Some(Token::LBrace)), lexer.next_token());
        assert_eq!(
            Ok(Some(Token::String("asdf".to_string()))),
            lexer.next_token()
        );
        assert_eq!(Ok(Some(Token::Colon)), lexer.next_token());
        assert_eq!(Ok(Some(Token::Number("1".to_string()))), lexer.next_token());
        assert_eq!(Ok(Some(Token::Comma)), lexer.next_token());
        assert_eq!(
            Ok(Some(Token::String("🗻∈🌏".to_string()))),
            lexer.next_token()
        );
        assert_eq!(Ok(Some(Token::Colon)), lexer.next_token());
        assert_eq!(Ok(Some(Token::True)), lexer.next_token());
        assert_eq!(Ok(Some(Token::Comma)), lexer.next_token());
        assert_eq!(
            Ok(Some(Token::String("🗻".to_string()))),
            lexer.next_token()
        );
        assert_eq!(Ok(Some(Token::Colon)), lexer.next_token());
        assert_eq!(
            Ok(Some(Token::Number("42".to_string()))),
            lexer.next_token()
        );
        assert_eq!(Ok(Some(Token::RBrace)), lexer.next_token());
    }

    #[test]
    fn lex_single_tokens() {
        assert_eq!(
            Ok(Some(Token::True)),
            JsonLexer {
                input: "true",
                byte_offset: 0
            }
            .next_token()
        );
        assert_eq!(
            Ok(Some(Token::False)),
            JsonLexer {
                input: "false",
                byte_offset: 0
            }
            .next_token()
        );
        assert_eq!(
            Ok(Some(Token::Null)),
            JsonLexer {
                input: "null",
                byte_offset: 0
            }
            .next_token()
        );
        assert_eq!(
            Ok(Some(Token::Comma)),
            JsonLexer {
                input: ",",
                byte_offset: 0
            }
            .next_token()
        );
        assert_eq!(
            Ok(Some(Token::Colon)),
            JsonLexer {
                input: ":",
                byte_offset: 0
            }
            .next_token()
        );
        assert_eq!(
            Ok(Some(Token::LBrace)),
            JsonLexer {
                input: "{",
                byte_offset: 0
            }
            .next_token()
        );
        assert_eq!(
            Ok(Some(Token::RBrace)),
            JsonLexer {
                input: "}",
                byte_offset: 0
            }
            .next_token()
        );
        assert_eq!(
            Ok(Some(Token::LBracket)),
            JsonLexer {
                input: "[",
                byte_offset: 0
            }
            .next_token()
        );
        assert_eq!(
            Ok(Some(Token::RBracket)),
            JsonLexer {
                input: "]",
                byte_offset: 0
            }
            .next_token()
        );
        assert_eq!(
            Ok(Some(Token::String("asdf".to_string()))),
            JsonLexer {
                input: "\"asdf\"",
                byte_offset: 0
            }
            .next_token()
        );
        assert_eq!(
            Ok(Some(Token::String(r#"as\"df"#.to_string()))),
            JsonLexer {
                input: r#""as\"df""#,
                byte_offset: 0
            }
            .next_token()
        );
        assert_eq!(
            Ok(Some(Token::String(r#"as\uFFFFdf"#.to_string()))),
            JsonLexer {
                input: r#""as\uFFFFdf""#,
                byte_offset: 0
            }
            .next_token()
        );
        assert_eq!(
            Ok(Some(Token::Number("1".to_string()))),
            JsonLexer {
                input: "1",
                byte_offset: 0
            }
            .next_token()
        );
        assert_eq!(
            Ok(Some(Token::Number("1.2".to_string()))),
            JsonLexer {
                input: "1.2",
                byte_offset: 0
            }
            .next_token()
        );
        assert_eq!(
            Ok(Some(Token::Number("1.2E2".to_string()))),
            JsonLexer {
                input: "1.2E2",
                byte_offset: 0
            }
            .next_token()
        );
        assert_eq!(
            Ok(Some(Token::Number("1.2E-2".to_string()))),
            JsonLexer {
                input: "1.2E-2",
                byte_offset: 0
            }
            .next_token()
        );
        assert_eq!(
            Ok(Some(Token::Number("1.2E+2".to_string()))),
            JsonLexer {
                input: "1.2E+2",
                byte_offset: 0
            }
            .next_token()
        );
        assert_eq!(
            Ok(Some(Token::Number("1.2e2".to_string()))),
            JsonLexer {
                input: "1.2e2",
                byte_offset: 0
            }
            .next_token()
        );
        assert_eq!(
            Ok(Some(Token::Number("1.2e-2".to_string()))),
            JsonLexer {
                input: "1.2e-2",
                byte_offset: 0
            }
            .next_token()
        );
        assert_eq!(
            Ok(Some(Token::Number("1.2e+2".to_string()))),
            JsonLexer {
                input: "1.2e+2",
                byte_offset: 0
            }
            .next_token()
        );
    }
}
