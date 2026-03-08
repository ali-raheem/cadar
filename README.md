# cadar

`cadar` is a Rust implementation of a **CADA-to-Ada transpiler**.

**CADA** is a C-inspired surface syntax for Ada/SPARK with a bias toward modern
preferences: braces instead of `begin`/`end`, `fn` instead of `function`,
`->` for return types, and declaration order like `Integer Count = 0;`.

This public alpha is aimed at a **documented, package-based subset** of
Ada/SPARK. The goal is not full language coverage. The goal is a reliable path
for real CADA programs that transpile to normal Ada and build with GNAT.

## Project Links

- Crate: `cadar`
- Repository: `https://github.com/ali-raheem/cadar`
- Examples: [`examples/`](examples/)

## Prerequisites

Public alpha is officially supported on **Ubuntu/Linux** with:

- Rust toolchain
- GNAT (`gnatmake`)
- `gprbuild`

`gnatprove` is **not** required for this alpha and is not wired into the CLI
yet.

## Quick Start

### Install from crates.io

```bash
cargo install cadar
```

### Build from source

```bash
git clone https://github.com/ali-raheem/cadar.git
cd cadar
cargo build
```

### Transpile and build an example

```bash
cadar --write --split-units --build --out-dir build/hello examples/01_hello_world.cada
cd build/hello
./main
```

If you want a GNAT project file for project-driven workflows, add
`--emit-project`. When `--emit-project` and `--build` are used together,
`cadar` builds with `gprbuild -P cadar.gpr`; otherwise `--build` uses
`gnatmake`.

## Public Alpha Contract

The supported path for this release is:

- package-based CADA programs
- `--write --split-units` output
- GNAT compile/run in CI on generated Ada
- one or more `.cada` input files per invocation

If your code stays within the documented subset below, `cadar` should be
treated as a real alpha tool. If it depends on broader Ada/SPARK coverage, it
is outside the alpha promise.

## Supported in Public Alpha

- lexer, parser, semantic validation, Ada lowering, pretty-printing, and CLI
- functions and procedures, including grouped parameter modes
- imports and `use`, including package aliases with `import ... as ...`
- local declarations, nested block locals, constants, assignments, and call
  statements
- control flow: `if`, `while`, `for`, `case`, `null`, `break`, `continue`, and
  procedure `return;`
- exceptions with `raise`, `try`, `catch`, and `catch (others)`
- assertions, loop invariants, loop variants, and SPARK-style `global(...)` /
  `depends(...)`
- packages, package bodies, package state, private package sections, private
  body helpers, and derived specs for body-only packages
- contracts with `requires(...)` and `ensures(...)`
- record types, enum types, range subtypes, arrays, indexing, slicing, nested
  aggregates, record aggregates, arrays of records, and arrays of arrays
- float and character literals
- array and string attributes such as `.first`, `.last`, `.length`, `.range`,
  and type images like `Integer.image(X)`
- named call arguments and defaulted parameters
- aggregate output and split-unit Ada emission
- optional `cadar.gpr` emission for split-unit project workflows

## Not in Public Alpha

- full Ada or full SPARK coverage
- top-level overload sets in split-unit output
- `gnatprove` integration
- prebuilt release binaries
- official Windows or macOS support
- broad advanced Ada features such as generics, tasking, tagged types, and
  richer exception support

## Known Limits

- split-unit output requires unique top-level Ada library unit names, so
  top-level overload sets must stay inside packages or use aggregate output
- identifiers that differ only by case are rejected because Ada treats them as
  the same name
- user-defined identifiers must avoid Ada reserved words such as `record`,
  `task`, or `end`
- external package-qualified references should be explicit: add `import P;` or
  `use P;` before referring to `P.X`
- external top-level subprogram calls should also be explicit: add
  `import Name;` before calling another library-unit subprogram
- `use` is for packages in the supported CADA surface; do not write `use Name;`
  for top-level subprograms or top-level types
- `import` / `use` should name packages or library units, not package members
- `import` / `use` clauses should stay at the top of a source file

## Examples

The repository includes 25 runnable examples in [`examples/`](examples/), from
minimal `Text_IO` programs through multi-package flows with contracts, package
state, nested aggregates, and CLI/text handling. These are part of the GNAT
integration suite and are the best reference for the supported subset.

## Reporting Issues

When filing bugs, include:

- the CADA source
- generated Ada if relevant
- `cadar --version`
- `gnatmake --version`
- `gprbuild --version`
- the exact command used

## Status

`cadar` is now a **source-first public alpha**: usable for real package-based
programs on the documented subset, with GNAT-backed integration coverage and a
clear boundary around unsupported features.
