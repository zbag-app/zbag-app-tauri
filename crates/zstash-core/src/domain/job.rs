//! Job domain types for async long-running operations.

use serde::{Deserialize, Serialize};

/// Unique identifier for a background job.
pub type JobId = String;

/// Type of job operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobType {
    /// Send transaction (proving + broadcast).
    Send,
    /// Shield transparent funds.
    Shield,
}

/// Current state of a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobState {
    /// Job is queued but not yet started.
    Queued,
    /// Job is actively running.
    Running,
    /// Job completed successfully.
    Completed,
    /// Job failed with an error.
    Failed,
    /// Job was cancelled by the user.
    Cancelled,
}

/// Phase within a transaction job for granular progress reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobPhase {
    /// Initial validation and setup.
    Preparing,
    /// Generating zero-knowledge proofs (CPU intensive).
    Proving,
    /// Broadcasting transaction to the network.
    Broadcasting,
    /// Operation complete.
    Done,
}

/// Progress information for a job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobProgress {
    /// Job identifier.
    pub job_id: JobId,
    /// Type of operation.
    pub job_type: JobType,
    /// Current state.
    pub state: JobState,
    /// Current phase within the operation.
    pub phase: JobPhase,
    /// Progress percentage (0-100), if determinable.
    pub progress_percent: Option<u8>,
    /// Transaction ID once known (after proving, before or after broadcast).
    pub txid: Option<String>,
    /// Error message if the job failed.
    pub error: Option<String>,
    /// Whether the job can be cancelled in its current state.
    pub can_cancel: bool,
}

impl JobProgress {
    /// Create a new job progress in queued state.
    pub fn queued(job_id: JobId, job_type: JobType) -> Self {
        Self {
            job_id,
            job_type,
            state: JobState::Queued,
            phase: JobPhase::Preparing,
            progress_percent: Some(0),
            txid: None,
            error: None,
            can_cancel: true,
        }
    }

    /// Create a progress update for the proving phase.
    pub fn proving(job_id: JobId, job_type: JobType, progress_percent: Option<u8>) -> Self {
        Self {
            job_id,
            job_type,
            state: JobState::Running,
            phase: JobPhase::Proving,
            progress_percent,
            txid: None,
            error: None,
            can_cancel: true,
        }
    }

    /// Create a progress update for the broadcasting phase.
    pub fn broadcasting(job_id: JobId, job_type: JobType, txid: String) -> Self {
        Self {
            job_id,
            job_type,
            state: JobState::Running,
            phase: JobPhase::Broadcasting,
            progress_percent: Some(90),
            txid: Some(txid),
            error: None,
            can_cancel: false, // Cannot cancel once signed
        }
    }

    /// Create a completed progress.
    pub fn completed(job_id: JobId, job_type: JobType, txid: String) -> Self {
        Self {
            job_id,
            job_type,
            state: JobState::Completed,
            phase: JobPhase::Done,
            progress_percent: Some(100),
            txid: Some(txid),
            error: None,
            can_cancel: false,
        }
    }

    /// Create a failed progress.
    pub fn failed(job_id: JobId, job_type: JobType, error: String, txid: Option<String>) -> Self {
        Self {
            job_id,
            job_type,
            state: JobState::Failed,
            phase: JobPhase::Done,
            progress_percent: None,
            txid,
            error: Some(error),
            can_cancel: false,
        }
    }

    /// Create a cancelled progress.
    pub fn cancelled(job_id: JobId, job_type: JobType) -> Self {
        Self {
            job_id,
            job_type,
            state: JobState::Cancelled,
            phase: JobPhase::Done,
            progress_percent: None,
            txid: None,
            error: None,
            can_cancel: false,
        }
    }
}
