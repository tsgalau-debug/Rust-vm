//! aimt — "otak" Rust sisi guest (bin WASM).
//! v1 = mesin evaluasi policy yang portabel, hidup di atas capability-model + shared-types.
//!
//! Subcommand:
//!   policy-check       <backend> <timeout_ms> <max_output_bytes> [-- args...]
//!       -> evaluasi memakai POLICY BUILTIN Rust-native (backend-gate).
//!   policy-check-json  <policy_json> <backend> <timeout_ms> <max_output_bytes> [-- args...]
//!       -> evaluasi memakai policy dari argv JSON (fitur sekunder; string backend = PascalCase,
//!          mis. "Wasm", karena itu bentuk serde enum BackendKind).
//!   help
//!
//! Kontrak guest: SELALU cetak satu baris JSON; keputusan ada di field `decision`
//! (allowed|denied); `ok` = "evaluasi berhasil dilakukan" (bukan "diizinkan").
//! Exit code proses = 0 selama tool tidak crash (konsisten dengan wasm-fs).
//!
//! Conformance dengan aimtctl.mjs (JS) berlaku pada irisan semantik yang sama:
//! deny_args / allow_args / max_arg_count / clamp-timeout. Dimensi engine-gate (JS)
//! vs backend-gate (Rust) sengaja berbeda; backend-gate adalah dimensi ekstra Rust.

use capability_model::Policy;
use serde_json::{json, Value};
use shared_types::{BackendKind, CommandRequest};
use std::collections::BTreeMap;

/// Policy contoh Rust-native. Ceiling numeriknya sengaja selaras dengan policies/default.json
/// (max_timeout 10000, max_output 1048576) supaya conformance clamp-timeout bisa ditunjukkan.
fn builtin_policy() -> Policy {
    Policy {
        name: "builtin-default".into(),
        allowed_backends: vec![BackendKind::Wasm, BackendKind::ProcessJail],
        max_timeout_ms: 10_000,
        max_output_bytes: 1_048_576,
        max_arg_count: 64,
        allow_args: None,
        deny_args: vec!["rm -rf".into(), "sudo".into(), "--privileged".into()],
    }
}

/// Mapping CLI (kebab/lower) -> BackendKind. Terpisah dari bentuk serde (PascalCase).
fn parse_backend(s: &str) -> Result<BackendKind, String> {
    match s {
        "wasm" => Ok(BackendKind::Wasm),
        "process-jail" => Ok(BackendKind::ProcessJail),
        "firecracker" => Ok(BackendKind::Firecracker),
        "libkrun" => Ok(BackendKind::Libkrun),
        "remote" => Ok(BackendKind::Remote),
        other => Err(format!("unknown backend: {other}")),
    }
}

fn build_req(argv: Vec<String>, timeout_ms: u32, maxout: u64) -> CommandRequest {
    CommandRequest {
        request_id: String::new(),
        lease_id: String::new(),
        capability_token: String::new(),
        argv,
        env: BTreeMap::new(),
        cwd: None,
        stdin: None,
        timeout_ms,
        max_stdout_bytes: maxout,
        max_stderr_bytes: maxout,
        stream: false,
    }
}

fn err(cmd: &str, msg: &str) -> Value {
    json!({ "command": cmd, "ok": false, "error": msg })
}

fn args_to_value(a: &[String]) -> Value {
    Value::Array(a.iter().map(|s| Value::from(s.clone())).collect())
}

/// Inti: parse input -> evaluate -> JSON. Murni & teruji di host (tanpa fs/env).
fn evaluate_to_value(
    cmd: &str,
    policy: &Policy,
    backend_str: &str,
    timeout_str: &str,
    maxout_str: &str,
    check_args: Vec<String>,
) -> Value {
    let backend = match parse_backend(backend_str) {
        Ok(b) => b,
        Err(e) => return err(cmd, &e),
    };
    let timeout_ms: u32 = match timeout_str.parse() {
        Ok(v) => v,
        Err(e) => return err(cmd, &format!("invalid timeout_ms: {e}")),
    };
    let maxout: u64 = match maxout_str.parse() {
        Ok(v) => v,
        Err(e) => return err(cmd, &format!("invalid max_output_bytes: {e}")),
    };

    let args_val = args_to_value(&check_args);
    let req = build_req(check_args, timeout_ms, maxout);

    match policy.evaluate(backend, &req) {
        Ok(lim) => json!({
            "command": cmd,
            "ok": true,
            "policy": policy.name.clone(),
            "decision": "allowed",
            "backend": backend_str,
            "limits": {
                "timeout_ms": (lim.timeout_ms as u64),
                "max_stdout_bytes": lim.max_stdout_bytes,
                "max_stderr_bytes": lim.max_stderr_bytes
            },
            "args": args_val
        }),
        Err(e) => json!({
            "command": cmd,
            "ok": true,
            "policy": policy.name.clone(),
            "decision": "denied",
            "backend": backend_str,
            "reason": e.to_string(),
            "args": args_val
        }),
    }
}

fn split_check(rest: &[String]) -> Vec<String> {
    if rest.first().map(|s| s.as_str()) == Some("--") {
        rest[1..].to_vec()
    } else {
        rest.to_vec()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let sub = args.get(1).map(String::as_str);

    let out = match sub {
        Some("policy-check") => match (args.get(2), args.get(3), args.get(4)) {
            (Some(b), Some(t), Some(m)) => {
                let check = split_check(&args[5..]);
                evaluate_to_value("policy-check", &builtin_policy(), b, t, m, check)
            }
            _ => err(
                "policy-check",
                "usage: aimt policy-check <backend> <timeout_ms> <max_output_bytes> [-- args...]",
            ),
        },

        Some("policy-check-json") => match (args.get(2), args.get(3), args.get(4), args.get(5)) {
            (Some(pj), Some(b), Some(t), Some(m)) => {
                match serde_json::from_str::<Policy>(pj) {
                    Err(e) => err("policy-check-json", &format!("invalid policy json: {e}")),
                    Ok(policy) => {
                        let check = split_check(&args[6..]);
                        evaluate_to_value("policy-check-json", &policy, b, t, m, check)
                    }
                }
            }
            _ => err(
                "policy-check-json",
                "usage: aimt policy-check-json <policy_json> <backend> <timeout_ms> <max_output_bytes> [-- args...]",
            ),
        },

        Some("help") | Some("--help") | Some("-h") => json!({
            "command": "aimt",
            "ok": true,
            "subcommands": [
                "policy-check <backend> <timeout_ms> <max_output_bytes> [-- args...]",
                "policy-check-json <policy_json> <backend> <timeout_ms> <max_output_bytes> [-- args...]",
                "help"
            ]
        }),

        Some(other) => err("aimt", &format!("unknown subcommand: {other}")),
        None => err("aimt", "missing subcommand; try: aimt help"),
    };

    println!("{out}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_backend_ok_and_err() {
        assert_eq!(parse_backend("wasm").unwrap(), BackendKind::Wasm);
        assert_eq!(parse_backend("firecracker").unwrap(), BackendKind::Firecracker);
        assert!(parse_backend("nope").is_err());
    }

    #[test]
    fn builtin_denies_dangerous_arg() {
        let v = evaluate_to_value(
            "policy-check",
            &builtin_policy(),
            "wasm",
            "5000",
            "1048576",
            vec!["rm -rf /".into()],
        );
        assert_eq!(v["ok"], true);
        assert_eq!(v["decision"], "denied");
    }

    #[test]
    fn builtin_allows_safe_arg() {
        let v = evaluate_to_value(
            "policy-check",
            &builtin_policy(),
            "wasm",
            "5000",
            "1048576",
            vec!["halo".into()],
        );
        assert_eq!(v["decision"], "allowed");
        assert_eq!(v["limits"]["timeout_ms"], json!(5000));
    }

    #[test]
    fn builtin_denies_disallowed_backend() {
        let v = evaluate_to_value(
            "policy-check",
            &builtin_policy(),
            "firecracker",
            "5000",
            "1048576",
            vec!["halo".into()],
        );
        assert_eq!(v["decision"], "denied");
    }

    #[test]
    fn clamp_timeout_to_ceiling() {
        let v = evaluate_to_value(
            "policy-check",
            &builtin_policy(),
            "wasm",
            "99999",
            "1048576",
            vec!["x".into()],
        );
        assert_eq!(v["limits"]["timeout_ms"], json!(10000));
    }

    #[test]
    fn invalid_backend_is_tool_error() {
        let v = evaluate_to_value("policy-check", &builtin_policy(), "bogus", "5000", "1048576", vec![]);
        assert_eq!(v["ok"], false);
    }

    #[test]
    fn invalid_timeout_is_tool_error() {
        let v = evaluate_to_value("policy-check", &builtin_policy(), "wasm", "abc", "1048576", vec![]);
        assert_eq!(v["ok"], false);
    }

    #[test]
    fn json_policy_parse_ok() {
        // Perhatikan: string backend = PascalCase ("Wasm") = bentuk serde BackendKind.
        let p: Policy = serde_json::from_str(
            r#"{"name":"t","allowed_backends":["Wasm"],"max_timeout_ms":1,"max_output_bytes":1,"max_arg_count":1,"allow_args":null,"deny_args":[]}"#,
        )
        .unwrap();
        assert_eq!(p.name, "t");
    }

    #[test]
    fn json_policy_parse_err() {
        assert!(serde_json::from_str::<Policy>("not json").is_err());
    }
}
