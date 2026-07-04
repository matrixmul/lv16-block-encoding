use std::fs;
use std::path::PathBuf;

use block_encoding_matrix_lv16::{build_target_metadata, canonical_json, repo_root, write_json};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut output = repo_root().join("challenges/target_16q.json");
    let mut check = false;
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--output" => {
                i += 1;
                output = PathBuf::from(
                    args.get(i)
                        .ok_or_else(|| "--output requires a path".to_string())?,
                );
            }
            "--check" => check = true,
            "--help" | "-h" => {
                println!("Usage: generate-target [--output PATH] [--check]");
                return Ok(());
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }

    let target = build_target_metadata();
    if check {
        let actual_bytes = fs::read(&output)
            .map_err(|_| format!("missing target metadata: {}", output.display()))?;
        let actual: serde_json::Value = serde_json::from_slice(&actual_bytes)
            .map_err(|error| format!("failed to parse {}: {error}", output.display()))?;
        if canonical_json(&actual) != canonical_json(&target) {
            return Err(format!("target metadata is stale: {}", output.display()));
        }
        println!("target metadata is current: {}", output.display());
        return Ok(());
    }

    write_json(&output, &target)
        .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    println!("wrote {}", output.display());
    println!("target_id={}", target["target_id"].as_str().unwrap());
    println!(
        "metadata_sha256={}",
        target["metadata_sha256"].as_str().unwrap()
    );
    println!(
        "reference_sha256={}",
        target["reference"]["sha256"].as_str().unwrap()
    );
    Ok(())
}
