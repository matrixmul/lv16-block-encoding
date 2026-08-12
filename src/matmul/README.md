# MatrixMul LV16 Clifford Phase-Dialog Candidate

## Candidate

The circuit declares 17 qubits and emits the normal same-width MatrixMul
construction except for the final `q14-q16` parity pair. Define:

```text
a  = CX(14,15)
A  = CX(15,14)
b  = CX(15,16)
h  = H(15)
L  = RZ(edge14) on q14
R  = RZ(edge15) on q16
M  = RZ(mixer15) on q15
```

The selected Clifford Dialog is:

```text
a a b A h h L R b A h M h
```

The CNOTs and Hadamards are tracked with a signed Clifford tableau. `L` and `R`
are emitted only when the physical Z images equal `Z14*Z15` and `Z15*Z16`;
`M` is emitted only in the q15 X basis. Erasing the three phase nodes leaves an
identity tableau. `L` and `R` share one weighted-depth layer.

The final-round leading Z phase on q16 is biased by `-2.262829e-6`. A dense
12-decimal active-set search selected this basin after complete Windows and
Ubuntu verifier replays.

## Metrics

```text
previous ranked best: 38860.82500410921
candidate score:      38786.020651776074
improvement:             74.804352333136

qubits: 17
h / rz / cx: 63 / 154 / 130
weighted_gate_volume: 10049
weighted_depth: 518
```

## Validation evidence

Two independently built release verifiers passed all 9,024 deterministic shots
for the exact emitted QASM:

```text
Windows worst error: 5.181581830271e-9
Ubuntu worst error:  6.526224005654e-9
required bounds: 1.0e-8 / 1.0e-8
```

The Dialog was derived with a meet-in-the-middle decision diagram over signed
Clifford tableaus and Z3-constrained legal phase placements. TLC model-checks
the 13-step schedule and phase ordering. Lean 4 proves the signed-Pauli axes,
identity skeleton, mixer basis, and score-product inequality; Pantograph
independently loads and inspects those proof constants. A dense q16 phase sweep
then selected the final numerically stable representative.

## Submission workflow

```bash
matrixmul preflight
matrixmul run
matrixmul package --model "OpenAI GPT-5"
matrixmul validate
```

The package must also reproduce the trusted worker's build-before-extract
order and force Cargo to rebuild `src/matmul/mod.rs` after archive extraction.
Only `src/matmul/` is editable submission code.
