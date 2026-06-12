use crate::snapshot_builder::{build_witness, format_digest, load_snapshot, write_prover_toml};
use std::process::Command;

pub fn generate_witness(snapshot_path: &str, prover_toml_path: &str) -> anyhow::Result<()> {
    let snapshot = load_snapshot(snapshot_path)?;
    let witness = build_witness(&snapshot)?;

    write_prover_toml(prover_toml_path, &witness)?;
    println!("wrote witness inputs to {prover_toml_path}");
    println!("merkle_root = {}", format_digest(&witness.merkle_root));
    println!(
        "selected_total_sats = {}",
        witness.values.iter().sum::<u64>()
    );

    Ok(())
}

pub fn execute_circuit(snapshot_path: &str, prover_toml_path: &str) -> anyhow::Result<()> {
    generate_witness(snapshot_path, prover_toml_path)?;

    let status = Command::new("nargo").arg("execute").status()?;
    if !status.success() {
        anyhow::bail!("nargo execute failed");
    }

    println!("circuit constraints executed successfully");
    Ok(())
}
