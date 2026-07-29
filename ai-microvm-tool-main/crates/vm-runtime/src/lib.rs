//! vm-runtime — lapisan orkestrasi "OS".
//! Runtime::execute adalah wujud dari north star: satu panggilan, balik CommandResult,
//! apapun backend di belakangnya. Policy gate & konteks orkestrasi dilapisi di atas ini.

use shared_types::{CommandRequest, CommandResult, ToolError, VmProfile};

pub mod backend;
pub mod lifecycle;
pub mod pool;
pub mod snapshot;

pub use backend::{MicroVmBackend, SnapshotKind, SnapshotOptions, SnapshotRef};
pub use lifecycle::can_transition;
pub use pool::{VmHandle, VmPool};
pub use snapshot::SnapshotManifest;

/// FNV-1a 64-bit: hash deterministik & stabil lintas run/mesin (BUKAN DefaultHasher).
/// Dipakai untuk memetakan nama profile → profile_id di dalam VmTcb.
pub(crate) fn fnv1a_64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub struct Runtime<B: MicroVmBackend> {
    pool: VmPool,
    backend: B,
}

impl<B: MicroVmBackend> Runtime<B> {
    pub fn new(backend: B) -> Self {
        Self { pool: VmPool::new(), backend }
    }

    /// Eksekusi satu permintaan. Auto-warm on miss (supaya "just works"),
    /// lalu acquire → start → execute → release (slot selalu dikembalikan).
    pub fn execute(&self, profile: &VmProfile, req: CommandRequest) -> Result<CommandResult, ToolError> {
        let pid = fnv1a_64(&profile.name);

        let handle = match self.pool.acquire(pid) {
            Ok(h) => h,
            Err(ToolError::PoolExhausted) => {
                self.pool.warm(profile, &self.backend)?;
                self.pool.acquire(pid)?
            }
            Err(e) => return Err(e),
        };

        let handle = self.pool.start(handle)?;
        let result = self.backend.execute(handle.vm_id, req);
        let _ = self.pool.release(handle); // best-effort; slot wajib kembali
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{SnapshotOptions, SnapshotRef};
    use shared_types::{BackendKind, CapabilitySet, ExecutionMetrics, SnapshotPolicy, VmId};
    use std::collections::BTreeMap;
    use std::sync::Mutex as StdMutex;

    fn profile(name: &str) -> VmProfile {
        VmProfile {
            name: name.into(), backend: BackendKind::Firecracker, kernel: "k".into(), rootfs: "r".into(),
            vcpu: 1, mem_mib: 256, timeout_ms: 5000, capabilities: CapabilitySet::minimal(),
            snapshot_policy: SnapshotPolicy::GoldenOnly, labels: BTreeMap::new(),
        }
    }
    fn req() -> CommandRequest {
        CommandRequest {
            request_id: "r".into(), lease_id: "l".into(), capability_token: "t".into(),
            argv: vec!["x".into()], env: BTreeMap::new(), cwd: None, stdin: None,
            timeout_ms: 1000, max_stdout_bytes: 1024, max_stderr_bytes: 1024, stream: false,
        }
    }

    struct Mock {
        calls: StdMutex<Vec<String>>,
        seq: StdMutex<u64>,
        fail_execute: bool,
    }
    impl Mock {
        fn new(fail: bool) -> Self { Self { calls: StdMutex::new(vec![]), seq: StdMutex::new(0), fail_execute: fail } }
        fn count(&self, prefix: &str) -> usize { self.calls.lock().unwrap().iter().filter(|c| c.starts_with(prefix)).count() }
    }
    impl MicroVmBackend for Mock {
        fn create(&self, p: &VmProfile) -> Result<VmId, ToolError> {
            let mut s = self.seq.lock().unwrap(); *s += 1; let id = *s;
            self.calls.lock().unwrap().push(format!("create:{id}:{}", p.name)); Ok(VmId(id))
        }
        fn stop(&self, _: VmId) -> Result<(), ToolError> { Ok(()) }
        fn destroy(&self, _: VmId) -> Result<(), ToolError> { Ok(()) }
        fn snapshot(&self, _: VmId, _: SnapshotOptions) -> Result<SnapshotRef, ToolError> {
            Ok(SnapshotRef { id: "s".into(), profile: "p".into(), content_id: "c".into() })
        }
        fn restore(&self, s: SnapshotRef, _: &VmProfile) -> Result<VmId, ToolError> { Ok(VmId(1)) }
        fn execute(&self, v: VmId, _: CommandRequest) -> Result<CommandResult, ToolError> {
            self.calls.lock().unwrap().push(format!("execute:{}", v.0));
            if self.fail_execute { return Err(ToolError::GuestAgent("boom".into())); }
            Ok(CommandResult {
                request_id: "r".into(), exit_code: 0, stdout: b"ok\n".to_vec(), stderr: vec![],
                timed_out: false, killed: false,
                metrics: ExecutionMetrics { wall_time_ms: 1, user_cpu_ms: 1, sys_cpu_ms: 0, max_rss_bytes: 0, oom_killed: false, bytes_stdout: 3, bytes_stderr: 0 },
                artifacts: vec![],
            })
        }
    }

    #[test]
    fn execute_auto_warms_and_pools() {
        let rt = Runtime::new(Mock::new(false));
        let p = profile("a");
        let r1 = rt.execute(&p, req()).unwrap();
        let r2 = rt.execute(&p, req()).unwrap();
        assert_eq!(r1.stdout, b"ok\n");
        assert_eq!(r2.exit_code, 0);
        // dua eksekusi, tapi hanya SATU create → slot dipakai ulang (pooling terbukti)
        assert_eq!(rt.backend.count("create:"), 1);
        assert_eq!(rt.backend.count("execute:"), 2);
    }

    #[test]
    fn execute_propagates_backend_error() {
        let rt = Runtime::new(Mock::new(true));
        let err = rt.execute(&profile("a"), req()).unwrap_err();
        assert_eq!(err, ToolError::GuestAgent("boom".into()));
    }
}
