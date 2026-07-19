# Changelog

All notable changes to rsshogi will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [1.0.0] - 2026-07-19

### Added

- Rust crate `rsshogi` for board state, move generation, records, book formats, and training-data formats.
- Python distributions `rsshogi` and `rsshogi-avx2`. Both provide the `rsshogi` import package.
- Rust and Python documentation with runnable examples.

### Compatibility

- The standard and AVX2 Python distributions are mutually exclusive because both provide the same import package.

[Unreleased]: https://github.com/nyoki-mtl/rsshogi/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/nyoki-mtl/rsshogi/releases/tag/v1.0.0
