mod prover;
mod snapshot_builder;
mod verifier;

use clap::{Parser, Subcommand};

const DEFAULT_SNAPSHOT_PATH: &str = "snapshots/utxo_snapshot.json";
const DEFAULT_PROVER_TOML_PATH: &str = "Prover.toml";

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
    }

    Ok(())
}
