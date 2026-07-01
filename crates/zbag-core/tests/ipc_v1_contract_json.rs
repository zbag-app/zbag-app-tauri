use serde_json::json;

use zbag_core::domain::{
    BackupAction, ShieldAction, SupportedToken, SyncPhase, SyncProgress, SyncStatus,
    WalletLockStatus, WalletStatus,
};
use zbag_core::errors;
use zbag_core::ipc::v1::commands::backup::{RestoreWalletRequest, VerifyBackupRequest};
use zbag_core::ipc::v1::commands::balance::GetBalanceResponse;
use zbag_core::ipc::v1::commands::swap::{GetSupportedTokensRequest, GetSupportedTokensResponse};
use zbag_core::ipc::v1::commands::version::{GetVersionRequest, GetVersionResponse};
use zbag_core::ipc::v1::commands::wallet::{
    CreateWalletRequest, CreateWalletResponse, ViewSeedPhraseResponse,
};
use zbag_core::ipc::v1::common::{SCHEMA_VERSION, ensure_schema_version};
use zbag_core::version::VersionInfo;

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
    let offline_phase = serde_json::to_value(SyncPhase::Offline).unwrap();
    assert_eq!(offline_phase, json!("Offline"));

    let syncing = serde_json::to_value(SyncStatus::Syncing {
        progress_percent: 42,
    })
    .unwrap();
    assert_eq!(syncing, json!({ "Syncing": { "progress_percent": 42 } }));

    let offline = serde_json::to_value(SyncStatus::Offline {
        retry_in_seconds: 20,
    })
    .unwrap();
    assert_eq!(offline, json!({ "Offline": { "retry_in_seconds": 20 } }));

    let err = serde_json::to_value(SyncStatus::Error {
        message: "oops".to_string(),
    })
    .unwrap();
    assert_eq!(err, json!({ "Error": { "message": "oops" } }));

    let shield = serde_json::to_value(ShieldAction::Available {
        amount: "1".to_string(),
    })
    .unwrap();
    assert_eq!(shield, json!({ "Available": { "amount": "1" } }));

    let backup = serde_json::to_value(BackupAction::Required).unwrap();
    assert_eq!(backup, json!("Required"));
}

#[test]
fn sync_progress_retry_in_seconds_is_number_or_absent() {
    let no_retry = SyncProgress {
        phase: SyncPhase::Idle,
        scan_frontier_height: 0,
        wallet_tip_height: 0,
        progress_percent: 0,
        eta_seconds: None,
        retry_in_seconds: None,
        error_message: None,
    };
    let no_retry_json = serde_json::to_value(&no_retry).unwrap();
    assert!(
        no_retry_json.get("retry_in_seconds").is_none(),
        "retry_in_seconds must be omitted when None"
    );
    assert!(
        no_retry_json.get("error_message").is_none(),
        "error_message must be omitted when None"
    );

    let with_retry = SyncProgress {
        phase: SyncPhase::Offline,
        scan_frontier_height: 123,
        wallet_tip_height: 456,
        progress_percent: 42,
        eta_seconds: None,
        retry_in_seconds: Some(20),
        error_message: None,
    };
    let with_retry_json = serde_json::to_value(&with_retry).unwrap();
    assert_eq!(with_retry_json["retry_in_seconds"], json!(20));
    assert!(
        with_retry_json.get("error_message").is_none(),
        "error_message must be omitted when None"
    );

    let with_error = SyncProgress {
        phase: SyncPhase::Error,
        scan_frontier_height: 123,
        wallet_tip_height: 456,
        progress_percent: 42,
        eta_seconds: None,
        retry_in_seconds: Some(20),
        error_message: Some("Failed to scan blocks".to_string()),
    };
    let with_error_json = serde_json::to_value(&with_error).unwrap();
    assert_eq!(
        with_error_json["error_message"],
        json!("Failed to scan blocks")
    );
}

#[test]
fn seed_phrase_only_in_allowed_backend_payloads() {
    let create_wallet = CreateWalletResponse {
        schema_version: SCHEMA_VERSION,
        wallet: zbag_core::domain::WalletInfo {
            id: uuid::Uuid::nil(),
            name: "w".to_string(),
            wallet_type: zbag_core::domain::WalletType::Software,
            network: zbag_core::domain::Network::Testnet,
            remember_unlock_enabled: false,
            created_at: 0,
            last_opened_at: None,
        },
        seed_phrase: vec!["abandon".to_string().into(); 24],
        backup_challenge: zbag_core::ipc::v1::commands::wallet::BackupChallenge {
            challenge_id: "c".to_string(),
            indices: vec![1, 2, 3, 4],
            expires_at: 0,
        },
    };

    let view_seed = ViewSeedPhraseResponse {
        schema_version: SCHEMA_VERSION,
        seed_phrase: vec!["abandon".to_string().into(); 24],
    };

    let get_balance = GetBalanceResponse {
        schema_version: SCHEMA_VERSION,
        balance: zbag_core::domain::Balance {
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
        privacy_posture: zbag_core::domain::PrivacyPosture::Optimal,
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

#[test]
fn ipc_debug_redacts_sensitive_strings() {
    let restore = RestoreWalletRequest {
        schema_version: SCHEMA_VERSION,
        name: "w".to_string(),
        network: zbag_core::domain::Network::Testnet,
        password: "pw".into(),
        remember_unlock: false,
        seed_phrase: "this is secret".into(),
        birthday_date: None,
    };
    let restore_dbg = format!("{restore:?}");
    assert!(!restore_dbg.contains("this is secret"));
    assert!(!restore_dbg.contains("pw"));
    assert!(restore_dbg.contains("[REDACTED]"));

    let verify = VerifyBackupRequest {
        schema_version: SCHEMA_VERSION,
        wallet_id: uuid::Uuid::nil(),
        challenge_id: "c".to_string(),
        word_challenges: std::collections::BTreeMap::from([(1u8, "this is secret".into())]),
    };
    let verify_dbg = format!("{verify:?}");
    assert!(!verify_dbg.contains("this is secret"));
    assert!(verify_dbg.contains("[REDACTED]"));
}

#[test]
fn version_response_json_shape() {
    let request = GetVersionRequest {
        schema_version: SCHEMA_VERSION,
    };
    let request_json = serde_json::to_value(&request).unwrap();
    assert_eq!(request_json["schema_version"], json!(SCHEMA_VERSION));

    let response = GetVersionResponse {
        schema_version: SCHEMA_VERSION,
        version_info: VersionInfo::current(),
    };
    let response_json = serde_json::to_value(&response).unwrap();
    assert!(response_json.get("version_info").is_some());
    assert!(response_json["version_info"].get("version").is_some());
    assert!(response_json["version_info"].get("git_commit").is_some());
    assert!(response_json["version_info"].get("full_version").is_some());
}

#[test]
fn get_supported_tokens_request_json_shape() {
    // Test serialization
    let request = GetSupportedTokensRequest {
        schema_version: SCHEMA_VERSION,
    };
    let request_json = serde_json::to_value(&request).unwrap();
    assert_eq!(request_json["schema_version"], json!(SCHEMA_VERSION));

    // Test round-trip
    let decoded: GetSupportedTokensRequest = serde_json::from_value(request_json).unwrap();
    assert_eq!(decoded.schema_version, SCHEMA_VERSION);
}

#[test]
fn get_supported_tokens_request_denies_unknown_fields() {
    let request = json!({
        "schema_version": SCHEMA_VERSION,
        "extra_field": "should_fail"
    });
    let decoded: Result<GetSupportedTokensRequest, _> = serde_json::from_value(request);
    assert!(decoded.is_err(), "unknown fields must be rejected");
}

#[test]
fn get_supported_tokens_response_empty_list() {
    // Test with empty token list
    let response = GetSupportedTokensResponse {
        schema_version: SCHEMA_VERSION,
        tokens: vec![],
    };
    let response_json = serde_json::to_value(&response).unwrap();
    assert_eq!(response_json["schema_version"], json!(SCHEMA_VERSION));
    assert_eq!(response_json["tokens"], json!([]));

    // Test round-trip
    let decoded: GetSupportedTokensResponse = serde_json::from_value(response_json).unwrap();
    assert_eq!(decoded.schema_version, SCHEMA_VERSION);
    assert!(decoded.tokens.is_empty());
}

#[test]
fn get_supported_tokens_response_populated_list() {
    // Test with populated token list
    let tokens = vec![
        SupportedToken {
            asset_id: "nep141:wrap.near".to_string(),
            symbol: "NEAR".to_string(),
            chain: "near".to_string(),
            decimals: 24,
            usd_price: Some(3.45),
            icon: Some("https://example.com/near.png".to_string()),
        },
        SupportedToken {
            asset_id: "eth".to_string(),
            symbol: "ETH".to_string(),
            chain: "eth".to_string(),
            decimals: 18,
            usd_price: None,
            icon: None,
        },
    ];

    let response = GetSupportedTokensResponse {
        schema_version: SCHEMA_VERSION,
        tokens: tokens.clone(),
    };
    let response_json = serde_json::to_value(&response).unwrap();
    assert_eq!(response_json["schema_version"], json!(SCHEMA_VERSION));
    assert_eq!(response_json["tokens"].as_array().unwrap().len(), 2);

    // Verify first token structure
    let first_token = &response_json["tokens"][0];
    assert_eq!(first_token["asset_id"], json!("nep141:wrap.near"));
    assert_eq!(first_token["symbol"], json!("NEAR"));
    assert_eq!(first_token["chain"], json!("near"));
    assert_eq!(first_token["decimals"], json!(24));
    assert_eq!(first_token["usd_price"], json!(3.45));
    assert_eq!(first_token["icon"], json!("https://example.com/near.png"));

    // Verify second token with null optional fields
    let second_token = &response_json["tokens"][1];
    assert_eq!(second_token["asset_id"], json!("eth"));
    assert!(second_token["usd_price"].is_null());
    assert!(second_token["icon"].is_null());

    // Test round-trip
    let decoded: GetSupportedTokensResponse = serde_json::from_value(response_json).unwrap();
    assert_eq!(decoded.schema_version, SCHEMA_VERSION);
    assert_eq!(decoded.tokens.len(), 2);
    assert_eq!(decoded.tokens[0].asset_id, "nep141:wrap.near");
    assert_eq!(decoded.tokens[1].symbol, "ETH");
}
