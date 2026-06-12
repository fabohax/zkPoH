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

## Using zkPoH

This tutorial walks through the current prototype from a clean clone to a
successful Noir constraint run. In this version, `prove` means "generate witness
inputs and execute the Noir circuit constraints." It does not yet produce a
portable cryptographic proof artifact with a separate verifier command.

### 1. Install Requirements

Install:

* Rust and Cargo
* Noir / Nargo `1.0.0-beta.7` or compatible
* Bitcoin Core, only if you want to run the regtest tutorial

Check the main tools:

```bash
cargo --version
nargo --version
bitcoin-cli -version
```

### 2. Clone and Check the Project

```bash
git clone https://github.com/fabohax/zkPoH.git
cd zkPoH
```

Run the Rust and Noir tests:

```bash
cargo test
nargo test
```

Check the Noir circuit:

```bash
nargo check
```

### 3. Run the Built-In Snapshot

The default snapshot is `snapshots/utxo_snapshot.json`. It contains two example
UTXOs whose values sum to exactly `100_000_000` sats.

Generate `Prover.toml` from that snapshot:

```bash
cargo run -- build-witness
```

Execute the Noir circuit with the generated inputs:

```bash
nargo execute
```

Or run the full prototype path:

```bash
cargo run -- prove
```

Expected result:

```text
circuit constraints executed successfully
```

### 4. Inspect the Witness Inputs

`Prover.toml` is the input file consumed by `nargo execute`. It contains:

* public `merkle_root`
* private `txid_tags`
* private `vouts`
* private `values`
* private `merkle_paths`
* private `merkle_indices`

The current prototype converts each UTXO into a leaf with:

```text
leaf = blake2s(txid_tag || vout || value)
```

Then it computes the two-leaf Merkle root with:

```text
root = blake2s(left_leaf || right_leaf)
```

`txid_tag` is currently the final 8 bytes of the Bitcoin txid interpreted as a
big-endian `u64`. This keeps the circuit compact for the prototype.

### 5. Use the Regtest Fixture

The repository includes a regtest-derived fixture:

```text
snapshots/regtest_utxo_snapshot.json
Prover.regtest.toml
```

To regenerate witness inputs from the regtest snapshot:

```bash
cargo run -- build-witness \
  --snapshot snapshots/regtest_utxo_snapshot.json \
  --output Prover.regtest.toml
```

To run the circuit against the regtest snapshot:

```bash
cargo run -- prove \
  --snapshot snapshots/regtest_utxo_snapshot.json \
  --output Prover.toml
```

This should solve the Noir witness and report a total of `100000000` sats.

### 6. Create Fresh Regtest UTXOs

Start a local regtest node if one is not already running:

```bash
bitcoind -regtest -daemon -fallbackfee=0.0001
bitcoin-cli -regtest -rpcwait getblockchaininfo
```

Create a dedicated wallet:

```bash
bitcoin-cli -regtest createwallet zkpoh-regtest
```

If the wallet already exists, load it instead:

```bash
bitcoin-cli -regtest loadwallet zkpoh-regtest
```

Mine spendable regtest BTC:

```bash
MINING_ADDR=$(bitcoin-cli -regtest -rpcwallet=zkpoh-regtest getnewaddress mining bech32)
bitcoin-cli -regtest generatetoaddress 101 "$MINING_ADDR"
```

Create two wallet UTXOs that sum to 1 BTC:

```bash
ADDR_A=$(bitcoin-cli -regtest -rpcwallet=zkpoh-regtest getnewaddress proof-a bech32)
ADDR_B=$(bitcoin-cli -regtest -rpcwallet=zkpoh-regtest getnewaddress proof-b bech32)

TXID=$(bitcoin-cli -regtest -rpcwallet=zkpoh-regtest sendmany "" \
  "{\"$ADDR_A\":0.42,\"$ADDR_B\":0.58}")

bitcoin-cli -regtest generatetoaddress 1 "$MINING_ADDR"
```

Generate a zkPoH snapshot automatically from the wallet's spendable confirmed
UTXOs:

```bash
cargo run -- snapshot-regtest \
  --wallet zkpoh-regtest \
  --output snapshots/regtest_utxo_snapshot.json
```

The command selects the smallest pair of safe, spendable, confirmed UTXOs whose
combined value meets the threshold. The current circuit expects exactly two
UTXOs, so the snapshot generator writes exactly two entries.

Then generate and execute the witness:

```bash
cargo run -- prove \
  --snapshot snapshots/regtest_utxo_snapshot.json \
  --output Prover.regtest.toml
```

#### Manual Snapshot Check

To inspect or build the snapshot manually, list the two UTXOs:

```bash
bitcoin-cli -regtest -rpcwallet=zkpoh-regtest listunspent \
  1 9999999 "[\"$ADDR_A\",\"$ADDR_B\"]"
```

Copy the resulting `txid`, `vout`, `amount`, and `address` fields into a snapshot
JSON file with this shape. Convert BTC amounts to sats for `value`, and replace
the example `vout` values with the actual output indexes from `listunspent`.

```json
{
  "snapshot": "bitcoin-regtest-utxo-snapshot",
  "timestamp": "2026-06-12T00:00:00Z",
  "threshold_sats": 100000000,
  "utxos": [
    {
      "txid": "<txid>",
      "vout": 1,
      "value": 42000000,
      "address": "<address-a>"
    },
    {
      "txid": "<txid>",
      "vout": 2,
      "value": 58000000,
      "address": "<address-b>"
    }
  ]
}
```

Verify each UTXO is live in Bitcoin Core, using the actual `vout` numbers from
`listunspent`:

```bash
VOUT_A=1
VOUT_B=2

bitcoin-cli -regtest gettxout "$TXID" "$VOUT_A"
bitcoin-cli -regtest gettxout "$TXID" "$VOUT_B"
```

Then run:

```bash
cargo run -- prove --snapshot snapshots/regtest_utxo_snapshot.json --output Prover.regtest.toml
```

### 7. Try Failure Cases

The Noir tests already cover failure behavior:

```bash
nargo test
```

The circuit rejects:

* below-threshold values
* invalid Merkle paths
* invalid Merkle indices

You can also edit `Prover.toml` manually and run:

```bash
nargo execute
```

If the root, path, or threshold no longer matches, witness solving fails.

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
1.00 BTC ≥ 1 BTC
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
