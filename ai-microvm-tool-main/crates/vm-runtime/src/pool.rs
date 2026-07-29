//! VmPool = tabel proses kernel. Slot = satu VmTcb (fixed-size) di dalam Vec → O(1) by index.
//! free-list = slot yang sedang Ready. generation = penjaga anti-ABA: handle lama yang
//! menunjuk slot yang sudah dipakai ulang akan ditolak (stale).
use std::sync::Mutex;

use shared_types::{ToolError, VmId, VmProfile, VmState, VmTcb};

use crate::backend::MicroVmBackend;
use crate::lifecycle::can_transition;

#[derive(Debug, Clone, Copy)]
pub struct VmHandle {
    pub slot: usize,
    pub generation: u32,
    pub vm_id: VmId,
}

struct Inner {
    tcbs: Vec<VmTcb>,
    free: Vec<usize>,
}

pub struct VmPool {
    inner: Mutex<Inner>,
}

impl VmPool {
    pub fn new() -> Self {
        Self { inner: Mutex::new(Inner { tcbs: Vec::new(), free: Vec::new() }) }
    }

    fn transition(t: &mut VmTcb, to: VmState) -> Result<(), ToolError> {
        let cur = t.state().ok_or_else(|| ToolError::InvalidRequest("corrupt state byte".into()))?;
        if !can_transition(cur, to) {
            return Err(ToolError::InvalidRequest(format!("illegal transition {cur:?}->{to:?}")));
        }
        t.state = to.to_u8();
        t.bump_generation();
        Ok(())
    }

    /// Panaskan satu VM (via backend.create) dan taruh di tabel sebagai Ready.
    pub fn warm(&self, profile: &VmProfile, backend: &dyn MicroVmBackend) -> Result<VmId, ToolError> {
        let vm_id = backend.create(profile)?;
        let pid = crate::fnv1a_64(&profile.name);
        let mut g = self.inner.lock().unwrap();
        g.tcbs.push(VmTcb::new(vm_id.0, pid, VmState::Ready));
        let idx = g.tcbs.len() - 1;
        g.free.push(idx);
        Ok(vm_id)
    }

    /// Ambil slot Ready milik profile ini → Leased. Kosong = PoolExhausted.
    pub fn acquire(&self, profile_id: u64) -> Result<VmHandle, ToolError> {
        let mut g = self.inner.lock().unwrap();
        let pos = g.free.iter().position(|&i| {
            g.tcbs[i].profile_id == profile_id && g.tcbs[i].state() == Some(VmState::Ready)
        });
        let idx = pos.ok_or(ToolError::PoolExhausted)?;
        g.free.swap_remove(idx);
        let t = &mut g.tcbs[idx];
        Self::transition(t, VmState::Leased)?;
        Ok(VmHandle { slot: idx, generation: t.generation, vm_id: VmId(t.vm_id) })
    }

    /// Leased → Running (memulai eksekusi).
    pub fn start(&self, handle: VmHandle) -> Result<VmHandle, ToolError> {
        let mut g = self.inner.lock().unwrap();
        let t = &mut g.tcbs[handle.slot];
        if t.generation != handle.generation {
            return Err(ToolError::InvalidRequest("stale handle".into()));
        }
        Self::transition(t, VmState::Running)?;
        Ok(VmHandle { slot: handle.slot, generation: t.generation, vm_id: VmId(t.vm_id) })
    }

    /// Running → Scrubbing → Ready, kembalikan slot ke free-list.
    pub fn release(&self, handle: VmHandle) -> Result<(), ToolError> {
        let mut g = self.inner.lock().unwrap();
        let t = &mut g.tcbs[handle.slot];
        if t.generation != handle.generation {
            return Err(ToolError::InvalidRequest("stale handle".into()));
        }
        Self::transition(t, VmState::Scrubbing)?;
        Self::transition(t, VmState::Ready)?;
        g.free.push(handle.slot);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{SnapshotOptions, SnapshotRef};
    use shared_types::{BackendKind, CapabilitySet, CommandRequest, CommandResult, ExecutionMetrics, SnapshotPolicy};
    use std::collections::BTreeMap;
    use std::sync::Mutex as StdMutex;

    fn profile(name: &str) -> VmProfile {
        VmProfile {
            name: name.into(), backend: BackendKind::Firecracker, kernel: "k".into(), rootfs: "r".into(),
            vcpu: 1, mem_mib: 256, timeout_ms: 5000, capabilities: CapabilitySet::minimal(),
            snapshot_policy: SnapshotPolicy::GoldenOnly, labels: BTreeMap::new(),
        }
    }
    fn pid(name: &str) -> u64 { crate::fnv1a_64(name) }

    struct Mock { calls: StdMutex<Vec<String>>, seq: StdMutex<u64> }
    impl Mock {
        fn new() -> Self { Self { calls: StdMutex::new(vec![]), seq: StdMutex::new(0) } }
        fn record(&self, s: String) { self.calls.lock().unwrap().push(s); }
    }
    impl MicroVmBackend for Mock {
        fn create(&self, p: &VmProfile) -> Result<VmId, ToolError> {
            let mut s = self.seq.lock().unwrap(); *s += 1; let id = *s;
            self.record(format!("create:{id}:{}", p.name)); Ok(VmId(id))
        }
        fn stop(&self, v: VmId) -> Result<(), ToolError> { self.record(format!("stop:{}", v.0)); Ok(()) }
        fn destroy(&self, v: VmId) -> Result<(), ToolError> { self.record(format!("destroy:{}", v.0)); Ok(()) }
        fn snapshot(&self, v: VmId, _: SnapshotOptions) -> Result<SnapshotRef, ToolError> {
            self.record(format!("snapshot:{}", v.0)); Ok(SnapshotRef { id: "s".into(), profile: "p".into(), content_id: "c".into() })
        }
        fn restore(&self, s: SnapshotRef, _: &VmProfile) -> Result<VmId, ToolError> {
            self.record(format!("restore:{}", s.id)); Ok(VmId(999))
        }
        fn execute(&self, v: VmId, _: CommandRequest) -> Result<CommandResult, ToolError> {
            self.record(format!("execute:{}", v.0));
            Ok(CommandResult {
                request_id: "r".into(), exit_code: 0, stdout: b"ok\n".to_vec(), stderr: vec![],
                timed_out: false, killed: false,
                metrics: ExecutionMetrics { wall_time_ms: 1, user_cpu_ms: 1, sys_cpu_ms: 0, max_rss_bytes: 0, oom_killed: false, bytes_stdout: 3, bytes_stderr: 0 },
                artifacts: vec![],
            })
        }
    }

    #[test]
    fn warm_acquire_release_reuses_slot() {
        let pool = VmPool::new();
        let mock = Mock::new();
        let p = profile("a");
        pool.warm(&p, &mock).unwrap();

        let h1 = pool.acquire(pid("a")).unwrap();
        let h2 = pool.start(h1).unwrap();
        assert!(h2.generation > h1.generation, "generation harus naik tiap transisi");
        pool.release(h2).unwrap();

        let h3 = pool.acquire(pid("a")).unwrap();
        assert_eq!(h3.slot, h1.slot, "slot bekas release harus dipakai ulang");
    }

    #[test]
    fn acquire_without_warm_is_exhausted() {
        let pool = VmPool::new();
        assert_eq!(pool.acquire(pid("a")).unwrap_err(), ToolError::PoolExhausted);
    }

    #[test]
    fn profile_mismatch_is_exhausted() {
        let pool = VmPool::new();
        let mock = Mock::new();
        pool.warm(&profile("a"), &mock).unwrap();
        assert_eq!(pool.acquire(pid("b")).unwrap_err(), ToolError::PoolExhausted);
    }

    #[test]
    fn stale_handle_rejected() {
        let pool = VmPool::new();
        let mock = Mock::new();
        pool.warm(&profile("a"), &mock).unwrap();
        let h1 = pool.acquire(pid("a")).unwrap();
        let _h2 = pool.start(h1).unwrap(); // generation naik
        let stale = pool.release(h1);      // h1 membawa generation lama
        assert!(matches!(stale, Err(ToolError::InvalidRequest(_))));
    }
}
