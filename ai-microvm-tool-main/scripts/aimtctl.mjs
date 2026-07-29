#!/usr/bin/env node
// scripts/aimtctl.mjs
// Reference CLI untuk dipanggil AI agent via run_command.
// Prinsip: stdout HANYA JSON; log/error ke stderr; exit code bermakna.

import { spawn, execSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { WASI } from "node:wasi";
import { resolveConfig, pickEngine, PROJECT_ROOT } from "./profile.mjs";
import { loadPolicy, evaluatePolicy } from "./policy.mjs";

const __dirname = dirname(fileURLToPath(import.meta.url));
const MAX_OUTPUT = 1_000_000; // 1 MB per stream (bibit policy)

function parseArgs(argv) {
  const opts = {
    profile: "wasm-echo",
    policy: "default",
    output: "json",
    timeoutMs: undefined,   // undefined = "user tidak meminta" → pakai profile
    engine: undefined,
    wasm: undefined,
    args: [],
  };
  let afterDash = false;
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (afterDash) { opts.args.push(a); continue; }
    if (a === "--") { afterDash = true; continue; }
    if (a === "--profile") { opts.profile = argv[++i]; continue; }
    if (a === "--policy") { opts.policy = argv[++i]; continue; }
    if (a === "--output") { opts.output = argv[++i]; continue; }
    if (a === "--timeout-ms") { opts.timeoutMs = Number(argv[++i]) || undefined; continue; }
    if (a === "--engine") { opts.engine = argv[++i]; continue; }
    if (a === "--wasm") { opts.wasm = argv[++i]; continue; }
    opts.args.push(a);
  }
  return opts;
}

// cari wasm dari beberapa kandidat path
function resolveWasm(cwd) {
  const candidates = [
    process.env.AIMT_WASM,
    resolve(__dirname, "../wasm/wasm-echo.wasm"),
    resolve(__dirname, "../artifacts/wasm/wasm-echo.wasm"),
    resolve(__dirname, "../target/wasm32-wasip1/release/wasm-echo.wasm"),
    resolve(cwd, "wasm/wasm-echo.wasm"),
    resolve(cwd, "artifacts/wasm/wasm-echo.wasm"),
    resolve(cwd, "target/wasm32-wasip1/release/wasm-echo.wasm"),
    resolve(cwd, "ai-microvm-tool-main/wasm/wasm-echo.wasm"),
    resolve(cwd, "ai-microvm-tool-main/artifacts/wasm/wasm-echo.wasm"),
    resolve(cwd, "ai-microvm-tool-main/target/wasm32-wasip1/release/wasm-echo.wasm"),
  ].filter(Boolean);
  return candidates.find((p) => existsSync(p)) || null;
}

function detectWasmtime() {
  try { return execSync("wasmtime --version", { stdio: ["ignore", "pipe", "ignore"] }).toString().trim(); }
  catch { return null; }
}

function listDir(relDir, ext, root = PROJECT_ROOT) {
  const dir = resolve(root, relDir);
  try {
    return readdirSync(dir)
      .filter((f) => f.endsWith(ext))
      .map((f) => f.slice(0, -ext.length))
      .sort();
  } catch {
    return [];
  }
}

function runNodeWasi(wasmPath, args, timeoutMs, maxOutput = MAX_OUTPUT, sandbox = {}) {
  return new Promise((res) => {
    const start = Date.now();
    let stdout = "", stderr = "", truncated = false, timedOut = false;

    const wasiConfig = JSON.stringify({
      env: sandbox.env ?? {},
      preopens: sandbox.preopens ?? {},
    });

    const runnerScript = `
import { WASI } from "node:wasi";
import { readFileSync } from "node:fs";

const [wasmPath, configJson, ...args] = process.argv.slice(1);
const cfg = JSON.parse(configJson);
const wasi = new WASI({
  version: "preview1",
  args: ["wasm-echo", ...args],
  env: cfg.env,
  preopens: cfg.preopens,
});

const wasmBuffer = readFileSync(wasmPath);
const wasmModule = await WebAssembly.compile(wasmBuffer);
const instance = await WebAssembly.instantiate(wasmModule, wasi.getImportObject());
try {
  wasi.start(instance);
} catch (e) {
  if (e && typeof e.code === "number") process.exit(e.code);
  process.stderr.write(e.message || String(e));
  process.exit(1);
}
`;

    const child = spawn(process.execPath, ["--no-warnings", "--input-type=module", "-e", runnerScript, wasmPath, wasiConfig, ...args], {
      stdio: ["ignore", "pipe", "pipe"],
    });

    const timer = setTimeout(() => { timedOut = true; child.kill("SIGKILL"); }, timeoutMs);

    const append = (which, data) => {
      const s = data.toString();
      if (which === "out") { if (stdout.length < maxOutput) stdout += s; else truncated = true; }
      else { if (stderr.length < maxOutput) stderr += s; else truncated = true; }
    };

    child.stdout.on("data", (d) => append("out", d));
    child.stderr.on("data", (d) => append("err", d));

    child.on("error", (e) => {
      clearTimeout(timer);
      res({ exit_code: -1, timed_out: timedOut, truncated, stdout, stderr: stderr + "\n[spawn error] " + e.message, wall_time_ms: Date.now() - start, engine: "node-wasi" });
    });

    child.on("close", (code) => {
      clearTimeout(timer);
      res({ exit_code: code ?? -1, timed_out: timedOut, truncated, stdout, stderr, wall_time_ms: Date.now() - start, engine: "node-wasi" });
    });
  });
}

function runWasmtime(wasm, args, timeoutMs, maxOutput = MAX_OUTPUT) {
  return new Promise((res) => {
    const start = Date.now();
    let stdout = "", stderr = "", truncated = false, timedOut = false;

    const child = spawn("wasmtime", ["run", "--", wasm, ...args], {
      stdio: ["ignore", "pipe", "pipe"],
    });

    const timer = setTimeout(() => { timedOut = true; child.kill("SIGKILL"); }, timeoutMs);

    const append = (which, data) => {
      const s = data.toString();
      if (which === "out") { if (stdout.length < maxOutput) stdout += s; else truncated = true; }
      else { if (stderr.length < maxOutput) stderr += s; else truncated = true; }
    };

    child.stdout.on("data", (d) => append("out", d));
    child.stderr.on("data", (d) => append("err", d));

    child.on("error", (e) => {
      clearTimeout(timer);
      res({ exit_code: -1, timed_out: timedOut, truncated, stdout, stderr: stderr + "\n[spawn error] " + e.message, wall_time_ms: Date.now() - start, engine: "wasmtime" });
    });

    child.on("close", (code) => {
      clearTimeout(timer);
      res({ exit_code: code ?? -1, timed_out: timedOut, truncated, stdout, stderr, wall_time_ms: Date.now() - start, engine: "wasmtime" });
    });
  });
}

function emit(opts, obj, code) {
  if (opts.output === "json") console.log(JSON.stringify(obj));
  else process.stdout.write(obj.stdout ?? "");
  process.exit(code);
}

// Resolusi penuh TANPA eksekusi. Dipakai bersama oleh run & explain (DRY).
function prepare(opts) {
  const cwd = process.cwd();
  const cfg = resolveConfig(opts);
  const wasmtime = detectWasmtime();
  const engine = pickEngine(cfg.engine, !!wasmtime);

  const wasmRequested = cfg.wasm;          // path yang "diminta" profile/flag
  const wasmExists = existsSync(wasmRequested);
  const wasm = wasmExists ? wasmRequested : resolveWasm(cwd); // graceful fallback
  const wasmFallback = !wasmExists;

  const policy = loadPolicy(opts.policy);
  const decision = evaluatePolicy(policy, {
    args: opts.args,
    engine,
    timeoutMs: cfg.timeoutMs,
    profileMaxOutput: cfg.maxOutput,
  });

  return { cwd, cfg, wasmtime, engine, wasmRequested, wasmExists, wasm, wasmFallback, policy, decision };
}

// ---- main ----
const [,, cmd, ...rest] = process.argv;

if (cmd === "doctor") {
  const wasmtime = detectWasmtime();
  const doc = {
    ok: true,
    command: "doctor",
    runner: "aimtctl.mjs",
    node: process.version,
    project_root: PROJECT_ROOT,
    engines: {
      "node-wasi": true,
      "wasmtime-cli": !!wasmtime,
      wasmtime_version: wasmtime || null,
    },
    profiles: listDir("profiles", ".json"),
    policies: listDir("policies", ".json"),
    wasms: listDir("wasm", ".wasm"),
  };
  console.log(JSON.stringify(doc));
  process.exit(0);
}

const opts = (cmd === "run" || cmd === "explain") ? parseArgs(rest) : null;

if (cmd === "explain") {
  const p = prepare(opts);
  const base = p.wasmRequested ? p.wasmRequested.split("/").pop() : null;
  const doc = {
    ok: true,
    command: "explain",
    profile: opts.profile,
    profile_source: p.cfg.profile.source,
    policy: p.policy.name,
    policy_source: p.policy.source,
    engine_requested: p.cfg.engine,
    engine_resolved: p.engine,
    wasmtime_available: !!p.wasmtime,
    wasm: { requested: p.wasmRequested, exists: p.wasmExists, resolved: p.wasm, fallback: p.wasmFallback },
    argv0: base ? base.replace(/\.wasm$/i, "") : null,
    args: opts.args,
    policy_decision: p.decision.allowed ? "allowed" : "denied",
  };
  if (!p.decision.allowed) {
    doc.reason = p.decision.reason;
  } else {
    doc.limits = p.decision.limits;
    doc.sandbox = { env_keys: Object.keys(p.cfg.env ?? {}), preopens: p.cfg.preopens ?? {} };
  }
  console.log(JSON.stringify(doc));
  process.exit(0);
}

if (cmd !== "run") {
  console.error("usage: aimtctl <run|doctor|explain> ...");
  process.exit(2);
}

// ---- run (perilaku identik dengan sebelumnya, kini via prepare) ----
const p = prepare(opts);

if (!p.wasm) emit(opts, { ok: false, error: "wasm tidak ditemukan; build dulu atau set AIMT_WASM", profile: opts.profile, policy: p.policy.name, engine: p.engine }, 1);

if (!p.decision.allowed) {
  emit(opts, {
    ok: false,
    backend: "wasm",
    profile: opts.profile,
    profile_source: p.cfg.profile.source,
    policy: p.policy.name,
    policy_source: p.policy.source,
    policy_decision: "denied",
    reason: p.decision.reason,
    engine: p.engine,
    wasm: p.wasm,
  }, 3);
}

const timeoutMs = p.decision.limits.timeout_ms;
const maxOutput = p.decision.limits.max_output_bytes;

let r;
if (p.engine === "wasmtime-cli") {
  r = await runWasmtime(p.wasm, opts.args, timeoutMs, maxOutput);
} else if (p.engine === "node-wasi") {
  r = await runNodeWasi(p.wasm, opts.args, timeoutMs, maxOutput, { env: p.cfg.env, preopens: p.cfg.preopens });
} else {
  emit(opts, { ok: false, error: `engine tidak dikenal: ${p.engine}` }, 2);
}

const out = {
  ok: r.exit_code === 0 && !r.timed_out,
  backend: "wasm",
  profile: opts.profile,
  profile_source: p.cfg.profile.source,
  policy: p.policy.name,
  policy_source: p.policy.source,
  policy_decision: "allowed",
  limits: p.decision.limits,
  sandbox: { env_keys: Object.keys(p.cfg.env ?? {}), preopens: p.cfg.preopens ?? {} },
  wasmtime: p.wasmtime || "node-wasi (native)",
  wasm: p.wasm,
  ...r,
  engine: p.engine,
};
if (p.wasmFallback) out.wasm_fallback = true;
emit(opts, out, r.exit_code === 0 ? 0 : 1);
