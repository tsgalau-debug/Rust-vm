use thiserror::Error;

/// Taksonomi error. Agent memakai varian untuk adaptasi (retry / ganti profile / tolak).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ToolError {
    #[error("policy denied: {0}")]
    PolicyDenied(String),
    #[error("vm pool exhausted")]
    PoolExhausted,
    #[error("backend error: {0}")]
    Backend(String),
    #[error("snapshot error: {0}")]
    Snapshot(String),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("guest agent error: {0}")]
    GuestAgent(String),
    #[error("command timeout after {0} ms")]
    Timeout(u32),
    #[error("oom killed")]
    OomKilled,
    #[error("invalid capability token")]
    InvalidCapability,
    #[error("invalid request: {0}")]
    InvalidRequest(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn messages_are_descriptive() {
        assert!(ToolError::Timeout(5000).to_string().contains("5000"));
        assert!(ToolError::PolicyDenied("no net".into()).to_string().contains("no net"));
        assert_eq!(ToolError::OomKilled.to_string(), "oom killed");
    }
    #[test]
    fn implements_std_error() {
        let e: Box<dyn std::error::Error> = Box::new(ToolError::PoolExhausted);
        assert!(e.to_string().contains("exhausted"));
    }
}
