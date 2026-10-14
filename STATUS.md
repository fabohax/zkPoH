# Current status and development priorities

Reviewed: 2026-10-13.

zkPoH is an experimental Rust/Noir prototype for private Bitcoin UTXO
membership and balance-threshold checks. It does not yet implement the complete
statement: **“I control distinct UTXOs in an accepted Bitcoin snapshot whose
combined value is at least the required threshold,”** with independently
verifiable zero-knowledge proof artifacts.

## Implemented today

- Rust library and CLI for loading snapshots and generating Noir witness inputs.
- A fixed four-leaf Blake2s Merkle tree, with zero-value padding.
- Noir checks for Merkle membership and a hardcoded 100,000,000-sat threshold.
- Regtest wallet UTXO selection and snapshot generation.
- Off-circuit P2WPKH ownership signing with WIF keys, and signature verification.
- Local circuit execution through `nargo execute`.

The review ran `cargo test` and `nargo test`: all four Rust tests and all three
Noir tests passed. The Noir tests cover valid input, insufficient balance, and
an invalid Merkle path. Passing these tests does not establish the stronger
ownership-bound statement above.

## Gaps and required development

### 1. Ownership is not constrained inside the ZK circuit

The active circuit in [`src/main.nr`](src/main.nr) checks membership and values,
but receives no public keys, signatures, or output scripts. A prover can select
UTXOs without demonstrating control of them. The separate ownership JSON exposes
the selected outputs and public keys to its verifier.

Required work:

- Commit to the actual output script in each snapshot leaf.
- Verify that a private key or signature controls that committed output inside
  the circuit, initially for a clearly defined script type.
- Bind ownership to the public snapshot root, threshold, verifier challenge,
  expiry, and context.
- Keep selected outputs, keys, signatures, values, and paths private.

Acceptance: wrong keys, wrong signatures, substituted scripts, and mismatched
challenges fail; a valid ownership-bound proof reveals only its defined public
inputs.

### 2. Duplicate UTXOs can inflate the total

The circuit verifies each slot independently and then sums all four values.
It does not require distinct indices or outpoints. From code inspection, the
same valid 0.3 BTC leaf and path could be supplied four times to satisfy the
1 BTC threshold. This finding has not yet been reproduced in a dedicated test.
The normal witness builder uses indices `[0, 1, 2, 3]`, but a prover can construct
a witness independently of that builder.

Required work:

- Enforce uniqueness of counted outputs inside the circuit.
- Reject duplicate outpoints during snapshot construction as well; distinct
  indices alone cannot prevent a malformed snapshot containing duplicate entries.
- Define how unused zero-value slots participate in uniqueness checks.

Acceptance: repeated selection of one output fails, duplicate snapshot entries
fail, and valid padded selections still pass.

### 3. Portable proof generation and independent verification are missing

[`src/prover.rs`](src/prover.rs) invokes `nargo execute`, which solves the witness;
the CLI does not produce a portable cryptographic proof. The `verify` command
calls `nargo check` through [`src/verifier.rs`](src/verifier.rs), which checks the
circuit rather than verifying a supplied proof.

Required work:

- Integrate a proof backend compatible with the chosen Noir version.
- Generate proof artifacts and define their public-input and verification-key
  formats.
- Implement a separate verifier that accepts the proof and expected public
  statement without requiring the private witness.
- Clarify command names and documentation so circuit execution, compilation,
  proof generation, and proof verification are distinct operations.

Acceptance: a proof verifies in a separate process; mutations to the proof or
public inputs fail verification.

### 4. Snapshot commitments are a small prototype

[`src/snapshot_builder.rs`](src/snapshot_builder.rs) limits the entire snapshot
to four entries, uses only the final eight bytes of each transaction ID, and
hashes `(txid_tag, vout, value)` without an output script/address. The root is
computed from the supplied snapshot. The current regtest helper selects wallet
outputs rather than building a full chain UTXO snapshot.

Required work:

- Specify canonical leaf encodings using full transaction IDs, output indices,
  values, and scripts.
- Separate snapshot construction from private selection and path generation.
- Support larger snapshots and paths while bounding selected outputs separately.
- Bind snapshots to a Bitcoin network and exact block/epoch.
- Define how a verifier accepts or independently reproduces a snapshot root.

Acceptance: independent builders produce the same root for the same defined
snapshot; changing any committed output field invalidates membership; an
unaccepted root is rejected by the verifier.

### 5. Off-circuit ownership verification is incomplete

[`verify_ownership_proof`](src/ownership.rs) checks the supplied challenge digest,
public-key/address correspondence, and individual ECDSA signatures. It does not
reconstruct the canonical challenge from the envelope fields, reconstruct the
snapshot root, enforce the total threshold, or validate freshness. An empty
signature list currently passes because the verification loop executes zero
times.

Required work:

- Reject empty and duplicate output/signature sets.
- Reconstruct the challenge from a canonical encoding and compare it with the
  supplied challenge.
- Bind every signed output and envelope field to the accepted snapshot and
  expected threshold; validate the total with checked arithmetic.
- Accept verifier expectations for the nonce, expiry, and context, and define
  replay handling.
- Preserve explicit disclosure that this workflow exposes selected outputs.

Acceptance: empty proofs, altered envelope fields, insufficient totals,
duplicate outputs, stale challenges, and invalid signatures fail.

### 6. Threshold configuration is inconsistent

Snapshot JSON and the regtest CLI expose `threshold_sats`, but the active Noir
circuit always enforces 100,000,000 sats. The witness builder does not use the
snapshot threshold to configure that circuit.

Required work: either reject unsupported thresholds consistently or make the
threshold a public circuit input bound to ownership and verifier expectations.

Acceptance: all components agree on the exact threshold; a proof for a lower
threshold cannot satisfy a verifier requesting a higher one.

### 7. The remote attested prover remains a specification

[`specs/zkPoH_Verifiable_Attested_Remote_Prover_Spec_v0.2.md`](specs/zkPoH_Verifiable_Attested_Remote_Prover_Spec_v0.2.md)
describes a broader architecture. Its protocol schemas, server API/workers,
signed manifests and receipts, encrypted attested execution, ZK attestation
adapters, policy registry, and integration vectors are not implemented here.

Required work: implement the specification in phases after the local
ownership-bound proof and independent verifier work. Keep the conventional
trusted service and attested confidential service claims explicit.

Acceptance: first demonstrate a remote regtest proof verified independently;
then demonstrate encrypted requests and locally verifiable attestation under
a defined policy, with the specification's negative tests.

## Suggested development order

1. Fix duplicate counting and incomplete off-circuit verification, with targeted
   regression tests.
2. Resolve threshold semantics and define canonical snapshot/challenge formats.
3. Add cryptographic proof generation and independent verification.
4. Implement script-bound ownership inside ZK and test the complete statement.
5. Scale snapshot construction and establish verifier snapshot trust.
6. Build the remote conventional baseline, then the attested proving architecture.

## Later extensions

- Taproot/Schnorr support and broader script coverage.
- Utreexo integration.
- Application-specific nullifiers or reuse controls where required.
- Performance measurements, privacy tests, and operational hardening.

These extensions do not replace the correctness and ownership requirements above.
