//! Lower validated AtomC tokens into the existing stack VM instruction set.
use crate::analyser::Analyser;
use crate::builtins;
use crate::layout::{self, Structs, Ty};
use crate::lexer::{Token, TokenCode as T};
use crate::modules;
use crate::vm::{ExtFunc, InstrArg, Instructions, Opcode as O, Vm, GLOBAL_SIZE};
use std::collections::BTreeMap;
use std::path::Path;

type Result<T> = std::result::Result<T, String>;
#[derive(Debug, Clone)]
enum Expr {
    Int(i32),
    Double(f64),
    Char(u8),
    Str(String),
    Var(String),
    Call(String, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Field(Box<Expr>, String),
    Unary(bool, Box<Expr>),
    Binary(u8, Box<Expr>, Box<Expr>),
    Assign(Box<Expr>, Box<Expr>),
    Cast(Ty, Box<Expr>),
}
#[derive(Debug)]
enum Stmt {
    Block(Vec<Stmt>),
    Decl(String, Ty),
    Expr(Expr),
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    Loop(Option<Expr>, Option<Expr>, Option<Expr>, Box<Stmt>),
    Break,
    Return(Option<Expr>),
    Empty,
}
struct Function {
    location: String,
    name: String,
    ret: Ty,
    params: Vec<(String, Ty)>,
    body: Stmt,
}
struct Syntax<'a> {
    tokens: &'a [Token<'a>],
    pos: usize,
}
impl<'a> Syntax<'a> {
    fn current(&self) -> T<'a> {
        self.tokens[self.pos].code.clone()
    }
    fn eat(&mut self, t: T<'a>) -> bool {
        if self.current() == t {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, t: T<'a>) -> Result<()> {
        if self.eat(t) {
            Ok(())
        } else {
            Err(format!(
                "{}: lowering: unexpected token",
                self.tokens[self.pos].location()
            ))
        }
    }
    fn name(&mut self) -> Result<String> {
        if let T::ID(n) = self.current() {
            self.pos += 1;
            Ok(n.into())
        } else {
            Err("expected name".into())
        }
    }
    fn is_type(&self) -> bool {
        matches!(
            self.current(),
            T::INT | T::DOUBLE | T::CHAR | T::VOID | T::STRUCT
        )
    }
    fn ty(&mut self) -> Result<Ty> {
        let ty = match self.current() {
            T::INT => Ty::Int,
            T::DOUBLE => Ty::Double,
            T::CHAR => Ty::Char,
            T::VOID => Ty::Void,
            T::STRUCT => {
                self.pos += 1;
                return Ok(Ty::Struct(self.name()?));
            }
            _ => return Err("expected type".into()),
        };
        self.pos += 1;
        Ok(ty)
    }
    fn array(&mut self, ty: Ty) -> Result<Ty> {
        if !self.eat(T::LBRACKET) {
            return Ok(ty);
        }
        let n = if let T::CT_INT(n) = self.current() {
            self.pos += 1;
            usize::try_from(n).map_err(|_| "invalid array dimension")?
        } else {
            0
        };
        self.expect(T::RBRACKET)?;
        Ok(Ty::Array(Box::new(ty), n))
    }
    fn stmt(&mut self) -> Result<Stmt> {
        if self.eat(T::LACC) {
            let mut stmts = Vec::new();
            while !self.eat(T::RACC) {
                stmts.push(self.stmt()?);
            }
            return Ok(Stmt::Block(stmts));
        }
        if self.is_type() {
            let ty = self.ty()?;
            let name = self.name()?;
            let ty = self.array(ty)?;
            self.expect(T::SEMICOLON)?;
            return Ok(Stmt::Decl(name, ty));
        }
        if self.eat(T::IF) {
            self.expect(T::LPAR)?;
            let e = self.expr(0)?;
            self.expect(T::RPAR)?;
            let yes = Box::new(self.stmt()?);
            let no = if self.eat(T::ELSE) {
                Some(Box::new(self.stmt()?))
            } else {
                None
            };
            return Ok(Stmt::If(e, yes, no));
        }
        if self.eat(T::WHILE) {
            self.expect(T::LPAR)?;
            let e = self.expr(0)?;
            self.expect(T::RPAR)?;
            return Ok(Stmt::Loop(None, Some(e), None, Box::new(self.stmt()?)));
        }
        if self.eat(T::FOR) {
            self.expect(T::LPAR)?;
            let init = self.optional(T::SEMICOLON)?;
            let cond = self.optional(T::SEMICOLON)?;
            let step = self.optional(T::RPAR)?;
            return Ok(Stmt::Loop(init, cond, step, Box::new(self.stmt()?)));
        }
        if self.eat(T::RETURN) {
            return Ok(Stmt::Return(self.optional(T::SEMICOLON)?));
        }
        if self.eat(T::BREAK) {
            self.expect(T::SEMICOLON)?;
            return Ok(Stmt::Break);
        }
        if self.eat(T::SEMICOLON) {
            return Ok(Stmt::Empty);
        }
        let e = self.expr(0)?;
        self.expect(T::SEMICOLON)?;
        Ok(Stmt::Expr(e))
    }
    fn optional(&mut self, end: T<'a>) -> Result<Option<Expr>> {
        if self.eat(end.clone()) {
            return Ok(None);
        }
        let e = self.expr(0)?;
        self.expect(end)?;
        Ok(Some(e))
    }
    // Precedence climbing; assignment is right associative.
    fn expr(&mut self, min: u8) -> Result<Expr> {
        let mut lhs = self.primary()?;
        loop {
            let (prec, op) = match self.current() {
                T::ASSIGN => (1, 0),
                T::OR => (2, 1),
                T::AND => (3, 2),
                T::EQUAL => (4, 3),
                T::NOTEQ => (4, 4),
                T::LESS => (5, 5),
                T::LESSEQ => (5, 6),
                T::GREATER => (5, 7),
                T::GREATEREQ => (5, 8),
                T::ADD => (6, 9),
                T::SUB => (6, 10),
                T::MUL => (7, 11),
                T::DIV => (7, 12),
                _ => break,
            };
            if prec < min {
                break;
            }
            self.pos += 1;
            let rhs = self.expr(if op == 0 { prec } else { prec + 1 })?;
            lhs = if op == 0 {
                Expr::Assign(Box::new(lhs), Box::new(rhs))
            } else {
                Expr::Binary(op, Box::new(lhs), Box::new(rhs))
            };
        }
        Ok(lhs)
    }
    fn primary(&mut self) -> Result<Expr> {
        let token = self.current();
        self.pos += 1;
        let mut e = match token {
            T::CT_INT(n) => Expr::Int(i32::try_from(n).map_err(|_| "integer literal exceeds i32")?),
            T::CT_REAL(n) => Expr::Double(n),
            T::CT_CHAR(c) => Expr::Char(u8::try_from(c as u32).map_err(|_| "char exceeds byte")?),
            T::CT_STRING(s) => Expr::Str(unescape(s)?),
            T::ID(n) => {
                if self.eat(T::LPAR) {
                    let mut args = Vec::new();
                    if !self.eat(T::RPAR) {
                        loop {
                            args.push(self.expr(0)?);
                            if self.eat(T::RPAR) {
                                break;
                            }
                            self.expect(T::COMMA)?;
                        }
                    }
                    Expr::Call(n.into(), args)
                } else {
                    Expr::Var(n.into())
                }
            }
            T::SUB if self.current() == T::CT_INT(2147483648) => {
                self.pos += 1;
                Expr::Int(i32::MIN)
            }
            T::SUB | T::NOT => Expr::Unary(token == T::NOT, Box::new(self.primary()?)),
            T::LPAR => {
                if self.is_type() {
                    let ty = self.ty()?;
                    let ty = self.array(ty)?;
                    self.expect(T::RPAR)?;
                    Expr::Cast(ty, Box::new(self.primary()?))
                } else {
                    let e = self.expr(0)?;
                    self.expect(T::RPAR)?;
                    e
                }
            }
            _ => return Err("expected expression".into()),
        };
        loop {
            if self.eat(T::LBRACKET) {
                let idx = self.expr(0)?;
                self.expect(T::RBRACKET)?;
                e = Expr::Index(Box::new(e), Box::new(idx));
            } else if self.eat(T::DOT) {
                e = Expr::Field(Box::new(e), self.name()?);
            } else {
                break;
            }
        }
        Ok(e)
    }
}
fn unescape(s: &str) -> Result<String> {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        out.push(if c == '\\' {
            match chars.next().ok_or("incomplete escape")? {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '0' => '\0',
                'a' => '\x07',
                'b' => '\x08',
                'f' => '\x0c',
                'v' => '\x0b',
                '\\' => '\\',
                '\'' => '\'',
                '"' => '"',
                _ => return Err("unsupported escape".into()),
            }
        } else {
            c
        });
    }
    Ok(out)
}

#[derive(Clone)]
struct Binding {
    ty: Ty,
    offset: i64,
    kind: O,
    indirect: bool,
}
#[derive(Clone)]
struct Signature {
    ret: Ty,
    params: Vec<Ty>,
    target: Option<usize>,
    ext: Option<ExtFunc>,
}
pub struct Program {
    pub instructions: Instructions,
    pub globals: Vec<u8>,
}
impl Program {
    pub fn into_vm(self, input: &str) -> Result<Vm> {
        let mut vm = Vm::with_input(input);
        vm.checked = true;
        vm.alloc_global(self.globals.len())
            .map_err(|e| e.to_string())?;
        vm.write_memory(0, &self.globals)
            .map_err(|e| e.to_string())?;
        vm.instructions = self.instructions;
        Ok(vm)
    }
}
struct Generator {
    code: Instructions,
    globals: Vec<u8>,
    structs: Structs,
    scopes: Vec<BTreeMap<String, Binding>>,
    signatures: BTreeMap<String, Signature>,
    local_size: usize,
    args_size: usize,
    ret: Ty,
    breaks: Vec<Vec<usize>>,
}
impl Generator {
    fn emit(&mut self, op: O) -> usize {
        self.code.add_instr(op)
    }
    fn int(&mut self, op: O, n: usize) -> usize {
        self.code.add_instr_i(op, n as i64)
    }
    fn pair(&mut self, op: O, a: i64, b: usize) -> usize {
        self.code.add_instr_ii(op, a, b as i64)
    }
    fn label(&mut self) -> usize {
        self.emit(O::Nop)
    }
    fn patch(&mut self, at: usize, to: usize) {
        self.code.get_mut(at).unwrap().args[0] = InstrArg::Instr(to);
    }
    fn size(&self, ty: &Ty) -> Result<usize> {
        if matches!(ty, Ty::Array(_, 0)) {
            Ok(8)
        } else if *ty == Ty::Void {
            Ok(0)
        } else {
            Ok(layout::layout(ty, &self.structs)?.size)
        }
    }
    fn binding(&self, name: &str) -> Result<Binding> {
        self.scopes
            .iter()
            .rev()
            .find_map(|s| s.get(name))
            .cloned()
            .ok_or(format!("unknown variable {name}"))
    }
    fn field(&self, ty: &Ty, name: &str) -> Result<layout::Field> {
        if let Ty::Struct(n) = ty {
            self.structs
                .get(n)
                .and_then(|s| s.fields.iter().find(|f| f.name == name))
                .cloned()
                .ok_or("unknown struct field".into())
        } else {
            Err("field access requires a struct object".into())
        }
    }
    fn ty(&self, e: &Expr) -> Result<Ty> {
        Ok(match e {
            Expr::Int(_) => Ty::Int,
            Expr::Double(_) => Ty::Double,
            Expr::Char(_) => Ty::Char,
            Expr::Str(s) => Ty::Array(Box::new(Ty::Char), s.len() + 1),
            Expr::Var(n) => self.binding(n)?.ty,
            Expr::Call(n, _) => self
                .signatures
                .get(n)
                .ok_or("unknown function")?
                .ret
                .clone(),
            Expr::Index(a, _) => {
                if let Ty::Array(t, _) = self.ty(a)? {
                    *t
                } else {
                    return Err("index requires array".into());
                }
            }
            Expr::Field(a, n) => self.field(&self.ty(a)?, n)?.ty,
            Expr::Unary(not, a) => {
                if *not {
                    Ty::Int
                } else {
                    self.ty(a)?
                }
            }
            Expr::Binary(op, a, b) => {
                if *op <= 8 {
                    Ty::Int
                } else {
                    arithmetic(&self.ty(a)?, &self.ty(b)?)?
                }
            }
            Expr::Assign(a, _) => self.ty(a)?,
            Expr::Cast(t, _) => t.clone(),
        })
    }
    fn address(&mut self, e: &Expr) -> Result<Ty> {
        let ty = self.ty(e)?;
        match e {
            Expr::Var(n) => {
                let b = self.binding(n)?;
                self.pair(
                    b.kind,
                    b.offset,
                    if b.indirect { 8 } else { self.size(&b.ty)? },
                );
                if b.indirect {
                    self.int(O::Load, 8);
                }
            }
            Expr::Index(a, i) => {
                let parent = self.expr(a)?;
                if !matches!(parent, Ty::Array(_, _)) {
                    return Err("index requires array".into());
                }
                let it = self.expr(i)?;
                self.cast(&it, &Ty::Int)?;
                self.int(O::Index, self.size(&ty)?);
            }
            Expr::Field(a, n) => {
                let at = self.address(a)?;
                let f = self.field(&at, n)?;
                self.pair(O::Field, f.offset as i64, self.size(&ty)?);
            }
            _ => return Err("expression has no storage".into()),
        }
        Ok(ty)
    }
    fn cast(&mut self, from: &Ty, to: &Ty) -> Result<()> {
        if from == to || matches!((from,to), (Ty::Array(a,_),Ty::Array(b,_)) if a == b) {
            return Ok(());
        }
        let op = match (from, to) {
            (Ty::Char, Ty::Int) => O::CastCI,
            (Ty::Char, Ty::Double) => O::CastCD,
            (Ty::Int, Ty::Char) => O::CastIC,
            (Ty::Int, Ty::Double) => O::CastID,
            (Ty::Double, Ty::Char) => O::CastDC,
            (Ty::Double, Ty::Int) => O::CastDI,
            _ => return Err("unsupported conversion".into()),
        };
        self.emit(op);
        Ok(())
    }
    fn boolean(&mut self, e: &Expr) -> Result<()> {
        let t = self.expr(e)?;
        self.emit(match t {
            Ty::Char => O::NotC,
            Ty::Int => O::NotI,
            Ty::Double => O::NotD,
            _ => return Err("scalar condition required".into()),
        });
        self.emit(O::NotI);
        Ok(())
    }
    fn expr(&mut self, e: &Expr) -> Result<Ty> {
        let ty = self.ty(e)?;
        match e {
            Expr::Int(n) => {
                self.code.add_instr_i(O::PushCtI, *n as i64);
            }
            Expr::Double(n) => {
                self.code.add_instr_d(O::PushCtD, *n);
            }
            Expr::Char(n) => {
                self.int(O::PushCtC, *n as usize);
            }
            Expr::Str(s) => {
                let offset = self.globals.len();
                self.globals.extend(s.bytes());
                self.globals.push(0);
                if self.globals.len() > GLOBAL_SIZE {
                    return Err("globals exceed guest memory".into());
                }
                self.pair(O::ReadOnlyRef, offset as i64, s.len() + 1);
            }
            Expr::Var(_) | Expr::Index(_, _) | Expr::Field(_, _) => {
                self.address(e)?;
                if !matches!(ty, Ty::Array(_, _)) {
                    self.int(O::Load, self.size(&ty)?);
                }
            }
            Expr::Assign(a, b) => {
                if matches!(ty, Ty::Array(_, _) | Ty::Void) {
                    return Err("assignment requires an object value".into());
                }
                self.address(a)?;
                let bt = self.expr(b)?;
                self.cast(&bt, &ty)?;
                let size = self.size(&ty)?;
                self.pair(O::Insert, (8 + size) as i64, size);
                self.int(O::Store, size);
            }
            Expr::Cast(t, a) => {
                let from = self.expr(a)?;
                self.cast(&from, t)?;
            }
            Expr::Unary(not, a) => {
                let t = self.expr(a)?;
                self.emit(match (not, t) {
                    (true, Ty::Char) => O::NotC,
                    (true, Ty::Int) => O::NotI,
                    (true, Ty::Double) => O::NotD,
                    (false, Ty::Char) => O::NegC,
                    (false, Ty::Int) => O::NegI,
                    (false, Ty::Double) => O::NegD,
                    _ => return Err("numeric operand required".into()),
                });
            }
            Expr::Binary(op, a, b) if *op <= 2 => {
                self.boolean(a)?;
                let branch = self.emit(if *op == 1 { O::JtI } else { O::JfI });
                self.boolean(b)?;
                let end = self.emit(O::Jmp);
                let constant = self.label();
                self.int(O::PushCtI, usize::from(*op == 1));
                let done = self.label();
                self.patch(branch, constant);
                self.patch(end, done);
            }
            Expr::Binary(op, a, b) => {
                let at = self.ty(a)?;
                let bt = self.ty(b)?;
                if matches!((&at, &bt), (Ty::Array(_, _), Ty::Array(_, _)))
                    && (*op == 3 || *op == 4)
                {
                    self.expr(a)?;
                    self.expr(b)?;
                    self.emit(if *op == 3 { O::EqA } else { O::NeqA });
                } else {
                    let common = arithmetic(&at, &bt)?;
                    let from = self.expr(a)?;
                    self.cast(&from, &common)?;
                    let from = self.expr(b)?;
                    self.cast(&from, &common)?;
                    let ops = match common {
                        Ty::Char => [
                            O::EqC,
                            O::NeqC,
                            O::LessC,
                            O::LessEqC,
                            O::GreaterC,
                            O::GreaterEqC,
                            O::AddC,
                            O::SubC,
                            O::MulC,
                            O::DivC,
                        ],
                        Ty::Int => [
                            O::EqI,
                            O::NeqI,
                            O::LessI,
                            O::LessEqI,
                            O::GreaterI,
                            O::GreaterEqI,
                            O::AddI,
                            O::SubI,
                            O::MulI,
                            O::DivI,
                        ],
                        Ty::Double => [
                            O::EqD,
                            O::NeqD,
                            O::LessD,
                            O::LessEqD,
                            O::GreaterD,
                            O::GreaterEqD,
                            O::AddD,
                            O::SubD,
                            O::MulD,
                            O::DivD,
                        ],
                        _ => return Err("numeric operands required".into()),
                    };
                    self.emit(ops[(*op - 3) as usize]);
                }
            }
            Expr::Call(n, args) => {
                let sig = self.signatures.get(n).cloned().ok_or("unknown function")?;
                if args.len() != sig.params.len() {
                    return Err("argument count mismatch".into());
                }
                for (arg, param) in args.iter().zip(&sig.params) {
                    let from = self.expr(arg)?;
                    self.cast(&from, param)?;
                }
                if let Some(ext) = sig.ext {
                    self.code.add_instr_ext(ext);
                } else {
                    self.code
                        .add_instr_target(O::Call, sig.target.ok_or("unresolved function")?);
                }
            }
        }
        Ok(ty)
    }
    fn discard(&mut self, e: &Expr) -> Result<()> {
        let t = self.expr(e)?;
        let size = if matches!(t, Ty::Array(_, _)) {
            8
        } else {
            self.size(&t)?
        };
        if size != 0 {
            self.int(O::Drop, size);
        }
        Ok(())
    }
    fn stmt(&mut self, s: &Stmt) -> Result<()> {
        match s {
            Stmt::Block(stmts) => {
                self.scopes.push(BTreeMap::new());
                for s in stmts {
                    self.stmt(s)?;
                }
                self.scopes.pop();
            }
            Stmt::Decl(n, t) => {
                let l = layout::layout(t, &self.structs)?;
                let offset = layout::align_up(self.local_size, l.align)?;
                self.local_size = offset
                    .checked_add(l.size)
                    .filter(|n| *n <= layout::MAX_OBJECT)
                    .ok_or("locals exceed stack limit")?;
                self.scopes.last_mut().unwrap().insert(
                    n.clone(),
                    Binding {
                        ty: t.clone(),
                        offset: offset as i64,
                        kind: O::LocalRef,
                        indirect: false,
                    },
                );
            }
            Stmt::Expr(e) => self.discard(e)?,
            Stmt::Empty => (),
            Stmt::If(e, yes, no) => {
                self.boolean(e)?;
                let skip = self.emit(O::JfI);
                self.stmt(yes)?;
                let end = self.emit(O::Jmp);
                let other = self.label();
                self.patch(skip, other);
                if let Some(no) = no {
                    self.stmt(no)?;
                }
                let done = self.label();
                self.patch(end, done);
            }
            Stmt::Loop(init, cond, step, body) => {
                if let Some(e) = init {
                    self.discard(e)?;
                }
                let head = self.label();
                let exit = if let Some(e) = cond {
                    self.boolean(e)?;
                    Some(self.emit(O::JfI))
                } else {
                    None
                };
                self.breaks.push(Vec::new());
                self.stmt(body)?;
                if let Some(e) = step {
                    self.discard(e)?;
                }
                self.code.add_instr_target(O::Jmp, head);
                let end = self.label();
                if let Some(exit) = exit {
                    self.patch(exit, end);
                }
                for at in self.breaks.pop().unwrap() {
                    self.patch(at, end);
                }
            }
            Stmt::Break => {
                if self.breaks.is_empty() {
                    return Err("break outside loop".into());
                }
                let at = self.emit(O::Jmp);
                self.breaks.last_mut().unwrap().push(at);
            }
            Stmt::Return(e) => {
                let ret = self.ret.clone();
                if let Some(e) = e {
                    let from = self.expr(e)?;
                    self.cast(&from, &ret)?;
                }
                self.pair(O::Ret, self.args_size as i64, self.size(&ret)?);
            }
        }
        Ok(())
    }
}
fn arithmetic(a: &Ty, b: &Ty) -> Result<Ty> {
    if !matches!(a, Ty::Char | Ty::Int | Ty::Double)
        || !matches!(b, Ty::Char | Ty::Int | Ty::Double)
    {
        return Err("numeric operands required".into());
    }
    Ok(if *a == Ty::Double || *b == Ty::Double {
        Ty::Double
    } else if *a == Ty::Int || *b == Ty::Int {
        Ty::Int
    } else {
        Ty::Char
    })
}

pub fn compile(source: &str) -> Result<Program> {
    compile_named(source, "<source>")
}

fn compile_named(source: &str, name: &str) -> Result<Program> {
    if source.len() > modules::MAX_SOURCE_BYTES {
        return Err(format!("{name}: source exceeds 1 MiB"));
    }
    let tokens = modules::lex(source, name)?;
    if let Some(import) = tokens
        .windows(2)
        .find(|pair| pair[0].code == T::IMPORT && matches!(pair[1].code, T::CT_STRING(_)))
    {
        return Err(format!(
            "{}: source imports require a file compilation entry point",
            import[0].location()
        ));
    }
    crate::diagnostic::catch(|| compile_tokens(&tokens, name))
}

/// Resolve source dependencies at compile time, then produce self-contained bytecode.
pub fn compile_file(path: &Path) -> Result<Program> {
    let files = modules::load(path)?;
    let entry = files.last().ok_or("missing entry file")?;
    let mut imports = Vec::new();
    let mut bodies = Vec::new();
    for file in &files {
        let tokens = file.tokens()?;
        for import in tokens[..file.body_start].chunks_exact(3) {
            if matches!(import[1].code, T::ID(_)) {
                imports.extend_from_slice(import);
            }
        }
        let mut body: Vec<_> = tokens.into_iter().skip(file.body_start).collect();
        // Only top-level parsers may cross a file boundary. Incomplete syntax
        // cannot consume the next file's declarations.
        body.last_mut().unwrap().code = T::MODULE_END;
        bodies.extend(body);
    }
    imports.extend(bodies);
    imports.push(Token {
        code: T::END,
        source: &entry.name,
        line: entry.text.lines().count().max(1),
        pos: 1,
    });
    crate::diagnostic::catch(|| compile_tokens(&imports, &entry.name))
}

fn compile_tokens(tokens: &[Token<'_>], entry: &str) -> Result<Program> {
    let mut depth = 0usize;
    let mut segment = 0;
    for token in tokens {
        if token.code == T::MODULE_END {
            depth = 0;
            segment = 0;
            continue;
        }
        segment += 1;
        match token.code {
            T::LACC | T::LPAR | T::LBRACKET => {
                depth += 1;
                if depth > 64 {
                    return Err(format!("{}: source nesting exceeds 64", token.location()));
                }
            }
            T::RACC | T::RPAR | T::RBRACKET => {
                depth = depth.saturating_sub(1);
            }
            _ => (),
        }
        if matches!(token.code, T::SEMICOLON | T::LACC | T::RACC) {
            segment = 0;
        }
        if segment > 256 {
            return Err(format!(
                "{}: expression exceeds 256 tokens",
                token.location()
            ));
        }
    }
    let mut analyser = Analyser::new(tokens);
    analyser.parse();
    let mut parser = Syntax { tokens, pos: 0 };
    let mut g = Generator {
        code: Instructions::new(),
        globals: Vec::new(),
        structs: Structs::new(),
        scopes: vec![BTreeMap::new()],
        signatures: BTreeMap::new(),
        local_size: 0,
        args_size: 0,
        ret: Ty::Void,
        breaks: Vec::new(),
    };
    for builtin in builtins::ALL
        .iter()
        .filter(|b| analyser.libraries().contains(&b.library))
    {
        g.signatures.insert(
            builtin.name.into(),
            Signature {
                ret: builtin.result.ty(),
                params: builtin.params.iter().map(|p| p.ty()).collect(),
                target: None,
                ext: Some(builtin.function),
            },
        );
    }
    while parser.eat(T::IMPORT) {
        parser.name()?;
        parser.expect(T::SEMICOLON)?;
    }
    let mut functions = Vec::new();
    while parser.current() != T::END {
        if parser.eat(T::MODULE_END) {
            continue;
        }
        let origin = parser.tokens[parser.pos].clone();
        let location = origin.location();
        let ty = parser.ty()?;
        if let Ty::Struct(name) = &ty {
            if parser.eat(T::LACC) {
                let mut fields = Vec::new();
                while !parser.eat(T::RACC) {
                    let t = parser.ty()?;
                    let n = parser.name()?;
                    let t = parser.array(t)?;
                    parser.expect(T::SEMICOLON)?;
                    fields.push((n, t));
                }
                parser.expect(T::SEMICOLON)?;
                let definition = layout::structure(fields, &g.structs)?;
                g.structs.insert(name.clone(), definition);
                continue;
            }
        }
        let name = parser.name()?;
        if name == "main" && origin.source != entry {
            return Err(format!("{location}: only the entry file may define main"));
        }
        if parser.eat(T::LPAR) {
            let mut params = Vec::new();
            if !parser.eat(T::RPAR) {
                loop {
                    let t = parser.ty()?;
                    let n = parser.name()?;
                    let t = parser.array(t)?;
                    let t = if let Ty::Array(e, _) = t {
                        Ty::Array(e, 0)
                    } else {
                        t
                    };
                    params.push((n, t));
                    if parser.eat(T::RPAR) {
                        break;
                    }
                    parser.expect(T::COMMA)?;
                }
            }
            let body = parser.stmt().map_err(|e| format!("{location}: {e}"))?;
            functions.push(Function {
                location,
                name,
                ret: ty,
                params,
                body,
            });
        } else {
            let ty = parser.array(ty)?;
            parser.expect(T::SEMICOLON)?;
            let l = layout::layout(&ty, &g.structs)?;
            let offset = layout::align_up(g.globals.len(), l.align)?;
            let end = offset
                .checked_add(l.size)
                .filter(|n| *n <= GLOBAL_SIZE)
                .ok_or("globals exceed guest memory")?;
            g.globals.resize(end, 0);
            g.scopes[0].insert(
                name,
                Binding {
                    ty,
                    offset: offset as i64,
                    kind: O::GlobalRef,
                    indirect: false,
                },
            );
        }
    }
    let entry = g.emit(O::Call);
    let entry_drop = g.emit(O::Nop);
    g.emit(O::Halt);
    for f in functions {
        let target = g.int(O::Enter, 0);
        g.signatures.insert(
            f.name.clone(),
            Signature {
                ret: f.ret.clone(),
                params: f.params.iter().map(|(_, t)| t.clone()).collect(),
                target: Some(target),
                ext: None,
            },
        );
        g.ret = f.ret.clone();
        g.local_size = 0;
        g.args_size = f.params.iter().try_fold(0usize, |total, (_, t)| {
            total
                .checked_add(g.size(t)?)
                .filter(|n| *n <= layout::MAX_OBJECT - 16)
                .ok_or("function arguments exceed guest stack limit".to_string())
        })?;
        g.scopes.push(BTreeMap::new());
        let mut offset = -(16 + g.args_size as i64);
        for (n, t) in f.params {
            let size = g.size(&t)?;
            let indirect = matches!(t, Ty::Array(_, _));
            g.scopes.last_mut().unwrap().insert(
                n,
                Binding {
                    ty: t,
                    offset,
                    kind: O::ArgRef,
                    indirect,
                },
            );
            offset += size as i64;
        }
        g.stmt(&f.body)
            .map_err(|e| format!("{}: {e}", f.location))?;
        if f.ret == Ty::Void {
            g.pair(O::Ret, g.args_size as i64, 0);
        } else {
            // A missing non-void return traps instead of consuming uninitialized stack bytes.
            g.emit(O::Trap);
        }
        g.code.get_mut(target).unwrap().args[0] = InstrArg::Int(g.local_size as i64);
        g.scopes.pop();
    }
    let main = g
        .signatures
        .get("main")
        .ok_or("program requires main()")?
        .clone();
    if !main.params.is_empty() || !matches!(main.ret, Ty::Void | Ty::Int) {
        return Err("main must be int main() or void main()".into());
    }
    g.patch(entry, main.target.ok_or("missing main")?);
    if main.ret == Ty::Int {
        let i = g.code.get_mut(entry_drop).unwrap();
        i.opcode = O::Drop;
        i.args[0] = InstrArg::Int(4);
    }
    Ok(Program {
        instructions: g.code,
        globals: g.globals,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(s: &str, input: &str) -> Result<String> {
        let mut vm = compile(s)?.into_vm(input)?;
        vm.run().map_err(|e| e.to_string())?;
        assert_eq!(vm.stack_len(), 0);
        Ok(vm.take_output())
    }
    #[test]
    fn recursive_calls_conversions_and_io() {
        assert_eq!(run("import io; int fact(int n){if(n<2)return 1;return n*fact(n-1);} int main(){put_i(fact(get_i()));put_d(2+0.5);return 0;}","5").unwrap(),"120\n2.5\n");
    }
    #[test]
    fn arrays_structs_loops_and_reference_parameters() {
        let src = "import io; struct P{char c;double d;int x[2];}; int sum(int a[],int n){int i;int s;s=0;for(i=0;i<n;i=i+1){s=s+a[i];}return s;} int main(){struct P p;int a[2];a[0]=3;a[1]=4;p.x[0]=sum(a,2);p.d=2.5;put_i(p.x[0]);put_d(p.d);return 0;}";
        assert_eq!(run(src, "").unwrap(), "7\n2.5\n");
    }
    #[test]
    fn short_circuit_and_break() {
        assert_eq!(run("import io; int main(){int x;x=0;if(0 && (x=1))x=2;if(1 || (x=3))put_i(x);while(1){break;}return 0;}","").unwrap(),"0\n");
    }
    #[test]
    fn bounds_survive_parameter_passing() {
        let s = "void f(int a[]){a[2]=9;} int main(){int a[2];int b;f(a);return 0;}";
        assert!(run(s, "").unwrap_err().contains("bounds"));
        assert!(
            run("import io; int main(){int a[2];put_i(a[-1]);return 0;}", "")
                .unwrap_err()
                .contains("bounds")
        );
    }
    #[test]
    fn rejects_invalid_entry_break_and_recursive_layout() {
        for s in [
            "int f(){return 1;}",
            "int main(){break;return 0;}",
            "struct S{struct S s;};int main(){return 0;}",
        ] {
            assert!(compile(s).is_err());
        }
    }
    #[test]
    fn numeric_edges_and_escaped_literals() {
        let src = r#"import io; int main(){char c;c=300;put_i(c);put_i(-2147483648/-1);put_c('\n');put_s("quoted: \"ok\"");return 0;}"#;
        assert_eq!(run(src, "").unwrap(), "44\n-2147483648\n\nquoted: \"ok\"\n");
    }
    #[test]
    fn struct_parameters_are_copied_and_inner_arrays_are_bounded() {
        let src = "import io; struct S{int x[2];int y;};void f(struct S s){s.x[0]=99;}int main(){struct S s;s.x[0]=4;f(s);put_i(s.x[0]);return 0;}";
        assert_eq!(run(src, "").unwrap(), "4\n");
        assert!(run(
            "struct S{int x[2];int y;};int main(){struct S s;s.x[2]=5;return 0;}",
            ""
        )
        .unwrap_err()
        .contains("bounds"));
    }
    #[test]
    fn source_depth_and_size_limits_are_errors() {
        let src = format!(
            "int main(){{return {}1{};}}",
            "(".repeat(60),
            ")".repeat(60)
        );
        run(&src, "").unwrap();
        let src = format!(
            "int main(){{return {}1{};}}",
            "(".repeat(1000),
            ")".repeat(1000)
        );
        assert!(compile(&src).is_err());
        assert!(compile(&" ".repeat(1024 * 1024 + 1)).is_err());
        let chain = format!("int main(){{{}return 0;}}", "if(1);else ".repeat(1000));
        assert!(compile(&chain).is_err());
        assert!(compile("struct Big{char data[20000];};void f(struct Big a,struct Big b){}int main(){return 0;}").is_err());
    }

    #[test]
    fn imports_are_explicit_and_duplicates_are_idempotent() {
        assert!(compile("int main(){put_i(1);return 0;}").is_err());
        assert!(compile("int main(){get_i();return 0;}").is_err());
        assert_eq!(
            run("import io;import io;int main(){put_i(7);return 0;}", "").unwrap(),
            "7\n"
        );
        assert!(compile("int main(){seconds();return 0;}").is_ok());
        // Unimported names remain available for ordinary user functions.
        assert!(compile("int put_i(int n){return n;}int main(){return put_i(7);}").is_ok());
    }
    #[test]
    fn rejects_unknown_malformed_late_and_conflicting_imports() {
        for source in [
            "import missing;int main(){return 0;}",
            "import io int main(){return 0;}",
            "import \"io\";int main(){return 0;}",
            "int x;import io;int main(){return 0;}",
            "int main(){import io;return 0;}",
            "import io;int put_i(int x){return x;}int main(){return 0;}",
        ] {
            assert!(compile(source).is_err(), "{source}");
        }
    }

    #[test]
    fn math_is_optional_and_independent_of_io() {
        assert!(compile("int main(){return sqrt(4);}").is_err());
        assert!(compile("import io;int main(){return sqrt(4);}").is_err());
        assert!(compile("import math;int main(){put_d(sqrt(4));return 0;}").is_err());
        assert!(compile("import math;int main(){return sqrt(4);}").is_ok());
        assert!(compile("double sqrt(double x){return x;}int main(){return sqrt(4);}").is_ok());
        assert!(
            compile("import math;double sqrt(double x){return x;}int main(){return 0;}").is_err()
        );
        assert_eq!(run("import math;import io;import math;int main(){int x;x=1;put_d(pow(x=x+1,x=x+1));return 0;}", "").unwrap(), "8\n");
    }
    #[test]
    fn math_functions_execute_with_numeric_conversions() {
        let src = "import io;import math;int main(){put_d(sqrt(2));put_d(pow(2,3));put_d(sin(0));put_d(cos(0));put_d(tan(0));put_d(fabs(-2.5));put_d(floor(-2.5));put_d(ceil(-2.5));put_d(round(-2.5));put_d(exp(1));put_d(log(1));put_d(log10(100));put_i(abs(-7));put_d(fabs('A'));return 0;}";
        let output = run(src, "").unwrap();
        let actual: Vec<f64> = output.lines().map(|s| s.parse().unwrap()).collect();
        let expected = [
            std::f64::consts::SQRT_2,
            8.0,
            0.0,
            1.0,
            0.0,
            2.5,
            -3.0,
            -2.0,
            -3.0,
            std::f64::consts::E,
            0.0,
            2.0,
            7.0,
            65.0,
        ];
        assert_eq!(actual.len(), expected.len());
        for (a, b) in actual.iter().zip(expected) {
            assert!((a - b).abs() < 1e-12, "{a} != {b}");
        }
    }
    #[test]
    fn math_rejects_invalid_calls_domains_and_nonfinite_values() {
        for expression in ["sqrt()", "pow(2)", "sin(1,2)", "sqrt(\"bad\")"] {
            assert!(
                compile(&format!("import math;int main(){{{expression};return 0;}}")).is_err(),
                "{expression}"
            );
        }
        for expression in [
            "sqrt(-1)",
            "log(0)",
            "log(-1)",
            "log10(0)",
            "pow(-2,0.5)",
            "pow(0,-1)",
            "exp(1000)",
            "abs(-2147483648)",
            "sin(1.0/0.0)",
            "fabs(0.0/0.0)",
        ] {
            let source = format!("import math;int main(){{{expression};return 0;}}");
            assert!(
                run(&source, "")
                    .unwrap_err()
                    .contains("math domain or range error"),
                "{expression}"
            );
        }
        for input in ["NaN", "inf", "-inf"] {
            assert!(run(
                "import io;import math;int main(){sqrt(get_d());return 0;}",
                input
            )
            .unwrap_err()
            .contains("math domain"));
        }
    }

    #[test]
    fn diagnostics_identify_the_source_file() {
        for source in [
            "int main(){missing();return 0;}",
            "int main(){return @;}",
            "int main(){return 999999999999999999999999999;}",
        ] {
            let error = compile_named(source, "lib/broken.atomc").err().unwrap();
            assert!(error.contains("lib/broken.atomc"), "{error}");
            assert!(error.contains("Line: 1"), "{error}");
        }
        let mut lexer = crate::lexer::Lexer::with_source("int x;", "lib/values.atomc");
        assert_eq!(lexer.get_tokens()[0].location(), "lib/values.atomc:1:1");
    }

    #[test]
    fn custom_imports_share_structs_globals_functions_and_builtins() {
        let project = crate::test_support::Project::new();
        project.write("lib/geometry.atomc", "import math;struct Circle{double radius;};int count;double area(struct Circle c){count=count+1;return 3*pow(c.radius,2);}");
        let entry = project.write("main.atomc", "import io;import \"lib/geometry.atomc\";int main(){struct Circle c;c.radius=2;put_d(area(c));put_i(count);put_d(sqrt(9));return 0;}");
        let mut vm = compile_file(&entry).unwrap().into_vm("").unwrap();
        vm.run().unwrap();
        assert_eq!(vm.output(), "12\n1\n3\n");
        assert_eq!(vm.stack_len(), 0);
    }
    #[test]
    fn custom_import_diamonds_compile_once_and_serialize_deterministically() {
        let project = crate::test_support::Project::new();
        project.write("common.atomc", "int twice(int x){return x*2;}");
        project.write(
            "lib/a.atomc",
            "import \"../common.atomc\";int a(){return twice(2);}",
        );
        project.write(
            "b.atomc",
            "import \"common.atomc\";int b(){return twice(3);}",
        );
        let entry = project.write("main.atomc", "import io;import \"lib/a.atomc\";import \"b.atomc\";import \"./common.atomc\";int main(){put_i(a()+b());return 0;}");
        let bytes = crate::bytecode::encode(&compile_file(&entry).unwrap()).unwrap();
        assert_eq!(
            bytes,
            crate::bytecode::encode(&compile_file(&entry).unwrap()).unwrap()
        );
        let mut vm = crate::bytecode::decode(&bytes)
            .unwrap()
            .into_vm("")
            .unwrap();
        vm.run().unwrap();
        assert_eq!(vm.output(), "10\n");
    }
    #[test]
    fn module_boundaries_duplicates_and_entry_ownership_are_enforced() {
        let project = crate::test_support::Project::new();
        let entry = project.write("main.atomc", "import \"lib.atomc\";int main(){return 0;}");
        for (library, expected) in [
            ("int main(){return 0;}", "only the entry file"),
            ("int x", "end of source module"),
            ("struct Broken{", "end of source module"),
            ("int f(){return 1;", "end of source module"),
            ("int f(){return missing;}", "undefined symbol: missing"),
        ] {
            project.write("lib.atomc", library);
            let error = compile_file(&entry).err().unwrap();
            assert!(error.contains(expected), "{error}");
            assert!(error.contains("lib.atomc"), "{error}");
        }
        project.write("lib.atomc", "int main(){return 0;}");
        project.write("main.atomc", "import \"lib.atomc\";");
        assert!(compile_file(&entry)
            .err()
            .unwrap()
            .contains("only the entry file"));
        project.write("lib.atomc", "int x;");
        project.write(
            "main.atomc",
            "import \"lib.atomc\";int x;int main(){return 0;}",
        );
        assert!(compile_file(&entry)
            .err()
            .unwrap()
            .contains("symbol redefinition: x"));
        assert!(compile("import \"lib.atomc\";int main(){return 0;}")
            .err()
            .unwrap()
            .contains("file compilation entry point"));
    }

    #[test]
    fn baseline_program_compiles_and_runs() {
        assert_eq!(run(include_str!("../data/main.atomc"), "").unwrap(), "");
    }
}
