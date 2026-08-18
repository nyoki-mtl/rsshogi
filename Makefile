ifeq ($(OS),Windows_NT)
POWERSHELL := powershell -NoProfile -ExecutionPolicy Bypass -File
else
POWERSHELL := pwsh -NoProfile -File
endif

.PHONY: help format format-check lint lint-fix test test-rust py-format py-lint typecheck check check-public-export clean sync check-env setup-native

help:
	@echo "Usage: make [target]"
	@echo ""
	@echo "Available targets:"
	@echo "  format     - Format Rust + Python examples"
	@echo "  format-check - Check Rust + Python formatting without modifying files"
	@echo "  lint       - Lint Rust + Python examples"
	@echo "  lint-fix   - Auto-fix Python lint issues"
	@echo "  test       - Run core Rust tests and Python binding tests"
	@echo "  sync       - Sync Python dev dependencies (uv)"
	@echo "  py-format  - Format Python examples only"
	@echo "  py-lint    - Lint Python examples only"
	@echo "  typecheck  - Run Python type checks (ty)"
	@echo "  check-public-export - Dry-run public export hygiene checks"
	@echo "  check      - format-check, lint, test, typecheck"
	@echo "  clean      - Remove build and cache artifacts"
	@echo "  check-env  - Show native Windows tool status"
	@echo "  setup-native - Install/sync native Windows development tools"

format:
	cargo fmt --all
	uv run ruff format examples/python

format-check:
	cargo fmt --all -- --check
	uv run ruff format --check examples/python

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	uv run ruff check examples/python

lint-fix:
	uv run ruff check examples/python --fix

test:
	$(MAKE) test-rust
	uv run maturin develop -m crates/rsshogi-py/Cargo.toml
	uv run pytest tests/python

test-rust:
ifeq ($(OS),Windows_NT)
	$(POWERSHELL) scripts/windows/test-rust.ps1
else
	cargo check -p rsshogi --all-targets
	cargo test --doc -p rsshogi --all-features
	@if cargo nextest --version >/dev/null 2>&1; then \
		echo "[make] cargo-nextest detected: running nextest"; \
		cargo nextest run -p rsshogi --tests --all-features; \
	else \
		echo "[make] cargo-nextest not found: falling back to cargo test"; \
		cargo test -p rsshogi --tests --all-features; \
	fi
endif

py-format:
	uv run ruff format examples/python

py-lint:
	uv run ruff check examples/python

typecheck:
	uv run ty check tests/python/typecheck_record_api.py --extra-search-path crates/rsshogi-py/python --error unused-ignore-comment

check-public-export:
	$(POWERSHELL) scripts/check_public_export.ps1

check: format-check lint test typecheck

sync:
	uv sync --dev

clean:
ifeq ($(OS),Windows_NT)
	$(POWERSHELL) scripts/windows/clean.ps1
else
	cargo clean
	find . -type d -name "__pycache__" -exec rm -rf {} +
	find . -type f -name "*.pyc" -delete
	find . -type d -name ".pytest_cache" -exec rm -rf {} +
	find . -type d -name ".ruff_cache" -exec rm -rf {} +
endif

check-env:
ifeq ($(OS),Windows_NT)
	$(POWERSHELL) scripts/windows/check-env.ps1
else
	@command -v git
	@command -v rustup
	@command -v rustc
	@command -v cargo
	@command -v uv
	@command -v python
endif

setup-native:
ifeq ($(OS),Windows_NT)
	$(POWERSHELL) scripts/windows/setup-native.ps1
else
	@echo "setup-native is intended for Windows native development."
endif
