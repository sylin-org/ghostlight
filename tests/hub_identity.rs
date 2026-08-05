// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Workspace identity and same-user admission tests for ADR-0096.

use ghostlight::hub::peer::{PeerCred, PeerUser};
use ghostlight::hub::workspace::WorkspaceRegistry;
use ghostlight_transport::workspace_id::WorkspaceId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn registry() -> WorkspaceRegistry {
    WorkspaceRegistry::new(Arc::new(Mutex::new(HashMap::new())))
}

#[test]
fn workspace_id_is_csprng_v4_and_never_formats_as_bearer_material() {
    let first = WorkspaceId::mint();
    let second = WorkspaceId::mint();
    assert_ne!(first.as_str(), second.as_str());
    assert_eq!(WorkspaceId::parse(first.as_str()), Some(first.clone()));
    assert!(!format!("{first}").contains(first.as_str()));
    assert!(!format!("{first:?}").contains(first.as_str()));
}

#[test]
fn workspace_authority_is_same_user_and_never_process_id() {
    let registry = registry();
    let original = PeerCred {
        user: PeerUser("user-a".into()),
        pid: 100,
    };
    let same_user_new_process = PeerCred {
        user: PeerUser("user-a".into()),
        pid: 999,
    };
    let other_user = PeerCred {
        user: PeerUser("user-b".into()),
        pid: 100,
    };
    let workspace = registry.mint(&original.user, false).unwrap();

    assert!(registry.contains(&workspace, &same_user_new_process.user));
    assert!(!registry.contains(&workspace, &other_user.user));
}
