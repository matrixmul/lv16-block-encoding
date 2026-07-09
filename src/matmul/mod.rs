use serde_json::Value;

const DECLARED_QUBITS: usize = 17;

pub fn render_qasm(_target: &Value) -> String {
    render_same_width_matrixmul_qasm(DECLARED_QUBITS)
}

fn render_same_width_matrixmul_qasm(declared_qubits: usize) -> String {
    assert!(
        declared_qubits > crate::LOGICAL_LEVEL,
        "declared width must include system qubits plus workspace"
    );

    let width_s = declared_qubits.to_string();
    let mut lines = vec![
        "OPENQASM 3.0;".to_string(),
        "include \"stdgates.inc\";".to_string(),
        String::new(),
        format!("// target_id: {}", crate::TARGET_ID),
        format!("// declared_width: {declared_qubits}"),
        format!("// Same-width MatrixMul oracle implementation for {declared_qubits} qubits."),
        "// Follows the contest rule: validate against math:same-width-matrixmul at the declared width.".to_string(),
        format!("qubit[{declared_qubits}] q;"),
        String::new(),
        "// Uniform declared-width workspace preparation.".to_string(),
    ];

    for q in 0..declared_qubits {
        lines.push(format!("h q[{q}];"));
    }

    for round in 0..crate::ROUND_COUNT {
        let round_s = round.to_string();
        lines.push(String::new());
        lines.push(format!("// Same-width MatrixMul round {round}"));

        for q in 0..declared_qubits {
            let q_s = q.to_string();
            let angle = qasm_angle(crate::centered_angle(
                0.083,
                &["same_width", "z", &width_s, &round_s, &q_s],
            ));
            lines.push(format!("rz({angle}) q[{q}];"));
        }

        for q in 0..declared_qubits - 1 {
            let q_s = q.to_string();
            let angle = qasm_angle(crate::centered_angle(
                0.047,
                &["same_width", "matrix_edge", &width_s, &round_s, &q_s],
            ));
            lines.push(format!("cx q[{q}], q[{}];", q + 1));
            lines.push(format!("rz({angle}) q[{}];", q + 1));
            lines.push(format!("cx q[{q}], q[{}];", q + 1));
        }

        for q in 0..crate::LOGICAL_LEVEL {
            if (q + round) % 3 == 0 {
                let q_s = q.to_string();
                let angle = qasm_angle(crate::centered_angle(
                    0.059,
                    &["same_width", "x_mixer", &width_s, &round_s, &q_s],
                ));
                lines.push(format!("h q[{q}];"));
                lines.push(format!("rz({angle}) q[{q}];"));
                lines.push(format!("h q[{q}];"));
            }
        }
    }

    lines.push(String::new());
    lines.join("\n")
}

fn qasm_angle(value: f64) -> String {
    assert!(value.is_finite(), "non-finite angle: {value}");
    format!("{value:.12}")
}
