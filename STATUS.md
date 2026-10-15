# Current status and development priorities

Reviewed: 2026-10-13. Updated: 2026-10-15 for the circuit soundness and
proving work in [#2](https://github.com/fabohax/zkPoH/issues/2),
[#3](https://github.com/fabohax/zkPoH/issues/3),
[#4](https://github.com/fabohax/zkPoH/issues/4), and
[#6](https://github.com/fabohax/zkPoH/issues/6).

zkPoH is an experimental Rust/Noir prototype for private Bitcoin UTXO
membership and balance-threshold checks. It does not yet implement the complete
statement: **“I control distinct UTXOs in an accepted Bitcoin snapshot whose
combined value is at least the required threshold,”** with independently
verifiable zero-knowledge proof artifacts.

## Implemented today

- Rust library and CLI for loading snapshots and generating Noir witness inputs.
- A depth-16 Blake2s Merkle tree (up to 65,536 leaves) built sparsely, with
  zero-value padding at distinct indices. Snapshots with a repeated outpoint
  are rejected.
- Noir checks for Merkle membership, a public `threshold_sats`, distinct
  indices, distinct outpoints among counted slots, indices below
  `2^MERKLE_DEPTH`, and per-value bounds that keep the sum from wrapping.
- Regtest wallet UTXO selection and snapshot generation.
- Off-circuit P2WPKH ownership signing with WIF keys, and signature verification.
- Proof generation with Barretenberg (`prove`) and verification (`verify`)
  that pins the verification key and checks the public inputs against the
  snapshot before `bb verify`.
- CI running `cargo run -- test-all` with Nargo 1.0.0-rc.4 and bb 5.0.0.

`test-all` runs 11 Rust tests and 10 Noir tests, then generates a proof,
verifies it, and checks that it is rejected for a different snapshot and for an
untrusted verification key. Passing these tests does not establish the stronger
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

### 2. Duplicate UTXOs can inflate the total — done

Resolved by #2 and #3. The circuit requires pairwise-distinct Merkle indices,
indices below `2^MERKLE_DEPTH` (so an index cannot alias a lower one), and
pairwise-distinct `(txid_tag, vout)` among counted slots. Zero-value padding
slots are exempt from the outpoint check because they add nothing to the total.
The snapshot builder rejects a repeated `(txid, vout)` before building the tree.

Regression tests: `duplicate_utxo_fails`, `index_out_of_range_fails`,
`duplicate_outpoint_at_distinct_indices_fails` (four copies of one 0.3 BTC
output at indices 0..3) and `zero_value_padding_slots_may_share_an_outpoint` in
`src/main.nr`; `duplicate_outpoints_in_snapshot_rejected` in
`src/snapshot_builder.rs`.

### 3. Portable proof generation and independent verification — done

Resolved by #2 and #4. `prove` runs `nargo compile`, `nargo execute`, and
`bb prove --write_vk`, leaving `target/proof`, `target/public_inputs`, and
`target/vk`. `verify --snapshot <file>` needs no private witness: it checks
`target/vk` against the pinned `trusted_vk.sha256`, decodes the public inputs
(33 big-endian 32-byte fields: root bytes in fields 0–31, `threshold_sats` in
field 32) and compares them to the snapshot's recomputed root and threshold,
then runs `bb verify`. `demo` / `check-circuit` / `prove` / `verify` are now
distinct operations.

Still open: the verifier recomputes the root from the snapshot file it is given.
How a verifier accepts a snapshot root in the first place is gap 4.

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

### 6. Threshold configuration is inconsistent — done

Resolved by #2. `threshold_sats` is a public circuit input taken from the
snapshot; `verify` rejects a proof whose public threshold differs from the
snapshot's. Binding the threshold to ownership remains part of gap 1.

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

1. ~~Fix duplicate counting~~ (done) and incomplete off-circuit verification
   (gap 5), with targeted regression tests.
2. ~~Resolve threshold semantics~~ (done) and define canonical snapshot/challenge
   formats.
3. ~~Add cryptographic proof generation and independent verification.~~ (done)
4. Implement script-bound ownership inside ZK and test the complete statement.
5. Scale snapshot construction and establish verifier snapshot trust.
6. Build the remote conventional baseline, then the attested proving architecture.

## Later extensions

- Taproot/Schnorr support and broader script coverage.
- Utreexo integration.
- Application-specific nullifiers or reuse controls where required.
- Performance measurements, privacy tests, and operational hardening.

These extensions do not replace the correctness and ownership requirements above.
