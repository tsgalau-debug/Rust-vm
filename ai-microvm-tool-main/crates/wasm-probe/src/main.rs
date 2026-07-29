use std::env;
use std::fs;

fn main() {
    // --- ENV: bukti nyata hardening (hanya env dari profile yang muncul) ---
    let mut vars: Vec<(String, String)> = env::vars().collect();
    vars.sort_by(|a, b| a.0.cmp(&b.0));

    println!("[env]");
    for (k, v) in &vars {
        println!("{k}={v}");
    }
    println!("[env-count] {}", vars.len());

    // --- ARGS (argv0 dilewat) ---
    let args: Vec<String> = env::args().skip(1).collect();
    println!("[args] {}", args.len());
    for a in &args {
        println!("  - {a}");
    }

    // --- FS: tiap arg berawalan '/' dibaca via preopen ---
    let paths: Vec<&String> = args.iter().filter(|a| a.starts_with('/')).collect();
    if !paths.is_empty() {
        println!("[fs]");
        for p in paths {
            match fs::read_to_string(p) {
                Ok(s) => {
                    let preview: String = s.chars().take(200).collect();
                    println!("  {p} OK {} bytes", s.len());
                    if !preview.is_empty() {
                        println!("    | {preview}");
                    }
                }
                Err(e) => println!("  {p} ERR {e}"),
            }
        }
    }
}
