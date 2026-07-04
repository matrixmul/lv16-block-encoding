use std::fs;
use std::path::PathBuf;

use block_encoding_matrix_lv16::{build_target_metadata, matmul, repo_root, sha256_text};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let root = repo_root();
    let mut target_path = root.join("challenges/target_16q.json");
    let mut output = root.join("dist/solution.qasm");
    let mut check = false;
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--target" => {
                i += 1;
                target_path = PathBuf::from(
                    args.get(i)
                        .ok_or_else(|| "--target requires a path".to_string())?,
                );
            }
            "--output" => {
                i += 1;
                output = PathBuf::from(
                    args.get(i)
                        .ok_or_else(|| "--output requires a path".to_string())?,
                );
            }
            "--check" => check = true,
            "--help" | "-h" => {
                println!("Usage: generate-solution [--target PATH] [--output PATH] [--check]");
                return Ok(());
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }

    let target = if target_path.exists() {
        let bytes = fs::read(&target_path)
            .map_err(|error| format!("failed to read {}: {error}", target_path.display()))?;
        serde_json::from_slice(&bytes)
            .map_err(|error| format!("failed to parse {}: {error}", target_path.display()))?
    } else {
        build_target_metadata()
    };
    let qasm = matmul::render_qasm(&target);
    let digest = sha256_text(&qasm);

    if check {
        let actual = fs::read(&output)
            .map_err(|_| format!("missing generated solution QASM: {}", output.display()))?;
        if actual != qasm.as_bytes() {
            return Err(format!(
                "generated solution QASM is stale: {}",
                output.display()
            ));
        }
        println!("generated solution QASM is current: {}", output.display());
        return Ok(());
    }

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(&output, qasm.as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    println!("wrote {}", output.display());
    println!("sha256={digest}");
    Ok(())
}
