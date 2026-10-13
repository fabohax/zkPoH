//! Library API for zkPoH witness generation, ownership proofs, and circuit helpers.

pub mod ownership;
pub mod prover;
pub mod regtest;
pub mod snapshot_builder;
pub mod verifier;

pub use ownership::{
    verify_ownership_file, verify_ownership_proof, OwnershipProof, SignOwnershipOptions,
    UtxoOwnershipSignature,
};
pub use prover::{execute_circuit, format_sats_as_btc, generate_witness, prove};
pub use regtest::{generate_regtest_snapshot, RegtestSnapshotOptions};
pub use snapshot_builder::{
    blake2s_digest, build_witness, empty_leaf, format_digest, hash_leaf, hash_pair, leaf_for,
    load_snapshot, txid_tag, verify_path, write_prover_toml, HashDigest, MerkleTree, UtxoEntry,
    UtxoSnapshot, WitnessInput, DIGEST_BYTES, LEAF_PREIMAGE_BYTES, MAX_LEAVES, MERKLE_DEPTH,
    NODE_PREIMAGE_BYTES, SELECTED_UTXOS,
};
pub use verifier::{check_circuit, verify_proof};
