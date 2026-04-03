#![allow(non_camel_case_types)]

#[derive(Debug)]
pub enum TokenCode<'a> {
    START_CODE,
    END_CODE,
    WORD(&'a str),
}

#[derive(Debug)]
pub struct Token<'a> {
    pub code: TokenCode<'a>,
    pub line: usize,
}

#[derive(Debug)]
enum State {
    START_CODE,
    START_TOKEN,
    IN_TOKEN,
    END,
}

#[derive(Debug)]
pub struct Lexer<'a> {
    pub src: &'a [u8],
    pub line: usize,
    pub pos: usize,
    tokens: Vec<Token<'a>>,
    state: State,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Lexer<'a> {
        Lexer {
            src: src.as_bytes(),
            line: 0,
            pos: 0,
            tokens: Vec::new(),
            state: State::START_CODE,
        }
    }

    pub fn tokenize(&mut self) {
        loop {
            let token = self.next_token();
            let end = matches!(token.code, TokenCode::END_CODE);
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

    fn next_char(&mut self) -> Option<u8> {
        if self.pos >= self.src.len() {
            return None;
        }
        let ch = self.src[self.pos];
        self.pos += 1;
        Some(ch)
    }

    fn peek_char(&self) -> Option<u8> {
        if self.pos >= self.src.len() {
            return None;
        }
        Some(self.src[self.pos])
    }

    fn get_slice(&self, interval: (usize, usize)) -> &'a str {
        std::str::from_utf8(&self.src[interval.0..interval.1]).unwrap()
    }

    fn next_token(&mut self) -> Token<'a> {
        let mut interval = (self.pos, self.pos);

        loop {
            match self.state {
                State::START_CODE => {
                    self.state = State::START_TOKEN;
                    return Token {
                        code: TokenCode::START_CODE,
                        line: 0,
                    };
                }

                State::START_TOKEN => {
                    let c = self.next_char();

                    match c {
                        None => {
                            self.state = State::END;
                        }

                        Some(c) if c.is_ascii_whitespace() => {
                            if c == b'\n' {
                                self.line += 1;
                            }
                            continue;
                        }

                        Some(_) => {
                            interval.0 = self.pos - 1;
                            interval.1 = self.pos;
                            self.state = State::IN_TOKEN;
                        }
                    }
                }

                State::IN_TOKEN => {
                    let c = self.next_char();

                    match c {
                        None => {
                            self.state = State::END;
                            return Token {
                                code: TokenCode::WORD(self.get_slice(interval)),
                                line: self.line,
                            };
                        }

                        Some(c) if c.is_ascii_whitespace() => {
                            let token = Token {
                                code: TokenCode::WORD(self.get_slice(interval)),
                                line: self.line,
                            };
                            self.state = State::START_TOKEN;
                            if c == b'\n' {
                                self.line += 1;
                            }
                            return token;
                        }

                        Some(_) => {
                            // self.pos += 1;
                            interval.1 = self.pos;
                        }
                    }
                }

                State::END => {
                    return Token {
                        code: TokenCode::END_CODE,
                        line: self.line,
                    };
                }
            }
        }
    }
}
