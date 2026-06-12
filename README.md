# zkPoH: Zero Knowledge Proof-of-Hodl

A proof-of-concept demonstrating how to use **Noir** to prove that selected Bitcoin UTXOs have a combined value of at least **1 BTC**, without revealing which UTXOs they are.

## Overview

The prover generates a proof for the following statement:

> I know a set of Bitcoin UTXOs belonging to a committed Bitcoin snapshot whose combined value is at least 100,000,000 sats.

The verifier learns only that the statement is true.

The proof does not reveal:

* UTXO identifiers,
* Bitcoin addresses,
* exact balances,
* transaction history,
* private keys.

## Status

⚠️ Experimental educational project.

This repository prioritizes simplicity and portability over production readiness.

Current implementation status:

* Rust can load the sample snapshot and generate `Prover.toml`.
* Noir verifies a fixed two-UTXO, one-level Merkle proof.
* Noir enforces `sum(values) >= 100_000_000`.
* Tests cover valid input, below-threshold input, and a wrong Merkle path.
* The prototype Merkle tree uses Blake2s over fixed byte encodings.
* Bitcoin ownership is still assumed, not proven.

## Architecture

```
Bitcoin Snapshot
       │
       ▼
Build Merkle Tree
       │
       ▼
Publish Merkle Root
       │
       ▼
Prover selects owned UTXOs
       │
       ▼
Generate Merkle inclusion proofs
       │
       ▼
Noir Circuit
 ├─ Verify membership
 ├─ Sum UTXO values
 └─ Assert total ≥ 1 BTC
       │
       ▼
Generate Proof
       │
       ▼
Verifier checks proof
```

## Statement

Public statement:

```
∃ utxos :
    valid_membership(utxos)
∧   sum(values) ≥ 100_000_000
```

Private witness:

* UTXO entries,
* Merkle paths,
* UTXO values.

Public inputs:

* Merkle root,
* proof.

## Repository Structure

```
zk-proof-of-hodl/
├── circuits/
│   ├── merkle.nr
│   ├── threshold.nr
│   └── main.nr
├── snapshots/
│   └── utxo_snapshot.json
├── src/
│   ├── snapshot_builder.rs
│   ├── prover.rs
│   └── verifier.rs
├── examples/
├── scripts/
├── Nargo.toml
└── README.md
```

## Circuit Design

### Membership Verification

Each provided UTXO must belong to the committed Bitcoin snapshot.

Inputs:

```
leaf
merkle_path
merkle_index
merkle_root
```

Constraint:

```
blake2s(left_digest || right_digest) == merkle_root
```

### Threshold Verification

The circuit aggregates the values of all provided UTXOs.

Constraint:

```
Σ(value_i) ≥ 100_000_000
```

If the condition is not satisfied, proof generation fails.

## Assumptions

Current prototype assumptions:

* Bitcoin snapshot is generated off-chain.
* Snapshot is trusted.
* UTXO ownership is assumed by the prover.
* No nullifiers are implemented.
* Proofs represent ownership only at snapshot time.

Future versions may include:

* Schnorr ownership verification,
* Utreexo commitments,
* snapshot epochs,
* nullifiers,
* arbitrary thresholds.

## Requirements

* Noir
* Nargo
* Rust
* Bitcoin Core (optional for regtest experiments)

## Development

Clone the repository:

```bash
git clone https://github.com/fabohax/zkPoH.git
cd zkPoH
```

Build the Noir circuit:

```bash
nargo check
```

Generate witness inputs from the sample snapshot:

```bash
cargo run -- build-witness
```

Execute the circuit constraints with example inputs:

```bash
nargo execute
```

Run the Rust and Noir tests:

```bash
cargo test
nargo test
```

Run the prototype end-to-end constraint check:

```bash
cargo run -- prove
```

## Example

Given the sample private UTXOs:

```
0.42 BTC
0.58 BTC
```

The circuit computes:

```
0.42 + 0.58 = 1.00 BTC
```

Since:

```
1.13 BTC ≥ 1 BTC
```

a valid proof is generated.

The verifier learns only:

```
The prover controls at least 1 BTC.
```

## Roadmap

* [x] Prototype Merkle membership proofs
* [x] Prototype 1 BTC threshold proof
* [ ] Bitcoin regtest snapshot generation
* [ ] Schnorr ownership gadget
* [ ] Arbitrary threshold support
* [ ] Utreexo integration
* [ ] Nullifier support
* [ ] Taproot interoperability

## License

MIT
