use serde_json::json;

use zstash_core::domain::{BackupAction, ShieldAction, SyncStatus, WalletLockStatus, WalletStatus};
use zstash_core::errors;
use zstash_core::ipc::v1::commands::balance::GetBalanceResponse;
use zstash_core::ipc::v1::commands::wallet::{
    CreateWalletRequest, CreateWalletResponse, ViewSeedPhraseResponse,
};
use zstash_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};

#[test]
fn schema_version_enforcement() {
    ensure_schema_version(SCHEMA_VERSION).expect("current schema version should be accepted");
    let err = ensure_schema_version(SCHEMA_VERSION + 1).expect_err("mismatch should be rejected");
    assert_eq!(err.code, errors::SCHEMA_VERSION_MISMATCH);
}

#[test]
fn deny_unknown_fields_on_requests() {
    let request = json!({
        "schema_version": SCHEMA_VERSION,
        "name": "Test",
        "network": "Testnet",
        "password": "pw",
        "remember_unlock": false,
        "extra": "nope"
    });

    let decoded: Result<CreateWalletRequest, _> = serde_json::from_value(request);
    assert!(decoded.is_err(), "unknown fields must be rejected");
}

#[test]
fn enum_json_shapes_match_contract() {
    let syncing = serde_json::to_value(SyncStatus::Syncing {
        progress_percent: 42,
    })
    .unwrap();
    assert_eq!(syncing, json!({ "Syncing": { "progress_percent": 42 } }));

    let shield = serde_json::to_value(ShieldAction::Available {
        amount: "1".to_string(),
    })
    .unwrap();
    assert_eq!(shield, json!({ "Available": { "amount": "1" } }));

    let backup = serde_json::to_value(BackupAction::Required).unwrap();
    assert_eq!(backup, json!("Required"));
}

#[test]
fn seed_phrase_only_in_allowed_backend_payloads() {
    let create_wallet = CreateWalletResponse {
        schema_version: SCHEMA_VERSION,
        wallet: zstash_core::domain::WalletInfo {
            id: uuid::Uuid::nil(),
            name: "w".to_string(),
            wallet_type: zstash_core::domain::WalletType::Software,
            network: zstash_core::domain::Network::Testnet,
            remember_unlock_enabled: false,
            created_at: 0,
            last_opened_at: None,
        },
        seed_phrase: vec!["abandon".to_string(); 24],
        backup_challenge: zstash_core::ipc::v1::commands::wallet::BackupChallenge {
            challenge_id: "c".to_string(),
            indices: vec![1, 2, 3, 4],
            expires_at: 0,
        },
    };

    let view_seed = ViewSeedPhraseResponse {
        schema_version: SCHEMA_VERSION,
        seed_phrase: vec!["abandon".to_string(); 24],
    };

    let get_balance = GetBalanceResponse {
        schema_version: SCHEMA_VERSION,
        balance: zstash_core::domain::Balance {
            shielded_spendable: "0".to_string(),
            shielded_pending: "0".to_string(),
            transparent_total: "0".to_string(),
            total: "0".to_string(),
        },
    };

    let wallet_status = WalletStatus {
        lock_status: WalletLockStatus::Locked,
        backup_status: BackupAction::Required,
        sync_status: SyncStatus::Synced,
        shield_status: ShieldAction::None,
        privacy_posture: zstash_core::domain::PrivacyPosture::Optimal,
    };

    let create_json = serde_json::to_string(&create_wallet).unwrap();
    assert!(create_json.contains("\"seed_phrase\""));

    let view_json = serde_json::to_string(&view_seed).unwrap();
    assert!(view_json.contains("\"seed_phrase\""));

    let balance_json = serde_json::to_string(&get_balance).unwrap();
    assert!(!balance_json.contains("\"seed_phrase\""));

    let status_json = serde_json::to_string(&wallet_status).unwrap();
    assert!(!status_json.contains("\"seed_phrase\""));
    assert!(!status_json.to_lowercase().contains("mnemonic"));
}
