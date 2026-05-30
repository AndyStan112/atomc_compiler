use crate::lexer::{Token, TokenCode};

#[derive(Debug, Clone, PartialEq)]
pub enum TypeBase<'a> {
    Int,
    Double,
    Char,
    Struct(&'a str),
    Void,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Type<'a> {
    pub tb: TypeBase<'a>,
    pub n: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Var,
    Fn,
    Struct,
    Param,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MemoryKind {
    Global,
    Arg,
    Local,
    StructMember,
}

#[derive(Debug, Clone, PartialEq)]
enum ScopeKind {
    Global,
    Fn,
    Struct,
    Block,
}

#[derive(Debug, Clone)]
pub struct Symbol<'a> {
    pub name: &'a str,
    pub kind: SymbolKind,
    pub memory: MemoryKind,
    pub typ: Type<'a>,
    pub var_idx: usize,
    pub param_idx: usize,
    pub params: Vec<Symbol<'a>>,
    pub members: Vec<Symbol<'a>>,
}

#[derive(Debug, Clone)]
struct Scope<'a> {
    name: String,
    kind: ScopeKind,
    symbols: Vec<Symbol<'a>>,
    parent: Option<usize>,
    children: Vec<usize>,
}

#[derive(Debug, Clone, Copy)]
enum Owner<'a> {
    Fn(&'a str),
    Struct(&'a str),
}

#[derive(Debug, Clone, Copy)]
enum ErrorKind {
    Syntax,
    Domain,
    Type,
}

pub struct Analyser<'a> {
    tokens: &'a [Token<'a>],
    pos: usize,
    scopes: Vec<Scope<'a>>,
    scope_stack: Vec<usize>,
    owner: Option<Owner<'a>>,
    global_offset: usize,
    anonymous_scope_counter: usize,
    last_type: Option<Type<'a>>,
    last_array_decl_present: bool,
}

pub type Parser<'a> = Analyser<'a>;

impl<'a> Analyser<'a> {
    pub fn new(tokens: &'a [Token<'a>]) -> Self {
        Self {
            tokens,
            pos: 0,
            scopes: vec![Scope {
                name: "global".to_string(),
                kind: ScopeKind::Global,
                symbols: Vec::new(),
                parent: None,
                children: Vec::new(),
            }],
            scope_stack: vec![0],
            owner: None,
            global_offset: 0,
            anonymous_scope_counter: 0,
            last_type: None,
            last_array_decl_present: false,
        }
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

        self.error_at(
            ErrorKind::Syntax,
            self.pos,
            format!(
                "expected {}, found {} `{}`",
                expected_name,
                self.token_kind(token),
                self.token_text(token)
            ),
        )
    }

    fn domain_error_at(&self, token_pos: usize, message: String) -> ! {
        self.error_at(ErrorKind::Domain, token_pos, message)
    }

    #[allow(dead_code)]
    fn type_error_at(&self, token_pos: usize, message: String) -> ! {
        self.error_at(ErrorKind::Type, token_pos, message)
    }

    fn error_at(&self, kind: ErrorKind, token_pos: usize, message: String) -> ! {
        let token = &self.tokens[token_pos];
        let kind_text = match kind {
            ErrorKind::Syntax => "Syntax error",
            ErrorKind::Domain => "Domain error",
            ErrorKind::Type => "Type error",
        };

        panic!(
            "\n{}: {}\nScope: {}\nLine: {}, position: {}\nNearby: ... {} ...\n",
            kind_text,
            message,
            self.current_scope_text(),
            token.line,
            token.pos,
            self.nearby_tokens_at(token_pos, 3, 3)
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
        self.nearby_tokens_at(self.pos, before, after)
    }

    fn nearby_tokens_at(&self, pos: usize, before: usize, after: usize) -> String {
        if self.tokens.is_empty() {
            return String::new();
        }

        let start = pos.saturating_sub(before);
        let end = (pos + after + 1).min(self.tokens.len());

        let mut parts = Vec::new();

        for i in start..end {
            let text = self.token_text(&self.tokens[i]);

            if i == pos {
                parts.push(format!(">>{}<<", text));
            } else {
                parts.push(text);
            }
        }

        parts.join(" ")
    }

    fn current_scope_idx(&self) -> usize {
        *self.scope_stack.last().unwrap()
    }

    fn current_scope(&self) -> &Scope<'a> {
        &self.scopes[self.current_scope_idx()]
    }

    fn push_scope(&mut self, name: String, kind: ScopeKind) {
        let parent = self.current_scope_idx();
        let idx = self.scopes.len();

        self.scopes.push(Scope {
            name,
            kind,
            symbols: Vec::new(),
            parent: Some(parent),
            children: Vec::new(),
        });

        self.scopes[parent].children.push(idx);
        self.scope_stack.push(idx);
    }

    fn drop_scope(&mut self) {
        self.scope_stack.pop();
    }

    fn current_scope_text(&self) -> String {
        let mut parts = Vec::new();

        for idx in &self.scope_stack {
            parts.push(self.scopes[*idx].name.clone());
        }

        parts.join(" > ")
    }

    fn add_symbol_to_current_scope(&mut self, symbol: Symbol<'a>) {
        let scope_idx = self.current_scope_idx();
        self.scopes[scope_idx].symbols.push(symbol);
    }

    fn find_symbol(&self, name: &str) -> Option<&Symbol<'a>> {
        for scope_idx in self.scope_stack.iter().rev() {
            for symbol in self.scopes[*scope_idx].symbols.iter().rev() {
                if symbol.name == name {
                    return Some(symbol);
                }
            }
        }

        None
    }

    fn find_symbol_in_current_scope(&self, name: &str) -> Option<&Symbol<'a>> {
        self.current_scope()
            .symbols
            .iter()
            .rev()
            .find(|symbol| symbol.name == name)
    }

    fn find_symbol_mut_anywhere(
        &mut self,
        name: &str,
        kind: SymbolKind,
    ) -> Option<&mut Symbol<'a>> {
        for scope in &mut self.scopes {
            for symbol in &mut scope.symbols {
                if symbol.name == name && symbol.kind == kind {
                    return Some(symbol);
                }
            }
        }

        None
    }

    fn new_symbol(&self, name: &'a str, kind: SymbolKind, memory: MemoryKind, typ: Type<'a>) -> Symbol<'a> {
        Symbol {
            name,
            kind,
            memory,
            typ,
            var_idx: 0,
            param_idx: 0,
            params: Vec::new(),
            members: Vec::new(),
        }
    }

    fn alloc_global(&mut self, size: usize) -> usize {
        let offset = self.global_offset;
        self.global_offset += size;
        offset
    }

    fn type_size(&self, typ: &Type<'a>) -> usize {
        let base_size = match typ.tb {
            TypeBase::Int => 4,
            TypeBase::Double => 8,
            TypeBase::Char => 1,
            TypeBase::Void => 0,
            TypeBase::Struct(name) => self.struct_size(name),
        };

        if typ.n > 0 {
            base_size * typ.n as usize
        } else {
            base_size
        }
    }

    fn struct_size(&self, name: &str) -> usize {
        for scope in &self.scopes {
            for symbol in &scope.symbols {
                if symbol.name == name && symbol.kind == SymbolKind::Struct {
                    return symbol.members.iter().map(|member| self.type_size(&member.typ)).sum();
                }
            }
        }

        0
    }

    fn type_text(&self, typ: &Type<'a>) -> String {
        let base = match typ.tb {
            TypeBase::Int => "int".to_string(),
            TypeBase::Double => "double".to_string(),
            TypeBase::Char => "char".to_string(),
            TypeBase::Void => "void".to_string(),
            TypeBase::Struct(name) => format!("struct {}", name),
        };

        if typ.n > 0 {
            format!("{}[{}]", base, typ.n)
        } else if typ.n == 0 {
            format!("{}[]", base)
        } else {
            base
        }
    }

    fn id_name_at(&self, token_pos: usize) -> &'a str {
        match self.tokens[token_pos].code {
            TokenCode::ID(name) => name,
            _ => self.expected_error("identifier"),
        }
    }

    fn ct_int_at(&self, token_pos: usize) -> i64 {
        match self.tokens[token_pos].code {
            TokenCode::CT_INT(value) => value,
            _ => self.expected_error("integer constant"),
        }
    }

    fn function_scope_idx(&self, name: &str) -> Option<usize> {
        let expected = format!("function {}", name);

        self.scopes
            .iter()
            .position(|scope| scope.kind == ScopeKind::Fn && scope.name == expected)
    }

    pub fn print_symbols(&self) {
        self.print_scope_symbols(0, 0);
    }

    fn print_scope_symbols(&self, scope_idx: usize, indent: usize) {
        for symbol in &self.scopes[scope_idx].symbols {
            let prefix = "  ".repeat(indent);

            println!(
                "{}{} {:?} {:?} {}",
                prefix,
                symbol.name,
                symbol.kind,
                symbol.memory,
                self.type_text(&symbol.typ)
            );

            match symbol.kind {
                SymbolKind::Struct => {
                    for member in &symbol.members {
                        println!(
                            "{}  member {} {:?} {:?} {}",
                            prefix,
                            member.name,
                            member.kind,
                            member.memory,
                            self.type_text(&member.typ)
                        );
                    }
                }
                SymbolKind::Fn => {
                    for param in &symbol.params {
                        println!(
                            "{}  param {} {:?} {:?} {}",
                            prefix,
                            param.name,
                            param.kind,
                            param.memory,
                            self.type_text(&param.typ)
                        );
                    }

                    if let Some(fn_scope_idx) = self.function_scope_idx(symbol.name) {
                        self.print_function_scope(fn_scope_idx, indent + 1);
                    }
                }
                _ => {}
            }
        }
    }

    fn print_function_scope(&self, scope_idx: usize, indent: usize) {
        let prefix = "  ".repeat(indent);

        for symbol in &self.scopes[scope_idx].symbols {
            if symbol.kind == SymbolKind::Var {
                println!(
                    "{}local {} {:?} {:?} {}",
                    prefix,
                    symbol.name,
                    symbol.kind,
                    symbol.memory,
                    self.type_text(&symbol.typ)
                );
            }
        }

        for child_idx in &self.scopes[scope_idx].children {
            self.print_child_scope(*child_idx, indent);
        }
    }

    fn print_child_scope(&self, scope_idx: usize, indent: usize) {
        let prefix = "  ".repeat(indent);

        if self.scopes[scope_idx].kind == ScopeKind::Block {
            println!("{}{}", prefix, self.scopes[scope_idx].name);
            self.print_function_scope(scope_idx, indent + 1);
        }
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

        let name_pos = self.pos;

        if !self.consume(|code| matches!(code, TokenCode::ID(_))) {
            self.expected_error("struct name");
        }

        let name = self.id_name_at(name_pos);

        if !self.consume(|code| matches!(code, TokenCode::LACC)) {
            self.pos = start_pos;
            return false;
        }

        if self.find_symbol_in_current_scope(name).is_some() {
            self.domain_error_at(name_pos, format!("symbol redefinition: {}", name));
        }

        let typ = Type {
            tb: TypeBase::Struct(name),
            n: -1,
        };

        let symbol = self.new_symbol(name, SymbolKind::Struct, MemoryKind::Global, typ);
        self.add_symbol_to_current_scope(symbol);

        let old_owner = self.owner;
        self.owner = Some(Owner::Struct(name));
        self.push_scope(format!("struct {}", name), ScopeKind::Struct);

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

        self.owner = old_owner;
        self.drop_scope();

        true
    }

    pub fn var_def(&mut self) -> bool {
        if !self.type_base() {
            return false;
        }

        let mut typ = self.last_type.clone().unwrap();
        let name_pos = self.pos;

        self.expect("identifier after type", |code| {
            matches!(code, TokenCode::ID(_))
        });

        let name = self.id_name_at(name_pos);

        self.array_decl();
        typ = self.last_type.clone().unwrap_or(typ);

        if self.last_array_decl_present && typ.n == 0 {
            self.domain_error_at(name_pos, "a vector variable must have a specified dimension".to_string());
        }

        self.expect("`;` after variable declaration", |code| {
            matches!(code, TokenCode::SEMICOLON)
        });

        if self.find_symbol_in_current_scope(name).is_some() {
            self.domain_error_at(name_pos, format!("symbol redefinition: {}", name));
        }

        let memory = match self.current_scope().kind {
            ScopeKind::Global => MemoryKind::Global,
            ScopeKind::Struct => MemoryKind::StructMember,
            ScopeKind::Fn | ScopeKind::Block => MemoryKind::Local,
        };

        let mut symbol = self.new_symbol(name, SymbolKind::Var, memory.clone(), typ.clone());

        symbol.var_idx = match memory {
            MemoryKind::Global => self.alloc_global(self.type_size(&typ)),
            MemoryKind::StructMember => self.current_scope().symbols.iter().map(|symbol| self.type_size(&symbol.typ)).sum(),
            MemoryKind::Local => self.current_scope().symbols.iter().filter(|symbol| symbol.kind == SymbolKind::Var).count(),
            MemoryKind::Arg => 0,
        };

        self.add_symbol_to_current_scope(symbol.clone());

        if let Some(Owner::Struct(owner_name)) = self.owner {
            if let Some(owner) = self.find_symbol_mut_anywhere(owner_name, SymbolKind::Struct) {
                owner.members.push(symbol);
            }
        }

        true
    }

    pub fn type_base(&mut self) -> bool {
        self.last_type = None;

        if self.consume(|code| matches!(code, TokenCode::INT)) {
            self.last_type = Some(Type {
                tb: TypeBase::Int,
                n: -1,
            });
            return true;
        }

        if self.consume(|code| matches!(code, TokenCode::DOUBLE)) {
            self.last_type = Some(Type {
                tb: TypeBase::Double,
                n: -1,
            });
            return true;
        }

        if self.consume(|code| matches!(code, TokenCode::CHAR)) {
            self.last_type = Some(Type {
                tb: TypeBase::Char,
                n: -1,
            });
            return true;
        }

        if self.consume(|code| matches!(code, TokenCode::STRUCT)) {
            let name_pos = self.pos;

            self.expect("struct type name", |code| {
                matches!(code, TokenCode::ID(_))
            });

            let name = self.id_name_at(name_pos);

            match self.find_symbol(name) {
                Some(symbol) if symbol.kind == SymbolKind::Struct => {
                    self.last_type = Some(Type {
                        tb: TypeBase::Struct(name),
                        n: -1,
                    });
                }
                _ => self.domain_error_at(name_pos, format!("undefined structure: {}", name)),
            }

            return true;
        }

        false
    }

    pub fn array_decl(&mut self) -> bool {
        self.last_array_decl_present = false;

        if !self.consume(|code| matches!(code, TokenCode::LBRACKET)) {
            return false;
        }

        self.last_array_decl_present = true;
        let mut n = 0;

        let size_pos = self.pos;
        if self.consume(|code| matches!(code, TokenCode::CT_INT(_))) {
            n = self.ct_int_at(size_pos);
        }

        if let Some(typ) = &mut self.last_type {
            typ.n = n;
        }

        self.expect("`]` after array declaration", |code| {
            matches!(code, TokenCode::RBRACKET)
        });

        true
    }

    pub fn fn_def(&mut self) -> bool {
        let start_pos = self.pos;

        let is_void = self.consume(|code| matches!(code, TokenCode::VOID));
        let mut typ = if is_void {
            Type {
                tb: TypeBase::Void,
                n: -1,
            }
        } else {
            Type {
                tb: TypeBase::Void,
                n: -1,
            }
        };

        if !is_void && !self.type_base() {
            return false;
        }

        if !is_void {
            typ = self.last_type.clone().unwrap();
        }

        let name_pos = self.pos;

        if !self.consume(|code| matches!(code, TokenCode::ID(_))) {
            if is_void {
                self.expected_error("function name after `void`");
            }

            self.pos = start_pos;
            return false;
        }

        let name = self.id_name_at(name_pos);

        if !self.consume(|code| matches!(code, TokenCode::LPAR)) {
            if is_void {
                self.expected_error("`(` after function name");
            }

            self.pos = start_pos;
            return false;
        }

        if self.find_symbol_in_current_scope(name).is_some() {
            self.domain_error_at(name_pos, format!("symbol redefinition: {}", name));
        }

        let symbol = self.new_symbol(name, SymbolKind::Fn, MemoryKind::Global, typ);
        self.add_symbol_to_current_scope(symbol);

        let old_owner = self.owner;
        self.owner = Some(Owner::Fn(name));
        self.anonymous_scope_counter = 0;
        self.push_scope(format!("function {}", name), ScopeKind::Fn);

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

        if !self.stm_compound_with_new_domain(false) {
            self.expected_error("function body");
        }

        self.drop_scope();
        self.owner = old_owner;

        true
    }

    pub fn fn_param(&mut self) -> bool {
        if !self.type_base() {
            return false;
        }

        let mut typ = self.last_type.clone().unwrap();
        let name_pos = self.pos;

        self.expect("parameter name", |code| {
            matches!(code, TokenCode::ID(_))
        });

        let name = self.id_name_at(name_pos);

        self.array_decl();
        typ = self.last_type.clone().unwrap_or(typ);

        if self.last_array_decl_present {
            typ.n = 0;
        }

        if self.find_symbol_in_current_scope(name).is_some() {
            self.domain_error_at(name_pos, format!("symbol redefinition: {}", name));
        }

        let mut symbol = self.new_symbol(name, SymbolKind::Param, MemoryKind::Arg, typ);
        symbol.param_idx = self.current_scope().symbols.iter().filter(|symbol| symbol.kind == SymbolKind::Param).count();

        self.add_symbol_to_current_scope(symbol.clone());

        if let Some(Owner::Fn(owner_name)) = self.owner {
            if let Some(owner) = self.find_symbol_mut_anywhere(owner_name, SymbolKind::Fn) {
                owner.params.push(symbol);
            }
        }

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
        self.stm_compound_with_new_domain(true)
    }

    fn stm_compound_with_new_domain(&mut self, new_domain: bool) -> bool {
        if !self.consume(|code| matches!(code, TokenCode::LACC)) {
            return false;
        }

        if new_domain {
            self.anonymous_scope_counter += 1;
            self.push_scope(format!("block{}", self.anonymous_scope_counter), ScopeKind::Block);
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

        if new_domain {
            self.drop_scope();
        }

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
        let name_pos = self.pos;

        if self.consume(|code| matches!(code, TokenCode::ID(_))) {
            let name = self.id_name_at(name_pos);

            if self.find_symbol(name).is_none() {
                self.domain_error_at(name_pos, format!("undefined symbol: {}", name));
            }

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
