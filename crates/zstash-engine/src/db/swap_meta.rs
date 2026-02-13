use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use zstash_core::domain::{SwapInfo, SwapState, SwapType};

pub fn insert_swap(conn: &Connection, wallet_id: Uuid, swap: &SwapInfo) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO swaps (
            id, remote_id, wallet_id, swap_type, input_asset, input_amount, output_asset, output_amount,
            deposit_address, deposit_memo, destination_address, refund_address,
            state, deadline, last_error, created_at, updated_at
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
            ?9, ?10, ?11, ?12,
            ?13, ?14, ?15, ?16, ?17
        )",
        params![
            swap.id.to_string(),
            swap.remote_id,
            wallet_id.to_string(),
            format!("{:?}", swap.swap_type),
            swap.input_asset,
            swap.input_amount,
            swap.output_asset,
            swap.output_amount,
            swap.deposit_address,
            swap.deposit_memo,
            swap.destination_address,
            swap.refund_address,
            format!("{:?}", swap.state),
            swap.deadline,
            swap.last_error,
            swap.created_at,
            swap.updated_at,
        ],
    )?;
    Ok(())
}

pub fn get_swap(conn: &Connection, swap_id: Uuid) -> rusqlite::Result<Option<(Uuid, SwapInfo)>> {
    let mut stmt = conn.prepare(
        "SELECT
            id, remote_id, wallet_id, swap_type, input_asset, input_amount, output_asset, output_amount,
            deposit_address, deposit_memo, destination_address, refund_address,
            state, deadline, last_error, created_at, updated_at
         FROM swaps WHERE id = ?1",
    )?;
    let row = stmt
        .query_row([swap_id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let remote_id: Option<String> = row.get(1)?;
            let wallet_id_str: String = row.get(2)?;
            let swap_type: String = row.get(3)?;
            let state: String = row.get(12)?;

            let wallet_id = Uuid::parse_str(&wallet_id_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

            let info = SwapInfo {
                id: Uuid::parse_str(&id_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
                remote_id,
                swap_type: parse_swap_type(&swap_type),
                input_asset: row.get(4)?,
                input_amount: row.get(5)?,
                output_asset: row.get(6)?,
                output_amount: row.get(7)?,
                deposit_address: row.get(8)?,
                deposit_memo: row.get(9)?,
                destination_address: row.get(10)?,
                refund_address: row.get(11)?,
                state: parse_swap_state(&state),
                deadline: row.get(13)?,
                last_error: row.get(14)?,
                created_at: row.get(15)?,
                updated_at: row.get(16)?,
            };

            Ok((wallet_id, info))
        })
        .optional()?;

    Ok(row)
}

pub fn list_swaps_for_wallet(
    conn: &Connection,
    wallet_id: Uuid,
) -> rusqlite::Result<Vec<SwapInfo>> {
    let mut stmt = conn.prepare(
        "SELECT
            id, remote_id, swap_type, input_asset, input_amount, output_asset, output_amount,
            deposit_address, deposit_memo, destination_address, refund_address,
            state, deadline, last_error, created_at, updated_at
         FROM swaps WHERE wallet_id = ?1
         ORDER BY created_at DESC",
    )?;

    let swaps = stmt
        .query_map([wallet_id.to_string()], swap_info_from_list_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(swaps)
}

/// Lists swaps eligible for polling/resume after app restart.
///
/// # States included
///
/// - **Draft**: Included because swap flows may persist a Draft swap with a deposit address
///   before the state transitions (e.g., crash/restart during FromZec flows).
/// - **AwaitingDeposit**, **Pending**: Standard active swap states.
/// - **Confirming**: Included because the remote status may advance to a terminal state while
///   the local DB still shows an intermediate state.
///
/// `deadline` is only applied to pre-confirmation states (`Draft` and `AwaitingDeposit`)
/// so expired swaps in `Pending`/`Confirming` can still reach terminal outcomes on resume.
pub fn list_pollable_swaps_for_wallet(
    conn: &Connection,
    wallet_id: Uuid,
) -> rusqlite::Result<Vec<SwapInfo>> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let mut stmt = conn.prepare(
        "SELECT
            id, remote_id, swap_type, input_asset, input_amount, output_asset, output_amount,
            deposit_address, deposit_memo, destination_address, refund_address,
            state, deadline, last_error, created_at, updated_at
         FROM swaps
         WHERE wallet_id = ?1
           AND deposit_address IS NOT NULL
           AND (
               state IN ('Pending', 'Confirming')
               OR (
                   state IN ('Draft', 'AwaitingDeposit')
                   AND (deadline IS NULL OR deadline > ?2)
               )
           )
         ORDER BY created_at DESC",
    )?;

    let swaps = stmt
        .query_map(
            params![wallet_id.to_string(), now_ms],
            swap_info_from_list_row,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(swaps)
}

pub fn update_swap(conn: &Connection, wallet_id: Uuid, swap: &SwapInfo) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE swaps SET
            remote_id = ?2,
            wallet_id = ?3,
            swap_type = ?4,
            input_asset = ?5,
            input_amount = ?6,
            output_asset = ?7,
            output_amount = ?8,
            deposit_address = ?9,
            deposit_memo = ?10,
            destination_address = ?11,
            refund_address = ?12,
            state = ?13,
            deadline = ?14,
            last_error = ?15,
            updated_at = ?16
         WHERE id = ?1",
        params![
            swap.id.to_string(),
            swap.remote_id,
            wallet_id.to_string(),
            format!("{:?}", swap.swap_type),
            swap.input_asset,
            swap.input_amount,
            swap.output_asset,
            swap.output_amount,
            swap.deposit_address,
            swap.deposit_memo,
            swap.destination_address,
            swap.refund_address,
            format!("{:?}", swap.state),
            swap.deadline,
            swap.last_error,
            swap.updated_at,
        ],
    )?;
    Ok(())
}

fn swap_info_from_list_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SwapInfo> {
    let id_str: String = row.get(0)?;
    let swap_type: String = row.get(2)?;
    let state: String = row.get(11)?;

    Ok(SwapInfo {
        id: Uuid::parse_str(&id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        remote_id: row.get(1)?,
        swap_type: parse_swap_type(&swap_type),
        input_asset: row.get(3)?,
        input_amount: row.get(4)?,
        output_asset: row.get(5)?,
        output_amount: row.get(6)?,
        deposit_address: row.get(7)?,
        deposit_memo: row.get(8)?,
        destination_address: row.get(9)?,
        refund_address: row.get(10)?,
        state: parse_swap_state(&state),
        deadline: row.get(12)?,
        last_error: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}

fn parse_swap_type(value: &str) -> SwapType {
    match value {
        "ToZec" => SwapType::ToZec,
        "FromZec" => SwapType::FromZec,
        _ => SwapType::ToZec,
    }
}

fn parse_swap_state(value: &str) -> SwapState {
    match value {
        "Draft" => SwapState::Draft,
        "AwaitingDeposit" => SwapState::AwaitingDeposit,
        "Pending" => SwapState::Pending,
        "Confirming" => SwapState::Confirming,
        "Completed" => SwapState::Completed,
        "Refunded" => SwapState::Refunded,
        "Failed" => SwapState::Failed,
        _ => SwapState::Pending,
    }
}
