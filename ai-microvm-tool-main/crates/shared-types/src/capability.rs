use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySet {
    pub net: bool,
    pub write_paths: Vec<String>,
    pub read_paths: Vec<String>,
    pub max_processes: u32,
    pub max_open_files: u32,
    pub allow_syscalls: Vec<String>,
    pub deny_syscalls: Vec<String>,
}

impl CapabilitySet {
    /// Capability paling ketat: tanpa jaringan, tanpa proses tambahan.
    pub fn minimal() -> Self {
        Self { net: false, write_paths: vec![], read_paths: vec![], max_processes: 1, max_open_files: 16, allow_syscalls: vec![], deny_syscalls: vec![] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn minimal_has_no_net() {
        let c = CapabilitySet::minimal();
        assert!(!c.net);
        assert_eq!(c.max_processes, 1);
    }
    #[test]
    fn serde_roundtrip() {
        let c = CapabilitySet { net: true, write_paths: vec!["/tmp".into()], ..CapabilitySet::minimal() };
        let s = serde_json::to_string(&c).unwrap();
        let back: CapabilitySet = serde_json::from_str(&s).unwrap();
        assert_eq!(back, c);
    }
}
