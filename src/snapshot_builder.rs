use serde::{Deserialize, Serialize};
use std::fs;

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
