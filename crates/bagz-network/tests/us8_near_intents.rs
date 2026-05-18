use bagz_core::domain::SwapState;
use bagz_network::near_intents::{RemoteStatus, map_remote_status_to_local_state};

#[test]
fn maps_success_to_confirming() {
    assert_eq!(
        map_remote_status_to_local_state(&RemoteStatus::Success),
        SwapState::Confirming
    );
}

#[test]
fn maps_terminal_states() {
    assert_eq!(
        map_remote_status_to_local_state(&RemoteStatus::Refunded),
        SwapState::Refunded
    );
    assert_eq!(
        map_remote_status_to_local_state(&RemoteStatus::Failed),
        SwapState::Failed
    );
}
