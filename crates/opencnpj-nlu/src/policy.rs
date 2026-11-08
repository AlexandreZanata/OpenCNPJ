//! Selective decisions: uncertain semantics never become executable requests.

use opencnpj_query::{ir::Request, validate::ValidRequest};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Execute,
    ExecuteWithNote,
    Clarify,
    Reject,
}

#[derive(Debug, Clone, Serialize)]
pub struct Interpretation {
    pub decision: Decision,
    pub request: Option<ValidRequest>,
    pub act: String,
    pub reason: String,
    pub notes: Vec<String>,
    /// Rule evidence, not a calibrated probability.
    pub evidence: f64,
    pub options: Vec<Request>,
}

impl Interpretation {
    pub(crate) fn stop(decision: Decision, reason: impl Into<String>) -> Self {
        Self {
            decision,
            request: None,
            act: "new_query".into(),
            reason: reason.into(),
            notes: Vec::new(),
            evidence: 0.0,
            options: Vec::new(),
        }
    }
}
