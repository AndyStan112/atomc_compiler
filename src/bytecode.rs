//! Versioned, canonical bytecode with no native pointers or host function names.
use crate::compiler::Program;
use crate::vm::{ExtFunc, InstrArg as A, Instructions, Opcode as O, GLOBAL_SIZE, STACK_SIZE};
use std::collections::BTreeMap;

pub const MAX_INSTRUCTIONS: usize = 65536;
pub const MAX_BYTECODE: usize = 20 + GLOBAL_SIZE + 18 * MAX_INSTRUCTIONS;
const MAGIC: &[u8; 8] = b"ATOMCBC\0";
// IDs are explicit wire values. Do not reorder or reuse existing IDs.
const OPS: &[(u16, O, &str)] = &[
    (1, O::Trap, ""),
    (2, O::GlobalRef, "ii"),
    (3, O::LocalRef, "ii"),
    (4, O::ArgRef, "ii"),
    (5, O::Index, "i"),
    (6, O::Field, "ii"),
    (7, O::ReadOnlyRef, "ii"),
    (8, O::AddC, ""),
    (9, O::AddD, ""),
    (10, O::AddI, ""),
    (11, O::AndA, ""),
    (12, O::AndC, ""),
    (13, O::AndD, ""),
    (14, O::AndI, ""),
    (15, O::Call, "t"),
    (16, O::CallExt, "e"),
    (17, O::CastCD, ""),
    (18, O::CastCI, ""),
    (19, O::CastDC, ""),
    (20, O::CastDI, ""),
    (21, O::CastIC, ""),
    (22, O::CastID, ""),
    (23, O::DivC, ""),
    (24, O::DivD, ""),
    (25, O::DivI, ""),
    (26, O::Drop, "i"),
    (27, O::Enter, "i"),
    (28, O::EqA, ""),
    (29, O::EqC, ""),
    (30, O::EqD, ""),
    (31, O::EqI, ""),
    (32, O::GreaterC, ""),
    (33, O::GreaterD, ""),
    (34, O::GreaterI, ""),
    (35, O::GreaterEqC, ""),
    (36, O::GreaterEqD, ""),
    (37, O::GreaterEqI, ""),
    (38, O::Halt, ""),
    (39, O::Insert, "ii"),
    (40, O::JfA, "t"),
    (41, O::JfC, "t"),
    (42, O::JfD, "t"),
    (43, O::JfI, "t"),
    (44, O::Jmp, "t"),
    (45, O::JtA, "t"),
    (46, O::JtC, "t"),
    (47, O::JtD, "t"),
    (48, O::JtI, "t"),
    (49, O::LessC, ""),
    (50, O::LessD, ""),
    (51, O::LessI, ""),
    (52, O::LessEqC, ""),
    (53, O::LessEqD, ""),
    (54, O::LessEqI, ""),
    (55, O::Load, "i"),
    (56, O::MulC, ""),
    (57, O::MulD, ""),
    (58, O::MulI, ""),
    (59, O::NegC, ""),
    (60, O::NegD, ""),
    (61, O::NegI, ""),
    (62, O::NeqA, ""),
    (63, O::NeqC, ""),
    (64, O::NeqD, ""),
    (65, O::NeqI, ""),
    (66, O::Nop, ""),
    (67, O::NotA, ""),
    (68, O::NotC, ""),
    (69, O::NotD, ""),
    (70, O::NotI, ""),
    (71, O::OrA, ""),
    (72, O::OrC, ""),
    (73, O::OrD, ""),
    (74, O::OrI, ""),
    (75, O::PushCtC, "i"),
    (76, O::PushCtD, "d"),
    (77, O::PushCtI, "i"),
    (78, O::Ret, "ii"),
    (79, O::Store, "i"),
    (80, O::SubC, ""),
    (81, O::SubD, ""),
    (82, O::SubI, ""),
];
const EXTERNALS: &[ExtFunc] = &[
    ExtFunc::PutS,
    ExtFunc::GetS,
    ExtFunc::PutI,
    ExtFunc::GetI,
    ExtFunc::PutD,
    ExtFunc::GetD,
    ExtFunc::PutC,
    ExtFunc::GetC,
    ExtFunc::Seconds,
];

pub fn encode(program: &Program) -> Result<Vec<u8>, String> {
    if program.globals.len() > GLOBAL_SIZE {
        return Err("too many global bytes".into());
    }
    let mut order = Vec::new();
    let mut ids = BTreeMap::new();
    let mut next = program.instructions.first;
    while let Some(id) = next {
        if order.len() >= MAX_INSTRUCTIONS || ids.insert(id, order.len()).is_some() {
            return Err("invalid instruction chain".into());
        }
        let instr = program.instructions.get(id).ok_or("missing instruction")?;
        order.push(instr);
        next = instr.next;
    }
    if order.is_empty() {
        return Err("empty bytecode".into());
    }
    let mut out = Vec::new();
    out.extend(MAGIC);
    out.extend(1u16.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out.extend((program.globals.len() as u32).to_le_bytes());
    out.extend((order.len() as u32).to_le_bytes());
    out.extend(&program.globals);
    for instr in order {
        let (id, _, sig) = OPS
            .iter()
            .find(|(_, op, _)| *op == instr.opcode)
            .ok_or("unsafe or unsupported opcode")?;
        let mut values = [0u64; 2];
        for (i, tag) in sig.bytes().enumerate() {
            values[i] = match (tag, instr.args[i]) {
                (b'i', A::Int(n)) => n as u64,
                (b'd', A::Double(n)) => n.to_bits(),
                (b't', A::Instr(n)) => {
                    *ids.get(&n).ok_or("target outside instruction chain")? as u64
                }
                (b'e', A::ExtFunc(f)) => EXTERNALS
                    .iter()
                    .position(|e| *e == f)
                    .ok_or("unknown external")? as u64,
                _ => return Err("invalid instruction operand type".into()),
            };
        }
        for i in sig.len()..2 {
            if instr.args[i] != A::None {
                return Err("unused operand is not empty".into());
            }
        }
        out.extend(id.to_le_bytes());
        out.extend(values[0].to_le_bytes());
        out.extend(values[1].to_le_bytes());
    }
    // Apply the same structural validation to both newly generated and loaded code.
    decode(&out)?;
    Ok(out)
}

pub fn decode(bytes: &[u8]) -> Result<Program, String> {
    if bytes.len() < 20 || bytes.len() > MAX_BYTECODE || &bytes[..8] != MAGIC {
        return Err("invalid bytecode header or size".into());
    }
    let u16_at = |at| u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
    let u32_at = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    if u16_at(8) != 1 || u16_at(10) != 0 {
        return Err("unsupported bytecode version or flags".into());
    }
    let globals = u32_at(12);
    let count = u32_at(16);
    if globals > GLOBAL_SIZE
        || count == 0
        || count > MAX_INSTRUCTIONS
        || bytes.len() != 20 + globals + count * 18
    {
        return Err("invalid bytecode lengths".into());
    }
    let mut code = Instructions::new();
    for i in 0..count {
        let at = 20 + globals + i * 18;
        let id = u16_at(at);
        let (_, op, sig) = OPS
            .iter()
            .find(|(wire, _, _)| *wire == id)
            .ok_or("unknown opcode")?;
        let mut args = [A::None; 2];
        for j in 0..2 {
            let n = u64::from_le_bytes(bytes[at + 2 + j * 8..at + 10 + j * 8].try_into().unwrap());
            args[j] = match sig.as_bytes().get(j) {
                Some(b'i') => A::Int(n as i64),
                Some(b'd') => A::Double(f64::from_bits(n)),
                Some(b't') => {
                    if n >= count as u64 {
                        return Err("invalid branch target".into());
                    }
                    A::Instr(n as usize)
                }
                Some(b'e') => A::ExtFunc(
                    *EXTERNALS
                        .get(usize::try_from(n).map_err(|_| "invalid external")?)
                        .ok_or("invalid external")?,
                ),
                None if n == 0 => A::None,
                _ => return Err("invalid or noncanonical operand".into()),
            };
        }
        validate(*op, args, globals)?;
        let slot = code.add_instr(*op);
        code.get_mut(slot).unwrap().args = args;
    }
    Ok(Program {
        instructions: code,
        globals: bytes[20..20 + globals].to_vec(),
    })
}
fn validate(op: O, args: [A; 2], globals: usize) -> Result<(), String> {
    let integer = |i| {
        if let A::Int(n) = args[i] {
            Ok(n)
        } else {
            Err("integer operand required".to_string())
        }
    };
    let size = |i| -> Result<usize, String> {
        usize::try_from(integer(i)?)
            .ok()
            .filter(|n| *n <= STACK_SIZE)
            .ok_or("operand size out of range".into())
    };
    match op {
        O::GlobalRef | O::ReadOnlyRef => {
            let offset = size(0)?;
            let len = size(1)?;
            if offset + len > globals || len == 0 {
                return Err("global reference outside data".into());
            }
        }
        O::LocalRef => {
            size(0)?;
            if size(1)? == 0 {
                return Err("empty local reference".into());
            }
        }
        O::ArgRef => {
            if !(-((STACK_SIZE + 16) as i64)..=-16).contains(&integer(0)?) || size(1)? == 0 {
                return Err("invalid argument reference".into());
            }
        }
        O::Field | O::Ret | O::Insert => {
            size(0)?;
            size(1)?;
        }
        O::Index | O::Load | O::Store => {
            if size(0)? == 0 {
                return Err("zero operand size".into());
            }
        }
        O::Drop | O::Enter => {
            size(0)?;
        }
        O::PushCtC => {
            if !(0..=255).contains(&integer(0)?) {
                return Err("invalid char constant".into());
            }
        }
        O::PushCtI => {
            i32::try_from(integer(0)?).map_err(|_| "invalid int constant")?;
        }
        _ => (),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::compile;
    #[test]
    fn round_trip_is_canonical_and_executable() {
        let p = compile("int twice(int x){return x*2;}int main(){put_i(twice(get_i()));return 0;}")
            .unwrap();
        let bytes = encode(&p).unwrap();
        let p = decode(&bytes).unwrap();
        assert_eq!(encode(&p).unwrap(), bytes);
        let mut vm = p.into_vm("21").unwrap();
        vm.run().unwrap();
        assert_eq!(vm.output(), "42\n");
    }
    #[test]
    fn rejects_every_truncation_trailing_data_and_bad_header() {
        let bytes = encode(&compile("int main(){return 0;}").unwrap()).unwrap();
        for len in 0..bytes.len() {
            assert!(decode(&bytes[..len]).is_err());
        }
        let mut b = bytes.clone();
        b.push(0);
        assert!(decode(&b).is_err());
        for at in [0, 8, 10, 12, 16, 20] {
            let mut b = bytes.clone();
            b[at] = 255;
            assert!(decode(&b).is_err(), "offset {at}");
        }
    }
    #[test]
    fn rejects_invalid_targets_ext_ids_and_sizes() {
        let bytes = encode(&compile("int main(){put_i(1);return 0;}").unwrap()).unwrap();
        let mut bad = bytes.clone();
        bad[22..30].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(decode(&bad).is_err());
        let mut bad = bytes.clone();
        let call = OPS.iter().find(|(_, op, _)| *op == O::CallExt).unwrap().0;
        bad[20..22].copy_from_slice(&call.to_le_bytes());
        bad[22..30].copy_from_slice(&99u64.to_le_bytes());
        assert!(decode(&bad).is_err());
        let mut bad = bytes;
        let load = OPS.iter().find(|(_, op, _)| *op == O::Load).unwrap().0;
        bad[20..22].copy_from_slice(&load.to_le_bytes());
        bad[22..30].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(decode(&bad).is_err());
    }
    #[test]
    fn arbitrary_bytes_never_panic() {
        let mut state = 1u64;
        for len in 0..1024 {
            let mut bytes = vec![0; len];
            for b in &mut bytes {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                *b = (state >> 32) as u8;
            }
            assert!(decode(&bytes).is_err());
        }
    }
}
