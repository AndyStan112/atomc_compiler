use crate::lexer::{Token, TokenCode};

pub struct Parser<'a> {
    tokens: &'a [Token<'a>],
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token<'a>]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn current(&self) -> &Token<'a> {
        &self.tokens[self.pos]
    }

    fn consume(&mut self, expected: fn(&TokenCode<'a>) -> bool) -> bool {
        if expected(&self.current().code) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, expected_name: &str, expected: fn(&TokenCode<'a>) -> bool) {
        if !self.consume(expected) {
            self.expected_error(expected_name);
        }
    }

    fn expected_error(&self, expected_name: &str) -> ! {
        let token = self.current();

        panic!(
            "\nSyntax error: expected {}, found {} `{}`\nLine: {}, position: {}\nNearby: ... {} ...\n",
            expected_name,
            self.token_kind(token),
            self.token_text(token),
            token.line,
            token.pos,
            self.nearby_tokens(3, 3)
        )
    }

    fn token_text(&self, token: &Token<'a>) -> String {
        match &token.code {
            TokenCode::ID(value) => value.to_string(),

            TokenCode::BREAK => "break".to_string(),
            TokenCode::CHAR => "char".to_string(),
            TokenCode::DOUBLE => "double".to_string(),
            TokenCode::ELSE => "else".to_string(),
            TokenCode::FOR => "for".to_string(),
            TokenCode::IF => "if".to_string(),
            TokenCode::INT => "int".to_string(),
            TokenCode::RETURN => "return".to_string(),
            TokenCode::STRUCT => "struct".to_string(),
            TokenCode::VOID => "void".to_string(),
            TokenCode::WHILE => "while".to_string(),

            TokenCode::CT_INT(value) => value.to_string(),
            TokenCode::CT_REAL(value) => value.to_string(),
            TokenCode::CT_CHAR(value) => format!("'{}'", value),
            TokenCode::CT_STRING(value) => format!("\"{}\"", value),

            TokenCode::COMMA => ",".to_string(),
            TokenCode::SEMICOLON => ";".to_string(),
            TokenCode::LPAR => "(".to_string(),
            TokenCode::RPAR => ")".to_string(),
            TokenCode::LBRACKET => "[".to_string(),
            TokenCode::RBRACKET => "]".to_string(),
            TokenCode::LACC => "{".to_string(),
            TokenCode::RACC => "}".to_string(),

            TokenCode::ADD => "+".to_string(),
            TokenCode::SUB => "-".to_string(),
            TokenCode::MUL => "*".to_string(),
            TokenCode::DIV => "/".to_string(),
            TokenCode::DOT => ".".to_string(),
            TokenCode::AND => "&&".to_string(),
            TokenCode::OR => "||".to_string(),
            TokenCode::NOT => "!".to_string(),
            TokenCode::ASSIGN => "=".to_string(),
            TokenCode::EQUAL => "==".to_string(),
            TokenCode::NOTEQ => "!=".to_string(),
            TokenCode::LESS => "<".to_string(),
            TokenCode::LESSEQ => "<=".to_string(),
            TokenCode::GREATER => ">".to_string(),
            TokenCode::GREATEREQ => ">=".to_string(),

            TokenCode::END => "end of file".to_string(),
        }
    }

    fn token_kind(&self, token: &Token<'a>) -> String {
        match &token.code {
            TokenCode::ID(_) => "identifier".to_string(),
            TokenCode::CT_INT(_) => "integer constant".to_string(),
            TokenCode::CT_REAL(_) => "real constant".to_string(),
            TokenCode::CT_CHAR(_) => "char constant".to_string(),
            TokenCode::CT_STRING(_) => "string constant".to_string(),
            TokenCode::END => "END".to_string(),
            other => format!("{:?}", other),
        }
    }

    fn nearby_tokens(&self, before: usize, after: usize) -> String {
        if self.tokens.is_empty() {
            return String::new();
        }

        let start = self.pos.saturating_sub(before);
        let end = (self.pos + after + 1).min(self.tokens.len());

        let mut parts = Vec::new();

        for i in start..end {
            let text = self.token_text(&self.tokens[i]);

            if i == self.pos {
                parts.push(format!(">>{}<<", text));
            } else {
                parts.push(text);
            }
        }

        parts.join(" ")
    }

    pub fn parse(&mut self) {
        self.unit();
    }

    pub fn unit(&mut self) -> bool {
        loop {
            let start_pos = self.pos;

            if self.struct_def() {
                continue;
            }

            self.pos = start_pos;

            if self.fn_def() {
                continue;
            }

            self.pos = start_pos;

            if self.var_def() {
                continue;
            }

            self.pos = start_pos;

            if self.consume(|code| matches!(code, TokenCode::END)) {
                return true;
            }

            self.expected_error("struct definition, function definition, variable definition, or end of file");
        }
    }

    pub fn struct_def(&mut self) -> bool {
        let start_pos = self.pos;

        if !self.consume(|code| matches!(code, TokenCode::STRUCT)) {
            return false;
        }

        if !self.consume(|code| matches!(code, TokenCode::ID(_))) {
            self.expected_error("struct name");
        }

        if !self.consume(|code| matches!(code, TokenCode::LACC)) {
            self.pos = start_pos;
            return false;
        }

        while !matches!(self.current().code, TokenCode::RACC | TokenCode::END) {
            if !self.var_def() {
                self.expected_error("variable declaration inside struct");
            }
        }

        self.expect("`}` after struct body", |code| {
            matches!(code, TokenCode::RACC)
        });

        self.expect("`;` after struct definition", |code| {
            matches!(code, TokenCode::SEMICOLON)
        });

        true
    }

    pub fn var_def(&mut self) -> bool {
        if !self.type_base() {
            return false;
        }

        self.expect("identifier after type", |code| {
            matches!(code, TokenCode::ID(_))
        });

        self.array_decl();

        self.expect("`;` after variable declaration", |code| {
            matches!(code, TokenCode::SEMICOLON)
        });

        true
    }

    pub fn type_base(&mut self) -> bool {
        if self.consume(|code| matches!(code, TokenCode::INT)) {
            return true;
        }

        if self.consume(|code| matches!(code, TokenCode::DOUBLE)) {
            return true;
        }

        if self.consume(|code| matches!(code, TokenCode::CHAR)) {
            return true;
        }

        if self.consume(|code| matches!(code, TokenCode::STRUCT)) {
            self.expect("struct type name", |code| {
                matches!(code, TokenCode::ID(_))
            });

            return true;
        }

        false
    }

    pub fn array_decl(&mut self) -> bool {
        if !self.consume(|code| matches!(code, TokenCode::LBRACKET)) {
            return false;
        }

        self.consume(|code| matches!(code, TokenCode::CT_INT(_)));

        self.expect("`]` after array declaration", |code| {
            matches!(code, TokenCode::RBRACKET)
        });

        true
    }

    pub fn fn_def(&mut self) -> bool {
        let start_pos = self.pos;

        let is_void = self.consume(|code| matches!(code, TokenCode::VOID));

        if !is_void && !self.type_base() {
            return false;
        }

        if !self.consume(|code| matches!(code, TokenCode::ID(_))) {
            if is_void {
                self.expected_error("function name after `void`");
            }

            self.pos = start_pos;
            return false;
        }

        if !self.consume(|code| matches!(code, TokenCode::LPAR)) {
            if is_void {
                self.expected_error("`(` after function name");
            }

            self.pos = start_pos;
            return false;
        }

        if self.fn_param() {
            while self.consume(|code| matches!(code, TokenCode::COMMA)) {
                if !self.fn_param() {
                    self.expected_error("function parameter after `,`");
                }
            }
        }

        self.expect("`)` after function parameters", |code| {
            matches!(code, TokenCode::RPAR)
        });

        if !self.stm_compound() {
            self.expected_error("function body");
        }

        true
    }

    pub fn fn_param(&mut self) -> bool {
        if !self.type_base() {
            return false;
        }

        self.expect("parameter name", |code| {
            matches!(code, TokenCode::ID(_))
        });

        self.array_decl();

        true
    }

    pub fn stm(&mut self) -> bool {
        if self.stm_compound() {
            return true;
        }

        if self.consume(|code| matches!(code, TokenCode::IF)) {
            self.expect("`(` after `if`", |code| {
                matches!(code, TokenCode::LPAR)
            });

            if !self.expr() {
                self.expected_error("expression after `if (`");
            }

            self.expect("`)` after if condition", |code| {
                matches!(code, TokenCode::RPAR)
            });

            if !self.stm() {
                self.expected_error("statement after if condition");
            }

            if self.consume(|code| matches!(code, TokenCode::ELSE)) {
                if !self.stm() {
                    self.expected_error("statement after `else`");
                }
            }

            return true;
        }

        if self.consume(|code| matches!(code, TokenCode::WHILE)) {
            self.expect("`(` after `while`", |code| {
                matches!(code, TokenCode::LPAR)
            });

            if !self.expr() {
                self.expected_error("expression after `while (`");
            }

            self.expect("`)` after while condition", |code| {
                matches!(code, TokenCode::RPAR)
            });

            if !self.stm() {
                self.expected_error("statement after while condition");
            }

            return true;
        }

        if self.consume(|code| matches!(code, TokenCode::FOR)) {
            self.expect("`(` after `for`", |code| {
                matches!(code, TokenCode::LPAR)
            });

            self.expr();

            self.expect("first `;` in for statement", |code| {
                matches!(code, TokenCode::SEMICOLON)
            });

            self.expr();

            self.expect("second `;` in for statement", |code| {
                matches!(code, TokenCode::SEMICOLON)
            });

            self.expr();

            self.expect("`)` after for clauses", |code| {
                matches!(code, TokenCode::RPAR)
            });

            if !self.stm() {
                self.expected_error("statement after for clauses");
            }

            return true;
        }

        if self.consume(|code| matches!(code, TokenCode::BREAK)) {
            self.expect("`;` after `break`", |code| {
                matches!(code, TokenCode::SEMICOLON)
            });

            return true;
        }

        if self.consume(|code| matches!(code, TokenCode::RETURN)) {
            self.expr();

            self.expect("`;` after `return` statement", |code| {
                matches!(code, TokenCode::SEMICOLON)
            });

            return true;
        }

        let start_pos = self.pos;

        if self.expr() {
            self.expect("`;` after expression statement", |code| {
                matches!(code, TokenCode::SEMICOLON)
            });

            return true;
        }

        self.pos = start_pos;

        if self.consume(|code| matches!(code, TokenCode::SEMICOLON)) {
            return true;
        }

        false
    }

    pub fn stm_compound(&mut self) -> bool {
        if !self.consume(|code| matches!(code, TokenCode::LACC)) {
            return false;
        }

        loop {
            let start_pos = self.pos;

            if self.var_def() {
                continue;
            }

            self.pos = start_pos;

            if self.stm() {
                continue;
            }

            self.pos = start_pos;
            break;
        }

        self.expect("`}` after compound statement", |code| {
            matches!(code, TokenCode::RACC)
        });

        true
    }

    pub fn expr(&mut self) -> bool {
        self.expr_assign()
    }

    pub fn expr_assign(&mut self) -> bool {
        let start_pos = self.pos;

        if self.expr_unary() {
            if self.consume(|code| matches!(code, TokenCode::ASSIGN)) {
                if !self.expr_assign() {
                    self.expected_error("expression after `=`");
                }

                return true;
            }
        }

        self.pos = start_pos;

        self.expr_or()
    }

    pub fn expr_or(&mut self) -> bool {
        if !self.expr_and() {
            return false;
        }

        while self.consume(|code| matches!(code, TokenCode::OR)) {
            if !self.expr_and() {
                self.expected_error("expression after `||`");
            }
        }

        true
    }

    pub fn expr_and(&mut self) -> bool {
        if !self.expr_eq() {
            return false;
        }

        while self.consume(|code| matches!(code, TokenCode::AND)) {
            if !self.expr_eq() {
                self.expected_error("expression after `&&`");
            }
        }

        true
    }

    pub fn expr_eq(&mut self) -> bool {
        if !self.expr_rel() {
            return false;
        }

        while self.consume(|code| matches!(code, TokenCode::EQUAL | TokenCode::NOTEQ)) {
            if !self.expr_rel() {
                self.expected_error("expression after equality operator");
            }
        }

        true
    }

    pub fn expr_rel(&mut self) -> bool {
        if !self.expr_add() {
            return false;
        }

        while self.consume(|code| {
            matches!(
                code,
                TokenCode::LESS | TokenCode::LESSEQ | TokenCode::GREATER | TokenCode::GREATEREQ
            )
        }) {
            if !self.expr_add() {
                self.expected_error("expression after relational operator");
            }
        }

        true
    }

    pub fn expr_add(&mut self) -> bool {
        if !self.expr_mul() {
            return false;
        }

        while self.consume(|code| matches!(code, TokenCode::ADD | TokenCode::SUB)) {
            if !self.expr_mul() {
                self.expected_error("expression after `+` or `-`");
            }
        }

        true
    }

    pub fn expr_mul(&mut self) -> bool {
        if !self.expr_cast() {
            return false;
        }

        while self.consume(|code| matches!(code, TokenCode::MUL | TokenCode::DIV)) {
            if !self.expr_cast() {
                self.expected_error("expression after `*` or `/`");
            }
        }

        true
    }

    pub fn expr_cast(&mut self) -> bool {
        let start_pos = self.pos;

        if self.consume(|code| matches!(code, TokenCode::LPAR)) {
            if self.type_base() {
                self.array_decl();

                self.expect("`)` after cast type", |code| {
                    matches!(code, TokenCode::RPAR)
                });

                if !self.expr_cast() {
                    self.expected_error("expression after cast");
                }

                return true;
            }
        }

        self.pos = start_pos;

        self.expr_unary()
    }

    pub fn expr_unary(&mut self) -> bool {
        if self.consume(|code| matches!(code, TokenCode::SUB | TokenCode::NOT)) {
            if !self.expr_unary() {
                self.expected_error("expression after unary operator");
            }

            return true;
        }

        self.expr_postfix()
    }

    pub fn expr_postfix(&mut self) -> bool {
        if !self.expr_primary() {
            return false;
        }

        loop {
            if self.consume(|code| matches!(code, TokenCode::LBRACKET)) {
                if !self.expr() {
                    self.expected_error("expression inside `[` `]`");
                }

                self.expect("`]` after index expression", |code| {
                    matches!(code, TokenCode::RBRACKET)
                });

                continue;
            }

            if self.consume(|code| matches!(code, TokenCode::DOT)) {
                self.expect("field name after `.`", |code| {
                    matches!(code, TokenCode::ID(_))
                });

                continue;
            }

            break;
        }

        true
    }

    pub fn expr_primary(&mut self) -> bool {
        if self.consume(|code| matches!(code, TokenCode::ID(_))) {
            if self.consume(|code| matches!(code, TokenCode::LPAR)) {
                if self.expr() {
                    while self.consume(|code| matches!(code, TokenCode::COMMA)) {
                        if !self.expr() {
                            self.expected_error("expression after `,`");
                        }
                    }
                }

                self.expect("`)` after function call arguments", |code| {
                    matches!(code, TokenCode::RPAR)
                });
            }

            return true;
        }

        if self.consume(|code| matches!(code, TokenCode::CT_INT(_))) {
            return true;
        }

        if self.consume(|code| matches!(code, TokenCode::CT_REAL(_))) {
            return true;
        }

        if self.consume(|code| matches!(code, TokenCode::CT_CHAR(_))) {
            return true;
        }

        if self.consume(|code| matches!(code, TokenCode::CT_STRING(_))) {
            return true;
        }

        if self.consume(|code| matches!(code, TokenCode::LPAR)) {
            if !self.expr() {
                self.expected_error("expression after `(`");
            }

            self.expect("`)` after expression", |code| {
                matches!(code, TokenCode::RPAR)
            });

            return true;
        }

        false
    }
}