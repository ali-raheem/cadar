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

This transpiles CADA into split Ada units and builds them with GNAT in one
command:

```bash
cargo run -- --write --split-units --build --out-dir build/hello examples/01_hello_world.cada
cd build/hello
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
cadar --write --split-units --build --out-dir build/hello examples/01_hello_world.cada
```

If you want a GNAT project file for `gprbuild`, `gnatprove`, or other project
driven tooling, add `--emit-project`. That writes `cadar.gpr` beside the
generated Ada units. When `--emit-project` and `--build` are used together,
`cadar` builds through `gprbuild -P cadar.gpr`; otherwise `--build` uses
`gnatmake`.

## What Is Implemented

The current compiler is a real end-to-end pipeline:

- lexer, parser, AST, semantic validation, Ada lowering, pretty-printing, and CLI
- one or more `.cada` input files per CLI invocation
- functions and procedures, including grouped parameter modes
- imports and `use`, including package aliases with `import ... as ...`
- local declarations, including nested control-flow block locals, constants,
  assignments, procedure `return;`, value returns, and call statements
- control flow: `if`, `while`, `for`, `case`, `null`, `break`, and `continue`
- exceptions with `raise`, `try`, `catch`, and `catch (others)`
- body assertions with `assert(...)`
- loop invariants and loop variants
- SPARK-style dataflow contracts with `global(...)` and `depends(...)`
- packages, package bodies, package-level object declarations, body-private
  helper subprograms, and derived specs for body-only packages
- contracts with `requires(...)` and `ensures(...)`
- record types, enum types, and range subtypes
- record aggregates, constrained arrays, array literals, indexing, slicing, and
  nested aggregates such as arrays of records and arrays of arrays
- float and character literals
- qualified names and array/string attributes such as `.first`, `.last`,
  `.length`, `.range`, and `Integer.image(X)`
- named call arguments and defaulted parameters
- aggregate output or split-unit Ada file emission
- optional `cadar.gpr` emission for split-unit GNAT project workflows
- GNAT-backed integration tests, including the repository examples and
  multi-file package graphs

The examples in [`examples/`](examples/) are ordered from minimal to more
feature-rich and are intended to show the current usable subset.

## Current Supported Use

Today, the most reliable path is:

- package-based CADA programs
- `--write --split-units` output
- GNAT compile/run in CI on the generated Ada

Important current limit:

- split-unit output requires unique top-level Ada library unit names, so
  top-level overload sets are not supported there yet; put overloads inside a
  package or use aggregate output instead
- identifiers that differ only by case are rejected, because Ada treats them as
  the same name
- user-defined identifiers must avoid Ada reserved words such as `record`,
  `task`, or `end`
- external package-qualified references should be explicit: add `import P;` or
  `use P;` before referring to `P.X`
- external top-level subprogram calls should also be explicit: add
  `import Name;` before calling another library-unit subprogram like `Name(...)`
- `use` is for packages in the supported CADA surface; do not write `use Name;`
  for top-level subprograms or top-level types
- `import` / `use` should name library units or packages, not package members
  like `Math.Add` or `State.Count`
- `import` / `use` clauses should stay at the top of a source file, before any
  top-level declarations

## What Is Left To Do

Important remaining work includes:

- richer name resolution and more precise type checking
- more complete expression and type coverage
- more SPARK-oriented features such as `Refined_Post`
- more Ada coverage: private/tagged types, generics, richer exception support,
  and tasking
- tighter source mapping, better diagnostics, and more output/toolchain polish
- optional `gnatprove` integration

## Current Status

`cadar` transpiles CADA source into normal Ada, supports split-unit output, and
includes GNAT-backed compile-and-run integration tests. The current repository
already contains runnable examples that demonstrate the implemented language
surface and serve as regression coverage for the generated Ada.
