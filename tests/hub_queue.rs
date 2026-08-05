// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Bounded per-user workspace admission for the ADR-0096 service.

use ghostlight::hub::peer::PeerUser;
use ghostlight::hub::{try_mint, MintQuota, MINT_QUOTA_EXCEEDED, PER_PEER_MINT_CAP};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[test]
fn per_peer_mint_cap_denies_a_flooding_peer_without_locking_out_others() {
    let quota: MintQuota = Arc::new(Mutex::new(HashMap::new()));
    let peer_a = PeerUser("peer-a".to_string());
    let peer_b = PeerUser("peer-b".to_string());
    let mut held = Vec::new();

    for _ in 0..PER_PEER_MINT_CAP {
        held.push(try_mint(&quota, &peer_a).expect("mint up to the per-user cap"));
    }
    assert_eq!(
        try_mint(&quota, &peer_a).err(),
        Some(MINT_QUOTA_EXCEEDED.to_string())
    );
    assert!(try_mint(&quota, &peer_b).is_ok());

    held.pop();
    assert!(try_mint(&quota, &peer_a).is_ok());
}
