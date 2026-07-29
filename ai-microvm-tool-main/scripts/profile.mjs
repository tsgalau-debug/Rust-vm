// scripts/profile.mjs
// Profile sebagai data. Nol dependensi (hanya fs + path).
// Root proyek dihitung dari lokasi file ini (scripts/..), bukan cwd —
// supaya tetap benar walau run_command dijalankan dari /app/applet.

import { readFileSync, existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
export const PROJECT_ROOT = resolve(__dirname, "..");

const BUILTIN = {
  backend: "wasm",
  engine: "auto",          // auto | node-wasi | wasmtime-cli
  wasm: null,              // diisi dari name
  timeout_ms: 5000,
  max_output_bytes: 1_000_000,
  env: {},
  preopens: {},
  allow_args: null,        // (bibit policy; null = belum dibatasi)
};

export function loadProfile(name, root = PROJECT_ROOT) {
  const p = resolve(root, "profiles", `${name}.json`);
  if (!existsSync(p)) {
    return { ...BUILTIN, name, wasm: `wasm/${name}.wasm`, source: "builtin" };
  }
  const raw = JSON.parse(readFileSync(p, "utf8"));
  return {
    ...BUILTIN,
    ...raw,
    name: raw.name ?? name,
    wasm: raw.wasm ?? `wasm/${name}.wasm`,
    source: `file:${p}`,
  };
}

export function resolveConfig(opts, root = PROJECT_ROOT) {
  const profile = loadProfile(opts.profile, root);
  return {
    profile,
    engine: opts.engine ?? profile.engine,                 // flag menang atas profile
    wasm: resolve(root, opts.wasm ?? profile.wasm),        // relatif terhadap root proyek
    timeoutMs: opts.timeoutMs ?? profile.timeout_ms ?? 5000,
    maxOutput: profile.max_output_bytes,
    env: profile.env ?? {},
    preopens: profile.preopens ?? {},
    allowArgs: profile.allow_args,
  };
}

export function pickEngine(engine, wasmtimeDetected) {
  if (engine === "auto") return wasmtimeDetected ? "wasmtime-cli" : "node-wasi";
  return engine;
}
