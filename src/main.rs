mod ownership;
mod prover;
mod regtest;
mod snapshot_builder;
mod verifier;

use bitcoin::Network;
use clap::{Parser, Subcommand};
use std::str::FromStr;

const DEFAULT_SNAPSHOT_PATH: &str = "snapshots/utxo_snapshot.json";
const DEFAULT_PROVER_TOML_PATH: &str = "Prover.toml";
const DEFAULT_OWNERSHIP_PROOF_PATH: &str = "ownership_proof.json";
const DEFAULT_REGTEST_WALLET: &str = "zkpoh-regtest";
const DEFAULT_REGTEST_SNAPSHOT_PATH: &str = "snapshots/regtest_utxo_snapshot.json";
const DEFAULT_THRESHOLD_SATS: u64 = 100_000_000;

#[derive(Debug, Parser)]
#[command(name = "zk-proof-of-hodl")]
#[command(about = "Prototype zk Proof-of-Hodl witness and circuit runner")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Generate Prover.toml from the sample UTXO snapshot.
    BuildWitness {
        #[arg(long, default_value = DEFAULT_SNAPSHOT_PATH)]
        snapshot: String,
        #[arg(long, default_value = DEFAULT_PROVER_TOML_PATH)]
        output: String,
    },
    /// Generate witness inputs and execute the Noir constraints.
    Prove {
        #[arg(long, default_value = DEFAULT_SNAPSHOT_PATH)]
        snapshot: String,
        #[arg(long, default_value = DEFAULT_PROVER_TOML_PATH)]
        output: String,
    },
    /// Check the Noir circuit.
    Verify,
    /// Generate a snapshot from spendable Bitcoin Core regtest wallet UTXOs.
    SnapshotRegtest {
        #[arg(long, default_value = DEFAULT_REGTEST_WALLET)]
        wallet: String,
        #[arg(long, default_value = DEFAULT_REGTEST_SNAPSHOT_PATH)]
        output: String,
        #[arg(long, default_value_t = DEFAULT_THRESHOLD_SATS)]
        threshold_sats: u64,
        #[arg(long, default_value_t = 1)]
        min_confirmations: u32,
        #[arg(long, default_value = "bitcoin-cli")]
        bitcoin_cli: String,
    },
    /// Sign a snapshot ownership challenge with Bitcoin WIF private keys.
    SignOwnership {
        #[arg(long, default_value = DEFAULT_REGTEST_SNAPSHOT_PATH)]
        snapshot: String,
        #[arg(long, default_value = DEFAULT_OWNERSHIP_PROOF_PATH)]
        output: String,
        #[arg(long, default_value = "regtest")]
        network: String,
        #[arg(long)]
        nonce: Option<String>,
        #[arg(long)]
        wif: Vec<String>,
        #[arg(long = "wif-file")]
        wif_files: Vec<String>,
        #[arg(long = "wif-env")]
        wif_envs: Vec<String>,
    },
    /// Verify a JSON ownership proof produced by sign-ownership.
    VerifyOwnership {
        #[arg(long, default_value = DEFAULT_OWNERSHIP_PROOF_PATH)]
        proof: String,
        #[arg(long, default_value = "regtest")]
        network: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::BuildWitness { snapshot, output } => {
            prover::generate_witness(&snapshot, &output)?;
        }
        Commands::Prove { snapshot, output } => {
            prover::execute_circuit(&snapshot, &output)?;
        }
        Commands::Verify => {
            verifier::check_circuit()?;
        }
        Commands::SnapshotRegtest {
            wallet,
            output,
            threshold_sats,
            min_confirmations,
            bitcoin_cli,
        } => {
            regtest::generate_regtest_snapshot(&regtest::RegtestSnapshotOptions {
                bitcoin_cli,
                wallet,
                output,
                threshold_sats,
                min_confirmations,
            })?;
        }
        Commands::SignOwnership {
            snapshot,
            output,
            network,
            nonce,
            wif,
            wif_files,
            wif_envs,
        } => {
            ownership::sign_ownership(&ownership::SignOwnershipOptions {
                snapshot,
                output,
                network: parse_network(&network)?,
                nonce,
                wifs: wif,
                wif_files,
                wif_envs,
            })?;
        }
        Commands::VerifyOwnership { proof, network } => {
            ownership::verify_ownership_file(&proof, parse_network(&network)?)?;
        }
    }

    Ok(())
}

fn parse_network(network: &str) -> anyhow::Result<Network> {
    Network::from_str(network).map_err(|error| anyhow::anyhow!("invalid network: {error}"))
}
