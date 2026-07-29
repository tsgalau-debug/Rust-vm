.PHONY: check
check:
	cargo check --workspace --target wasm32-wasip1

.PHONY: wasm
wasm:
	cargo build --release --target wasm32-wasip1 -p wasm-echo

.PHONY: run
run:
	wasmtime run -- target/wasm32-wasip1/release/wasm-echo.wasm hello from makefile

.PHONY: clean
clean:
	cargo clean
