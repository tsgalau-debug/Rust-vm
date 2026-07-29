# ai-microvm-tool

Experimental AI tool execution plane.

## Stage 1

Basic Rust WASM build pipeline.

## Local Build

```bash
rustup target add wasm32-wasip1
cargo build --release --target wasm32-wasip1 -p wasm-echo
```

## Run

```bash
wasmtime run -- target/wasm32-wasip1/release/wasm-echo.wasm hello world
```

## CI

GitHub Actions builds the WASM artifact.

Artifact name:

```text
wasm-echo
```
