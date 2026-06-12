use crate::snapshot_builder::{build_witness, format_digest, load_snapshot, UtxoEntry};
use bitcoin::secp256k1::{ecdsa::Signature, Message, Secp256k1};
use bitcoin::{Address, CompressedPublicKey, Network, PrivateKey};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct SignOwnershipOptions {
    pub snapshot: String,
    pub output: String,
    pub network: Network,
    pub nonce: Option<String>,
    pub wifs: Vec<String>,
    pub wif_files: Vec<String>,
    pub wif_envs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OwnershipProof {
    pub version: u32,
    pub snapshot: String,
    pub threshold_sats: u64,
    pub merkle_root: Vec<u8>,
    pub challenge: String,
    pub challenge_sha256: String,
    pub generated_at: String,
    pub signatures: Vec<UtxoOwnershipSignature>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UtxoOwnershipSignature {
    pub txid: String,
    pub vout: u32,
    pub value: u64,
    pub address: String,
    pub public_key: String,
    pub signature_der: String,
}

struct SigningKey {
    private_key: PrivateKey,
    public_key: CompressedPublicKey,
    address: Address,
}

pub fn sign_ownership(options: &SignOwnershipOptions) -> anyhow::Result<()> {
    let snapshot = load_snapshot(&options.snapshot)?;
    let witness = build_witness(&snapshot)?;
    let nonce = match &options.nonce {
        Some(nonce) => nonce.clone(),
        None => random_nonce(),
    };
    let challenge = ownership_challenge(
        &snapshot.snapshot,
        snapshot.threshold_sats,
        &format_digest(&witness.merkle_root),
        &nonce,
        &snapshot.utxos,
    );
    let challenge_digest = challenge_digest(&challenge);
    let keys = load_signing_keys(options)?;

    if keys.is_empty() {
        anyhow::bail!("no WIF private keys provided; use --wif-file, --wif-env, or --wif");
    }

    let secp = Secp256k1::new();
    let message = Message::from_digest(challenge_digest);
    let mut signatures = Vec::with_capacity(snapshot.utxos.len());

    for utxo in &snapshot.utxos {
        let key = keys
            .iter()
            .find(|key| key.address.to_string() == utxo.address)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "no provided WIF matches UTXO address {} ({}:{})",
                    utxo.address,
                    utxo.txid,
                    utxo.vout
                )
            })?;
        let signature = secp.sign_ecdsa(&message, &key.private_key.inner);

        secp.verify_ecdsa(&message, &signature, &key.public_key.0)?;

        signatures.push(UtxoOwnershipSignature {
            txid: utxo.txid.clone(),
            vout: utxo.vout,
            value: utxo.value,
            address: utxo.address.clone(),
            public_key: key.public_key.to_string(),
            signature_der: hex::encode(signature.serialize_der()),
        });
    }

    let proof = OwnershipProof {
        version: 1,
        snapshot: snapshot.snapshot,
        threshold_sats: snapshot.threshold_sats,
        merkle_root: witness.merkle_root.to_vec(),
        challenge,
        challenge_sha256: hex::encode(challenge_digest),
        generated_at: unix_timestamp_label()?,
        signatures,
    };

    verify_ownership_proof(&proof, options.network)?;

    let data = serde_json::to_string_pretty(&proof)?;
    fs::write(&options.output, format!("{data}\n"))?;

    println!("wrote ownership proof to {}", options.output);
    println!("signed_utxos = {}", proof.signatures.len());
    println!("challenge_sha256 = {}", proof.challenge_sha256);

    Ok(())
}

pub fn verify_ownership_file(path: &str, network: Network) -> anyhow::Result<()> {
    let data = fs::read_to_string(path)?;
    let proof = serde_json::from_str(&data)?;
    verify_ownership_proof(&proof, network)?;

    println!("ownership proof verified: {path}");
    Ok(())
}

pub fn verify_ownership_proof(proof: &OwnershipProof, network: Network) -> anyhow::Result<()> {
    let secp = Secp256k1::verification_only();
    let digest = challenge_digest(&proof.challenge);
    let message = Message::from_digest(digest);

    if hex::encode(digest) != proof.challenge_sha256 {
        anyhow::bail!("challenge_sha256 does not match challenge content");
    }

    for signature in &proof.signatures {
        let public_key = CompressedPublicKey::from_str(&signature.public_key)?;
        let address = Address::p2wpkh(&public_key, network);
        if address.to_string() != signature.address {
            anyhow::bail!(
                "public key does not match address {} for {}:{}",
                signature.address,
                signature.txid,
                signature.vout
            );
        }

        let signature_bytes = hex::decode(&signature.signature_der)?;
        let signature = Signature::from_der(&signature_bytes)?;
        secp.verify_ecdsa(&message, &signature, &public_key.0)?;
    }

    Ok(())
}

fn load_signing_keys(options: &SignOwnershipOptions) -> anyhow::Result<Vec<SigningKey>> {
    let mut wifs = options.wifs.clone();

    for file in &options.wif_files {
        let contents = fs::read_to_string(file)?;
        wifs.extend(
            contents
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(ToOwned::to_owned),
        );
    }

    for env_name in &options.wif_envs {
        let value = env::var(env_name)?;
        wifs.extend(
            value
                .split(',')
                .map(str::trim)
                .filter(|wif| !wif.is_empty())
                .map(ToOwned::to_owned),
        );
    }

    wifs.sort();
    wifs.dedup();

    let secp = Secp256k1::new();
    wifs.into_iter()
        .map(|wif| {
            let private_key = PrivateKey::from_wif(&wif)?;
            if private_key.network != options.network.into() {
                anyhow::bail!(
                    "WIF network {:?} does not match requested network {:?}",
                    private_key.network,
                    options.network
                );
            }
            let public_key = CompressedPublicKey::from_private_key(&secp, &private_key)?;
            let address = Address::p2wpkh(&public_key, options.network);
            Ok(SigningKey {
                private_key,
                public_key,
                address,
            })
        })
        .collect()
}

fn ownership_challenge(
    snapshot_name: &str,
    threshold_sats: u64,
    merkle_root: &str,
    nonce: &str,
    utxos: &[UtxoEntry],
) -> String {
    let mut lines = vec![
        "zkPoH ownership challenge v1".to_string(),
        format!("snapshot={snapshot_name}"),
        format!("threshold_sats={threshold_sats}"),
        format!("merkle_root={merkle_root}"),
        format!("nonce={nonce}"),
        format!("utxo_count={}", utxos.len()),
    ];

    for (index, utxo) in utxos.iter().enumerate() {
        lines.push(format!(
            "utxo[{index}]={}:{}:{}:{}",
            utxo.txid, utxo.vout, utxo.value, utxo.address
        ));
    }

    lines.join("\n")
}

fn challenge_digest(challenge: &str) -> [u8; 32] {
    Sha256::digest(challenge.as_bytes()).into()
}

fn random_nonce() -> String {
    let mut nonce = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut nonce);
    hex::encode(nonce)
}

fn unix_timestamp_label() -> anyhow::Result<String> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    Ok(format!("unix:{timestamp}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_is_deterministic_for_same_inputs() {
        let utxos = vec![UtxoEntry {
            txid: "a".repeat(64),
            vout: 1,
            value: 42_000_000,
            address: "bcrt1qexample".to_string(),
        }];

        let first = ownership_challenge("snapshot", 100, "[1, 2]", "nonce", &utxos);
        let second = ownership_challenge("snapshot", 100, "[1, 2]", "nonce", &utxos);

        assert_eq!(first, second);
        assert_eq!(challenge_digest(&first), challenge_digest(&second));
    }
}
