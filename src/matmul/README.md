# MatrixMul LV16 17q Same-Width Submission Note

This note documents a 17-qubit `matrixmul-lv16-varq-v3` score-beat candidate. It is packaged by `matrixmul package --model MODEL`; the generated `dist/submission-note.md` prepends `Model: <LLM>` and then includes this file.

## Summary

The candidate is a declared-width **17q same-width MatrixMul oracle**. It does not project, truncate, or pad a 42q baseline circuit. The generated QASM declares:

```text
qubit[17] q;
```

The current leaderboard best before this candidate was the accepted 32q route:

```text
current best: 119818.95676394449
candidate:    39335.75555394863
```

## Contest-rule basis

The `matrixmul-lv16-varq-v3` contract accepts declared widths from 17 through 42 and validates candidate circuits at their **declared width** using:

```text
matrixmul_lv16_same_width_qasm_equivalence
```

Only `src/matmul/` is packaged as editable submission code, with this note and `src/matmul/architecture.mmd` as required explanation artifacts. The submitted QASM uses only supported gates (`h`, `rz`, `cx`) and is checked through the official loop:

```bash
matrixmul preflight
cargo run --release --bin verify -- dist/solution.qasm --shot-count 64 --json
matrixmul run
matrixmul package --model "Hermes gpt-5.5"
matrixmul validate
```

This submission follows those rules directly: the declared width is the actual implementation width, and every operation addresses only `q[0]` through `q[16]`.

## Algorithm

`src/matmul/mod.rs` emits the verifier's same-width MatrixMul instruction family for `DECLARED_QUBITS = 17`.

The circuit construction is deterministic:

1. **Declare width:** emit `qubit[17] q;`.
2. **Prepare workspace:** apply `h` to every declared qubit.
3. **Four MatrixMul rounds:** for each `round in 0..ROUND_COUNT`:
   - apply `same_width/z` phases on all 17 wires with `centered_angle(0.083, ["same_width", "z", width, round, q])`;
   - apply nearest-neighbor `same_width/matrix_edge` parity gadgets across the 16 adjacent pairs: `cx q[i], q[i+1]; rz(angle) q[i+1]; cx q[i], q[i+1];`;
   - apply `same_width/x_mixer` blocks (`h; rz; h`) on logical system wires `q < LOGICAL_LEVEL` when `(q + round) % 3 == 0`.
4. **Angle generation:** all `rz` angles use the repository's public `centered_angle` helper and the same domain strings used by `build_same_width_matrixmul_oracle_instructions` in `src/util/verify.rs`.

This keeps the candidate aligned with the mathematical same-width oracle at 17 qubits instead of relying on an external projected reference.

## Optimization workflow

The optimization is a measured declared-width reduction under the published verifier contract:

| Step | Action | Trusted/local result |
|---|---|---:|
| Accepted 32q | same-width oracle route | `119818.95676394449` |
| Candidate 17q | minimum allowed declared width | `39335.75555394863` |

The score drops because the declared width is lower and the 17q construction removes 15 adjacent `matrix_edge` parity gadgets per round compared with 32q, while preserving the same declared-width oracle semantics.

## 17q metric shape

The same-width oracle has:

- one initial `h` per declared wire;
- four `same_width/z` rotations per wire;
- four rounds of 16 edge gadgets, each with two `cx` and one `rz`;
- the same 22 logical `x_mixer` blocks because `LOGICAL_LEVEL` stays fixed.

Validated metrics:

```text
score: 39335.75555394863
qubits: 17
weighted_gate_volume: 10045
weighted_depth: 533
gates: 343
h: 61
rz: 154
cx: 128
max two-qubit distance: 1
```

## Validation and packaging evidence

The candidate is promoted only after:

- official preflight passes;
- a 64-shot trusted sanity check passes;
- full `matrixmul run` passes all `9024` trusted shots;
- package validation reports `PACKAGE_OK`, `METRICS_OK`, `FUNCTIONAL_OK`, and `ARCHITECTURE_METADATA_OK`.

Credential discipline: submission should use an API token only in process environment or the contest UI/session, not in source files, shell profiles, git config, docs, or logs.
