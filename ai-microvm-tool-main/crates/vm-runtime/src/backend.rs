//! Trait MicroVmBackend = "driver" ke hypervisor apa pun.
//! Firecracker / libkrun / process-jail / mock semuanya mengisinya.
//! Orkestrasi (Runtime/Pool) tidak peduli backend mana — persis seperti OS tak peduli vendor disk.
use serde::{Deserialize, Serialize};
use shared_types::{CommandRequest, CommandResult, ToolError, VmId, VmProfile};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotKind { Golden, Warm, Debug }

#[derive(Debug, Clone, Default)]
pub struct SnapshotOptions { pub kind: SnapshotKind }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotRef { pub id: String, pub profile: String, pub content_id: String }

pub trait MicroVmBackend: Send + Sync {
    fn create(&self, profile: &VmProfile) -> Result<VmId, ToolError>;
    fn stop(&self, vm_id: VmId) -> Result<(), ToolError>;
    fn destroy(&self, vm_id: VmId) -> Result<(), ToolError>;
    fn snapshot(&self, vm_id: VmId, opts: SnapshotOptions) -> Result<SnapshotRef, ToolError>;
    fn restore(&self, snap: SnapshotRef, profile: &VmProfile) -> Result<VmId, ToolError>;
    fn execute(&self, vm_id: VmId, req: CommandRequest) -> Result<CommandResult, ToolError>;
}
