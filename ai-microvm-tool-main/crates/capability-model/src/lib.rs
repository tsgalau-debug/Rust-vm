//! capability-model — "security module" (LSM) kernel.
//! Duduk di antara orkestrator dan vm-runtime: menolak permintaan SEBELUM
//! menyentuh backend. Murni & deterministik; pemuatan file ada di lapisan atas.
use serde::{Deserialize, Serialize};
use shared_types::{BackendKind, CommandRequest, ToolError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    pub name: String,
    pub allowed_backends: Vec<BackendKind>,
    pub max_timeout_ms: u32,
    pub max_output_bytes: u64,
    pub max_arg_count: usize,
    pub allow_args: Option<Vec<String>>,
    pub deny_args: Vec<String>,
}

/// Batas yang BENAR-BENAR ditegakkan setelah clamp (policy = ceiling).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub timeout_ms: u32,
    pub max_stdout_bytes: u64,
    pub max_stderr_bytes: u64,
}

fn matches_any(arg: &str, pats: &[String]) -> bool {
    pats.iter().any(|p| arg.contains(p.as_str()))
}

impl Policy {
    /// Evaluasi satu permintaan. Ok(Limits) = lolos (dengan clamp);
    /// Err(PolicyDenied) = ditolak pre-flight (backend tidak dijalankan).
    pub fn evaluate(&self, backend: BackendKind, req: &CommandRequest) -> Result<Limits, ToolError> {
        if !self.allowed_backends.contains(&backend) {
            return Err(ToolError::PolicyDenied(format!(
                "backend '{backend:?}' tidak diizinkan oleh policy '{}'",
                self.name
            )));
        }

        if req.argv.len() > self.max_arg_count {
            return Err(ToolError::PolicyDenied(format!(
                "jumlah argumen {} melebihi max_arg_count {}",
                req.argv.len(),
                self.max_arg_count
            )));
        }

        for arg in &req.argv {
            if matches_any(arg, &self.deny_args) {
                return Err(ToolError::PolicyDenied(format!(
                    "argumen mengandung pola terlarang: {arg:?}"
                )));
            }
        }

        if let Some(allow) = &self.allow_args {
            for arg in &req.argv {
                if !matches_any(arg, allow) {
                    return Err(ToolError::PolicyDenied(format!(
                        "argumen tidak cocok allowlist: {arg:?}"
                    )));
                }
            }
        }

        Ok(Limits {
            timeout_ms: req.timeout_ms.min(self.max_timeout_ms),
            max_stdout_bytes: req.max_stdout_bytes.min(self.max_output_bytes),
            max_stderr_bytes: req.max_stderr_bytes.min(self.max_output_bytes),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn req(argv: &[&str], timeout_ms: u32) -> CommandRequest {
        CommandRequest {
            request_id: "r".into(),
            lease_id: "l".into(),
            capability_token: "t".into(),
            argv: argv.iter().map(|s| s.to_string()).collect(),
            env: BTreeMap::new(),
            cwd: None,
            stdin: None,
            timeout_ms,
            max_stdout_bytes: 9_999_999,
            max_stderr_bytes: 9_999_999,
            stream: false,
        }
    }

    fn pol() -> Policy {
        Policy {
            name: "default".into(),
            allowed_backends: vec![BackendKind::Wasm],
            max_timeout_ms: 10_000,
            max_output_bytes: 1_000_000,
            max_arg_count: 64,
            allow_args: None,
            deny_args: vec!["rm -rf".into(), "sudo".into()],
        }
    }

    #[test]
    fn allowed_backend_passes_with_clamp() {
        let lim = pol().evaluate(BackendKind::Wasm, &req(&["halo"], 5000)).unwrap();
        assert_eq!(lim.timeout_ms, 5000);
        assert_eq!(lim.max_stdout_bytes, 1_000_000); // 9_999_999 di-clamp ke ceiling
    }

    #[test]
    fn backend_not_allowed_is_denied() {
        let err = pol().evaluate(BackendKind::Firecracker, &req(&["halo"], 1000)).unwrap_err();
        match err {
            ToolError::PolicyDenied(m) => assert!(m.contains("Firecracker")),
            e => panic!("salah varian: {e:?}"),
        }
    }

    #[test]
    fn deny_pattern_is_denied() {
        let err = pol().evaluate(BackendKind::Wasm, &req(&["rm -rf /"], 1000)).unwrap_err();
        assert!(matches!(err, ToolError::PolicyDenied(_)));
    }

    #[test]
    fn allowlist_rejects_unlisted() {
        let mut p = pol();
        p.allow_args = Some(vec!["halo".into()]);
        let err = p.evaluate(BackendKind::Wasm, &req(&["jahat"], 1000)).unwrap_err();
        assert!(matches!(err, ToolError::PolicyDenied(_)));
    }

    #[test]
    fn allowlist_accepts_match() {
        let mut p = pol();
        p.allow_args = Some(vec!["halo".into()]);
        assert!(p.evaluate(BackendKind::Wasm, &req(&["halo dunia"], 1000)).is_ok());
    }

    #[test]
    fn too_many_args_is_denied() {
        let mut p = pol();
        p.max_arg_count = 2;
        let err = p.evaluate(BackendKind::Wasm, &req(&["a", "b", "c"], 1000)).unwrap_err();
        assert!(matches!(err, ToolError::PolicyDenied(_)));
    }

    #[test]
    fn clamp_timeout_and_output() {
        let lim = pol().evaluate(BackendKind::Wasm, &req(&["x"], 99_999)).unwrap();
        assert_eq!(lim.timeout_ms, 10_000); // 99_999 di-clamp
        assert_eq!(lim.max_stderr_bytes, 1_000_000);
    }

    #[test]
    fn policy_serde_roundtrip() {
        let p = pol();
        let s = serde_json::to_string(&p).unwrap();
        let back: Policy = serde_json::from_str(&s).unwrap();
        assert_eq!(back, p);
    }
}
