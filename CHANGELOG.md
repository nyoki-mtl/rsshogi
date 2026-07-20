# Changelog

All notable changes to rsshogi will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [1.0.1] - 2026-07-21

Distribution-only release. The library code, public API, and runtime behavior are
identical to 1.0.0.

### Added

- Python wheels for macOS x86_64 (Intel) in the standard `rsshogi` distribution.
- Python wheels for Linux aarch64 (manylinux) in the standard `rsshogi` distribution.

### Changed

- The Rust crate `rsshogi` is republished at 1.0.1 to keep the crate, the Python
  distributions, and the release tag on the same version. It contains no code changes.

### Compatibility

- The AVX2 distribution `rsshogi-avx2` keeps its x86_64-only platform set. AVX2 is an
  x86 instruction set, so there is no arm64 AVX2 build.

## [1.0.0] - 2026-07-19

### Added

- Rust crate `rsshogi` for board state, move generation, records, book formats, and training-data formats.
- Python distributions `rsshogi` and `rsshogi-avx2`. Both provide the `rsshogi` import package.
- Rust and Python documentation with runnable examples.

### Compatibility

- The standard and AVX2 Python distributions are mutually exclusive because both provide the same import package.

[Unreleased]: https://github.com/nyoki-mtl/rsshogi/compare/v1.0.1...HEAD
[1.0.1]: https://github.com/nyoki-mtl/rsshogi/releases/tag/v1.0.1
[1.0.0]: https://github.com/nyoki-mtl/rsshogi/releases/tag/v1.0.0
