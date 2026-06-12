use std::process::Command;

pub fn check_circuit() -> anyhow::Result<()> {
    let status = Command::new("nargo").arg("check").status()?;
    if !status.success() {
        anyhow::bail!("nargo check failed");
    }

    println!("circuit checked successfully");
    Ok(())
}
