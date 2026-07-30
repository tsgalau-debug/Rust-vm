//! wasm-fs — tool inspeksi & tulis filesystem di dalam sandbox WASI.
//! Subcommand: stat | read | list | write.
//!   write <path> <content>  -> buat/timpa file (semantik overwrite).
//! Kontrak: SELALU cetak tepat satu baris JSON; sukses/gagal di field `ok`
//! (bukan exit code) — exit code proses = "tool crash atau tidak".
//! Nol panic: semua Result di-match.
//!
//! Catatan batas: konten `write` lewat argv → untuk konten kompleks via shell
//! perhatikan quoting; via pintu HTTP (/api/run, args = array JSON) bersih total.
//! Jalur stdin (tanpa argv) = jangkar 1b'.

use serde_json::{json, Value};
use std::io::Read;

const READ_LIMIT: usize = 65_536;  // batas baca per op
const WRITE_LIMIT: usize = 65_536; // batas tulis per op (juga jaga argv raksasa)

fn read_up_to<R: Read>(mut r: R, limit: usize) -> std::io::Result<(Vec<u8>, bool)> {
    let mut out: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        if out.len() >= limit {
            let n = r.read(&mut tmp)?;
            return Ok((out, n > 0));
        }
        let want = (limit - out.len()).min(tmp.len());
        let n = r.read(&mut tmp[..want])?;
        if n == 0 {
            return Ok((out, false));
        }
        out.extend_from_slice(&tmp[..n]);
    }
}

fn check_write_len(n: usize) -> Result<(), String> {
    if n > WRITE_LIMIT {
        Err(format!("content too large ({n} > {WRITE_LIMIT})"))
    } else {
        Ok(())
    }
}

fn err_json(op: &str, msg: &str) -> Value {
    json!({ "op": op, "ok": false, "error": msg })
}

fn usage_json() -> Value {
    json!({ "op": "usage", "ok": false, "error": "usage: wasm-fs <stat|read|list|write> <path> [content]" })
}

fn op_stat(p: &str) -> Value {
    match std::fs::metadata(p) {
        Ok(m) => {
            let kind = if m.is_file() {
                "file"
            } else if m.is_dir() {
                "dir"
            } else if m.is_symlink() {
                "symlink"
            } else {
                "unknown"
            };
            let mtime: Value = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| Value::from(d.as_secs()))
                .unwrap_or(Value::Null);
            json!({ "op": "stat", "path": p, "ok": true, "kind": kind, "size": m.len(), "mtime_unix": mtime })
        }
        Err(e) => err_json("stat", &format!("{e}")),
    }
}

fn op_read(p: &str) -> Value {
    let total = std::fs::metadata(p).ok().map(|m| m.len());
    let f = match std::fs::File::open(p) {
        Ok(f) => f,
        Err(e) => return err_json("read", &format!("open: {e}")),
    };
    let (bytes, truncated) = match read_up_to(f, READ_LIMIT) {
        Ok(x) => x,
        Err(e) => return err_json("read", &format!("read: {e}")),
    };
    let bytes_read = bytes.len() as u64;
    let content = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return err_json("read", "not utf-8 (binary file)"),
    };
    let size_val: Value = total.map(Value::from).unwrap_or(Value::Null);
    json!({
        "op": "read", "path": p, "ok": true,
        "size": size_val, "bytes_read": bytes_read,
        "truncated": truncated, "content": content
    })
}

fn op_list(p: &str) -> Value {
    let rd = match std::fs::read_dir(p) {
        Ok(rd) => rd,
        Err(e) => return err_json("list", &format!("read_dir: {e}")),
    };
    let mut entries: Vec<Value> = Vec::new();
    for e in rd {
        if let Ok(de) = e {
            let name = de.file_name().to_string_lossy().into_owned();
            let (kind, size) = match de.metadata() {
                Ok(m) => (
                    if m.is_dir() { "dir" } else if m.is_file() { "file" } else { "other" },
                    m.len(),
                ),
                Err(_) => ("unknown", 0u64),
            };
            entries.push(json!({ "name": name, "kind": kind, "size": size }));
        }
    }
    entries.sort_by(|a, b| {
        let an = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let bn = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
        an.cmp(bn)
    });
    let count = entries.len() as u64;
    let entries_val = Value::Array(entries);
    json!({ "op": "list", "path": p, "ok": true, "count": count, "entries": entries_val })
}

fn op_write(p: &str, content: &str) -> Value {
    if let Err(msg) = check_write_len(content.len()) {
        return err_json("write", &msg);
    }
    match std::fs::write(p, content) {
        Ok(()) => json!({ "op": "write", "path": p, "ok": true, "bytes_written": content.len() }),
        Err(e) => err_json("write", &format!("write: {e}")),
    }
}

fn dispatch(args: &[String]) -> Value {
    let op = args.get(1).map(String::as_str);
    match op {
        None => usage_json(),
        Some("write") => match (args.get(2), args.get(3)) {
            (Some(p), Some(c)) => op_write(p, c),
            (Some(_), None) => err_json("write", "missing content argument"),
            (None, _) => err_json("write", "missing path argument"),
        },
        Some(o) => match args.get(2) {
            None => err_json(o, "missing path argument"),
            Some(p) => match o {
                "stat" => op_stat(p),
                "read" => op_read(p),
                "list" => op_list(p),
                _ => usage_json(),
            },
        },
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    println!("{}", dispatch(&args));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn read_up_to_under_limit() {
        let (b, t) = read_up_to(Cursor::new(b"abc"), 10).unwrap();
        assert_eq!(b, b"abc");
        assert!(!t);
    }

    #[test]
    fn read_up_to_truncated() {
        let (b, t) = read_up_to(Cursor::new(b"abcdef"), 3).unwrap();
        assert_eq!(b, b"abc");
        assert!(t);
    }

    #[test]
    fn read_up_to_exact_limit_not_truncated() {
        let (b, t) = read_up_to(Cursor::new(b"abc"), 3).unwrap();
        assert_eq!(b, b"abc");
        assert!(!t);
    }

    #[test]
    fn dispatch_unknown_op() {
        let v = dispatch(&["wasm-fs".into(), "bogus".into(), "/x".into()]);
        assert_eq!(v["ok"], false);
    }

    #[test]
    fn dispatch_missing_path() {
        let v = dispatch(&["wasm-fs".into(), "stat".into()]);
        assert_eq!(v["ok"], false);
    }

    #[test]
    fn stat_nonexistent_is_ok_false() {
        let v = op_stat("/definitely/does/not/exist/aimt-wasm-fs-xyz");
        assert_eq!(v["ok"], false);
        assert_eq!(v["op"], "stat");
    }

    #[test]
    fn check_write_len_bounds() {
        assert!(check_write_len(0).is_ok());
        assert!(check_write_len(WRITE_LIMIT).is_ok());
        assert!(check_write_len(WRITE_LIMIT + 1).is_err());
    }

    #[test]
    fn dispatch_write_missing_content() {
        let v = dispatch(&["wasm-fs".into(), "write".into(), "/p".into()]);
        assert_eq!(v["ok"], false);
        assert_eq!(v["op"], "write");
    }
}
