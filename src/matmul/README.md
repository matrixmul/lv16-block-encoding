# MatrixMul LV16 17q Ten-CNOT Final-Tail Parity-Network Submission Note

This note documents a 17-qubit `matrixmul-lv16-varq-v3` score-beat candidate. It is packaged by `matrixmul package --model MODEL`; the generated `dist/submission-note.md` prepends `Model: <LLM>` and then includes this file.

## Summary

The candidate is a declared-width **17q same-width MatrixMul oracle** with an
exact parity-network rewrite of the two final ZZ phases. It does not project,
truncate, or pad a 42q baseline circuit. The generated QASM declares:

```text
qubit[17] q;
```

The live leaderboard best before this candidate and the new score are:

```text
current best: 39096.04545219376
candidate:    38939.36001014912
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
matrixmul package --model "OpenAI GPT-5"
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
   - in the final round, synthesize edges 14 and 15 as one exact shared-control
     parity network. Three local CNOT identity pairs stabilize the verifier's
     MPS gauge; reversed edge 14 and forward edge 15 then apply the same
     `Z14*Z15` and `Z15*Z16` phases with a shorter weighted critical path;
   - apply `same_width/x_mixer` blocks (`h; rz; h`) on logical system wires `q < LOGICAL_LEVEL` when `(q + round) % 3 == 0`.
4. **Angle generation:** all `rz` angles use the repository's public `centered_angle` helper and the same domain strings used by `build_same_width_matrixmul_oracle_instructions` in `src/util/verify.rs`.

This keeps the candidate aligned with the mathematical same-width oracle at 17 qubits instead of relying on an external projected reference.

## Optimization workflow

The optimization is a measured exact parity-network rewrite under the published verifier contract:

| Step | Action | Trusted/local result |
|---|---|---:|
| Accepted 17q | serial same-width oracle | `39335.75555394863` |
| Four-CNOT tail rewrite | shorter, but numerically unstable | rejected |
| Six-CNOT parity search | one 9,024-shot miss at `1.038e-8` | rejected |
| Eight-CNOT tail | local 9,024-shot pass at `9.99e-9`; too close to tolerance | rejected |
| Ten-CNOT tail | repeated 9,024-shot passes at `3.59e-11`; identical across three builds | `38939.36001014912` |
| Fourteen-CNOT robust tail | 9,024-shot max error `7.93e-14`; trusted rank-1 submission `sub_998888d72e253c94` | protected fallback at `39096.04545219376` |

The score drops because the final tail no longer serializes both 18-tick ZZ
gadgets on the critical path. Six extra identity-prefix CNOTs provide numerical
gauge resets, while weighted depth falls from 526 for the protected winner to
522. The ideal unitary is unchanged.

## 17q metric shape

The same-width oracle has:

- one initial `h` per declared wire;
- four `same_width/z` rotations per wire;
- 62 ordinary edge gadgets plus an exact ten-CNOT/two-RZ final-tail network;
- the same 22 logical `x_mixer` blocks because `LOGICAL_LEVEL` stays fixed.

Validated metrics:

```text
score: 38939.36001014912
qubits: 17
weighted_gate_volume: 10051
weighted_depth: 522
gates: 349
h: 61
rz: 154
cx: 134
max two-qubit distance: 1
```

## Validation and packaging evidence

The promoted source generates QASM SHA-256
`9c566eb5b78abafbd65ee0a81dc2e917a5b6abcf348277ab0804eb6a0b6cfa6a`.
Three repeated full local trusted runs report identical maxima:

```text
ok: true
evaluated_shots: 9024
score: 38939.36001014912
max_infidelity: 3.592e-11
max_norm_delta: 3.594e-11
```

The same 9,024-shot result also passes verifier binaries rebuilt with
`-Ctarget-cpu=native` and with explicit `+fma,+avx2` target features. This is
the cross-build safety gate tightened after lower-margin local candidates. The
promoted candidate has about 278x local headroom against the `1e-8` tolerance.
The already accepted fourteen-CNOT circuit remains safely ranked if the hosted
worker exposes a platform-specific numerical difference.

### Trusted-worker rebuild compatibility

Official runs 14, 15, and 16 initially failed before evaluating the submitted
circuit. The worker workflow builds the repository baseline first, then
extracts `src/matmul` from the submission archive. Because tar preserved a
source modification time older than that first build, Cargo treated the stale
baseline binary as fresh. The worker log proved this by reproducing baseline
QASM SHA-256 `c2852f142eb3b63954029fcc3163fc4be2ca0ca915e4e9d0464d4741d80d5245`
and score `39335.75555394863`, then reporting a score-metadata mismatch.

The package therefore gives `src/matmul/mod.rs` a future modification time
before archiving. This does not change source or QASM semantics; it forces the
worker's post-extraction `cargo run` to compile the submitted module instead of
reusing the pre-extraction baseline binary. Package promotion additionally
reproduces the exact build-before-extract sequence in a clean worktree. This
method produced trusted rank-1 submission `sub_998888d72e253c94`, so the
worker-rebuild fix is already proven independently of this candidate.

The candidate is submitted only after:

- official preflight passes;
- a 64-shot trusted sanity check passes;
- full `matrixmul run` passes all `9024` trusted shots;
- package validation reports `PACKAGE_OK`, `METRICS_OK`, `FUNCTIONAL_OK`, and `ARCHITECTURE_METADATA_OK`.

Credential discipline: submission should use an API token only in process environment or the contest UI/session, not in source files, shell profiles, git config, docs, or logs.
