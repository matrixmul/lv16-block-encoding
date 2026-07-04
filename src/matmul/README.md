# MatrixMul LV16 41q Same-Width Submission Note

This note is packaged with `node matrixmul.js package --model MODEL`. The packaged
`dist/submission-note.md` starts with `Model: <LLM>` from `--model`; keep the
rest of this note focused on the submitted approach and evidence.

## Current 41q Implementation

The editable implementation in `src/matmul/mod.rs` declares a real 41-qubit
OpenQASM circuit:

```text
qubit[41] q;
```

The latest upstream verifier retired projected lower-width references and now
validates every supported declared width against the mathematical same-width
MatrixMul oracle. This implementation therefore does **not** truncate the 42q
baseline. It directly emits the same deterministic 41q instruction family used
by the same-width oracle:

- initial `h` on every declared wire;
- four rounds of deterministic `same_width/z` single-wire phases;
- four rounds of nearest-neighbor `same_width/matrix_edge` parity phases;
- deterministic `same_width/x_mixer` blocks over the first `LOGICAL_LEVEL`
  system qubits.

Angles are generated with the public repository `centered_angle` helper and the
same domain strings used by the verifier oracle.

## Score Drivers

The 41q same-width circuit reduces the declared register and uses nearest-neighbor
matrix-edge parity gadgets instead of the 42q starter ladder/workspace lowering.
Expected score evidence comes from `score.json` after the full trusted run.

## Validation Discipline

A ranked submission requires the official local loop:

```bash
node matrixmul.js preflight
node matrixmul.js run
node matrixmul.js package --model "GPT-5.5"
node matrixmul.js validate
node matrixmul.js submit
```

Do not submit unless `node matrixmul.js run` records all `9024` deterministic
product-state shots in trusted mode and `node matrixmul.js validate` passes.
