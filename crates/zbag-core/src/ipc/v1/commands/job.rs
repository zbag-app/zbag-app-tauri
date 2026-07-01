//! IPC command types for background job management.

use serde::{Deserialize, Serialize};

use crate::domain::{JobId, JobProgress};

/// Request to start a send transaction as a background job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartSendJobRequest {
    pub schema_version: u32,
    /// Proposal ID from PrepareSendResponse.
    pub proposal_id: String,
    /// Re-auth token (purpose: Spend).
    pub reauth_token: String,
}

/// Response when a send job is started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartSendJobResponse {
    pub schema_version: u32,
    /// Unique job identifier for tracking progress.
    pub job_id: JobId,
}

/// Request to start a shield operation as a background job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartShieldJobRequest {
    pub schema_version: u32,
    pub account_id: u32,
    pub consolidate: bool,
    /// Re-auth token (purpose: Spend).
    pub reauth_token: String,
}

/// Response when a shield job is started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartShieldJobResponse {
    pub schema_version: u32,
    /// Unique job identifier for tracking progress.
    pub job_id: JobId,
}

/// Request to cancel a running job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelJobRequest {
    pub schema_version: u32,
    pub job_id: JobId,
}

/// Response when cancelling a job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelJobResponse {
    pub schema_version: u32,
    /// True if the job was cancelled, false if it could not be cancelled.
    pub cancelled: bool,
}

/// Request to get the current status of a job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetJobStatusRequest {
    pub schema_version: u32,
    pub job_id: JobId,
}

/// Response with current job status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetJobStatusResponse {
    pub schema_version: u32,
    pub progress: JobProgress,
}

/// Request to list all active jobs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListJobsRequest {
    pub schema_version: u32,
}

/// Response listing all active jobs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListJobsResponse {
    pub schema_version: u32,
    pub jobs: Vec<JobProgress>,
}
