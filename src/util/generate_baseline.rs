use std::fs;
use std::path::PathBuf;

use block_encoding_matrix_lv16::{
    build_target_metadata, render_baseline_qasm, repo_root, sha256_text,
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
                return Err(
                    "generate-baseline is retired; the fixed 42-qubit baseline already exists"
                        .to_string(),
                );
            }
            "--qubits" | "--all-widths" => {
                return Err(
                    "lower-width baseline generation has been removed; submit an actual declared-width implementation instead"
                        .to_string(),
                );
            }
            "--check" => check = true,
            "--help" | "-h" => {
                println!(
                    "Usage: generate-baseline [--target PATH] [--check]\n\
                     \n\
                     Retired utility: checks the fixed 42-qubit baseline reference hash only. \
                     It no longer writes QASM or generates lower-width baselines."
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
    let qasm = render_baseline_qasm(&target);
    let digest = sha256_text(&qasm);
    if let Some(expected) = target["reference"]["sha256"].as_str() {
        if digest != expected {
            return Err(format!(
                "fixed {target_qubits}-qubit baseline hash {digest} does not match target reference {expected}"
            ));
        }
    }

    if check {
        println!("fixed {target_qubits}-qubit baseline reference is current: sha256={digest}");
        return Ok(());
    }

    println!("generate-baseline is retired; fixed {target_qubits}-qubit baseline sha256={digest}");
    Ok(())
}
