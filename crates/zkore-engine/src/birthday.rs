use zkore_core::domain::Network;

/// Estimates a wallet "birthday height" from an approximate first-transaction date.
///
/// This is intentionally conservative (it may return a lower height than necessary) to avoid
/// missing funds at the cost of scanning a bit more.
pub fn estimate_birthday_height(network: Network, birthday_date_ms: i64) -> u32 {
    let genesis_ms = match network {
        Network::Mainnet => MAINNET_GENESIS_MS,
        Network::Testnet => TESTNET_GENESIS_MS,
    };

    if birthday_date_ms <= genesis_ms {
        return 0;
    }

    let delta_ms = birthday_date_ms.saturating_sub(genesis_ms);
    let estimated = delta_ms / AVERAGE_BLOCK_TIME_MS;

    // Apply a safety margin to ensure we scan slightly earlier than the estimate.
    let estimated_with_margin = estimated.saturating_sub(SAFETY_MARGIN_BLOCKS as i64);

    estimated_with_margin.clamp(0, u32::MAX as i64) as u32
}

// Zcash has a target block time of 75 seconds.
const AVERAGE_BLOCK_TIME_MS: i64 = 75_000;

// Safety margin (~6.25 hours) to reduce the risk of estimating too high.
const SAFETY_MARGIN_BLOCKS: u32 = 300;

// Best-effort fixed points; we can replace this with a weekly checkpoint table later.
const MAINNET_GENESIS_MS: i64 = 1_477_612_800_000; // 2016-10-28T00:00:00Z
const TESTNET_GENESIS_MS: i64 = 1_477_612_800_000; // Conservative default for v1
