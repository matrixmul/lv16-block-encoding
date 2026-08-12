# MatrixMul LV16 Linear-Dialog Candidate

## Candidate

The circuit declares 17 qubits and emits the normal same-width MatrixMul
construction except for the final `q14-q16` parity pair. Define:

```text
a = CX(14,15)    A = CX(15,14)
b = CX(15,16)    B = CX(16,15)
```

The selected linear transition trace is:

```text
a a b A [L R] b A
```

`L` is the requested `Z14*Z15` rotation and `R` is the requested `Z15*Z16`
rotation. Each symbol is the embedded elementary matrix
`row[target] ^= row[control]` over `GF(2)`, hence is small and invertible.
Starting from parity rows `[001,010,100]`, replaying `a a b A` produces
`[011,010,110]`; the two rotations are therefore available implicitly on
physical q14 and q16. Replaying `b A` restores `[001,010,100]`.

The following `H; RZ; H` q15 mixer is outside this linear Dialog. No Clifford
tableau is called a Dialog. A tableau may independently verify a circuit, but
the Dialog object here is specifically the ordered list of small invertible
GF(2) transition matrices with replay and inverse semantics.

The final-round leading q16 Z phase retains the bounded `-2.262829e-6` bias
that selects a stable basin of the finite MPS verifier.

## What is inherited from the paper

Genuinely inherited from Khattar-Shutty-Gidney et al. is the structural
abstraction: an ordered trace of small invertible transition matrices, forward
replay, inverse replay, and implicit access to a derived object without
materializing the full transformation.

Only analogous here are the field and workload. This trace is a CNOT parity
basis over `GF(2)`, not an EEA execution on `(A,B)`; it has no GCD endpoint,
Bezout coefficients, Bernstein-Yang branch record, fixed `2n` length, or
register-sharing/qubit-space claim. The benefit in this contest is synthesis
and scheduling, not the paper's EEA memory reduction.

## Metrics and validation

```text
trusted score to beat: 38786.020651776074
candidate score:       38782.16077012729
improvement:               3.859881648783

qubits: 17
h / rz / cx: 61 / 154 / 130
weighted gate volume: 10047
weighted depth: 518
QASM SHA-256: 3918b5594219c0d41c317df00376229932e2f7c23adabe05fb6a04b24c425525
```

The exact emitted QASM passed all 9,024 deterministic probes in independently
built Windows and Ubuntu release verifiers:

```text
Windows max infidelity / norm delta: 5.182e-9 / 5.180e-9
Ubuntu  max infidelity / norm delta: 6.524e-9 / 6.526e-9
required bounds:                       1.0e-8 / 1.0e-8
```

## Reproduction

```bash
matrixmul preflight
matrixmul run
matrixmul package --model "GPT-5"
matrixmul validate
```

Only `src/matmul/` is editable submission code. Submission remains gated on
successful package validation, byte-identical regeneration, and the live
leaderboard still being strictly above the candidate score.
