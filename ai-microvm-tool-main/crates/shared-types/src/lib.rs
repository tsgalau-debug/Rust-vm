//! shared-types — kontrak "OS" untuk microVM.
//! Stabil, minimal, bebas backend/tool/agent. Seperti header kernel:
//! semua lapisan berbicara lewat tipe-tipe ini.

use serde::{Deserialize, Serialize};

pub mod capability;
pub mod error;
pub mod profile;
pub mod protocol;
pub mod tcb;

pub use capability::CapabilitySet;
pub use error::ToolError;
pub use profile::{SnapshotPolicy, VmProfile};
pub use protocol::{ArtifactRef, CommandRequest, CommandResult, ExecutionMetrics};
pub use tcb::VmTcb;

/// Identitas VM. Newtype supaya tidak tertukar dengan u64 biasa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VmId(pub u64);

impl From<u64> for VmId {
    fn from(v: u64) -> Self { VmId(v) }
}

/// State machine siklus hidup VM (mirip state proses di kernel).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VmState {
    Building = 0,
    Warming = 1,
    Ready = 2,
    Leased = 3,
    Running = 4,
    Scrubbing = 5,
    Snapshotting = 6,
    Faulted = 7,
    Paused = 8,
    Destroyed = 9,
}

impl VmState {
    pub fn to_u8(self) -> u8 { self as u8 }
    pub fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            0 => Self::Building, 1 => Self::Warming, 2 => Self::Ready,
            3 => Self::Leased, 4 => Self::Running, 5 => Self::Scrubbing,
            6 => Self::Snapshotting, 7 => Self::Faulted, 8 => Self::Paused,
            9 => Self::Destroyed, _ => return None,
        })
    }
}

/// Jenis backend eksekusi. Dipilih oleh router, bukan hardcoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BackendKind {
    Wasm,
    ProcessJail,
    Firecracker,
    Libkrun,
    Remote,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vm_id_from_u64_and_copy() {
        let a = VmId::from(42);
        let b = VmId(42);
        assert_eq!(a, b);
        let c = a; // Copy
        assert_eq!(c.0, 42);
    }

    #[test]
    fn vm_state_roundtrip_u8() {
        for v in 0..=9u8 {
            let s = VmState::from_u8(v).expect("valid state");
            assert_eq!(s.to_u8(), v);
        }
        assert!(VmState::from_u8(255).is_none());
        assert_eq!(VmState::Running.to_u8(), 4);
    }

    #[test]
    fn backend_kind_serde_roundtrip() {
        for k in [BackendKind::Wasm, BackendKind::ProcessJail, BackendKind::Firecracker, BackendKind::Libkrun, BackendKind::Remote] {
            let s = serde_json::to_string(&k).unwrap();
            let back: BackendKind = serde_json::from_str(&s).unwrap();
            assert_eq!(back, k);
        }
    }
}
