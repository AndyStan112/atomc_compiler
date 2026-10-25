#![allow(dead_code)] // Preserve the baseline symbol-table inspection API.
use crate::layout::{self, Ty, Structs};
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
pub enum CtVal<'a> {
    Int(i64),
    Double(f64),
    Char(char),
    Str(&'a str),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetVal<'a> {
    pub typ: Type<'a>,
    pub is_lval: bool,
    pub is_ct_val: bool,
    pub ct_val: Option<CtVal<'a>>,
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
    statement_depth: usize,
    scopes: Vec<Scope<'a>>,
    scope_stack: Vec<usize>,
    owner: Option<Owner<'a>>,
    global_offset: usize,
    layouts: Structs,
    anonymous_scope_counter: usize,
    last_type: Option<Type<'a>>,
    last_array_decl_present: bool,
    last_retval: Option<RetVal<'a>>,
}

pub type Parser<'a> = Analyser<'a>;

impl<'a> Analyser<'a> {
    pub fn new(tokens: &'a [Token<'a>]) -> Self {
        Self {
            tokens,
            pos: 0,
            statement_depth: 0,
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
            layouts: Structs::new(),
            anonymous_scope_counter: 0,
            last_type: None,
            last_array_decl_present: false,
            last_retval: None,
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


    fn create_type(&self, tb: TypeBase<'a>, n: i64) -> Type<'a> {
        Type { tb, n }
    }

    fn create_retval(&self, typ: Type<'a>, is_lval: bool, is_ct_val: bool) -> RetVal<'a> {
        RetVal {
            typ,
            is_lval,
            is_ct_val,
            ct_val: None,
        }
    }

    fn int_type(&self) -> Type<'a> {
        self.create_type(TypeBase::Int, -1)
    }

    fn can_be_scalar(&self, rv: &RetVal<'a>) -> bool {
        rv.typ.n < 0 && matches!(rv.typ.tb, TypeBase::Char | TypeBase::Int | TypeBase::Double)
    }

    fn same_base_type(&self, dst: &Type<'a>, src: &Type<'a>) -> bool {
        match (&dst.tb, &src.tb) {
            (TypeBase::Int, TypeBase::Int)
            | (TypeBase::Double, TypeBase::Double)
            | (TypeBase::Char, TypeBase::Char)
            | (TypeBase::Void, TypeBase::Void) => true,
            (TypeBase::Struct(dst_name), TypeBase::Struct(src_name)) => dst_name == src_name,
            _ => false,
        }
    }

    fn conv_to(&self, src: &Type<'a>, dst: &Type<'a>) -> bool {
        if src.n >= 0 {
            return dst.n >= 0 && self.same_base_type(dst, src);
        }

        if dst.n >= 0 {
            return false;
        }

        match (&src.tb, &dst.tb) {
            (TypeBase::Char | TypeBase::Int | TypeBase::Double, TypeBase::Char | TypeBase::Int | TypeBase::Double) => true,
            (TypeBase::Struct(src_name), TypeBase::Struct(dst_name)) => src_name == dst_name,
            _ => false,
        }
    }

    fn cast(&self, dst: &Type<'a>, src: &Type<'a>, token_pos: usize) {
        if src.n >= 0 {
            if dst.n >= 0 {
                if !self.same_base_type(dst, src) {
                    self.type_error_at(token_pos, "an array cannot be converted to an array of another type".to_string());
                }
            } else {
                self.type_error_at(token_pos, "an array cannot be converted to a non-array".to_string());
            }
        } else if dst.n >= 0 {
            self.type_error_at(token_pos, "a non-array cannot be converted to an array".to_string());
        }

        if !self.conv_to(src, dst) {
            self.type_error_at(token_pos, "incompatible types".to_string());
        }
    }

    fn get_arith_type(&self, s1: &Type<'a>, s2: &Type<'a>, token_pos: usize) -> Type<'a> {
        if s1.n >= 0 || s2.n >= 0 {
            self.type_error_at(token_pos, "invalid array operand".to_string());
        }

        match (&s1.tb, &s2.tb) {
            (TypeBase::Double, TypeBase::Char | TypeBase::Int | TypeBase::Double)
            | (TypeBase::Char | TypeBase::Int, TypeBase::Double) => self.create_type(TypeBase::Double, -1),
            (TypeBase::Int, TypeBase::Char | TypeBase::Int)
            | (TypeBase::Char, TypeBase::Int) => self.create_type(TypeBase::Int, -1),
            (TypeBase::Char, TypeBase::Char) => self.create_type(TypeBase::Char, -1),
            _ => self.type_error_at(token_pos, "invalid arithmetic operand type".to_string()),
        }
    }

    fn current_function_type(&self) -> Option<Type<'a>> {
        let name = match self.owner {
            Some(Owner::Fn(name)) => name,
            _ => return None,
        };

        for scope in &self.scopes {
            for symbol in &scope.symbols {
                if symbol.name == name && symbol.kind == SymbolKind::Fn {
                    return Some(symbol.typ.clone());
                }
            }
        }

        None
    }

    fn find_struct_member(&self, struct_name: &str, field_name: &str) -> Option<Symbol<'a>> {
        for scope in &self.scopes {
            for symbol in &scope.symbols {
                if symbol.name == struct_name && symbol.kind == SymbolKind::Struct {
                    return symbol.members.iter().find(|member| member.name == field_name).cloned();
                }
            }
        }

        None
    }

    fn add_ext_func(&mut self, name: &'a str, typ: Type<'a>) {
        let symbol = self.new_symbol(name, SymbolKind::Fn, MemoryKind::Global, typ);
        self.add_symbol_to_current_scope(symbol);
    }

    fn add_func_arg(&mut self, func_name: &'a str, name: &'a str, typ: Type<'a>) {
        if let Some(func) = self.find_symbol_mut_anywhere(func_name, SymbolKind::Fn) {
            let mut arg = Symbol {
                name,
                kind: SymbolKind::Param,
                memory: MemoryKind::Arg,
                typ,
                var_idx: 0,
                param_idx: func.params.len(),
                params: Vec::new(),
                members: Vec::new(),
            };

            arg.param_idx = func.params.len();
            func.params.push(arg);
        }
    }

    pub fn add_ext_funcs(&mut self) {
        if self.find_symbol_in_current_scope("put_s").is_some() {
            return;
        }

        self.add_ext_func("put_s", self.create_type(TypeBase::Void, -1));
        self.add_func_arg("put_s", "s", self.create_type(TypeBase::Char, 0));

        self.add_ext_func("get_s", self.create_type(TypeBase::Void, -1));
        self.add_func_arg("get_s", "s", self.create_type(TypeBase::Char, 0));

        self.add_ext_func("put_i", self.create_type(TypeBase::Void, -1));
        self.add_func_arg("put_i", "i", self.create_type(TypeBase::Int, -1));

        self.add_ext_func("get_i", self.create_type(TypeBase::Int, -1));

        self.add_ext_func("put_d", self.create_type(TypeBase::Void, -1));
        self.add_func_arg("put_d", "d", self.create_type(TypeBase::Double, -1));

        self.add_ext_func("get_d", self.create_type(TypeBase::Double, -1));

        self.add_ext_func("put_c", self.create_type(TypeBase::Void, -1));
        self.add_func_arg("put_c", "c", self.create_type(TypeBase::Char, -1));

        self.add_ext_func("get_c", self.create_type(TypeBase::Char, -1));

        self.add_ext_func("seconds", self.create_type(TypeBase::Double, -1));
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

    fn guest_type(typ: &Type<'a>) -> Ty {
        let base = match typ.tb {
            TypeBase::Int => Ty::Int, TypeBase::Double => Ty::Double,
            TypeBase::Char => Ty::Char, TypeBase::Void => Ty::Void,
            TypeBase::Struct(name) => Ty::Struct(name.into()),
        };
        if typ.n > 0 { Ty::Array(Box::new(base), typ.n as usize) } else { base }
    }

    fn type_size(&self, typ: &Type<'a>) -> usize {
        layout::layout(&Self::guest_type(typ), &self.layouts)
            .unwrap_or_else(|e| self.domain_error_at(self.pos, e)).size
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
        self.add_ext_funcs();
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

        let fields = self.current_scope().symbols.iter()
            .map(|s| (s.name.to_string(), Self::guest_type(&s.typ))).collect();
        let definition = layout::structure(fields, &self.layouts)
            .unwrap_or_else(|e| self.domain_error_at(name_pos, e));
        self.layouts.insert(name.to_string(), definition);
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
            MemoryKind::Global => {
                let l = layout::layout(&Self::guest_type(&typ), &self.layouts)
                    .unwrap_or_else(|e| self.domain_error_at(name_pos, e));
                self.global_offset = layout::align_up(self.global_offset, l.align).unwrap();
                let size = l.size;
                if self.global_offset.checked_add(size).is_none_or(|end| end > layout::MAX_OBJECT) {
                    self.domain_error_at(name_pos, "globals exceed guest memory limit".into());
                }
                self.alloc_global(size)
            },
            MemoryKind::StructMember => {
                let l = layout::layout(&Self::guest_type(&typ), &self.layouts)
                    .unwrap_or_else(|e| self.domain_error_at(name_pos, e));
                let end = self.current_scope().symbols.last().map(|s| s.var_idx + self.type_size(&s.typ)).unwrap_or(0);
                layout::align_up(end, l.align).unwrap()
            },
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
        self.statement_depth += 1;
        if self.statement_depth > 128 {
            self.domain_error_at(self.pos, "statement nesting exceeds 128".into());
        }
        let result = self.stm_inner();
        self.statement_depth -= 1;
        result
    }

    fn stm_inner(&mut self) -> bool {
        if self.stm_compound() {
            return true;
        }

        if self.consume(|code| matches!(code, TokenCode::IF)) {
            let cond_pos = self.pos;

            self.expect("`(` after `if`", |code| {
                matches!(code, TokenCode::LPAR)
            });

            if !self.expr() {
                self.expected_error("expression after `if (`");
            }

            let cond = self.last_retval.clone().unwrap();
            if !self.can_be_scalar(&cond) {
                self.type_error_at(cond_pos, "the if condition must be a scalar value".to_string());
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
            let cond_pos = self.pos;

            self.expect("`(` after `while`", |code| {
                matches!(code, TokenCode::LPAR)
            });

            if !self.expr() {
                self.expected_error("expression after `while (`");
            }

            let cond = self.last_retval.clone().unwrap();
            if !self.can_be_scalar(&cond) {
                self.type_error_at(cond_pos, "the while condition must be a scalar value".to_string());
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

            let cond_pos = self.pos;
            if self.expr() {
                let cond = self.last_retval.clone().unwrap();
                if !self.can_be_scalar(&cond) {
                    self.type_error_at(cond_pos, "the for condition must be a scalar value".to_string());
                }
            }

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
            let return_pos = self.pos;
            let has_expr = self.expr();
            let fn_type = self.current_function_type().unwrap_or_else(|| self.create_type(TypeBase::Void, -1));

            if has_expr {
                let expr = self.last_retval.clone().unwrap();

                if matches!(fn_type.tb, TypeBase::Void) {
                    self.type_error_at(return_pos, "a void function cannot return a value".to_string());
                }

                if !self.can_be_scalar(&expr) {
                    self.type_error_at(return_pos, "the return value must be a scalar value".to_string());
                }

                if !self.conv_to(&expr.typ, &fn_type) {
                    self.type_error_at(return_pos, "cannot convert the return expression type to the function return type".to_string());
                }
            } else if !matches!(fn_type.tb, TypeBase::Void) {
                self.type_error_at(return_pos, "a non-void function must return a value".to_string());
            }

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
        if self.expr_or() {
            let dst = self.last_retval.clone().unwrap();
            let assign_pos = self.pos;

            if self.consume(|code| matches!(code, TokenCode::ASSIGN)) {
                if !self.expr_assign() {
                    self.expected_error("expression after `=`");
                }

                let src = self.last_retval.clone().unwrap();

                if !dst.is_lval {
                    self.type_error_at(assign_pos, "the assign destination must be a left-value".to_string());
                }

                if dst.is_ct_val {
                    self.type_error_at(assign_pos, "the assign destination cannot be constant".to_string());
                }

                if !self.can_be_scalar(&dst) {
                    self.type_error_at(assign_pos, "the assign destination must be scalar".to_string());
                }

                if !self.can_be_scalar(&src) {
                    self.type_error_at(assign_pos, "the assign source must be scalar".to_string());
                }

                self.cast(&dst.typ, &src.typ, assign_pos);

                let mut rv = dst;
                rv.is_lval = false;
                rv.is_ct_val = true;
                self.last_retval = Some(rv);

                return true;
            }
            return true;
        }
        false
    }

    pub fn expr_or(&mut self) -> bool {
        if !self.expr_and() {
            return false;
        }

        while self.consume(|code| matches!(code, TokenCode::OR)) {
            let op_pos = self.pos.saturating_sub(1);
            let left = self.last_retval.clone().unwrap();

            if !self.expr_and() {
                self.expected_error("expression after `||`");
            }

            let right = self.last_retval.clone().unwrap();
            self.get_arith_type(&left.typ, &right.typ, op_pos);
            self.last_retval = Some(self.create_retval(self.int_type(), false, true));
        }

        true
    }

    pub fn expr_and(&mut self) -> bool {
        if !self.expr_eq() {
            return false;
        }

        while self.consume(|code| matches!(code, TokenCode::AND)) {
            let op_pos = self.pos.saturating_sub(1);
            let left = self.last_retval.clone().unwrap();

            if !self.expr_eq() {
                self.expected_error("expression after `&&`");
            }

            let right = self.last_retval.clone().unwrap();
            self.get_arith_type(&left.typ, &right.typ, op_pos);
            self.last_retval = Some(self.create_retval(self.int_type(), false, true));
        }

        true
    }

    pub fn expr_eq(&mut self) -> bool {
        if !self.expr_rel() {
            return false;
        }

        while self.consume(|code| matches!(code, TokenCode::EQUAL | TokenCode::NOTEQ)) {
            let op_pos = self.pos.saturating_sub(1);
            let left = self.last_retval.clone().unwrap();

            if !self.expr_rel() {
                self.expected_error("expression after equality operator");
            }

            let right = self.last_retval.clone().unwrap();
            self.get_arith_type(&left.typ, &right.typ, op_pos);
            self.last_retval = Some(self.create_retval(self.int_type(), false, true));
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
            let op_pos = self.pos.saturating_sub(1);
            let left = self.last_retval.clone().unwrap();

            if !self.expr_add() {
                self.expected_error("expression after relational operator");
            }

            let right = self.last_retval.clone().unwrap();
            self.get_arith_type(&left.typ, &right.typ, op_pos);
            self.last_retval = Some(self.create_retval(self.int_type(), false, true));
        }

        true
    }

    pub fn expr_add(&mut self) -> bool {
        if !self.expr_mul() {
            return false;
        }

        while self.consume(|code| matches!(code, TokenCode::ADD | TokenCode::SUB)) {
            let op_pos = self.pos.saturating_sub(1);
            let left = self.last_retval.clone().unwrap();

            if !self.expr_mul() {
                self.expected_error("expression after `+` or `-`");
            }

            let right = self.last_retval.clone().unwrap();
            let typ = self.get_arith_type(&left.typ, &right.typ, op_pos);
            self.last_retval = Some(self.create_retval(typ, false, true));
        }

        true
    }

    pub fn expr_mul(&mut self) -> bool {
        if !self.expr_cast() {
            return false;
        }

        while self.consume(|code| matches!(code, TokenCode::MUL | TokenCode::DIV)) {
            let op_pos = self.pos.saturating_sub(1);
            let left = self.last_retval.clone().unwrap();

            if !self.expr_cast() {
                self.expected_error("expression after `*` or `/`");
            }

            let right = self.last_retval.clone().unwrap();
            let typ = self.get_arith_type(&left.typ, &right.typ, op_pos);
            self.last_retval = Some(self.create_retval(typ, false, true));
        }

        true
    }

    pub fn expr_cast(&mut self) -> bool {
        let start_pos = self.pos;

        if self.consume(|code| matches!(code, TokenCode::LPAR)) {
            let cast_pos = self.pos.saturating_sub(1);

            if self.type_base() {
                self.array_decl();
                let typ = self.last_type.clone().unwrap();

                self.expect("`)` after cast type", |code| {
                    matches!(code, TokenCode::RPAR)
                });

                if !self.expr_cast() {
                    self.expected_error("expression after cast");
                }

                let op = self.last_retval.clone().unwrap();

                if matches!(typ.tb, TypeBase::Struct(_)) {
                    self.type_error_at(cast_pos, "cannot convert to a struct type".to_string());
                }

                if matches!(op.typ.tb, TypeBase::Struct(_)) {
                    self.type_error_at(cast_pos, "cannot convert a struct".to_string());
                }

                if op.typ.n >= 0 && typ.n < 0 {
                    self.type_error_at(cast_pos, "an array can be converted only to another array".to_string());
                }

                if op.typ.n < 0 && typ.n >= 0 {
                    self.type_error_at(cast_pos, "a scalar can be converted only to another scalar".to_string());
                }

                self.last_retval = Some(self.create_retval(typ, false, true));

                return true;
            }
        }

        self.pos = start_pos;

        self.expr_unary()
    }

    pub fn expr_unary(&mut self) -> bool {
        if self.consume(|code| matches!(code, TokenCode::SUB | TokenCode::NOT)) {
            let op_pos = self.pos.saturating_sub(1);
            let is_not = matches!(self.tokens[op_pos].code, TokenCode::NOT);

            if !self.expr_unary() {
                self.expected_error("expression after unary operator");
            }

            let mut rv = self.last_retval.clone().unwrap();

            if !self.can_be_scalar(&rv) {
                self.type_error_at(op_pos, "unary operator must have a scalar operand".to_string());
            }

            if is_not {
                rv.typ = self.int_type();
            }

            rv.is_lval = false;
            rv.is_ct_val = true;
            rv.ct_val = None;
            self.last_retval = Some(rv);

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
                let index_pos = self.pos.saturating_sub(1);
                let mut rv = self.last_retval.clone().unwrap();

                if !self.expr() {
                    self.expected_error("expression inside `[` `]`");
                }

                let idx = self.last_retval.clone().unwrap();

                self.expect("`]` after index expression", |code| {
                    matches!(code, TokenCode::RBRACKET)
                });

                if rv.typ.n < 0 {
                    self.type_error_at(index_pos, "only an array can be indexed".to_string());
                }

                if !self.conv_to(&idx.typ, &self.int_type()) {
                    self.type_error_at(index_pos, "the index is not convertible to int".to_string());
                }

                rv.typ.n = -1;
                rv.is_lval = true;
                rv.is_ct_val = false;
                rv.ct_val = None;
                self.last_retval = Some(rv);

                continue;
            }

            if self.consume(|code| matches!(code, TokenCode::DOT)) {
                let dot_pos = self.pos.saturating_sub(1);
                let field_pos = self.pos;

                self.expect("field name after `.`", |code| {
                    matches!(code, TokenCode::ID(_))
                });

                let mut rv = self.last_retval.clone().unwrap();
                let field_name = self.id_name_at(field_pos);

                let struct_name = match rv.typ.tb {
                    TypeBase::Struct(name) => name,
                    _ => self.type_error_at(dot_pos, "a field can only be selected from a struct".to_string()),
                };

                let member = match self.find_struct_member(struct_name, field_name) {
                    Some(member) => member,
                    None => self.type_error_at(field_pos, format!("the structure {} does not have a field {}", struct_name, field_name)),
                };

                rv.typ = member.typ;
                rv.is_lval = true;
                rv.is_ct_val = rv.typ.n >= 0;
                rv.ct_val = None;
                self.last_retval = Some(rv);

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
            let symbol = match self.find_symbol(name) {
                Some(symbol) => symbol.clone(),
                None => self.domain_error_at(name_pos, format!("undefined symbol: {}", name)),
            };

            if self.consume(|code| matches!(code, TokenCode::LPAR)) {
                if symbol.kind != SymbolKind::Fn {
                    self.type_error_at(name_pos, "only a function can be called".to_string());
                }

                let mut param_idx = 0;

                if self.expr() {
                    let arg = self.last_retval.clone().unwrap();

                    if param_idx >= symbol.params.len() {
                        self.type_error_at(name_pos, "too many arguments in function call".to_string());
                    }

                    if !self.conv_to(&arg.typ, &symbol.params[param_idx].typ) {
                        self.type_error_at(name_pos, "in call, cannot convert the argument type to the parameter type".to_string());
                    }

                    param_idx += 1;

                    while self.consume(|code| matches!(code, TokenCode::COMMA)) {
                        if !self.expr() {
                            self.expected_error("expression after `,`");
                        }

                        let arg = self.last_retval.clone().unwrap();

                        if param_idx >= symbol.params.len() {
                            self.type_error_at(name_pos, "too many arguments in function call".to_string());
                        }

                        if !self.conv_to(&arg.typ, &symbol.params[param_idx].typ) {
                            self.type_error_at(name_pos, "in call, cannot convert the argument type to the parameter type".to_string());
                        }

                        param_idx += 1;
                    }
                }

                self.expect("`)` after function call arguments", |code| {
                    matches!(code, TokenCode::RPAR)
                });

                if param_idx < symbol.params.len() {
                    self.type_error_at(name_pos, "too few arguments in function call".to_string());
                }

                self.last_retval = Some(self.create_retval(symbol.typ, false, true));
            } else {
                if symbol.kind == SymbolKind::Fn {
                    self.type_error_at(name_pos, "a function can only be called".to_string());
                }

                let is_ct_val = symbol.typ.n >= 0;
                self.last_retval = Some(self.create_retval(symbol.typ, true, is_ct_val));
            }

            return true;
        }

        let token_pos = self.pos;

        if self.consume(|code| matches!(code, TokenCode::CT_INT(_))) {
            let value = match self.tokens[token_pos].code {
                TokenCode::CT_INT(value) => value,
                _ => 0,
            };
            let mut rv = self.create_retval(self.int_type(), false, true);
            rv.ct_val = Some(CtVal::Int(value));
            self.last_retval = Some(rv);
            return true;
        }

        if self.consume(|code| matches!(code, TokenCode::CT_REAL(_))) {
            let value = match self.tokens[token_pos].code {
                TokenCode::CT_REAL(value) => value,
                _ => 0.0,
            };
            let mut rv = self.create_retval(self.create_type(TypeBase::Double, -1), false, true);
            rv.ct_val = Some(CtVal::Double(value));
            self.last_retval = Some(rv);
            return true;
        }

        if self.consume(|code| matches!(code, TokenCode::CT_CHAR(_))) {
            let value = match self.tokens[token_pos].code {
                TokenCode::CT_CHAR(value) => value,
                _ => '\0',
            };
            let mut rv = self.create_retval(self.create_type(TypeBase::Char, -1), false, true);
            rv.ct_val = Some(CtVal::Char(value));
            self.last_retval = Some(rv);
            return true;
        }

        if self.consume(|code| matches!(code, TokenCode::CT_STRING(_))) {
            let value = match self.tokens[token_pos].code {
                TokenCode::CT_STRING(value) => value,
                _ => "",
            };
            let mut rv = self.create_retval(self.create_type(TypeBase::Char, 0), false, true);
            rv.ct_val = Some(CtVal::Str(value));
            self.last_retval = Some(rv);
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

#[cfg(test)]
mod type_analysis_tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    fn tk(code: TokenCode<'static>) -> Token<'static> {
        Token { code, line: 1, pos: 1 }
    }

    fn id(name: &'static str) -> Token<'static> {
        tk(TokenCode::ID(name))
    }

    fn i(value: i64) -> Token<'static> {
        tk(TokenCode::CT_INT(value))
    }

    fn d(value: f64) -> Token<'static> {
        tk(TokenCode::CT_REAL(value))
    }

    fn c(value: char) -> Token<'static> {
        tk(TokenCode::CT_CHAR(value))
    }

    fn s(value: &'static str) -> Token<'static> {
        tk(TokenCode::CT_STRING(value))
    }

    fn parses(tokens: Vec<Token<'static>>) {
        let mut analyser = Analyser::new(&tokens);
        analyser.parse();
    }

    fn parse_panics(tokens: Vec<Token<'static>>) -> String {
        let result = catch_unwind(AssertUnwindSafe(|| parses(tokens)));

        match result {
            Ok(_) => panic!("expected analyser to panic"),
            Err(payload) => {
                if let Some(message) = payload.downcast_ref::<String>() {
                    message.clone()
                } else if let Some(message) = payload.downcast_ref::<&str>() {
                    message.to_string()
                } else {
                    String::new()
                }
            }
        }
    }

    fn assert_type_error(tokens: Vec<Token<'static>>, expected: &str) {
        let message = parse_panics(tokens);
        assert!(message.contains("Type error"), "{}", message);
        assert!(message.contains(expected), "{}", message);
    }

    fn assert_domain_error(tokens: Vec<Token<'static>>, expected: &str) {
        let message = parse_panics(tokens);
        assert!(message.contains("Domain error"), "{}", message);
        assert!(message.contains(expected), "{}", message);
    }

    fn abs_function_tokens() -> Vec<Token<'static>> {
        vec![
            tk(TokenCode::DOUBLE), id("abs"), tk(TokenCode::LPAR), tk(TokenCode::DOUBLE), id("x"), tk(TokenCode::RPAR),
            tk(TokenCode::LACC),
            tk(TokenCode::IF), tk(TokenCode::LPAR), id("x"), tk(TokenCode::LESS), i(0), tk(TokenCode::RPAR),
            tk(TokenCode::RETURN), tk(TokenCode::SUB), id("x"), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RETURN), id("x"), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RACC),
        ]
    }

    fn main_prefix() -> Vec<Token<'static>> {
        vec![
            tk(TokenCode::INT), id("main"), tk(TokenCode::LPAR), tk(TokenCode::RPAR),
            tk(TokenCode::LACC),
        ]
    }

    fn main_suffix() -> Vec<Token<'static>> {
        vec![
            tk(TokenCode::RETURN), i(0), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RACC),
            tk(TokenCode::END),
        ]
    }

    fn program_with_main(body: Vec<Token<'static>>) -> Vec<Token<'static>> {
        let mut tokens = Vec::new();
        tokens.extend(main_prefix());
        tokens.extend(body);
        tokens.extend(main_suffix());
        tokens
    }

    fn program_with_abs_and_main(body: Vec<Token<'static>>) -> Vec<Token<'static>> {
        let mut tokens = abs_function_tokens();
        tokens.extend(main_prefix());
        tokens.extend(body);
        tokens.extend(main_suffix());
        tokens
    }

    #[test]
    fn accepts_valid_scalar_assignments_and_function_call() {
        parses(program_with_abs_and_main(vec![
            tk(TokenCode::INT), id("a"), tk(TokenCode::SEMICOLON),
            tk(TokenCode::DOUBLE), id("r"), tk(TokenCode::SEMICOLON),
            id("a"), tk(TokenCode::ASSIGN), i(1), tk(TokenCode::SEMICOLON),
            id("r"), tk(TokenCode::ASSIGN), id("abs"), tk(TokenCode::LPAR), d(0.5), tk(TokenCode::RPAR), tk(TokenCode::SEMICOLON),
            id("put_i"), tk(TokenCode::LPAR), id("a"), tk(TokenCode::RPAR), tk(TokenCode::SEMICOLON),
            id("put_d"), tk(TokenCode::LPAR), id("r"), tk(TokenCode::RPAR), tk(TokenCode::SEMICOLON),
        ]));
    }

    #[test]
    fn accepts_valid_arrays_struct_fields_and_char_strings() {
        let tokens = vec![
            tk(TokenCode::STRUCT), id("Point"), tk(TokenCode::LACC),
            tk(TokenCode::INT), id("x"), tk(TokenCode::SEMICOLON),
            tk(TokenCode::DOUBLE), id("y"), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RACC), tk(TokenCode::SEMICOLON),
            tk(TokenCode::VOID), id("set"), tk(TokenCode::LPAR), tk(TokenCode::CHAR), id("text"), tk(TokenCode::LBRACKET), tk(TokenCode::RBRACKET), tk(TokenCode::RPAR),
            tk(TokenCode::LACC),
            id("put_s"), tk(TokenCode::LPAR), id("text"), tk(TokenCode::RPAR), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RETURN), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RACC),
            tk(TokenCode::INT), id("main"), tk(TokenCode::LPAR), tk(TokenCode::RPAR),
            tk(TokenCode::LACC),
            tk(TokenCode::CHAR), id("text"), tk(TokenCode::LBRACKET), i(8), tk(TokenCode::RBRACKET), tk(TokenCode::SEMICOLON),
            tk(TokenCode::STRUCT), id("Point"), id("p"), tk(TokenCode::SEMICOLON),
            id("text"), tk(TokenCode::LBRACKET), i(0), tk(TokenCode::RBRACKET), tk(TokenCode::ASSIGN), c('a'), tk(TokenCode::SEMICOLON),
            id("p"), tk(TokenCode::DOT), id("x"), tk(TokenCode::ASSIGN), i(3), tk(TokenCode::SEMICOLON),
            id("set"), tk(TokenCode::LPAR), id("text"), tk(TokenCode::RPAR), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RETURN), i(0), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RACC),
            tk(TokenCode::END),
        ];

        parses(tokens);
    }

    #[test]
    fn rejects_assigning_to_rvalue() {
        assert_type_error(program_with_main(vec![
            i(2), tk(TokenCode::ASSIGN), i(3), tk(TokenCode::SEMICOLON),
        ]), "the assign destination must be a left-value");
    }

    #[test]
    fn rejects_assigning_to_function_name() {
        assert_type_error(program_with_abs_and_main(vec![
            id("abs"), tk(TokenCode::ASSIGN), i(0), tk(TokenCode::SEMICOLON),
        ]), "a function can only be called");
    }

    #[test]
    fn rejects_array_used_as_scalar_assignment_destination() {
        assert_type_error(program_with_main(vec![
            tk(TokenCode::INT), id("v"), tk(TokenCode::LBRACKET), i(10), tk(TokenCode::RBRACKET), tk(TokenCode::SEMICOLON),
            id("v"), tk(TokenCode::ASSIGN), i(1), tk(TokenCode::SEMICOLON),
        ]), "the assign destination cannot be constant");
    }

    #[test]
    fn rejects_too_many_function_arguments() {
        assert_type_error(program_with_abs_and_main(vec![
            tk(TokenCode::DOUBLE), id("r"), tk(TokenCode::SEMICOLON),
            id("r"), tk(TokenCode::ASSIGN), id("abs"), tk(TokenCode::LPAR), d(0.5), tk(TokenCode::COMMA), d(0.7), tk(TokenCode::RPAR), tk(TokenCode::SEMICOLON),
        ]), "too many arguments in function call");
    }

    #[test]
    fn rejects_too_few_function_arguments() {
        assert_type_error(program_with_abs_and_main(vec![
            tk(TokenCode::DOUBLE), id("r"), tk(TokenCode::SEMICOLON),
            id("r"), tk(TokenCode::ASSIGN), id("abs"), tk(TokenCode::LPAR), tk(TokenCode::RPAR), tk(TokenCode::SEMICOLON),
        ]), "too few arguments in function call");
    }

    #[test]
    fn rejects_calling_non_function() {
        assert_type_error(program_with_main(vec![
            tk(TokenCode::INT), id("a"), tk(TokenCode::SEMICOLON),
            id("a"), tk(TokenCode::LPAR), tk(TokenCode::RPAR), tk(TokenCode::SEMICOLON),
        ]), "only a function can be called");
    }

    #[test]
    fn rejects_indexing_non_array() {
        assert_type_error(program_with_main(vec![
            tk(TokenCode::INT), id("a"), tk(TokenCode::SEMICOLON),
            id("a"), tk(TokenCode::LBRACKET), i(0), tk(TokenCode::RBRACKET), tk(TokenCode::ASSIGN), i(1), tk(TokenCode::SEMICOLON),
        ]), "only an array can be indexed");
    }

    #[test]
    fn rejects_selecting_field_from_non_struct() {
        assert_type_error(program_with_main(vec![
            tk(TokenCode::INT), id("a"), tk(TokenCode::SEMICOLON),
            id("a"), tk(TokenCode::DOT), id("x"), tk(TokenCode::ASSIGN), i(1), tk(TokenCode::SEMICOLON),
        ]), "a field can only be selected from a struct");
    }

    #[test]
    fn rejects_missing_struct_field() {
        let tokens = vec![
            tk(TokenCode::STRUCT), id("Point"), tk(TokenCode::LACC),
            tk(TokenCode::INT), id("x"), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RACC), tk(TokenCode::SEMICOLON),
            tk(TokenCode::INT), id("main"), tk(TokenCode::LPAR), tk(TokenCode::RPAR),
            tk(TokenCode::LACC),
            tk(TokenCode::STRUCT), id("Point"), id("p"), tk(TokenCode::SEMICOLON),
            id("p"), tk(TokenCode::DOT), id("y"), tk(TokenCode::ASSIGN), i(1), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RETURN), i(0), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RACC),
            tk(TokenCode::END),
        ];

        assert_type_error(tokens, "does not have a field y");
    }

    #[test]
    fn rejects_array_operand_in_arithmetic_expression() {
        assert_type_error(program_with_main(vec![
            tk(TokenCode::INT), id("v"), tk(TokenCode::LBRACKET), i(10), tk(TokenCode::RBRACKET), tk(TokenCode::SEMICOLON),
            tk(TokenCode::INT), id("a"), tk(TokenCode::SEMICOLON),
            id("a"), tk(TokenCode::ASSIGN), id("v"), tk(TokenCode::ADD), i(1), tk(TokenCode::SEMICOLON),
        ]), "invalid array operand");
    }

    #[test]
    fn rejects_struct_operand_in_arithmetic_expression() {
        let tokens = vec![
            tk(TokenCode::STRUCT), id("Point"), tk(TokenCode::LACC),
            tk(TokenCode::INT), id("x"), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RACC), tk(TokenCode::SEMICOLON),
            tk(TokenCode::INT), id("main"), tk(TokenCode::LPAR), tk(TokenCode::RPAR),
            tk(TokenCode::LACC),
            tk(TokenCode::STRUCT), id("Point"), id("p"), tk(TokenCode::SEMICOLON),
            tk(TokenCode::INT), id("a"), tk(TokenCode::SEMICOLON),
            id("a"), tk(TokenCode::ASSIGN), id("p"), tk(TokenCode::ADD), i(1), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RETURN), i(0), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RACC),
            tk(TokenCode::END),
        ];

        assert_type_error(tokens, "invalid arithmetic operand type");
    }

    #[test]
    fn rejects_void_function_returning_value() {
        let tokens = vec![
            tk(TokenCode::VOID), id("f"), tk(TokenCode::LPAR), tk(TokenCode::RPAR),
            tk(TokenCode::LACC),
            tk(TokenCode::RETURN), i(1), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RACC),
            tk(TokenCode::END),
        ];

        assert_type_error(tokens, "a void function cannot return a value");
    }

    #[test]
    fn rejects_non_void_function_without_return_value() {
        let tokens = vec![
            tk(TokenCode::INT), id("f"), tk(TokenCode::LPAR), tk(TokenCode::RPAR),
            tk(TokenCode::LACC),
            tk(TokenCode::RETURN), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RACC),
            tk(TokenCode::END),
        ];

        assert_type_error(tokens, "a non-void function must return a value");
    }

    #[test]
    fn rejects_returning_array_value() {
        let tokens = vec![
            tk(TokenCode::INT), id("f"), tk(TokenCode::LPAR), tk(TokenCode::RPAR),
            tk(TokenCode::LACC),
            tk(TokenCode::INT), id("v"), tk(TokenCode::LBRACKET), i(4), tk(TokenCode::RBRACKET), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RETURN), id("v"), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RACC),
            tk(TokenCode::END),
        ];

        assert_type_error(tokens, "the return value must be a scalar value");
    }

    #[test]
    fn rejects_array_argument_passed_to_scalar_parameter() {
        assert_type_error(program_with_abs_and_main(vec![
            tk(TokenCode::INT), id("v"), tk(TokenCode::LBRACKET), i(10), tk(TokenCode::RBRACKET), tk(TokenCode::SEMICOLON),
            id("abs"), tk(TokenCode::LPAR), id("v"), tk(TokenCode::RPAR), tk(TokenCode::SEMICOLON),
        ]), "in call, cannot convert the argument type to the parameter type");
    }

    #[test]
    fn rejects_undefined_symbol() {
        assert_domain_error(program_with_main(vec![
            id("missing"), tk(TokenCode::ASSIGN), i(1), tk(TokenCode::SEMICOLON),
        ]), "undefined symbol: missing");
    }

    #[test]
    fn accepts_cast_between_scalar_numeric_types() {
        parses(program_with_main(vec![
            tk(TokenCode::INT), id("a"), tk(TokenCode::SEMICOLON),
            id("a"), tk(TokenCode::ASSIGN), d(1.5), tk(TokenCode::SEMICOLON),
        ]));
    }

    #[test]
    fn rejects_casting_struct_to_int() {
        let tokens = vec![
            tk(TokenCode::STRUCT), id("Point"), tk(TokenCode::LACC),
            tk(TokenCode::INT), id("x"), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RACC), tk(TokenCode::SEMICOLON),
            tk(TokenCode::INT), id("main"), tk(TokenCode::LPAR), tk(TokenCode::RPAR),
            tk(TokenCode::LACC),
            tk(TokenCode::STRUCT), id("Point"), id("p"), tk(TokenCode::SEMICOLON),
            tk(TokenCode::INT), id("a"), tk(TokenCode::SEMICOLON),
            id("a"), tk(TokenCode::ASSIGN), id("p"), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RETURN), i(0), tk(TokenCode::SEMICOLON),
            tk(TokenCode::RACC),
            tk(TokenCode::END),
        ];

        assert_type_error(tokens, "source must be scalar");
    }

    #[test]
    fn accepts_predefined_zero_argument_functions() {
        parses(program_with_main(vec![
            tk(TokenCode::INT), id("a"), tk(TokenCode::SEMICOLON),
            tk(TokenCode::DOUBLE), id("seconds_value"), tk(TokenCode::SEMICOLON),
            id("a"), tk(TokenCode::ASSIGN), id("get_i"), tk(TokenCode::LPAR), tk(TokenCode::RPAR), tk(TokenCode::SEMICOLON),
            id("seconds_value"), tk(TokenCode::ASSIGN), id("seconds"), tk(TokenCode::LPAR), tk(TokenCode::RPAR), tk(TokenCode::SEMICOLON),
        ]));
    }
}
