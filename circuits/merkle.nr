global DIGEST_BYTES: u32 = 32;
global NODE_PREIMAGE_BYTES: u32 = 64;

fn node_preimage(
    left: [u8; DIGEST_BYTES],
    right: [u8; DIGEST_BYTES],
) -> [u8; NODE_PREIMAGE_BYTES] {
    let mut input: [u8; NODE_PREIMAGE_BYTES] = [0; NODE_PREIMAGE_BYTES];

    for i in 0..DIGEST_BYTES {
        input[i] = left[i];
        input[i + DIGEST_BYTES] = right[i];
    }

    input
}

pub fn hash_pair(left: [u8; DIGEST_BYTES], right: [u8; DIGEST_BYTES]) -> [u8; DIGEST_BYTES] {
    std::hash::blake2s(node_preimage(left, right))
}

pub fn verify_merkle_path(
    leaf: [u8; DIGEST_BYTES],
    root: [u8; DIGEST_BYTES],
    path: [[u8; DIGEST_BYTES]; 2],
    index: u64,
) {
    let first_level = if index % 2 == 0 {
        hash_pair(leaf, path[0])
    } else {
        hash_pair(path[0], leaf)
    };

    let computed_root = if index < 2 {
        hash_pair(first_level, path[1])
    } else {
        assert(index < 4);
        hash_pair(path[1], first_level)
    };

    assert(computed_root == root);
}
