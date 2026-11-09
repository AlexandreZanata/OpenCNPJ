//! Selective decisions: uncertain semantics never become executable requests.

use opencnpj_query::{
    export::Export,
    ir::{MunicipalityId, Request, UfId},
    validate::ValidRequest,
};
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
    /// File requested with the question; the request supplies its rows.
    pub export: Option<Export>,
    /// A company looked up by its name instead of a cadastral query.
    pub company: Option<CompanySearch>,
}

/// Companies whose razão social or nome fantasia contains `name`, optionally in a place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompanySearch {
    pub name: String,
    #[serde(default)]
    pub municipalities: Vec<MunicipalityId>,
    #[serde(default)]
    pub ufs: Vec<UfId>,
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
            export: None,
            company: None,
        }
    }
}
