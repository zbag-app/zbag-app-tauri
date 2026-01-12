use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use uuid::Uuid;

use zstash_core::ipc::v1::commands::wallet::ReauthPurpose;

#[derive(Debug, Clone, Copy)]
struct TokenRecord {
    wallet_id: Uuid,
    purpose: ReauthPurpose,
    expires_at: SystemTime,
    used: bool,
}

pub trait Clock: Send + Sync {
    fn now(&self) -> SystemTime;
}

#[derive(Debug)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

#[derive(Debug)]
pub struct ReauthManager<C: Clock = SystemClock> {
    clock: C,
    tokens: HashMap<String, TokenRecord>,
}

impl<C: Clock> ReauthManager<C> {
    pub fn new(clock: C) -> Self {
        Self {
            clock,
            tokens: HashMap::new(),
        }
    }

    pub fn issue(&mut self, wallet_id: Uuid, purpose: ReauthPurpose) -> (String, SystemTime) {
        let token = Uuid::new_v4().to_string();
        let issued_at = self.clock.now();
        let expires_at = issued_at + Duration::from_secs(120);
        self.tokens.insert(
            token.clone(),
            TokenRecord {
                wallet_id,
                purpose,
                expires_at,
                used: false,
            },
        );
        (token, expires_at)
    }

    pub fn validate_and_consume(
        &mut self,
        token: &str,
        wallet_id: Uuid,
        purpose: ReauthPurpose,
    ) -> Result<(), ReauthError> {
        let now = self.clock.now();
        let record = self.tokens.get_mut(token).ok_or(ReauthError::Invalid)?;
        if record.used {
            return Err(ReauthError::Invalid);
        }
        if record.wallet_id != wallet_id || record.purpose != purpose {
            return Err(ReauthError::Invalid);
        }
        if now > record.expires_at {
            return Err(ReauthError::Expired);
        }
        record.used = true;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReauthError {
    #[error("reauth token invalid")]
    Invalid,
    #[error("reauth token expired")]
    Expired,
}
