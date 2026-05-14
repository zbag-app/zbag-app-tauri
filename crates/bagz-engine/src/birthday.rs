use zstash_core::domain::Network;

/// Estimates a wallet "birthday height" from an approximate first-transaction date.
///
/// This is intentionally conservative (it may return a lower height than necessary) to avoid
/// missing funds at the cost of scanning a bit more.
pub fn estimate_birthday_height(network: Network, birthday_date_ms: i64) -> u32 {
    match network {
        Network::Mainnet => estimate_from_checkpoints(MAINNET_CHECKPOINTS, birthday_date_ms),
        Network::Testnet => estimate_linear(TESTNET_GENESIS_MS, birthday_date_ms),
    }
}

fn estimate_from_checkpoints(checkpoints: &[(i64, u32)], birthday_date_ms: i64) -> u32 {
    if checkpoints.is_empty() {
        return 0;
    }

    let first_ts = checkpoints[0].0;
    if birthday_date_ms <= first_ts {
        return 0;
    }

    let idx = checkpoints.partition_point(|(ts, _height)| *ts <= birthday_date_ms);
    let (base_ts, base_height) = checkpoints[idx.saturating_sub(1)];

    let delta_ms = birthday_date_ms.saturating_sub(base_ts);
    let estimated = (base_height as i64).saturating_add(delta_ms / AVERAGE_BLOCK_TIME_MS);
    let estimated_with_margin = estimated.saturating_sub(SAFETY_MARGIN_BLOCKS as i64);
    estimated_with_margin.clamp(0, u32::MAX as i64) as u32
}

fn estimate_linear(genesis_ms: i64, birthday_date_ms: i64) -> u32 {
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

// Coarse checkpoints to reduce scan time for typical restores.
//
// IMPORTANT: timestamps are chosen conservatively (well after the corresponding height),
// so checkpoint heights should be <= actual chain height at that time.
const MAINNET_CHECKPOINTS: &[(i64, u32)] = &[
    (MAINNET_GENESIS_MS, 0),
    // Sapling activation (height 419,200) was in late 2018; by 2019-01-01 the height is safely beyond.
    (1_546_300_800_000, 419_200), // 2019-01-01T00:00:00Z
    // Canopy activation (height 1,046,400) was in late 2020; by 2021-01-01 the height is safely beyond.
    (1_609_459_200_000, 1_046_400), // 2021-01-01T00:00:00Z
    // NU5 activation (height 1,687,104) was in mid 2022; by 2022-06-01 the height is safely beyond.
    (1_654_041_600_000, 1_687_104), // 2022-06-01T00:00:00Z
];

// Best-effort fixed points; testnet resets make precise checkpoints fragile.
const MAINNET_GENESIS_MS: i64 = 1_477_612_800_000; // 2016-10-28T00:00:00Z
const TESTNET_GENESIS_MS: i64 = 1_477_612_800_000; // Conservative default for v1
