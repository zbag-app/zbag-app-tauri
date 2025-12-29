use anyhow::Context as _;
use rusqlite::Connection;
use uuid::Uuid;
use zcash_protocol::consensus::Parameters as _;
use zip32::DiversifierIndex;

use zkore_core::domain::{AddressInfo, AddressType, Network};
use zkore_core::errors;

use crate::db::rotation_meta;
use crate::error::ipc_err;

#[allow(deprecated)]
use zcash_client_backend::{
    address::Address,
    data_api::{Account as _, AddressSource, WalletRead as _, WalletWrite as _},
    keys::{ReceiverRequirement, UnifiedAddressRequest},
};

pub fn decode_address(network: Network, encoded: &str) -> anyhow::Result<Address> {
    let params = zcash_consensus_network(network);
    Address::decode(&params, encoded)
        .ok_or_else(|| ipc_err(errors::INVALID_RECIPIENT, "invalid recipient"))
}

pub fn get_receive_address(
    app_conn: &Connection,
    wallet_id: Uuid,
    wallet_conn: &mut Connection,
    network: Network,
    account_id: u32,
    address_type: AddressType,
) -> anyhow::Result<AddressInfo> {
    let params = zcash_consensus_network(network);
    let mut wdb = zcash_client_sqlite::WalletDb::from_connection(
        wallet_conn,
        params,
        zcash_client_sqlite::util::SystemClock,
        rand::rngs::OsRng,
    );

    let account = find_account_uuid(&mut wdb, account_id)?;

    match address_type {
        AddressType::ShieldedOnly => {
            // Require Orchard and Sapling; omit transparent receiver.
            let request = UnifiedAddressRequest::custom(
                ReceiverRequirement::Require,
                ReceiverRequirement::Require,
                ReceiverRequirement::Omit,
            )
            .expect("valid receiver requirements");

            let addresses = wdb
                .list_addresses(account)
                .context("failed to list wallet addresses")?;
            let mut max_existing: Option<u64> = None;
            for info in &addresses {
                let AddressSource::Derived {
                    diversifier_index, ..
                } = info.source();
                let di_u128 = u128::from(diversifier_index);
                let di_u64 = u64::try_from(di_u128).map_err(|_| {
                    ipc_err(errors::INTERNAL_ERROR, "diversifier index out of range")
                })?;
                max_existing = Some(max_existing.map_or(di_u64, |best| best.max(di_u64)));
            }

            let stored_next =
                rotation_meta::get_next_diversifier_index(app_conn, wallet_id, account_id)
                    .context("failed to load receive rotation state")?;

            let min_next = max_existing.map(|di| di.saturating_add(1)).unwrap_or(0);
            let mut next_di = stored_next.unwrap_or(min_next);
            if next_di < min_next {
                next_di = min_next;
            }

            let mut candidate = next_di;
            for _ in 0..1_000 {
                let maybe = wdb
                    .get_address_for_index(account, DiversifierIndex::from(candidate), request)
                    .context("failed to derive receive address")?;
                if let Some(ua) = maybe {
                    let now_ms = chrono::Utc::now().timestamp_millis();
                    rotation_meta::set_next_diversifier_index(
                        app_conn,
                        wallet_id,
                        account_id,
                        candidate.saturating_add(1),
                        now_ms,
                    )
                    .context("failed to update receive rotation state")?;

                    return Ok(AddressInfo {
                        encoded: ua.encode(&params),
                        address_type,
                        diversifier_index: candidate.to_string(),
                    });
                }

                candidate = candidate
                    .checked_add(1)
                    .ok_or_else(|| ipc_err(errors::INTERNAL_ERROR, "diversifier index overflow"))?;
            }

            Err(ipc_err(
                errors::INTERNAL_ERROR,
                "unable to derive receive address",
            ))
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

            let Some((addr, meta)) = receivers.into_iter().min_by_key(|(_addr, meta)| {
                meta.address_index().map(|i| i.index()).unwrap_or(u32::MAX)
            }) else {
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

pub(crate) fn find_account_uuid(
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

        // Unknown account ID (not derived, not a Zkore-tagged imported account).
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
