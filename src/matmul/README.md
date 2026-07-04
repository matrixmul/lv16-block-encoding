# MatrixMul LV16 Baseline Memory Note

This note is packaged with `node matrixmul.js package --model MODEL`. The packaged
`dist/submission-note.md` starts with `Model: <LLM>` from `--model`; keep the
rest of this note focused on the submitted approach and evidence.

## Current Baseline Review

The checked-in editable implementation is intentionally thin:

```rust
pub fn render_qasm(target: &Value) -> String {
    crate::render_baseline_qasm(target)
}
```

All circuit structure therefore comes from the trusted repository generator in
`src/lib.rs`. The baseline is not an optimized hand-written circuit; it is the
canonical full-width 42-qubit reference circuit exposed through the editable
`src/matmul/mod.rs` entrypoint. MatrixMul LV16 submissions may declare widths
from 17 through 42 qubits. The verifier must use the implementation's declared
width consistently and must not validate a lower-width circuit by truncating or
projecting the 42-qubit target. Lower-width generated baselines are retired;
the fixed baseline remains the 42-qubit reference.

## Algorithm Shape

The target is a deterministic weighted ladder-Laplacian block-encoding problem:
logical level 16 and 4 rounds. The full checked-in target contains 42 qubits:
32 two-rail matrix-ladder qubits plus 10 block-encoding workspace qubits.

The starter generator emits OpenQASM 3.0 with `qubit[42] q;`, then prepares
every declared qubit with `h`. It walks the full target metadata terms round by
round:

- `z_phase`: emitted as one `rz(angle)` on the target qubit.
- `zz_phase`: emitted as `cx control,target; rz(angle) target; cx control,target`.
- `x_mixer`: emitted as `h; rz(angle); h` on the selected ladder qubit.

Angles are deterministically derived from SHA-256 based target metadata and are
printed to 12 decimal places. The submitted artifact hash matches the generated
full-width reference circuit hash recorded in `score.json`.

## Score Drivers

Current trusted score evidence from `score.json`:

- Score: `283024.0671745073`.
- Logical qubits: `42`.
- Gates: `948` total, with `86 h`, `414 rz`, and `448 cx`.
- Raw depth: `119`.
- Weighted gate volume: `28470`.
- Weighted depth: `1595`.

The weighted volume is dominated by arbitrary-angle rotations: `414 rz` gates
cost `64` each, contributing `26496` of the `28470` total weighted volume.
The CX gates contribute `448` base entangling units. The layout also has `240`
distance-2 CX operations, which add `480` routing swap equivalents and `1440`
more weighted entangling volume. Weighted depth is larger than raw depth
because `rz` gates have duration `16`, and routed CX spans synchronize all
qubits between the touched endpoints.

## Validation Evidence

The trusted validation gate records `9024` deterministic product-state shots
using the Matrix Product State verifier at the implementation's declared width.
For the current baseline that means comparing against the generated full-width
42-qubit reference. For lower-width implementations with no registered
reference artifact, validation runs against the submitted implementation itself
and never synthesizes a projected or truncated lower-width baseline.

## Optimization Opportunities

A competitive submission should preserve trusted-probe behavior while replacing
this literal generated-reference lowering with a smaller or better scheduled circuit.
The main opportunities are:

- Reduce the number of arbitrary `rz` rotations or combine/cancel phase terms.
- Reduce `cx` pairs around `zz_phase` terms through commuting or shared-basis
  structure.
- Avoid distance-2 couplings where possible, since each extra distance adds
  routing swap equivalents.
- Reschedule independent terms to lower weighted depth, especially around long
  runs of `rz` and routed `cx` operations.

Treat this baseline as the exact full-width reference and the first score
target, not as evidence that the current lowering is near optimal.
