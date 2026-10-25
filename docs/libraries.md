# Built-in libraries

Imports appear before every declaration and function definition:

```c
import io;
import math;

int main() {
    put_d(sqrt(9));
    put_i(abs(-7));
    return 0;
}
```

An import makes that library's functions visible under their unqualified names.
Both libraries are optional and independent. Duplicate imports are harmless.
Unknown library names, missing semicolons, imports inside functions, and imports
after declarations are compilation errors. An imported function cannot be
redefined. Without its library import, the same name can be used for a normal
user-defined function. The core `double seconds()` helper needs no import.

Imports resolve a fixed registry compiled into AtomC. They do not read source
files, search directories or environment variables, load native libraries, or
resolve arbitrary host symbols. Source-file modules and `#include` remain
unsupported. Imports control source-level visibility; bytecode has only the
allowlisted external IDs documented in [the bytecode format](bytecode.md).

## `io`

Existing programs using I/O must add `import io;` at the top of the file. All I/O
continues to use the streams supplied to the VM; there is no guest file API.

| Signature | Behavior |
| --- | --- |
| `void put_i(int value)` | Write a decimal integer and newline. |
| `int get_i()` | Read a whitespace-delimited integer. |
| `void put_d(double value)` | Write a number and newline. |
| `double get_d()` | Read a whitespace-delimited floating point number. |
| `void put_c(char value)` | Write the character, without adding newline. |
| `char get_c()` | Read the next UTF-8 character; values above 255 are rejected. |
| `void put_s(char text[])` | Write a NUL-terminated string and newline. |
| `void get_s(char text[])` | Read a line and store it with a NUL terminator. |

`put_s` searches only within the array reference's extent. `get_s` fails before
writing if the line plus its terminator does not fit. Literal strings are read
only. The existing guest bounds and execution limits still apply.

## `math`

| Signature | Behavior |
| --- | --- |
| `double sqrt(double x)` | Square root; x must be nonnegative. |
| `double pow(double base, double exponent)` | Real power, for operands with a finite real result. |
| `double sin(double x)` | Sine; x is in radians. |
| `double cos(double x)` | Cosine; x is in radians. |
| `double tan(double x)` | Tangent; x is in radians. |
| `double fabs(double x)` | Floating point absolute value. |
| `double floor(double x)` | Greatest integer value less than or equal to x. |
| `double ceil(double x)` | Least integer value greater than or equal to x. |
| `double round(double x)` | Round to nearest integer value; ties go away from zero. |
| `double exp(double x)` | Exponential with base e. |
| `double log(double x)` | Natural logarithm; x must be positive. |
| `double log10(double x)` | Base-10 logarithm; x must be positive. |
| `int abs(int x)` | Integer absolute value; the minimum i32 value is rejected. |

The existing numeric conversions apply to function arguments. For example,
`sqrt(9)` converts the integer to double; `abs(-2.5)` converts to int before
calling `abs` and therefore returns 2. Arrays and structs are not numeric inputs.

Floating point math rejects non-finite inputs and results. Invalid real domains
(such as `sqrt(-1)` or `pow(-2, 0.5)`), overflow to infinity, and `abs(-2147483648)`
stop execution with `math domain or range error`. Underflow to finite zero is
allowed. Math functions perform no I/O and do not inspect host state. They use
Rust's floating point operations; transcendental results are approximate and are
not promised to be bit-identical across platforms. No math constants are exported
yet.

Run the example interactively:

```sh
cargo run -- examples/math.atomc
```

Or supply a value through stdin:

```sh
printf '9\n' | cargo run --quiet -- examples/math.atomc
```

It prints a square root of 3 and a square of 81. Source and bytecode execution
expose the same math functions and domain checks.
