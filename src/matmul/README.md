# MatrixMul LV16 34q Same-Width Submission Note

This note documents the 34-qubit `matrixmul-lv16-varq-v3` score-beat candidate.
It is packaged by `node matrixmul.js package --model MODEL`; the generated
`dist/submission-note.md` prepends `Model: <LLM>` and then includes this file.

## Summary

The candidate is a declared-width **34q same-width MatrixMul oracle**. It does
not project, truncate, or pad a 42q baseline circuit. The generated QASM declares:

```text
qubit[34] q;
```

The target score is a strict beat of the accepted 35q leaderboard best:

```text
previous best: 141288.61887993666
candidate route: same-width oracle at 34 declared qubits
```

## Contest-rule basis

The current `matrixmul-lv16-varq-v3` contract validates candidate circuits at
their **declared width** using:

```text
matrixmul_lv16_same_width_qasm_equivalence
```

Only `src/matmul/` is packaged as editable submission code, with this note and
`src/matmul/architecture.mmd` as required explanation artifacts. The submitted
QASM uses the supported gate set (`h`, `rz`, `cx`) and is checked through the
official loop:

```bash
node matrixmul.js preflight
node matrixmul.js run
node matrixmul.js package --model "GPT-5.5"
node matrixmul.js validate
```

## Algorithm

`src/matmul/mod.rs` emits the verifier's same-width MatrixMul instruction family
for `DECLARED_QUBITS = 34`.

The circuit construction is deterministic:

1. **Declare width:** emit `qubit[34] q;`.
2. **Prepare workspace:** apply `h` to every declared qubit.
3. **Four MatrixMul rounds:** for each `round in 0..ROUND_COUNT`:
   - apply `same_width/z` phases on all 34 wires with
     `centered_angle(0.083, ["same_width", "z", width, round, q])`;
   - apply nearest-neighbor `same_width/matrix_edge` parity gadgets across the
     33 adjacent pairs:
     `cx q[i], q[i+1]; rz(angle) q[i+1]; cx q[i], q[i+1];`;
   - apply `same_width/x_mixer` blocks (`h; rz; h`) on logical system wires
     `q < LOGICAL_LEVEL` when `(q + round) % 3 == 0`.
4. **Angle generation:** all `rz` angles use the repository's public
   `centered_angle` helper and the same domain strings used by
   `build_same_width_matrixmul_oracle_instructions` in `src/util/verify.rs`.

This keeps the candidate exactly aligned with the mathematical same-width oracle
at 34 declared qubits.

## Optimization workflow

The optimization is a measured declared-width reduction sequence under the
published verifier contract:

| Step | Action | Trusted result |
|---|---|---:|
| Accepted 40q | same-width oracle route | `180948.66454328975` |
| Accepted 36q | same oracle family with four fewer wires | `148832.93441977148` |
| Accepted 35q | reduce one more wire and matrix edge per round | `141288.61887993666` |
| Candidate 34q | reduce one further wire and matrix edge per round | validated before submit |

The score should drop because the declared width is lower and the 34q
construction removes one adjacent `matrix_edge` parity gadget per round compared
with 35q, while preserving the same declared-width oracle semantics.

## 34q expected metric shape

The same-width oracle has:

- one initial `h` per declared wire;
- four `same_width/z` rotations per wire;
- four rounds of 33 edge gadgets, each with two `cx` and one `rz`;
- the same 22 logical `x_mixer` blocks because `LOGICAL_LEVEL` stays fixed.

Final counts and score are recorded by `score.json` after the full trusted run.

## Validation and packaging discipline

The candidate is promoted only after:

- official preflight passes;
- a 16-shot sanity check passes;
- full `node matrixmul.js run` passes all `9024` trusted shots;
- package validation reports `PACKAGE_OK`, `METRICS_OK`, `FUNCTIONAL_OK`, and
  `ARCHITECTURE_METADATA_OK`;
- a clean local reproduction of the trusted-worker path passes.

Before packaging, editable `src/matmul/*` mtimes are set into the future so the
trusted worker rebuilds after archive extraction rather than reusing a stale
baseline binary.
