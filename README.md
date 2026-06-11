# zk-proof-of-hodl

A proof-of-concept demonstrating how to use **Noir** to generate a zero-knowledge proof that a user controls Bitcoin UTXOs with a combined value greater than **1 BTC**, without revealing which UTXOs they own.

## Overview

The prover generates a proof for the following statement:

> I know a set of Bitcoin UTXOs belonging to a committed Bitcoin snapshot whose combined value exceeds 100,000,000 sats.

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
compute_root(leaf, path) == merkle_root
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
git clone https://github.com/<username>/zk-proof-of-hodl.git
cd zk-proof-of-hodl
```

Build the Noir circuit:

```bash
nargo check
```

Execute with example inputs:

```bash
nargo execute
```

Generate a proof:

```bash
nargo prove
```

Verify the proof:

```bash
nargo verify
```

## Example

Given the following private UTXOs:

```
0.42 BTC
0.33 BTC
0.38 BTC
```

The circuit computes:

```
0.42 + 0.33 + 0.38 = 1.13 BTC
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

* [ ] Merkle membership proofs
* [ ] 1 BTC threshold proof
* [ ] Bitcoin regtest snapshot generation
* [ ] Schnorr ownership gadget
* [ ] Arbitrary threshold support
* [ ] Utreexo integration
* [ ] Nullifier support
* [ ] Taproot interoperability

## License

MIT
