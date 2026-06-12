use crate::snapshot_builder::{build_witness, format_digest, load_snapshot, write_prover_toml};
use std::path::Path;
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
    let prover_name = prover_name_from_path(prover_toml_path)?;

    let status = Command::new("nargo")
        .args(["execute", "--prover-name", &prover_name])
        .status()?;
    if !status.success() {
        anyhow::bail!("nargo execute failed");
    }

    println!("circuit constraints executed successfully");
    Ok(())
}

fn prover_name_from_path(path: &str) -> anyhow::Result<String> {
    let path = Path::new(path);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && parent != Path::new(".") {
            anyhow::bail!("nargo execute only supports prover TOML files in the project root");
        }
    }

    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| anyhow::anyhow!("invalid prover TOML path"))?;

    Ok(stem.to_string())
}
