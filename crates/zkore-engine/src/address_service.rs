use anyhow::Context as _;
use rusqlite::Connection;
use zcash_protocol::consensus::Parameters as _;

use zkore_core::domain::{AddressInfo, AddressType, Network};
use zkore_core::errors;

use crate::error::ipc_err;

#[allow(deprecated)]
use zcash_client_backend::{
    data_api::{Account as _, AddressSource, WalletRead as _, WalletWrite as _},
    keys::{ReceiverRequirement, UnifiedAddressRequest},
};

pub fn get_receive_address(
    conn: &mut Connection,
    network: Network,
    account_id: u32,
    address_type: AddressType,
) -> anyhow::Result<AddressInfo> {
    let params = zcash_consensus_network(network);
    let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
        conn,
        params,
        zcash_client_sqlite::util::SystemClock,
        rand::rngs::OsRng,
    );

    let account = find_account_uuid(&mut wdb, account_id)?;

    match address_type {
        AddressType::ShieldedOnly => {
            // Require Orchard and allow Sapling; omit transparent receiver.
            let request = UnifiedAddressRequest::custom(
                ReceiverRequirement::Require,
                ReceiverRequirement::Allow,
                ReceiverRequirement::Omit,
            )
            .expect("valid receiver requirements");

            if let Ok(addresses) = wdb.list_addresses(account) {
                let mut best: Option<(&zcash_client_backend::address::UnifiedAddress, u128)> = None;
                for info in addresses.iter() {
                    let ua = match info.address() {
                        zcash_client_backend::address::Address::Unified(ua) => ua,
                        _ => continue,
                    };

                    if ua.transparent().is_some() {
                        continue;
                    }

                    let di = match info.source() {
                        AddressSource::Derived { diversifier_index, .. } => u128::from(diversifier_index),
                    };

                    match best {
                        Some((_, best_di)) if di <= best_di => {}
                        _ => best = Some((ua, di)),
                    }
                }

                if let Some((ua, di)) = best {
                    return Ok(AddressInfo {
                        encoded: ua.encode(&params),
                        address_type,
                        diversifier_index: di.to_string(),
                    });
                }
            }

            let Some((ua, di)) = wdb
                .get_next_available_address(account, request)
                .context("failed to derive receive address")?
            else {
                return Err(ipc_err(errors::ACCOUNT_NOT_FOUND, "account not found"));
            };

            Ok(AddressInfo {
                encoded: ua.encode(&params),
                address_type,
                diversifier_index: u128::from(di).to_string(),
            })
        }
        AddressType::Transparent => {
            // v1: use a single stable transparent compatibility address per account (no rotation).
            let mut receivers = wdb
                .get_transparent_receivers(account, false, false)
                .context("failed to list transparent receivers")?;

            if receivers.is_empty() {
                // Force derivation of at least one transparent receiver by generating a UA that
                // permits a transparent receiver.
                let _ = wdb
                    .get_next_available_address(account, UnifiedAddressRequest::ALLOW_ALL)
                    .context("failed to derive an address with transparent receiver")?;
                receivers = wdb
                    .get_transparent_receivers(account, false, false)
                    .context("failed to list transparent receivers")?;
            }

            let Some((addr, meta)) = receivers
                .into_iter()
                .min_by_key(|(_addr, meta)| meta.address_index().map(|i| i.index()).unwrap_or(u32::MAX))
            else {
                return Err(ipc_err(
                    errors::INTERNAL_ERROR,
                    "no transparent receiver available",
                ));
            };

            let index = meta.address_index().map(|i| i.index()).unwrap_or(0);
            Ok(AddressInfo {
                encoded: addr.to_zcash_address(params.network_type()).encode(),
                address_type,
                diversifier_index: index.to_string(),
            })
        }
    }
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

        let Some(derivation) = account.source().key_derivation() else {
            continue;
        };
        let derived_id: u32 = derivation.account_index().into();
        if derived_id == account_id {
            return Ok(account_uuid);
        }
    }

    Err(ipc_err(errors::ACCOUNT_NOT_FOUND, "account not found"))
}

fn zcash_consensus_network(network: Network) -> zcash_protocol::consensus::Network {
    match network {
        Network::Mainnet => zcash_protocol::consensus::Network::MainNetwork,
        Network::Testnet => zcash_protocol::consensus::Network::TestNetwork,
    }
}
