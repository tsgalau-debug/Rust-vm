//! wire — format biner untuk CommandRequest/CommandResult melintasi batas host<->guest (vsock).
//! Frame = header tetap 24 byte (little-endian, lintas-mesin) + payload JSON.
//! Ini bagian dari ABI: host & guest WAJIB sepakat pada layout ini.
use thiserror::Error;

use crate::protocol::{CommandRequest, CommandResult};

pub const MAGIC: u32 = 0x4149_4D57; // 'AIMW'
pub const VERSION: u16 = 1;
pub const HEADER_LEN: usize = 24;
pub const MAX_PAYLOAD: u32 = 16 * 1024 * 1024; // penolak frame raksasa (DoS)

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgType {
    Request = 1,
    Response = 2,
}

impl MsgType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Request),
            2 => Some(Self::Response),
            _ => None,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WireError {
    #[error("truncated frame")]
    Truncated,
    #[error("invalid magic")]
    InvalidMagic,
    #[error("unsupported wire version")]
    UnsupportedVersion,
    #[error("unknown message type")]
    UnknownMsgType,
    #[error("payload too large")]
    PayloadTooLarge,
    #[error("invalid payload: {0}")]
    InvalidPayload(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedFrame {
    pub msg_type: MsgType,
    pub request_id: u64,
    pub payload: Vec<u8>,
}

fn put_u32(h: &mut [u8], off: usize, v: u32) {
    h[off..off + 4].copy_from_slice(&v.to_le_bytes());
}
fn put_u16(h: &mut [u8], off: usize, v: u16) {
    h[off..off + 2].copy_from_slice(&v.to_le_bytes());
}
fn put_u64(h: &mut [u8], off: usize, v: u64) {
    h[off..off + 8].copy_from_slice(&v.to_le_bytes());
}
fn get_u32(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}
fn get_u16(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([b[off], b[off + 1]])
}
fn get_u64(b: &[u8], off: usize) -> u64 {
    u64::from_le_bytes([
        b[off], b[off + 1], b[off + 2], b[off + 3], b[off + 4], b[off + 5], b[off + 6], b[off + 7],
    ])
}

/// Susun satu frame. Menolak payload > MAX_PAYLOAD di sisi pengirim.
pub fn encode_frame(msg_type: MsgType, request_id: u64, payload: &[u8]) -> Result<Vec<u8>, WireError> {
    if payload.len() > MAX_PAYLOAD as usize {
        return Err(WireError::PayloadTooLarge);
    }
    let mut h = [0u8; HEADER_LEN];
    put_u32(&mut h, 0, MAGIC);
    put_u16(&mut h, 4, VERSION);
    h[6] = msg_type as u8;
    h[7] = 0; // flags
    put_u64(&mut h, 8, request_id);
    put_u32(&mut h, 16, payload.len() as u32);
    put_u32(&mut h, 20, 0); // reserved

    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(&h);
    out.extend_from_slice(payload);
    Ok(out)
}

/// Urai satu frame dari depan buffer.
pub fn decode_frame(buf: &[u8]) -> Result<DecodedFrame, WireError> {
    if buf.len() < HEADER_LEN {
        return Err(WireError::Truncated);
    }
    if get_u32(buf, 0) != MAGIC {
        return Err(WireError::InvalidMagic);
    }
    if get_u16(buf, 4) != VERSION {
        return Err(WireError::UnsupportedVersion);
    }
    let msg_type = MsgType::from_u8(buf[6]).ok_or(WireError::UnknownMsgType)?;
    let request_id = get_u64(buf, 8);
    let payload_len = get_u32(buf, 16);
    if payload_len > MAX_PAYLOAD {
        return Err(WireError::PayloadTooLarge);
    }
    let end = HEADER_LEN + payload_len as usize;
    if buf.len() < end {
        return Err(WireError::Truncated);
    }
    Ok(DecodedFrame {
        msg_type,
        request_id,
        payload: buf[HEADER_LEN..end].to_vec(),
    })
}

// ---- helper bertipe: Request / Response ----

pub fn encode_request(request_id: u64, req: &CommandRequest) -> Result<Vec<u8>, WireError> {
    let payload = serde_json::to_vec(req).map_err(|e| WireError::InvalidPayload(e.to_string()))?;
    encode_frame(MsgType::Request, request_id, &payload)
}

pub fn decode_request(buf: &[u8]) -> Result<(u64, CommandRequest), WireError> {
    let f = decode_frame(buf)?;
    if f.msg_type != MsgType::Request {
        return Err(WireError::InvalidPayload("expected Request frame".into()));
    }
    let req: CommandRequest =
        serde_json::from_slice(&f.payload).map_err(|e| WireError::InvalidPayload(e.to_string()))?;
    Ok((f.request_id, req))
}

pub fn encode_response(request_id: u64, res: &CommandResult) -> Result<Vec<u8>, WireError> {
    let payload = serde_json::to_vec(res).map_err(|e| WireError::InvalidPayload(e.to_string()))?;
    encode_frame(MsgType::Response, request_id, &payload)
}

pub fn decode_response(buf: &[u8]) -> Result<(u64, CommandResult), WireError> {
    let f = decode_frame(buf)?;
    if f.msg_type != MsgType::Response {
        return Err(WireError::InvalidPayload("expected Response frame".into()));
    }
    let res: CommandResult =
        serde_json::from_slice(&f.payload).map_err(|e| WireError::InvalidPayload(e.to_string()))?;
    Ok((f.request_id, res))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{ArtifactRef, ExecutionMetrics};
    use std::collections::BTreeMap;

    fn req() -> CommandRequest {
        let mut env = BTreeMap::new();
        env.insert("APP_ENV".into(), "sandbox".into());
        CommandRequest {
            request_id: "r1".into(),
            lease_id: "l1".into(),
            capability_token: "tok".into(),
            argv: vec!["/bin/probe".into(), "--x".into()],
            env,
            cwd: Some("/tmp".into()),
            stdin: Some(vec![9, 8, 7]),
            timeout_ms: 5000,
            max_stdout_bytes: 1024,
            max_stderr_bytes: 1024,
            stream: false,
        }
    }

    fn res() -> CommandResult {
        CommandResult {
            request_id: "r1".into(),
            exit_code: 0,
            stdout: b"ok\n".to_vec(),
            stderr: vec![],
            timed_out: false,
            killed: false,
            metrics: ExecutionMetrics {
                wall_time_ms: 3,
                user_cpu_ms: 1,
                sys_cpu_ms: 0,
                max_rss_bytes: 4096,
                oom_killed: false,
                bytes_stdout: 3,
                bytes_stderr: 0,
            },
            artifacts: vec![ArtifactRef {
                name: "o".into(),
                path: "/scratch/o".into(),
                sha256: "cafe".into(),
                size_bytes: 3,
            }],
        }
    }

    fn hdr(version: u16, msg_type: u8, payload_len: u32) -> [u8; HEADER_LEN] {
        let mut h = [0u8; HEADER_LEN];
        put_u32(&mut h, 0, MAGIC);
        put_u16(&mut h, 4, version);
        h[6] = msg_type;
        h[7] = 0;
        put_u64(&mut h, 8, 1);
        put_u32(&mut h, 16, payload_len);
        put_u32(&mut h, 20, 0);
        h
    }

    #[test]
    fn round_trip_request() {
        let r = req();
        let bytes = encode_request(42, &r).unwrap();
        let (id, back) = decode_request(&bytes).unwrap();
        assert_eq!(id, 42);
        assert_eq!(back, r);
    }

    #[test]
    fn round_trip_response() {
        let r = res();
        let bytes = encode_response(7, &r).unwrap();
        let (id, back) = decode_response(&bytes).unwrap();
        assert_eq!(id, 7);
        assert_eq!(back, r);
    }

    #[test]
    fn invalid_magic() {
        let mut bytes = encode_request(1, &req()).unwrap();
        bytes[0] = 0;
        assert_eq!(decode_frame(&bytes).unwrap_err(), WireError::InvalidMagic);
    }

    #[test]
    fn unsupported_version() {
        let h = hdr(2, 1, 0);
        assert_eq!(decode_frame(&h).unwrap_err(), WireError::UnsupportedVersion);
    }

    #[test]
    fn unknown_msg_type() {
        let h = hdr(1, 99, 0);
        assert_eq!(decode_frame(&h).unwrap_err(), WireError::UnknownMsgType);
    }

    #[test]
    fn truncated_header() {
        assert_eq!(decode_frame(&[0u8; 10]).unwrap_err(), WireError::Truncated);
    }

    #[test]
    fn truncated_payload() {
        let mut bytes = encode_request(1, &req()).unwrap();
        let cut = HEADER_LEN + 2; // header utuh, payload terpotong
        bytes.truncate(cut);
        assert_eq!(decode_frame(&bytes).unwrap_err(), WireError::Truncated);
    }

    #[test]
    fn payload_too_large_rejected() {
        // sisi pengirim menolak dulu ...
        assert_eq!(
            encode_frame(MsgType::Request, 1, &vec![0; MAX_PAYLOAD as usize + 1]).unwrap_err(),
            WireError::PayloadTooLarge
        );
        // ... dan sisi penerima menolak dari header walau body belum ada
        let h = hdr(1, 1, MAX_PAYLOAD + 1);
        assert_eq!(decode_frame(&h).unwrap_err(), WireError::PayloadTooLarge);
    }
}
