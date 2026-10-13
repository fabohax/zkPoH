use crate::snapshot_builder::SELECTED_UTXOS;
use crate::snapshot_builder::{build_witness, format_digest, load_snapshot, write_prover_toml};
use std::path::Path;
use std::process::Command;

pub const CIRCUIT_JSON: &str = "target/zkpoh.json";
pub const WITNESS_GZ: &str = "target/zkpoh.gz";
pub const PROOF_DIR: &str = "target";

/// Selects the first `SELECTED_UTXOS` entries of the snapshot unless `selected` is given.
pub fn generate_witness(
    snapshot_path: &str,
    prover_toml_path: &str,
    selected: Option<&[usize]>,
) -> anyhow::Result<()> {
    let snapshot = load_snapshot(snapshot_path)?;
    let default: Vec<usize> = (0..snapshot.utxos.len().min(SELECTED_UTXOS)).collect();
    let selected = selected.unwrap_or(&default);
    let witness = build_witness(&snapshot, selected)?;
    let selected_total_sats = witness.values.iter().sum::<u64>();

    write_prover_toml(prover_toml_path, &witness)?;
    println!("witness inputs written");
    println!("  snapshot = {snapshot_path}");
    println!("  output = {prover_toml_path}");
    println!("  snapshot_utxos = {}", snapshot.utxos.len());
    println!("  selected_utxos = {}", selected.len());
    println!("  selected_total_sats = {selected_total_sats}");
    println!(
        "  selected_total_btc = {}",
        format_sats_as_btc(selected_total_sats)
    );
    println!("  threshold_sats = {}", witness.threshold_sats);
    println!("  merkle_root = {}", format_digest(&witness.merkle_root));
    Ok(())
}

/// Witness generation plus `nargo execute`: solves the constraints, produces no proof.
pub fn execute_circuit(snapshot_path: &str, prover_toml_path: &str) -> anyhow::Result<()> {
    generate_witness(snapshot_path, prover_toml_path, None)?;
    let prover_name = prover_name_from_path(prover_toml_path)?;
    run("nargo", &["execute", "--prover-name", &prover_name])?;
    println!("zkPoH constraints solved; witness at {WITNESS_GZ}");
    Ok(())
}

/// Full pipeline: witness, `nargo compile`, `nargo execute`, `bb prove --write_vk`.
/// Leaves `proof`, `public_inputs`, `vk`, and `vk_hash` in `target/`.
pub fn prove(snapshot_path: &str, prover_toml_path: &str) -> anyhow::Result<()> {
    generate_witness(snapshot_path, prover_toml_path, None)?;
    let prover_name = prover_name_from_path(prover_toml_path)?;
    run("nargo", &["compile"])?;
    run("nargo", &["execute", "--prover-name", &prover_name])?;
    run(
        "bb",
        &[
            "prove",
            "-b",
            CIRCUIT_JSON,
            "-w",
            WITNESS_GZ,
            "--write_vk",
            "-o",
            PROOF_DIR,
        ],
    )?;
    println!("proof written to {PROOF_DIR}/proof, key to {PROOF_DIR}/vk");
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

pub fn run(program: &str, args: &[&str]) -> anyhow::Result<()> {
    println!("running: {program} {}", args.join(" "));
    let status = Command::new(program).args(args).status()?;
    if !status.success() {
        anyhow::bail!("{program} {} failed", args.join(" "));
    }
    Ok(())
}

pub fn format_sats_as_btc(sats: u64) -> String {
    format!("{}.{:08}", sats / 100_000_000, sats % 100_000_000)
}
