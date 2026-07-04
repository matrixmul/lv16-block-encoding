# Matrix Multiplication via Block-Encoding Quantum Circuit

_Can quantum circuits provide more efficient matrix multiplication than GPUs?_

This quantum circuit optimization contest is to build 65536 x 65536 matrix multiplication (equivalent GPU's 563 TFLOPs and 192 GB VRAM) quantum circuits using block-encoding matrix multiplication. Can you and your AI agents find ways to optimize beyond the same-width baseline quantum circuit?

## AI Agent Quick Start

If you are using an AI coding agent, paste this prompt into the agent:

```text
You are working in the MatrixMul LV16 block-encoding contest repo.

First, read README.md, benchmark.json, docs/submission-package-v1.md,
src/matmul/mod.rs, src/matmul/architecture.mmd, and src/matmul/README.md.

Use the local CLI help before changing code:

  node matrixmul.js --help
  node matrixmul.js preflight --help
  node matrixmul.js package --help

Only edit files under src/matmul/. Do not rely on changes to benchmark.json,
matrixmul.js, challenges/target_16q.json, src/util/*, Cargo.toml, Cargo.lock,
score.json, or generated dist/* artifacts; those are infrastructure or local
outputs and are not accepted as contender edits.

Optimize src/matmul/mod.rs so it generates a valid OpenQASM 3.0 circuit with
an allowed declared width from 17 through 42, supported gates only, and lower
score while matching the official same-width trusted probe set. Do not claim a
lower width unless a real same-width reference implementation is registered in
the target metadata. Keep src/matmul/architecture.mmd
updated with the required Algorithm and Optimization branches, and keep
src/matmul/README.md updated with the submitted strategy and evidence.

For candidate changes, run:

  node matrixmul.js preflight
  node matrixmul.js run
  node matrixmul.js package --model "model-name"
  node matrixmul.js validate
```

## Challenge

Level 16 represents a 2^16, 65,536-dimensional Laplacian-style sparse matrix target.
If treated as a dense operator, the target spans 65,536 x 65,536 entries: more
than 4.29 billion matrix cells. The contest does not ask you to multiply that
matrix directly. It asks you to compile the target into a compact unitary circuit
whose observable behavior matches the generated reference on the contest's
deterministic product-state probe set.

The useful output of the contest is a block-encoding primitive: a circuit `U_A`
whose projected behavior represents this large structured matrix `A`, up to the
normalization and ancilla convention of the construction. In that sense the
submitted circuit is not a dense matrix-multiplication kernel; it is a reusable
quantum representation of one 65,536-dimensional matrix operator.

True products build on that primitive. Composing `U_A` with itself gives the
natural local experiment for `A^2`, while a general `AB` product also needs a
compatible block encoding `U_B` for the second matrix `B`. Thus `A^2` can rely
only on a successful result from this contest, but `AB` relies on this contest's
result plus a second block-encoded operand with matching system and ancilla
conventions.

The live target is fixed by `challenges/target_16q.json`:

- 32 two-rail matrix-ladder qubits encode the Level-16 sparse structure.
- 10 workspace qubits support the block-encoding construction.
- 4 deterministic rounds generate phase and ladder-coupling terms.
- 9,024 deterministic product-state shots form the trusted acceptance gate.

The declared-width range is 17 through 42 qubits: 16 system qubits plus at
least one workspace/control wire, up to the full 42-qubit block-encoding
construction. No compression or prefix truncation is accepted as a validation
shortcut. A candidate must match an official reference implementation for the
same declared width on the trusted shots and stay inside the cost guards for
size, gate count, two-qubit distance, weighted volume, and weighted depth. The
checked-in target metadata currently registers the full 42-qubit reference; any
lower-width track entry must add a real same-width reference rather than reusing
the 42-qubit target with terms skipped. The accepted gate set is deliberately
narrow: `h`, `x`, `y`,
`z`, `rz`, `cx`, and `cnot`; `barrier` is allowed as a non-operational
annotation. Measurement, reset, classical control, loops, and dynamic constructs
are rejected.

The current baseline is intentionally plain: it emits the generated full-width
reference ladder at 42 qubits, with 948 gates, 414 `rz` rotations, 448 `cx`
gates, weighted gate volume 28,470, weighted depth 1,595, and score
283,024.0671745073. Lower scores rank higher. Your job is to keep the
same-width trusted-probe behavior while making the circuit cheaper.

## Contest Layout

```text
.github/workflows/validate.yml
benchmark.json
matrixmul.js
challenges/target_16q.json
Cargo.toml
docs/submission-package-v1.md
src/lib.rs
src/matmul/mod.rs
src/matmul/architecture.mmd
src/matmul/README.md
src/util/generate_target.rs
src/util/generate_baseline.rs
src/util/generate_solution.rs
src/util/verify.rs
tools/trusted-worker.mjs
```

The target is deterministic. Re-running the generators should not change the
checked-in metadata or baseline circuit.

## Editable Submission Boundary

Contenders may edit only files under `src/matmul/`. The local package and the
trusted worker enforce this exact boundary from `benchmark.json`:

```json
"editablePaths": ["src/matmul"]
```

The current editable submission files are:

- `src/matmul/mod.rs` - circuit-generation source for the submitted QASM.
- `src/matmul/architecture.mmd` - required Mermaid diagram for the leaderboard
  modal preview.
- `src/matmul/README.md` - required submission note source packaged by
  `node matrixmul.js package --model MODEL`.

You may add regular files or subdirectories inside `src/matmul/` if your
implementation needs them. Do not rely on changes outside `src/matmul/`: files
such as `benchmark.json`, `matrixmul.js`, `challenges/target_16q.json`,
`src/util/*`, `Cargo.toml`, `Cargo.lock`, `score.json`, and generated `dist/*`
artifacts are infrastructure, verifier inputs, or local outputs, not accepted
contender edits. The package archive contains only `src/matmul/`, and the
trusted worker commits only that editable path after validation.

## Quick Start

```powershell
cargo run --release --bin generate-target -- --check
cargo run --release --bin generate-solution -- --output dist/solution.qasm
node .\matrixmul.js preflight
```

Optimize the source code under `src/matmul`. It must generate an OpenQASM 3.0
circuit that declares an allowed width from `qubit[17] q;` through
`qubit[42] q;` and matches the official same-width trusted probe set.
Keep
`src/matmul/architecture.mmd` updated with a Mermaid diagram whose root is
`Target circuit: MatrixMul LV16` and whose two top-level explanation branches
are `Algorithm` and `Optimization`. Keep `src/matmul/README.md` as the memory
note packaged with the submission. The checked-in implementation is the current
baseline; there is no separate `submissions` folder.

`dist/solution.qasm` is a generated local artifact used for verification and
metadata hashing. The verifier compares it against a canonical reference circuit
registered for the candidate's declared width on the trusted probe set generated
from the non-editable target metadata and generator code. If no official
same-width reference exists, the candidate is rejected instead of being checked
against a truncated wider target.

For a score candidate:

```powershell
node .\matrixmul.js run
node .\matrixmul.js package --model "model-name"
node .\matrixmul.js validate
```

## Scoring

Scoring follows the sibling contest shape, adapted for this gate set. Lower is
better:

```text
score = qubits * sqrt(weighted_gate_volume * weighted_depth)
```

For the strict v1 gate set, the volume term is composed directly from the
generated circuit:

```text
native_1q = count(h) + count(x) + count(y) + count(z)
rz_synthesis = 64 * count(rz)
cx_like = count(cx) + count(cnot)
extra_distance = sum(max(abs(control - target) - 1, 0)) over cx/cnot gates
routing = 6 * extra_distance

weighted_gate_volume = native_1q + rz_synthesis + cx_like + routing
```

Weighted depth schedules the same costs on the touched qubit span of a line
topology:

```text
h/x/y/z duration = 1
rz duration = 16
cx/cnot duration = 1 + 6 * max(abs(control - target) - 1, 0)
```

The extra factors beyond `qubits * sqrt(count * depth)` are needed because this
circuit uses arbitrary `rz` angles and distance-2 rail couplings. The verifier
reports the allowed-gate counts, weighted volume terms, weighted depth model,
maximum two-qubit distance, and per-shot fidelity against the reference circuit.

## Useful Optimization Primitives

The submitted circuit must stay inside the allowed gate set (`h`, `x`, `y`,
`z`, `rz`, `cx`, `cnot`, plus non-operational `barrier`), but contenders do not
have to preserve the baseline lowering. Useful primitives to investigate:

- **RZ accumulation and cancellation.** Consecutive `rz(a)` and `rz(b)` gates
  on the same qubit can be folded into `rz(a + b)` when no intervening
  non-commuting operation touches that qubit. This is high leverage because
  each `rz` costs 64 weighted-volume units and 16 weighted-depth units.
- **Diagonal-layer commuting.** Baseline `z_phase` and `zz_phase` terms are
  diagonal in the computational basis, so they commute with each other. Within a
  diagonal region, reorder terms to group work by qubit, share parity
  computations, or expose parallelism.
- **Parity-phase gadgets.** The baseline pattern
  `cx control,target; rz(theta) target; cx control,target` implements a
  two-qubit parity phase. Multiple parity phases that share edges, controls, or
  targets may be cheaper if their parities are computed once, reused, then
  uncomputed.
- **Basis-frame tracking.** The baseline `x_mixer` term is `h; rz(theta); h`,
  equivalent to applying a rotation in the X basis. Track whether a qubit is in
  the Z or X frame so adjacent `h` gates can cancel and same-basis rotations can
  be combined.
- **CNOT direction and rail layout.** `cx` and `cnot` score the same, but the
  touched qubit span affects routing cost and weighted depth. Reversing a CNOT
  with local Hadamards, remapping logical rails, or changing the parity target
  can reduce distance penalties if the final unitary still matches validation.
- **Nearest-neighbor scheduling.** Independent single-qubit gates and disjoint
  `cx` pairs can be scheduled in parallel. Avoid serializing long runs of
  unrelated rotations, and prefer layouts that keep two-qubit distance at 1.
- **Ancilla lifetime shortening.** The baseline declares 42 qubits, including
  10 block-encoding workspace qubits. A lower declared width is rankable only
  when a real same-width reference implementation is registered; do not treat
  skipped higher-width terms as a valid lower-width implementation.
- **Finite-probe equivalence checks.** The trusted gate is the fixed 9024-shot
  product-state probe set, not symbolic equality over all states. Prefer
  algebraic equivalence where possible, but always confirm candidate shortcuts
  with `preflight`, `smoke`, smaller `--shot-count` runs, and finally
  `node matrixmul.js run`.

## Trusted Validation

Final validation follows the 9024-shot convention used by the sibling ECDLP
contest repos. Each shot is a deterministic same-width product-state probe
derived from `target_id`, the declared width, the shot index, the qubit index,
and the target metadata domain separator. This keeps validation reproducible
while avoiding a hand-written list of thousands of probes.

Run a full trusted validation with:

```powershell
node .\matrixmul.js run
```

For local iteration, use cheaper gates first:

```powershell
cargo run --release --bin verify -- my_submission.qasm --preflight
cargo run --release --bin verify -- my_submission.qasm --smoke
cargo run --release --bin verify -- my_submission.qasm --shot-count 64
```

GitHub Actions shards the full 9024-shot baseline check across 16 jobs using
`--shot-shard INDEX/16`.

## Validation Contract

Generated circuits must:

- Be UTF-8 OpenQASM 3.0 files.
- Include `src/matmul/architecture.mmd` with Mermaid anchors for the target
  circuit, `Algorithm`, and `Optimization`. The target root must branch to the
  two explanation nodes. Use `Algorithm` for circuit structure and `Optimization`
  for score tradeoffs, search choices, and simplifications.
- Declare a supported width from `qubit[17] q;` through `qubit[42] q;`.
  Width 17 is the lower bound because the 16-qubit system register needs at
  least one workspace/control wire. A width is valid only when the target
  metadata registers an official same-width reference implementation.
- Use only supported unitary gates: `h`, `x`, `y`, `z`, `rz`, `cx`, and `cnot`.
  `barrier` directives are allowed as non-operational annotations and ignored by
  the verifier.
- Avoid measurement, reset, classical control, loops, and dynamic constructs.
- Stay within the cost guard limits in `challenges/target_16q.json`. Preflight
  rejects before trusted simulation if raw size, gate count, two-qubit distance,
  routing cost, weighted gate volume, or weighted depth exceeds those caps.
- Match the reference on all 9024 deterministic product-state shots within the
  configured fidelity, norm, and MPS truncation tolerances.

The 9024-shot path is the trusted acceptance gate and ranking contract. It is a
finite deterministic probe contract, not a symbolic proof of full operator
equivalence on every possible input state. `--preflight`, `--smoke`, and small
`--shot-count` runs are cheap reject paths for development, not final score
evidence.

## Submission Workflow

`benchmark.json` is the contest contract. It fixes the editable path,
artifact path, score model, required validation gate, and 9024-shot trusted
count. The Node CLI mirrors the sibling contest convention:

```powershell
node .\matrixmul.js setup
node .\matrixmul.js preflight
node .\matrixmul.js run
node .\matrixmul.js package --model "model-name"
node .\matrixmul.js validate
```

To submit to the public leaderboard, get an API key from
<https://matrixmul.com>:

1. Open <https://matrixmul.com> in a browser.
2. Sign in with your contest account.
3. Open the account page and create or copy a contest API key.
4. Save it locally for this CLI:

   ```powershell
   node .\matrixmul.js login "your-api-key"
   node .\matrixmul.js config
   ```

`login` verifies the key against `https://matrixmul.com/api/me` and stores it in
the local MatrixMul CLI config file with user-only permissions. For a temporary
or CI-style credential, skip `login` and set `MATRIXMUL_API_KEY` or
`MATRIXMUL_API_TOKEN` in the process environment instead.

After `validate` passes, upload the package:

```powershell
node .\matrixmul.js submit --watch
```

Before uploading, `submit` validates the local package again and fetches the
current track leaderboard. It rejects locally unless the validated score is
strictly lower than the current best ranked, non-deleted score for
`matrixmul-lv16-varq-v3`; if no ranked submissions exist yet, the first valid
submission can establish the frontier.
Use `--source-url URL` when you want the submission record to point at a public
branch, commit, or pull request. Use `--api URL` only for a non-production
contest server; the default API is `https://matrixmul.com`.

If you already have a submission id, poll it or inspect server-side validation
logs directly:

```powershell
node .\matrixmul.js status SUBMISSION_ID --watch --poll-interval 10
node .\matrixmul.js logs SUBMISSION_ID
node .\matrixmul.js leaderboard
```

The package command writes `dist/submission.tar.gz`,
`dist/submission-note.md`, and `dist/submission-metadata.json`. The note file
always starts with `Model: <LLM>` from `--model`; use the rest of the note to
explain the submitted strategy, tradeoffs, and evidence. The metadata
binds the trusted score to generated `dist/solution.qasm` with a SHA-256
digest, binds the modal preview to `src/matmul/architecture.mmd`, and is rejected
unless `validation.shots` is exactly 9024.

The contest server dispatches `.github/workflows/trusted-worker.yml` for each
accepted upload. That workflow downloads the submitted archive with a scoped
one-submission claim, reruns the official verifier, checks the package contract,
commits only `benchmark.json` `editablePaths` to the contest repository main
branch, and calls the server back with pass/fail plus the accepted commit SHA.
After the trusted worker passes, the server can rank the submission and arrange
the official merge with the contestant credited as co-author.

## Related Works

The contest is a gate-level optimization problem, not a literature benchmark,
but these papers are useful context for block-encoding matrix circuits and
matrix arithmetic:

- [Hamiltonian Simulation by Qubitization](https://arxiv.org/abs/1610.06546),
  Low and Chuang. Introduces qubitization, the block-encoding-style projection
  of an operator into a larger unitary with low ancilla overhead.
- [Quantum singular value transformation and beyond: exponential improvements
  for quantum matrix arithmetics](https://arxiv.org/abs/1806.01838), Gilyen,
  Su, Low, and Wiebe. Foundational QSVT framework for transforming
  block-encoded matrices and composing quantum linear-algebra algorithms.
- [Hamiltonian singular value transformation and inverse block
  encoding](https://arxiv.org/abs/2104.01410), Lloyd et al. Develops
  Hamiltonian block-encoding ideas and discusses matrix multiplication from
  inverse block encoding.
- [Explicit Quantum Circuits for Block Encodings of Certain Sparse
  Matrices](https://arxiv.org/abs/2203.10236), Camps, Lin, Van Beeumen, and
  Yang. Gives explicit circuit constructions for structured sparse matrix
  block encodings, closest in spirit to this contest's circuit-level focus.
- [Block-encoding dense and full-rank kernels using hierarchical matrices:
  applications in quantum numerical linear
  algebra](https://arxiv.org/abs/2201.11329), Nguyen, Kiani, and Lloyd. Shows
  how hierarchical matrix structure can make dense kernel matrices amenable to
  block encoding.
- [Block Encoding of Sparse Matrices via Coherent
  Permutation](https://arxiv.org/abs/2508.21667), Setty. Targets explicit
  gate-level sparse-matrix block encodings with attention to control overhead,
  amplitude reordering, and connectivity.
- [Products between block-encodings](https://arxiv.org/abs/2509.15779), Dong,
  Li, and Xue. Focuses directly on matrix-matrix, Kronecker, and Hadamard
  products between block-encoded matrices with reduced ancilla use.
- [Block encoding of sparse matrices with a periodic diagonal
  structure](https://arxiv.org/abs/2602.10589), Zecchi et al. Gives explicit
  LCU-based circuits for sparse matrices with periodic diagonal structure.
- [Beyond Sparsity: Quantum Block Encoding for Dense Matrices via
  Hierarchically Low Rank Compression](https://arxiv.org/abs/2602.09745), Tang
  and Lai. Explores structured dense matrices through sparse lifts and direct
  recursive block-encoding constructions.
- [Unitaria: Quantum Linear Algebra via Block
  Encodings](https://arxiv.org/abs/2605.10768), Deiml et al. Presents a
  software interface for composing block encodings through operations including
  matrix multiplication and extracting circuits/resource estimates.


## Credits

This contest was inspired by [ECDSA.fail](https://ecdsa.fail) quantum circuit optimization and Prof. Tom Yeh's [AI by Hand](https://www.byhand.ai/) lecture series.
