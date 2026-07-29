use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::capability::CapabilitySet;
use crate::BackendKind;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotPolicy { Never, GoldenOnly, GoldenPlusWarmPool, Persistent }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmProfile {
    pub name: String,
    pub backend: BackendKind,
    pub kernel: String,
    pub rootfs: String,
    pub vcpu: u8,
    pub mem_mib: u16,
    pub timeout_ms: u32,
    pub capabilities: CapabilitySet,
    pub snapshot_policy: SnapshotPolicy,
    pub labels: BTreeMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_serde_roundtrip() {
        let p = VmProfile {
            name: "firecracker-python".into(), backend: BackendKind::Firecracker,
            kernel: "vmlinux-6.1".into(), rootfs: "python.ext4".into(),
            vcpu: 1, mem_mib: 256, timeout_ms: 5000,
            capabilities: CapabilitySet::minimal(),
            snapshot_policy: SnapshotPolicy::GoldenPlusWarmPool,
            labels: BTreeMap::from([("kvm".into(), "true".into())]),
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: VmProfile = serde_json::from_str(&s).unwrap();
        assert_eq!(back, p);
    }
}
