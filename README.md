# AtomC

An experimental Rust compiler and stack VM for a C subset. Requires a Rust
toolchain supporting edition 2024. No third-party dependencies are needed.

Run a real program with standard input and output:

```sh
cargo run -- examples/sum.atomc
```

The example starts with `import io;`, which makes the input/output functions available.
The program prints a prompt; enter two integers, for example `20 22`. It prints
`Sum:` followed by `42`. Piped input works too:

```sh
printf '20 22\n' | cargo run --quiet -- examples/sum.atomc
```

Compile once and execute the bytecode:

```sh
cargo run -- --compile examples/sum.atomc /tmp/sum.atbc
printf '20 22\n' | cargo run --quiet -- --run-bytecode /tmp/sum.atbc
```

The output file must not already exist. With no arguments, AtomC compiles and
runs the original `data/main.atomc` (which does not print output). Errors go to
stderr with nonzero exit status. Guest output alone goes to stdout.

Built-in libraries are imported at the start of a source file:

```c
import io;

int main() {
    put_i(get_i());
    return 0;
}
```

I/O functions require `import io;`; existing source files using them need this
line. Duplicate imports are harmless. Unknown libraries, imports inside functions,
and imports after declarations are errors. Imports select a fixed built-in
registry; they do not load files, native libraries, or source modules. The runtime
helper `double seconds()` remains available without an import.

Supported features include scalar numeric types and casts, globals and scoped
locals, fixed arrays, nested structs, functions and recursion, `if`/`else`,
`while`, `for`, `break`, and short-circuit logic. Definitions precede use. Arrays
passed to functions retain their actual bounds. Guest objects start zeroed.
Pointers, dynamic allocation, preprocessors, host library imports, and struct
return values are unsupported. Input/output uses only explicitly supplied
streams; guest code cannot read environment variables or files or execute
processes. Execution has memory, instruction, call-depth and output limits.

See [guest ABI and limits](docs/guest-abi.md) and [bytecode format](docs/bytecode.md).
The original hand-assembled VM demo remains in unit tests. The reference baseline
is commit `82606ae`; implementation changes are committed in functional steps.

```sh
cargo test
cargo test --release
```
