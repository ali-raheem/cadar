# Changelog

## 0.2.0-alpha.1

First public alpha release.

This release defines the supported alpha contract:

- package-based CADA programs
- split-unit Ada output via `--write --split-units`
- GNAT and `gprbuild` workflows on Ubuntu/Linux
- documented subset coverage backed by repository examples and GNAT tests

Highlights:

- packages, package bodies, private package sections, and package state
- contracts, assertions, loop annotations, and SPARK-style dataflow clauses
- records, enums, ranges, arrays, nested aggregates, indexing, and slicing
- named arguments, default parameters, package aliases, and exception handling
- multi-file input, split-unit emission, optional `cadar.gpr`, and CLI build
  support

Known alpha limits:

- no top-level overload sets in split-unit output
- no `gnatprove` integration yet
- no prebuilt release binaries
- no promise of full Ada/SPARK coverage
