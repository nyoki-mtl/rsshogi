# rsshogi workspace tasks
#
# The workspace contains two crates: `rsshogi` (core library) and
# `rsshogi-py` (PyO3 bindings). Recipes here are thin wrappers around cargo /
# mdBook for day-to-day development. `just check` delegates to the Makefile so
# local checks match CI coverage (Rust all-features lint/tests + Python checks).

default:
	@just --list

# Build all workspace crates
build:
	cargo build

# Build release binaries
release:
	cargo build --release

# Run all tests (nextest if available, plus doctests)
# data ecosystem は default-off feature のため all-features で実行する（make test / CI と同一）。
test:
	@if cargo nextest --version >/dev/null 2>&1; then \
		echo "[just] cargo-nextest detected: running nextest (tests) + cargo test (doctests)"; \
		cargo nextest run --workspace --tests --all-features; \
		cargo test --doc -p rsshogi --all-features; \
	else \
		cargo test --workspace --tests --all-features; \
		cargo test --doc -p rsshogi --all-features; \
	fi

# Tests for the core crate only（data 系を含め all-features で検証）
test-core:
	cargo test -p rsshogi --all-features

# Perft reference dataset (fast)
# --tests で doctest 対象を外す（default-off の data 系 doctest を compile しないため）。
perft-test:
	cargo test -p rsshogi --tests perft_reference_dataset_matches_expectations

# Perft startpos verification (slow / ignored tests)
perft-verify:
	cargo test -p rsshogi --test perft -- --ignored perft_startpos_depth_five_matches_reference
	cargo test -p rsshogi --release --test perft -- --ignored perft_startpos_depth_six_matches_reference

# Format only
fmt:
	cargo fmt --all

# Clippy (pedantic/nursery/cargo, warnings as errors)
clippy:
	cargo clippy --workspace --all-targets --all-features -- \
		-W clippy::pedantic -W clippy::nursery -W clippy::cargo \
		-A clippy::module_name_repetitions -A clippy::missing_panics_doc \
		-A clippy::missing_errors_doc \
		-D warnings

# Format + clippy together
lint:
	cargo fmt --all
	just clippy

# Benchmarks (criterion)
bench:
	cargo bench -p rsshogi

# Build Rustdoc + mdBook (bundles the TypeScript board asset first)
docs:
	cargo doc --no-deps --workspace
	just docs-prepare-assets
	mdbook build docs/book

# mdBook only (skip Rustdoc)
book:
	just docs-prepare-assets
	mdbook build docs/book

# Bundle the mdBook front-end asset (TypeScript -> ESM)
docs-prepare-assets:
	npx --yes esbuild \
		docs/book/src/assets/shogi-board.ts \
		--bundle \
		--format=iife \
		--global-name=RShogiBoard \
		--target=es2018 \
		--outfile=docs/book/src/assets/shogi-board.js \
		--minify

# Full pre-commit check (delegates to Makefile for CI parity)
check:
	make check

# Clean build artifacts
clean:
	cargo clean
