// zkPoH: prove that SELECTED_UTXOS leaves of a snapshot Merkle tree are distinct,
// are members of the tree under `merkle_root`, and sum to at least `threshold_sats`.

global SELECTED_UTXOS: u32 = 4;
global MERKLE_DEPTH: u32 = 16;
global DIGEST_BYTES: u32 = 32;
global LEAF_PREIMAGE_BYTES: u32 = 24;
global NODE_PREIMAGE_BYTES: u32 = 64;
// 21,000,000 BTC in sats. Bounding each value keeps the u64 sum from wrapping.
global MAX_VALUE_SATS: u64 = 2100000000000000;

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

// Walks from `leaf` to the root. Bit i of `index` says whether the node at level i
// is the right child (1) or the left child (0). `path[i]` is the sibling at level i.
fn verify_merkle_path(
    leaf: [u8; DIGEST_BYTES],
    root: [u8; DIGEST_BYTES],
    path: [[u8; DIGEST_BYTES]; MERKLE_DEPTH],
    index: u64,
) {
    let mut node = leaf;
    let mut remaining = index;
    for i in 0..MERKLE_DEPTH {
        let is_right = (remaining & 1) == 1;
        if is_right {
            node = hash_pair(path[i], node);
        } else {
            node = hash_pair(node, path[i]);
        }
        remaining = remaining >> 1;
    }
    // index must be < 2^MERKLE_DEPTH: no bits may survive the walk.
    assert(remaining == 0, "index out of range");
    assert(node == root, "merkle root mismatch");
}

fn main(
    merkle_root: pub [u8; DIGEST_BYTES],
    threshold_sats: pub u64,
    txid_tags: [u64; SELECTED_UTXOS],
    vouts: [u64; SELECTED_UTXOS],
    values: [u64; SELECTED_UTXOS],
    merkle_paths: [[[u8; DIGEST_BYTES]; MERKLE_DEPTH]; SELECTED_UTXOS],
    merkle_indices: [u64; SELECTED_UTXOS],
) {
    let mut total_value: u64 = 0;
    for i in 0..SELECTED_UTXOS {
        assert(values[i] <= MAX_VALUE_SATS, "value exceeds supply");
        let leaf = hash_leaf(txid_tags[i], vouts[i], values[i]);
        verify_merkle_path(leaf, merkle_root, merkle_paths[i], merkle_indices[i]);
        total_value += values[i];
    }

    // No leaf may be counted twice: every pair of indices must differ, and no two
    // counted slots may name the same outpoint. The outpoint check catches a snapshot
    // that lists one output several times at different positions. Zero-value slots are
    // padding and add nothing to the total, so they are exempt from the outpoint check.
    for i in 0..SELECTED_UTXOS {
        for j in (i + 1)..SELECTED_UTXOS {
            assert(merkle_indices[i] != merkle_indices[j], "duplicate utxo");
            let both_counted = (values[i] != 0) & (values[j] != 0);
            let same_outpoint = (txid_tags[i] == txid_tags[j]) & (vouts[i] == vouts[j]);
            assert(!(both_counted & same_outpoint), "duplicate outpoint");
        }
    }

    assert(total_value >= threshold_sats, "below threshold");
}

// ---------------------------------------------------------------------------
// Tests. A small tree is built in-test: leaves at indices 0..3 hold the test
// UTXOs and empty leaves; every level above is computed the same way the Rust
// builder does it. `empty_tree_nodes()[i]` is the root of an all-empty subtree
// of height i.

fn empty_tree_nodes() -> [[u8; DIGEST_BYTES]; MERKLE_DEPTH] {
    let mut nodes: [[u8; DIGEST_BYTES]; MERKLE_DEPTH] = [[0; DIGEST_BYTES]; MERKLE_DEPTH];
    let mut node = hash_leaf(0, 0, 0);
    for i in 0..MERKLE_DEPTH {
        nodes[i] = node;
        node = hash_pair(node, node);
    }
    nodes
}

struct TestTree {
    root: [u8; DIGEST_BYTES],
    paths: [[[u8; DIGEST_BYTES]; MERKLE_DEPTH]; 4],
}

// Four leaves at indices 0..3; everything else empty.
fn four_leaf_tree(leaves: [[u8; DIGEST_BYTES]; 4]) -> TestTree {
    let empties = empty_tree_nodes();
    let node_0 = hash_pair(leaves[0], leaves[1]);
    let node_1 = hash_pair(leaves[2], leaves[3]);
    let mut node = hash_pair(node_0, node_1);
    let mut paths: [[[u8; DIGEST_BYTES]; MERKLE_DEPTH]; 4] = [[[0; DIGEST_BYTES]; MERKLE_DEPTH]; 4];
    paths[0][0] = leaves[1];
    paths[1][0] = leaves[0];
    paths[2][0] = leaves[3];
    paths[3][0] = leaves[2];
    paths[0][1] = node_1;
    paths[1][1] = node_1;
    paths[2][1] = node_0;
    paths[3][1] = node_0;
    for level in 2..MERKLE_DEPTH {
        for k in 0..4 {
            paths[k][level] = empties[level];
        }
        node = hash_pair(node, empties[level]);
    }
    TestTree { root: node, paths }
}

fn sample_leaves(value_0: u64) -> [[u8; DIGEST_BYTES]; 4] {
    let empty = hash_leaf(0, 0, 0);
    [hash_leaf(1, 0, value_0), hash_leaf(2, 1, 58000000), empty, empty]
}

#[test]
fn valid_padded_four_leaf_witness_passes() {
    let tree = four_leaf_tree(sample_leaves(42000000));
    main(
        tree.root,
        100000000,
        [1, 2, 0, 0],
        [0, 1, 0, 0],
        [42000000, 58000000, 0, 0],
        tree.paths,
        [0, 1, 2, 3],
    );
}

#[test]
fn threshold_is_a_public_input() {
    let tree = four_leaf_tree(sample_leaves(41000000));
    main(
        tree.root,
        99000000,
        [1, 2, 0, 0],
        [0, 1, 0, 0],
        [41000000, 58000000, 0, 0],
        tree.paths,
        [0, 1, 2, 3],
    );
}

#[test(should_fail_with = "below threshold")]
fn below_threshold_witness_fails() {
    let tree = four_leaf_tree(sample_leaves(41000000));
    main(
        tree.root,
        100000000,
        [1, 2, 0, 0],
        [0, 1, 0, 0],
        [41000000, 58000000, 0, 0],
        tree.paths,
        [0, 1, 2, 3],
    );
}

#[test(should_fail_with = "merkle root mismatch")]
fn wrong_merkle_path_fails() {
    let tree = four_leaf_tree(sample_leaves(42000000));
    let mut paths = tree.paths;
    paths[0][0] = paths[1][0]; // leaf_0's sibling replaced with leaf_0 itself
    main(
        tree.root,
        100000000,
        [1, 2, 0, 0],
        [0, 1, 0, 0],
        [42000000, 58000000, 0, 0],
        paths,
        [0, 1, 2, 3],
    );
}

#[test(should_fail_with = "duplicate utxo")]
fn duplicate_utxo_fails() {
    // leaf_1 (58,000,000 sats) placed in slots 1 and 2 with the same index and path.
    let tree = four_leaf_tree(sample_leaves(42000000));
    main(
        tree.root,
        100000000,
        [1, 2, 2, 0],
        [0, 1, 1, 0],
        [42000000, 58000000, 58000000, 0],
        [tree.paths[0], tree.paths[1], tree.paths[1], tree.paths[3]],
        [0, 1, 1, 3],
    );
}

#[test(should_fail_with = "duplicate outpoint")]
fn duplicate_outpoint_at_distinct_indices_fails() {
    // Regression for fabohax/zkPoH#3: one 0.3 BTC output listed four times in the
    // snapshot, at four distinct indices with four valid paths, sums to 1.2 BTC.
    let leaf = hash_leaf(7, 0, 30000000);
    let tree = four_leaf_tree([leaf, leaf, leaf, leaf]);
    main(
        tree.root,
        100000000,
        [7, 7, 7, 7],
        [0, 0, 0, 0],
        [30000000, 30000000, 30000000, 30000000],
        tree.paths,
        [0, 1, 2, 3],
    );
}

#[test]
fn zero_value_padding_slots_may_share_an_outpoint() {
    // Two padding slots both carry outpoint (0, 0); they are not counted.
    let tree = four_leaf_tree(sample_leaves(42000000));
    main(
        tree.root,
        100000000,
        [1, 2, 0, 0],
        [0, 1, 0, 0],
        [42000000, 58000000, 0, 0],
        tree.paths,
        [0, 1, 2, 3],
    );
}

#[test(should_fail_with = "index out of range")]
fn index_out_of_range_fails() {
    let tree = four_leaf_tree(sample_leaves(42000000));
    main(
        tree.root,
        100000000,
        [1, 2, 0, 0],
        [0, 1, 0, 0],
        [42000000, 58000000, 0, 0],
        tree.paths,
        [0, 1, 2, 3 + 65536],
    );
}

#[test(should_fail_with = "value exceeds supply")]
fn value_above_supply_fails() {
    let empty = hash_leaf(0, 0, 0);
    let tree = four_leaf_tree([
        hash_leaf(1, 0, 2100000000000001),
        hash_leaf(2, 1, 58000000),
        empty,
        empty,
    ]);
    main(
        tree.root,
        100000000,
        [1, 2, 0, 0],
        [0, 1, 0, 0],
        [2100000000000001, 58000000, 0, 0],
        tree.paths,
        [0, 1, 2, 3],
    );
}

// Shared test vectors. The Rust crate asserts the same bytes in
// `snapshot_builder::tests::hash_vectors_match_noir`.
#[test]
fn hash_vectors_match_rust() {
    let empty = hash_leaf(0, 0, 0);
    assert(
        empty
            == [
                0x10, 0x32, 0xc4, 0xe3, 0x7b, 0xf6, 0xcf, 0x09, 0x51, 0x69, 0xc2, 0x57, 0x66, 0x18,
                0x93, 0x89, 0xc7, 0x41, 0x02, 0x5d, 0x8f, 0x15, 0x73, 0x84, 0x8d, 0xb5, 0x3b, 0xa4,
                0x84, 0xc1, 0x07, 0xd3,
            ],
    );
    assert(
        hash_pair(empty, empty)
            == [
                0xc9, 0xf6, 0x83, 0x87, 0x46, 0x96, 0x6a, 0xc2, 0x08, 0x86, 0x24, 0x49, 0xce, 0xb1,
                0x3d, 0xf2, 0x8a, 0xdd, 0x06, 0x07, 0x9c, 0xc8, 0xe3, 0x24, 0x79, 0x85, 0x30, 0x80,
                0x0e, 0x95, 0x49, 0xe1,
            ],
    );
    let tree = four_leaf_tree(sample_leaves(42000000));
    assert(
        tree.root
            == [
                0xd0, 0x52, 0x89, 0x3a, 0xae, 0x2d, 0x5f, 0x26, 0x34, 0xdf, 0xd4, 0xaa, 0xa3, 0x6a,
                0xd2, 0xa0, 0xe8, 0x24, 0x0e, 0x3d, 0xc5, 0x78, 0x95, 0x01, 0xe3, 0xe8, 0x07, 0x62,
                0xe1, 0xf8, 0xdf, 0x4f,
            ],
    );
}
