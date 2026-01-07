pub const ACCOUNT_ID_KEY_SOURCE_PREFIX: &str = "zkore-account-id:";

pub fn key_source_for_account_id(account_id: u32) -> String {
    format!("{ACCOUNT_ID_KEY_SOURCE_PREFIX}{account_id}")
}

pub fn parse_account_id_from_key_source(key_source: &str) -> Option<u32> {
    key_source
        .strip_prefix(ACCOUNT_ID_KEY_SOURCE_PREFIX)
        .and_then(|s| s.parse::<u32>().ok())
}
