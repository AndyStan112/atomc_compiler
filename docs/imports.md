# Custom source imports

AtomC supports quoted source-file imports alongside its built-in libraries:

```c
import io;
import "lib/geometry.atomc";

int main() {
    struct Circle circle;
    circle.radius = get_d();
    put_d(circle_area(circle));
    put_i(calculations);
    return 0;
}
```

The library can define structs, globals, and functions, and can import its own
dependencies:

```c
// lib/geometry.atomc
import math;

struct Circle {
    double radius;
};

int calculations;

double circle_area(struct Circle circle) {
    calculations = calculations + 1;
    return 3.141592653589793 * pow(circle.radius, 2);
}
```

## Resolution and visibility

All imports must appear before the file's declarations. Bare names (`import io;`,
`import math;`) select built-ins. Quoted paths select UTF-8 `.atomc` source files.
Paths use `/`, contain no escape sequences, and are relative to the canonical
parent directory of the importing file, regardless of the process working
directory. Absolute import paths are rejected.

The entry file's canonical parent directory is the project root for this version.
Every dependency's canonical path must remain inside that root. Subdirectories
are supported; `../` is allowed between directories inside the root. Symlinks
resolving outside the root are rejected. Symlinks resolving to the same file
inside the root count as one canonical dependency. Imported inputs must be
regular files, not directories, devices, or pipes.

The compiler visits dependencies in import declaration order and emits each file
once, before the files that import it. Repeated imports and diamond dependencies
are deduplicated by canonical path. Circular imports are rejected with a chain
of filenames. Definitions within a file must still precede their uses.

All files share one global namespace. Imported functions, structs, globals, and
built-ins are visible throughout the compilation unit under unqualified names.
In particular, `import math;` in a dependency makes math built-ins available to
the rest of the compilation unit as well. Duplicate symbol definitions, including
conflicts with imported built-ins, are errors. Only the entry file may define
`main()`, and it must still be `int main()` or `void main()` without parameters.

Each file is lexed separately. Explicit module boundary tokens prevent incomplete
syntax in one file from continuing into another. Front-end errors include the
source filename, line, and position; import resolution errors identify the import
site. Files remain in memory until analysis and instruction generation finish,
so token text never depends on leaked or invalid source references.

## Limits and execution

A compilation accepts at most **128 distinct files**, **32 import levels**, and
**1 MiB of combined source bytes**, including the entry file. Repeated canonical
files count once. Existing expression, statement, memory, and execution limits
continue to apply across the combined program. Embedded NUL bytes are rejected.

File access happens only during compilation. The VM does not resolve imports or
receive module paths, source contents, or file handles as runtime capabilities.
Imported functions compile to ordinary guest instructions. Existing guest memory
checks and host isolation remain in effect.

Bytecode embeds all compiled dependencies without changing the bytecode format.
It runs even after the source files are removed or moved:

```sh
cargo run -- --compile examples/modules/main.atomc /tmp/circle.atbc
printf '2\n' | cargo run --quiet -- --run-bytecode /tmp/circle.atbc
```

The output artifact must not already exist. Run the multi-file source example
interactively with:

```sh
cargo run -- examples/modules/main.atomc
```

The compiler's in-memory `compile(&str)` entry point remains filesystem-free and
rejects quoted imports. The file entry point `compile_file(&Path)` performs the
resolution described here and is used by both CLI compilation modes.

Namespaces, import aliases, selective exports, private declarations, package
search paths, remote dependencies, native libraries, separate compilation,
function prototypes, and `#include` are not implemented.
