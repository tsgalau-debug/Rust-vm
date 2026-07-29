#!/usr/bin/env node
// scripts/smoke.mjs
// Regression net untuk aimtctl. Nol dependensi. Memanggil pintu CLI via child_process.
// Exit 0 kalau semua lulus, 1 kalau ada gagal.

import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const AIMT = resolve(__dirname, "aimtctl.mjs");
const has = (arr, v) => Array.isArray(arr) && arr.includes(v);

const cases = [
  {
    name: "doctor: inventaris lengkap",
    args: ["doctor"],
    expectExit: 0,
    check: (o) =>
      o.ok === true &&
      has(o.profiles, "wasm-echo") && has(o.profiles, "wasm-echo-fs") &&
      has(o.policies, "default") && has(o.policies, "strict") &&
      o.engines?.["node-wasi"] === true,
  },
  {
    name: "run: allowed, engine node-wasi, stdout benar",
    args: ["run", "--profile", "wasm-echo", "--policy", "default", "--output", "json", "--", "halo", "smoke"],
    expectExit: 0,
    check: (o) =>
      o.ok === true && o.policy_decision === "allowed" && o.engine === "node-wasi" &&
      /hello from wasm-echo/.test(o.stdout || ""),
  },
  {
    name: "run: denied via deny_args (exit 3, WASM tak jalan)",
    args: ["run", "--profile", "wasm-echo", "--policy", "default", "--output", "json", "--", "rm -rf /"],
    expectExit: 3,
    check: (o) => o.ok === false && o.policy_decision === "denied",
  },
  {
    name: "run: engine dibatasi policy strict (exit 3)",
    args: ["run", "--profile", "wasm-echo", "--policy", "strict", "--engine", "wasmtime-cli", "--output", "json", "--", "halo"],
    expectExit: 3,
    check: (o) => o.policy_decision === "denied" && /wasmtime-cli/.test(o.reason || ""),
  },
  {
    name: "explain: clamp timeout+output ke ceiling strict",
    args: ["explain", "--profile", "wasm-echo", "--policy", "strict", "--timeout-ms", "99999", "--", "halo"],
    expectExit: 0,
    check: (o) =>
      o.policy_decision === "allowed" &&
      o.limits?.timeout_ms === 3000 && o.limits?.max_output_bytes === 65536,
  },
  {
    name: "explain: wasm belum ada -> exists:false, fallback:true",
    args: ["explain", "--profile", "wasm-echo", "--wasm", "wasm/nope.wasm", "--", "halo"],
    expectExit: 0,
    check: (o) => o.wasm?.exists === false && o.wasm?.fallback === true,
  },
  {
    name: "run: sandbox env+preopens terisi dari profile",
    args: ["run", "--profile", "wasm-echo-fs", "--policy", "default", "--output", "json", "--", "halo"],
    expectExit: 0,
    check: (o) =>
      o.ok === true &&
      has(o.sandbox?.env_keys, "APP_ENV") && has(o.sandbox?.env_keys, "GREETING") &&
      o.sandbox?.preopens?.["/sandbox"] === "/tmp",
  },
  {
    name: "explain: sandbox terisi (preview)",
    args: ["explain", "--profile", "wasm-echo-fs", "--policy", "default", "--", "halo"],
    expectExit: 0,
    check: (o) =>
      Array.isArray(o.sandbox?.env_keys) && o.sandbox.env_keys.length === 2 &&
      o.sandbox?.preopens?.["/sandbox"] === "/tmp",
  },
];

const results = [];
for (const c of cases) {
  const r = spawnSync(process.execPath, [AIMT, ...c.args], { encoding: "utf8", timeout: 15000 });

  let parsed, parseErr, checkOk = false, checkErr;
  try { parsed = JSON.parse(r.stdout); } catch (e) { parseErr = e.message; }
  if (!parseErr) { try { checkOk = !!c.check(parsed); } catch (e) { checkErr = e.message; } }

  const timedOut = r.error?.code === "ETIMEDOUT";
  const exitOk = r.status === c.expectExit;
  const pass = !timedOut && exitOk && !parseErr && !checkErr && checkOk;

  let detail = "";
  if (timedOut) detail = "TIMEOUT (>15s)";
  else if (!exitOk) detail = `exit got ${r.status}`;
  if (parseErr) detail += (detail ? "; " : "") + `stdout not JSON (${parseErr})`;
  if (checkErr) detail += (detail ? "; " : "") + `check threw (${checkErr})`;
  if (!checkOk && !checkErr && !parseErr) detail += (detail ? "; " : "") + "check returned false";

  results.push({ name: c.name, pass, detail });
  console.log(`${pass ? "PASS" : "FAIL"}  ${c.name}${pass ? "" : "\n       " + detail}`);
}

const failed = results.filter((x) => !x.pass);
console.log(`\nsmoke: ${AIMT}`);
console.log(`${results.length - failed.length}/${results.length} passed`);
process.exit(failed.length ? 1 : 0);
