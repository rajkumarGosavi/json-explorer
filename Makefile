# JSON Explorer automation targets
ifeq ($(OS),Windows_NT)
SHELL := C:/Program Files/Git/bin/bash.exe
endif

CARGO_MANIFEST := src-tauri/Cargo.toml

.PHONY: install dev build test test-rust test-frontend lint fixture bench bench-index bench-search bench-crit mem load profile-app

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

# --- Profiling & load testing (see scripts/profile-app.ps1, benches/, examples/) ---

# Statistical build+search benchmark (criterion). Runs on a tiny built-in
# fixture by default; point at a real file with FILE and pick a query with Q:
#   make bench-crit
#   make bench-crit FILE=samples/1GB.json Q=fox
bench-crit:
	BENCH_FILE="$(FILE)" BENCH_QUERY="$(or $(Q),fox)" \
		cargo bench --manifest-path $(CARGO_MANIFEST) -p json-index

# Heap-profile build_index (+optional search) with dhat -> dhat-heap.json.
# Watch peak-heap / file-size stay a flat fraction as files grow:
#   make mem FILE=samples/1GB.json Q=fox
mem:
	cargo run --release --manifest-path $(CARGO_MANIFEST) -p json-index --example mem_index -- "$(FILE)" $(Q)

# Concurrent read + search-cancel churn load test. Reports p50/p90/p99 latency:
#   make load FILE=samples/1GB.json
#   make load FILE=samples/1GB.json Q=fox THREADS=8 OPS=20000
load:
	cargo run --release --manifest-path $(CARGO_MANIFEST) -p json-index --example load_test -- "$(FILE)" $(or $(Q),fox) $(or $(THREADS),8) $(or $(OPS),20000)

# Sample CPU + RAM of the running app (start the app first, then run this).
#   make profile-app
#   make profile-app SECONDS=120
profile-app:
	powershell -ExecutionPolicy Bypass -File scripts/profile-app.ps1 -Seconds $(or $(SECONDS),60)

clean:
	rm -rf src-tauri/target || true
	node -e "['dist','coverage','src-tauri/gen'].forEach(d=>require('fs').rmSync(d,{recursive:true,force:true}))"