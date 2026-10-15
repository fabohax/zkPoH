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
    blake2s_digest, build_witness, check_distinct_outpoints, empty_leaf, format_digest, hash_leaf,
    hash_pair, leaf_for, load_snapshot, snapshot_root, snapshot_tree, txid_tag, verify_path,
    write_prover_toml, HashDigest, MerkleTree, UtxoEntry, UtxoSnapshot, WitnessInput, DIGEST_BYTES,
    LEAF_PREIMAGE_BYTES, MAX_LEAVES, MERKLE_DEPTH, NODE_PREIMAGE_BYTES, SELECTED_UTXOS,
};
pub use verifier::{
    check_circuit, check_public_inputs, check_vk, decode_public_inputs, expected_statement,
    sha256_hex, trusted_vk_sha256, verify_proof, ExpectedStatement, PUBLIC_INPUTS_BYTES,
    PUBLIC_INPUT_FIELDS, PUBLIC_INPUT_FIELD_BYTES, TRUSTED_VK_PATH,
};
