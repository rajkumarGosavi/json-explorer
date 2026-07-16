# JSON Explorer automation targets
ifeq ($(OS),Windows_NT)
SHELL := C:/Program Files/Git/bin/bash.exe
endif

CARGO_MANIFEST := src-tauri/Cargo.toml

.PHONY: install dev build test test-rust test-frontend lint fixture bench bench-index bench-search

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

bench: bench-index bench-search

# Times build_index against a fixture (see `make fixture`):
#   make bench-index ARGS="samples/5MB.json"
bench-index:
	cargo run --release --manifest-path $(CARGO_MANIFEST) -p json-index --example bench_index -- $(ARGS)

# Times search_bytes (including node_at_offset path resolution) against a
# fixture with a query — watch per-hit time stay flat as file size grows:
#   make bench-search ARGS="samples/5MB.json fox"
#   make bench-search ARGS="samples/1GB.json fox"
bench-search:
	cargo run --release --manifest-path $(CARGO_MANIFEST) -p json-index --example bench_search -- $(ARGS)

clean:
	rm -rf src-tauri/target || true
	node -e "['dist','coverage','src-tauri/gen'].forEach(d=>require('fs').rmSync(d,{recursive:true,force:true}))"