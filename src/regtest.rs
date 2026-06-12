use crate::snapshot_builder::{UtxoEntry, UtxoSnapshot, SELECTED_UTXOS};
use serde::Deserialize;
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct RegtestSnapshotOptions {
    pub bitcoin_cli: String,
    pub wallet: String,
    pub output: String,
    pub threshold_sats: u64,
    pub min_confirmations: u32,
}

#[derive(Clone, Debug, Deserialize)]
struct BitcoinCliUtxo {
    txid: String,
    vout: u32,
    address: String,
    amount: serde_json::Number,
    confirmations: u32,
    spendable: bool,
    safe: bool,
}

pub fn generate_regtest_snapshot(options: &RegtestSnapshotOptions) -> anyhow::Result<()> {
    let mut utxos = list_unspent(options)?;

    utxos.retain(|utxo| {
        utxo.confirmations >= options.min_confirmations && utxo.spendable && utxo.safe
    });
    utxos.sort_by_key(|utxo| btc_amount_to_sats(&utxo.amount.to_string()).unwrap_or(u64::MAX));

    let selected = select_utxos(&utxos, options.threshold_sats)?;
    let total_sats = selected.iter().map(|entry| entry.value).sum::<u64>();
    let snapshot = UtxoSnapshot {
        snapshot: format!("bitcoin-regtest-wallet-{}", options.wallet),
        timestamp: unix_timestamp_label()?,
        threshold_sats: options.threshold_sats,
        utxos: selected,
    };

    let data = serde_json::to_string_pretty(&snapshot)?;
    fs::write(&options.output, format!("{data}\n"))?;

    println!("wrote regtest snapshot to {}", options.output);
    println!("selected_utxos = {}", snapshot.utxos.len());
    println!("selected_total_sats = {total_sats}");
    for utxo in &snapshot.utxos {
        println!(
            "{}:{} value={} address={}",
            utxo.txid, utxo.vout, utxo.value, utxo.address
        );
    }

    Ok(())
}

fn list_unspent(options: &RegtestSnapshotOptions) -> anyhow::Result<Vec<BitcoinCliUtxo>> {
    let wallet = format!("-rpcwallet={}", options.wallet);
    let min_confirmations = options.min_confirmations.to_string();
    let output = Command::new(&options.bitcoin_cli)
        .args(["-regtest", &wallet, "listunspent", &min_confirmations])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("bitcoin-cli listunspent failed: {stderr}");
    }

    let stdout = String::from_utf8(output.stdout)?;
    let utxos = serde_json::from_str(&stdout)?;
    Ok(utxos)
}

fn select_utxos(utxos: &[BitcoinCliUtxo], threshold_sats: u64) -> anyhow::Result<Vec<UtxoEntry>> {
    if SELECTED_UTXOS != 2 {
        anyhow::bail!("regtest snapshot selection currently expects exactly 2 UTXOs");
    }

    let mut best_pair = None;

    for first_index in 0..utxos.len() {
        for second_index in (first_index + 1)..utxos.len() {
            let first_value = btc_amount_to_sats(&utxos[first_index].amount.to_string())?;
            let second_value = btc_amount_to_sats(&utxos[second_index].amount.to_string())?;
            let total_sats = first_value + second_value;

            if total_sats >= threshold_sats {
                best_pair = Some((first_index, second_index, first_value, second_value));
                break;
            }
        }

        if best_pair.is_some() {
            break;
        }
    }

    let Some((first_index, second_index, first_value, second_value)) = best_pair else {
        anyhow::bail!("no pair of eligible UTXOs meets threshold {threshold_sats}");
    };

    Ok(vec![
        utxo_entry(&utxos[first_index], first_value),
        utxo_entry(&utxos[second_index], second_value),
    ])
}

fn utxo_entry(utxo: &BitcoinCliUtxo, value: u64) -> UtxoEntry {
    UtxoEntry {
        txid: utxo.txid.clone(),
        vout: utxo.vout,
        value,
        address: utxo.address.clone(),
    }
}

fn btc_amount_to_sats(amount: &str) -> anyhow::Result<u64> {
    let (whole, fractional) = amount.split_once('.').unwrap_or((amount, ""));
    let whole_sats = whole.parse::<u64>()? * 100_000_000;
    let mut padded_fractional = fractional.to_string();

    if padded_fractional.len() > 8 {
        anyhow::bail!("BTC amount has more than 8 decimal places: {amount}");
    }

    while padded_fractional.len() < 8 {
        padded_fractional.push('0');
    }

    let fractional_sats = if padded_fractional.is_empty() {
        0
    } else {
        padded_fractional.parse::<u64>()?
    };

    Ok(whole_sats + fractional_sats)
}

fn unix_timestamp_label() -> anyhow::Result<String> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    Ok(format!("unix:{timestamp}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_btc_amounts_to_sats() {
        assert_eq!(btc_amount_to_sats("0.42").unwrap(), 42_000_000);
        assert_eq!(btc_amount_to_sats("0.58000000").unwrap(), 58_000_000);
        assert_eq!(btc_amount_to_sats("1").unwrap(), 100_000_000);
        assert_eq!(btc_amount_to_sats("50.00000000").unwrap(), 5_000_000_000);
    }
}
