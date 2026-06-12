global SELECTED_UTXOS: u32 = 2;
global MERKLE_DEPTH: u32 = 1;
global DIGEST_BYTES: u32 = 32;
global LEAF_PREIMAGE_BYTES: u32 = 24;
global NODE_PREIMAGE_BYTES: u32 = 64;
global THRESHOLD_SATS: u64 = 100000000;

fn u64_to_be_bytes(value: u64) -> [u8; 8] {
    [
        (value >> 56) as u8,
        (value >> 48) as u8,
        (value >> 40) as u8,
        (value >> 32) as u8,
        (value >> 24) as u8,
        (value >> 16) as u8,
        (value >> 8) as u8,
        value as u8,
    ]
}

fn leaf_preimage(txid_tag: u64, vout: u64, value: u64) -> [u8; LEAF_PREIMAGE_BYTES] {
    let txid_bytes = u64_to_be_bytes(txid_tag);
    let vout_bytes = u64_to_be_bytes(vout);
    let value_bytes = u64_to_be_bytes(value);
    let mut input: [u8; LEAF_PREIMAGE_BYTES] = [0; LEAF_PREIMAGE_BYTES];

    for i in 0..8 {
        input[i] = txid_bytes[i];
        input[i + 8] = vout_bytes[i];
        input[i + 16] = value_bytes[i];
    }

    input
}

fn node_preimage(left: [u8; DIGEST_BYTES], right: [u8; DIGEST_BYTES]) -> [u8; NODE_PREIMAGE_BYTES] {
    let mut input: [u8; NODE_PREIMAGE_BYTES] = [0; NODE_PREIMAGE_BYTES];

    for i in 0..DIGEST_BYTES {
        input[i] = left[i];
        input[i + DIGEST_BYTES] = right[i];
    }

    input
}

fn hash_leaf(txid_tag: u64, vout: u64, value: u64) -> [u8; DIGEST_BYTES] {
    std::hash::blake2s(leaf_preimage(txid_tag, vout, value))
}

fn hash_pair(left: [u8; DIGEST_BYTES], right: [u8; DIGEST_BYTES]) -> [u8; DIGEST_BYTES] {
    std::hash::blake2s(node_preimage(left, right))
}

fn verify_merkle_path(
    leaf: [u8; DIGEST_BYTES],
    root: [u8; DIGEST_BYTES],
    path: [[u8; DIGEST_BYTES]; MERKLE_DEPTH],
    index: u64,
) {
    let computed_root = if index == 0 {
        hash_pair(leaf, path[0])
    } else {
        assert(index == 1);
        hash_pair(path[0], leaf)
    };

    assert(computed_root == root);
}

fn main(
    merkle_root: pub [u8; DIGEST_BYTES],
    txid_tags: [u64; SELECTED_UTXOS],
    vouts: [u64; SELECTED_UTXOS],
    values: [u64; SELECTED_UTXOS],
    merkle_paths: [[[u8; DIGEST_BYTES]; MERKLE_DEPTH]; SELECTED_UTXOS],
    merkle_indices: [u64; SELECTED_UTXOS],
) {
    let leaf_0 = hash_leaf(txid_tags[0], vouts[0], values[0]);
    let leaf_1 = hash_leaf(txid_tags[1], vouts[1], values[1]);

    verify_merkle_path(leaf_0, merkle_root, merkle_paths[0], merkle_indices[0]);
    verify_merkle_path(leaf_1, merkle_root, merkle_paths[1], merkle_indices[1]);

    let total_value = values[0] + values[1];
    assert(total_value >= THRESHOLD_SATS);
}

#[test]
fn valid_two_leaf_witness_passes() {
    let leaf_0 = hash_leaf(1, 0, 42000000);
    let leaf_1 = hash_leaf(2, 1, 58000000);
    let root = hash_pair(leaf_0, leaf_1);

    main(
        root,
        [1, 2],
        [0, 1],
        [42000000, 58000000],
        [[leaf_1], [leaf_0]],
        [0, 1],
    );
}

#[test(should_fail)]
fn below_threshold_witness_fails() {
    let leaf_0 = hash_leaf(1, 0, 41000000);
    let leaf_1 = hash_leaf(2, 1, 58000000);
    let root = hash_pair(leaf_0, leaf_1);

    main(
        root,
        [1, 2],
        [0, 1],
        [41000000, 58000000],
        [[leaf_1], [leaf_0]],
        [0, 1],
    );
}

#[test(should_fail)]
fn wrong_merkle_path_fails() {
    let leaf_0 = hash_leaf(1, 0, 42000000);
    let leaf_1 = hash_leaf(2, 1, 58000000);
    let root = hash_pair(leaf_0, leaf_1);

    main(
        root,
        [1, 2],
        [0, 1],
        [42000000, 58000000],
        [[leaf_0], [leaf_0]],
        [0, 1],
    );
}
