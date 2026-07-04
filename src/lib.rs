use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

pub const SCHEMA_VERSION: &str = "1.0.0";
pub const TARGET_ID: &str = "matrixmul-lv16-varq-v3";
pub const TERM_DOMAIN_ID: &str = "matrixmul-lv16-term-domain-v1";
pub const LOGICAL_LEVEL: usize = 16;
pub const QUBIT_COUNT: usize = 42;
pub const SUBMISSION_QUBITS: usize = QUBIT_COUNT;
pub const MIN_SUBMISSION_QUBITS: usize = LOGICAL_LEVEL + 1;
pub const MAX_SUBMISSION_QUBITS: usize = QUBIT_COUNT;
pub const ROUND_COUNT: usize = 4;
pub const TRUSTED_SHOT_COUNT: usize = 9024;
pub const SHOT_STATE_ALPHABET: [&str; 6] = ["0", "1", "+", "-", "i", "-i"];
pub const SHOT_DOMAIN_SEPARATOR: &str = "matrixmul-lv16-varq-v3:trusted-shot:v1";

pub mod matmul;

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn stable_u64(parts: &[&str]) -> u64 {
    let payload = parts.join("|");
    let digest = Sha256::digest(payload.as_bytes());
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(bytes)
}

pub fn sha256_text(text: &str) -> String {
    sha256_hex(text.as_bytes())
}

pub fn sha256_hex(payload: &[u8]) -> String {
    let digest = Sha256::digest(payload);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn canonical_json(value: &Value) -> String {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            serde_json::to_string(value).expect("scalar JSON serialization cannot fail")
        }
        Value::Array(items) => {
            let body = items
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",");
            format!("[{body}]")
        }
        Value::Object(map) => {
            let mut keys = map.keys().collect::<Vec<_>>();
            keys.sort();
            let body = keys
                .into_iter()
                .map(|key| {
                    let encoded_key =
                        serde_json::to_string(key).expect("JSON key serialization cannot fail");
                    format!("{encoded_key}:{}", canonical_json(&map[key]))
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{body}}}")
        }
    }
}

pub fn sha256_json(value: &Value) -> String {
    sha256_text(&canonical_json(value))
}

pub fn write_json(path: &Path, value: &Value) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    fs::write(path, text)
}

pub fn centered_angle(scale: f64, parts: &[&str]) -> f64 {
    let mut stable_parts = vec![TERM_DOMAIN_ID];
    stable_parts.extend_from_slice(parts);
    let unit = stable_u64(&stable_parts) as f64 / ((1_u128 << 64) - 1) as f64;
    round12((2.0 * unit - 1.0) * scale)
}

fn round12(value: f64) -> f64 {
    (value * 1_000_000_000_000.0).round() / 1_000_000_000_000.0
}

fn object(entries: Vec<(&str, Value)>) -> Value {
    let mut map = Map::new();
    for (key, value) in entries {
        map.insert(key.to_string(), value);
    }
    Value::Object(map)
}

pub fn ladder_edges() -> Vec<Value> {
    let mut edges = Vec::new();
    for i in 0..LOGICAL_LEVEL {
        edges.push(object(vec![
            ("kind", json!("rung")),
            ("qubits", json!([2 * i, 2 * i + 1])),
        ]));
    }
    for i in 0..LOGICAL_LEVEL - 1 {
        edges.push(object(vec![
            ("kind", json!("upper_rail")),
            ("qubits", json!([2 * i, 2 * (i + 1)])),
        ]));
        edges.push(object(vec![
            ("kind", json!("lower_rail")),
            ("qubits", json!([2 * i + 1, 2 * (i + 1) + 1])),
        ]));
    }
    for q in 31..QUBIT_COUNT - 1 {
        edges.push(object(vec![
            ("kind", json!("block_ancilla_chain")),
            ("qubits", json!([q, q + 1])),
        ]));
    }
    edges
}

pub fn qubit_layout() -> Vec<Value> {
    (0..QUBIT_COUNT)
        .map(|q| {
            if q < 32 {
                object(vec![
                    ("index", json!(q)),
                    ("role", json!("matrix_ladder")),
                    ("rail", json!(if q % 2 == 0 { "upper" } else { "lower" })),
                    ("level", json!(q / 2)),
                ])
            } else {
                object(vec![
                    ("index", json!(q)),
                    ("role", json!("block_encoding_ancilla")),
                    ("level", json!(q - 32)),
                ])
            }
        })
        .collect()
}

pub fn build_terms() -> Vec<Value> {
    let mut terms = Vec::new();
    let edges = ladder_edges();
    for round_index in 0..ROUND_COUNT {
        let round_s = round_index.to_string();
        for q in 0..QUBIT_COUNT {
            let q_s = q.to_string();
            terms.push(object(vec![
                ("kind", json!("z_phase")),
                ("round", json!(round_index)),
                ("qubits", json!([q])),
                (
                    "angle",
                    json!(centered_angle(0.083, &["z", &round_s, &q_s])),
                ),
            ]));
        }

        for (edge_index, edge) in edges.iter().enumerate() {
            let edge_kind = edge["kind"].as_str().expect("edge kind");
            let edge_index_s = edge_index.to_string();
            let angle_scale = if edge_kind == "block_ancilla_chain" {
                0.031
            } else {
                0.047
            };
            terms.push(object(vec![
                ("kind", json!("zz_phase")),
                ("round", json!(round_index)),
                ("edge_kind", json!(edge_kind)),
                ("edge_index", json!(edge_index)),
                ("qubits", edge["qubits"].clone()),
                (
                    "angle",
                    json!(centered_angle(
                        angle_scale,
                        &["zz", &round_s, &edge_index_s]
                    )),
                ),
            ]));
        }

        for level in 0..LOGICAL_LEVEL {
            if (level + round_index) % 3 == 0 {
                let q = 2 * level + (round_index % 2);
                let level_s = level.to_string();
                terms.push(object(vec![
                    ("kind", json!("x_mixer")),
                    ("round", json!(round_index)),
                    ("qubits", json!([q])),
                    (
                        "angle",
                        json!(centered_angle(0.059, &["x", &round_s, &level_s])),
                    ),
                ]));
            }
        }
    }
    terms
}

pub fn build_smoke_probes() -> Vec<Value> {
    let mut probes = Vec::new();
    probes.push(object(vec![
        ("name", json!("zero")),
        ("states", json!(vec!["0"; QUBIT_COUNT])),
    ]));
    probes.push(object(vec![
        ("name", json!("uniform_plus")),
        ("states", json!(vec!["+"; QUBIT_COUNT])),
    ]));

    let computational = (0..QUBIT_COUNT)
        .map(|q| {
            let q_s = q.to_string();
            if matches!(stable_u64(&["probe", "computational", &q_s]) % 5, 0 | 3) {
                "1"
            } else {
                "0"
            }
        })
        .collect::<Vec<_>>();
    probes.push(object(vec![
        ("name", json!("deterministic_computational")),
        ("states", json!(computational)),
    ]));

    let phase_states = (0..QUBIT_COUNT)
        .map(|q| {
            let q_s = q.to_string();
            SHOT_STATE_ALPHABET
                [stable_u64(&["probe", "phase", &q_s]) as usize % SHOT_STATE_ALPHABET.len()]
        })
        .collect::<Vec<_>>();
    probes.push(object(vec![
        ("name", json!("phase_product")),
        ("states", json!(phase_states)),
    ]));
    probes
}

pub fn build_trusted_shot_for_width(shot_index: usize, qubits: usize) -> Value {
    let shot_s = shot_index.to_string();
    let width_s = qubits.to_string();
    let states = (0..qubits)
        .map(|q| {
            let q_s = q.to_string();
            SHOT_STATE_ALPHABET[stable_u64(&[
                SHOT_DOMAIN_SEPARATOR,
                TARGET_ID,
                &width_s,
                &shot_s,
                &q_s,
            ]) as usize
                % SHOT_STATE_ALPHABET.len()]
        })
        .collect::<Vec<_>>();
    object(vec![
        ("index", json!(shot_index)),
        ("name", json!(format!("shot_{shot_index:04}"))),
        ("states", json!(states)),
    ])
}

pub fn build_trusted_shot(shot_index: usize) -> Value {
    build_trusted_shot_for_width(shot_index, SUBMISSION_QUBITS)
}

pub fn trusted_shot_stream_sha256_for_width(qubits: usize) -> String {
    let shots = (0..TRUSTED_SHOT_COUNT)
        .map(|index| build_trusted_shot_for_width(index, qubits))
        .collect::<Vec<_>>();
    sha256_json(&Value::Array(shots))
}

pub fn trusted_shot_stream_sha256() -> String {
    trusted_shot_stream_sha256_for_width(SUBMISSION_QUBITS)
}

pub fn trusted_shot_width_range_sha256() -> String {
    let streams = (MIN_SUBMISSION_QUBITS..=MAX_SUBMISSION_QUBITS)
        .map(|qubits| {
            object(vec![
                ("qubits", json!(qubits)),
                (
                    "stream_sha256",
                    json!(trusted_shot_stream_sha256_for_width(qubits)),
                ),
            ])
        })
        .collect::<Vec<_>>();
    sha256_json(&Value::Array(streams))
}

pub fn build_target_metadata() -> Value {
    let terms = Value::Array(build_terms());
    let term_digest = sha256_json(&terms);
    let edges = Value::Array(ladder_edges());
    let mut target = object(vec![
        ("schema_version", json!(SCHEMA_VERSION)),
        ("target_id", json!(TARGET_ID)),
        ("term_domain_id", json!(TERM_DOMAIN_ID)),
        ("name", json!("Block-Encoding Matrix Ladder LV16")),
        ("qubits", json!(QUBIT_COUNT)),
        ("logical_level", json!(LOGICAL_LEVEL)),
        ("rounds", json!(ROUND_COUNT)),
        (
            "description",
            json!(
                "Deterministic weighted ladder-Laplacian block-encoding target. \
                 The 32 ladder qubits encode a level-16 two-rail sparse matrix; \
                 the final 10 qubits are block-encoding workspace."
            ),
        ),
        (
            "laplacian",
            object(vec![
                ("matrix_dimension", json!(1_u64 << LOGICAL_LEVEL)),
                ("rails", json!(2)),
                ("levels", json!(LOGICAL_LEVEL)),
                ("boundary", json!("open")),
                ("edge_count", json!(ladder_edges().len())),
                ("edge_digest", json!(sha256_json(&edges))),
            ]),
        ),
        ("qubit_layout", Value::Array(qubit_layout())),
        ("terms", terms),
        ("terms_sha256", json!(term_digest)),
        ("smoke_probes", Value::Array(build_smoke_probes())),
        (
            "validation",
            object(vec![(
                "trusted_shots",
                object(vec![
                    ("count", json!(TRUSTED_SHOT_COUNT)),
                    ("domain_separator", json!(SHOT_DOMAIN_SEPARATOR)),
                    ("state_alphabet", json!(SHOT_STATE_ALPHABET)),
                    ("stream_sha256", json!(trusted_shot_stream_sha256())),
                    (
                        "width_range_stream_sha256",
                        json!(trusted_shot_width_range_sha256()),
                    ),
                    (
                        "derivation",
                        json!(
                            "For declared width n, shot index s, and qubit q with 0 <= q < n, choose state_alphabet[\
                             sha256(domain_separator|target_id|n|s|q)[0:8] mod len(state_alphabet)]."
                        ),
                    ),
                ]),
            )]),
        ),
        (
            "limits",
            object(vec![
                ("min_qubits", json!(MIN_SUBMISSION_QUBITS)),
                ("max_qubits", json!(MAX_SUBMISSION_QUBITS)),
                ("max_gates", json!(50_000)),
                ("max_depth", json!(50_000)),
                ("max_two_qubit_gates", json!(30_000)),
                ("max_rotation_gates", json!(20_000)),
                ("max_remote_two_qubit_gates", json!(20_000)),
                ("max_routing_swap_equivalents", json!(100_000)),
                ("max_two_qubit_distance", json!(8)),
                ("max_weighted_gate_volume", json!(2_000_000)),
                ("max_weighted_depth", json!(100_000)),
                ("max_qasm_bytes", json!(5_000_000)),
            ]),
        ),
        (
            "scoring",
            object(vec![
                ("model", json!("logical_hardware_volume_v1")),
                (
                    "description",
                    json!(
                        "Lower is better. Reference-style hardware volume: declared qubits times sqrt(weighted gate volume times weighted depth). For v1 gates, weighted gate volume is h+x+y+z plus 64*rz plus cx/cnot plus 6 per extra two-qubit distance."
                    ),
                ),
                ("single_qubit_weight", json!(1)),
                ("single_qubit_depth_weight", json!(1)),
                ("rotation_synthesis_weight", json!(64)),
                ("rotation_depth_weight", json!(16)),
                ("entangling_gate_weight", json!(1)),
                ("entangling_depth_weight", json!(1)),
                ("swap_entangling_equivalent", json!(3)),
                ("routing_swap_per_extra_distance", json!(2)),
            ]),
        ),
        (
            "verifier",
            object(vec![
                ("default_max_bond", json!(64)),
                ("svd_cutoff", json!(1e-12)),
                ("fidelity_atol", json!(1e-8)),
                ("norm_atol", json!(1e-8)),
            ]),
        ),
    ]);

    let reference_qasm = render_baseline_qasm(&target);
    let full_width_reference = object(vec![
        ("path", json!("generated:src/util/generate_baseline.rs")),
        ("sha256", json!(sha256_text(&reference_qasm))),
        ("generator", json!("src/util/generate_baseline.rs")),
        (
            "width_policy",
            json!(
                "The verifier must validate candidates against an official reference for the \
                 same declared width. The stored sha256 is the full 42-qubit target reference; \
                 lower-width references must be separately registered and are never synthesized \
                 by truncating the 42-qubit target."
            ),
        ),
    ]);
    target["reference"] = full_width_reference.clone();
    let mut by_width = Map::new();
    by_width.insert(QUBIT_COUNT.to_string(), full_width_reference);
    target["references"] = object(vec![
        (
            "policy",
            json!(
                "A declared width is valid only when an official same-width reference exists. \
                 Do not project a wider target by skipping terms outside the declared width."
            ),
        ),
        ("by_width", Value::Object(by_width)),
    ]);
    let mut without_metadata = target.clone();
    without_metadata
        .as_object_mut()
        .expect("target object")
        .remove("metadata_sha256");
    target["metadata_sha256"] = json!(sha256_json(&without_metadata));
    target
}

pub fn render_baseline_qasm(target: &Value) -> String {
    let qubits = target["qubits"].as_u64().expect("target qubits") as usize;
    render_reference_qasm(target, qubits)
}

pub fn render_reference_qasm(target: &Value, declared_qubits: usize) -> String {
    let max_qubits = target["qubits"].as_u64().expect("target qubits") as usize;
    assert!(declared_qubits > 0, "declared qubits must be positive");
    assert!(
        declared_qubits <= max_qubits,
        "declared qubits exceed target max width"
    );
    let qubits = declared_qubits;
    let terms = target["terms"].as_array().expect("target terms");
    let mut lines = vec![
        "OPENQASM 3.0;".to_string(),
        "include \"stdgates.inc\";".to_string(),
        String::new(),
        format!("// target_id: {}", target["target_id"].as_str().unwrap()),
        format!("// declared_width: {qubits}"),
        format!(
            "// terms_sha256: {}",
            target["terms_sha256"].as_str().unwrap()
        ),
        "// Generated by src/util/generate_baseline.rs. This checked-in baseline is editable;"
            .to_string(),
        "// preserve full-width target equivalence.".to_string(),
        format!("qubit[{qubits}] q;"),
        String::new(),
        "// Uniform full-width workspace preparation.".to_string(),
    ];

    for q in 0..qubits {
        lines.push(format!("h q[{q}];"));
    }

    let mut current_round = None;
    for term in terms {
        let round = term["round"].as_u64().expect("term round") as usize;
        if current_round != Some(round) {
            current_round = Some(round);
            lines.push(String::new());
            lines.push(format!("// Matrix ladder round {round}"));
        }

        let kind = term["kind"].as_str().expect("term kind");
        let qubit_values = term["qubits"].as_array().expect("term qubits");
        let qubits = qubit_values
            .iter()
            .map(|value| value.as_u64().expect("qubit") as usize)
            .collect::<Vec<_>>();
        if qubits.iter().any(|&q| q >= declared_qubits) {
            continue;
        }
        let angle = qasm_angle(term["angle"].as_f64().expect("term angle"));
        match kind {
            "z_phase" => lines.push(format!("rz({angle}) q[{}];", qubits[0])),
            "zz_phase" => {
                lines.push(format!("cx q[{}], q[{}];", qubits[0], qubits[1]));
                lines.push(format!("rz({angle}) q[{}];", qubits[1]));
                lines.push(format!("cx q[{}], q[{}];", qubits[0], qubits[1]));
            }
            "x_mixer" => {
                lines.push(format!("h q[{}];", qubits[0]));
                lines.push(format!("rz({angle}) q[{}];", qubits[0]));
                lines.push(format!("h q[{}];", qubits[0]));
            }
            other => panic!("unsupported term kind: {other}"),
        }
    }

    lines.push(String::new());
    lines.join("\n")
}

fn qasm_angle(value: f64) -> String {
    assert!(value.is_finite(), "non-finite angle: {value}");
    format!("{value:.12}")
}
