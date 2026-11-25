//! Pure building blocks for #1216 (Merkle airdrop), #1220 (lending vault),
//! #1222 (permit approvals) and #1223 (key flagging).
//!
//! These helpers hold no storage and expose no entrypoints. The entrypoints
//! for each issue will build on them in follow-up PRs.

use soroban_sdk::{xdr::ToXdr, Address, Bytes, BytesN, Env, Vec};

/// #1216: leaf hash for a claim, `sha256(xdr(claimer) || amount_be)`.
pub fn airdrop_leaf(env: &Env, claimer: &Address, amount: i128) -> BytesN<32> {
    let mut data = claimer.clone().to_xdr(env);
    data.extend_from_array(&amount.to_be_bytes());
    env.crypto().sha256(&data).to_bytes()
}

/// #1216: verifies a Merkle proof using sorted-pair SHA-256 hashing.
pub fn verify_merkle_proof(
    env: &Env,
    root: &BytesN<32>,
    leaf: BytesN<32>,
    proof: &Vec<BytesN<32>>,
) -> bool {
    let mut node = leaf;
    for sibling in proof.iter() {
        let (a, b) = if node <= sibling {
            (node, sibling)
        } else {
            (sibling, node)
        };
        let mut buf = Bytes::from_array(env, &a.to_array());
        buf.extend_from_array(&b.to_array());
        node = env.crypto().sha256(&buf).to_bytes();
    }
    node == *root
}

/// #1220: maximum borrow for `collateral_value` at `max_ltv_bps`.
pub fn max_borrow(collateral_value: i128, max_ltv_bps: u32) -> Option<i128> {
    collateral_value
        .checked_mul(max_ltv_bps as i128)
        .map(|v| v / 10_000)
}

/// #1220: a position is healthy while `collateral * ltv >= debt`.
pub fn is_healthy(collateral_value: i128, debt: i128, max_ltv_bps: u32) -> bool {
    max_borrow(collateral_value, max_ltv_bps).is_some_and(|max| max >= debt)
}

/// #1222: a permit is valid until its deadline, inclusive.
pub fn permit_not_expired(now: u64, deadline: u64) -> bool {
    deadline >= now
}

/// #1223: flagging freezes a key once unique flaggers reach the threshold.
pub fn flag_threshold_reached(unique_flaggers: u32, flag_threshold: u32) -> bool {
    flag_threshold > 0 && unique_flaggers >= flag_threshold
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn pair(env: &Env, x: &BytesN<32>, y: &BytesN<32>) -> BytesN<32> {
        let (a, b) = if x <= y { (x, y) } else { (y, x) };
        let mut buf = Bytes::from_array(env, &a.to_array());
        buf.extend_from_array(&b.to_array());
        env.crypto().sha256(&buf).to_bytes()
    }

    #[test]
    fn merkle_proof_accepts_valid_and_rejects_tampered() {
        let env = Env::default();
        let (u1, u2) = (Address::generate(&env), Address::generate(&env));
        let (l1, l2) = (airdrop_leaf(&env, &u1, 5), airdrop_leaf(&env, &u2, 7));
        let root = pair(&env, &l1, &l2);
        let proof = Vec::from_array(&env, [l2.clone()]);
        assert!(verify_merkle_proof(&env, &root, l1, &proof));
        let wrong = airdrop_leaf(&env, &u1, 6);
        assert!(!verify_merkle_proof(&env, &root, wrong, &proof));
    }

    #[test]
    fn vault_ltv_and_health() {
        assert_eq!(max_borrow(1_000, 5_000), Some(500));
        assert!(is_healthy(1_000, 500, 5_000));
        assert!(!is_healthy(1_000, 501, 5_000));
        assert_eq!(max_borrow(i128::MAX, 5_000), None);
    }

    #[test]
    fn permit_deadline_and_flag_threshold() {
        assert!(permit_not_expired(10, 10));
        assert!(!permit_not_expired(11, 10));
        assert!(flag_threshold_reached(3, 3));
        assert!(!flag_threshold_reached(2, 3));
        assert!(!flag_threshold_reached(1, 0));
    }
}
