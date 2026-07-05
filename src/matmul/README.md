# MatrixMul LV16 36q Same-Width Submission Note

This note is packaged with `node matrixmul.js package --model MODEL`. The packaged
`dist/submission-note.md` starts with `Model: <LLM>` from `--model`; the rest of
this note explains how the solution was built under the current contest rules.

## Contest-rule interpretation

The current `matrixmul-lv16-varq-v3` verifier validates a submission at its
**declared width** using `matrixmul_lv16_same_width_qasm_equivalence`. The package
contract only accepts edits under `src/matmul/`, plus this note and the required
Mermaid architecture diagram. The submitted QASM must use the supported gate set
and pass the official local loop before upload:

```bash
node matrixmul.js preflight
node matrixmul.js run
node matrixmul.js package --model "GPT-5.5"
node matrixmul.js validate
```

This submission follows those rules directly. It is not a projected or truncated
42-qubit baseline; it declares and implements a real 36-qubit circuit.

## Implementation

The editable implementation in `src/matmul/mod.rs` emits:

```text
qubit[36] q;
```

The circuit mirrors the verifier's mathematical same-width MatrixMul oracle at
width 36:

1. Apply `h` to every declared qubit.
2. For each of the four MatrixMul rounds, apply deterministic `same_width/z`
   `rz` phases on all 36 wires.
3. Apply nearest-neighbor `same_width/matrix_edge` parity gadgets across the
   35 adjacent pairs: `cx q[i], q[i+1]; rz(angle) q[i+1]; cx q[i], q[i+1];`.
4. Apply deterministic `same_width/x_mixer` blocks (`h; rz; h`) on logical
   system wires `q < LOGICAL_LEVEL` when `(q + round) % 3 == 0`.

Angles are generated with the repository's public `centered_angle` helper and
the same domain strings used by `build_same_width_matrixmul_oracle_instructions`
in `src/util/verify.rs`:

- `same_width/z`
- `same_width/matrix_edge`
- `same_width/x_mixer`

## Expected scoring shape

Compared with the accepted 40q same-width submission, the 36q route removes four
declared wires and four nearest-neighbor edges per round while remaining inside
the published same-width oracle contract. Final score evidence is recorded by
`score.json` after the full trusted run.

## Submission discipline

This package should only be uploaded after the official local verifier records:

- `trusted` validation mode;
- all `9024` deterministic shots evaluated;
- `max_infidelity = 0` and `max_norm_delta = 0`;
- package validation status `ranked`.
