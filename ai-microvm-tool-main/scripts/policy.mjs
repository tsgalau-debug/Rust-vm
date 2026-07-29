// scripts/policy.mjs
// Policy sebagai data. Nol dependensi. Pre-flight: keputusan allow/deny + limit.
import { readFileSync, existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
export const PROJECT_ROOT = resolve(__dirname, "..");

const BUILTIN_POLICY = {
  name: "default",
  allowed_engines: ["auto", "node-wasi", "wasmtime-cli"],
  max_timeout_ms: 10000,
  max_output_bytes: 1_000_000,
  max_arg_count: 64,
  allow_args: null,
  deny_args: [],
};

export function loadPolicy(name = "default", root = PROJECT_ROOT) {
  const p = resolve(root, "policies", `${name}.json`);
  if (!existsSync(p)) {
    return { ...BUILTIN_POLICY, source: "builtin" };
  }
  const raw = JSON.parse(readFileSync(p, "utf8"));
  return { ...BUILTIN_POLICY, ...raw, source: `file:${p}` };
}

function matchesAny(arg, patterns) {
  return (patterns || []).some((pat) => arg.includes(pat));
}

function deny(reason) {
  return { allowed: false, reason };
}

// ctx: { args: string[], engine: string, timeoutMs: number, profileMaxOutput: number }
export function evaluatePolicy(policy, ctx) {
  if (!policy.allowed_engines.includes(ctx.engine)) {
    return deny(`engine '${ctx.engine}' tidak diizinkan oleh policy '${policy.name}'`);
  }

  if (ctx.args.length > policy.max_arg_count) {
    return deny(`jumlah argumen ${ctx.args.length} melebihi max_arg_count ${policy.max_arg_count}`);
  }

  for (const arg of ctx.args) {
    if (matchesAny(arg, policy.deny_args)) {
      return deny(`argumen mengandung pola terlarang: ${JSON.stringify(arg)}`);
    }
  }

  if (policy.allow_args != null) {
    for (const arg of ctx.args) {
      if (!matchesAny(arg, policy.allow_args)) {
        return deny(`argumen tidak cocok allowlist: ${JSON.stringify(arg)}`);
      }
    }
  }

  // allowed → clamp limit (policy = ceiling)
  return {
    allowed: true,
    limits: {
      timeout_ms: Math.min(ctx.timeoutMs ?? policy.max_timeout_ms, policy.max_timeout_ms),
      max_output_bytes: Math.min(ctx.profileMaxOutput ?? policy.max_output_bytes, policy.max_output_bytes),
    },
  };
}
