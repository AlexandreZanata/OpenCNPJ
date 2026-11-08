//! Data model for official source-backed indicators.

use serde::{Deserialize, Serialize};

/// Lifecycle states admitted for a source or dataset.
pub const SOURCE_STATES: &[&str] =
    &["discovered", "verified", "fetched", "normalized", "queryable", "commercially_enabled", "stale", "failed", "unavailable"];

/// State of one cell in a source table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellStatus {
    Observed,
    Missing,
    Suppressed,
    NotAvailable,
    Malformed,
}

impl CellStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Missing => "missing",
            Self::Suppressed => "suppressed",
            Self::NotAvailable => "not_available",
            Self::Malformed => "malformed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "observed" => Some(Self::Observed),
            "missing" => Some(Self::Missing),
            "suppressed" => Some(Self::Suppressed),
            "not_available" => Some(Self::NotAvailable),
            "malformed" => Some(Self::Malformed),
            _ => None,
        }
    }

    pub fn is_observed(self) -> bool {
        self == Self::Observed
    }
}

/// One normalized observation. A source table cell maps to at most one row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    pub dataset_id: String,
    pub variable_id: String,
    pub variable_label: String,
    pub unit: String,
    pub classification_id: String,
    pub classification_label: String,
    pub category_id: String,
    pub category_label: String,
    pub geography_level: String,
    pub geography_code: String,
    pub geography_name: String,
    pub period: String,
    pub value: Option<f64>,
    pub status: CellStatus,
    pub source_url: String,
}

impl Observation {
    /// Stable identity used to detect duplicate cells across editions.
    pub fn key(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}",
            self.dataset_id,
            self.variable_id,
            self.classification_id,
            self.category_id,
            self.geography_level,
            self.geography_code,
            self.period
        )
    }

    pub fn category(&self) -> Option<(&str, &str)> {
        if self.category_id.is_empty() { None } else { Some((&self.classification_id, &self.category_id)) }
    }
}

/// Manifest embedded in an immutable `.ocind` SQLite container.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatasetManifest {
    pub format_version: u32,
    pub source_id: String,
    pub dataset_id: String,
    pub title: String,
    pub publisher: String,
    pub source_url: String,
    pub metadata_url: String,
    pub licence: String,
    pub licence_url: String,
    pub licence_status: String,
    pub attribution: String,
    pub retrieved_at: String,
    pub publication: Option<String>,
    pub coverage: String,
    pub unit: String,
    pub period_from: Option<String>,
    pub period_to: Option<String>,
    pub raw_sha256: String,
    pub raw_bytes: u64,
    pub row_count: u64,
    pub accepted_rows: u64,
    pub rejected_rows: u64,
    pub duplicate_rows: u64,
    pub malformed_rows: u64,
    pub state: String,
    pub limitations: Vec<String>,
}

impl DatasetManifest {
    pub fn validate(&self) -> Result<(), String> {
        if self.format_version != 1 {
            return Err(format!("unsupported indicator format version {}", self.format_version));
        }
        for (name, value) in [
            ("source_id", &self.source_id),
            ("dataset_id", &self.dataset_id),
            ("title", &self.title),
            ("publisher", &self.publisher),
            ("source_url", &self.source_url),
            ("licence", &self.licence),
            ("licence_url", &self.licence_url),
            ("licence_status", &self.licence_status),
            ("attribution", &self.attribution),
            ("retrieved_at", &self.retrieved_at),
            ("coverage", &self.coverage),
        ] {
            if value.trim().is_empty() {
                return Err(format!("empty manifest field {name}"));
            }
        }
        if !self.source_url.starts_with("https://") || !self.licence_url.starts_with("https://") {
            return Err("source and licence URLs must be HTTPS".into());
        }
        if !SOURCE_STATES.contains(&self.state.as_str()) {
            return Err(format!("unknown dataset state '{}'", self.state));
        }
        if self.row_count != self.accepted_rows.saturating_add(self.rejected_rows) {
            return Err(format!(
                "row accounting mismatch: total {}, accepted {}, rejected {}",
                self.row_count, self.accepted_rows, self.rejected_rows
            ));
        }
        if self.rejected_rows != self.duplicate_rows.saturating_add(self.malformed_rows) {
            return Err("rejected rows must equal duplicate plus malformed rows".into());
        }
        Ok(())
    }
}
