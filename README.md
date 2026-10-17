# zkpoh: Zero Knowledge Proof-of-Hodl

A proof-of-concept demonstrating how to use **Noir** to prove that selected Bitcoin UTXOs from a committed snapshot have a combined value of at least **1 BTC**, without revealing which UTXOs they are.

The current Noir circuit demonstrates private Merkle inclusion plus private
threshold logic. It does **not yet** prove UTXO ownership inside the circuit.
Without an ownership binding step, a prover could select arbitrary UTXOs from
the committed snapshot. The repository includes an off-circuit ownership signer
for P2WPKH-style experiments, and the intended production direction is to move
that binding into the ZK proof.

## Overview

The current circuit generates a proof for the following statement:

> I know a set of Bitcoin UTXOs belonging to a committed Bitcoin snapshot whose combined value is at least 100,000,000 sats.

The target ownership-bound statement is stronger:

> I know keys or valid ownership signatures for hidden UTXOs in this snapshot whose hidden values sum to at least 100,000,000 sats.

The verifier should learn only that the ownership-bound statement is true.

The Noir proof does not reveal:

* UTXO identifiers,
* Bitcoin addresses,
* exact balances,
* transaction history,
* private keys.

The current v0 off-circuit ownership JSON does reveal selected UTXO and public
key data to whoever verifies it. The v1 goal is to verify ownership inside the
ZK proof so those details remain private.

## Status

⚠️ Experimental educational project.

See [Current status and development priorities](STATUS.md) for implementation
gaps, acceptance criteria, and the suggested development order.

This repository prioritizes simplicity and portability over production readiness.

Current implementation status:

* Rust can load the sample snapshot and generate `Prover.toml`.
* Noir verifies up to four selected UTXOs in a fixed two-level Merkle tree.
* Noir enforces `sum(values) >= 100_000_000`.
* Tests cover valid input, below-threshold input, and a wrong Merkle path.
* The prototype Merkle tree uses Blake2s over fixed byte encodings.
* Bitcoin ownership can be checked off-circuit with signed WIF ownership proofs.
* Bitcoin ownership is not yet verified by the Noir circuit.

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
Sign ownership challenge
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

This is the statement currently enforced by the Noir circuit. On its own, it
proves that some hidden UTXOs exist in the committed snapshot and pass the
threshold. It does not prove that the prover controls those UTXOs.

Ownership must be bound to the same snapshot root, threshold, and verifier
challenge. Conceptually, the verifier gives the prover a fresh challenge:

```text
challenge = H(
  "zkPoH-v1",
  merkle_root,
  threshold,
  verifier_nonce,
  expiry,
  context
)
```

For each selected UTXO, the prover signs that challenge with the key controlling
the output. For example, a P2WPKH output should satisfy:

```text
HASH160(pubkey_i) == scriptPubKey_pubkeyhash_i
VerifySignature(pubkey_i, challenge, sig_i) == true
```

The stronger target statement is therefore:

```text
∃ utxos, pubkeys, signatures :
    MerkleVerify(utxo_i, merkle_path_i, merkle_root)
∧   output_is_controlled_by(pubkey_i, utxo_i.scriptPubKey)
∧   VerifySignature(pubkey_i, challenge, sig_i)
∧   sum(values) ≥ threshold
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
zkpoh/
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
* Snapshot is trusted. The verifier recomputes its Merkle root and requires the
  proof's public root to match it; the snapshot itself is not independently
  reproduced yet.
* The Noir circuit does not verify UTXO ownership.
* Off-circuit ownership proofs reveal the selected UTXOs and public keys to the
  verifier, so they weaken the intended privacy model.
* No nullifiers are implemented.
* Proofs represent ownership only at snapshot time.

Future versions may include:

* in-circuit P2WPKH ownership verification,
* Schnorr ownership verification for Taproot x-only output keys,
* BIP-322-style message signing support for broader Bitcoin script coverage,
* Utreexo commitments,
* snapshot epochs,
* nullifiers,
* arbitrary thresholds.

## Requirements

* Rust
* Noir / Nargo `1.0.0-rc.4`
* Barretenberg `bb` `5.0.0` (the version Noir `1.0.0-rc.4` installs itself)
* Bitcoin Core (optional for regtest experiments)

Install the Noir toolchain:

```bash
curl -L https://raw.githubusercontent.com/noir-lang/noirup/refs/heads/main/install | bash
noirup -v 1.0.0-rc.4
curl -L https://raw.githubusercontent.com/AztecProtocol/aztec-packages/refs/heads/next/barretenberg/bbup/install | bash
bbup -v 5.0.0
```

`bbup -nv 1.0.0-rc.4` does not resolve to a bb release yet, so pass the bb
version explicitly.

## Using zkPoH

This tutorial walks through the current prototype from a clean clone to a
verified proof. `prove` generates the witness inputs, compiles and executes the
Noir circuit, and runs Barretenberg to produce `target/proof`,
`target/public_inputs`, and `target/vk`. `verify` checks that the verification
key is the trusted one, that the public inputs match the snapshot's root and
threshold, and then runs `bb verify`.

## Using zkpoh as a Rust Library

The crate exposes the witness, Merkle hashing, ownership proof, regtest snapshot,
and circuit helper APIs from `src/lib.rs`.

From another local Rust project:

```toml
[dependencies]
zkpoh = { path = "../zkPoH" }
```

Basic witness generation. `build_witness` takes the indices of the UTXOs to
prove (1 to `SELECTED_UTXOS`); the tree is built over the whole snapshot, which
is rejected if it lists an outpoint twice:

```rust
use zkpoh::{build_witness, format_digest, load_snapshot};

fn main() -> anyhow::Result<()> {
    let snapshot = load_snapshot("snapshots/utxo_snapshot.json")?;
    let witness = build_witness(&snapshot, &[0, 1])?;

    println!("merkle_root = {}", format_digest(&witness.merkle_root));
    println!("threshold_sats = {}", witness.threshold_sats);
    Ok(())
}
```

Other entry points:

* `snapshot_root(&snapshot)` is the public root a verifier should expect;
  `snapshot_tree(&snapshot)` is the `MerkleTree` behind it, with `path(index)`.
* `prove(snapshot_path, prover_toml_path)` runs the full proving pipeline.
* `expected_statement(snapshot_path)` and
  `verify_proof(&expected, trusted_vk_sha256)` are what the `verify` command
  calls; `decode_public_inputs` and `check_vk` are usable on their own.

Run the included example:

```bash
cargo run --example build_witness
```

### 1. Install Requirements

Install:

* Rust and Cargo
* Noir / Nargo `1.0.0-rc.4` and Barretenberg `bb` `5.0.0` (see Requirements)
* Bitcoin Core, only if you want to run the regtest tutorial

Check the main tools:

```bash
cargo --version
nargo --version
bb --version
bitcoin-cli -version
```

### 2. Clone and Check the Project

```bash
git clone https://github.com/fabohax/zkPoH.git
cd zkPoH
```

Run the full local validation suite:

```bash
cargo run -- test-all
```

Or use the Makefile shortcut:

```bash
make test
```

You can also run the individual checks directly:

```bash
cargo test
nargo test
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

Check the Noir circuit:

```bash
cargo run -- check-circuit
```

### 3. Run the Built-In Snapshot

The default snapshot is `snapshots/utxo_snapshot.json`. It contains two example
UTXOs whose values sum to exactly `100_000_000` sats. The circuit supports up
to four UTXOs; unused slots are padded with empty leaves.

Run the full happy path:

```bash
cargo run -- demo
```

Or:

```bash
make demo
```

Generate `Prover.toml` from that snapshot:

```bash
cargo run -- build-witness
```

Execute the Noir circuit with the generated inputs:

```bash
nargo execute
```

Or run the full prototype path, which also produces a Barretenberg proof:

```bash
cargo run -- prove
cargo run -- verify
```

Expected result:

```text
proof written to target/proof, key to target/vk
...
verification key matches trusted_vk.sha256
public inputs match: merkle_root = d052893a..., threshold_sats = 100000000
...
proof verified
```

`verify` does three things, in order, and stops at the first failure:

1. Hashes `target/vk` and compares it to `trusted_vk.sha256`. The key is
   deterministic for a given circuit and `bb` version, so this pins the circuit
   the proof was made for. After an intentional circuit change, run
   `make prove` then `make update-trusted-vk` and commit the new digest.
   `--vk-sha256 <hex>` overrides the file.
2. Decodes `target/public_inputs` and compares it to the snapshot named by
   `--snapshot` (default `snapshots/utxo_snapshot.json`): the Merkle root must
   equal the root recomputed from the snapshot, and the threshold must equal the
   snapshot's `threshold_sats`. `bb` writes 33 big-endian 32-byte fields: one
   root byte per field in fields 0–31, `threshold_sats` in field 32.
3. Runs `bb verify -p target/proof -k target/vk -i target/public_inputs`.

`bb verify` alone only checks that the proof matches whatever inputs and key it
is handed. `test-all` also checks that the same proof is rejected for a
different snapshot and for an untrusted key.

### 4. Inspect the Witness Inputs

`Prover.toml` is the input file consumed by `nargo execute`. It contains:

* public `merkle_root`
* public `threshold_sats`
* private `txid_tags`
* private `vouts`
* private `values`
* private `merkle_paths`
* private `merkle_indices`

The current prototype converts each UTXO into a leaf with:

```text
leaf = blake2s(txid_tag || vout || value)
```

The snapshot is rejected if it lists the same `txid:vout` twice. Then the
leaves go into a depth-16 Merkle tree (up to 65,536 leaves).
Unused positions hash as `hash_leaf(0, 0, 0)`; empty subtrees are precomputed
per level so a sparse tree builds quickly. Unused proof slots are padded with
an empty leaf at a distinct index past the last real leaf:

```text
node = blake2s(left || right)
root = node at level 16
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

This should solve the Noir witness and report totals in sats and BTC.

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

The command selects the smallest set of up to four safe, spendable, confirmed
UTXOs whose combined value meets the threshold.

Then generate and execute the witness:

```bash
cargo run -- prove \
  --snapshot snapshots/regtest_utxo_snapshot.json \
  --output Prover.regtest.toml
```

#### Manual Snapshot Check

To inspect or build the snapshot manually, list the selected UTXOs:

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

### 7. Sign UTXO Ownership from the Terminal

The circuit currently proves snapshot membership and threshold. The terminal
ownership signer implements the simpler **v0 off-circuit ownership check**: each
selected UTXO address must match a provided Bitcoin private key, and the key
must sign the deterministic zkPoH ownership challenge.

This prevents the prover from claiming unrelated snapshot UTXOs in the terminal
workflow, but it is not the final privacy model. Because the verifier checks the
signatures outside the ZK proof, the verifier may learn the selected UTXOs,
addresses, public keys, and signatures.

Preferred key input is a file with one WIF private key per line:

```bash
chmod 600 /path/to/wifs.txt
cargo run -- sign-ownership \
  --snapshot snapshots/regtest_utxo_snapshot.json \
  --wif-file /path/to/wifs.txt \
  --output ownership_proof.json \
  --network regtest
```

You can also read WIFs from an environment variable. Multiple WIFs may be
comma-separated:

```bash
export ZKPOH_WIFS='c...'
cargo run -- sign-ownership \
  --snapshot snapshots/regtest_utxo_snapshot.json \
  --wif-env ZKPOH_WIFS \
  --output ownership_proof.json \
  --network regtest
```

For one-off testing only, pass WIFs directly:

```bash
cargo run -- sign-ownership \
  --snapshot snapshots/regtest_utxo_snapshot.json \
  --wif c... \
  --output ownership_proof.json \
  --network regtest
```

Passing secrets directly in shell commands can leak through shell history and
process listings. Prefer `--wif-file` or `--wif-env` for terminal use.

The signer writes `ownership_proof.json` containing:

* the canonical ownership challenge
* the challenge SHA-256 digest
* the Merkle root
* each signed UTXO
* each compressed public key
* each DER-encoded ECDSA signature

The CLI verifies every signature and checks that each public key maps to the
UTXO's P2WPKH address before writing the proof file. This is still an
off-circuit ownership check.

Verify an ownership proof later:

```bash
cargo run -- verify-ownership \
  --proof ownership_proof.json \
  --network regtest
```

The intended **v1 in-circuit ownership check** keeps the selected UTXOs,
pubkeys, signatures, and Merkle paths private. The Noir circuit should verify
internally that each UTXO belongs to the committed snapshot, each output is
controlled by the corresponding key or valid signature, each signature is bound
to the challenge, and the private total passes the threshold. The verifier would
then see only a valid proof for a public snapshot root, threshold, and
challenge/context.

Taproot support should ideally verify Schnorr signatures against the x-only
output key. Broader Bitcoin script support likely needs a BIP-322-style message
signing design; proving arbitrary script satisfaction inside ZK is a larger
piece of work.

Lightning channel funding outputs need additional care because they are often
not simple single-key outputs. Depending on whether the funding output is
2-of-2 multisig, MuSig2, Taproot key-path, or another construction, the proof
may need to express unilateral participation, cooperative control, or a
channel-specific ownership condition without revealing the funding output.

### 8. Try Failure Cases

The Noir tests already cover failure behavior:

```bash
nargo test
```

The circuit rejects:

* below-threshold values
* invalid Merkle paths
* duplicate Merkle indices (the same leaf counted twice)
* duplicate outpoints among counted slots (one output listed several times in
  the snapshot at different positions); zero-value padding slots are exempt
* Merkle indices at or above `2^MERKLE_DEPTH` (an alias of a lower index)
* values above the total bitcoin supply (prevents `u64` wrap in the sum)

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

With ownership binding integrated into the proof, the verifier learns only:

```
The prover controls at least 1 BTC.
```

## Roadmap

* [x] Prototype Merkle membership proofs
* [x] Prototype 1 BTC threshold proof
* [x] Bitcoin regtest snapshot generation
* [x] Up to four selected UTXOs
* [ ] Schnorr ownership gadget
* [ ] Arbitrary threshold support
* [ ] Utreexo integration
* [ ] Nullifier support
* [ ] Taproot interoperability

## License

MIT

## Discussion

[delvingbitcoin.org/t/zkpoh-zero-knowledge-proof-of-hodl/2699](https://delvingbitcoin.org/t/zkpoh-zero-knowledge-proof-of-hodl/2699)
