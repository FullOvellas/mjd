use std::{error::Error, fmt::Display};

use mjl::{JsonLexer, LexError, Token};

#[derive(Debug)]
pub struct Json<'a> {
    pub value: Value<'a>,
}

#[derive(Debug)]
pub struct Pair<'a> {
    pub key: &'a str,
    pub value: Value<'a>,
}

#[derive(Debug)]
pub enum Value<'a> {
    Object(Vec<Pair<'a>>),
    Array(Vec<Value<'a>>),
    Str(&'a str),
    Number(&'a str),
    Boolean(BooleanVal),
    Null,
}

#[derive(Debug)]
pub enum BooleanVal {
    True,
    False,
}

impl Display for BooleanVal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BooleanVal::True => write!(f, "true"),
            BooleanVal::False => write!(f, "false"),
        }
    }
}

pub struct JsonParser<'a> {
    pub lexer: JsonLexer<'a>,
    pub tokens: Vec<Token<'a>>,
    pub position: usize,
}

impl<'a> JsonParser<'a> {
    fn parse_json(&mut self) -> Result<Json<'a>, Box<dyn Error>> {
        let value = self.parse_value()?;

        if self.current()?.is_some() {
            Err(Box::new(JsonParseError(
                "unexpected content following root value".to_string(),
            )))
        } else {
            Ok(Json { value })
        }
    }

    fn parse_value(&mut self) -> Result<Value<'a>, Box<dyn Error>> {
        use Token::*;
        use Value::*;
        if let Some(t) = self.current()? {
            let result = match t {
                LBrace => self.parse_object()?,
                String(s) => {
                    self.position += 1;
                    Str(s)
                }
                LBracket => self.parse_array()?,
                True => {
                    self.position += 1;
                    Boolean(BooleanVal::True)
                }
                False => {
                    self.position += 1;
                    Boolean(BooleanVal::False)
                }
                Token::Number(n) => {
                    self.position += 1;
                    Value::Number(n)
                }
                Token::Null => {
                    self.position += 1;
                    Value::Null
                }
                t => {
                    return Err(Box::new(JsonParseError(format!(
                        "expected a value, but got {t:?}"
                    ))));
                }
            };
            Ok(result)
        } else {
            Err(Box::new(JsonParseError(
                "expected value but input ended prematurely".to_string(),
            )))
        }
    }

    fn parse_array(&mut self) -> Result<Value<'a>, Box<dyn Error>> {
        use Token::*;
        use Value::*;
        self.position += 1; // skip over OpenSquareBracket
        let mut values = Vec::new();
        loop {
            match self.current()? {
                Some(RBracket) => {
                    self.position += 1; // done with current array, skip over CloseSquareBracket
                    return Ok(Array(values));
                }
                Some(_) => {
                    if !values.is_empty() {
                        self.expect_skip(&Comma)?;
                    }
                    values.push(self.parse_value()?);
                }
                None => {
                    return Err(Box::new(JsonParseError(
                        "unclosed array delimiter".to_string(),
                    )));
                }
            }
        }
    }

    fn parse_object(&mut self) -> Result<Value<'a>, Box<dyn Error>> {
        self.position += 1;
        let mut pairs = Vec::new();
        let mut seen_keys = std::collections::HashSet::new();

        loop {
            match self.current()? {
                Some(Token::RBrace) => {
                    self.position += 1;
                    return Ok(Value::Object(pairs));
                }
                Some(_) => {
                    if !pairs.is_empty() {
                        self.expect_skip(&Token::Comma)?;
                    }
                    let pair = self.parse_pair()?;
                    if !seen_keys.insert(pair.key) {
                        return Err(Box::new(JsonParseError(format!(
                            "duplicate key: {}",
                            pair.key
                        ))));
                    }
                    pairs.push(pair);
                }
                None => return Err(Box::new(JsonParseError("unclosed object".to_string()))),
            }
        }
    }

    fn expect_string(&mut self) -> Result<&'a str, Box<dyn Error>> {
        use Token::*;
        match self.current()? {
            Some(String(s)) => {
                self.position += 1;
                Ok(s)
            }
            Some(t) => Err(Box::new(JsonParseError(format!(
                "expected string, but got {:?}",
                t
            )))),
            None => Err(Box::new(JsonParseError(
                "expected string, but input ended prematurely".to_string(),
            ))),
        }
    }

    fn expect_skip(&mut self, expected: &Token) -> Result<(), Box<dyn Error>> {
        use std::mem::discriminant;
        if let Some(t) = self.current()? {
            if discriminant(&t) == discriminant(expected) {
                self.position += 1;
                Ok(())
            } else {
                Err(Box::new(JsonParseError(format!(
                    "expected {expected:?}, but got {t:?}"
                ))))
            }
        } else {
            Err(Box::new(JsonParseError(format!(
                "expected {expected:?}, but input ended prematurely"
            ))))
        }
    }

    fn current(&mut self) -> Result<Option<Token<'a>>, LexError> {
        let t = self.tokens.get(self.position);
        if let Some(t) = t {
            Ok(Some(t.clone()))
        } else {
            let t = self.lexer.next_token()?;
            if let Some(u) = &t {
                self.tokens.push(u.clone());
                Ok(t)
            } else {
                Ok(None)
            }
        }
    }

    fn parse_pair(&mut self) -> Result<Pair<'a>, Box<dyn Error>> {
        let key = self.expect_string()?;
        self.expect_skip(&Token::Colon)?;
        let value = self.parse_value()?;
        Ok(Pair { key, value })
    }
}

pub fn parse(lexer: JsonLexer) -> Result<Json, Box<dyn Error>> {
    let mut p = JsonParser {
        lexer,
        tokens: Vec::new(),
        position: 0,
    };

    p.parse_json()
}

#[derive(Debug)]
pub struct JsonParseError(String);

impl Display for JsonParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for JsonParseError {}
