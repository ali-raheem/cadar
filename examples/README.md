# Examples

These examples are ordered from minimal to more feature-rich. Each `.cada`
program defines a `Main` entry point, transpiles with `cadar --write
--split-units`, and is exercised by the GNAT integration tests. Together they
represent the practical public-alpha subset that `cadar` is expected to handle
reliably.

For split-unit output, keep overload sets inside packages rather than as
top-level subprograms, since Ada library-unit filenames must stay unique.
When one top-level subprogram calls another, add `import Name;` explicitly in
the calling source so visibility matches Ada library-unit behavior.

- `01_hello_world.cada`: smallest runnable program using `Text_IO`.
- `02_control_flow.cada`: `while`, `for`, `if`, `case`, and boolean logic.
- `03_packages_and_contracts.cada`: package spec/body plus pre/postconditions.
- `04_types_and_ranges.cada`: package-contained record, enum, range subtype, and
  qualified names.
- `05_body_only_package.cada`: body-only package with derived Ada spec output.
- `06_arrays.cada`: constrained arrays, aggregates, indexing, assignment, and
  `length`.
- `07_record_aggregates.cada`: record construction with named fields and normal
  field access.
- `08_named_args_and_defaults.cada`: named actual parameters and defaulted
  formals rendered to Ada `=>` and `:=`.
- `09_asserts.cada`: statement-level assertions lowered to Ada `pragma Assert`.
- `10_loop_annotations.cada`: loop invariants and variants lowered to Ada loop
  pragmas for `while` and `for`.
- `11_dataflow_contracts.cada`: SPARK-style `global(...)` and `depends(...)`
  clauses lowered to Ada dataflow aspects.
- `12_package_state.cada`: package-level objects used as real mutable package
  state across calls.
- `13_private_package_helpers.cada`: package-body helper subprograms that stay
  private to the implementation while public operations remain in the spec.
- `14_nested_block_locals.cada`: local declarations inside `if` and `while`
  bodies, lowered through Ada `declare` blocks.
- `15_float_and_character_literals.cada`: native decimal `Float` literals and
  single-quoted `Character` literals.
- `16_loop_control.cada`: procedure `return;` plus loop `break;` and
  `continue;`, lowered to Ada `return;`, `exit;`, and `continue;`.
- `17_arrays_of_records.cada`: package-based arrays of records, nested
  aggregates, indexing, and qualified type construction.
- `18_matrix_trace.cada`: nested array types, matrix-style nested array
  aggregates, chained indexing, and loop-based computation.
- `19_inventory_report.cada`: a larger package-based flow with shared package
  types across packages, record arrays, nested aggregates, named arguments,
  defaults, loops, and cross-package calls.
- `20_stateful_contracts.cada`: package state plus contracts shared across
  packages, including record types, pre/postconditions, dataflow aspects, and
  reporting over public package state.
- `21_alert_pipeline.cada`: a deeper package graph with shared record arrays,
  package state accumulation, cross-package reporting, contracts, and a final
  multi-package check in `Main`.
- `22_private_package_sections.cada`: package specs with a `private { ... }`
  section, including hidden types, hidden state, and private helper
  declarations consumed by the package body.
- `23_import_aliases.cada`: package aliases with `import ... as ...`, including
  aliases for external Ada packages like `Ada.Command_Line`.
- `24_exceptions.cada`: `raise`, `try`, `catch`, and `catch (others)` lowered
  to Ada exception handling blocks.
- `25_string_slices.cada`: string slicing with `.first`, `.last`, `.length`,
  package aliases, and a simple command-line driven text helper.

Expected stdout for each example lives in the matching `.stdout` file.
