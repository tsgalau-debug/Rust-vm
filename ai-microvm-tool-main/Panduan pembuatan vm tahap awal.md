Oke, kita mulai **pelan-pelan dari struktur dasar dulu**.

Fokus tahap ini hanya satu:

> **Membuat satu crate Rust sederhana yang bisa dikompilasi ke WASM, lalu build-nya dijalankan lewat GitHub CI.**

Kita **tidak masuk Firecracker dulu**, tidak masuk Cloud Run dulu, tidak masuk worker dulu.  
Kita bikin fondasi paling kecil tapi benar.

---

# 1. Target Tahap Ini

Setelah tahap ini selesai, Anda punya:

```text
1. Repo Rust workspace
2. Satu crate kecil: wasm-echo
3. Bisa compile ke target wasm32-wasip1
4. Bisa dijalankan dengan wasmtime
5. GitHub Actions bisa build WASM otomatis
6. Artifact .wasm bisa di-download dari GitHub Actions
```

Ini dulu.

---

# 2. Struktur Dasar Repo

Buat struktur sesederhana ini:

```text
ai-microvm-tool/
├── .github/
│   └── workflows/
│       └── wasm-ci.yml
│
├── crates/
│   └── wasm-echo/
│       ├── src/
│       │   └── main.rs
│       └── Cargo.toml
│
├── artifacts/
│   └── wasm/
│       └── .gitkeep
│
├── profiles/
│   └── .gitkeep
│
├── policies/
│   └── .gitkeep
│
├── .gitignore
├── Cargo.toml
├── rust-toolchain.toml
└── README.md
```

Ini sudah cukup untuk mulai.

---

# 3. Buat Folder Dasar

Jalankan:

```bash
mkdir ai-microvm-tool
cd ai-microvm-tool

mkdir -p .github/workflows
mkdir -p crates/wasm-echo/src
mkdir -p artifacts/wasm
mkdir -p profiles
mkdir -p policies

touch .gitignore
touch Cargo.toml
touch rust-toolchain.toml
touch README.md
touch artifacts/wasm/.gitkeep
touch profiles/.gitkeep
touch policies/.gitkeep
touch crates/wasm-echo/Cargo.toml
touch crates/wasm-echo/src/main.rs
touch .github/workflows/wasm-ci.yml
```

---

# 4. File Root: `Cargo.toml`

Isi file:

```toml
# Cargo.toml

[workspace]
resolver = "2"

members = [
  "crates/wasm-echo",
]
```

Ini membuat repo Anda siap jadi workspace.

Nanti crate lain tinggal ditambah, misalnya:

```text
crates/wasm-backend
crates/control-plane
crates/shared-types
```

Tapi sekarang satu dulu.

---

# 5. File: `rust-toolchain.toml`

Isi:

```toml
# rust-toolchain.toml

[toolchain]
channel = "stable"
targets = ["wasm32-wasip1"]
```

Fungsinya:

```text
memastikan Rust memakai target wasm32-wasip1
```

Kenapa `wasm32-wasip1`?

Karena kita ingin WASM yang bisa dijalankan sebagai command-line program dengan WASI.

Contoh:

```bash
wasmtime run wasm-echo.wasm hello
```

Kalau `wasm32-unknown-unknown`, itu lebih cocok untuk WASM tanpa I/O, misalnya browser/embedded. Untuk eksperimen tool CLI, `wasm32-wasip1` lebih tepat.

---

# 6. File: `.gitignore`

Isi:

```gitignore
# .gitignore

/target

node_modules
.env
*.log

# Jangan commit artifact WASM hasil build lokal
/artifacts/wasm/*.wasm
/artifacts/wasm/*.component.wasm

# Tapi biarkan .gitkeep
!/artifacts/wasm/.gitkeep
```

Prinsip:

```text
Source code di-commit.
Artifact build tidak perlu di-commit.
Artifact nanti dihasilkan oleh GitHub CI.
```

---

# 7. Crate WASM: `wasm-echo`

File:

```text
crates/wasm-echo/Cargo.toml
```

Isi:

```toml
[package]
name = "wasm-echo"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "wasm-echo"
path = "src/main.rs"

[profile.release]
opt-level = "s"
lto = true
strip = true
```

Penjelasan singkat:

```text
opt-level = "s"
  → optimasi ukuran

lto = true
  → link-time optimization

strip = true
  → buang symbol debug agar lebih kecil
```

---

# 8. Source Code: `main.rs`

File:

```text
crates/wasm-echo/src/main.rs
```

Isi:

```rust
fn main() {
    let args: Vec<String> = std::env::args().collect();

    println!("hello from wasm-echo");

    if args.len() <= 1 {
        println!("no arguments provided");
        return;
    }

    println!("arguments:");

    for arg in args.iter().skip(1) {
        println!("- {arg}");
    }
}
```

Program ini sangat sederhana.

Input:

```bash
wasm-echo halo dari wasm
```

Output:

```text
hello from wasm-echo
arguments:
- halo
- dari
- wasm
```

Ini cukup untuk membuktikan:

```text
Rust → WASM → WASI → stdout → args
```

---

# 9. Build Lokal Dulu

Sebelum GitHub CI, pastikan lokal bisa build.

Install target WASM:

```bash
rustup target add wasm32-wasip1
```

Build:

```bash
cargo build --release --target wasm32-wasip1 -p wasm-echo
```

Output akan ada di:

```text
target/wasm32-wasip1/release/wasm-echo.wasm
```

Cek:

```bash
ls -lh target/wasm32-wasip1/release/wasm-echo.wasm
```

---

# 10. Install Wasmtime

Untuk menjalankan WASM, pakai `wasmtime`.

Linux / WSL / AI Studio:

```bash
curl https://wasmtime.dev/install.sh -sSf | bash
```

Lalu restart shell atau:

```bash
source ~/.bashrc
```

Cek:

```bash
wasmtime --version
```

---

# 11. Jalankan WASM Secara Lokal

Jalankan:

```bash
wasmtime run -- target/wasm32-wasip1/release/wasm-echo.wasm halo dari wasm
```

Expected output:

```text
hello from wasm-echo
arguments:
- halo
- dari
- wasm
```

Kalau ini berhasil, berarti fondasi Anda valid.

---

# 12. GitHub Actions untuk Build WASM

Sekarang kita buat CI.

File:

```text
.github/workflows/wasm-ci.yml
```

Isi:

```yaml
name: wasm-ci

on:
  push:
    branches:
      - main
    paths:
      - "crates/**"
      - "Cargo.toml"
      - "Cargo.lock"
      - ".github/workflows/wasm-ci.yml"

  pull_request:
    paths:
      - "crates/**"
      - "Cargo.toml"
      - "Cargo.lock"
      - ".github/workflows/wasm-ci.yml"

  workflow_dispatch:

permissions:
  contents: read

concurrency:
  group: wasm-ci-${{ github.ref }}
  cancel-in-progress: true

jobs:
  build-wasm:
    name: Build WASM
    runs-on: ubuntu-latest

    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-wasip1

      - name: Cache cargo registry and build
        uses: Swatinem/rust-cache@v2
        with:
          shared-key: wasm-ci

      - name: Cargo check
        run: cargo check --workspace --target wasm32-wasip1

      - name: Build wasm-echo
        run: cargo build --release --target wasm32-wasip1 -p wasm-echo

      - name: List build output
        run: |
          echo "Listing WASM build output"
          ls -lh target/wasm32-wasip1/release || true

      - name: Upload wasm artifact
        uses: actions/upload-artifact@v4
        with:
          name: wasm-echo
          path: target/wasm32-wasip1/release/wasm-echo.wasm
          if-no-files-found: error
          retention-days: 14
```

---

# 13. Penjelasan Workflow CI

Workflow ini melakukan:

```text
1. Checkout repo
2. Install Rust stable
3. Install target wasm32-wasip1
4. Cache cargo
5. Cargo check
6. Build release WASM
7. Upload artifact wasm-echo.wasm
```

Artifact akan muncul di tab:

```text
GitHub → Actions → workflow run → Artifacts
```

Nama artifact:

```text
wasm-echo
```

Isinya:

```text
wasm-echo.wasm
```

---

# 14. Commit dan Push

Jalankan:

```bash
git init
git add .
git commit -m "feat: basic wasm-echo crate and github ci"
```

Kalau repo GitHub Anda misalnya:

```text
https://github.com/username/ai-microvm-tool
```

Maka:

```bash
git branch -M main
git remote add origin https://github.com/username/ai-microvm-tool.git
git push -u origin main
```

---

# 15. Cek GitHub Actions

Buka:

```text
https://github.com/username/ai-microvm-tool/actions
```

Anda harus melihat workflow:

```text
wasm-ci
```

Kalau sukses, ada artifact:

```text
wasm-echo
```

---

# 16. Download Artifact dari GitHub Actions

## Cara Manual

Buka:

```text
GitHub → Actions → wasm-ci → pilih run → Artifacts
```

Download:

```text
wasm-echo.zip
```

Lalu extract ke:

```text
artifacts/wasm/
```

Sehingga menjadi:

```text
artifacts/wasm/wasm-echo.wasm
```

---

## Cara CLI

Jika Anda pakai `gh`:

Login:

```bash
gh auth login
```

Lihat run:

```bash
gh run list --workflow=wasm-ci
```

Ambil `run_id`, lalu:

```bash
gh run download <run_id> \
  --name wasm-echo \
  --dir artifacts/wasm \
  --clobber
```

Contoh:

```bash
gh run download 1234567890 \
  --name wasm-echo \
  --dir artifacts/wasm \
  --clobber
```

Lalu cek:

```bash
ls -lh artifacts/wasm
```

Harus ada:

```text
wasm-echo.wasm
```

---

# 17. Jalankan Artifact Hasil CI

Jalankan:

```bash
wasmtime run -- artifacts/wasm/wasm-echo.wasm halo dari github-ci
```

Expected:

```text
hello from wasm-echo
arguments:
- halo
- dari
- github-ci
```

Kalau ini berhasil, berarti:

```text
GitHub CI berhasil build Rust → WASM
Artifact bisa dipakai lokal
```

---

# 18. Tambahkan `README.md`

Isi minimal:

```md
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
```

---

# 19. Tambahkan Makefile Opsional

Biar enak, buat:

```bash
touch Makefile
```

Isi:

```make
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
```

Sekarang Anda bisa:

```bash
make wasm
make run
```

---

# 20. Struktur Akhir Tahap 1

Sekarang repo Anda terlihat seperti ini:

```text
ai-microvm-tool/
├── .github/
│   └── workflows/
│       └── wasm-ci.yml
│
├── crates/
│   └── wasm-echo/
│       ├── src/
│       │   └── main.rs
│       └── Cargo.toml
│
├── artifacts/
│   └── wasm/
│       └── .gitkeep
│
├── profiles/
│   └── .gitkeep
│
├── policies/
│   └── .gitkeep
│
├── .gitignore
├── Cargo.toml
├── Makefile
├── README.md
└── rust-toolchain.toml
```

Ini sudah cukup sebagai fondasi.

---

# 21. Kenapa Struktur Ini Bagus untuk Mulai?

Karena dia memenuhi prinsip Anda:

> tidak membatasi, tapi menyediakan reference machine.

Struktur ini belum mengunci apa pun.

Nanti Anda bisa tambah:

```text
crates/shared-types
crates/wasm-backend
crates/process-jail-backend
crates/control-plane
crates/remote-worker-backend
crates/firecracker-backend
```

Tapi sekarang Anda sudah punya:

```text
Rust workspace
WASM target
CI pipeline
Artifact strategy
```

Ini fondasi yang sehat.

---

# 22. Error Umum dan Solusinya

## Error: `target wasm32-wasip1 not installed`

Solusi:

```bash
rustup target add wasm32-wasip1
```

---

## Error: `wasmtime: command not found`

Solusi:

```bash
curl https://wasmtime.dev/install.sh -sSf | bash
source ~/.bashrc
```

Cek:

```bash
wasmtime --version
```

---

## Error: `failed to select package`

Pastikan `Cargo.toml` root benar:

```toml
[workspace]
resolver = "2"

members = [
  "crates/wasm-echo",
]
```

---

## Error: `no such file or directory: wasm-echo.wasm`

Cek path output:

```bash
ls target/wasm32-wasip1/release/
```

Nama file seharusnya:

```text
wasm-echo.wasm
```

---

## GitHub Actions gagal di `cargo check`

Pastikan workflow punya step target:

```yaml
- name: Install Rust stable
  uses: dtolnay/rust-toolchain@stable
  with:
    targets: wasm32-wasip1
```

---

# 23. Tahap Berikutnya Setelah Ini

Kalau tahap ini sudah jalan, urutan berikutnya yang paling masuk akal:

## Tahap 2: Control Plane Node.js Minimal

Tambah:

```text
apps/control-plane/
├── server.js
├── package.json
└── Dockerfile
```

Tugasnya:

```text
HTTP POST /v1/exec
→ spawn wasmtime
→ jalankan artifacts/wasm/wasm-echo.wasm
→ return stdout/stderr
```

---

## Tahap 3: Policy Sederhana

Tambah:

```text
policies/default.toml
```

Validasi:

```text
timeout
max output
argv allowlist
```

---

## Tahap 4: GitHub CI Build Docker Cloud Run

Nanti CI bisa:

```text
1. build WASM
2. copy ke image Cloud Run
3. deploy
```

Tapi itu nanti.

---

# 24. Saran Saya Sekarang

Kerjakan dulu sampai titik ini sukses:

```bash
cargo build --release --target wasm32-wasip1 -p wasm-echo
wasmtime run -- target/wasm32-wasip1/release/wasm-echo.wasm halo
```

Lalu:

```text
push ke GitHub
GitHub Actions hijau
artifact wasm-echo bisa didownload
```

Kalau itu sudah beres, baru kita lanjut ke:

```text
Node.js control plane minimal
```

---

