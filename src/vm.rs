#![allow(dead_code)]

use std::collections::HashMap;
use std::fmt;
use std::io::{BufRead, Write};
use std::time::Instant;

pub const STACK_SIZE: usize = 32 * 1024;
pub const GLOBAL_SIZE: usize = 32 * 1024;

pub const INT_SIZE: usize = size_of::<i32>();
pub const DOUBLE_SIZE: usize = size_of::<f64>();
pub const CHAR_SIZE: usize = size_of::<u8>();
pub const ADDR_SIZE: usize = 8;

pub type Address = usize;
pub type InstrId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtFunc {
    PutS,
    GetS,
    PutI,
    GetI,
    PutD,
    GetD,
    PutC,
    GetC,
    Seconds,
}

impl ExtFunc {
    pub fn from_name(name: &str) -> Option<Self> {
        crate::builtins::ALL
            .iter()
            .find(|b| b.name == name)
            .map(|b| b.function)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    Trap,
    GlobalRef,
    LocalRef,
    ArgRef,
    Index,
    Field,
    ReadOnlyRef,
    AddC,
    AddD,
    AddI,
    AndA,
    AndC,
    AndD,
    AndI,
    Call,
    CallExt,
    CastCD,
    CastCI,
    CastDC,
    CastDI,
    CastIC,
    CastID,
    DivC,
    DivD,
    DivI,
    Drop,
    Enter,
    EqA,
    EqC,
    EqD,
    EqI,
    GreaterC,
    GreaterD,
    GreaterI,
    GreaterEqC,
    GreaterEqD,
    GreaterEqI,
    Halt,
    Insert,
    JfA,
    JfC,
    JfD,
    JfI,
    Jmp,
    JtA,
    JtC,
    JtD,
    JtI,
    LessC,
    LessD,
    LessI,
    LessEqC,
    LessEqD,
    LessEqI,
    Load,
    MulC,
    MulD,
    MulI,
    NegC,
    NegD,
    NegI,
    NeqA,
    NeqC,
    NeqD,
    NeqI,
    Nop,
    NotA,
    NotC,
    NotD,
    NotI,
    Offset,
    OrA,
    OrC,
    OrD,
    OrI,
    PushFpAddr,
    PushCtA,
    PushCtC,
    PushCtD,
    PushCtI,
    Ret,
    Store,
    SubC,
    SubD,
    SubI,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InstrArg {
    None,
    Int(i64),
    Double(f64),
    Address(Address),
    Instr(InstrId),
    ExtFunc(ExtFunc),
}

impl InstrArg {
    fn int(self) -> Result<i64, VmError> {
        match self {
            Self::Int(value) => Ok(value),
            _ => Err(VmError::InvalidArgument("integer")),
        }
    }

    fn address(self) -> Result<Address, VmError> {
        match self {
            Self::Address(value) => Ok(value),
            _ => Err(VmError::InvalidArgument("address")),
        }
    }

    fn instr(self) -> Result<InstrId, VmError> {
        match self {
            Self::Instr(value) => Ok(value),
            _ => Err(VmError::InvalidArgument("instruction")),
        }
    }

    fn double(self) -> Result<f64, VmError> {
        match self {
            Self::Double(value) => Ok(value),
            _ => Err(VmError::InvalidArgument("double")),
        }
    }

    fn ext_func(self) -> Result<ExtFunc, VmError> {
        match self {
            Self::ExtFunc(value) => Ok(value),
            _ => Err(VmError::InvalidArgument("external function")),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Instr {
    pub opcode: Opcode,
    pub args: [InstrArg; 2],
    pub last: Option<InstrId>,
    pub next: Option<InstrId>,
}

impl Instr {
    pub fn new(opcode: Opcode) -> Self {
        Self {
            opcode,
            args: [InstrArg::None; 2],
            last: None,
            next: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct Instructions {
    items: Vec<Option<Instr>>,
    pub first: Option<InstrId>,
    pub last: Option<InstrId>,
}

impl Instructions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, id: InstrId) -> Option<&Instr> {
        self.items.get(id)?.as_ref()
    }

    pub fn get_mut(&mut self, id: InstrId) -> Option<&mut Instr> {
        self.items.get_mut(id)?.as_mut()
    }

    pub fn add_instr(&mut self, opcode: Opcode) -> InstrId {
        self.add(Instr::new(opcode))
    }

    pub fn add_instr_i(&mut self, opcode: Opcode, value: i64) -> InstrId {
        let mut instr = Instr::new(opcode);
        instr.args[0] = InstrArg::Int(value);
        self.add(instr)
    }

    pub fn add_instr_ii(&mut self, opcode: Opcode, value1: i64, value2: i64) -> InstrId {
        let mut instr = Instr::new(opcode);
        instr.args = [InstrArg::Int(value1), InstrArg::Int(value2)];
        self.add(instr)
    }

    pub fn add_instr_d(&mut self, opcode: Opcode, value: f64) -> InstrId {
        let mut instr = Instr::new(opcode);
        instr.args[0] = InstrArg::Double(value);
        self.add(instr)
    }

    pub fn add_instr_a(&mut self, opcode: Opcode, address: Address) -> InstrId {
        let mut instr = Instr::new(opcode);
        instr.args[0] = InstrArg::Address(address);
        self.add(instr)
    }

    pub fn add_instr_target(&mut self, opcode: Opcode, target: InstrId) -> InstrId {
        let mut instr = Instr::new(opcode);
        instr.args[0] = InstrArg::Instr(target);
        self.add(instr)
    }

    pub fn add_instr_ext(&mut self, func: ExtFunc) -> InstrId {
        let mut instr = Instr::new(Opcode::CallExt);
        instr.args[0] = InstrArg::ExtFunc(func);
        self.add(instr)
    }

    pub fn add_instr_after(&mut self, after: InstrId, opcode: Opcode) -> Result<InstrId, VmError> {
        let instr = Instr::new(opcode);
        self.insert_instr_after(after, instr)
    }

    pub fn insert_instr_after(
        &mut self,
        after: InstrId,
        mut instr: Instr,
    ) -> Result<InstrId, VmError> {
        let next = self
            .get(after)
            .ok_or(VmError::InvalidInstruction(after))?
            .next;
        let id = self.items.len();

        instr.last = Some(after);
        instr.next = next;
        self.items.push(Some(instr));
        self.get_mut(after).unwrap().next = Some(id);

        if let Some(next) = next {
            self.get_mut(next).unwrap().last = Some(id);
        } else {
            self.last = Some(id);
        }

        Ok(id)
    }

    pub fn delete_instructions_after(&mut self, start: InstrId) -> Result<(), VmError> {
        let mut current = self
            .get(start)
            .ok_or(VmError::InvalidInstruction(start))?
            .next;

        self.get_mut(start).unwrap().next = None;
        self.last = Some(start);

        while let Some(id) = current {
            let next = self.get(id).and_then(|instr| instr.next);
            self.items[id] = None;
            current = next;
        }

        Ok(())
    }

    fn add(&mut self, mut instr: Instr) -> InstrId {
        let id = self.items.len();
        instr.last = self.last;

        if let Some(last) = self.last {
            self.get_mut(last).unwrap().next = Some(id);
        } else {
            self.first = Some(id);
        }

        self.items.push(Some(instr));
        self.last = Some(id);
        id
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VmError {
    Bounds,
    ResourceLimit,
    DivisionByZero,
    GlobalOverflow,
    InvalidAddress(Address),
    InvalidArgument(&'static str),
    InvalidInstruction(InstrId),
    InvalidInput(&'static str),
    MissingFramePointer,
    MissingInstruction,
    StackOverflow,
    StackUnderflow,
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bounds => write!(f, "guest object bounds violation"),
            Self::ResourceLimit => write!(f, "guest resource limit exceeded"),
            Self::DivisionByZero => write!(f, "division by zero"),
            Self::GlobalOverflow => write!(f, "insufficient globals space"),
            Self::InvalidAddress(address) => write!(f, "invalid memory address: {address}"),
            Self::InvalidArgument(expected) => {
                write!(f, "invalid instruction argument, expected {expected}")
            }
            Self::InvalidInstruction(id) => write!(f, "invalid instruction: {id}"),
            Self::InvalidInput(expected) => write!(f, "invalid input, expected {expected}"),
            Self::MissingFramePointer => write!(f, "frame pointer is not initialized"),
            Self::MissingInstruction => {
                write!(f, "instruction pointer reached the end without HALT")
            }
            Self::StackOverflow => write!(f, "out of stack"),
            Self::StackUnderflow => write!(f, "not enough stack bytes"),
        }
    }
}

impl std::error::Error for VmError {}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Reference {
    address: usize,
    size: usize,
    writable: bool,
    frame: Option<usize>,
}
struct Frame {
    fp: usize,
    end: usize,
    caller_floor: usize,
    return_ip: usize,
    old_fp: Option<usize>,
}
pub struct Vm {
    pub checked: bool,
    pub fuel: usize,
    refs: HashMap<usize, Reference>,
    ref_ids: HashMap<Reference, usize>,
    next_ref: usize,
    frames: Vec<Frame>,
    calls: Vec<(usize, usize)>,
    operand_floor: usize,
    pub instructions: Instructions,
    memory: Vec<u8>,
    n_globals: usize,
    sp: usize,
    fp: Option<usize>,
    input: Vec<char>,
    input_pos: usize,
    output: String,
    started_at: Instant,
    pub trace: bool,
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

impl Vm {
    pub fn new() -> Self {
        Self {
            checked: false,
            fuel: 1_000_000,
            refs: HashMap::new(),
            ref_ids: HashMap::new(),
            next_ref: 1,
            frames: Vec::new(),
            calls: Vec::new(),
            operand_floor: GLOBAL_SIZE,
            instructions: Instructions::new(),
            memory: vec![0; GLOBAL_SIZE + STACK_SIZE],
            n_globals: 0,
            sp: GLOBAL_SIZE,
            fp: None,
            input: Vec::new(),
            input_pos: 0,
            output: String::new(),
            started_at: Instant::now(),
            trace: false,
        }
    }

    pub fn with_input(input: &str) -> Self {
        let mut vm = Self::new();
        vm.set_input(input);
        vm
    }

    pub fn set_input(&mut self, input: &str) {
        self.input = input.chars().collect();
        self.input_pos = 0;
    }

    pub fn output(&self) -> &str {
        &self.output
    }

    pub fn take_output(&mut self) -> String {
        std::mem::take(&mut self.output)
    }

    pub fn stack_len(&self) -> usize {
        self.sp - GLOBAL_SIZE
    }

    pub fn alloc_global(&mut self, size: usize) -> Result<Address, VmError> {
        if size > GLOBAL_SIZE - self.n_globals {
            return Err(VmError::GlobalOverflow);
        }

        let address = self.n_globals;
        self.n_globals += size;
        Ok(address)
    }

    pub fn read_memory(&self, address: Address, size: usize) -> Result<&[u8], VmError> {
        let end = address
            .checked_add(size)
            .ok_or(VmError::InvalidAddress(address))?;

        if end > self.memory.len() {
            return Err(VmError::InvalidAddress(address));
        }

        Ok(&self.memory[address..end])
    }

    pub fn write_memory(&mut self, address: Address, bytes: &[u8]) -> Result<(), VmError> {
        let end = address
            .checked_add(bytes.len())
            .ok_or(VmError::InvalidAddress(address))?;

        if end > self.memory.len() {
            return Err(VmError::InvalidAddress(address));
        }

        self.memory[address..end].copy_from_slice(bytes);
        Ok(())
    }

    pub fn push_c(&mut self, value: u8) -> Result<(), VmError> {
        self.push_bytes(&[value])
    }

    pub fn pop_c(&mut self) -> Result<u8, VmError> {
        Ok(self.pop_bytes(CHAR_SIZE)?[0])
    }

    pub fn push_i(&mut self, value: i32) -> Result<(), VmError> {
        self.push_bytes(&value.to_le_bytes())
    }

    pub fn pop_i(&mut self) -> Result<i32, VmError> {
        let bytes: [u8; INT_SIZE] = self.pop_bytes(INT_SIZE)?.try_into().unwrap();
        Ok(i32::from_le_bytes(bytes))
    }

    pub fn push_d(&mut self, value: f64) -> Result<(), VmError> {
        self.push_bytes(&value.to_le_bytes())
    }

    pub fn pop_d(&mut self) -> Result<f64, VmError> {
        let bytes: [u8; DOUBLE_SIZE] = self.pop_bytes(DOUBLE_SIZE)?.try_into().unwrap();
        Ok(f64::from_le_bytes(bytes))
    }

    pub fn push_a(&mut self, value: Address) -> Result<(), VmError> {
        self.push_bytes(&(value as u64).to_le_bytes())
    }

    pub fn pop_a(&mut self) -> Result<Address, VmError> {
        let bytes: [u8; ADDR_SIZE] = self.pop_bytes(ADDR_SIZE)?.try_into().unwrap();
        usize::try_from(u64::from_le_bytes(bytes)).map_err(|_| VmError::Bounds)
    }

    pub fn run(&mut self) -> Result<(), VmError> {
        let first = self.instructions.first.ok_or(VmError::MissingInstruction)?;
        self.run_from(first)
    }

    /// Only these explicitly supplied streams are available to the guest.
    pub fn run_with_io(
        &mut self,
        input: &mut dyn BufRead,
        output: &mut dyn Write,
    ) -> Result<(), VmError> {
        let first = self.instructions.first.ok_or(VmError::MissingInstruction)?;
        self.execute(first, Some((input, output)))
    }

    pub fn run_from(&mut self, start: InstrId) -> Result<(), VmError> {
        self.execute(start, None)
    }

    fn execute(
        &mut self,
        start: InstrId,
        mut io: Option<(&mut dyn BufRead, &mut dyn Write)>,
    ) -> Result<(), VmError> {
        let mut written = self.output.len();
        self.sp = GLOBAL_SIZE;
        self.fp = None;
        self.started_at = Instant::now();
        self.refs.clear();
        self.ref_ids.clear();
        self.next_ref = 1;
        self.frames.clear();
        self.calls.clear();
        self.operand_floor = GLOBAL_SIZE;
        let mut fuel = self.fuel;
        let mut ip = Some(start);

        while let Some(id) = ip {
            if fuel == 0 || self.output.len() > 1024 * 1024 {
                return Err(VmError::ResourceLimit);
            }
            fuel -= 1;
            let instr = self
                .instructions
                .get(id)
                .ok_or(VmError::InvalidInstruction(id))?
                .clone();

            if self.trace {
                println!(
                    "{id}/{:?}\t{:?}\t{:?}",
                    self.stack_len(),
                    instr.opcode,
                    instr.args
                );
            }

            if self.checked
                && matches!(
                    instr.opcode,
                    Opcode::PushCtA | Opcode::PushFpAddr | Opcode::Offset
                )
            {
                return Err(VmError::InvalidArgument(
                    "bounded guest reference instruction",
                ));
            }
            let next = instr.next;
            ip = match instr.opcode {
                Opcode::Trap => {
                    return Err(VmError::InvalidArgument(
                        "function ended without a return value",
                    ))
                }
                Opcode::GlobalRef | Opcode::ReadOnlyRef => {
                    let address = self.arg_size(instr.args[0])?;
                    let size = self.arg_size(instr.args[1])?;
                    if address.checked_add(size).is_none_or(|e| e > self.n_globals) {
                        return Err(VmError::Bounds);
                    }
                    let handle = self.reference(Reference {
                        address,
                        size,
                        writable: instr.opcode == Opcode::GlobalRef,
                        frame: None,
                    })?;
                    self.push_a(handle)?;
                    next
                }
                Opcode::LocalRef | Opcode::ArgRef => {
                    let offset = instr.args[0].int()?;
                    let size = self.arg_size(instr.args[1])?;
                    let frame = self.frames.last().ok_or(VmError::MissingFramePointer)?;
                    let address = frame
                        .fp
                        .checked_add_signed(isize::try_from(offset).map_err(|_| VmError::Bounds)?)
                        .ok_or(VmError::Bounds)?;
                    let end = address.checked_add(size).ok_or(VmError::Bounds)?;
                    if instr.opcode == Opcode::LocalRef {
                        if address < frame.fp || end > frame.end {
                            return Err(VmError::Bounds);
                        }
                    } else if address < frame.caller_floor || end > frame.fp - 2 * ADDR_SIZE {
                        return Err(VmError::Bounds);
                    }
                    let handle = self.reference(Reference {
                        address,
                        size,
                        writable: true,
                        frame: Some(self.frames.len()),
                    })?;
                    self.push_a(handle)?;
                    next
                }
                Opcode::Index | Opcode::Field => {
                    let (offset, size) = if instr.opcode == Opcode::Index {
                        let stride = self.arg_size(instr.args[0])?;
                        let index = usize::try_from(self.pop_i()?).map_err(|_| VmError::Bounds)?;
                        if stride == 0 {
                            return Err(VmError::Bounds);
                        }
                        (index.checked_mul(stride).ok_or(VmError::Bounds)?, stride)
                    } else {
                        (self.arg_size(instr.args[0])?, self.arg_size(instr.args[1])?)
                    };
                    let handle = self.pop_a()?;
                    let parent = self.get_reference(handle)?;
                    if offset.checked_add(size).is_none_or(|e| e > parent.size) {
                        return Err(VmError::Bounds);
                    }
                    let handle = self.reference(Reference {
                        address: parent.address + offset,
                        size,
                        ..parent
                    })?;
                    self.push_a(handle)?;
                    next
                }
                Opcode::AddC => {
                    let right = self.pop_c()?;
                    let left = self.pop_c()?;
                    self.push_c(left.wrapping_add(right))?;
                    next
                }
                Opcode::AddD => {
                    let right = self.pop_d()?;
                    let left = self.pop_d()?;
                    self.push_d(left + right)?;
                    next
                }
                Opcode::AddI => {
                    let right = self.pop_i()?;
                    let left = self.pop_i()?;
                    self.push_i(left.wrapping_add(right))?;
                    next
                }
                Opcode::AndA => {
                    let right = self.pop_a()?;
                    let left = self.pop_a()?;
                    self.push_i(i32::from(left != 0 && right != 0))?;
                    next
                }
                Opcode::AndC => {
                    let right = self.pop_c()?;
                    let left = self.pop_c()?;
                    self.push_i(i32::from(left != 0 && right != 0))?;
                    next
                }
                Opcode::AndD => {
                    let right = self.pop_d()?;
                    let left = self.pop_d()?;
                    self.push_i(i32::from(left != 0.0 && right != 0.0))?;
                    next
                }
                Opcode::AndI => {
                    let right = self.pop_i()?;
                    let left = self.pop_i()?;
                    self.push_i(i32::from(left != 0 && right != 0))?;
                    next
                }
                Opcode::Call => {
                    let target = instr.args[0].instr()?;
                    if self.checked {
                        if self.calls.len() >= 256 {
                            return Err(VmError::ResourceLimit);
                        }
                        self.calls
                            .push((next.ok_or(VmError::MissingInstruction)?, self.sp));
                    }
                    self.push_a(next.ok_or(VmError::MissingInstruction)?)?;
                    Some(target)
                }
                Opcode::CallExt => {
                    let func = instr.args[0].ext_func()?;
                    if let Some((reader, _)) = io.as_mut() {
                        self.refill_input(func, *reader)?;
                    }
                    self.run_ext_func(func)?;
                    if let Some((_, writer)) = io.as_mut() {
                        writer
                            .write_all(self.output[written..].as_bytes())
                            .map_err(|_| VmError::InvalidInput("writable output stream"))?;
                        writer
                            .flush()
                            .map_err(|_| VmError::InvalidInput("writable output stream"))?;
                        written = self.output.len();
                    }
                    next
                }
                Opcode::CastCD => {
                    let value = self.pop_c()?;
                    self.push_d(value as f64)?;
                    next
                }
                Opcode::CastCI => {
                    let value = self.pop_c()?;
                    self.push_i(value as i32)?;
                    next
                }
                Opcode::CastDC => {
                    let value = self.pop_d()?;
                    self.push_c(value as u8)?;
                    next
                }
                Opcode::CastDI => {
                    let value = self.pop_d()?;
                    self.push_i(value as i32)?;
                    next
                }
                Opcode::CastIC => {
                    let value = self.pop_i()?;
                    self.push_c(value as u8)?;
                    next
                }
                Opcode::CastID => {
                    let value = self.pop_i()?;
                    self.push_d(value as f64)?;
                    next
                }
                Opcode::DivC => {
                    let right = self.pop_c()?;
                    let left = self.pop_c()?;
                    if right == 0 {
                        return Err(VmError::DivisionByZero);
                    }
                    self.push_c(left / right)?;
                    next
                }
                Opcode::DivD => {
                    let right = self.pop_d()?;
                    let left = self.pop_d()?;
                    self.push_d(left / right)?;
                    next
                }
                Opcode::DivI => {
                    let right = self.pop_i()?;
                    let left = self.pop_i()?;
                    if right == 0 {
                        return Err(VmError::DivisionByZero);
                    }
                    self.push_i(left.wrapping_div(right))?;
                    next
                }
                Opcode::Drop => {
                    let size = self.arg_size(instr.args[0])?;
                    self.drop_bytes(size)?;
                    next
                }
                Opcode::Enter => {
                    let size = self.arg_size(instr.args[0])?;
                    self.push_a(self.fp.unwrap_or(usize::MAX))?;
                    self.fp = Some(self.sp);
                    self.reserve_stack(size)?;
                    if self.checked {
                        let (return_ip, call_sp) =
                            *self.calls.last().ok_or(VmError::MissingFramePointer)?;
                        let fp = self.fp.unwrap();
                        if fp != call_sp + 2 * ADDR_SIZE
                            || self.frames.len() + 1 != self.calls.len()
                        {
                            return Err(VmError::Bounds);
                        }
                        let old_fp = self.frames.last().map(|f| f.fp);
                        self.frames.push(Frame {
                            fp,
                            end: self.sp,
                            caller_floor: self.operand_floor,
                            return_ip,
                            old_fp,
                        });
                        self.operand_floor = self.sp;
                    }
                    next
                }
                Opcode::EqA => {
                    let right = self.pop_a()?;
                    let left = self.pop_a()?;
                    self.push_i(i32::from(left == right))?;
                    next
                }
                Opcode::EqC => {
                    let right = self.pop_c()?;
                    let left = self.pop_c()?;
                    self.push_i(i32::from(left == right))?;
                    next
                }
                Opcode::EqD => {
                    let right = self.pop_d()?;
                    let left = self.pop_d()?;
                    self.push_i(i32::from(left == right))?;
                    next
                }
                Opcode::EqI => {
                    let right = self.pop_i()?;
                    let left = self.pop_i()?;
                    self.push_i(i32::from(left == right))?;
                    next
                }
                Opcode::GreaterC => {
                    let right = self.pop_c()?;
                    let left = self.pop_c()?;
                    self.push_i(i32::from(left > right))?;
                    next
                }
                Opcode::GreaterD => {
                    let right = self.pop_d()?;
                    let left = self.pop_d()?;
                    self.push_i(i32::from(left > right))?;
                    next
                }
                Opcode::GreaterI => {
                    let right = self.pop_i()?;
                    let left = self.pop_i()?;
                    self.push_i(i32::from(left > right))?;
                    next
                }
                Opcode::GreaterEqC => {
                    let right = self.pop_c()?;
                    let left = self.pop_c()?;
                    self.push_i(i32::from(left >= right))?;
                    next
                }
                Opcode::GreaterEqD => {
                    let right = self.pop_d()?;
                    let left = self.pop_d()?;
                    self.push_i(i32::from(left >= right))?;
                    next
                }
                Opcode::GreaterEqI => {
                    let right = self.pop_i()?;
                    let left = self.pop_i()?;
                    self.push_i(i32::from(left >= right))?;
                    next
                }
                Opcode::Halt => return Ok(()),
                Opcode::Insert => {
                    let destination_size = self.arg_size(instr.args[0])?;
                    let value_size = self.arg_size(instr.args[1])?;
                    self.insert(destination_size, value_size)?;
                    next
                }
                Opcode::JfA => Self::jump_if(self.pop_a()? == 0, instr.args[0], next)?,
                Opcode::JfC => Self::jump_if(self.pop_c()? == 0, instr.args[0], next)?,
                Opcode::JfD => Self::jump_if(self.pop_d()? == 0.0, instr.args[0], next)?,
                Opcode::JfI => Self::jump_if(self.pop_i()? == 0, instr.args[0], next)?,
                Opcode::Jmp => Some(instr.args[0].instr()?),
                Opcode::JtA => Self::jump_if(self.pop_a()? != 0, instr.args[0], next)?,
                Opcode::JtC => Self::jump_if(self.pop_c()? != 0, instr.args[0], next)?,
                Opcode::JtD => Self::jump_if(self.pop_d()? != 0.0, instr.args[0], next)?,
                Opcode::JtI => Self::jump_if(self.pop_i()? != 0, instr.args[0], next)?,
                Opcode::LessC => {
                    let right = self.pop_c()?;
                    let left = self.pop_c()?;
                    self.push_i(i32::from(left < right))?;
                    next
                }
                Opcode::LessD => {
                    let right = self.pop_d()?;
                    let left = self.pop_d()?;
                    self.push_i(i32::from(left < right))?;
                    next
                }
                Opcode::LessI => {
                    let right = self.pop_i()?;
                    let left = self.pop_i()?;
                    self.push_i(i32::from(left < right))?;
                    next
                }
                Opcode::LessEqC => {
                    let right = self.pop_c()?;
                    let left = self.pop_c()?;
                    self.push_i(i32::from(left <= right))?;
                    next
                }
                Opcode::LessEqD => {
                    let right = self.pop_d()?;
                    let left = self.pop_d()?;
                    self.push_i(i32::from(left <= right))?;
                    next
                }
                Opcode::LessEqI => {
                    let right = self.pop_i()?;
                    let left = self.pop_i()?;
                    self.push_i(i32::from(left <= right))?;
                    next
                }
                Opcode::Load => {
                    let size = self.arg_size(instr.args[0])?;
                    let address = self.pop_a()?;
                    let address = self.resolve(address, size, false)?;
                    let bytes = self.read_memory(address, size)?.to_vec();
                    self.push_bytes(&bytes)?;
                    next
                }
                Opcode::MulC => {
                    let right = self.pop_c()?;
                    let left = self.pop_c()?;
                    self.push_c(left.wrapping_mul(right))?;
                    next
                }
                Opcode::MulD => {
                    let right = self.pop_d()?;
                    let left = self.pop_d()?;
                    self.push_d(left * right)?;
                    next
                }
                Opcode::MulI => {
                    let right = self.pop_i()?;
                    let left = self.pop_i()?;
                    self.push_i(left.wrapping_mul(right))?;
                    next
                }
                Opcode::NegC => {
                    let value = self.pop_c()?;
                    self.push_c(value.wrapping_neg())?;
                    next
                }
                Opcode::NegD => {
                    let value = self.pop_d()?;
                    self.push_d(-value)?;
                    next
                }
                Opcode::NegI => {
                    let value = self.pop_i()?;
                    self.push_i(value.wrapping_neg())?;
                    next
                }
                Opcode::NeqA => {
                    let right = self.pop_a()?;
                    let left = self.pop_a()?;
                    self.push_i(i32::from(left != right))?;
                    next
                }
                Opcode::NeqC => {
                    let right = self.pop_c()?;
                    let left = self.pop_c()?;
                    self.push_i(i32::from(left != right))?;
                    next
                }
                Opcode::NeqD => {
                    let right = self.pop_d()?;
                    let left = self.pop_d()?;
                    self.push_i(i32::from(left != right))?;
                    next
                }
                Opcode::NeqI => {
                    let right = self.pop_i()?;
                    let left = self.pop_i()?;
                    self.push_i(i32::from(left != right))?;
                    next
                }
                Opcode::Nop => next,
                Opcode::NotA => {
                    let value = self.pop_a()?;
                    self.push_i(i32::from(value == 0))?;
                    next
                }
                Opcode::NotC => {
                    let value = self.pop_c()?;
                    self.push_i(i32::from(value == 0))?;
                    next
                }
                Opcode::NotD => {
                    let value = self.pop_d()?;
                    self.push_i(i32::from(value == 0.0))?;
                    next
                }
                Opcode::NotI => {
                    let value = self.pop_i()?;
                    self.push_i(i32::from(value == 0))?;
                    next
                }
                Opcode::Offset => {
                    let offset = self.pop_i()? as isize;
                    let address = self.pop_a()?;
                    let result = address
                        .checked_add_signed(offset)
                        .ok_or(VmError::InvalidAddress(address))?;
                    self.ensure_address(result, 0)?;
                    self.push_a(result)?;
                    next
                }
                Opcode::OrA => {
                    let right = self.pop_a()?;
                    let left = self.pop_a()?;
                    self.push_i(i32::from(left != 0 || right != 0))?;
                    next
                }
                Opcode::OrC => {
                    let right = self.pop_c()?;
                    let left = self.pop_c()?;
                    self.push_i(i32::from(left != 0 || right != 0))?;
                    next
                }
                Opcode::OrD => {
                    let right = self.pop_d()?;
                    let left = self.pop_d()?;
                    self.push_i(i32::from(left != 0.0 || right != 0.0))?;
                    next
                }
                Opcode::OrI => {
                    let right = self.pop_i()?;
                    let left = self.pop_i()?;
                    self.push_i(i32::from(left != 0 || right != 0))?;
                    next
                }
                Opcode::PushFpAddr => {
                    let fp = self.fp.ok_or(VmError::MissingFramePointer)?;
                    let offset = instr.args[0].int()? as isize;
                    let address = fp
                        .checked_add_signed(offset)
                        .ok_or(VmError::InvalidAddress(fp))?;
                    self.ensure_address(address, 0)?;
                    self.push_a(address)?;
                    next
                }
                Opcode::PushCtA => {
                    self.push_a(instr.args[0].address()?)?;
                    next
                }
                Opcode::PushCtC => {
                    self.push_c(instr.args[0].int()? as u8)?;
                    next
                }
                Opcode::PushCtD => {
                    self.push_d(instr.args[0].double()?)?;
                    next
                }
                Opcode::PushCtI => {
                    self.push_i(instr.args[0].int()? as i32)?;
                    next
                }
                Opcode::Ret => {
                    let args_size = self.arg_size(instr.args[0])?;
                    let return_size = self.arg_size(instr.args[1])?;
                    if self.checked {
                        if return_size > self.sp - self.operand_floor {
                            return Err(VmError::StackUnderflow);
                        }
                        let frame = self.frames.pop().ok_or(VmError::MissingFramePointer)?;
                        let args_end = frame.fp - 2 * ADDR_SIZE;
                        let base = args_end
                            .checked_sub(args_size)
                            .filter(|b| *b >= frame.caller_floor)
                            .ok_or(VmError::Bounds)?;
                        let value = self.memory[self.sp - return_size..self.sp].to_vec();
                        self.refs
                            .retain(|_, r| r.frame.is_none_or(|depth| depth <= self.frames.len()));
                        self.ref_ids.retain(|_, id| self.refs.contains_key(id));
                        self.sp = base;
                        self.fp = frame.old_fp;
                        self.operand_floor = frame.caller_floor;
                        self.calls.pop();
                        self.push_bytes(&value)?;
                        ip = Some(frame.return_ip);
                        continue;
                    }
                    let old_sp = self.sp;
                    let fp = self.fp.ok_or(VmError::MissingFramePointer)?;

                    if return_size > old_sp - GLOBAL_SIZE {
                        return Err(VmError::StackUnderflow);
                    }

                    let return_value = self.memory[old_sp - return_size..old_sp].to_vec();
                    self.sp = fp;
                    let old_fp = self.pop_a()?;
                    let return_ip = self.pop_a()?;
                    self.fp = (old_fp != usize::MAX).then_some(old_fp);
                    self.drop_bytes(args_size)?;
                    self.push_bytes(&return_value)?;
                    Some(return_ip)
                }
                Opcode::Store => {
                    let size = self.arg_size(instr.args[0])?;
                    if size > STACK_SIZE || self.sp - self.operand_floor < ADDR_SIZE + size {
                        return Err(VmError::StackUnderflow);
                    }

                    let address_pos = self.sp - size - ADDR_SIZE;
                    let address = u64::from_le_bytes(
                        self.memory[address_pos..address_pos + ADDR_SIZE]
                            .try_into()
                            .unwrap(),
                    );
                    let address = usize::try_from(address).map_err(|_| VmError::Bounds)?;
                    let address = self.resolve(address, size, true)?;
                    let value = self.memory[self.sp - size..self.sp].to_vec();
                    self.write_memory(address, &value)?;
                    self.sp -= ADDR_SIZE + size;
                    next
                }
                Opcode::SubC => {
                    let right = self.pop_c()?;
                    let left = self.pop_c()?;
                    self.push_c(left.wrapping_sub(right))?;
                    next
                }
                Opcode::SubD => {
                    let right = self.pop_d()?;
                    let left = self.pop_d()?;
                    self.push_d(left - right)?;
                    next
                }
                Opcode::SubI => {
                    let right = self.pop_i()?;
                    let left = self.pop_i()?;
                    self.push_i(left.wrapping_sub(right))?;
                    next
                }
            };
        }

        Err(VmError::MissingInstruction)
    }

    fn refill_input(&mut self, func: ExtFunc, reader: &mut dyn BufRead) -> Result<(), VmError> {
        if !matches!(
            func,
            ExtFunc::GetI | ExtFunc::GetD | ExtFunc::GetC | ExtFunc::GetS
        ) {
            return Ok(());
        }
        let rest: String = self.input[self.input_pos..].iter().collect();
        let need = match func {
            ExtFunc::GetI | ExtFunc::GetD => rest.chars().all(char::is_whitespace),
            ExtFunc::GetS => rest.is_empty() || rest == "\n",
            _ => rest.is_empty(),
        };
        if need {
            let mut consumed = 0;
            loop {
                let mut bytes = Vec::new();
                std::io::Read::take(&mut *reader, 1024 * 1024 + 1)
                    .read_until(b'\n', &mut bytes)
                    .map_err(|_| VmError::InvalidInput("readable input stream"))?;
                consumed += bytes.len();
                if consumed > 1024 * 1024 {
                    return Err(VmError::ResourceLimit);
                }
                let eof = bytes.is_empty();
                let line =
                    String::from_utf8(bytes).map_err(|_| VmError::InvalidInput("UTF-8 input"))?;
                let skip = matches!(func, ExtFunc::GetI | ExtFunc::GetD)
                    && line.chars().all(char::is_whitespace);
                self.set_input(&line);
                if !skip || eof {
                    break;
                }
            }
        }
        Ok(())
    }

    fn run_ext_func(&mut self, func: ExtFunc) -> Result<(), VmError> {
        match func {
            ExtFunc::PutS => {
                let address = self.pop_a()?;
                let value = self.read_c_string(address)?;
                self.output.push_str(&value);
                self.output.push('\n');
            }
            ExtFunc::GetS => {
                let address = self.pop_a()?;
                let value = self.read_input_line();
                let mut bytes = value.into_bytes();
                bytes.push(0);
                let address = self.resolve(address, bytes.len(), true)?;
                self.write_memory(address, &bytes)?;
            }
            ExtFunc::PutI => {
                let value = self.pop_i()?;
                self.output
                    .push_str(&format!("{}{value}\n", if self.checked { "" } else { "#" }));
            }
            ExtFunc::GetI => {
                let value = self
                    .read_input_word()
                    .parse::<i32>()
                    .map_err(|_| VmError::InvalidInput("integer"))?;
                self.push_i(value)?;
            }
            ExtFunc::PutD => {
                let value = self.pop_d()?;
                self.output
                    .push_str(&format!("{}{value}\n", if self.checked { "" } else { "#" }));
            }
            ExtFunc::GetD => {
                let value = self
                    .read_input_word()
                    .parse::<f64>()
                    .map_err(|_| VmError::InvalidInput("double"))?;
                self.push_d(value)?;
            }
            ExtFunc::PutC => {
                let value = self.pop_c()?;
                if !self.checked {
                    self.output.push('#');
                }
                self.output.push(value as char);
                if !self.checked {
                    self.output.push('\n');
                }
            }
            ExtFunc::GetC => {
                let value = self
                    .input
                    .get(self.input_pos)
                    .copied()
                    .ok_or(VmError::InvalidInput("character"))?;
                self.input_pos += 1;
                self.push_c(
                    u8::try_from(value as u32)
                        .map_err(|_| VmError::InvalidInput("byte character"))?,
                )?;
            }
            ExtFunc::Seconds => self.push_d(self.started_at.elapsed().as_secs_f64())?,
        }

        Ok(())
    }

    fn reference(&mut self, r: Reference) -> Result<usize, VmError> {
        // Deduplicate repeated references in loops while retaining lifetime identity.
        if let Some(&id) = self.ref_ids.get(&r) {
            return Ok(id);
        }
        if self.refs.len() >= 65536 {
            return Err(VmError::ResourceLimit);
        }
        let id = self.next_ref;
        self.next_ref = id.checked_add(1).ok_or(VmError::ResourceLimit)?;
        self.refs.insert(id, r);
        self.ref_ids.insert(r, id);
        Ok(id)
    }
    fn get_reference(&self, id: usize) -> Result<Reference, VmError> {
        self.refs.get(&id).copied().ok_or(VmError::Bounds)
    }
    fn resolve(&self, address: usize, size: usize, write: bool) -> Result<usize, VmError> {
        if !self.checked {
            return Ok(address);
        }
        let r = self.get_reference(address)?;
        if size > r.size || (write && !r.writable) {
            return Err(VmError::Bounds);
        }
        Ok(r.address)
    }

    fn push_bytes(&mut self, bytes: &[u8]) -> Result<(), VmError> {
        if bytes.len() > self.memory.len() - self.sp {
            return Err(VmError::StackOverflow);
        }

        self.memory[self.sp..self.sp + bytes.len()].copy_from_slice(bytes);
        self.sp += bytes.len();
        Ok(())
    }

    fn pop_bytes(&mut self, size: usize) -> Result<&[u8], VmError> {
        if self.sp - self.operand_floor < size {
            return Err(VmError::StackUnderflow);
        }

        self.sp -= size;
        Ok(&self.memory[self.sp..self.sp + size])
    }

    fn reserve_stack(&mut self, size: usize) -> Result<(), VmError> {
        if size > self.memory.len() - self.sp {
            return Err(VmError::StackOverflow);
        }

        self.memory[self.sp..self.sp + size].fill(0);
        self.sp += size;
        Ok(())
    }

    fn drop_bytes(&mut self, size: usize) -> Result<(), VmError> {
        if self.sp - self.operand_floor < size {
            return Err(VmError::StackUnderflow);
        }

        self.sp -= size;
        Ok(())
    }

    fn insert(&mut self, destination_size: usize, value_size: usize) -> Result<(), VmError> {
        if destination_size > self.sp - self.operand_floor
            || value_size > self.sp - self.operand_floor
        {
            return Err(VmError::StackUnderflow);
        }
        if value_size > self.memory.len() - self.sp {
            return Err(VmError::StackOverflow);
        }

        let value = self.memory[self.sp - value_size..self.sp].to_vec();
        let destination = self.sp - destination_size;
        self.memory
            .copy_within(destination..self.sp, destination + value_size);
        self.memory[destination..destination + value_size].copy_from_slice(&value);
        self.sp += value_size;
        Ok(())
    }

    fn jump_if(
        condition: bool,
        target: InstrArg,
        next: Option<InstrId>,
    ) -> Result<Option<InstrId>, VmError> {
        if condition {
            Ok(Some(target.instr()?))
        } else {
            Ok(next)
        }
    }

    fn arg_size(&self, arg: InstrArg) -> Result<usize, VmError> {
        usize::try_from(arg.int()?).map_err(|_| VmError::InvalidArgument("non-negative size"))
    }

    fn ensure_address(&self, address: Address, size: usize) -> Result<(), VmError> {
        self.read_memory(address, size).map(|_| ())
    }

    fn read_c_string(&self, address: Address) -> Result<String, VmError> {
        let size = if self.checked {
            self.get_reference(address)?.size
        } else {
            self.memory.len().saturating_sub(address)
        };
        let address = self.resolve(address, size, false)?;
        self.ensure_address(address, 1)?;
        let end = self.memory[address..address + size]
            .iter()
            .position(|byte| *byte == 0)
            .map(|offset| address + offset)
            .ok_or(VmError::InvalidAddress(address))?;
        Ok(String::from_utf8_lossy(&self.memory[address..end]).into_owned())
    }

    fn read_input_word(&mut self) -> String {
        while self
            .input
            .get(self.input_pos)
            .is_some_and(|value| value.is_whitespace())
        {
            self.input_pos += 1;
        }

        let start = self.input_pos;
        while self
            .input
            .get(self.input_pos)
            .is_some_and(|value| !value.is_whitespace())
        {
            self.input_pos += 1;
        }

        self.input[start..self.input_pos].iter().collect()
    }

    fn read_input_line(&mut self) -> String {
        if self.input.get(self.input_pos) == Some(&'\n') {
            self.input_pos += 1;
        }

        let start = self.input_pos;
        while self
            .input
            .get(self.input_pos)
            .is_some_and(|value| *value != '\n')
        {
            self.input_pos += 1;
        }

        self.input[start..self.input_pos].iter().collect()
    }
}

pub fn mv_test() -> Result<Vm, VmError> {
    let mut vm = Vm::new();
    let value = vm.alloc_global(INT_SIZE)?;

    vm.instructions.add_instr_a(Opcode::PushCtA, value);
    vm.instructions.add_instr_i(Opcode::PushCtI, 3);
    vm.instructions.add_instr_i(Opcode::Store, INT_SIZE as i64);

    let loop_start = vm.instructions.add_instr_a(Opcode::PushCtA, value);
    vm.instructions.add_instr_i(Opcode::Load, INT_SIZE as i64);
    vm.instructions.add_instr_ext(ExtFunc::PutI);

    vm.instructions.add_instr_a(Opcode::PushCtA, value);
    vm.instructions.add_instr_a(Opcode::PushCtA, value);
    vm.instructions.add_instr_i(Opcode::Load, INT_SIZE as i64);
    vm.instructions.add_instr_i(Opcode::PushCtI, 1);
    vm.instructions.add_instr(Opcode::SubI);
    vm.instructions.add_instr_i(Opcode::Store, INT_SIZE as i64);

    vm.instructions.add_instr_a(Opcode::PushCtA, value);
    vm.instructions.add_instr_i(Opcode::Load, INT_SIZE as i64);
    vm.instructions.add_instr_target(Opcode::JtI, loop_start);
    vm.instructions.add_instr(Opcode::Halt);

    Ok(vm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_references_reject_neighbor_access_and_literal_writes() {
        for (index, readonly) in [(2, false), (-1, false), (0, true)] {
            let mut vm = Vm::new();
            vm.checked = true;
            vm.alloc_global(12).unwrap();
            vm.instructions.add_instr_ii(
                if readonly {
                    Opcode::ReadOnlyRef
                } else {
                    Opcode::GlobalRef
                },
                0,
                8,
            );
            vm.instructions.add_instr_i(Opcode::PushCtI, index);
            vm.instructions.add_instr_i(Opcode::Index, 4);
            vm.instructions.add_instr_i(Opcode::PushCtI, 99);
            vm.instructions.add_instr_i(Opcode::Store, 4);
            vm.instructions.add_instr(Opcode::Halt);
            assert_eq!(vm.run(), Err(VmError::Bounds));
            assert_eq!(vm.read_memory(8, 4).unwrap(), &[0; 4]);
        }
    }
    #[test]
    fn checked_strings_and_execution_are_bounded() {
        let mut vm = Vm::with_input("too long");
        vm.checked = true;
        vm.alloc_global(4).unwrap();
        vm.instructions.add_instr_ii(Opcode::GlobalRef, 0, 4);
        vm.instructions.add_instr_ext(ExtFunc::GetS);
        vm.instructions.add_instr(Opcode::Halt);
        assert_eq!(vm.run(), Err(VmError::Bounds));
        let mut vm = Vm::new();
        vm.checked = true;
        vm.fuel = 10;
        vm.instructions.add_instr_target(Opcode::Jmp, 0);
        assert_eq!(vm.run(), Err(VmError::ResourceLimit));
        assert_eq!(vm.alloc_global(usize::MAX), Err(VmError::GlobalOverflow));
    }

    #[test]
    fn stack_uses_each_types_exact_size() {
        let mut vm = Vm::new();

        vm.push_c(b'a').unwrap();
        vm.push_i(42).unwrap();
        vm.push_d(3.5).unwrap();
        vm.push_a(123).unwrap();

        assert_eq!(
            vm.stack_len(),
            CHAR_SIZE + INT_SIZE + DOUBLE_SIZE + ADDR_SIZE
        );
        assert_eq!(vm.pop_a().unwrap(), 123);
        assert_eq!(vm.pop_d().unwrap(), 3.5);
        assert_eq!(vm.pop_i().unwrap(), 42);
        assert_eq!(vm.pop_c().unwrap(), b'a');
    }

    #[test]
    fn manually_generated_loop_counts_down() {
        let mut vm = mv_test().unwrap();

        vm.run().unwrap();

        assert_eq!(vm.output(), "#3\n#2\n#1\n");
        assert_eq!(vm.stack_len(), 0);
    }

    #[test]
    fn call_enter_and_ret_restore_the_stack_frame() {
        let mut vm = Vm::new();

        vm.instructions.add_instr_i(Opcode::PushCtI, 7);
        let call = vm.instructions.add_instr(Opcode::Call);
        vm.instructions.add_instr_ext(ExtFunc::PutI);
        vm.instructions.add_instr(Opcode::Halt);

        let function = vm.instructions.add_instr_i(Opcode::Enter, 0);
        vm.instructions.add_instr_i(
            Opcode::PushFpAddr,
            -(ADDR_SIZE as i64 * 2 + INT_SIZE as i64),
        );
        vm.instructions.add_instr_i(Opcode::Load, INT_SIZE as i64);
        vm.instructions.add_instr_i(Opcode::PushCtI, 2);
        vm.instructions.add_instr(Opcode::MulI);
        vm.instructions
            .add_instr_ii(Opcode::Ret, INT_SIZE as i64, INT_SIZE as i64);

        vm.instructions.get_mut(call).unwrap().args[0] = InstrArg::Instr(function);
        vm.run().unwrap();

        assert_eq!(vm.output(), "#14\n");
        assert_eq!(vm.stack_len(), 0);
    }

    #[test]
    fn predefined_functions_read_and_write_vm_values() {
        let mut vm = Vm::with_input("12 2.5");

        vm.instructions.add_instr_ext(ExtFunc::GetI);
        vm.instructions.add_instr_ext(ExtFunc::PutI);
        vm.instructions.add_instr_ext(ExtFunc::GetD);
        vm.instructions.add_instr_ext(ExtFunc::PutD);
        vm.instructions.add_instr_ext(ExtFunc::Seconds);
        vm.instructions
            .add_instr_i(Opcode::Drop, DOUBLE_SIZE as i64);
        vm.instructions.add_instr(Opcode::Halt);

        vm.run().unwrap();

        assert_eq!(vm.output(), "#12\n#2.5\n");
        assert_eq!(vm.stack_len(), 0);

        let mut char_vm = Vm::with_input("Z");
        char_vm.instructions.add_instr_ext(ExtFunc::GetC);
        char_vm.instructions.add_instr_ext(ExtFunc::PutC);
        char_vm.instructions.add_instr(Opcode::Halt);
        char_vm.run().unwrap();

        assert_eq!(char_vm.output(), "#Z\n");
    }

    #[test]
    fn insert_duplicates_the_top_value_at_the_requested_depth() {
        let mut vm = Vm::new();

        vm.push_i(10).unwrap();
        vm.push_i(20).unwrap();
        vm.insert(INT_SIZE * 2, INT_SIZE).unwrap();

        assert_eq!(vm.pop_i().unwrap(), 20);
        assert_eq!(vm.pop_i().unwrap(), 10);
        assert_eq!(vm.pop_i().unwrap(), 20);
    }

    #[test]
    fn instruction_links_survive_insertion_and_deletion() {
        let mut instructions = Instructions::new();
        let first = instructions.add_instr(Opcode::Nop);
        let last = instructions.add_instr(Opcode::Halt);
        let inserted = instructions
            .add_instr_after(first, Opcode::PushCtI)
            .unwrap();

        assert_eq!(instructions.get(first).unwrap().next, Some(inserted));
        assert_eq!(instructions.get(inserted).unwrap().next, Some(last));
        assert_eq!(instructions.get(last).unwrap().last, Some(inserted));

        instructions.delete_instructions_after(inserted).unwrap();

        assert_eq!(instructions.last, Some(inserted));
        assert_eq!(instructions.get(inserted).unwrap().next, None);
        assert!(instructions.get(last).is_none());
    }
}
