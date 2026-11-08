//! Typed indicator operations and source-aware answers.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail, ensure};
use serde::Serialize;

use super::{
    model::{CellStatus, DatasetManifest, Observation},
    registry::Registry,
    store::StoreSet,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryFilter {
    pub classification_id: String,
    pub category_id: String,
}

impl CategoryFilter {
    pub fn new(classification_id: impl Into<String>, category_id: impl Into<String>) -> Self {
        Self { classification_id: classification_id.into(), category_id: category_id.into() }
    }

    fn pair(&self) -> (&str, &str) {
        (&self.classification_id, &self.category_id)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// All cells of one variable, optionally restricted by geography and period.
    Lookup { variable: String, geographies: Vec<String>, periods: Vec<String>, category: Option<CategoryFilter> },
    /// One variable across geographies in one period.
    Compare { variable: String, geographies: Vec<String>, period: Option<String>, category: Option<CategoryFilter> },
    /// One variable ranked across a territory.
    Rank {
        variable: String,
        period: Option<String>,
        geography_level: String,
        uf_code: Option<String>,
        category: Option<CategoryFilter>,
        limit: usize,
        descending: bool,
    },
    /// One variable over time for one geography.
    Trend { variable: String, geography: String, periods: Vec<String>, category: Option<CategoryFilter> },
    /// Numerator divided by denominator over aligned geography and period.
    Ratio {
        numerator: String,
        denominator: String,
        geographies: Vec<String>,
        period: Option<String>,
        category: Option<CategoryFilter>,
        scale: f64,
    },
}

impl Request {
    pub fn operation(&self) -> &'static str {
        match self {
            Self::Lookup { .. } => "lookup",
            Self::Compare { .. } => "compare",
            Self::Rank { .. } => "rank",
            Self::Trend { .. } => "trend",
            Self::Ratio { .. } => "ratio",
        }
    }

    pub fn variable_ids(&self) -> Vec<&str> {
        match self {
            Self::Lookup { variable, .. } | Self::Compare { variable, .. } | Self::Rank { variable, .. } | Self::Trend { variable, .. } => {
                vec![variable]
            }
            Self::Ratio { numerator, denominator, .. } => vec![numerator, denominator],
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceRef {
    pub source_id: String,
    pub dataset_id: String,
    pub title: String,
    pub publisher: String,
    pub licence: String,
    pub licence_url: String,
    pub licence_status: String,
    pub attribution: String,
    pub source_url: String,
    pub retrieved_at: String,
    pub coverage: String,
    pub state: String,
}

impl From<&DatasetManifest> for SourceRef {
    fn from(manifest: &DatasetManifest) -> Self {
        Self {
            source_id: manifest.source_id.clone(),
            dataset_id: manifest.dataset_id.clone(),
            title: manifest.title.clone(),
            publisher: manifest.publisher.clone(),
            licence: manifest.licence.clone(),
            licence_url: manifest.licence_url.clone(),
            licence_status: manifest.licence_status.clone(),
            attribution: manifest.attribution.clone(),
            source_url: manifest.source_url.clone(),
            retrieved_at: manifest.retrieved_at.clone(),
            coverage: manifest.coverage.clone(),
            state: manifest.state.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AnswerRow {
    pub geography_code: String,
    pub geography_name: String,
    pub geography_level: String,
    pub period: String,
    pub variable_id: String,
    pub variable_label: String,
    pub classification_id: String,
    pub classification_label: String,
    pub category_id: String,
    pub category_label: String,
    pub unit: String,
    pub value: Option<f64>,
    pub status: String,
    pub numerator: Option<f64>,
    pub denominator: Option<f64>,
}

impl AnswerRow {
    fn from_observation(observation: &Observation) -> Self {
        Self {
            geography_code: observation.geography_code.clone(),
            geography_name: observation.geography_name.clone(),
            geography_level: observation.geography_level.clone(),
            period: observation.period.clone(),
            variable_id: observation.variable_id.clone(),
            variable_label: observation.variable_label.clone(),
            classification_id: observation.classification_id.clone(),
            classification_label: observation.classification_label.clone(),
            category_id: observation.category_id.clone(),
            category_label: observation.category_label.clone(),
            unit: observation.unit.clone(),
            value: observation.value,
            status: observation.status.as_str().into(),
            numerator: None,
            denominator: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Answer {
    pub status: String,
    pub operation: String,
    pub title: String,
    pub unit: String,
    pub periods: Vec<String>,
    pub coverage: String,
    pub calculation: Option<String>,
    pub rows: Vec<AnswerRow>,
    pub sources: Vec<SourceRef>,
    pub limitations: Vec<String>,
    pub notes: Vec<String>,
    pub text: String,
}

/// A source-aware refusal before any domain data is imported.
pub fn unavailable(operation: &str, message: impl Into<String>) -> Answer {
    let message = message.into();
    Answer {
        status: "unavailable".into(),
        operation: operation.into(),
        title: "Fonte indisponível".into(),
        unit: String::new(),
        periods: Vec::new(),
        coverage: String::new(),
        calculation: None,
        rows: Vec::new(),
        sources: Vec::new(),
        limitations: vec![message.clone()],
        notes: Vec::new(),
        text: format!("Não foi possível responder: {message}"),
    }
}

pub fn execute(stores: &StoreSet, registry: &Registry, request: &Request) -> Result<Answer> {
    match request {
        Request::Lookup { variable, geographies, periods, category } => {
            lookup(stores, registry, variable, geographies, periods, category.as_ref())
        }
        Request::Compare { variable, geographies, period, category } => {
            compare(stores, registry, variable, geographies, period.as_deref(), category.as_ref())
        }
        Request::Rank { variable, period, geography_level, uf_code, category, limit, descending } => {
            rank(stores, registry, variable, period.as_deref(), geography_level, uf_code.as_deref(), category.as_ref(), *limit, *descending)
        }
        Request::Trend { variable, geography, periods, category } => {
            trend(stores, registry, variable, geography, periods, category.as_ref())
        }
        Request::Ratio { numerator, denominator, geographies, period, category, scale } => {
            ratio(stores, registry, numerator, denominator, geographies, period.as_deref(), category.as_ref(), *scale)
        }
    }
}

fn lookup(
    stores: &StoreSet,
    registry: &Registry,
    variable: &str,
    geographies: &[String],
    periods: &[String],
    category: Option<&CategoryFilter>,
) -> Result<Answer> {
    let observations = stores.unique_observations(variable, category.map(CategoryFilter::pair))?;
    let selected = filter(&observations, geographies, periods);
    let meta = meta_for(registry, variable);
    let rows = selected.iter().map(AnswerRow::from_observation).collect::<Vec<_>>();
    let notes = if selected.is_empty() {
        vec!["Nenhuma célula corresponde à seleção; isso não é o mesmo que zero observado.".into()]
    } else {
        Vec::new()
    };
    build_answer(
        registry,
        stores,
        request_variable_ids(&[variable]),
        "lookup",
        format!("Consulta: {}", meta.label),
        meta.unit,
        rows,
        None,
        notes,
        operation_limitations("lookup", &[variable]),
    )
}

fn compare(
    stores: &StoreSet,
    registry: &Registry,
    variable: &str,
    geographies: &[String],
    period: Option<&str>,
    category: Option<&CategoryFilter>,
) -> Result<Answer> {
    let observations = stores.unique_observations(variable, category.map(CategoryFilter::pair))?;
    let selected_period = period.map(str::to_string).or_else(|| latest_period(&observations));
    let selected = filter(&observations, geographies, &[])
        .into_iter()
        .filter(|observation| selected_period.as_ref().is_none_or(|period| &observation.period == period))
        .collect::<Vec<_>>();
    let meta = meta_for(registry, variable);
    let mut rows = selected.iter().map(AnswerRow::from_observation).collect::<Vec<_>>();
    let order = geographies.iter().enumerate().map(|(index, code)| (code, index)).collect::<BTreeMap<_, _>>();
    rows.sort_by_key(|row| (order.get(&row.geography_code).copied().unwrap_or(usize::MAX), row.geography_name.clone()));
    let notes =
        if selected_period.is_none() { vec!["Nenhum período observado foi encontrado para a variável.".into()] } else { Vec::new() };
    build_answer(
        registry,
        stores,
        request_variable_ids(&[variable]),
        "compare",
        format!("Comparação: {}", meta.label),
        meta.unit,
        rows,
        None,
        notes,
        operation_limitations("compare", &[variable]),
    )
}

#[allow(clippy::too_many_arguments)]
fn rank(
    stores: &StoreSet,
    registry: &Registry,
    variable: &str,
    period: Option<&str>,
    geography_level: &str,
    uf_code: Option<&str>,
    category: Option<&CategoryFilter>,
    limit: usize,
    descending: bool,
) -> Result<Answer> {
    ensure!(limit > 0, "rank limit must be positive");
    let observations = stores.unique_observations(variable, category.map(CategoryFilter::pair))?;
    let selected_period = period.map(str::to_string).or_else(|| latest_period(&observations));
    let mut scoped = observations
        .into_iter()
        .filter(|observation| observation.geography_level == geography_level)
        .filter(|observation| uf_code.is_none_or(|uf| observation.geography_code.starts_with(uf)))
        .filter(|observation| selected_period.as_ref().is_none_or(|period| &observation.period == period))
        .collect::<Vec<_>>();
    let missing = scoped.iter().filter(|observation| !observation.status.is_observed()).count();
    scoped.retain(|observation| observation.status.is_observed() && observation.value.is_some());
    scoped.sort_by(|left, right| {
        let left_value = left.value.unwrap_or(f64::NEG_INFINITY);
        let right_value = right.value.unwrap_or(f64::NEG_INFINITY);
        let order = left_value.total_cmp(&right_value);
        let order = if descending { order.reverse() } else { order };
        order.then_with(|| left.geography_code.cmp(&right.geography_code))
    });
    let mut rows = scoped.iter().take(limit).map(AnswerRow::from_observation).collect::<Vec<_>>();
    if !descending {
        // The textual answer reads best from the highest to the lowest value.
        rows.reverse();
    }
    let meta = meta_for(registry, variable);
    let mut notes = Vec::new();
    if missing > 0 {
        notes.push(format!("{missing} células sem valor observado foram excluídas do ranking."));
    }
    build_answer(
        registry,
        stores,
        request_variable_ids(&[variable]),
        "rank",
        format!("Ranking: {}", meta.label),
        meta.unit,
        rows,
        Some(format!(
            "ordenado por valor {}; células sem valor observado não entram no ranking",
            if descending { "decrescente" } else { "crescente" }
        )),
        notes,
        operation_limitations("rank", &[variable]),
    )
}

fn trend(
    stores: &StoreSet,
    registry: &Registry,
    variable: &str,
    geography: &str,
    periods: &[String],
    category: Option<&CategoryFilter>,
) -> Result<Answer> {
    let observations = stores.unique_observations(variable, category.map(CategoryFilter::pair))?;
    let geography = geography.to_string();
    let selected = filter(&observations, std::slice::from_ref(&geography), periods);
    let meta = meta_for(registry, variable);
    let rows = selected.iter().map(AnswerRow::from_observation).collect::<Vec<_>>();
    let mut notes = if selected.is_empty() {
        vec!["Não há série observada para essa geografia; ausência de célula não significa zero.".into()]
    } else {
        Vec::new()
    };
    let returned = selected.iter().map(|observation| observation.period.as_str()).collect::<BTreeSet<_>>();
    let absent = periods.iter().filter(|period| !returned.contains(period.as_str())).cloned().collect::<Vec<_>>();
    if !absent.is_empty() {
        notes.push(format!("Períodos solicitados sem célula publicada: {}. Não foram imputados.", absent.join(", ")));
    }
    build_answer(
        registry,
        stores,
        request_variable_ids(&[variable]),
        "trend",
        format!("Série temporal: {}", meta.label),
        meta.unit,
        rows,
        Some("observações ordenadas pelo período de referência publicado".into()),
        notes,
        operation_limitations("trend", &[variable]),
    )
}

#[allow(clippy::too_many_arguments)]
fn ratio(
    stores: &StoreSet,
    registry: &Registry,
    numerator: &str,
    denominator: &str,
    geographies: &[String],
    period: Option<&str>,
    category: Option<&CategoryFilter>,
    scale: f64,
) -> Result<Answer> {
    ensure!(scale > 0.0 && scale.is_finite(), "ratio scale must be positive and finite");
    let numerator_rows = stores.unique_observations(numerator, category.map(CategoryFilter::pair))?;
    let denominator_rows = stores.unique_observations(denominator, category.map(CategoryFilter::pair))?;
    let numerator_meta = meta_for(registry, numerator);
    let denominator_meta = meta_for(registry, denominator);
    let selected_period = period.map(str::to_string).or_else(|| latest_common_period(&numerator_rows, &denominator_rows));
    let mut numerator_map = BTreeMap::new();
    for observation in filter(&numerator_rows, geographies, &[])
        .into_iter()
        .filter(|observation| selected_period.as_ref().is_none_or(|period| &observation.period == period))
    {
        numerator_map.insert((observation.geography_level.clone(), observation.geography_code.clone()), observation);
    }
    let mut denominator_map = BTreeMap::new();
    for observation in filter(&denominator_rows, geographies, &[])
        .into_iter()
        .filter(|observation| selected_period.as_ref().is_none_or(|period| &observation.period == period))
    {
        denominator_map.insert((observation.geography_level.clone(), observation.geography_code.clone()), observation);
    }
    let keys = numerator_map.keys().chain(denominator_map.keys()).cloned().collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for key in keys {
        let numerator_row = numerator_map.get(&key);
        let denominator_row = denominator_map.get(&key);
        let (geography_level, geography_code) = key;
        let geography_name = numerator_row.or(denominator_row).map(|row| row.geography_name.clone()).unwrap_or_default();
        let period =
            numerator_row.or(denominator_row).map(|row| row.period.clone()).or_else(|| selected_period.clone()).unwrap_or_default();
        let numerator_value = numerator_row.and_then(|row| row.status.is_observed().then_some(row.value).flatten());
        let denominator_value = denominator_row.and_then(|row| row.status.is_observed().then_some(row.value).flatten());
        let (value, status) = match (numerator_value, denominator_value) {
            (Some(numerator_value), Some(denominator_value)) if denominator_value != 0.0 => {
                (Some(numerator_value / denominator_value * scale), "observed")
            }
            (Some(_), Some(_)) => (None, "undefined_zero_denominator"),
            (None, Some(_)) => (None, "missing_numerator"),
            (Some(_), None) => (None, "missing_denominator"),
            (None, None) => (None, "missing_pair"),
        };
        rows.push(AnswerRow {
            geography_code,
            geography_name,
            geography_level,
            period,
            variable_id: format!("{numerator}/{denominator}"),
            variable_label: format!("{} / {}", numerator_meta.label, denominator_meta.label),
            classification_id: category.map(|value| value.classification_id.clone()).unwrap_or_default(),
            classification_label: String::new(),
            category_id: category.map(|value| value.category_id.clone()).unwrap_or_default(),
            category_label: String::new(),
            unit: format!("{} por {}", numerator_meta.unit, denominator_meta.unit),
            value,
            status: status.into(),
            numerator: numerator_value,
            denominator: denominator_value,
        });
    }
    rows.sort_by(|left, right| left.geography_name.cmp(&right.geography_name));
    let unit = format!("{} por {}", numerator_meta.unit, denominator_meta.unit);
    let calculation = Some(format!("({} / {}) × {}", numerator_meta.label, denominator_meta.label, scale));
    build_answer(
        registry,
        stores,
        request_variable_ids(&[numerator, denominator]),
        "ratio",
        format!("Razão: {} por {}", numerator_meta.label, denominator_meta.label),
        unit,
        rows,
        calculation,
        Vec::new(),
        operation_limitations("ratio", &[numerator, denominator]),
    )
}

fn request_variable_ids<'a>(variables: &[&'a str]) -> Vec<&'a str> {
    variables.to_vec()
}

struct VariableMeta {
    label: String,
    unit: String,
}

fn meta_for(registry: &Registry, variable: &str) -> VariableMeta {
    if let Some(variable) = registry.variable(variable) {
        return VariableMeta { label: variable.label.clone(), unit: variable.unit.clone() };
    }
    VariableMeta { label: variable.to_string(), unit: String::new() }
}

fn filter(observations: &[Observation], geographies: &[String], periods: &[String]) -> Vec<Observation> {
    observations
        .iter()
        .filter(|observation| geographies.is_empty() || geographies.iter().any(|code| code == &observation.geography_code))
        .filter(|observation| periods.is_empty() || periods.iter().any(|period| period == &observation.period))
        .cloned()
        .collect()
}

fn latest_period(observations: &[Observation]) -> Option<String> {
    observations.iter().filter(|observation| observation.status.is_observed()).map(|observation| observation.period.clone()).max()
}

fn latest_common_period(numerator: &[Observation], denominator: &[Observation]) -> Option<String> {
    let numerator_periods = numerator
        .iter()
        .filter(|observation| observation.status.is_observed())
        .map(|observation| observation.period.as_str())
        .collect::<BTreeSet<_>>();
    let denominator_periods = denominator
        .iter()
        .filter(|observation| observation.status.is_observed())
        .map(|observation| observation.period.as_str())
        .collect::<BTreeSet<_>>();
    numerator_periods.intersection(&denominator_periods).max().map(|period| (*period).to_string())
}

#[allow(clippy::too_many_arguments)]
fn build_answer(
    registry: &Registry,
    stores: &StoreSet,
    variables: Vec<&str>,
    operation: &str,
    title: String,
    unit: String,
    rows: Vec<AnswerRow>,
    calculation: Option<String>,
    notes: Vec<String>,
    mut limitations: Vec<String>,
) -> Result<Answer> {
    let dataset_ids = stores
        .manifests()
        .filter(|manifest| variables.iter().any(|variable| variable_belongs_to(registry, variable, &manifest.dataset_id)))
        .map(|manifest| manifest.dataset_id.clone())
        .collect::<BTreeSet<_>>();
    let manifests = stores.manifests_for(&dataset_ids);
    let mut sources = manifests.iter().map(|manifest| SourceRef::from(*manifest)).collect::<Vec<_>>();
    sources.sort_by(|left, right| left.dataset_id.cmp(&right.dataset_id));
    for manifest in &manifests {
        for limitation in &manifest.limitations {
            if !limitations.contains(limitation) {
                limitations.push(limitation.clone());
            }
        }
    }
    limitations.sort();
    limitations.dedup();
    let periods =
        rows.iter().map(|row| row.period.clone()).filter(|period| !period.is_empty()).collect::<BTreeSet<_>>().into_iter().collect();
    let coverage =
        manifests.iter().map(|manifest| manifest.coverage.as_str()).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>().join("; ");
    let status = if rows.is_empty() {
        "empty"
    } else if rows.iter().any(|row| row.status == "observed") {
        if rows.iter().all(|row| row.status == "observed") { "ok" } else { "partial" }
    } else {
        "no_observed_value"
    };
    let unit =
        if unit.is_empty() { rows.iter().map(|row| row.unit.clone()).find(|unit| !unit.is_empty()).unwrap_or_default() } else { unit };
    let mut answer = Answer {
        status: status.into(),
        operation: operation.into(),
        title,
        unit,
        periods,
        coverage,
        calculation,
        rows,
        sources,
        limitations,
        notes,
        text: String::new(),
    };
    answer.text = render_pt(&answer);
    Ok(answer)
}

fn variable_belongs_to(registry: &Registry, variable: &str, dataset_id: &str) -> bool {
    registry.variable(variable).is_some_and(|value| value.dataset_id == dataset_id)
}

fn operation_limitations(operation: &str, variables: &[&str]) -> Vec<String> {
    let mut limitations = vec!["Uma estatística municipal não é um fato observado sobre um CNPJ individual.".into()];
    if matches!(operation, "compare" | "rank" | "trend") {
        limitations.push("Associação, ranking ou tendência não prova causalidade, demanda ou vínculo comercial.".into());
    }
    if variables.iter().any(|variable| variable.starts_with("pib_") || variable.starts_with("va_")) {
        limitations.push("PIB municipal e valor adicionado não são renda domiciliar nem poder de compra.".into());
    }
    if variables.iter().any(|variable| variable.starts_with("cempre_")) {
        limitations.push("CEMPRE não é o status cadastral ativo da Receita Federal.".into());
    }
    if variables.iter().any(|variable| variable.starts_with("pam_") || variable.starts_with("ppm_") || variable.starts_with("lspa_")) {
        limitations
            .push("Produção agropecuária regional não identifica propriedade rural, rebanho ou intenção de compra de um CNPJ.".into());
    }
    limitations
}

/// Render a compact Portuguese answer with period, unit, source and limits.
pub fn render_pt(answer: &Answer) -> String {
    let mut lines = Vec::new();
    lines.push(answer.title.clone());
    if answer.periods.is_empty() {
        lines.push("Período: não informado pela seleção.".into());
    } else {
        lines.push(format!("Período: {}.", answer.periods.join(", ")));
    }
    if !answer.unit.is_empty() {
        lines.push(format!("Unidade: {}.", answer.unit));
    }
    if answer.rows.is_empty() {
        lines.push("Resultado: nenhuma célula observada para a seleção.".into());
    } else {
        for row in answer.rows.iter().take(30) {
            let value = match row.value {
                Some(value) => format_br(value),
                None => format!("sem valor ({})", row.status),
            };
            let category = if row.category_label.is_empty() { String::new() } else { format!(" - {}", row.category_label) };
            lines.push(format!("{} ({}) [{}]{}: {value}", row.geography_name, row.geography_code, row.period, category));
        }
        if answer.rows.len() > 30 {
            lines.push(format!("... {} linhas adicionais no JSON.", answer.rows.len() - 30));
        }
    }
    if let Some(calculation) = &answer.calculation {
        lines.push(format!("Cálculo: {calculation}."));
    }
    if !answer.coverage.is_empty() {
        lines.push(format!("Cobertura: {}.", answer.coverage));
    }
    for source in &answer.sources {
        lines.push(format!(
            "Fonte: {} ({}, {}). Licença: {} ({}).",
            source.title, source.dataset_id, source.source_url, source.licence, source.licence_status
        ));
    }
    for limitation in &answer.limitations {
        lines.push(format!("Limitação: {limitation}"));
    }
    for note in &answer.notes {
        lines.push(format!("Observação: {note}"));
    }
    lines.join("\n")
}

/// Format a number with Brazilian thousands and decimal separators.
pub fn format_br(value: f64) -> String {
    let rounded = if value.fract().abs() < 1e-9 { format!("{value:.0}") } else { format!("{value:.2}") };
    let (sign, digits) = match rounded.strip_prefix('-') {
        Some(rest) => ("-", rest.to_string()),
        None => ("", rounded),
    };
    let (integer, decimal) = match digits.split_once('.') {
        Some((integer, decimal)) => (integer.to_string(), Some(decimal.to_string())),
        None => (digits, None),
    };
    let mut grouped = String::new();
    for (index, character) in integer.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push('.');
        }
        grouped.push(character);
    }
    let grouped = grouped.chars().rev().collect::<String>();
    match decimal {
        Some(decimal) => format!("{sign}{grouped},{decimal}"),
        None => format!("{sign}{grouped}"),
    }
}

/// Convert a two-letter UF to its IBGE code when known.
pub fn uf_code(uf: &str) -> Option<&'static str> {
    Some(match uf {
        "RO" => "11",
        "AC" => "12",
        "AM" => "13",
        "RR" => "14",
        "PA" => "15",
        "AP" => "16",
        "TO" => "17",
        "MA" => "21",
        "PI" => "22",
        "CE" => "23",
        "RN" => "24",
        "PB" => "25",
        "PE" => "26",
        "AL" => "27",
        "SE" => "28",
        "BA" => "29",
        "MG" => "31",
        "ES" => "32",
        "RJ" => "33",
        "SP" => "35",
        "PR" => "41",
        "SC" => "42",
        "RS" => "43",
        "MS" => "50",
        "MT" => "51",
        "GO" => "52",
        "DF" => "53",
        _ => return None,
    })
}

/// Shared validation to avoid treating conflicting units or periods as a join.
pub fn same_unit(rows: &[Observation]) -> Result<()> {
    let units = rows.iter().map(|row| row.unit.as_str()).collect::<BTreeSet<_>>();
    if units.len() > 1 {
        bail!("selection mixes incompatible units: {units:?}");
    }
    Ok(())
}

pub fn require_observed(rows: &[Observation]) -> Result<()> {
    ensure!(!rows.is_empty(), "selection returned no cells");
    Ok(())
}

pub fn status_name(status: CellStatus) -> &'static str {
    status.as_str()
}

pub fn source_from_manifest(manifest: &DatasetManifest) -> SourceRef {
    SourceRef::from(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brazilian_number_format() {
        assert_eq!(format_br(1234567.0), "1.234.567");
        assert_eq!(format_br(1234.5), "1.234,50");
        assert_eq!(format_br(-0.25), "-0,25");
    }

    #[test]
    fn uf_codes_are_distinct() {
        assert_eq!(uf_code("MT"), Some("51"));
        assert_eq!(uf_code("SP"), Some("35"));
        assert_eq!(uf_code("XX"), None);
    }
}
