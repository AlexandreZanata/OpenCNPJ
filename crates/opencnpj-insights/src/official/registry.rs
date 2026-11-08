//! Immutable official-source registry compiled into the insights crate.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

use super::model::SOURCE_STATES;

const REGISTRY_TOML: &str = include_str!("../../official/sources.toml");

fn default_category() -> String {
    "uncategorized".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct Source {
    pub id: String,
    #[serde(default = "default_category")]
    pub category: String,
    pub title: String,
    pub publisher: String,
    pub discovery_url: String,
    pub metadata_url: String,
    pub licence: String,
    pub licence_url: String,
    pub licence_status: String,
    pub attribution: String,
    pub cadence: String,
    pub state: String,
    pub checked_at: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Dataset {
    pub id: String,
    #[serde(default = "default_category")]
    pub category: String,
    pub source_id: String,
    pub title: String,
    pub aggregate: String,
    pub api_url: String,
    pub metadata_url: String,
    #[serde(default)]
    pub default_localities: Option<String>,
    pub cadence: String,
    pub coverage: String,
    pub state: String,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Variable {
    pub id: String,
    pub dataset_id: String,
    pub source_variable: String,
    pub label: String,
    pub unit: String,
    #[serde(default)]
    pub synonyms: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Category {
    pub id: String,
    pub dataset_id: String,
    pub classification_id: String,
    pub category_id: String,
    pub label: String,
    #[serde(default)]
    pub synonyms: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RegistryFile {
    version: u32,
    #[serde(default)]
    source: Vec<Source>,
    #[serde(default)]
    dataset: Vec<Dataset>,
    #[serde(default)]
    variable: Vec<Variable>,
    #[serde(default)]
    category: Vec<Category>,
}

#[derive(Debug, Clone)]
pub struct Registry {
    sources: BTreeMap<String, Source>,
    datasets: BTreeMap<String, Dataset>,
    variables: BTreeMap<String, Variable>,
    categories: BTreeMap<String, Category>,
}

impl Registry {
    /// Load and validate the embedded registry.
    pub fn builtin() -> Result<Self> {
        let file: RegistryFile = toml::from_str(REGISTRY_TOML).context("parse embedded official source registry")?;
        ensure!(file.version == 1, "unsupported official registry version {}", file.version);
        ensure!(!file.source.is_empty(), "official registry has no sources");
        let mut sources = BTreeMap::new();
        for source in file.source {
            validate_url(&source.discovery_url)?;
            validate_url(&source.metadata_url)?;
            validate_url(&source.licence_url)?;
            ensure!(SOURCE_STATES.contains(&source.state.as_str()), "unknown source state '{}'", source.state);
            ensure!(!source.licence_status.is_empty(), "source {} has no licence status", source.id);
            ensure!(!source.category.is_empty(), "source {} has no category", source.id);
            ensure!(sources.insert(source.id.clone(), source).is_none(), "duplicate official source ID");
        }
        let mut datasets = BTreeMap::new();
        for dataset in file.dataset {
            ensure!(sources.contains_key(&dataset.source_id), "dataset {} references unknown source", dataset.id);
            validate_url(&dataset.api_url)?;
            validate_url(&dataset.metadata_url)?;
            ensure!(SOURCE_STATES.contains(&dataset.state.as_str()), "unknown dataset state '{}'", dataset.state);
            ensure!(!dataset.category.is_empty(), "dataset {} has no category", dataset.id);
            ensure!(datasets.insert(dataset.id.clone(), dataset).is_none(), "duplicate official dataset ID");
        }
        let mut variables = BTreeMap::new();
        for variable in file.variable {
            ensure!(datasets.contains_key(&variable.dataset_id), "variable {} references unknown dataset", variable.id);
            ensure!(!variable.source_variable.is_empty(), "variable {} has no source variable", variable.id);
            ensure!(variables.insert(variable.id.clone(), variable).is_none(), "duplicate official variable ID");
        }
        let mut categories = BTreeMap::new();
        for category in file.category {
            ensure!(datasets.contains_key(&category.dataset_id), "category {} references unknown dataset", category.id);
            ensure!(categories.insert(category.id.clone(), category).is_none(), "duplicate official category ID");
        }
        Ok(Self { sources, datasets, variables, categories })
    }

    pub fn sources(&self) -> impl Iterator<Item = &Source> {
        self.sources.values()
    }

    pub fn datasets(&self) -> impl Iterator<Item = &Dataset> {
        self.datasets.values()
    }

    pub fn variables(&self) -> impl Iterator<Item = &Variable> {
        self.variables.values()
    }

    pub fn categories(&self) -> impl Iterator<Item = &Category> {
        self.categories.values()
    }

    pub fn source(&self, id: &str) -> Option<&Source> {
        self.sources.get(id)
    }

    pub fn dataset(&self, id: &str) -> Option<&Dataset> {
        self.datasets.get(id)
    }

    pub fn variable(&self, id: &str) -> Option<&Variable> {
        self.variables.get(id)
    }

    pub fn category(&self, id: &str) -> Option<&Category> {
        self.categories.get(id)
    }

    pub fn variable_by_source(&self, dataset_id: &str, source_variable: &str) -> Option<&Variable> {
        self.variables.values().find(|v| v.dataset_id == dataset_id && v.source_variable == source_variable)
    }

    /// All dataset IDs used by a curated variable or category.
    pub fn dataset_ids(&self) -> BTreeSet<String> {
        self.datasets.keys().cloned().collect()
    }
}

fn validate_url(url: &str) -> Result<()> {
    if !url.starts_with("https://") {
        bail!("registry URL must be HTTPS: {url}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_registry_is_consistent() {
        let registry = Registry::builtin().unwrap();
        assert!(registry.source("ibge").is_some());
        assert!(registry.dataset("ibge-6579").is_some());
        assert_eq!(registry.variable("populacao_estimada").unwrap().source_variable, "9324");
        assert_eq!(registry.category("soja").unwrap().category_id, "2713");
        assert!(registry.variable_by_source("ibge-1612", "214").is_some());
    }
}
