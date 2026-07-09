# JSON Explorer automation targets

CARGO_MANIFEST := src-tauri/Cargo.toml

.PHONY: install dev build test test-rust test-frontend lint fixture

install:
	pnpm install

dev:
	pnpm tauri dev

build:
	pnpm tauri build

test: test-rust test-frontend

test-rust:
	cargo test --manifest-path $(CARGO_MANIFEST) --workspace

test-frontend:
	pnpm test

lint:
	cargo clippy --manifest-path $(CARGO_MANIFEST) --workspace

# Generate large JSON fixture files for benchmarks (lands in M1):
#   make fixture ARGS="--size 2gb"
fixture:
	cargo run --release --manifest-path $(CARGO_MANIFEST) -p json-index --example gen_fixture -- $(ARGS)
