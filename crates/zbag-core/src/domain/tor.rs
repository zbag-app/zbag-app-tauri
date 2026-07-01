use serde::{Deserialize, Serialize};

/// Current Tor runtime status.
///
/// When [`TorState::enabled`] is `true`, the typical state machine is:
/// `Off` -> `Connecting` -> `On` (or `Error` on failures/timeouts).
///
/// When [`TorState::enabled`] is `false`, the status should remain `Off`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TorStatus {
    Off,
    Connecting,
    /// Tor bootstrap completed successfully.
    ///
    /// Note: this does not guarantee lightwalletd connectivity. The first real
    /// gRPC call is the application-level connectivity check.
    On,
    Error,
}

/// User preference + current Tor runtime status.
///
/// `enabled` indicates whether the user has Tor enabled. `status` tracks the
/// runtime state while bootstrapping/operating Tor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TorState {
    pub enabled: bool,
    pub status: TorStatus,
    pub last_error: Option<String>,
}
