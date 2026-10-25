# AtomC guest ABI (version 1)

The compiler uses the existing stack VM. Its new bounded reference instructions
are mandatory for compiled programs. The original hand-assembled demo remains
available to unit tests in legacy mode; loaded bytecode always uses checked mode.

## Values and storage

All multi-byte values are little endian. `char` is an unsigned byte, `int` is a
signed 32-bit integer, and `double` is IEEE 754 binary64. Scalar storage alignment
is respectively 1, 4, and 8 bytes. Integer arithmetic wraps at the operand width;
integer division by zero traps. Numeric conversions use Rust's defined numeric
cast behavior (float to integer saturates, NaN becomes zero). Logical and comparison
results are 32-bit integers. `&&` and `||` short circuit left to right.

Struct fields retain declaration order. Each starts at the next multiple of its
alignment. Struct alignment is the largest field alignment, and total size is
rounded to that alignment. Array stride is the padded element size. Empty,
incomplete, recursive-by-value and oversized structs are rejected. Zero-sized
array objects are rejected; an array parameter's omitted/declared dimension is
normalized to a reference to the caller's actual array.

## Calls

Arguments are evaluated and pushed left to right without inter-argument padding.
Scalars and structs are passed by value, arrays by an opaque 8-byte bounded
reference. The reference retains the actual length and write permission; it is
never a host pointer. Parameters are mutable copies, except array contents which
alias caller storage. Functions must be defined before use; self recursion works.

`Call` pushes an 8-byte return instruction index. `Enter(local_bytes)` pushes an
8-byte saved frame pointer, establishes FP immediately above it, then reserves
zeroed locals. Locals have aligned offsets relative to FP. An argument's offset is
`-16 - total_argument_bytes + bytes_of_preceding_arguments`. Operand values are
above the reserved local area and cannot pop into locals or control metadata.

`Ret(argument_bytes, result_bytes)` removes arguments and the frame and leaves
only the result on the caller's operand stack. Checked execution keeps authoritative
return addresses, frame bounds and caller state in VM-owned metadata. Guest writes
cannot change them. Local references expire on return. No source-level pointers,
reference returns, variadic calls or host ABI calls are supported. Falling through
a non-void function traps; void fallthrough returns normally.

The entry point is `int main()` or `void main()` without parameters. The bootstrap
calls it, discards an integer result, and halts. The CLI reports compilation/runtime
failure through its exit status; a guest's main return value is currently discarded.

## Objects and I/O

`GlobalRef(offset, size)` and `LocalRef(offset, size)` create bounded references.
`ArgRef` accesses only the current call's argument region. `ReadOnlyRef` creates a
read-only reference to literal bytes. `Index(stride)` rejects negative indices and
requires the entire element to fit in the parent reference. `Field(offset, size)`
narrows the parent reference to the field. `Load`/`Store` check the resulting
reference extent; stores also require write permission. String operations remain
within that extent. `get_s` rejects oversized input before modifying memory;
`put_s` requires a NUL terminator inside the object. String literals include NUL. String and character literals support the standard single-character escapes (`\n`, `\t`, `\0`, escaped quotes and backslashes, etc.).

The `io` built-in library exports `put_s`, `get_s`, `put_i`, `get_i`, `put_d`,
`get_d`, `put_c`, and `get_c`. Source programs must begin with `import io;` to use
them. `seconds()` remains a core built-in. Imports occur before all declarations,
are idempotent, and resolve only allowlisted built-in names. Imported functions
use unqualified names and cannot be redefined. Imports are source-level name
visibility, not bytecode permissions; loaded bytecode uses the same fixed external
allowlist. The optional `math` library adds checked scalar math functions; see
[the library reference](libraries.md) for signatures, domain rules, and rounding
behavior. Math calls use the same guest argument/result convention and do not
receive I/O or other host capabilities. Numeric output ends in newline; `put_s` adds newline;
`put_c` writes the byte value as a Unicode character encoded in UTF-8. Numeric input consumes a whitespace-delimited
token. Character input consumes the next UTF-8 character and rejects values above 255. String input reads a line,
skipping one pending newline, and includes a terminating NUL in the destination.
`seconds` returns elapsed VM runtime. The VM accepts supplied input and buffers
output; it does not access files, environment variables, shell commands, or process
creation. The trusted CLI alone reads explicitly named program files and stdin,
writes explicitly named bytecode output files, and prints guest output.

Limits: 32 KiB globals, 32 KiB stack, 256 calls, 65,536 live references, one million
instructions per run, and approximately 1 MiB buffered output (checked each
instruction). Compiler input is limited to 1 MiB, delimiter nesting to 64, statement nesting to 128, and
individual source segments between braces/semicolons to 256 tokens. These are
resource bounds, not a claim of a formally verified sandbox. Bytecode is untrusted
and must use the checked execution path.
