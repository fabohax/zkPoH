use crate::prover::{run, PROOF_DIR};
use crate::snapshot_builder::{load_snapshot, snapshot_root, HashDigest, DIGEST_BYTES};
use sha2::{Digest as _, Sha256};
use std::fs;
use std::process::Command;

/// Where the SHA-256 of the trusted verification key is checked in. `make update-trusted-vk`
/// rewrites it after an intentional circuit change.
pub const TRUSTED_VK_PATH: &str = "trusted_vk.sha256";

/// `bb` writes each public input as one big-endian 32-byte field element.
pub const PUBLIC_INPUT_FIELD_BYTES: usize = 32;
/// `merkle_root: [u8; 32]` is 32 fields (one byte each), `threshold_sats: u64` is one more.
pub const PUBLIC_INPUT_FIELDS: usize = DIGEST_BYTES + 1;
pub const PUBLIC_INPUTS_BYTES: usize = PUBLIC_INPUT_FIELDS * PUBLIC_INPUT_FIELD_BYTES;

/// The public statement a verifier expects a proof to be about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpectedStatement {
    pub merkle_root: HashDigest,
    pub threshold_sats: u64,
}

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

/// The statement a proof over `snapshot_path` must carry: the snapshot's Merkle root and
/// its `threshold_sats`.
pub fn expected_statement(snapshot_path: &str) -> anyhow::Result<ExpectedStatement> {
    let snapshot = load_snapshot(snapshot_path)?;
    Ok(ExpectedStatement {
        merkle_root: snapshot_root(&snapshot)?,
        threshold_sats: snapshot.threshold_sats,
    })
}

/// Reads the trusted vk digest from `TRUSTED_VK_PATH`.
pub fn trusted_vk_sha256() -> anyhow::Result<String> {
    let text = fs::read_to_string(TRUSTED_VK_PATH)
        .map_err(|error| anyhow::anyhow!("cannot read {TRUSTED_VK_PATH}: {error}"))?;
    let digest = text.trim();
    if digest.len() != 64 || !digest.chars().all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!("{TRUSTED_VK_PATH} must hold one 64-hex-digit SHA-256");
    }
    Ok(digest.to_ascii_lowercase())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Decodes `bb`'s `public_inputs` file into the statement it carries.
pub fn decode_public_inputs(bytes: &[u8]) -> anyhow::Result<ExpectedStatement> {
    if bytes.len() != PUBLIC_INPUTS_BYTES {
        anyhow::bail!(
            "public_inputs is {} bytes; expected {PUBLIC_INPUTS_BYTES} ({PUBLIC_INPUT_FIELDS} fields)",
            bytes.len()
        );
    }
    let (fields, remainder) = bytes.as_chunks::<PUBLIC_INPUT_FIELD_BYTES>();
    debug_assert!(remainder.is_empty());
    let mut merkle_root = [0_u8; DIGEST_BYTES];
    let mut threshold_sats = 0_u64;
    for (i, field) in fields.iter().enumerate() {
        if i < DIGEST_BYTES {
            // One root byte per field: the top 31 bytes must be zero.
            if field[..PUBLIC_INPUT_FIELD_BYTES - 1]
                .iter()
                .any(|&b| b != 0)
            {
                anyhow::bail!("public input {i} is not a byte");
            }
            merkle_root[i] = field[PUBLIC_INPUT_FIELD_BYTES - 1];
        } else {
            if field[..PUBLIC_INPUT_FIELD_BYTES - 8]
                .iter()
                .any(|&b| b != 0)
            {
                anyhow::bail!("public input {i} is not a u64");
            }
            let mut be = [0_u8; 8];
            be.copy_from_slice(&field[PUBLIC_INPUT_FIELD_BYTES - 8..]);
            threshold_sats = u64::from_be_bytes(be);
        }
    }
    Ok(ExpectedStatement {
        merkle_root,
        threshold_sats,
    })
}

/// Fails unless the proof's public inputs are exactly `expected`.
pub fn check_public_inputs(bytes: &[u8], expected: &ExpectedStatement) -> anyhow::Result<()> {
    let actual = decode_public_inputs(bytes)?;
    if actual.merkle_root != expected.merkle_root {
        anyhow::bail!(
            "proof is for merkle root {} but the snapshot root is {}",
            hex::encode(actual.merkle_root),
            hex::encode(expected.merkle_root)
        );
    }
    if actual.threshold_sats != expected.threshold_sats {
        anyhow::bail!(
            "proof is for threshold {} sats but the snapshot requires {}",
            actual.threshold_sats,
            expected.threshold_sats
        );
    }
    Ok(())
}

/// Fails unless `vk_bytes` hashes to the trusted digest.
pub fn check_vk(vk_bytes: &[u8], trusted_sha256: &str) -> anyhow::Result<()> {
    let actual = sha256_hex(vk_bytes);
    if actual != trusted_sha256.to_ascii_lowercase() {
        anyhow::bail!(
            "verification key sha256 {actual} is not the trusted {trusted_sha256}; \
             if the circuit changed on purpose, run `make update-trusted-vk`"
        );
    }
    Ok(())
}

/// Verifies the files `prove` wrote in `target/`: the verification key must be the trusted
/// one, the public inputs must match `expected`, and then `bb verify` must accept the proof.
/// `bb verify` alone checks only that the proof matches whatever inputs and key it is given.
pub fn verify_proof(expected: &ExpectedStatement, trusted_vk_sha256: &str) -> anyhow::Result<()> {
    let proof = format!("{PROOF_DIR}/proof");
    let vk = format!("{PROOF_DIR}/vk");
    let public_inputs = format!("{PROOF_DIR}/public_inputs");

    check_vk(&fs::read(&vk)?, trusted_vk_sha256)?;
    println!("verification key matches {TRUSTED_VK_PATH}");
    check_public_inputs(&fs::read(&public_inputs)?, expected)?;
    println!(
        "public inputs match: merkle_root = {}, threshold_sats = {}",
        hex::encode(expected.merkle_root),
        expected.threshold_sats
    );
    run(
        "bb",
        &["verify", "-p", &proof, "-k", &vk, "-i", &public_inputs],
    )?;
    println!("proof verified");
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

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(root: &HashDigest, threshold_sats: u64) -> Vec<u8> {
        let mut out = Vec::with_capacity(PUBLIC_INPUTS_BYTES);
        for &byte in root {
            let mut field = [0_u8; PUBLIC_INPUT_FIELD_BYTES];
            field[PUBLIC_INPUT_FIELD_BYTES - 1] = byte;
            out.extend_from_slice(&field);
        }
        let mut field = [0_u8; PUBLIC_INPUT_FIELD_BYTES];
        field[PUBLIC_INPUT_FIELD_BYTES - 8..].copy_from_slice(&threshold_sats.to_be_bytes());
        out.extend_from_slice(&field);
        out
    }

    fn statement() -> ExpectedStatement {
        let mut merkle_root = [0_u8; DIGEST_BYTES];
        for (i, byte) in merkle_root.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(7).wrapping_add(1);
        }
        ExpectedStatement {
            merkle_root,
            threshold_sats: 100_000_000,
        }
    }

    #[test]
    fn public_inputs_round_trip() {
        let expected = statement();
        let bytes = encode(&expected.merkle_root, expected.threshold_sats);
        assert_eq!(bytes.len(), 1056);
        assert_eq!(decode_public_inputs(&bytes).unwrap(), expected);
        check_public_inputs(&bytes, &expected).unwrap();
    }

    #[test]
    fn mismatched_root_or_threshold_rejected() {
        let expected = statement();
        let mut other_root = expected.merkle_root;
        other_root[31] ^= 1;
        let error = check_public_inputs(&encode(&other_root, 100_000_000), &expected).unwrap_err();
        assert!(error.to_string().contains("merkle root"), "{error}");

        let error =
            check_public_inputs(&encode(&expected.merkle_root, 99_999_999), &expected).unwrap_err();
        assert!(error.to_string().contains("threshold"), "{error}");
    }

    #[test]
    fn malformed_public_inputs_rejected() {
        let expected = statement();
        assert!(decode_public_inputs(&[0_u8; 1055]).is_err());
        let mut bytes = encode(&expected.merkle_root, expected.threshold_sats);
        bytes[0] = 1; // high byte of field 0 set: not a byte-sized value
        assert!(decode_public_inputs(&bytes).is_err());
        let mut bytes = encode(&expected.merkle_root, expected.threshold_sats);
        bytes[PUBLIC_INPUTS_BYTES - 9] = 1; // byte above the u64 in the threshold field
        assert!(decode_public_inputs(&bytes).is_err());
    }

    #[test]
    fn untrusted_vk_rejected() {
        let vk = b"not a real vk";
        let trusted = sha256_hex(vk);
        check_vk(vk, &trusted).unwrap();
        check_vk(vk, &trusted.to_ascii_uppercase()).unwrap();
        assert!(check_vk(b"another vk", &trusted).is_err());
    }
}
