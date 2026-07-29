use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Permintaan eksekusi — analog syscall exec: argv + env + batas resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandRequest {
    pub request_id: String,
    pub lease_id: String,
    pub capability_token: String,
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: Option<String>,
    pub stdin: Option<Vec<u8>>,
    pub timeout_ms: u32,
    pub max_stdout_bytes: u64,
    pub max_stderr_bytes: u64,
    pub stream: bool,
}

/// Hasil eksekusi — analog waitpid + rusage: exit + output + metrik.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandResult {
    pub request_id: String,
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub timed_out: bool,
    pub killed: bool,
    pub metrics: ExecutionMetrics,
    pub artifacts: Vec<ArtifactRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    pub wall_time_ms: u64,
    pub user_cpu_ms: u64,
    pub sys_cpu_ms: u64,
    pub max_rss_bytes: u64,
    pub oom_killed: bool,
    pub bytes_stdout: u64,
    pub bytes_stderr: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRef {
    pub name: String,
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request() -> CommandRequest {
        let mut env = BTreeMap::new();
        env.insert("APP_ENV".into(), "sandbox".into());
        CommandRequest {
            request_id: "req_1".into(), lease_id: "lease_1".into(), capability_token: "tok".into(),
            argv: vec!["/bin/probe".into(), "--x".into()], env, cwd: Some("/tmp".into()),
            stdin: Some(vec![1, 2, 3]), timeout_ms: 5000,
            max_stdout_bytes: 1_048_576, max_stderr_bytes: 1_048_576, stream: false,
        }
    }

    #[test]
    fn request_json_roundtrip() {
        let r = sample_request();
        let s = serde_json::to_string(&r).unwrap();
        let back: CommandRequest = serde_json::from_str(&s).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn result_json_roundtrip() {
        let r = CommandResult {
            request_id: "req_1".into(), exit_code: 0, stdout: b"hello\n".to_vec(), stderr: vec![],
            timed_out: false, killed: false,
            metrics: ExecutionMetrics { wall_time_ms: 12, user_cpu_ms: 5, sys_cpu_ms: 2, max_rss_bytes: 4096, oom_killed: false, bytes_stdout: 6, bytes_stderr: 0 },
            artifacts: vec![ArtifactRef { name: "out.bin".into(), path: "/scratch/out.bin".into(), sha256: "deadbeef".into(), size_bytes: 6 }],
        };
        let s = serde_json::to_string(&r).unwrap();
        let back: CommandResult = serde_json::from_str(&s).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn env_is_deterministic_order() {
        // BTreeMap → kunci terurut saat diserialisasi (kontrak OS wajib deterministik)
        let mut env = BTreeMap::new();
        env.insert("Z".into(), "1".into());
        env.insert("A".into(), "2".into());
        let r = CommandRequest { env, ..sample_request() };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.find("\"A\"").unwrap() < s.find("\"Z\"").unwrap());
    }
}
