#![allow(non_camel_case_types)]

#[derive(Debug, Clone, PartialEq)]
pub enum TokenCode<'a> {
    ID(&'a str),

    BREAK,
    CHAR,
    DOUBLE,
    ELSE,
    FOR,
    IF,
    INT,
    RETURN,
    STRUCT,
    VOID,
    WHILE,

    CT_INT(i64),
    CT_REAL(f64),
    CT_CHAR(char),
    CT_STRING(&'a str),

    COMMA,
    SEMICOLON,
    LPAR,
    RPAR,
    LBRACKET,
    RBRACKET,
    LACC,
    RACC,

    ADD,
    SUB,
    MUL,
    DIV,
    DOT,
    AND,
    OR,
    NOT,
    ASSIGN,
    EQUAL,
    NOTEQ,
    LESS,
    LESSEQ,
    GREATER,
    GREATEREQ,

    END,
}

#[derive(Debug)]
pub struct Token<'a> {
    pub code: TokenCode<'a>,
    pub line: usize,
    pub pos: usize,
}

#[derive(Debug, Clone, Copy)]
enum State {
    START,

    ID,
    ID_END,

    INT,
    ZERO,
    OCTAL_INT,
    HEX,
    HEX_INT,

    REAL_DOT,
    REAL_FRAC,
    REAL_EXP,
    REAL_EXP_SIGN,
    REAL_EXP_NUM,

    STRING,
    CHAR,
    CHAR_END,

    SLASH,
    LINE_COMMENT,

    AND1,
    OR1,
    ASSIGN1,
    NOT1,
    LESS1,
    GREATER1,

    END,
}

pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: usize,
    tokens: Vec<Token<'a>>,
    state: State,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src: src.as_bytes(),
            line: 1,
            pos: 0,
            tokens: Vec::new(),
            state: State::START,
        }
    }

    pub fn tokenize(&mut self) {
        loop {
            let token = self.next_token();
            let end = matches!(token.code, TokenCode::END);
            self.tokens.push(token);
            if end {
                break;
            }
        }
    }

    pub fn get_tokens(&mut self) -> &Vec<Token<'a>> {
        if self.tokens.is_empty() {
            self.tokenize();
        }
        &self.tokens
    }

    fn advance(&mut self) -> Option<u8> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }
    fn slice(&self, start: usize, end: usize) -> &'a str {
        std::str::from_utf8(&self.src[start..end]).unwrap()
    }

    fn panic(&self, msg: &str) -> ! {
        panic!("[{} at line {} pos {}]", msg, self.line, self.pos + 1)
    }

    fn keyword_or_id(&self, s: &'a str) -> TokenCode<'a> {
        match s {
            "break" => TokenCode::BREAK,
            "char" => TokenCode::CHAR,
            "double" => TokenCode::DOUBLE,
            "else" => TokenCode::ELSE,
            "for" => TokenCode::FOR,
            "if" => TokenCode::IF,
            "int" => TokenCode::INT,
            "return" => TokenCode::RETURN,
            "struct" => TokenCode::STRUCT,
            "void" => TokenCode::VOID,
            "while" => TokenCode::WHILE,
            _ => TokenCode::ID(s),
        }
    }

    fn next_token(&mut self) -> Token<'a> {
        let mut start = self.pos;

        loop {
            let c = self.peek();

            match self.state {
                State::START => {
                    start = self.pos;

                    match c {
                        None | Some(b'\0') => {
                            self.state = State::END;
                            return Token {
                                code: TokenCode::END,
                                line: self.line,
                                pos: self.pos,
                            };
                        }

                        Some(b' ' | b'\t' | b'\r') => {
                            self.advance();
                        }

                        Some(b'\n') => {
                            self.advance();
                            self.line += 1;
                        }

                        Some(c) if c.is_ascii_alphabetic() || c == b'_' => {
                            self.state = State::ID;
                            self.advance();
                        }

                        Some(c) if c.is_ascii_digit() => {
                            if c == b'0' {
                                self.state = State::ZERO;
                            } else {
                                self.state = State::INT;
                            }
                            self.advance();
                        }

                        Some(b'"') => {
                            self.state = State::STRING;
                            self.advance();
                        }

                        Some(b'\'') => {
                            self.state = State::CHAR;
                            self.advance();
                        }

                        Some(b'/') => {
                            self.state = State::SLASH;
                            self.advance();
                        }

                        Some(b'&') => {
                            self.state = State::AND1;
                            self.advance();
                        }
                        Some(b'|') => {
                            self.state = State::OR1;
                            self.advance();
                        }
                        Some(b'=') => {
                            self.state = State::ASSIGN1;
                            self.advance();
                        }
                        Some(b'!') => {
                            self.state = State::NOT1;
                            self.advance();
                        }
                        Some(b'<') => {
                            self.state = State::LESS1;
                            self.advance();
                        }
                        Some(b'>') => {
                            self.state = State::GREATER1;
                            self.advance();
                        }

                        Some(b'+') => {
                            self.advance();
                            return self.tk(TokenCode::ADD);
                        }
                        Some(b'-') => {
                            self.advance();
                            return self.tk(TokenCode::SUB);
                        }
                        Some(b'*') => {
                            self.advance();
                            return self.tk(TokenCode::MUL);
                        }
                        Some(b',') => {
                            self.advance();
                            return self.tk(TokenCode::COMMA);
                        }
                        Some(b';') => {
                            self.advance();
                            return self.tk(TokenCode::SEMICOLON);
                        }
                        Some(b'(') => {
                            self.advance();
                            return self.tk(TokenCode::LPAR);
                        }
                        Some(b')') => {
                            self.advance();
                            return self.tk(TokenCode::RPAR);
                        }
                        Some(b'[') => {
                            self.advance();
                            return self.tk(TokenCode::LBRACKET);
                        }
                        Some(b']') => {
                            self.advance();
                            return self.tk(TokenCode::RBRACKET);
                        }
                        Some(b'{') => {
                            self.advance();
                            return self.tk(TokenCode::LACC);
                        }
                        Some(b'}') => {
                            self.advance();
                            return self.tk(TokenCode::RACC);
                        }
                        Some(b'.') => {
                            self.advance();
                            return self.tk(TokenCode::DOT);
                        }

                        _ => self.panic("invalid character"),
                    }
                }

                State::ID => match c {
                    Some(c) if c.is_ascii_alphanumeric() || c == b'_' => {
                        self.advance();
                    }
                    _ => self.state = State::ID_END,
                },

                State::ID_END => {
                    let text = self.slice(start, self.pos);
                    self.state = State::START;
                    return self.tk(self.keyword_or_id(text));
                }

                State::INT => match c {
                    Some(c) if c.is_ascii_digit() => {
                        self.advance();
                    }
                    Some(b'.') => {
                        self.state = State::REAL_DOT;
                        self.advance();
                    }
                    Some(b'e') | Some(b'E') => {
                        self.state = State::REAL_EXP;
                        self.advance();
                    }
                    _ => {
                        let text = self.slice(start, self.pos);
                        self.state = State::START;
                        return self.tk(TokenCode::CT_INT(text.parse().unwrap()));
                    }
                },

                State::ZERO => match c {
                    Some(b'x') => {
                        self.state = State::HEX;
                        self.advance();
                    }
                    Some(b'.') => {
                        self.state = State::REAL_DOT;
                        self.advance();
                    }
                    Some(b'0'..=b'9') => {
                        self.state = State::OCTAL_INT;
                        self.advance();
                    }
                    _ => {
                        self.state = State::START;
                        return self.tk(TokenCode::CT_INT(0));
                    }
                },

                State::OCTAL_INT => {
                    match c {
                        Some(b'0'..=b'7') => self.advance(),
                        Some(b'8' | b'9') => self.panic("invalid octal digit"),
                        _ => {
                            let text = self.slice(start, self.pos);
                            self.state = State::START;
                            return self.tk(TokenCode::CT_INT(
                                i64::from_str_radix(text.strip_prefix("0").unwrap(), 8).unwrap(),
                            ));
                        }
                    };
                }

                State::HEX => match c {
                    Some(c) if c.is_ascii_hexdigit() => {
                        self.state = State::HEX_INT;
                        self.advance();
                    }
                    _ => self.panic("expected hex digit after 0x"),
                },

                State::HEX_INT => {
                    match c {
                        Some(c) if c.is_ascii_hexdigit() => self.advance(),
                        _ => {
                            let text = self.slice(start, self.pos);
                            self.state = State::START;
                            return self.tk(TokenCode::CT_INT(i64::from_str_radix(text.strip_prefix("0x").unwrap(), 16).unwrap(),));
                        }
                    };
                }

                State::REAL_DOT => match c {
                    Some(c) if c.is_ascii_digit() => {
                        self.state = State::REAL_FRAC;
                        self.advance();
                    }
                    _ => self.panic("expected digit after '.'"),
                },

                State::REAL_FRAC => {
                    match c {
                        Some(c) if c.is_ascii_digit() => self.advance(),
                        Some(b'e') | Some(b'E') => {
                            self.state = State::REAL_EXP;
                            self.advance()
                        }
                        _ => {
                            let text = self.slice(start, self.pos);
                            self.state = State::START;
                            return self.tk(TokenCode::CT_REAL(text.parse().unwrap()));
                        }
                    };
                }

                State::REAL_EXP => match c {
                    Some(b'+' | b'-') => {
                        self.state = State::REAL_EXP_SIGN;
                        self.advance();
                    }
                    Some(c) if c.is_ascii_digit() => {
                        self.state = State::REAL_EXP_NUM;
                        self.advance();
                    }
                    _ => self.panic("invalid exponent"),
                },

                State::REAL_EXP_SIGN => match c {
                    Some(c) if c.is_ascii_digit() => {
                        self.state = State::REAL_EXP_NUM;
                        self.advance();
                    }
                    _ => self.panic("expected digit after exponent sign"),
                },

                State::REAL_EXP_NUM => {
                    match c {
                        Some(c) if c.is_ascii_digit() => self.advance(),
                        _ => {
                            let text = self.slice(start, self.pos);
                            self.state = State::START;
                            return self.tk(TokenCode::CT_REAL(text.parse().unwrap()));
                        }
                    };
                }

                State::STRING => match self.advance() {
                    Some(b'"') => {
                        let text = self.slice(start, self.pos);
                        self.state = State::START;
                        return self.tk(TokenCode::CT_STRING(&text[1..text.len() - 1]));
                    }
                    Some(b'\n') | None => {
                        self.panic("expected '\"' maybe you forgot to close a string ?")
                    }
                    _ => {}
                },

                State::CHAR => match self.advance() {
                    Some(b'\'') | Some(b'\n') | None => self.panic("invalid char literal"),
                    _ => self.state = State::CHAR_END,
                },

                State::CHAR_END => match self.advance() {
                    Some(b'\'') => {
                        let text = self.slice(start, self.pos);
                        self.state = State::START;
                        return self.tk(TokenCode::CT_CHAR(char::from(text.as_bytes()[1])));
                    }
                    _ => self.panic("expected closing ' for char"),
                },

                State::SLASH => match c {
                    Some(b'/') => {
                        self.state = State::LINE_COMMENT;
                        self.advance();
                    }
                    _ => {
                        self.state = State::START;
                        return self.tk(TokenCode::DIV);
                    }
                },

                State::LINE_COMMENT => match self.advance() {
                    Some(b'\n') | None => {
                        self.state = State::START;
                        self.line += 1;
                    }
                    _ => {}
                },

                State::AND1 => {
                    if self.peek() == Some(b'&') {
                        self.advance();
                        self.state = State::START;
                        return self.tk(TokenCode::AND);
                    } else {
                        self.panic("expected '&'");
                    }
                }

                State::OR1 => {
                    if self.peek() == Some(b'|') {
                        self.advance();
                        self.state = State::START;
                        return self.tk(TokenCode::OR);
                    } else {
                        self.panic("expected '|'");
                    }
                }

                State::ASSIGN1 => {
                    if self.peek() == Some(b'=') {
                        self.advance();
                        self.state = State::START;
                        return self.tk(TokenCode::EQUAL);
                    } else {
                        self.state = State::START;
                        return self.tk(TokenCode::ASSIGN);
                    }
                }

                State::NOT1 => {
                    if self.peek() == Some(b'=') {
                        self.advance();
                        self.state = State::START;
                        return self.tk(TokenCode::NOTEQ);
                    } else {
                        self.state = State::START;
                        return self.tk(TokenCode::NOT);
                    }
                }

                State::LESS1 => {
                    if self.peek() == Some(b'=') {
                        self.advance();
                        self.state = State::START;
                        return self.tk(TokenCode::LESSEQ);
                    } else {
                        self.state = State::START;
                        return self.tk(TokenCode::LESS);
                    }
                }

                State::GREATER1 => {
                    if self.peek() == Some(b'=') {
                        self.advance();
                        self.state = State::START;
                        return self.tk(TokenCode::GREATEREQ);
                    } else {
                        self.state = State::START;
                        return self.tk(TokenCode::GREATER);
                    }
                }

                State::END => {
                    return self.tk(TokenCode::END);
                }
            }
        }
    }

    fn tk(&self, code: TokenCode<'a>) -> Token<'a> {
        Token {
            code,
            line: self.line,
            pos: self.pos,
        }
    }
}
