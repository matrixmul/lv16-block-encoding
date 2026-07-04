use std::fs;
use std::path::PathBuf;

use block_encoding_matrix_lv16::{
    build_target_metadata, render_baseline_qasm, render_reference_qasm, repo_root, sha256_text,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let root = repo_root();
    let mut target_path = root.join("challenges/target_16q.json");
    let mut output = root.join("dist/baseline.qasm");
    let mut qubits = None;
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
            "--qubits" => {
                i += 1;
                qubits = Some(
                    args.get(i)
                        .ok_or_else(|| "--qubits requires a value".to_string())?
                        .parse::<usize>()
                        .map_err(|_| "--qubits must be an integer".to_string())?,
                );
            }
            "--check" => check = true,
            "--help" | "-h" => {
                println!(
                    "Usage: generate-baseline [--target PATH] [--output PATH] [--qubits N] [--check]"
                );
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
    let target_qubits = target["qubits"]
        .as_u64()
        .ok_or_else(|| "target qubits missing".to_string())? as usize;
    let qasm = if let Some(qubits) = qubits {
        render_reference_qasm(&target, qubits)
    } else {
        render_baseline_qasm(&target)
    };
    let digest = sha256_text(&qasm);
    if qubits.unwrap_or(target_qubits) == target_qubits {
        if let Some(expected) = target["reference"]["sha256"].as_str() {
            if digest != expected {
                return Err(format!(
                    "generated baseline hash {digest} does not match target reference {expected}"
                ));
            }
        }
    }

    if check {
        let actual = fs::read(&output)
            .map_err(|_| format!("missing baseline QASM: {}", output.display()))?;
        if actual != qasm.as_bytes() {
            return Err(format!("baseline QASM is stale: {}", output.display()));
        }
        println!("baseline QASM is current: {}", output.display());
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
