use blake2::{Blake2s256, Digest as _};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::fs;

/// Number of UTXO slots in one proof. Must match `SELECTED_UTXOS` in `src/main.nr`.
pub const SELECTED_UTXOS: usize = 4;
/// Merkle tree depth. Must match `MERKLE_DEPTH` in `src/main.nr`.
pub const MERKLE_DEPTH: usize = 16;
pub const MAX_LEAVES: usize = 1 << MERKLE_DEPTH;
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

/// Circuit inputs for one proof. Field names match `main` in `src/main.nr`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WitnessInput {
    pub merkle_root: HashDigest,
    pub threshold_sats: u64,
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

pub fn empty_leaf() -> HashDigest {
    hash_leaf(0, 0, 0)
}

/// A binary Merkle tree of depth `MERKLE_DEPTH` over `leaves`, padded on the right with
/// `empty_leaf()`. Only the occupied prefix of each level is stored; every node to the
/// right of it equals the all-empty subtree root for that level.
#[derive(Clone, Debug)]
pub struct MerkleTree {
    /// `levels[0]` is the leaves, `levels[MERKLE_DEPTH]` holds the root alone.
    levels: Vec<Vec<HashDigest>>,
    /// `empty[level]` is the root of an all-empty subtree of height `level`.
    empty: Vec<HashDigest>,
}

impl MerkleTree {
    pub fn build(leaves: &[HashDigest]) -> anyhow::Result<Self> {
        if leaves.is_empty() {
            anyhow::bail!("tree needs at least one leaf");
        }
        if leaves.len() > MAX_LEAVES {
            anyhow::bail!(
                "tree holds at most {MAX_LEAVES} leaves; got {}",
                leaves.len()
            );
        }

        let mut empty = Vec::with_capacity(MERKLE_DEPTH + 1);
        let mut node = empty_leaf();
        for _ in 0..=MERKLE_DEPTH {
            empty.push(node);
            node = hash_pair(&node, &node);
        }

        let mut levels = Vec::with_capacity(MERKLE_DEPTH + 1);
        levels.push(leaves.to_vec());
        for level in 0..MERKLE_DEPTH {
            let current = &levels[level];
            let mut next = Vec::with_capacity(current.len().div_ceil(2));
            for pair in current.chunks(2) {
                let right = pair.get(1).unwrap_or(&empty[level]);
                next.push(hash_pair(&pair[0], right));
            }
            levels.push(next);
        }

        Ok(Self { levels, empty })
    }

    pub fn root(&self) -> HashDigest {
        self.levels[MERKLE_DEPTH][0]
    }

    pub fn leaf_count(&self) -> usize {
        self.levels[0].len()
    }

    fn node(&self, level: usize, index: usize) -> HashDigest {
        self.levels[level]
            .get(index)
            .copied()
            .unwrap_or(self.empty[level])
    }

    /// Sibling hashes from the leaf level upward, as the circuit expects them.
    pub fn path(&self, leaf_index: usize) -> anyhow::Result<[HashDigest; MERKLE_DEPTH]> {
        if leaf_index >= MAX_LEAVES {
            anyhow::bail!("leaf index {leaf_index} is outside a depth-{MERKLE_DEPTH} tree");
        }
        let mut path = [[0_u8; DIGEST_BYTES]; MERKLE_DEPTH];
        let mut index = leaf_index;
        for (level, slot) in path.iter_mut().enumerate() {
            *slot = self.node(level, index ^ 1);
            index >>= 1;
        }
        Ok(path)
    }
}

/// Recomputes the root from a leaf and its path; mirrors `verify_merkle_path` in Noir.
pub fn verify_path(
    leaf: &HashDigest,
    path: &[HashDigest; MERKLE_DEPTH],
    index: usize,
) -> HashDigest {
    let mut node = *leaf;
    let mut remaining = index;
    for sibling in path {
        node = if remaining & 1 == 1 {
            hash_pair(sibling, &node)
        } else {
            hash_pair(&node, sibling)
        };
        remaining >>= 1;
    }
    node
}

pub fn leaf_for(utxo: &UtxoEntry) -> anyhow::Result<HashDigest> {
    Ok(hash_leaf(
        txid_tag(&utxo.txid)?,
        utxo.vout as u64,
        utxo.value,
    ))
}

/// Builds the tree over every UTXO in the snapshot and selects `selected` (indices into
/// `snapshot.utxos`) for the proof. Unused slots point at empty leaves just past the
/// occupied prefix, so every slot has a distinct index.
pub fn build_witness(snapshot: &UtxoSnapshot, selected: &[usize]) -> anyhow::Result<WitnessInput> {
    if snapshot.utxos.is_empty() {
        anyhow::bail!("snapshot must include at least one UTXO");
    }
    if selected.is_empty() || selected.len() > SELECTED_UTXOS {
        anyhow::bail!(
            "select between 1 and {SELECTED_UTXOS} UTXOs; got {}",
            selected.len()
        );
    }
    let mut sorted = selected.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    if sorted.len() != selected.len() {
        anyhow::bail!("selected UTXO indices must be distinct");
    }
    let leaf_count = snapshot.utxos.len();
    if leaf_count + (SELECTED_UTXOS - selected.len()) > MAX_LEAVES {
        anyhow::bail!("snapshot too large for depth {MERKLE_DEPTH} with padding");
    }

    let leaves = snapshot
        .utxos
        .iter()
        .map(leaf_for)
        .collect::<anyhow::Result<Vec<_>>>()?;
    let tree = MerkleTree::build(&leaves)?;

    let mut witness = WitnessInput {
        merkle_root: tree.root(),
        threshold_sats: snapshot.threshold_sats,
        txid_tags: [0; SELECTED_UTXOS],
        vouts: [0; SELECTED_UTXOS],
        values: [0; SELECTED_UTXOS],
        merkle_paths: [[[0; DIGEST_BYTES]; MERKLE_DEPTH]; SELECTED_UTXOS],
        merkle_indices: [0; SELECTED_UTXOS],
    };

    for (slot, &utxo_index) in selected.iter().enumerate() {
        let utxo = snapshot
            .utxos
            .get(utxo_index)
            .ok_or_else(|| anyhow::anyhow!("selected index {utxo_index} is out of range"))?;
        witness.txid_tags[slot] = txid_tag(&utxo.txid)?;
        witness.vouts[slot] = utxo.vout as u64;
        witness.values[slot] = utxo.value;
        witness.merkle_paths[slot] = tree.path(utxo_index)?;
        witness.merkle_indices[slot] = utxo_index as u64;
    }
    for (offset, slot) in (selected.len()..SELECTED_UTXOS).enumerate() {
        let padding_index = leaf_count + offset;
        witness.merkle_paths[slot] = tree.path(padding_index)?;
        witness.merkle_indices[slot] = padding_index as u64;
    }

    Ok(witness)
}

/// Writes `Prover.toml` for `nargo execute`. Integers are quoted strings, byte arrays are
/// arrays of decimal integers, as Nargo expects.
pub fn write_prover_toml(path: &str, witness: &WitnessInput) -> anyhow::Result<()> {
    let mut out = String::new();
    writeln!(out, "merkle_root = {}", format_digest(&witness.merkle_root))?;
    writeln!(out, "threshold_sats = \"{}\"", witness.threshold_sats)?;
    writeln!(out, "txid_tags = {}", format_u64s(&witness.txid_tags))?;
    writeln!(out, "vouts = {}", format_u64s(&witness.vouts))?;
    writeln!(out, "values = {}", format_u64s(&witness.values))?;
    let paths = witness
        .merkle_paths
        .iter()
        .map(|path| {
            let nodes = path.iter().map(format_digest).collect::<Vec<_>>();
            format!("[{}]", nodes.join(", "))
        })
        .collect::<Vec<_>>();
    writeln!(out, "merkle_paths = [{}]", paths.join(", "))?;
    writeln!(
        out,
        "merkle_indices = {}",
        format_u64s(&witness.merkle_indices)
    )?;
    fs::write(path, out)?;
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

fn format_u64s(values: &[u64]) -> String {
    let items = values
        .iter()
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{items}]")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex32(hex: &str) -> HashDigest {
        let bytes = hex::decode(hex).unwrap();
        bytes.as_slice().try_into().unwrap()
    }

    fn sample_snapshot() -> UtxoSnapshot {
        UtxoSnapshot {
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
        }
    }

    /// Same vectors as `hash_vectors_match_rust` in `src/main.nr`.
    #[test]
    fn hash_vectors_match_noir() {
        let empty = empty_leaf();
        assert_eq!(
            empty,
            hex32("1032c4e37bf6cf095169c25766189389c741025d8f1573848db53ba484c107d3")
        );
        assert_eq!(
            hash_leaf(1, 0, 42_000_000),
            hex32("f778b70dfb6d6387131a32dcbefeaf549425f0f0a751f351348da3a0a71223ab")
        );
        assert_eq!(
            hash_pair(&empty, &empty),
            hex32("c9f6838746966ac208862449ceb13df28add06079cc8e324798530800e9549e1")
        );
        let witness = build_witness(&sample_snapshot(), &[0, 1]).unwrap();
        assert_eq!(
            witness.merkle_root,
            hex32("d052893aae2d5f2634dfd4aaa36ad2a0e8240e3dc5789501e3e80762e1f8df4f")
        );
    }

    #[test]
    fn padded_slots_use_distinct_empty_indices() {
        let witness = build_witness(&sample_snapshot(), &[0, 1]).unwrap();
        assert_eq!(witness.values, [42_000_000, 58_000_000, 0, 0]);
        assert_eq!(witness.merkle_indices, [0, 1, 2, 3]);
        for slot in 0..SELECTED_UTXOS {
            let leaf = hash_leaf(
                witness.txid_tags[slot],
                witness.vouts[slot],
                witness.values[slot],
            );
            let root = verify_path(
                &leaf,
                &witness.merkle_paths[slot],
                witness.merkle_indices[slot] as usize,
            );
            assert_eq!(root, witness.merkle_root, "slot {slot}");
        }
    }

    #[test]
    fn hundred_leaf_tree_paths_verify() {
        let utxos = (0..100_u32)
            .map(|index| UtxoEntry {
                txid: format!("{index:064x}"),
                vout: index,
                value: 1_000_000 + u64::from(index),
                address: format!("addr{index}"),
            })
            .collect();
        let snapshot = UtxoSnapshot {
            snapshot: "hundred".to_string(),
            timestamp: "2026-06-11T00:00:00Z".to_string(),
            threshold_sats: 4_000_000,
            utxos,
        };
        let witness = build_witness(&snapshot, &[3, 42, 77, 99]).unwrap();
        for slot in 0..SELECTED_UTXOS {
            let leaf = hash_leaf(
                witness.txid_tags[slot],
                witness.vouts[slot],
                witness.values[slot],
            );
            let root = verify_path(
                &leaf,
                &witness.merkle_paths[slot],
                witness.merkle_indices[slot] as usize,
            );
            assert_eq!(root, witness.merkle_root, "slot {slot}");
        }
        assert_eq!(witness.merkle_indices, [3, 42, 77, 99]);
    }

    #[test]
    fn rejects_bad_selections() {
        let snapshot = sample_snapshot();
        assert!(build_witness(&snapshot, &[]).is_err());
        assert!(build_witness(&snapshot, &[0, 0]).is_err());
        assert!(build_witness(&snapshot, &[0, 1, 2]).is_err());
        assert!(build_witness(&snapshot, &[0, 1, 0, 1, 0]).is_err());
    }
}
