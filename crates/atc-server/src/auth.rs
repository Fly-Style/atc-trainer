use crate::error::AppError;
use atc_shared::ids::{SessionId, StudentToken, TrainerId, TrainerToken};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use rand::distributions::{Alphanumeric, DistString};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenSubject {
    Trainer { trainer_id: TrainerId },
    Student { session_id: SessionId, position_id: atc_shared::ids::PositionId },
}

#[derive(Debug, Default)]
pub struct TokenStore {
    inner: Mutex<TokenStoreInner>,
}

#[derive(Debug, Default)]
struct TokenStoreInner {
    trainer_tokens: HashMap<String, TrainerId>,
    student_tokens: HashMap<String, (SessionId, atc_shared::ids::PositionId)>,
}

impl TokenStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn issue_trainer(&self, trainer_id: TrainerId) -> TrainerToken {
        let token = random_token("tok_trainer_");
        self.inner
            .lock()
            .unwrap()
            .trainer_tokens
            .insert(token.clone(), trainer_id);
        TrainerToken::new(token)
    }

    pub fn issue_student(
        &self,
        session_id: SessionId,
        position_id: atc_shared::ids::PositionId,
    ) -> StudentToken {
        let token = random_token("tok_student_");
        self.inner
            .lock()
            .unwrap()
            .student_tokens
            .insert(token.clone(), (session_id, position_id));
        StudentToken::new(token)
    }

    pub fn lookup(&self, token: &str) -> Option<TokenSubject> {
        let g = self.inner.lock().unwrap();
        if let Some(tid) = g.trainer_tokens.get(token) {
            return Some(TokenSubject::Trainer { trainer_id: tid.clone() });
        }
        if let Some((sid, pid)) = g.student_tokens.get(token) {
            return Some(TokenSubject::Student { session_id: sid.clone(), position_id: pid.clone() });
        }
        None
    }
}

fn random_token(prefix: &str) -> String {
    let mut rng = rand::thread_rng();
    let tail = Alphanumeric.sample_string(&mut rng, 20);
    format!("{prefix}{tail}")
}

/// Bearer token extractor. Returns `Unauthorized` when the header is missing or invalid.
pub struct BearerToken(pub String);

impl<S: Send + Sync> FromRequestParts<S> for BearerToken {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::Unauthorized)?;
        let token = auth.strip_prefix("Bearer ").ok_or(AppError::Unauthorized)?;
        Ok(BearerToken(token.to_string()))
    }
}
