use zkpoh::{build_witness, format_digest, load_snapshot};

fn main() -> anyhow::Result<()> {
    let snapshot = load_snapshot("snapshots/utxo_snapshot.json")?;
    let witness = build_witness(&snapshot, &[0, 1])?;

    println!("selected_utxos = {}", snapshot.utxos.len());
    println!("merkle_root = {}", format_digest(&witness.merkle_root));

    Ok(())
}
