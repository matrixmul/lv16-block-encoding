# MatrixMul LV16 Optimization Research Ledger

> **Terminology correction (2026-08-12).** Earlier entries in this historical
> ledger used “Dialog” for Clifford-tableau or Hadamard-word searches. Those
> objects are not Dialog in the sense of Khattar–Shutty–Gidney et al. The
> current candidate uses that name only for its ordered trace of small
> invertible GF(2) transition matrices, with replay, inversion, and implicit
> parity access. Its surrounding Hadamard/RZ mixer remains outside the Dialog.

Last updated: 2026-08-05

## Objective and promotion rule

The accepted `matrixmul-lv16-varq-v3` frontier is `38939.36001014912` at 17
declared qubits (submission `sub_ab9cf9db196c6bea`, rank 1). A candidate is
submitted only if it is strictly lower, passes
all 9,024 local trusted shots, packages successfully, and passes
`matrixmul validate`. Final promotion requires the separate official GitHub
trusted worker. Cheap preflight and partial-shot results are rejection filters,
not submission evidence. Alternate-build reproduction and comfortable numerical
margin are normally required; the eight-CNOT experiment is a documented narrow
probe only because the accepted rank-1 fallback cannot be displaced by failure.

That protected accepted circuit has:

- 17 qubits;
- 349 gates: 61 `h`, 154 `rz`, and 134 `cx`;
- weighted gate volume 10,051;
- weighted depth 522;
- score `38939.36001014912`.

## Structural model

Each round contains a commuting diagonal block followed by sparse X-basis
mixers. The diagonal block consists of single-qubit Z rotations and
nearest-neighbor ZZ phase gadgets:

```text
cx q[i], q[i+1]
rz(theta) q[i+1]
cx q[i], q[i+1]
```

This is a CNOT-phase circuit and admits a phase-polynomial description. The
path edges are two-colorable, so the ZZ gadgets can be emitted in even/odd
layers without changing the mathematical unitary. Repeated Z or ZZ terms can
also be combined across a mixer boundary when the mixer touches none of the
term's support.

## Relevant primary research

- Amy, Azimzadeh, and Mosca, [On the CNOT-complexity of CNOT-PHASE
  circuits](https://arxiv.org/abs/1712.01859), establishes the
  circuit/phase-polynomial correspondence and parity-network synthesis view.
- Cowtan et al., [Phase Gadget Synthesis for Shallow
  Circuits](https://arxiv.org/abs/1906.01734), develops shallow phase-gadget
  compilation using ZX-calculus structure.
- van den Berg and Temme, [Circuit optimization of Hamiltonian simulation by
  simultaneous diagonalization of Pauli
  clusters](https://doi.org/10.22331/q-2020-09-12-322), shows how commuting
  Pauli clusters can reduce CNOT count and depth.
- Bravyi et al., [Clifford Circuit Optimization with Templates and Symbolic
  Pauli Gates](https://doi.org/10.22331/q-2021-11-16-580), motivates local
  template matching and symbolic Pauli frames.
- AlphaTensor-Quantum, [Quantum Circuit Optimization with
  AlphaTensor](https://arxiv.org/abs/2402.14396), applies phase-polynomial
  gadgetization and blockwise circuit search.
- Tang and Lai, [Beyond Sparsity: Quantum Block Encoding for Dense Matrices
  via Hierarchically Low Rank Compression](https://arxiv.org/abs/2602.09745),
  and Zecchi et al., [Block encoding of sparse matrices with a periodic
  diagonal structure](https://arxiv.org/abs/2602.10589), provide broader
  block-encoding context. Their constructions do not directly replace this
  contest's fixed same-width oracle.

## Measured candidate ledger

| Family | Best static score | Trusted evidence | Decision |
|---|---:|---|---|
| Even/odd edge coloring in all rounds | `28561.239556` | 64 shots failed; max infidelity `4.744e-4`, max norm delta `4.747e-4` | Reject |
| Move final edge before the last chain dependency | `38665.845070` | 64 shots failed on one probe; max infidelity `8.067e-8` | Reject |
| Move penultimate final-round edge | `38665.845070` | 64 shots failed on one probe; max error `2.018e-8` | Reject |
| 400 randomized exact final-round edge schedules | `38665.845070` | No 64-shot pass; best sampled max error about `6.786e-6` | Reject |
| Merge round-1/round-2 phase on edge 15 | `39206.316111` | Passed 64 shots at about `7.4e-12`; failed 512 shots with max error about `7.993e-6` | Reject |
| Merge final parity phase while retaining original CNOT pair | `38617.239065` | 64 shots failed on one probe at about `2.018e-8` | Reject |
| Remove smallest `rz(0.000180537515)` | `39210.244796` | 64 shots failed; numerical norm drift reached about `2.754e-4` | Reject |
| Remove final q3 X mixer, the smallest tail mixer | `39206.316111` | 64 shots failed; max infidelity `3.263e-6`, norm remained stable | Reject |
| Pauli frames around near-passing moved edge | `38669.694` and lower variants | All 64-shot variants retained the same roughly `2.018e-8` outlier | Reject |
| Local and remote entangling identity templates | `38677` to `39215` | No 64-shot pass; best retained the same roughly `2.018e-8` outlier | Reject |
| Zero-cost `2*pi` RZ representatives | `38665.845070` | Single and paired gauges did not stabilize the moved-edge schedule; variants that fixed one shot created new failures | Reject |
| Reverse one parity-gadget CNOT orientation | `38665.845070` | A 64-shot pass failed 17 shots by 512; exhaustive single reversals had no 512-shot winner | Reject |
| Four-CNOT exact q14-q16 parity networks | `38665.845070` | All depth-515/516/517 networks failed; only depth-533 networks were stable | Reject |
| Six-CNOT exact q14-q16 parity networks | `38707.219327` best static | Enumerated 1,010 networks. Best full candidate scored `38782.160770` but missed shot 1911 at `1.038e-8` | Reject |
| Eight-CNOT tail with two local identity pairs | `38860.825004` | Two local 9,024-shot passes at `9.994e-9`; old submission `sub_fd3aa315e2a281f8` reproduced stale baseline and never evaluated this circuit | **Controlled official probe; accepted ten-CNOT winner is protected** |
| Ten-CNOT tail with three local identity pairs | `38939.360010` | Repeated 9,024-shot passes at `3.59e-11`, identical in three builds; forced-rebuild submission `sub_ab9cf9db196c6bea` passed the trusted worker | **Accepted rank 1** |
| Fourteen-CNOT tail with five local identity pairs | `39096.045452` | All 9,024 shots at `7.93e-14`, identical in three builds; forced-rebuild submission `sub_998888d72e253c94` ranked first before the ten-CNOT promotion | Accepted fallback/history |

## Eight-CNOT probe construction

Let `a = cx q[14], q[15]`, `A = cx q[15], q[14]`,
`b = cx q[15], q[16]`, and `B = cx q[16], q[15]`. The serial final-round
edge-14/edge-15 suffix is replaced by:

```text
a;
a;
b; b;
A;
rz(theta14) q[14];
b;
rz(theta15) q[16];
A; b;
```

The four-CNOT prefix `a; a; b; b` is the identity. In the remaining network,
the first `b` commutes left across `rz(theta14) q[14]`, yielding the usual
shared-control form `A; b; rz(theta14); rz(theta15); A; b`. Thus
`A;rz(theta14);A` is the same `Z14*Z15` rotation as the original edge-14
gadget, while `b;rz(theta15);b` is the unchanged `Z15*Z16` rotation. The ideal
unitary is unchanged and the two RZ operations retain a shared weighted-depth
layer.

The identity prefix is intentionally retained. This eight-CNOT version passes
locally with only about `6.36e-12` of absolute norm-tolerance headroom. The
accepted ten-CNOT version improves the local maximum to `3.59e-11`. An exhaustive
identity-pair placement screen isolated hard probes 5668, 21, 7537, 4018,
1908, 6147, 732, and 8228, then promoted only candidates that survived the
early prefix, distributed shards, and the complete suite. After the official
worker issue was isolated and fixed with the fourteen- and ten-CNOT
submissions, the exact previously qualified eight-CNOT artifact became a
controlled official probe:

```text
candidate SHA-256:      81b39ad9913935f3236e04d4e55e6983ae5803f4d5134ffb236937f328e91fff
qubits:                 17
gates:                  347
h / rz / cx:            61 / 154 / 132
weighted gate volume:   10049
weighted depth:         520
score:                   38860.82500410921
trusted shots:           9024 / 9024
max infidelity:          9.989e-9
max norm delta:          9.994e-9
```

This is a further `78.53500603991` absolute score reduction, or approximately
`0.2017%`, from the accepted `38939.36001014912` rank-1 frontier.

### Cross-build stability gate

The protected ten-CNOT QASM passed all 9,024 shots with identical reported
maxima in:

1. three repeated default release runs;
2. a release verifier rebuilt with `-Ctarget-cpu=native`;
3. a release verifier rebuilt with `-Ctarget-feature=+fma,+avx2`.

The eight-CNOT artifact has passed two complete default-build runs plus the
native and explicit FMA/AVX2 builds with identical reported maxima. Its
`9.994e-9` maximum is nevertheless only about `0.064%` below the published
`1e-8` boundary. Official submission `sub_ab9cf9db196c6bea` protects rank 1 if
that narrow margin does not transfer to the hosted trusted worker.

## Trusted-worker stale-build finding

Authenticated logs for workflow run 16 showed that the official worker did
not execute the submitted fourteen-CNOT source:

1. the workflow compiled the checked-out accepted baseline;
2. `trusted-worker.mjs validate` then extracted the archive over
   `src/matmul`;
3. archive mtimes were older than the just-built binary, so each later Cargo
   invocation reported the target as fresh;
4. preflight, run, package, and validate all regenerated accepted-baseline
   QASM SHA-256
   `c2852f142eb3b63954029fcc3163fc4be2ca0ca915e4e9d0464d4741d80d5245`;
5. metadata comparison failed with reproduced score `39335.75555394863`
   versus submitted score `39096.04545219376`.

This explains official failures 14 through 16 and falsifies the earlier theory
that those hosted failures measured cross-CPU MPS drift. The submission package
must preserve a `src/matmul/mod.rs` mtime later than the worker's initial build,
forcing Cargo to rebuild after extraction. A clean local reproduction of the
same build-before-extract order is required before resubmission. That method was
then accepted as trusted submission `sub_998888d72e253c94`, proving the package
path independently of the ten-CNOT circuit. A clean accepted-main reproduction
first generated fourteen-CNOT SHA-256 `38fa483214bfd328d192ca1a0a5db45d5083b0e2d41289b570eef0a0930d280d`,
then extracted the ten-CNOT archive, forced compilation, generated SHA-256
`9c566eb5b78abafbd65ee0a81dc2e917a5b6abcf348277ab0804eb6a0b6cfa6a`,
and passed all 9,024 shots. The official worker subsequently accepted the same
package as `sub_ab9cf9db196c6bea` in merge commit
`8e54472a5d247cb6216a1b8fbcdd8f8ee0a7b7b6`.

## Main finding: verifier path sensitivity

The largest mathematical opportunities are real: edge coloring cuts weighted
depth from 533 to 281 without changing the ideal unitary, and legal phase
merges remove rotations and CNOT pairs. The official verifier nevertheless
simulates the mathematical oracle and candidate through separate MPS SVD
paths. Algebraically equivalent gate orders can therefore produce norm or
fidelity drift above the `1e-8` acceptance tolerance even when reported MPS
truncation error is around `1e-24`.

This makes source algebra alone insufficient. A viable submission must also be
numerically aligned with the verifier. In practice, the accepted baseline gets
exact fidelity because its instruction order is identical to the oracle's
instruction order.

## Further optimization directions

1. Treat the official worker as the final gate, and inspect authenticated job
   logs whenever its generic API status hides the actual failing boundary.
2. Retain at least two orders of magnitude of local fidelity and norm headroom
   for any future parity-network candidate. The ten-CNOT circuit retains about
   278x headroom and reproduces under alternate optimized builds.
3. If the contest verifier is revised, canonicalize and normalize MPS states
   before comparison or add an exact 17-qubit statevector audit for
   algebraically equivalent candidates. That would make the large phase-gadget
   depth reductions rankable on their mathematical merits.
4. Do not stack or submit any rejected family above without a new 9,024-shot
   result. Several candidates looked excellent at preflight or 64 shots and
   failed at 512.

## Credential discipline

No API key is stored in source, documentation, Git configuration, shell
profiles, or CLI configuration. Authentication is deferred until a strict
9,024-shot winner passes package validation.
