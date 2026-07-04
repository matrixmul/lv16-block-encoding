use std::f64::consts::{PI, TAU};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use block_encoding_matrix_lv16::render_baseline_qasm;
use nalgebra::{DMatrix, linalg::SVD};
use num_complex::Complex64;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

type C64 = Complex64;

#[derive(Debug)]
struct VerifyError(String);

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for VerifyError {}

type Result<T> = std::result::Result<T, VerifyError>;

#[derive(Clone, Debug)]
struct Instruction {
    name: String,
    qubits: Vec<usize>,
    params: Vec<f64>,
    line: usize,
}

#[derive(Clone, Debug)]
struct Circuit {
    qubits: usize,
    instructions: Vec<Instruction>,
    metrics: Metrics,
    sha256: String,
    byte_len: usize,
}

#[derive(Clone, Debug, Default)]
struct Metrics {
    gates: usize,
    single_qubit_gates: usize,
    two_qubit_gates: usize,
    h_gates: usize,
    x_gates: usize,
    y_gates: usize,
    z_gates: usize,
    rz_gates: usize,
    rotation_gates: usize,
    cx_gates: usize,
    cnot_gates: usize,
    swap_gates: usize,
    remote_two_qubit_gates: usize,
    two_qubit_distance_sum: usize,
    routing_swap_equivalents: usize,
    entangling_gate_equivalents: usize,
    rotation_synthesis_equivalents: usize,
    weighted_gate_volume: usize,
    weighted_depth: usize,
    max_distance: usize,
    depth: usize,
}

#[derive(Debug, Deserialize)]
struct Target {
    target_id: String,
    qubits: usize,
    limits: Limits,
    scoring: ScoringConfig,
    verifier: VerifierConfig,
    reference: Reference,
    smoke_probes: Vec<Shot>,
    validation: Validation,
}

#[derive(Debug, Deserialize)]
struct Limits {
    max_gates: Option<usize>,
    max_depth: Option<usize>,
    max_two_qubit_gates: Option<usize>,
    max_rotation_gates: Option<usize>,
    max_remote_two_qubit_gates: Option<usize>,
    max_routing_swap_equivalents: Option<usize>,
    max_two_qubit_distance: Option<usize>,
    max_weighted_gate_volume: Option<usize>,
    max_weighted_depth: Option<usize>,
    max_qasm_bytes: Option<usize>,
    min_qubits: Option<usize>,
    max_qubits: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
struct ScoringConfig {
    model: String,
    single_qubit_weight: usize,
    single_qubit_depth_weight: usize,
    rotation_synthesis_weight: usize,
    rotation_depth_weight: usize,
    entangling_gate_weight: usize,
    entangling_depth_weight: usize,
    swap_entangling_equivalent: usize,
    routing_swap_per_extra_distance: usize,
}

#[derive(Debug, Deserialize)]
struct VerifierConfig {
    default_max_bond: usize,
    svd_cutoff: f64,
    fidelity_atol: f64,
    norm_atol: f64,
    #[serde(default)]
    truncation_error_atol: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct Reference {
    sha256: String,
}

struct ReferenceSelection {
    label: String,
    circuit: Option<Circuit>,
    expected_sha256: Option<String>,
    errors: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct Shot {
    #[serde(default)]
    index: Option<usize>,
    name: String,
    states: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Validation {
    trusted_shots: TrustedShots,
}

#[derive(Debug, Deserialize)]
struct TrustedShots {
    count: usize,
    domain_separator: String,
    state_alphabet: Vec<String>,
}

#[derive(Clone, Debug)]
struct Args {
    candidate: PathBuf,
    target: PathBuf,
    reference: Option<PathBuf>,
    max_bond: Option<usize>,
    svd_cutoff: Option<f64>,
    fidelity_atol: Option<f64>,
    norm_atol: Option<f64>,
    truncation_error_atol: Option<f64>,
    preflight: bool,
    smoke: bool,
    shot_count: Option<usize>,
    shot_shard: Option<String>,
    json: bool,
    include_shot_details: bool,
}

#[derive(Clone, Debug)]
struct Tensor {
    left: usize,
    right: usize,
    data: Vec<C64>,
}

impl Tensor {
    fn new(left: usize, right: usize, data: Vec<C64>) -> Self {
        assert_eq!(left * 2 * right, data.len());
        Self { left, right, data }
    }

    fn get(&self, l: usize, p: usize, r: usize) -> C64 {
        self.data[(l * 2 + p) * self.right + r]
    }

    fn set(&mut self, l: usize, p: usize, r: usize, value: C64) {
        self.data[(l * 2 + p) * self.right + r] = value;
    }
}

#[derive(Clone, Debug)]
struct Mps {
    tensors: Vec<Tensor>,
    max_bond: usize,
    cutoff: f64,
    truncation_error: f64,
}

#[derive(Clone, Debug)]
struct ShotReport {
    name: String,
    index: Option<usize>,
    fidelity: f64,
    infidelity: f64,
    reference_norm: f64,
    candidate_norm: f64,
    norm_delta: f64,
    reference_truncation_error: f64,
    candidate_truncation_error: f64,
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    match verify(&args) {
        Ok(report) => {
            if args.json {
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            } else {
                print_text_report(&report);
            }
            let ok = report["ok"].as_bool().unwrap_or(false);
            std::process::exit(if ok { 0 } else { 1 });
        }
        Err(error) => {
            let report = json!({"ok": false, "errors": [error.to_string()]});
            if args.json {
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            } else {
                println!("ok: false");
                println!("errors:");
                println!("  - {error}");
            }
            std::process::exit(1);
        }
    }
}

fn parse_args() -> Result<Args> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_help();
        std::process::exit(0);
    }

    let mut candidate = None;
    let mut target = PathBuf::from("challenges/target_16q.json");
    let mut reference = None;
    let mut max_bond = None;
    let mut svd_cutoff = None;
    let mut fidelity_atol = None;
    let mut norm_atol = None;
    let mut truncation_error_atol = None;
    let mut preflight = false;
    let mut smoke = false;
    let mut shot_count = None;
    let mut shot_shard = None;
    let mut json = false;
    let mut include_shot_details = false;

    let mut i = 0;
    while i < raw.len() {
        let arg = raw[i].clone();
        match arg.as_str() {
            "--target" => {
                i += 1;
                target = next_path(&raw, i, "--target")?;
            }
            "--reference" => {
                i += 1;
                reference = Some(next_path(&raw, i, "--reference")?);
            }
            "--max-bond" => {
                i += 1;
                max_bond = Some(next_parse(&raw, i, "--max-bond")?);
            }
            "--svd-cutoff" => {
                i += 1;
                svd_cutoff = Some(next_parse(&raw, i, "--svd-cutoff")?);
            }
            "--fidelity-atol" => {
                i += 1;
                fidelity_atol = Some(next_parse(&raw, i, "--fidelity-atol")?);
            }
            "--norm-atol" => {
                i += 1;
                norm_atol = Some(next_parse(&raw, i, "--norm-atol")?);
            }
            "--truncation-error-atol" => {
                i += 1;
                truncation_error_atol = Some(next_parse(&raw, i, "--truncation-error-atol")?);
            }
            "--preflight" => preflight = true,
            "--smoke" => smoke = true,
            "--shot-count" => {
                i += 1;
                shot_count = Some(next_parse(&raw, i, "--shot-count")?);
            }
            "--shot-shard" | "--probe-shard" => {
                i += 1;
                shot_shard = Some(next_string(&raw, i, "--shot-shard")?);
            }
            "--json" => json = true,
            "--include-shot-details" => include_shot_details = true,
            option if option.starts_with("--") => {
                return Err(VerifyError(format!("unknown option: {option}")));
            }
            value => {
                if candidate.is_some() {
                    return Err(VerifyError(format!(
                        "unexpected positional argument: {value}"
                    )));
                }
                candidate = Some(PathBuf::from(value));
            }
        }
        i += 1;
    }

    Ok(Args {
        candidate: candidate
            .ok_or_else(|| VerifyError("missing candidate QASM path".to_string()))?,
        target,
        reference,
        max_bond,
        svd_cutoff,
        fidelity_atol,
        norm_atol,
        truncation_error_atol,
        preflight,
        smoke,
        shot_count,
        shot_shard,
        json,
        include_shot_details,
    })
}

fn print_help() {
    println!(
        "Usage: verify <candidate.qasm> [--target PATH] [--reference PATH] [--preflight|--smoke]\n\
         \n\
         By default, the reference circuit is selected from the official same-width references in the target metadata.\n\
         Trusted mode defaults to all 9024 deterministic shots. Use --shot-count N\n\
         or --shot-shard INDEX/TOTAL for bounded local checks and CI sharding.\n\
         Use --truncation-error-atol X to override the MPS truncation failure threshold."
    );
}

fn next_string(raw: &[String], i: usize, option: &str) -> Result<String> {
    raw.get(i)
        .cloned()
        .ok_or_else(|| VerifyError(format!("{option} requires a value")))
}

fn next_path(raw: &[String], i: usize, option: &str) -> Result<PathBuf> {
    Ok(PathBuf::from(next_string(raw, i, option)?))
}

fn next_parse<T: std::str::FromStr>(raw: &[String], i: usize, option: &str) -> Result<T> {
    next_string(raw, i, option)?
        .parse()
        .map_err(|_| VerifyError(format!("invalid value for {option}")))
}

fn width_reference_entry(
    target_value: &serde_json::Value,
    width: usize,
) -> Option<&serde_json::Value> {
    target_value
        .get("references")
        .and_then(|references| references.get("by_width"))
        .and_then(|by_width| by_width.get(width.to_string()))
}

fn select_official_reference(
    target_path: &Path,
    target_value: &serde_json::Value,
    target: &Target,
    declared_width: usize,
) -> Result<ReferenceSelection> {
    let Some(entry) = width_reference_entry(target_value, declared_width) else {
        if declared_width == target.qubits {
            let qasm = render_baseline_qasm(target_value);
            return Ok(ReferenceSelection {
                label: format!(
                    "generated:src/util/generate_baseline.rs#qubits={}",
                    target.qubits
                ),
                circuit: Some(parse_qasm_text(
                    "generated full-width reference",
                    &qasm,
                    &target.scoring,
                )?),
                expected_sha256: Some(target.reference.sha256.clone()),
                errors: Vec::new(),
            });
        }
        return Ok(ReferenceSelection {
            label: format!("unregistered:official-reference#qubits={declared_width}"),
            circuit: None,
            expected_sha256: None,
            errors: vec![format!(
                "no official same-width reference registered for declared width {declared_width}; refusing to validate by truncating or projecting the {}-qubit target",
                target.qubits
            )],
        });
    };

    let expected_sha256 = entry
        .get("sha256")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let Some(path) = entry.get("path").and_then(serde_json::Value::as_str) else {
        return Ok(ReferenceSelection {
            label: format!("invalid:official-reference#qubits={declared_width}"),
            circuit: None,
            expected_sha256,
            errors: vec![format!(
                "official reference for declared width {declared_width} is missing a QASM path"
            )],
        });
    };
    if path.starts_with("generated:") {
        if declared_width == target.qubits {
            let qasm = render_baseline_qasm(target_value);
            return Ok(ReferenceSelection {
                label: path.to_string(),
                circuit: Some(parse_qasm_text(
                    "generated full-width reference",
                    &qasm,
                    &target.scoring,
                )?),
                expected_sha256,
                errors: Vec::new(),
            });
        }
        return Ok(ReferenceSelection {
            label: path.to_string(),
            circuit: None,
            expected_sha256,
            errors: vec![format!(
                "official reference for declared width {declared_width} must be an actual same-width QASM artifact, not a generated projection"
            )],
        });
    }

    let reference_path = PathBuf::from(path);
    let reference_path = if reference_path.is_absolute() {
        reference_path
    } else {
        target_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(reference_path)
    };
    Ok(ReferenceSelection {
        label: reference_path.display().to_string(),
        circuit: Some(parse_qasm(&reference_path, &target.scoring)?),
        expected_sha256,
        errors: Vec::new(),
    })
}

fn verify(args: &Args) -> Result<serde_json::Value> {
    let target_bytes = fs::read(&args.target).map_err(io_error)?;
    let target_value: serde_json::Value = serde_json::from_slice(&target_bytes)
        .map_err(|err| VerifyError(format!("failed to parse target JSON: {err}")))?;
    let target: Target = serde_json::from_value(target_value.clone())
        .map_err(|err| VerifyError(format!("failed to parse target JSON: {err}")))?;
    let candidate = parse_qasm(&args.candidate, &target.scoring)?;
    let mut errors = check_prescreen(&candidate, &target);
    let reference_selection = if let Some(reference_path) = &args.reference {
        ReferenceSelection {
            label: reference_path.display().to_string(),
            circuit: Some(parse_qasm(reference_path, &target.scoring)?),
            expected_sha256: None,
            errors: Vec::new(),
        }
    } else {
        select_official_reference(&args.target, &target_value, &target, candidate.qubits)?
    };
    let reference_label = reference_selection.label;
    let reference = reference_selection.circuit;
    errors.extend(reference_selection.errors);

    if let Some(reference) = &reference {
        if reference.qubits != candidate.qubits {
            errors.push(format!(
                "reference width mismatch: candidate declares {}, reference declares {}",
                candidate.qubits, reference.qubits
            ));
        }
        if let Some(expected) = reference_selection.expected_sha256.as_deref() {
            if reference.sha256 != expected {
                errors.push(format!(
                    "reference hash mismatch: expected {}, got {}",
                    expected, reference.sha256
                ));
            }
        }
    }

    let max_bond = args.max_bond.unwrap_or(target.verifier.default_max_bond);
    let cutoff = args.svd_cutoff.unwrap_or(target.verifier.svd_cutoff);
    let fidelity_atol = args.fidelity_atol.unwrap_or(target.verifier.fidelity_atol);
    let norm_atol = args.norm_atol.unwrap_or(target.verifier.norm_atol);
    let truncation_error_atol = args
        .truncation_error_atol
        .or(target.verifier.truncation_error_atol)
        .unwrap_or(norm_atol);
    let (validation_mode, shots) = if errors.is_empty() {
        select_validation_shots(&target, args, candidate.qubits)?
    } else {
        (validation_mode_name(args).to_string(), Vec::new())
    };

    let mut shot_reports = Vec::new();
    if errors.is_empty() {
        let reference = reference
            .as_ref()
            .expect("reference circuit must exist when there are no validation errors");
        for shot in &shots {
            if shot.states.len() != candidate.qubits {
                return Err(VerifyError(format!(
                    "shot {} has {} states, expected {}",
                    shot.name,
                    shot.states.len(),
                    candidate.qubits
                )));
            }
            let reference_state = simulate(reference, &shot.states, max_bond, cutoff)?;
            let candidate_state = simulate(&candidate, &shot.states, max_bond, cutoff)?;
            let overlap = inner_product(&reference_state, &candidate_state);
            let reference_norm = inner_product(&reference_state, &reference_state).re;
            let candidate_norm = inner_product(&candidate_state, &candidate_state).re;
            let denominator = (reference_norm * candidate_norm).max(f64::MIN_POSITIVE);
            let fidelity = overlap.norm_sqr() / denominator;
            let infidelity = (1.0 - fidelity.min(1.0)).max(0.0);
            let norm_delta = (reference_norm - candidate_norm).abs();
            if infidelity > fidelity_atol || norm_delta > norm_atol {
                errors.push(format!(
                    "shot {} failed: infidelity={infidelity:.3e}, norm_delta={norm_delta:.3e}",
                    shot.name
                ));
            }
            if reference_state.truncation_error > truncation_error_atol
                || candidate_state.truncation_error > truncation_error_atol
            {
                errors.push(format!(
                    "shot {} exceeded MPS truncation tolerance: reference={:.3e}, candidate={:.3e}, atol={truncation_error_atol:.3e}",
                    shot.name,
                    reference_state.truncation_error,
                    candidate_state.truncation_error
                ));
            }
            shot_reports.push(ShotReport {
                name: shot.name.clone(),
                index: shot.index,
                fidelity,
                infidelity,
                reference_norm,
                candidate_norm,
                norm_delta,
                reference_truncation_error: reference_state.truncation_error,
                candidate_truncation_error: candidate_state.truncation_error,
            });
        }
    }

    let shot_summary = summarize_shots(&shot_reports);
    let score_breakdown = score_breakdown(&candidate.metrics, &target.scoring, candidate.qubits);
    let mut report = json!({
        "ok": errors.is_empty(),
        "target_id": target.target_id,
        "candidate": args.candidate.display().to_string(),
        "candidate_sha256": candidate.sha256,
        "reference": reference_label,
        "reference_sha256": reference.as_ref().map(|reference| reference.sha256.clone()),
        "score": score_breakdown["score"],
        "score_breakdown": score_breakdown,
        "metrics": metrics_json(&candidate.metrics),
        "cost_guard": cost_guard_json(&candidate, &target.limits),
        "max_bond": max_bond,
        "svd_cutoff": cutoff,
        "fidelity_atol": fidelity_atol,
        "norm_atol": norm_atol,
        "truncation_error_atol": truncation_error_atol,
        "validation": {
            "mode": validation_mode,
            "trusted_shots": target.validation.trusted_shots.count,
            "evaluated_shots": shot_reports.len(),
            "declared_qubits": candidate.qubits,
            "shot_count_override": args.shot_count,
            "shot_shard": args.shot_shard,
        },
        "shot_summary": shot_summary,
        "shots": [],
        "errors": errors,
    });

    if args.include_shot_details {
        report["shots"] = json!(shot_reports_json(&shot_reports));
    }

    Ok(report)
}

fn io_error(err: std::io::Error) -> VerifyError {
    VerifyError(err.to_string())
}

fn parse_qasm(path: &Path, scoring: &ScoringConfig) -> Result<Circuit> {
    let payload = fs::read(path).map_err(io_error)?;
    if payload.is_empty() {
        return Err(VerifyError(format!("empty QASM file: {}", path.display())));
    }
    parse_qasm_payload(&path.display().to_string(), payload, scoring)
}

fn parse_qasm_text(label: &str, text: &str, scoring: &ScoringConfig) -> Result<Circuit> {
    parse_qasm_payload(label, text.as_bytes().to_vec(), scoring)
}

fn parse_qasm_payload(label: &str, payload: Vec<u8>, scoring: &ScoringConfig) -> Result<Circuit> {
    if payload.is_empty() {
        return Err(VerifyError(format!("empty QASM file: {label}")));
    }
    let byte_len = payload.len();
    let sha256 = sha256_hex(&payload);
    let text = String::from_utf8(payload)
        .map_err(|err| VerifyError(format!("QASM is not UTF-8: {err}")))?;
    let mut qubits = None;
    let mut instructions = Vec::new();

    for (line, statement) in iter_statements(&text)? {
        if statement == "OPENQASM 3.0"
            || statement.starts_with("include ")
            || statement.starts_with("barrier ")
        {
            continue;
        }
        if let Some(count) = parse_declaration(&statement)? {
            qubits = Some(count);
            continue;
        }
        if is_disallowed_statement(&statement) {
            return Err(VerifyError(format!(
                "unsupported dynamic/classical statement on line {line}: {statement}"
            )));
        }
        instructions.push(parse_gate_statement(line, &statement)?);
    }

    let qubits =
        qubits.ok_or_else(|| VerifyError("missing qubit declaration: qubit[N] q;".to_string()))?;
    let metrics = compute_metrics(qubits, &instructions, scoring)?;
    Ok(Circuit {
        qubits,
        instructions,
        metrics,
        sha256,
        byte_len,
    })
}

fn iter_statements(text: &str) -> Result<Vec<(usize, String)>> {
    let mut statements = Vec::new();
    let mut buffer = String::new();
    let mut start_line = 1;
    for (offset, raw_line) in text.lines().enumerate() {
        let line_no = offset + 1;
        let line = raw_line
            .split_once("//")
            .map_or(raw_line, |(left, _)| left)
            .trim();
        if line.is_empty() {
            continue;
        }
        if buffer.is_empty() {
            start_line = line_no;
        } else {
            buffer.push(' ');
        }
        buffer.push_str(line);
        while let Some(pos) = buffer.find(';') {
            let statement = buffer[..pos].trim().to_string();
            buffer = buffer[pos + 1..].trim().to_string();
            if !statement.is_empty() {
                statements.push((start_line, statement));
            }
            start_line = line_no;
        }
    }
    if !buffer.trim().is_empty() {
        return Err(VerifyError(format!(
            "unterminated statement starting at line {start_line}"
        )));
    }
    Ok(statements)
}

fn parse_declaration(statement: &str) -> Result<Option<usize>> {
    let Some(rest) = statement.strip_prefix("qubit") else {
        return Ok(None);
    };
    let rest = rest.trim();
    let Some(after_open) = rest.strip_prefix('[') else {
        return Ok(None);
    };
    let Some((count_text, after_close)) = after_open.split_once(']') else {
        return Err(VerifyError(format!(
            "invalid qubit declaration: {statement}"
        )));
    };
    if after_close.trim() != "q" {
        return Err(VerifyError(format!(
            "expected qubit register named q: {statement}"
        )));
    }
    let count = count_text
        .trim()
        .parse::<usize>()
        .map_err(|_| VerifyError(format!("invalid qubit count: {count_text}")))?;
    Ok(Some(count))
}

fn is_disallowed_statement(statement: &str) -> bool {
    ["bit", "creg", "measure", "reset", "if ", "for ", "while "]
        .iter()
        .any(|prefix| statement.starts_with(prefix))
}

fn parse_gate_statement(line: usize, statement: &str) -> Result<Instruction> {
    let (head, operands) = split_gate_head(statement)
        .ok_or_else(|| VerifyError(format!("could not parse line {line}: {statement}")))?;
    let (name, params) = parse_gate_head(head)?;
    let qubits = operands
        .split(',')
        .map(parse_qubit)
        .collect::<Result<Vec<_>>>()?;
    Ok(Instruction {
        name: name.to_ascii_lowercase(),
        qubits,
        params,
        line,
    })
}

fn split_gate_head(statement: &str) -> Option<(&str, &str)> {
    let mut depth = 0_i32;
    for (idx, ch) in statement.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ if ch.is_whitespace() && depth == 0 => {
                let head = statement[..idx].trim();
                let operands = statement[idx..].trim();
                if head.is_empty() || operands.is_empty() {
                    return None;
                }
                return Some((head, operands));
            }
            _ => {}
        }
    }
    None
}

fn parse_gate_head(head: &str) -> Result<(String, Vec<f64>)> {
    if let Some(open) = head.find('(') {
        let close = head
            .rfind(')')
            .ok_or_else(|| VerifyError(format!("unterminated gate parameter list: {head}")))?;
        if close != head.len() - 1 {
            return Err(VerifyError(format!(
                "trailing text after gate parameters: {head}"
            )));
        }
        let name = head[..open].to_string();
        let params = if head[open + 1..close].trim().is_empty() {
            Vec::new()
        } else {
            head[open + 1..close]
                .split(',')
                .map(parse_angle)
                .collect::<Result<Vec<_>>>()?
        };
        Ok((name, params))
    } else {
        Ok((head.to_string(), Vec::new()))
    }
}

fn parse_qubit(token: &str) -> Result<usize> {
    let token = token.trim();
    let Some(rest) = token.strip_prefix("q[") else {
        return Err(VerifyError(format!(
            "expected qubit operand q[i], got {token:?}"
        )));
    };
    let Some(index_text) = rest.strip_suffix(']') else {
        return Err(VerifyError(format!(
            "expected qubit operand q[i], got {token:?}"
        )));
    };
    index_text
        .parse::<usize>()
        .map_err(|_| VerifyError(format!("invalid qubit index: {token}")))
}

fn compute_metrics(
    qubits: usize,
    instructions: &[Instruction],
    scoring: &ScoringConfig,
) -> Result<Metrics> {
    let mut depths = vec![0_usize; qubits];
    let mut weighted_times = vec![0_usize; qubits];
    let mut metrics = Metrics {
        gates: instructions.len(),
        ..Metrics::default()
    };

    for instruction in instructions {
        for &q in &instruction.qubits {
            if q >= qubits {
                return Err(VerifyError(format!(
                    "q[{q}] on line {} exceeds qubit count {qubits}",
                    instruction.line
                )));
            }
        }
        match instruction.qubits.len() {
            1 => {
                validate_single_qubit_gate(instruction)?;
                metrics.single_qubit_gates += 1;
                match instruction.name.as_str() {
                    "h" => metrics.h_gates += 1,
                    "x" => metrics.x_gates += 1,
                    "y" => metrics.y_gates += 1,
                    "z" => metrics.z_gates += 1,
                    "rz" => metrics.rz_gates += 1,
                    _ => unreachable!("validated single-qubit gate"),
                }
                let (gate_weight, depth_weight) = if is_rotation_gate(&instruction.name) {
                    (
                        scoring.rotation_synthesis_weight,
                        scoring.rotation_depth_weight,
                    )
                } else {
                    (
                        scoring.single_qubit_weight,
                        scoring.single_qubit_depth_weight,
                    )
                };
                if is_rotation_gate(&instruction.name) {
                    metrics.rotation_synthesis_equivalents += gate_weight;
                }
                metrics.weighted_gate_volume += gate_weight;
                let q = instruction.qubits[0];
                let weighted_time = weighted_times[q] + depth_weight;
                weighted_times[q] = weighted_time;
                metrics.weighted_depth = metrics.weighted_depth.max(weighted_time);
            }
            2 => {
                validate_two_qubit_gate(instruction)?;
                metrics.two_qubit_gates += 1;
                let distance = instruction.qubits[0].abs_diff(instruction.qubits[1]);
                metrics.max_distance = metrics.max_distance.max(distance);
                metrics.two_qubit_distance_sum += distance;
                if distance > 1 {
                    metrics.remote_two_qubit_gates += 1;
                }
                let routed_swaps =
                    scoring.routing_swap_per_extra_distance * distance.saturating_sub(1);
                metrics.routing_swap_equivalents += routed_swaps;
                let base_entangling_equivalents = scoring.entangling_gate_weight;
                let entangling_equivalents =
                    base_entangling_equivalents + routed_swaps * scoring.swap_entangling_equivalent;
                let weighted_depth_cost = entangling_equivalents * scoring.entangling_depth_weight;
                metrics.entangling_gate_equivalents += entangling_equivalents;
                metrics.weighted_gate_volume += entangling_equivalents;
                let left = instruction.qubits[0].min(instruction.qubits[1]);
                let right = instruction.qubits[0].max(instruction.qubits[1]);
                let current_weighted_time = weighted_times[left..=right]
                    .iter()
                    .copied()
                    .max()
                    .unwrap_or(0)
                    + weighted_depth_cost;
                for time in &mut weighted_times[left..=right] {
                    *time = current_weighted_time;
                }
                metrics.weighted_depth = metrics.weighted_depth.max(current_weighted_time);
            }
            arity => {
                return Err(VerifyError(format!(
                    "gate {:?} on line {} has unsupported arity {arity}",
                    instruction.name, instruction.line
                )));
            }
        }

        if is_rotation_gate(&instruction.name) {
            metrics.rotation_gates += 1;
        }
        if matches!(instruction.name.as_str(), "cx" | "cnot") {
            if instruction.name == "cnot" {
                metrics.cnot_gates += 1;
            } else {
                metrics.cx_gates += 1;
            }
        }
        let current_depth = instruction
            .qubits
            .iter()
            .map(|&q| depths[q])
            .max()
            .unwrap_or(0)
            + 1;
        for &q in &instruction.qubits {
            depths[q] = current_depth;
        }
        metrics.depth = metrics.depth.max(current_depth);
    }
    Ok(metrics)
}

fn validate_single_qubit_gate(instruction: &Instruction) -> Result<()> {
    if !matches!(instruction.name.as_str(), "h" | "x" | "y" | "z" | "rz") {
        return Err(VerifyError(format!(
            "unsupported single-qubit gate {:?} on line {}",
            instruction.name, instruction.line
        )));
    }
    if is_rotation_gate(&instruction.name) {
        if instruction.params.len() != 1 {
            return Err(VerifyError(format!(
                "gate {} on line {} expects one angle parameter",
                instruction.name, instruction.line
            )));
        }
    } else if !instruction.params.is_empty() {
        return Err(VerifyError(format!(
            "gate {} on line {} does not accept parameters",
            instruction.name, instruction.line
        )));
    }
    Ok(())
}

fn validate_two_qubit_gate(instruction: &Instruction) -> Result<()> {
    if !matches!(instruction.name.as_str(), "cx" | "cnot") {
        return Err(VerifyError(format!(
            "unsupported two-qubit gate {:?} on line {}",
            instruction.name, instruction.line
        )));
    }
    if !instruction.params.is_empty() {
        return Err(VerifyError(format!(
            "gate {} on line {} does not accept parameters",
            instruction.name, instruction.line
        )));
    }
    Ok(())
}

fn is_rotation_gate(name: &str) -> bool {
    name == "rz"
}

fn check_prescreen(circuit: &Circuit, target: &Target) -> Vec<String> {
    let mut errors = Vec::new();
    let min_qubits = target.limits.min_qubits.unwrap_or(1);
    let max_qubits = target.limits.max_qubits.unwrap_or(target.qubits);
    if circuit.qubits < min_qubits {
        errors.push(format!(
            "qubit count below minimum: {} < {}",
            circuit.qubits, min_qubits
        ));
    }
    if circuit.qubits > max_qubits {
        errors.push(format!(
            "qubit count exceeds limit: {} > {}",
            circuit.qubits, max_qubits
        ));
    }
    if let Some(limit) = target.limits.max_gates {
        if circuit.metrics.gates > limit {
            errors.push(format!(
                "gate count exceeds limit: {}",
                circuit.metrics.gates
            ));
        }
    }
    if let Some(limit) = target.limits.max_depth {
        if circuit.metrics.depth > limit {
            errors.push(format!("depth exceeds limit: {}", circuit.metrics.depth));
        }
    }
    if let Some(limit) = target.limits.max_two_qubit_gates {
        if circuit.metrics.two_qubit_gates > limit {
            errors.push(format!(
                "two-qubit gate count exceeds limit: {} > {}",
                circuit.metrics.two_qubit_gates, limit
            ));
        }
    }
    if let Some(limit) = target.limits.max_rotation_gates {
        if circuit.metrics.rotation_gates > limit {
            errors.push(format!(
                "rotation gate count exceeds limit: {} > {}",
                circuit.metrics.rotation_gates, limit
            ));
        }
    }
    if let Some(limit) = target.limits.max_remote_two_qubit_gates {
        if circuit.metrics.remote_two_qubit_gates > limit {
            errors.push(format!(
                "remote two-qubit gate count exceeds limit: {} > {}",
                circuit.metrics.remote_two_qubit_gates, limit
            ));
        }
    }
    if let Some(limit) = target.limits.max_routing_swap_equivalents {
        if circuit.metrics.routing_swap_equivalents > limit {
            errors.push(format!(
                "routing SWAP-equivalent count exceeds limit: {} > {}",
                circuit.metrics.routing_swap_equivalents, limit
            ));
        }
    }
    if let Some(limit) = target.limits.max_two_qubit_distance {
        if circuit.metrics.max_distance > limit {
            errors.push(format!(
                "two-qubit distance exceeds limit: {} > {}",
                circuit.metrics.max_distance, limit
            ));
        }
    }
    if let Some(limit) = target.limits.max_weighted_gate_volume {
        if circuit.metrics.weighted_gate_volume > limit {
            errors.push(format!(
                "weighted gate volume exceeds limit: {} > {}",
                circuit.metrics.weighted_gate_volume, limit
            ));
        }
    }
    if let Some(limit) = target.limits.max_weighted_depth {
        if circuit.metrics.weighted_depth > limit {
            errors.push(format!(
                "weighted depth exceeds limit: {} > {}",
                circuit.metrics.weighted_depth, limit
            ));
        }
    }
    if let Some(limit) = target.limits.max_qasm_bytes {
        if circuit.byte_len > limit {
            errors.push(format!(
                "QASM size exceeds limit: {} > {} bytes",
                circuit.byte_len, limit
            ));
        }
    }
    errors
}

fn cost_guard_json(circuit: &Circuit, limits: &Limits) -> serde_json::Value {
    json!({
        "observed": {
            "qasm_bytes": circuit.byte_len,
            "gates": circuit.metrics.gates,
            "depth": circuit.metrics.depth,
            "two_qubit_gates": circuit.metrics.two_qubit_gates,
            "rotation_gates": circuit.metrics.rotation_gates,
            "remote_two_qubit_gates": circuit.metrics.remote_two_qubit_gates,
            "routing_swap_equivalents": circuit.metrics.routing_swap_equivalents,
            "max_two_qubit_distance": circuit.metrics.max_distance,
            "weighted_gate_volume": circuit.metrics.weighted_gate_volume,
            "weighted_depth": circuit.metrics.weighted_depth,
        },
        "limits": {
            "max_qasm_bytes": limits.max_qasm_bytes,
            "max_gates": limits.max_gates,
            "max_depth": limits.max_depth,
            "max_two_qubit_gates": limits.max_two_qubit_gates,
            "max_rotation_gates": limits.max_rotation_gates,
            "max_remote_two_qubit_gates": limits.max_remote_two_qubit_gates,
            "max_routing_swap_equivalents": limits.max_routing_swap_equivalents,
            "max_two_qubit_distance": limits.max_two_qubit_distance,
            "max_weighted_gate_volume": limits.max_weighted_gate_volume,
            "max_weighted_depth": limits.max_weighted_depth,
        }
    })
}

fn validation_mode_name(args: &Args) -> &'static str {
    if args.preflight {
        "preflight"
    } else if args.smoke {
        "smoke"
    } else {
        "trusted"
    }
}

fn select_validation_shots(
    target: &Target,
    args: &Args,
    declared_qubits: usize,
) -> Result<(String, Vec<Shot>)> {
    if args.preflight {
        return Ok(("preflight".to_string(), Vec::new()));
    }
    if args.smoke {
        let shots = target
            .smoke_probes
            .clone()
            .into_iter()
            .map(|shot| shot_for_width(shot, declared_qubits))
            .collect::<Result<Vec<_>>>()?;
        return Ok((
            "smoke".to_string(),
            select_shard(shots, args.shot_shard.as_deref(), "--shot-shard")?,
        ));
    }

    let trusted = &target.validation.trusted_shots;
    let shot_count = args.shot_count.unwrap_or(trusted.count);
    if shot_count == 0 {
        return Err(VerifyError("--shot-count must be positive".to_string()));
    }
    if shot_count > trusted.count {
        return Err(VerifyError(format!(
            "--shot-count may not exceed trusted count {}",
            trusted.count
        )));
    }
    let shots = (0..shot_count)
        .map(|index| build_trusted_shot(target, index, declared_qubits))
        .collect::<Vec<_>>();
    Ok((
        "trusted".to_string(),
        select_shard(shots, args.shot_shard.as_deref(), "--shot-shard")?,
    ))
}

fn shot_for_width(mut shot: Shot, declared_qubits: usize) -> Result<Shot> {
    if shot.states.len() < declared_qubits {
        return Err(VerifyError(format!(
            "shot {} has {} states, cannot truncate to declared width {}",
            shot.name,
            shot.states.len(),
            declared_qubits
        )));
    }
    shot.states.truncate(declared_qubits);
    Ok(shot)
}

fn select_shard(items: Vec<Shot>, shard: Option<&str>, option_name: &str) -> Result<Vec<Shot>> {
    let Some(shard) = shard else {
        return Ok(items);
    };
    let Some((index_text, total_text)) = shard.split_once('/') else {
        return Err(VerifyError(format!(
            "{option_name} must use INDEX/TOTAL, for example 0/16"
        )));
    };
    let index = index_text
        .parse::<usize>()
        .map_err(|_| VerifyError(format!("invalid {option_name} index")))?;
    let total = total_text
        .parse::<usize>()
        .map_err(|_| VerifyError(format!("invalid {option_name} total")))?;
    if total == 0 || index >= total {
        return Err(VerifyError(format!(
            "{option_name} index must satisfy 0 <= INDEX < TOTAL"
        )));
    }
    let selected = items
        .into_iter()
        .enumerate()
        .filter_map(|(offset, shot)| (offset % total == index).then_some(shot))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(VerifyError(format!(
            "{option_name} {shard} selected no shots"
        )));
    }
    Ok(selected)
}

fn build_trusted_shot(target: &Target, shot_index: usize, declared_qubits: usize) -> Shot {
    let trusted = &target.validation.trusted_shots;
    let width_s = declared_qubits.to_string();
    let states = (0..declared_qubits)
        .map(|q| {
            let index = stable_u64(&[
                trusted.domain_separator.as_str(),
                target.target_id.as_str(),
                &width_s,
                &shot_index.to_string(),
                &q.to_string(),
            ]) as usize
                % trusted.state_alphabet.len();
            trusted.state_alphabet[index].clone()
        })
        .collect::<Vec<_>>();
    Shot {
        index: Some(shot_index),
        name: format!("shot_{shot_index:04}"),
        states,
    }
}

fn simulate(circuit: &Circuit, states: &[String], max_bond: usize, cutoff: f64) -> Result<Mps> {
    let mut sim = Mps::new(states, max_bond, cutoff)?;
    for instruction in &circuit.instructions {
        match instruction.qubits.len() {
            1 => {
                let gate = single_gate_matrix(&instruction.name, &instruction.params)?;
                sim.apply_single(instruction.qubits[0], gate);
            }
            2 => {
                let gate = two_gate_matrix(&instruction.name, &instruction.params)?;
                sim.apply_two(instruction.qubits[0], instruction.qubits[1], gate)?;
            }
            _ => unreachable!(),
        }
    }
    Ok(sim)
}

impl Mps {
    fn new(states: &[String], max_bond: usize, cutoff: f64) -> Result<Self> {
        let tensors = states
            .iter()
            .map(|state| {
                let vector = product_state_vector(state)?;
                Ok(Tensor::new(1, 1, vec![vector[0], vector[1]]))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            tensors,
            max_bond,
            cutoff,
            truncation_error: 0.0,
        })
    }

    fn apply_single(&mut self, q: usize, gate: [[C64; 2]; 2]) {
        let tensor = self.tensors[q].clone();
        let mut next = Tensor::new(
            tensor.left,
            tensor.right,
            vec![c0(); tensor.left * 2 * tensor.right],
        );
        for l in 0..tensor.left {
            for b in 0..2 {
                for r in 0..tensor.right {
                    let value = gate[b][0] * tensor.get(l, 0, r) + gate[b][1] * tensor.get(l, 1, r);
                    next.set(l, b, r, value);
                }
            }
        }
        self.tensors[q] = next;
    }

    fn apply_two(&mut self, q0: usize, q1: usize, gate: [[C64; 4]; 4]) -> Result<()> {
        if q0 == q1 {
            return Err(VerifyError(
                "two-qubit gate operands must be distinct".to_string(),
            ));
        }
        let left = q0.min(q1);
        let right = q0.max(q1);
        for site in (left + 1..right).rev() {
            self.apply_swap_adjacent(site)?;
        }
        let ordered_gate = reorder_two_qubit_gate(gate, (q0, q1), (left, right));
        self.apply_two_adjacent(left, ordered_gate)?;
        for site in left + 1..right {
            self.apply_swap_adjacent(site)?;
        }
        Ok(())
    }

    fn apply_swap_adjacent(&mut self, left: usize) -> Result<()> {
        self.apply_two_adjacent(left, swap_gate())
    }

    fn apply_two_adjacent(&mut self, left: usize, gate: [[C64; 4]; 4]) -> Result<()> {
        let a = self.tensors[left].clone();
        let b = self.tensors[left + 1].clone();
        let left_dim = a.left;
        let middle_dim = a.right;
        let right_dim = b.right;
        if middle_dim != b.left {
            return Err(VerifyError("inconsistent MPS bond dimensions".to_string()));
        }

        let mut theta = DMatrix::<C64>::zeros(left_dim * 2, 2 * right_dim);
        for l in 0..left_dim {
            for i in 0..2 {
                for j in 0..2 {
                    for r in 0..right_dim {
                        let mut value = c0();
                        for old_i in 0..2 {
                            for old_j in 0..2 {
                                let mut amp = c0();
                                for m in 0..middle_dim {
                                    amp += a.get(l, old_i, m) * b.get(m, old_j, r);
                                }
                                value += gate[i * 2 + j][old_i * 2 + old_j] * amp;
                            }
                        }
                        theta[(l * 2 + i, j * right_dim + r)] = value;
                    }
                }
            }
        }

        let svd = SVD::new(theta, true, true);
        let u = svd
            .u
            .ok_or_else(|| VerifyError("SVD did not return U".to_string()))?;
        let v_t = svd
            .v_t
            .ok_or_else(|| VerifyError("SVD did not return V^H".to_string()))?;
        let singular_values = svd.singular_values;
        let mut keep = singular_values.iter().filter(|&&s| s > self.cutoff).count();
        keep = keep.max(1).min(self.max_bond).min(singular_values.len());
        for s in singular_values.iter().skip(keep) {
            self.truncation_error += s * s;
        }

        let mut left_tensor = Tensor::new(left_dim, keep, vec![c0(); left_dim * 2 * keep]);
        for l in 0..left_dim {
            for i in 0..2 {
                for k in 0..keep {
                    left_tensor.set(l, i, k, u[(l * 2 + i, k)]);
                }
            }
        }

        let mut right_tensor = Tensor::new(keep, right_dim, vec![c0(); keep * 2 * right_dim]);
        for k in 0..keep {
            let sigma = C64::new(singular_values[k], 0.0);
            for j in 0..2 {
                for r in 0..right_dim {
                    right_tensor.set(k, j, r, sigma * v_t[(k, j * right_dim + r)]);
                }
            }
        }

        self.tensors[left] = left_tensor;
        self.tensors[left + 1] = right_tensor;
        Ok(())
    }
}

fn inner_product(left: &Mps, right: &Mps) -> C64 {
    let mut env = vec![c1()];
    let mut env_right = 1;
    for (a, b) in left.tensors.iter().zip(right.tensors.iter()) {
        let mut next = vec![c0(); a.right * b.right];
        for ar in 0..a.right {
            for br in 0..b.right {
                let mut value = c0();
                for al in 0..a.left {
                    for bl in 0..b.left {
                        let env_value = env[al * env_right + bl];
                        for p in 0..2 {
                            value += env_value * a.get(al, p, ar).conj() * b.get(bl, p, br);
                        }
                    }
                }
                next[ar * b.right + br] = value;
            }
        }
        env = next;
        env_right = b.right;
    }
    env[0]
}

fn single_gate_matrix(name: &str, params: &[f64]) -> Result<[[C64; 2]; 2]> {
    let one = c1();
    let zero = c0();
    match name {
        "x" => Ok([[zero, one], [one, zero]]),
        "y" => Ok([[zero, C64::new(0.0, -1.0)], [C64::new(0.0, 1.0), zero]]),
        "z" => Ok([[one, zero], [zero, -one]]),
        "h" => {
            let s = 1.0 / 2.0_f64.sqrt();
            Ok([
                [C64::new(s, 0.0), C64::new(s, 0.0)],
                [C64::new(s, 0.0), C64::new(-s, 0.0)],
            ])
        }
        "rz" => {
            if params.len() != 1 {
                return Err(VerifyError(format!(
                    "gate {name} expects one angle parameter"
                )));
            }
            let theta = params[0];
            Ok([[cis(-0.5 * theta), zero], [zero, cis(0.5 * theta)]])
        }
        _ => Err(VerifyError(format!(
            "unsupported single-qubit gate: {name}"
        ))),
    }
}

fn two_gate_matrix(name: &str, params: &[f64]) -> Result<[[C64; 4]; 4]> {
    if !params.is_empty() {
        return Err(VerifyError(format!(
            "gate {name} does not accept parameters"
        )));
    }
    let one = c1();
    let zero = c0();
    match name {
        "cx" | "cnot" => Ok([
            [one, zero, zero, zero],
            [zero, one, zero, zero],
            [zero, zero, zero, one],
            [zero, zero, one, zero],
        ]),
        _ => Err(VerifyError(format!("unsupported two-qubit gate: {name}"))),
    }
}

fn swap_gate() -> [[C64; 4]; 4] {
    let one = c1();
    let zero = c0();
    [
        [one, zero, zero, zero],
        [zero, zero, one, zero],
        [zero, one, zero, zero],
        [zero, zero, zero, one],
    ]
}

fn reorder_two_qubit_gate(
    gate: [[C64; 4]; 4],
    operands: (usize, usize),
    site_order: (usize, usize),
) -> [[C64; 4]; 4] {
    if operands == site_order {
        return gate;
    }
    if operands == (site_order.1, site_order.0) {
        let mut out = [[c0(); 4]; 4];
        for old_a in 0..2 {
            for old_b in 0..2 {
                for new_a in 0..2 {
                    for new_b in 0..2 {
                        out[new_b * 2 + new_a][old_b * 2 + old_a] =
                            gate[new_a * 2 + new_b][old_a * 2 + old_b];
                    }
                }
            }
        }
        return out;
    }
    unreachable!("site order must contain the same operands");
}

fn product_state_vector(label: &str) -> Result<[C64; 2]> {
    let s = 1.0 / 2.0_f64.sqrt();
    match label {
        "0" => Ok([c1(), c0()]),
        "1" => Ok([c0(), c1()]),
        "+" => Ok([C64::new(s, 0.0), C64::new(s, 0.0)]),
        "-" => Ok([C64::new(s, 0.0), C64::new(-s, 0.0)]),
        "i" => Ok([C64::new(s, 0.0), C64::new(0.0, s)]),
        "-i" => Ok([C64::new(s, 0.0), C64::new(0.0, -s)]),
        _ => Err(VerifyError(format!(
            "unsupported probe state label: {label:?}"
        ))),
    }
}

fn c0() -> C64 {
    C64::new(0.0, 0.0)
}

fn c1() -> C64 {
    C64::new(1.0, 0.0)
}

fn cis(theta: f64) -> C64 {
    C64::new(theta.cos(), theta.sin())
}

fn parse_angle(expr: &str) -> Result<f64> {
    let mut parser = AngleParser::new(expr);
    let value = parser.parse_expr()?;
    parser.skip_ws();
    if !parser.is_done() {
        return Err(VerifyError(format!(
            "unsupported angle expression: {expr:?}"
        )));
    }
    if !value.is_finite() {
        return Err(VerifyError(format!(
            "non-finite angle expression: {expr:?}"
        )));
    }
    Ok(value)
}

struct AngleParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> AngleParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn parse_expr(&mut self) -> Result<f64> {
        let mut value = self.parse_term()?;
        loop {
            self.skip_ws();
            if self.consume('+') {
                value += self.parse_term()?;
            } else if self.consume('-') {
                value -= self.parse_term()?;
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_term(&mut self) -> Result<f64> {
        let mut value = self.parse_factor()?;
        loop {
            self.skip_ws();
            if self.consume('*') {
                value *= self.parse_factor()?;
            } else if self.consume('/') {
                value /= self.parse_factor()?;
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_factor(&mut self) -> Result<f64> {
        self.skip_ws();
        if self.consume('+') {
            return self.parse_factor();
        }
        if self.consume('-') {
            return Ok(-self.parse_factor()?);
        }
        if self.consume('(') {
            let value = self.parse_expr()?;
            self.skip_ws();
            if !self.consume(')') {
                return Err(VerifyError(format!(
                    "missing ')' in angle expression {:?}",
                    self.input
                )));
            }
            return Ok(value);
        }
        if self.remaining().starts_with("pi") {
            self.pos += 2;
            return Ok(PI);
        }
        if self.remaining().starts_with("tau") {
            self.pos += 3;
            return Ok(TAU);
        }
        self.parse_number()
    }

    fn parse_number(&mut self) -> Result<f64> {
        self.skip_ws();
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() || matches!(ch, '.' | 'e' | 'E' | '+' | '-') {
                if matches!(ch, '+' | '-') && self.pos != start {
                    let prev = self.input[..self.pos].chars().next_back().unwrap_or('\0');
                    if prev != 'e' && prev != 'E' {
                        break;
                    }
                }
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
        if self.pos == start {
            return Err(VerifyError(format!(
                "expected angle number in {:?}",
                self.input
            )));
        }
        self.input[start..self.pos].parse::<f64>().map_err(|_| {
            VerifyError(format!(
                "invalid angle number {:?}",
                &self.input[start..self.pos]
            ))
        })
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(|ch| ch.is_whitespace()) {
            self.pos += self.peek().unwrap().len_utf8();
        }
    }

    fn consume(&mut self, ch: char) -> bool {
        if self.peek() == Some(ch) {
            self.pos += ch.len_utf8();
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn remaining(&self) -> &str {
        &self.input[self.pos..]
    }

    fn is_done(&self) -> bool {
        self.pos >= self.input.len()
    }
}

fn sha256_hex(payload: &[u8]) -> String {
    let digest = Sha256::digest(payload);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn stable_u64(parts: &[&str]) -> u64 {
    let payload = parts.join("|");
    let digest = Sha256::digest(payload.as_bytes());
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(bytes)
}

fn score_breakdown(metrics: &Metrics, scoring: &ScoringConfig, qubits: usize) -> serde_json::Value {
    let volume_product = metrics.weighted_gate_volume as f64 * metrics.weighted_depth as f64;
    let score = qubits as f64 * volume_product.sqrt();
    let native_single_qubit_gates =
        metrics.h_gates + metrics.x_gates + metrics.y_gates + metrics.z_gates;
    let rz_synthesis_volume = metrics.rz_gates * scoring.rotation_synthesis_weight;
    let cx_like_gates = metrics.cx_gates + metrics.cnot_gates;
    let cx_like_volume = cx_like_gates * scoring.entangling_gate_weight;
    let routing_entangling_volume =
        metrics.routing_swap_equivalents * scoring.swap_entangling_equivalent;
    json!({
        "model": scoring.model,
        "formula": "qubits * sqrt(weighted_gate_volume * weighted_depth)",
        "score": score,
        "qubits": qubits,
        "weighted_gate_volume": metrics.weighted_gate_volume,
        "weighted_depth": metrics.weighted_depth,
        "volume_product": volume_product,
        "allowed_gate_counts": {
            "h": metrics.h_gates,
            "x": metrics.x_gates,
            "y": metrics.y_gates,
            "z": metrics.z_gates,
            "rz": metrics.rz_gates,
            "cx": metrics.cx_gates,
            "cnot": metrics.cnot_gates,
        },
        "weighted_gate_volume_terms": {
            "native_one_qubit": native_single_qubit_gates,
            "rz_synthesis": rz_synthesis_volume,
            "cx_cnot_entangling": cx_like_volume,
            "routing_entangling": routing_entangling_volume,
        },
        "weighted_depth_model": {
            "native_one_qubit_duration": scoring.single_qubit_depth_weight,
            "rz_duration": scoring.rotation_depth_weight,
            "cx_cnot_base_duration": scoring.entangling_depth_weight,
            "routing_swap_per_extra_distance": scoring.routing_swap_per_extra_distance,
            "routing_swap_entangling_duration": scoring.swap_entangling_equivalent * scoring.entangling_depth_weight,
        },
        "entangling_gate_equivalents": metrics.entangling_gate_equivalents,
        "rotation_synthesis_equivalents": metrics.rotation_synthesis_equivalents,
        "routing_swap_equivalents": metrics.routing_swap_equivalents,
        "remote_two_qubit_gates": metrics.remote_two_qubit_gates,
        "two_qubit_distance_sum": metrics.two_qubit_distance_sum,
    })
}

fn metrics_json(metrics: &Metrics) -> serde_json::Value {
    json!({
        "gates": metrics.gates,
        "single_qubit_gates": metrics.single_qubit_gates,
        "two_qubit_gates": metrics.two_qubit_gates,
        "h_gates": metrics.h_gates,
        "x_gates": metrics.x_gates,
        "y_gates": metrics.y_gates,
        "z_gates": metrics.z_gates,
        "rz_gates": metrics.rz_gates,
        "rotation_gates": metrics.rotation_gates,
        "cx_gates": metrics.cx_gates,
        "cnot_gates": metrics.cnot_gates,
        "swap_gates": metrics.swap_gates,
        "remote_two_qubit_gates": metrics.remote_two_qubit_gates,
        "two_qubit_distance_sum": metrics.two_qubit_distance_sum,
        "routing_swap_equivalents": metrics.routing_swap_equivalents,
        "entangling_gate_equivalents": metrics.entangling_gate_equivalents,
        "rotation_synthesis_equivalents": metrics.rotation_synthesis_equivalents,
        "weighted_gate_volume": metrics.weighted_gate_volume,
        "weighted_depth": metrics.weighted_depth,
        "max_distance": metrics.max_distance,
        "depth": metrics.depth,
    })
}

fn summarize_shots(reports: &[ShotReport]) -> serde_json::Value {
    if reports.is_empty() {
        return json!({
            "evaluated_shots": 0,
            "min_fidelity": null,
            "max_infidelity": null,
            "max_norm_delta": null,
            "max_reference_truncation_error": null,
            "max_candidate_truncation_error": null,
        });
    }
    json!({
        "evaluated_shots": reports.len(),
        "min_fidelity": reports.iter().map(|report| report.fidelity).fold(f64::INFINITY, f64::min),
        "max_infidelity": reports.iter().map(|report| report.infidelity).fold(0.0, f64::max),
        "max_norm_delta": reports.iter().map(|report| report.norm_delta).fold(0.0, f64::max),
        "max_reference_truncation_error": reports.iter().map(|report| report.reference_truncation_error).fold(0.0, f64::max),
        "max_candidate_truncation_error": reports.iter().map(|report| report.candidate_truncation_error).fold(0.0, f64::max),
    })
}

fn shot_reports_json(reports: &[ShotReport]) -> Vec<serde_json::Value> {
    reports
        .iter()
        .map(|report| {
            json!({
                "name": report.name,
                "index": report.index,
                "fidelity": report.fidelity,
                "infidelity": report.infidelity,
                "reference_norm": report.reference_norm,
                "candidate_norm": report.candidate_norm,
                "norm_delta": report.norm_delta,
                "reference_truncation_error": report.reference_truncation_error,
                "candidate_truncation_error": report.candidate_truncation_error,
            })
        })
        .collect()
}

fn print_text_report(report: &serde_json::Value) {
    println!("target: {}", report["target_id"].as_str().unwrap_or(""));
    println!("candidate: {}", report["candidate"].as_str().unwrap_or(""));
    println!("ok: {}", report["ok"].as_bool().unwrap_or(false));
    println!("score: {:.6}", report["score"].as_f64().unwrap_or(0.0));
    if let Some(score_breakdown) = report["score_breakdown"].as_object() {
        println!(
            "score model: {}",
            score_breakdown
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("")
        );
        println!(
            "score breakdown: qubits={}, weighted_gate_volume={}, weighted_depth={}",
            score_breakdown
                .get("qubits")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            score_breakdown
                .get("weighted_gate_volume")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            score_breakdown
                .get("weighted_depth")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
        );
    }
    println!("metrics:");
    if let Some(metrics) = report["metrics"].as_object() {
        let ordered = [
            "gates",
            "single_qubit_gates",
            "two_qubit_gates",
            "rotation_gates",
            "cx_gates",
            "swap_gates",
            "remote_two_qubit_gates",
            "two_qubit_distance_sum",
            "routing_swap_equivalents",
            "entangling_gate_equivalents",
            "rotation_synthesis_equivalents",
            "weighted_gate_volume",
            "weighted_depth",
            "max_distance",
            "depth",
        ];
        for key in ordered {
            println!(
                "  {key}: {}",
                metrics.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
            );
        }
    }
    if let Some(validation) = report["validation"].as_object() {
        println!(
            "validation: {}, evaluated_shots={}, trusted_shots={}, shard={}",
            validation
                .get("mode")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            validation
                .get("evaluated_shots")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            validation
                .get("trusted_shots")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            validation
                .get("shot_shard")
                .and_then(|v| v.as_str())
                .unwrap_or("none")
        );
    }
    if let Some(summary) = report["shot_summary"].as_object() {
        if summary
            .get("evaluated_shots")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            > 0
        {
            println!(
                "shot summary: min_fidelity={:.12}, max_infidelity={:.3e}, max_norm_delta={:.3e}",
                summary
                    .get("min_fidelity")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0),
                summary
                    .get("max_infidelity")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0),
                summary
                    .get("max_norm_delta")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0),
            );
        }
    }
    if let Some(errors) = report["errors"].as_array() {
        if !errors.is_empty() {
            println!("errors:");
            for error in errors {
                println!("  - {}", error.as_str().unwrap_or(""));
            }
        }
    }
}
