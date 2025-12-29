use anyhow::Context as _;
use rusqlite::Connection;

use zkore_core::domain::{Balance, Network};
use zkore_core::errors;

use crate::error::ipc_err;

#[allow(deprecated)]
use zcash_client_backend::data_api::{Account as _, WalletRead as _};

pub fn get_balance(
    conn: &mut Connection,
    network: Network,
    account_id: u32,
) -> anyhow::Result<Balance> {
    let params = zcash_consensus_network(network);
    let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
        conn,
        params,
        zcash_client_sqlite::util::SystemClock,
        rand::rngs::OsRng,
    );

    let account = find_account_uuid(&mut wdb, account_id)?;

    let summary = wdb
        .get_wallet_summary(zcash_client_backend::data_api::wallet::ConfirmationsPolicy::default())
        .context("failed to compute wallet summary")?;

    let account_balance = summary
        .as_ref()
        .and_then(|s| s.account_balances().get(&account))
        .copied()
        .unwrap_or(zcash_client_backend::data_api::AccountBalance::ZERO);

    let shielded_spendable = account_balance.spendable_value().into_u64();
    let shielded_pending = (account_balance.change_pending_confirmation()
        + account_balance.value_pending_spendability()
        + account_balance.uneconomic_value())
    .expect("AccountBalance invariants ensure no overflow")
    .into_u64();
    let transparent_total = account_balance.unshielded_balance().total().into_u64();
    let total = account_balance.total().into_u64();

    Ok(Balance {
        shielded_spendable: shielded_spendable.to_string(),
        shielded_pending: shielded_pending.to_string(),
        transparent_total: transparent_total.to_string(),
        total: total.to_string(),
    })
}

fn find_account_uuid(
    wdb: &mut zcash_client_sqlite::WalletDb<
        &mut Connection,
        zcash_protocol::consensus::Network,
        zcash_client_sqlite::util::SystemClock,
        rand::rngs::OsRng,
    >,
    account_id: u32,
) -> anyhow::Result<zcash_client_sqlite::AccountUuid> {
    let account_uuids = wdb
        .get_account_ids()
        .context("failed to list wallet accounts")?;

    for account_uuid in account_uuids {
        let Some(account) = wdb
            .get_account(account_uuid)
            .context("failed to load wallet account")?
        else {
            continue;
        };

        if let Some(derivation) = account.source().key_derivation() {
            let derived_id: u32 = derivation.account_index().into();
            if derived_id == account_id {
                return Ok(account_uuid);
            }
            continue;
        }

        if let Some(key_source) = account.source().key_source()
            && crate::account_key_source::parse_account_id_from_key_source(key_source)
                == Some(account_id)
        {
            return Ok(account_uuid);
        }

        continue;
    }

    Err(ipc_err(errors::ACCOUNT_NOT_FOUND, "account not found"))
}

fn zcash_consensus_network(network: Network) -> zcash_protocol::consensus::Network {
    match network {
        Network::Mainnet => zcash_protocol::consensus::Network::MainNetwork,
        Network::Testnet => zcash_protocol::consensus::Network::TestNetwork,
    }
}
