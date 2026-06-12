use blake2::{Blake2s256, Digest as _};
use serde::{Deserialize, Serialize};
use std::fs;

pub const SELECTED_UTXOS: usize = 2;
pub const MERKLE_DEPTH: usize = 1;
pub const DIGEST_BYTES: usize = 32;
pub const LEAF_PREIMAGE_BYTES: usize = 24;
pub const NODE_PREIMAGE_BYTES: usize = 64;

pub type HashDigest = [u8; DIGEST_BYTES];

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UtxoEntry {
    pub txid: String,
    pub vout: u32,
    pub value: u64,
    pub address: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UtxoSnapshot {
    pub snapshot: String,
    pub timestamp: String,
    pub threshold_sats: u64,
    pub utxos: Vec<UtxoEntry>,
}

pub fn load_snapshot(path: &str) -> anyhow::Result<UtxoSnapshot> {
    let data = fs::read_to_string(path)?;
    let snapshot = serde_json::from_str(&data)?;
    Ok(snapshot)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WitnessInput {
    pub merkle_root: HashDigest,
    pub txid_tags: [u64; SELECTED_UTXOS],
    pub vouts: [u64; SELECTED_UTXOS],
    pub values: [u64; SELECTED_UTXOS],
    pub merkle_paths: [[HashDigest; MERKLE_DEPTH]; SELECTED_UTXOS],
    pub merkle_indices: [u64; SELECTED_UTXOS],
}

pub fn txid_tag(txid: &str) -> anyhow::Result<u64> {
    let suffix = txid
        .get(txid.len().saturating_sub(16)..)
        .ok_or_else(|| anyhow::anyhow!("txid is empty"))?;
    Ok(u64::from_str_radix(suffix, 16)?)
}

pub fn blake2s_digest(input: &[u8]) -> HashDigest {
    Blake2s256::digest(input).into()
}

pub fn hash_leaf(txid_tag: u64, vout: u64, value: u64) -> HashDigest {
    let mut input = [0_u8; LEAF_PREIMAGE_BYTES];
    input[0..8].copy_from_slice(&txid_tag.to_be_bytes());
    input[8..16].copy_from_slice(&vout.to_be_bytes());
    input[16..24].copy_from_slice(&value.to_be_bytes());
    blake2s_digest(&input)
}

pub fn hash_pair(left: &HashDigest, right: &HashDigest) -> HashDigest {
    let mut input = [0_u8; NODE_PREIMAGE_BYTES];
    input[0..DIGEST_BYTES].copy_from_slice(left);
    input[DIGEST_BYTES..NODE_PREIMAGE_BYTES].copy_from_slice(right);
    blake2s_digest(&input)
}

pub fn build_witness(snapshot: &UtxoSnapshot) -> anyhow::Result<WitnessInput> {
    if snapshot.utxos.len() != SELECTED_UTXOS {
        anyhow::bail!("this prototype expects exactly {SELECTED_UTXOS} UTXOs in the snapshot");
    }

    let mut txid_tags = [0_u64; SELECTED_UTXOS];
    let mut vouts = [0_u64; SELECTED_UTXOS];
    let mut values = [0_u64; SELECTED_UTXOS];
    let mut leaves = [[0_u8; DIGEST_BYTES]; SELECTED_UTXOS];

    for (index, utxo) in snapshot.utxos.iter().enumerate() {
        let tag = txid_tag(&utxo.txid)?;
        txid_tags[index] = tag;
        vouts[index] = utxo.vout as u64;
        values[index] = utxo.value;
        leaves[index] = hash_leaf(tag, utxo.vout as u64, utxo.value);
    }

    let merkle_root = hash_pair(&leaves[0], &leaves[1]);

    Ok(WitnessInput {
        merkle_root,
        txid_tags,
        vouts,
        values,
        merkle_paths: [[leaves[1]], [leaves[0]]],
        merkle_indices: [0, 1],
    })
}

pub fn write_prover_toml(path: &str, witness: &WitnessInput) -> anyhow::Result<()> {
    let contents = format!(
        "\
merkle_root = {merkle_root}
txid_tags = [\"{txid_0}\", \"{txid_1}\"]
vouts = [\"{vout_0}\", \"{vout_1}\"]
values = [\"{value_0}\", \"{value_1}\"]
merkle_paths = [[{path_0}], [{path_1}]]
merkle_indices = [\"{index_0}\", \"{index_1}\"]
",
        merkle_root = format_digest(&witness.merkle_root),
        txid_0 = witness.txid_tags[0],
        txid_1 = witness.txid_tags[1],
        vout_0 = witness.vouts[0],
        vout_1 = witness.vouts[1],
        value_0 = witness.values[0],
        value_1 = witness.values[1],
        path_0 = format_digest(&witness.merkle_paths[0][0]),
        path_1 = format_digest(&witness.merkle_paths[1][0]),
        index_0 = witness.merkle_indices[0],
        index_1 = witness.merkle_indices[1],
    );

    fs::write(path, contents)?;
    Ok(())
}

pub fn format_digest(digest: &HashDigest) -> String {
    let bytes = digest
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{bytes}]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_expected_witness_for_two_leaf_snapshot() {
        let snapshot = UtxoSnapshot {
            snapshot: "test".to_string(),
            timestamp: "2026-06-11T00:00:00Z".to_string(),
            threshold_sats: 100_000_000,
            utxos: vec![
                UtxoEntry {
                    txid: format!("{:064x}", 1),
                    vout: 0,
                    value: 42_000_000,
                    address: "addr0".to_string(),
                },
                UtxoEntry {
                    txid: format!("{:064x}", 2),
                    vout: 1,
                    value: 58_000_000,
                    address: "addr1".to_string(),
                },
            ],
        };

        let witness = build_witness(&snapshot).unwrap();
        let leaf_0 = hash_leaf(1, 0, 42_000_000);
        let leaf_1 = hash_leaf(2, 1, 58_000_000);

        assert_eq!(witness.merkle_root, hash_pair(&leaf_0, &leaf_1));
        assert_eq!(witness.merkle_paths, [[leaf_1], [leaf_0]]);
        assert_eq!(witness.values, [42_000_000, 58_000_000]);
    }
}
