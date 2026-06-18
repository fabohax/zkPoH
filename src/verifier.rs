use std::process::Command;

pub fn check_circuit() -> anyhow::Result<()> {
    let output = Command::new("nargo").arg("check").output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    print_without_existing_prover_note(&stdout);
    print_without_existing_prover_note(&stderr);

    if !output.status.success() {
        anyhow::bail!("nargo check failed");
    }

    println!("circuit checked successfully");
    Ok(())
}

fn print_without_existing_prover_note(output: &str) {
    for line in output.lines() {
        if line == "Note: Prover.toml already exists. Use --overwrite to force overwrite." {
            continue;
        }
        println!("{line}");
    }
}
