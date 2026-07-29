// apps/control-plane/server.js
const express = require("express");
const fs = require("node:fs");
const path = require("node:path");
const { spawn, execSync } = require("node:child_process");

const PORT = process.env.PORT || 8080;
const MAX_OUTPUT = 1_000_000; // 1 MB per stream (bibit policy)

// --- deteksi wasmtime sekali di startup (graceful degradation) ---
let wasmtimeVersion = null;
try {
  wasmtimeVersion = execSync("wasmtime --version").toString().trim();
} catch {
  // biarkan null; endpoint akan menolak dengan 503 yang jelas
}

// --- cari wasm-echo.wasm dari beberapa kandidat, jangan hardcoded ---
function resolveWasmPath() {
  const candidates = [
    process.env.WASM_ECHO_PATH,
    path.resolve(__dirname, "../../wasm/wasm-echo.wasm"),
    path.resolve(__dirname, "../../artifacts/wasm/wasm-echo.wasm"),
    path.resolve(__dirname, "../../target/wasm32-wasip1/release/wasm-echo.wasm"),
  ].filter(Boolean);

  for (const c of candidates) {
    try {
      if (fs.existsSync(c)) return c;
    } catch {
      /* abaikan */
    }
  }
  return null;
}

const wasmPath = resolveWasmPath();

console.log("[control-plane] wasmtime :", wasmtimeVersion || "TIDAK DITEMUKAN di PATH");
console.log("[control-plane] wasm     :", wasmPath || "TIDAK DITEMUKAN (build dulu / set WASM_ECHO_PATH)");

// --- eksekutor: spawn wasmtime, tangani timeout + truncate output ---
function runWasm(wasm, args, timeoutMs) {
  return new Promise((resolve) => {
    let stdout = "";
    let stderr = "";
    let truncated = false;
    let timedOut = false;

    const child = spawn("wasmtime", ["run", "--", wasm, ...args], {
      stdio: ["ignore", "pipe", "pipe"],
    });

    const timer = setTimeout(() => {
      timedOut = true;
      child.kill("SIGKILL");
    }, timeoutMs);

    const append = (which, data) => {
      const s = data.toString();
      if (which === "out") {
        if (stdout.length < MAX_OUTPUT) stdout += s;
        else truncated = true;
      } else {
        if (stderr.length < MAX_OUTPUT) stderr += s;
        else truncated = true;
      }
    };

    child.stdout.on("data", (d) => append("out", d));
    child.stderr.on("data", (d) => append("err", d));

    child.on("error", (err) => {
      clearTimeout(timer);
      resolve({
        exit_code: -1,
        timed_out: timedOut,
        truncated,
        stdout,
        stderr: stderr + "\n[spawn error] " + err.message,
      });
    });

    child.on("close", (code) => {
      clearTimeout(timer);
      resolve({
        exit_code: code ?? -1,
        timed_out: timedOut,
        truncated,
        stdout,
        stderr,
      });
    });
  });
}

// --- HTTP ---
const app = express();
app.use(express.json({ limit: "1mb" }));

app.get("/healthz", (req, res) => {
  res.json({
    ok: true,
    backend: "wasm",
    wasm: !!wasmPath,
    wasm_path: wasmPath,
    wasmtime: wasmtimeVersion,
  });
});

app.post("/v1/exec", async (req, res) => {
  if (!wasmPath) {
    return res.status(503).json({ error: "wasm-echo.wasm tidak ditemukan; build dulu atau set WASM_ECHO_PATH" });
  }
  if (!wasmtimeVersion) {
    return res.status(503).json({ error: "wasmtime tidak ditemukan di PATH" });
  }

  const body = req.body || {};
  const args = Array.isArray(body.args) ? body.args.filter((a) => typeof a === "string") : [];

  let timeoutMs = Number(body.timeout_ms) || 5000;
  timeoutMs = Math.max(100, Math.min(60000, timeoutMs | 0));

  const result = await runWasm(wasmPath, args, timeoutMs);
  res.json(result);
});

app.listen(PORT, () => {
  console.log(`[control-plane] listening on http://localhost:${PORT}`);
});
