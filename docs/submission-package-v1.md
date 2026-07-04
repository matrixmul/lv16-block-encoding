# MatrixMul Submission Package v1

`node matrixmul.js package --model MODEL` creates `dist/submission.tar.gz`,
`dist/submission-note.md`, and `dist/submission-metadata.json`.

The package archive contains only `src/matmul`. The QASM circuit is generated
from that source into `dist/solution.qasm` before verification and package
metadata hashing.

The package is valid only when `score.json` was produced by a clean trusted
verifier run over all 9024 deterministic shots. This is a finite deterministic
probe contract, not a symbolic proof of full operator equivalence. Cheap
`preflight`, `smoke`, and small `--shot-count` runs are local reject paths and
cannot be submitted.

MatrixMul LV16 packages may declare any width from `qubit[17] q;` through
`qubit[42] q;`. The trusted verifier must use the candidate's declared width
consistently for the mathematical same-width MatrixMul oracle and trusted
shots, and it must never validate by truncating, projecting, or self-comparing
against the fixed 42-qubit baseline. Generated lower-width baselines are
retired; the checked-in baseline remains the full 42-qubit starter artifact.

Required metadata fields include:

- `benchmark: "matrixmul-lv16-varq-v3"`
- `editablePaths: ["src/matmul"]`
- `artifact: "dist/solution.qasm"`
- `architectureDiagram.path: "src/matmul/architecture.mmd"`
- `scoreModel: "logical_hardware_volume_v1"`
- `validation.shots: 9024`
- `validation.gate: "matrixmul_lv16_same_width_qasm_equivalence"`
- `artifactSha256`, binding the trusted score to the submitted QASM
- `archiveSha256`, binding submitted metadata to the exact packaged source
  archive
- `architectureDiagram.sha256`, binding the leaderboard modal preview to the
  submitted Mermaid diagram

The server recomputes the score as:

```text
qubits * sqrt(weighted_gate_volume * weighted_depth)
```

For v1 gates, `weighted_gate_volume` is:

```text
count(h,x,y,z) + 64*count(rz) + count(cx,cnot)
  + 6*sum(max(abs(q0-q1)-1, 0)) over cx/cnot gates
```

`weighted_depth` schedules the same costs on the touched qubit span.

The trusted verifier applies the target's cost guard before shot simulation.
Packages are not rankable if the generated QASM exceeds the static caps for
QASM bytes, gate counts, rotation count, two-qubit distance, routing
SWAP-equivalents, weighted gate volume, weighted depth, or MPS truncation
tolerance.

`architecture.mmd` must be UTF-8 Mermaid beginning with a `flowchart` or `graph`
declaration and include these exact labels:

- `Target circuit: MatrixMul LV16`
- `Algorithm`
- `Optimization`

The target root must have outgoing edges to `Algorithm` and `Optimization`. Use
the `Algorithm` branch to explain the submitted circuit structure and use the
`Optimization` branch to explain score tradeoffs, simplifications, and search
choices. `dist/submission-note.md` begins with `Model: <LLM>` from `--model`;
the note body should explain the submitted approach and evidence.
