# cadar

`cadar` is a Rust implementation of a **CADA-to-Ada transpiler**.

**CADA** is a C-inspired surface syntax for Ada/SPARK with a bias toward modern
preferences: braces instead of `begin`/`end`, `fn` instead of `function`,
`->` for return types, and declaration order like `Integer Count = 0;`.

The goal is not to replace Ada semantics or the GNAT/SPARK toolchain. The goal
is to keep Ada/SPARK's meaning and analyzability while making the source syntax
feel closer to C, Rust, and other modern languages.

## Project Links

- Crate name: `cadar`
- GitHub: `git@github.com:ali-raheem/cadar.git`
- Runnable examples: [`examples/`](examples/)

## Quick Start

### Build from source

```bash
git clone git@github.com:ali-raheem/cadar.git
cd cadar
cargo build
```

### Transpile and run an example

This transpiles CADA into split Ada units, then builds and runs the result with
GNAT:

```bash
cargo run -- --write --split-units --out-dir build/hello examples/01_hello_world.cada
cd build/hello
gnatmake -q main.adb
./main
```

Expected output:

```text
Hello from CADA
```

### Install from crates.io

```bash
cargo install cadar
```

Then use it directly:

```bash
cadar --write --split-units --out-dir build/hello examples/01_hello_world.cada
```

## What Is Implemented

The current compiler is a real end-to-end pipeline:

- lexer, parser, AST, semantic validation, Ada lowering, pretty-printing, and CLI
- functions and procedures, including grouped parameter modes
- imports and `use`
- local declarations, constants, assignments, returns, and call statements
- control flow: `if`, `while`, `for`, `case`, and `null`
- packages, package bodies, and derived specs for body-only packages
- contracts with `requires(...)` and `ensures(...)`
- record types, enum types, and range subtypes
- qualified names and a small attribute surface such as `Integer.image(X)`
- aggregate output or split-unit Ada file emission
- GNAT-backed integration tests, including the repository examples

The examples in [`examples/`](examples/) are ordered from minimal to more
feature-rich and are intended to show the current usable subset.

## What Is Left To Do

Important remaining work includes:

- richer name resolution and more precise type checking
- arrays, aggregates, and more complete expression/type support
- named arguments and defaulted parameters
- more SPARK-oriented features such as assertions, loop invariants, and
  `Global`/`Depends`
- more Ada coverage: private/tagged types, generics, exceptions, and tasking
- tighter source mapping, better diagnostics, and more output/toolchain polish
- optional `gnatprove` integration

## Current Status

`cadar` transpiles CADA source into normal Ada, supports split-unit output, and
includes GNAT-backed compile-and-run integration tests. The current repository
already contains runnable examples that demonstrate the implemented language
surface and serve as regression coverage for the generated Ada.
