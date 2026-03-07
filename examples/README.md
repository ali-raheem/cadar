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

Expected stdout for each example lives in the matching `.stdout` file.
