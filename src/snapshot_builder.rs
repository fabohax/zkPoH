use blake2::{Blake2s256, Digest as _};
use serde::{Deserialize, Serialize};
use std::fs;

pub const SELECTED_UTXOS: usize = 4;
pub const MERKLE_DEPTH: usize = 2;
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
    if snapshot.utxos.is_empty() {
        anyhow::bail!("snapshot must include at least one UTXO");
    }
    if snapshot.utxos.len() > SELECTED_UTXOS {
        anyhow::bail!(
            "this prototype supports up to {SELECTED_UTXOS} UTXOs per snapshot; got {}",
            snapshot.utxos.len()
        );
    }

    let mut txid_tags = [0_u64; SELECTED_UTXOS];
    let mut vouts = [0_u64; SELECTED_UTXOS];
    let mut values = [0_u64; SELECTED_UTXOS];
    let mut leaves = [[0_u8; DIGEST_BYTES]; SELECTED_UTXOS];
    let empty_leaf = hash_leaf(0, 0, 0);
    leaves.fill(empty_leaf);

    for (index, utxo) in snapshot.utxos.iter().enumerate() {
        let tag = txid_tag(&utxo.txid)?;
        txid_tags[index] = tag;
        vouts[index] = utxo.vout as u64;
        values[index] = utxo.value;
        leaves[index] = hash_leaf(tag, utxo.vout as u64, utxo.value);
    }

    let node_0 = hash_pair(&leaves[0], &leaves[1]);
    let node_1 = hash_pair(&leaves[2], &leaves[3]);
    let merkle_root = hash_pair(&node_0, &node_1);

    Ok(WitnessInput {
        merkle_root,
        txid_tags,
        vouts,
        values,
        merkle_paths: [
            [leaves[1], node_1],
            [leaves[0], node_1],
            [leaves[3], node_0],
            [leaves[2], node_0],
        ],
        merkle_indices: [0, 1, 2, 3],
    })
}

pub fn write_prover_toml(path: &str, witness: &WitnessInput) -> anyhow::Result<()> {
    let contents = format!(
        "\
merkle_root = {merkle_root}
txid_tags = [\"{txid_0}\", \"{txid_1}\", \"{txid_2}\", \"{txid_3}\"]
vouts = [\"{vout_0}\", \"{vout_1}\", \"{vout_2}\", \"{vout_3}\"]
values = [\"{value_0}\", \"{value_1}\", \"{value_2}\", \"{value_3}\"]
merkle_paths = [[{path_0_0}, {path_0_1}], [{path_1_0}, {path_1_1}], [{path_2_0}, {path_2_1}], [{path_3_0}, {path_3_1}]]
merkle_indices = [\"{index_0}\", \"{index_1}\", \"{index_2}\", \"{index_3}\"]
",
        merkle_root = format_digest(&witness.merkle_root),
        txid_0 = witness.txid_tags[0],
        txid_1 = witness.txid_tags[1],
        txid_2 = witness.txid_tags[2],
        txid_3 = witness.txid_tags[3],
        vout_0 = witness.vouts[0],
        vout_1 = witness.vouts[1],
        vout_2 = witness.vouts[2],
        vout_3 = witness.vouts[3],
        value_0 = witness.values[0],
        value_1 = witness.values[1],
        value_2 = witness.values[2],
        value_3 = witness.values[3],
        path_0_0 = format_digest(&witness.merkle_paths[0][0]),
        path_0_1 = format_digest(&witness.merkle_paths[0][1]),
        path_1_0 = format_digest(&witness.merkle_paths[1][0]),
        path_1_1 = format_digest(&witness.merkle_paths[1][1]),
        path_2_0 = format_digest(&witness.merkle_paths[2][0]),
        path_2_1 = format_digest(&witness.merkle_paths[2][1]),
        path_3_0 = format_digest(&witness.merkle_paths[3][0]),
        path_3_1 = format_digest(&witness.merkle_paths[3][1]),
        index_0 = witness.merkle_indices[0],
        index_1 = witness.merkle_indices[1],
        index_2 = witness.merkle_indices[2],
        index_3 = witness.merkle_indices[3],
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
    fn builds_expected_witness_for_padded_four_leaf_snapshot() {
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
        let empty_leaf = hash_leaf(0, 0, 0);
        let node_0 = hash_pair(&leaf_0, &leaf_1);
        let node_1 = hash_pair(&empty_leaf, &empty_leaf);

        assert_eq!(witness.merkle_root, hash_pair(&node_0, &node_1));
        assert_eq!(
            witness.merkle_paths,
            [
                [leaf_1, node_1],
                [leaf_0, node_1],
                [empty_leaf, node_0],
                [empty_leaf, node_0]
            ]
        );
        assert_eq!(witness.values, [42_000_000, 58_000_000, 0, 0]);
    }

    #[test]
    fn rejects_more_than_four_utxos() {
        let snapshot = UtxoSnapshot {
            snapshot: "test".to_string(),
            timestamp: "2026-06-11T00:00:00Z".to_string(),
            threshold_sats: 100_000_000,
            utxos: (0..5)
                .map(|index| UtxoEntry {
                    txid: format!("{index:064x}"),
                    vout: index,
                    value: 25_000_000,
                    address: format!("addr{index}"),
                })
                .collect(),
        };

        assert!(build_witness(&snapshot).is_err());
    }
}
