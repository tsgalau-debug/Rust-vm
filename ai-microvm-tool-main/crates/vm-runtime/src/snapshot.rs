//! SnapshotManifest: identitas snapshot = hash dari isinya (kernel+rootfs+policy+agent).
//! Ini fondasi "golden snapshot" & reset deterministik: dua build identik → id identik.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shared_types::BackendKind;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotManifest {
    pub profile: String,
    pub backend: BackendKind,
    pub kernel_sha256: String,
    pub rootfs_sha256: String,
    pub policy_sha256: String,
    pub guest_agent_sha256: String,
    pub version: u32,
    /// content-addressed id (SHA-256 dari seluruh field di atas, hex).
    pub content_id: String,
}

impl SnapshotManifest {
    pub fn new(
        profile: impl Into<String>,
        backend: BackendKind,
        kernel_sha256: impl Into<String>,
        rootfs_sha256: impl Into<String>,
        policy_sha256: impl Into<String>,
        guest_agent_sha256: impl Into<String>,
        version: u32,
    ) -> Self {
        let mut m = Self {
            profile: profile.into(),
            backend,
            kernel_sha256: kernel_sha256.into(),
            rootfs_sha256: rootfs_sha256.into(),
            policy_sha256: policy_sha256.into(),
            guest_agent_sha256: guest_agent_sha256.into(),
            version,
            content_id: String::new(),
        };
        m.content_id = m.compute_content_id();
        m
    }

    fn canonical(&self) -> String {
        // label + NUL pemisah → tidak ada dua kombinasi field yang "bertabrakan"
        format!(
            "profile\0{}\0backend\0{:?}\0kernel\0{}\0rootfs\0{}\0policy\0{}\0agent\0{}\0version\0{}",
            self.profile, self.backend, self.kernel_sha256, self.rootfs_sha256,
            self.policy_sha256, self.guest_agent_sha256, self.version
        )
    }

    fn compute_content_id(&self) -> String {
        let mut h = Sha256::new();
        h.update(self.canonical().as_bytes());
        let out = h.finalize();
        hex_lower(&out)
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(kernel: &str) -> SnapshotManifest {
        SnapshotManifest::new("p", BackendKind::Firecracker, kernel, "r", "pol", "ag", 1)
    }

    #[test]
    fn deterministic() {
        assert_eq!(mk("k").content_id, mk("k").content_id);
    }

    #[test]
    fn different_input_different_id() {
        assert_ne!(mk("k1").content_id, mk("k2").content_id);
    }

    #[test]
    fn id_is_64_hex_chars() {
        let id = mk("k").content_id;
        assert_eq!(id.len(), 64);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
