# AtomC bytecode version 1

All integers and floating point bit patterns use little endian. Files contain:

| Field | Width | Value |
| --- | --- | --- |
| Magic | 8 bytes | `ATOMCBC` followed by NUL |
| Version | u16 | 1 |
| Flags | u16 | 0 |
| Global data length | u32 | 0–32768 |
| Instruction count | u32 | 1–65536 |
| Initial global bytes | length above | Includes zeroed objects, padding, and string literals |
| Instructions | count × 18 bytes | u16 opcode, u64 operand 0, u64 operand 1 |

Entry is instruction zero. Targets are zero-based instruction indices, never byte
offsets. Links are reconstructed in file order. Trailing bytes, truncation,
unknown versions, flags, opcodes and external IDs, invalid targets, nonzero unused
operands, oversized data and invalid size operands are rejected. A file is at most
1,212,436 bytes. There are no host pointers, native function addresses, external
library names or environment imports.

Operand signatures below: `i` is signed i64 (sizes must also pass nonnegative
range validation), `d` is an IEEE 754 binary64 bit pattern, `t` is an instruction
index, `e` is an external function ID. An empty signature requires both operands
to be zero; one-operand instructions require the second operand to be zero.
`PushCtI` and `PushCtC` require values in i32 and u8 range respectively.

The decoder validates structure and operands. It does not prove stack types or
source-level object provenance: runtime checks remain mandatory. Loaded files
always execute in checked mode with bounded memory, shadow call metadata and
resource limits described in [the guest ABI](guest-abi.md). Invalid stack behavior
or references fail with a VM error. Bytecode must never select legacy VM mode.

External IDs: 0 `put_s`, 1 `get_s`, 2 `put_i`, 3 `get_i`, 4 `put_d`, 5 `get_d`,
6 `put_c`, 7 `get_c`, 8 `seconds`. These pre-import IDs are unchanged, so existing
version 1 files continue to load. The math library appends the following IDs:

| ID | External |
| --- | --- |
| 9 | `sqrt` |
| 10 | `pow` |
| 11 | `sin` |
| 12 | `cos` |
| 13 | `tan` |
| 14 | `fabs` |
| 15 | `floor` |
| 16 | `ceil` |
| 17 | `round` |
| 18 | `exp` |
| 19 | `log` |
| 20 | `log10` |
| 21 | `abs` |

No other external calls exist. Older decoders reject the new math IDs as unknown.
Imports select source-level declarations and add no bytecode records or native
symbols; both signatures and external IDs come from one built-in registry.
Math uses the same runtime domain checks after deserialization. Custom source
imports compile into ordinary guest functions and data. Their source paths and
import graphs are not encoded, and execution requires no source files or runtime
file access. The bytecode format version and external IDs remain unchanged.

Opcode IDs are explicit and must remain stable within version 1:

| ID | Opcode | Operands |
| --- | --- | --- |
| 1 | `Trap` | `` |
| 2 | `GlobalRef` | `ii` |
| 3 | `LocalRef` | `ii` |
| 4 | `ArgRef` | `ii` |
| 5 | `Index` | `i` |
| 6 | `Field` | `ii` |
| 7 | `ReadOnlyRef` | `ii` |
| 8 | `AddC` | `` |
| 9 | `AddD` | `` |
| 10 | `AddI` | `` |
| 11 | `AndA` | `` |
| 12 | `AndC` | `` |
| 13 | `AndD` | `` |
| 14 | `AndI` | `` |
| 15 | `Call` | `t` |
| 16 | `CallExt` | `e` |
| 17 | `CastCD` | `` |
| 18 | `CastCI` | `` |
| 19 | `CastDC` | `` |
| 20 | `CastDI` | `` |
| 21 | `CastIC` | `` |
| 22 | `CastID` | `` |
| 23 | `DivC` | `` |
| 24 | `DivD` | `` |
| 25 | `DivI` | `` |
| 26 | `Drop` | `i` |
| 27 | `Enter` | `i` |
| 28 | `EqA` | `` |
| 29 | `EqC` | `` |
| 30 | `EqD` | `` |
| 31 | `EqI` | `` |
| 32 | `GreaterC` | `` |
| 33 | `GreaterD` | `` |
| 34 | `GreaterI` | `` |
| 35 | `GreaterEqC` | `` |
| 36 | `GreaterEqD` | `` |
| 37 | `GreaterEqI` | `` |
| 38 | `Halt` | `` |
| 39 | `Insert` | `ii` |
| 40 | `JfA` | `t` |
| 41 | `JfC` | `t` |
| 42 | `JfD` | `t` |
| 43 | `JfI` | `t` |
| 44 | `Jmp` | `t` |
| 45 | `JtA` | `t` |
| 46 | `JtC` | `t` |
| 47 | `JtD` | `t` |
| 48 | `JtI` | `t` |
| 49 | `LessC` | `` |
| 50 | `LessD` | `` |
| 51 | `LessI` | `` |
| 52 | `LessEqC` | `` |
| 53 | `LessEqD` | `` |
| 54 | `LessEqI` | `` |
| 55 | `Load` | `i` |
| 56 | `MulC` | `` |
| 57 | `MulD` | `` |
| 58 | `MulI` | `` |
| 59 | `NegC` | `` |
| 60 | `NegD` | `` |
| 61 | `NegI` | `` |
| 62 | `NeqA` | `` |
| 63 | `NeqC` | `` |
| 64 | `NeqD` | `` |
| 65 | `NeqI` | `` |
| 66 | `Nop` | `` |
| 67 | `NotA` | `` |
| 68 | `NotC` | `` |
| 69 | `NotD` | `` |
| 70 | `NotI` | `` |
| 71 | `OrA` | `` |
| 72 | `OrC` | `` |
| 73 | `OrD` | `` |
| 74 | `OrI` | `` |
| 75 | `PushCtC` | `i` |
| 76 | `PushCtD` | `d` |
| 77 | `PushCtI` | `i` |
| 78 | `Ret` | `ii` |
| 79 | `Store` | `i` |
| 80 | `SubC` | `` |
| 81 | `SubD` | `` |
| 82 | `SubI` | `` |
