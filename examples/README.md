# Examples

These examples are ordered from minimal to more feature-rich. Each `.cada`
program defines a `Main` entry point, transpiles with `cadar --write
--split-units`, and is exercised by the GNAT integration tests.

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

Expected stdout for each example lives in the matching `.stdout` file.
