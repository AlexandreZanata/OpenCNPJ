//! Typed answer blocks, templates, formatters and caveats.

use std::collections::BTreeMap;

use opencnpj_query::{
    ir::{CnaeCode, CnaeLevel, Competence, Day, Dim, FlowMetric, Grain, Metric, Op, Period, PlaceFilter, PopulationRef, Status, Tri},
    validate::ValidRequest,
};
use serde::Serialize;

use crate::execute::{Row, Value};

/// Answer decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Execute,
    ExecuteWithNote,
    Clarify,
    Reject,
}

#[derive(Debug, Clone, Default)]
pub struct MunicipalityLabel {
    pub name: String,
    pub uf: String,
}

#[derive(Debug, Clone, Default)]
pub struct SectorLabel {
    pub singular: String,
    pub plural: String,
    pub feminine: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Labels {
    pub municipalities: BTreeMap<u16, MunicipalityLabel>,
    pub sectors: BTreeMap<u16, SectorLabel>,
    pub ufs: BTreeMap<u8, String>,
    /// Region display names ("Centro-Oeste", "Região imediata de Sinop"), keyed by region id.
    pub regions: BTreeMap<u16, String>,
}

#[derive(Debug, Clone)]
pub struct Provenance {
    pub competence: Competence,
    pub anchor: Day,
    pub snapshot_sha256: String,
    pub municipality_catalogue: u32,
    pub sector_catalogue: u32,
    pub population: Option<PopulationRef>,
}

pub enum Payload<'a> {
    Value(&'a Value),
    Records { total: u64, page: u16, rows: Vec<Vec<String>> },
    Company { fields: Vec<(String, String)> },
    Meta { text: String },
}

pub struct RenderInput<'a> {
    pub request: &'a ValidRequest,
    pub payload: Payload<'a>,
    pub provenance: &'a Provenance,
    pub labels: &'a Labels,
    pub followup: bool,
    pub notice: Option<String>,
    pub suggestions: Vec<String>,
    pub unit_inferred: bool,
    pub timing_us: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Text { text: String },
    Kpi { label: String, value: String },
    Comparison { members: Vec<ComparisonMember>, difference_abs: String, difference_pct: Option<String> },
    Table { columns: Vec<String>, rows: Vec<Vec<String>>, total: Option<String> },
    Chart { kind: ChartKind, series: Vec<ChartPoint> },
    Records { columns: Vec<String>, rows: Vec<Vec<String>>, total: u64, page: u16 },
    Company { fields: Vec<(String, String)> },
    Methodology { items: Vec<String> },
    Sources { items: Vec<Source> },
    Caveats { items: Vec<Caveat> },
    Notice { text: String },
    Clarify { question: String, options: Vec<String> },
    Reject { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartKind {
    Bar,
    Line,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ComparisonMember {
    pub label: String,
    pub value: String,
    pub base: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChartPoint {
    pub label: String,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Source {
    pub name: String,
    pub competence: String,
    pub snapshot_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Caveat {
    pub id: &'static str,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Answer {
    pub decision: Decision,
    pub understood: String,
    pub blocks: Vec<Block>,
    pub suggestions: Vec<String>,
    pub query: serde_json::Value,
    pub timing_us: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    UnknownTemplate(String),
    MissingPlaceholder(String),
    InvalidPayload(&'static str),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownTemplate(key) => write!(f, "unknown template {key}"),
            Self::MissingPlaceholder(name) => write!(f, "missing placeholder {name}"),
            Self::InvalidPayload(reason) => write!(f, "invalid payload: {reason}"),
        }
    }
}

impl std::error::Error for RenderError {}

pub fn render(input: RenderInput<'_>) -> Result<Answer, RenderError> {
    let request = input.request.as_request();
    let templates = Templates::embedded();
    let key = template_key(request, &input.payload, input.followup)?;
    let mut values = base_placeholders(request, &input);
    fill_payload(request, &input, &mut values)?;
    let seed = xxhash_rust::xxh3::xxh3_64(&opencnpj_query::codec::encode(request));
    let text = capitalize(&templates.render(&key, seed, &values)?);
    let mut blocks = vec![Block::Text { text }];
    payload_blocks(request, &input, &mut blocks)?;
    blocks.push(Block::Methodology { items: methodology(request, &input) });
    blocks.push(Block::Sources {
        items: vec![Source {
            name: "Receita Federal — Dados Abertos do CNPJ".into(),
            competence: request.time.competence.to_string(),
            snapshot_sha256: input.provenance.snapshot_sha256.clone(),
        }],
    });
    let caveats = caveats(request, &input);
    if !caveats.is_empty() {
        blocks.push(Block::Caveats { items: caveats });
    }
    if let Some(notice) = &input.notice {
        blocks.push(Block::Notice { text: notice.clone() });
    }
    let suggestions = if input.suggestions.is_empty() { suggestions(request) } else { input.suggestions.clone() };
    let mut timing_us = input.timing_us.clone();
    timing_us.insert("render".into(), 0);
    Ok(Answer {
        decision: if input.notice.is_some() { Decision::ExecuteWithNote } else { Decision::Execute },
        understood: understood(request, &input),
        blocks,
        suggestions,
        query: serde_json::to_value(request).expect("request JSON"),
        timing_us,
    })
}

pub fn clarify(question: &str, options: Vec<String>, query: &ValidRequest) -> Answer {
    Answer {
        decision: Decision::Clarify,
        understood: String::new(),
        blocks: vec![Block::Clarify { question: question.into(), options }],
        suggestions: Vec::new(),
        query: serde_json::to_value(query.as_request()).expect("request JSON"),
        timing_us: BTreeMap::new(),
    }
}

pub fn reject(reason: &str, suggestions: Vec<String>, query: &ValidRequest) -> Answer {
    Answer {
        decision: Decision::Reject,
        understood: String::new(),
        blocks: vec![Block::Reject { reason: reason.into() }],
        suggestions,
        query: serde_json::to_value(query.as_request()).expect("request JSON"),
        timing_us: BTreeMap::new(),
    }
}

fn template_key(request: &opencnpj_query::ir::Request, payload: &Payload<'_>, followup: bool) -> Result<String, RenderError> {
    let turn = if followup { "followup" } else { "first" };
    if let Payload::Value(Value::Rows(rows)) = payload
        && rows.is_empty()
    {
        return Ok(format!("empty.none.rows.{turn}"));
    }
    let metric = metric_kind(request);
    let shape = match (&request.op, payload) {
        (Op::Count { .. }, Payload::Value(Value::Scalar { .. })) => "scalar",
        (Op::Count { .. }, Payload::Value(Value::Ratio { .. })) => "ratio",
        (Op::Compare { .. } | Op::Rank { .. } | Op::Breakdown { .. } | Op::Trend { .. }, Payload::Value(Value::Rows(_))) => "rows",
        (Op::List { .. }, Payload::Records { .. }) => "records",
        (Op::Lookup { .. }, Payload::Company { .. }) => "company",
        (Op::Meta { .. }, Payload::Meta { .. }) => "text",
        _ => return Err(RenderError::InvalidPayload("payload does not match the operation")),
    };
    let op = match request.op {
        Op::Count { .. } => "count",
        Op::Compare { .. } => "compare",
        Op::Rank { .. } => "rank",
        Op::Breakdown { .. } => "breakdown",
        Op::Trend { .. } => "trend",
        Op::List { .. } => "list",
        Op::Lookup { .. } => "lookup",
        Op::Meta { .. } => "meta",
    };
    Ok(format!("{op}.{metric}.{shape}.{turn}"))
}

fn metric_kind(request: &opencnpj_query::ir::Request) -> &'static str {
    match request.op {
        Op::List { .. } => "stock",
        Op::Lookup { .. } | Op::Meta { .. } => "none",
        _ => match request.op.metric() {
            Some(Metric::Stock) => "stock",
            Some(Metric::Flow(_)) => "flow",
            Some(Metric::ActiveChange { .. }) => "active_change",
            Some(Metric::Per10k(_)) => "per10k",
            Some(Metric::InhabitantsPer(_)) => "inhabitants_per",
            Some(Metric::Share) => "share",
            None => "none",
        },
    }
}

fn base_placeholders(request: &opencnpj_query::ir::Request, input: &RenderInput<'_>) -> BTreeMap<&'static str, String> {
    // Only values that exist are inserted: a template that names a missing value fails with
    // `MissingPlaceholder` instead of rendering a blank.
    let mut values = BTreeMap::new();
    let (unit_singular, unit_plural) = unit(request);
    let (status_singular, status_plural) = status_adjective(request);
    let (sector_singular, sector_plural) = sector_label(request, input.labels);
    values.insert("place", place_label(request, input.labels));
    let plural_subject = place_count(request) > 1;
    values.insert("have", if plural_subject { "têm" } else { "tem" }.into());
    values.insert("had", if plural_subject { "tiveram" } else { "teve" }.into());
    let place_em = place_em(request, input.labels);
    values.insert("place_em_cap", capitalize(&place_em));
    values.insert("place_em", place_em);
    values.insert("sector:label", sector_singular);
    values.insert("sector:label_plural", sector_plural);
    values.insert("unit", unit_plural.clone());
    values.insert("unit:singular", unit_singular);
    values.insert("unit:plural(value)", unit_plural);
    values.insert("status", status_plural.clone());
    values.insert("status:singular", status_singular);
    values.insert("status:adj(value)", status_plural);
    values.insert("metric_label", metric_label(request));
    // The activity is named once, and not at all when it is everything or when the sectors
    // are the members being compared ("padarias: 187; farmácias: 147").
    let compared_sectors = matches!(request.op, Op::Compare { dim: Dim::Sector, .. } | Op::Rank { dim: Dim::Sector, .. });
    let any_activity = matches!(request.scope.activity, opencnpj_query::ir::ActivityFilter::Any);
    let (sector_one, sector_many) = (values["sector:label"].clone(), values["sector:label_plural"].clone());
    values.insert("of_activity", if any_activity || compared_sectors { String::new() } else { format!(" de {sector_one}") });
    values.insert("in_activity", if any_activity || compared_sectors { String::new() } else { format!(" em {sector_many}") });
    values.insert("trend_subject", trend_subject(request, &values["of_activity"]));
    values.insert("flow", flow_phrase(request, 2));
    values.insert("competence:month_year", month_year(request.time.competence));
    values.insert("snapshot_sha256", input.provenance.snapshot_sha256.clone());
    values.insert(
        "catalogue",
        format!("municípios {} e setores {}", input.provenance.municipality_catalogue, input.provenance.sector_catalogue),
    );
    if let Some(reference) = population_ref(request, input.provenance) {
        values.insert("population_source", population_label(reference));
    }
    if let Op::Rank { dir, .. } = request.op {
        let ascending = dir == opencnpj_query::ir::Dir::Asc;
        values.insert("dir_most", if ascending { "menores" } else { "maiores" }.into());
        values.insert("dir_more", if ascending { "menos" } else { "mais" }.into());
    }
    values
}

fn fill_payload(
    request: &opencnpj_query::ir::Request,
    input: &RenderInput<'_>,
    values: &mut BTreeMap<&'static str, String>,
) -> Result<(), RenderError> {
    let (unit_singular, unit_plural) = unit(request);
    let (status_singular, status_plural) = status_adjective(request);
    if let Some(text) = period_date(request, true) {
        values.insert("from:date", text);
    }
    if let Some(text) = period_date(request, false) {
        values.insert("to:date", text);
    }
    if let Some(text) = period_month(request, true) {
        values.insert("from:month_year", text);
    }
    if let Some(text) = period_month(request, false) {
        values.insert("to:month_year", text);
    }
    match &input.payload {
        Payload::Value(Value::Scalar { value }) => {
            values.insert("value:int", int(*value));
            values.insert("value:dec1", decimal1(*value as f64));
            values.insert("unit:plural(value)", pluralize(*value, &unit_singular, &unit_plural).into());
            values.insert("status:adj(value)", pluralize(*value, &status_singular, &status_plural).into());
            values.insert("flow", flow_phrase(request, *value));
        }
        Payload::Value(Value::Ratio { num, den }) => {
            // No denominator (missing population, zero establishments): there is no number to
            // show, and inventing one (0,0 or the raw count) would break SR-11.
            let value = display_ratio(request, *num, *den).ok_or(RenderError::InvalidPayload("ratio without a denominator"))?;
            values.insert("value:dec1", decimal1(value));
            values.insert("value:int", int_u(*num));
            if let Some(population) = match request.op.metric() {
                Some(Metric::Per10k(_)) => Some(*den),
                Some(Metric::InhabitantsPer(_)) => Some(*num),
                _ => None,
            } {
                values.insert("population:int", int_u(population));
            }
            values.insert("unit:plural(value)", pluralize(2, &unit_singular, &unit_plural).into());
            values.insert("status:adj(value)", pluralize(2, &status_singular, &status_plural).into());
        }
        Payload::Value(Value::Rows(rows)) => {
            let dim = request.op.dimension();
            let labels: Vec<String> = rows.iter().map(|row| row_label(request, row.key, dim, input.labels)).collect();
            let displays: Vec<String> = rows.iter().map(|row| row_value(request, *row)).collect();
            values.insert("n:int", rows.len().to_string());
            values.insert("top", list(&labels.iter().take(3).zip(displays.iter()).map(|(l, v)| format!("{l} ({v})")).collect::<Vec<_>>()));
            if matches!(request.op, Op::Compare { .. }) && rows.len() >= 2 {
                let (a, b) = (rows[0], rows[1]);
                let a_metric = metric_value(request, a);
                let b_metric = metric_value(request, b);
                values.insert("a.place", labels[0].clone());
                values.insert("b.place", labels[1].clone());
                values.insert("a.value:int", displays[0].clone());
                values.insert("a.value:dec1", a_metric.map_or_else(|| NO_DATA.into(), decimal1));
                values.insert("a.share:pct1", a_metric.map_or_else(|| NO_DATA.into(), pct1));
                values.insert("b.value:int", displays[1].clone());
                values.insert("b.value:dec1", b_metric.map_or_else(|| NO_DATA.into(), decimal1));
                values.insert("b.share:pct1", b_metric.map_or_else(|| NO_DATA.into(), pct1));
                values.insert("diff", difference_text(a_metric, b_metric));
            }
            if matches!(request.op, Op::Trend { .. })
                && let (Some(first), Some(last)) = (rows.first(), rows.last())
            {
                let (first, last) = (first.value, last.value);
                values.insert("first:int", int(first));
                values.insert("last:int", int(last));
                values.insert(
                    "change_pct:pct1",
                    if first == 0 { "variação percentual indefinida".into() } else { pct1((last - first) as f64 * 100.0 / first as f64) },
                );
            }
        }
        Payload::Records { total, page, rows } => {
            values.insert("total:int", int_u(*total));
            values.insert("n:int", rows.len().to_string());
            values.insert("page:int", page.to_string());
            values.insert("unit:plural(value)", pluralize(2, &unit_singular, &unit_plural).into());
            values.insert("status:adj(value)", pluralize(2, &status_singular, &status_plural).into());
        }
        Payload::Company { fields } => {
            for (key, value) in fields {
                match key.as_str() {
                    "cnpj" => values.insert("cnpj", value.clone()),
                    "name" | "razao_social" => values.insert("name", mask_cpf(value)),
                    "status" | "situacao" => values.insert("status", value.clone()),
                    _ => None,
                };
            }
        }
        Payload::Meta { .. } => {}
    }
    Ok(())
}

fn payload_blocks(request: &opencnpj_query::ir::Request, input: &RenderInput<'_>, blocks: &mut Vec<Block>) -> Result<(), RenderError> {
    match &input.payload {
        Payload::Value(Value::Scalar { value }) => {
            blocks.push(Block::Kpi { label: kpi_label(request), value: int(*value) });
        }
        Payload::Value(Value::Ratio { num, den }) => {
            let value = display_ratio(request, *num, *den).ok_or(RenderError::InvalidPayload("ratio without a denominator"))?;
            blocks.push(Block::Kpi { label: kpi_label(request), value: decimal1(value) });
        }
        Payload::Value(Value::Rows(rows)) => {
            let dim = request.op.dimension();
            let labels: Vec<String> = rows.iter().map(|row| row_label(request, row.key, dim, input.labels)).collect();
            let displays: Vec<String> = rows.iter().map(|row| row_value(request, *row)).collect();
            let table_rows: Vec<Vec<String>> = labels.iter().cloned().zip(displays.iter().cloned()).map(|(a, b)| vec![a, b]).collect();
            if matches!(request.op, Op::Compare { .. }) && rows.len() >= 2 {
                let a_metric = metric_value(request, rows[0]);
                let b_metric = metric_value(request, rows[1]);
                let diff = a_metric.zip(b_metric).map(|(a, b)| (a - b, b));
                blocks.push(Block::Comparison {
                    members: rows
                        .iter()
                        .map(|row| ComparisonMember {
                            label: row_label(request, row.key, dim, input.labels),
                            value: row_value(request, *row),
                            base: (row.base != 0).then(|| int_u(row.base)),
                        })
                        .collect(),
                    difference_abs: diff.map_or_else(|| NO_DATA.into(), |(d, _)| decimal1(d)),
                    difference_pct: diff.filter(|(_, b)| *b != 0.0).map(|(d, b)| pct1(d / b.abs() * 100.0)),
                });
            }
            // Rows without a value (no denominator) stay in the table as "sem dado" but are
            // not drawn: a chart point needs a number.
            let chart = labels
                .iter()
                .zip(rows.iter())
                .filter_map(|(label, row)| metric_value(request, *row).map(|value| ChartPoint { label: label.clone(), value }))
                .collect::<Vec<_>>();
            if matches!(request.op, Op::Trend { .. }) {
                blocks.push(Block::Chart { kind: ChartKind::Line, series: chart });
            } else if matches!(request.op, Op::Rank { .. } | Op::Breakdown { .. } | Op::Compare { .. }) {
                blocks.push(Block::Chart { kind: ChartKind::Bar, series: chart });
            }
            blocks.push(Block::Table { columns: vec![column_label(dim, request), "Valor".into()], rows: table_rows, total: None });
        }
        Payload::Records { total, page, rows } => {
            // SR-14 / D-05: owner CPFs embedded in MEI/EI names are masked; the CNPJ column is kept.
            let rows = rows
                .iter()
                .map(|row| row.iter().enumerate().map(|(i, cell)| if i == 0 { cell.clone() } else { mask_cpf(cell) }).collect())
                .collect();
            blocks.push(Block::Records { columns: vec!["CNPJ".into(), "Nome".into()], rows, total: *total, page: *page });
        }
        Payload::Company { fields } => blocks.push(Block::Company {
            fields: fields.iter().map(|(key, value)| (key.clone(), if key == "cnpj" { value.clone() } else { mask_cpf(value) })).collect(),
        }),
        Payload::Meta { .. } => {}
    }
    Ok(())
}

/// An aggregate answer as a table for files: the row label, the displayed metric as a
/// number (`None` where a ratio has no denominator) and the row's base.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueTable {
    pub columns: [String; 3],
    pub rows: Vec<(String, Option<f64>, u64)>,
    /// Counts are whole numbers; ratios, percentages and densities are not.
    pub integer: bool,
}

pub fn value_table(query: &ValidRequest, value: &Value, labels: &Labels) -> ValueTable {
    let request = query.as_request();
    let integer = matches!(request.op.metric(), Some(Metric::Stock | Metric::Flow(_) | Metric::ActiveChange { pct: false }));
    let base = match request.op.metric() {
        Some(Metric::Per10k(_) | Metric::InhabitantsPer(_)) => "População",
        Some(Metric::Share) => "Total do recorte",
        Some(Metric::ActiveChange { pct: true }) => "Estoque inicial",
        _ => "Base",
    };
    let dim = request.op.dimension();
    let rows = match value {
        Value::Scalar { value } => vec![(place_label(request, labels), Some(*value as f64), 0)],
        Value::Ratio { num, den } => vec![(place_label(request, labels), display_ratio(request, *num, *den), *den)],
        Value::Rows(rows) => {
            rows.iter().map(|row| (row_label(request, row.key, dim, labels), metric_value(request, *row), row.base)).collect()
        }
    };
    ValueTable { columns: [column_label(dim, request), kpi_label(request), base.into()], rows, integer }
}

/// A short title for the answer ("Estabelecimentos baixados de padaria em Sorriso (MT)").
pub fn title(query: &ValidRequest, labels: &Labels) -> String {
    let request = query.as_request();
    let any_activity = matches!(request.scope.activity, opencnpj_query::ir::ActivityFilter::Any);
    let activity = if any_activity { String::new() } else { format!(" de {}", sector_label(request, labels).0) };
    capitalize(&format!("{} {}{activity} {}", unit(request).1, status_adjective(request).1, place_em(request, labels)))
}

/// Shown where a ratio has no denominator; never replaced by a made-up number (SR-11).
const NO_DATA: &str = "sem dado";

fn display_ratio(request: &opencnpj_query::ir::Request, num: u64, den: u64) -> Option<f64> {
    if den == 0 {
        return None;
    }
    Some(match request.op.metric() {
        Some(Metric::Per10k(_)) => num as f64 * 10_000.0 / den as f64,
        Some(Metric::InhabitantsPer(_)) => num as f64 / den as f64,
        Some(Metric::Share) => num as f64 * 100.0 / den as f64,
        _ => num as f64 / den as f64,
    })
}

/// The value as displayed: per 10k, inhabitants per establishment, percent for shares and
/// percent changes, the count otherwise. `None` when a ratio has no denominator.
fn metric_value(request: &opencnpj_query::ir::Request, row: Row) -> Option<f64> {
    match request.op.metric() {
        Some(Metric::Per10k(_) | Metric::InhabitantsPer(_) | Metric::Share) => ratio_value(request, row),
        Some(Metric::ActiveChange { pct: true }) => (row.base > 0).then(|| row.value as f64 * 100.0 / row.base as f64),
        _ => Some(row.value as f64),
    }
}

fn ratio_value(request: &opencnpj_query::ir::Request, row: Row) -> Option<f64> {
    if row.base == 0 {
        return None;
    }
    let (value, base) = (row.value as f64, row.base as f64);
    match request.op.metric() {
        Some(Metric::Per10k(_)) => Some(value * 10_000.0 / base),
        Some(Metric::InhabitantsPer(_)) => Some(value / base),
        Some(Metric::Share) => Some(value * 100.0 / base),
        _ => Some(value),
    }
}

fn row_value(request: &opencnpj_query::ir::Request, row: Row) -> String {
    let Some(value) = metric_value(request, row) else { return NO_DATA.into() };
    match request.op.metric() {
        Some(Metric::Per10k(_) | Metric::InhabitantsPer(_)) => decimal1(value),
        Some(Metric::Share | Metric::ActiveChange { pct: true }) => pct1(value),
        _ => int(row.value),
    }
}

/// "12,3% a mais" / "12,3% a menos" relative to the second member.
fn difference_text(a: Option<f64>, b: Option<f64>) -> String {
    match (a, b) {
        (Some(a), Some(b)) if b != 0.0 => {
            let pct = (a - b) / b.abs() * 100.0;
            if pct.abs() < 0.05 {
                "valores iguais".into()
            } else if pct > 0.0 {
                format!("{} a mais", pct1(pct))
            } else {
                format!("{} a menos", pct1(-pct))
            }
        }
        (Some(a), Some(b)) if a == b => "valores iguais".into(),
        _ => "sem comparação percentual".into(),
    }
}

fn row_label(request: &opencnpj_query::ir::Request, key: u64, dim: Option<Dim>, labels: &Labels) -> String {
    match dim {
        Some(Dim::Municipality) => labels
            .municipalities
            .get(&(key as u16))
            .map(|label| format!("{} ({})", label.name, label.uf))
            .unwrap_or_else(|| key.to_string()),
        Some(Dim::Uf) => labels.ufs.get(&(key as u8)).cloned().unwrap_or_else(|| key.to_string()),
        Some(Dim::Sector) => {
            labels.sectors.get(&(key as u16)).map_or_else(|| format!("setor sem rótulo (id {key})"), |label| label.plural.clone())
        }
        Some(Dim::Cnae(level)) => cnae_key_label(level, key),
        Some(Dim::Porte) => match key {
            0 => "Não informado".into(),
            1 => "Microempresa".into(),
            2 => "Pequena empresa".into(),
            _ => "Demais".into(),
        },
        Some(Dim::Status) => match key {
            1 => "Nula".into(),
            2 => "Ativa".into(),
            3 => "Suspensa".into(),
            4 => "Inapta".into(),
            _ => "Baixada".into(),
        },
        Some(Dim::HeadOffice) => {
            if key == 0 {
                "Matriz".into()
            } else {
                "Filial".into()
            }
        }
        Some(Dim::Mei) => {
            if key == 0 {
                "Não MEI".into()
            } else {
                "MEI".into()
            }
        }
        Some(Dim::Region(_)) => {
            u16::try_from(key).ok().and_then(|id| labels.regions.get(&id)).cloned().unwrap_or_else(|| format!("região {key}"))
        }
        Some(Dim::Year) => key.to_string(),
        Some(Dim::Month) => month_label(key),
        None => match request.op {
            Op::Trend { grain, .. } => match grain {
                Grain::Month => month_label(key),
                Grain::Year => key.to_string(),
            },
            Op::Compare { dim, .. } | Op::Rank { dim, .. } | Op::Breakdown { dim, .. } => row_label(request, key, Some(dim), labels),
            _ => key.to_string(),
        },
    }
}

fn cnae_key_label(level: CnaeLevel, key: u64) -> String {
    match level {
        CnaeLevel::Section => char::from(u8::try_from(key).unwrap_or(b'?')).to_string(),
        CnaeLevel::Division => format!("{key:02}"),
        CnaeLevel::Group => format!("{key:03}"),
        CnaeLevel::Class => format!("{key:05}"),
        CnaeLevel::Subclass => format!("{key:07}"),
    }
}

fn column_label(dim: Option<Dim>, _request: &opencnpj_query::ir::Request) -> String {
    match dim {
        Some(Dim::Municipality) => "Município",
        Some(Dim::Uf) => "UF",
        Some(Dim::Sector) => "Setor",
        Some(Dim::Cnae(_)) => "CNAE",
        Some(Dim::Porte) => "Porte",
        Some(Dim::Status) => "Situação",
        Some(Dim::HeadOffice) => "Tipo",
        Some(Dim::Mei) => "MEI",
        Some(Dim::Region(_)) => "Região",
        Some(Dim::Year) => "Ano",
        Some(Dim::Month) => "Mês",
        None => "Grupo",
    }
    .into()
}

fn understood(request: &opencnpj_query::ir::Request, input: &RenderInput<'_>) -> String {
    let unit = unit(request).1;
    let place = place_label(request, input.labels);
    let sector = sector_label(request, input.labels).0;
    format!("{unit} · {sector} · {place} · RFB {}", input.provenance.competence)
}

fn kpi_label(request: &opencnpj_query::ir::Request) -> String {
    let (unit_singular, unit_plural) = unit(request);
    let (status_singular, status_plural) = status_adjective(request);
    match request.op.metric() {
        Some(Metric::Flow(_)) => capitalize(flow_label(request)),
        Some(Metric::Per10k(_)) => capitalize(&format!("{unit_plural} {status_plural} por dez mil habitantes")),
        Some(Metric::InhabitantsPer(_)) => format!("Habitantes por {unit_singular} {status_singular}"),
        Some(Metric::ActiveChange { .. }) => capitalize(&format!("variação de {unit_plural} {status_plural}")),
        Some(Metric::Share) => "Participação".into(),
        _ => capitalize(&format!("{unit_plural} {status_plural}")),
    }
}

fn metric_label(request: &opencnpj_query::ir::Request) -> String {
    let (unit_singular, unit_plural) = unit(request);
    let (status_singular, status_plural) = status_adjective(request);
    match request.op.metric() {
        Some(Metric::Flow(_)) => flow_label(request).into(),
        Some(Metric::Per10k(_)) => format!("{unit_plural} {status_plural} por dez mil habitantes"),
        Some(Metric::InhabitantsPer(_)) => format!("habitantes por {unit_singular} {status_singular}"),
        Some(Metric::ActiveChange { .. }) => format!("variação de {unit_plural} {status_plural}"),
        Some(Metric::Share) => "participação".into(),
        _ => format!("{unit_plural} {status_plural}"),
    }
}

fn trend_subject(request: &opencnpj_query::ir::Request, of_activity: &str) -> String {
    let (_, unit_plural) = unit(request);
    match request.op.metric() {
        Some(Metric::Flow(FlowMetric::Registrations)) => {
            format!("o número de {unit_plural}{of_activity} registrados com início de atividade")
        }
        Some(Metric::Flow(FlowMetric::Closures)) => format!("o número de baixas de {unit_plural}{of_activity}"),
        Some(Metric::Flow(FlowMetric::NetRegistrations)) => format!("o saldo entre registros e baixas de {unit_plural}{of_activity}"),
        Some(Metric::Flow(FlowMetric::MeiOptIns)) => format!("o número de adesões ao MEI{of_activity}"),
        Some(Metric::Flow(FlowMetric::MeiExits)) => format!("o número de saídas do MEI{of_activity}"),
        _ => format!("o número de {unit_plural}{of_activity}"),
    }
}

/// Short name of a flow metric (KPI labels, metric labels).
fn flow_label(request: &opencnpj_query::ir::Request) -> &'static str {
    match request.op.metric() {
        Some(Metric::Flow(FlowMetric::Registrations)) => "registros com início de atividade",
        Some(Metric::Flow(FlowMetric::Closures)) => "baixas",
        Some(Metric::Flow(FlowMetric::NetRegistrations)) => "saldo entre registros e baixas",
        Some(Metric::Flow(FlowMetric::MeiOptIns)) => "adesões ao MEI",
        Some(Metric::Flow(FlowMetric::MeiExits)) => "saídas do MEI",
        _ => "",
    }
}

/// What follows the number in a flow sentence: registrations are cadastral starts of
/// activity, never physical openings (SR-01); closures are baixas (SR-04).
fn flow_phrase(request: &opencnpj_query::ir::Request, value: i64) -> String {
    let (unit_singular, unit_plural) = unit(request);
    let one = value.unsigned_abs() == 1;
    let unit = if one { unit_singular } else { unit_plural.clone() };
    let feminine = unit_feminine(request);
    match request.op.metric() {
        Some(Metric::Flow(FlowMetric::Registrations)) => {
            let registered = match (feminine, one) {
                (true, true) => "registrada",
                (true, false) => "registradas",
                (false, true) => "registrado",
                (false, false) => "registrados",
            };
            format!("{unit} {registered} com início de atividade no período")
        }
        Some(Metric::Flow(FlowMetric::Closures)) => format!("{} de {unit_plural}", if one { "baixa" } else { "baixas" }),
        Some(Metric::Flow(FlowMetric::NetRegistrations)) => format!("como saldo entre registros e baixas de {unit_plural}"),
        Some(Metric::Flow(FlowMetric::MeiOptIns)) => (if one { "adesão ao MEI" } else { "adesões ao MEI" }).into(),
        Some(Metric::Flow(FlowMetric::MeiExits)) => (if one { "saída do MEI" } else { "saídas do MEI" }).into(),
        _ => unit,
    }
}

/// Preposition + article for a UF name ("no Paraná", "na Bahia", "em Mato Grosso").
fn uf_em(name: &str) -> String {
    let prep = match name {
        "Bahia" | "Paraíba" => "na",
        "Acre"
        | "Amapá"
        | "Amazonas"
        | "Ceará"
        | "Distrito Federal"
        | "Espírito Santo"
        | "Maranhão"
        | "Pará"
        | "Paraná"
        | "Piauí"
        | "Rio de Janeiro"
        | "Rio Grande do Norte"
        | "Rio Grande do Sul"
        | "Tocantins" => "no",
        _ => "em",
    };
    format!("{prep} {name}")
}

fn place_em(request: &opencnpj_query::ir::Request, labels: &Labels) -> String {
    let base = match &request.scope.place {
        PlaceFilter::Any => "no Brasil".into(),
        PlaceFilter::Region(id) => {
            labels.regions.get(&id.0).map_or_else(|| format!("na região {}", id.0), |name| format!("na região {name}"))
        }
        PlaceFilter::Near { center, .. } => labels
            .municipalities
            .get(&center.0)
            .map_or_else(|| "na região do município".into(), |label| format!("na região de {} ({})", label.name, label.uf)),
        PlaceFilter::Ufs(ids) if ids.len() == 1 => uf_em(&labels.ufs.get(&ids[0].0).cloned().unwrap_or_else(|| ids[0].0.to_string())),
        _ => format!("em {}", place_names(request, labels)),
    };
    format!("{base}{}", exclusion_suffix(request, labels))
}

fn place_count(request: &opencnpj_query::ir::Request) -> usize {
    match &request.scope.place {
        PlaceFilter::Municipalities(ids) => ids.len(),
        PlaceFilter::Ufs(ids) => ids.len(),
        _ => 1,
    }
}

/// ", exceto Cuiabá (MT) e farmácias" — exclusions are part of what was counted (SR-13).
fn exclusion_suffix(request: &opencnpj_query::ir::Request, labels: &Labels) -> String {
    let exclude = &request.scope.exclude;
    let mut names: Vec<String> = exclude
        .municipalities
        .iter()
        .map(|id| labels.municipalities.get(&id.0).map_or_else(|| id.0.to_string(), |label| format!("{} ({})", label.name, label.uf)))
        .collect();
    names.extend(exclude.ufs.iter().map(|uf| labels.ufs.get(&uf.0).cloned().unwrap_or_else(|| uf.0.to_string())));
    names.extend(exclude.sectors.iter().map(|id| labels.sectors.get(&id.0).map_or_else(|| id.0.to_string(), |label| label.plural.clone())));
    if names.is_empty() { String::new() } else { format!(", exceto {}", list(&names)) }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn place_label(request: &opencnpj_query::ir::Request, labels: &Labels) -> String {
    format!("{}{}", place_names(request, labels), exclusion_suffix(request, labels))
}

fn place_names(request: &opencnpj_query::ir::Request, labels: &Labels) -> String {
    match &request.scope.place {
        PlaceFilter::Municipalities(ids) => list(
            &ids.iter()
                .map(|id| {
                    labels
                        .municipalities
                        .get(&id.0)
                        .map(|label| format!("{} ({})", label.name, label.uf))
                        .unwrap_or_else(|| id.0.to_string())
                })
                .collect::<Vec<_>>(),
        ),
        PlaceFilter::Ufs(ids) => {
            list(&ids.iter().map(|uf| labels.ufs.get(&uf.0).cloned().unwrap_or_else(|| uf.0.to_string())).collect::<Vec<_>>())
        }
        PlaceFilter::Region(id) => {
            labels.regions.get(&id.0).map_or_else(|| format!("a região {}", id.0), |name| format!("a região {name}"))
        }
        PlaceFilter::Near { center, .. } => labels
            .municipalities
            .get(&center.0)
            .map(|label| format!("a região de {} ({})", label.name, label.uf))
            .unwrap_or_else(|| "a região".into()),
        PlaceFilter::Any => "o Brasil".into(),
    }
}

fn sector_label(request: &opencnpj_query::ir::Request, labels: &Labels) -> (String, String) {
    match &request.scope.activity {
        opencnpj_query::ir::ActivityFilter::Sectors { ids, .. } => {
            let singles = ids
                .iter()
                .map(|id| labels.sectors.get(&id.0).map(|label| label.singular.clone()).unwrap_or_else(|| id.0.to_string()))
                .collect::<Vec<_>>();
            let plurals = ids
                .iter()
                .map(|id| labels.sectors.get(&id.0).map(|label| label.plural.clone()).unwrap_or_else(|| id.0.to_string()))
                .collect::<Vec<_>>();
            (list(&singles), list(&plurals))
        }
        opencnpj_query::ir::ActivityFilter::Cnae { codes, .. } => {
            let values = codes.iter().map(cnae_label).collect::<Vec<_>>();
            let prefix = if values.len() == 1 { "CNAE" } else { "CNAEs" };
            (format!("{prefix} {}", list(&values)), format!("{prefix} {}", list(&values)))
        }
        opencnpj_query::ir::ActivityFilter::Any => ("todas as atividades".into(), "todas as atividades".into()),
    }
}

fn cnae_label(code: &CnaeCode) -> String {
    match code {
        CnaeCode::Section(letter) => char::from(*letter).to_string(),
        CnaeCode::Division(value) => format!("{value:02}"),
        CnaeCode::Group(value) => format!("{value:03}"),
        CnaeCode::Class(value) => format!("{value:05}"),
        CnaeCode::Subclass(value) => {
            let text = format!("{value:07}");
            format!("{}-{}/{}", &text[..4], &text[4..5], &text[5..])
        }
    }
}

/// The counted unit as read in the answer: a head-office or branch filter on establishments
/// is part of what was counted, so it is named ("filiais"), not hidden in the methodology.
fn unit(request: &opencnpj_query::ir::Request) -> (String, String) {
    match (request.unit, request.scope.head_office) {
        (opencnpj_query::ir::Unit::HeadOffices, _) | (_, Tri::Yes) => ("matriz".into(), "matrizes".into()),
        (_, Tri::No) => ("filial".into(), "filiais".into()),
        (_, Tri::Any) => ("estabelecimento".into(), "estabelecimentos".into()),
    }
}

fn unit_feminine(request: &opencnpj_query::ir::Request) -> bool {
    request.unit == opencnpj_query::ir::Unit::HeadOffices || request.scope.head_office != Tri::Any
}

fn status_adjective(request: &opencnpj_query::ir::Request) -> (String, String) {
    let set = request.scope.status.unwrap_or_else(|| request.op.default_status());
    if set == opencnpj_query::ir::StatusSet::ALL {
        let phrase = "em qualquer situação cadastral".to_string();
        return (phrase.clone(), phrase);
    }
    if set.iter().count() > 1 {
        // Several statuses: name them instead of picking the first one.
        let phrase = format!("nas situações {}", list(&set.iter().map(status_name).collect::<Vec<_>>()));
        return (phrase.clone(), phrase);
    }
    let status = set.iter().next();
    let feminine = unit_feminine(request);
    let (singular, plural) = match status {
        Some(Status::Closed) => {
            if feminine {
                ("baixada", "baixadas")
            } else {
                ("baixado", "baixados")
            }
        }
        Some(Status::Suspended) => {
            if feminine {
                ("suspensa", "suspensas")
            } else {
                ("suspenso", "suspensos")
            }
        }
        Some(Status::Unfit) => {
            if feminine {
                ("inapta", "inaptas")
            } else {
                ("inapto", "inaptos")
            }
        }
        Some(Status::Null) => {
            if feminine {
                ("nula", "nulas")
            } else {
                ("nulo", "nulos")
            }
        }
        _ => {
            if feminine {
                ("ativa", "ativas")
            } else {
                ("ativo", "ativos")
            }
        }
    };
    (singular.into(), plural.into())
}

fn pluralize<'a>(n: i64, singular: &'a str, plural: &'a str) -> &'a str {
    if n.unsigned_abs() == 1 { singular } else { plural }
}

fn methodology(request: &opencnpj_query::ir::Request, input: &RenderInput<'_>) -> Vec<String> {
    // Every filter that changes the count is listed, so the reader can see exactly what was
    // counted (filters missing here were the cause of answers that read as unfiltered).
    let scope = &request.scope;
    let mut items = Vec::new();
    let cnae_scope = match &scope.activity {
        opencnpj_query::ir::ActivityFilter::Sectors { scope, .. } | opencnpj_query::ir::ActivityFilter::Cnae { scope, .. } => Some(*scope),
        opencnpj_query::ir::ActivityFilter::Any => None,
    };
    items.push(match cnae_scope {
        Some(opencnpj_query::ir::CnaeScope::Any) => {
            format!("Atividade: {} (CNAE principal ou secundária)", sector_label(request, input.labels).1)
        }
        Some(opencnpj_query::ir::CnaeScope::Main) => format!("Atividade: {} (CNAE principal)", sector_label(request, input.labels).1),
        None => "Atividade: todas".into(),
    });
    let status = scope.status.unwrap_or_else(|| request.op.default_status());
    items.push(format!("Situação cadastral: {}", list(&status.iter().map(status_name).collect::<Vec<_>>())));
    items.push(format!(
        "Unidade: {}",
        match request.unit {
            opencnpj_query::ir::Unit::Establishments => "estabelecimento (CNPJ completo)",
            opencnpj_query::ir::Unit::HeadOffices => "matriz",
        }
    ));
    match scope.head_office {
        Tri::Yes => items.push("Somente matrizes".into()),
        Tri::No => items.push("Somente filiais".into()),
        Tri::Any => {}
    }
    match scope.mei {
        Tri::Yes => items.push("Somente MEI".into()),
        Tri::No => items.push("Excluindo MEI".into()),
        Tri::Any => {}
    }
    match scope.simples {
        Tri::Yes => items.push("Somente optantes pelo Simples".into()),
        Tri::No => items.push("Excluindo optantes pelo Simples".into()),
        Tri::Any => {}
    }
    if scope.porte != opencnpj_query::ir::PorteSet::ALL {
        items.push(format!("Porte: {}", list(&scope.porte.iter().map(porte_name).collect::<Vec<_>>())));
    }
    if !scope.natureza.is_empty() {
        let codes = scope
            .natureza
            .iter()
            .map(|code| match code {
                opencnpj_query::ir::NaturezaCode::Group(group) => format!("grupo {group}"),
                opencnpj_query::ir::NaturezaCode::Code(code) => format!("{code:04}"),
            })
            .collect::<Vec<_>>();
        items.push(format!("Natureza jurídica: {}", list(&codes)));
    }
    if let Some(range) = scope.opened {
        items.push(format!("Data de início de atividade: {}", day_range(range)));
    }
    if let Some(range) = scope.status_date {
        items.push(format!("Data da situação cadastral: {}", day_range(range)));
    }
    if let Some(range) = scope.capital {
        let reais = |cents: u64| format!("R$ {}", int_u(cents / 100));
        items.push(format!("Capital social: {}", bounds(range.from.map(reais), range.to.map(reais))));
    }
    if let Some(range) = scope.place_population {
        let people = |n: u32| format!("{} habitantes", int_u(u64::from(n)));
        items.push(format!("Municípios com população {}", bounds(range.from.map(people), range.to.map(people))));
    }
    if let PlaceFilter::Near { center, rule } = &scope.place {
        let name = input
            .labels
            .municipalities
            .get(&center.0)
            .map_or_else(|| "o município".into(), |label| format!("{} ({})", label.name, label.uf));
        items.push(match rule {
            opencnpj_query::ir::NearRule::Neighbours(depth) => {
                format!(
                    "Região: {name} e municípios vizinhos (até {} {} de vizinhança)",
                    depth,
                    if *depth == 1 { "nível" } else { "níveis" }
                )
            }
            opencnpj_query::ir::NearRule::WithinKm(km) => format!("Região: municípios com sede a até {km} km de {name}"),
        });
    }
    let excluded = exclusion_suffix(request, input.labels);
    if let Some(names) = excluded.strip_prefix(", exceto ") {
        items.push(format!("Excluídos: {names}"));
    }
    if let Some(name) = &scope.name_prefix {
        items.push(format!("Início de {}: {}", name.label_pt_br(), name.prefix));
    }
    match scope.contact {
        opencnpj_query::ir::ContactNeed::Phone => items.push("Com telefone publicado".into()),
        opencnpj_query::ir::ContactNeed::Email => items.push("Com e-mail publicado".into()),
        opencnpj_query::ir::ContactNeed::Both => items.push("Com telefone e e-mail publicados".into()),
        opencnpj_query::ir::ContactNeed::Any => {}
    }
    if let Some(reference) = population_ref(request, input.provenance) {
        items.push(format!("População: {}", population_label(reference)));
    }
    if let Some(Period::Days(days)) = request.time.period {
        items.push(format!("Período: {}", day_range(days)));
    }
    if matches!(request.op, Op::Rank { metric: Metric::ActiveChange { pct: true }, .. }) {
        items.push(format!("Base mínima: {}", min_base(request)));
    }
    items
}

fn caveats(request: &opencnpj_query::ir::Request, input: &RenderInput<'_>) -> Vec<Caveat> {
    let mut items = Vec::new();
    let metric = request.op.metric();
    if matches!(metric, Some(Metric::Flow(FlowMetric::Registrations | FlowMetric::NetRegistrations))) {
        items.push(Caveat {
            id: "C-REG",
            text: "Data de início de atividade é a data cadastral; não comprova a abertura física do estabelecimento.".into(),
        });
    }
    if matches!(metric, Some(Metric::Flow(FlowMetric::Closures | FlowMetric::NetRegistrations))) {
        items
            .push(Caveat {
                id: "C-CLOSE", text: "Baixas são contadas pela data da situação cadastral atual (a última alteração).".into()
            });
    }
    if (matches!(metric, Some(Metric::Per10k(_) | Metric::InhabitantsPer(_))) || request.scope.place_population.is_some())
        && let Some(reference) = population_ref(request, input.provenance)
    {
        items.push(Caveat {
            id: "C-DENS",
            text: format!(
                "Densidade por habitante não indica, sozinha, oportunidade de mercado. População: {}.",
                population_label(reference)
            ),
        });
    }
    if request.scope.capital.is_some() || matches!(request.op, Op::List { order: opencnpj_query::ir::ListOrder::CapitalDesc, .. }) {
        items.push(Caveat { id: "C-CAP", text: "Capital social declarado não equivale a faturamento.".into() });
    }
    if input.unit_inferred {
        items.push(Caveat {
            id: "C-UNIT",
            text: "Contei estabelecimentos (cada CNPJ completo). Para contar só sedes, peça ‘só matrizes’.".into(),
        });
    }
    if let opencnpj_query::ir::ActivityFilter::Sectors { cnaes: codes, scope, .. }
    | opencnpj_query::ir::ActivityFilter::Cnae { codes, scope } = &request.scope.activity
    {
        let codes = codes.iter().map(cnae_label).collect::<Vec<_>>();
        let which = match scope {
            opencnpj_query::ir::CnaeScope::Main => "Atividade principal",
            opencnpj_query::ir::CnaeScope::Any => "Atividade principal ou secundária",
        };
        items.push(Caveat { id: "C-SECT", text: format!("{which}: CNAE {}.", list(&codes)) });
    }
    if let Some(Period::Days(days)) = request.time.period
        && days.to == Some(input.provenance.anchor)
    {
        items.push(Caveat {
            id: "C-PARTIAL",
            text: format!("O período vai até {}, data mais recente da base.", date(input.provenance.anchor)),
        });
    }
    if let Op::Rank { metric: Metric::ActiveChange { pct: true }, .. } = &request.op {
        items.push(Caveat {
            id: "C-MINBASE",
            text: format!("Locais com menos de {} estabelecimentos não entram na variação percentual.", min_base(request)),
        });
    }
    if request.scope.mei != Tri::Any {
        items.push(Caveat {
            id: "C-MEI",
            text: "MEI é uma opção tributária registrada na tabela do Simples; o porte cadastral costuma ser ‘micro’.".into(),
        });
    }
    items
}

fn suggestions(request: &opencnpj_query::ir::Request) -> Vec<String> {
    match request.op {
        Op::Count { .. } => vec!["Ver por porte".into(), "Listar os estabelecimentos".into(), "Só matrizes".into()],
        Op::Compare { .. } => vec!["Incluir outro local".into(), "Ver a tabela completa".into()],
        Op::Rank { .. } => vec!["Ver a lista completa".into(), "Filtrar por setor".into()],
        Op::Breakdown { .. } => vec!["Ver por município".into(), "Comparar dois grupos".into()],
        Op::Trend { .. } => vec!["Ver a tabela completa".into(), "Comparar períodos".into()],
        Op::List { .. } => vec!["Só matrizes".into(), "Com telefone".into(), "Mais recentes".into()],
        Op::Lookup { .. } => vec!["Ver os estabelecimentos".into(), "Ver a metodologia".into()],
        Op::Meta { .. } => vec!["Ver fontes".into(), "Ver a metodologia".into()],
    }
}

fn min_base(request: &opencnpj_query::ir::Request) -> u32 {
    match request.op {
        Op::Rank { min_base, .. } => min_base,
        _ => 0,
    }
}

fn period_date(request: &opencnpj_query::ir::Request, start: bool) -> Option<String> {
    match request.time.period {
        Some(Period::Days(days)) => Some(if start { days.from } else { days.to }?.to_string()),
        _ => None,
    }
}

fn period_month(request: &opencnpj_query::ir::Request, start: bool) -> Option<String> {
    match request.time.period {
        Some(Period::Competences { from, to }) => Some(month_year(if start { from } else { to })),
        Some(Period::Days(days)) => {
            let day = if start { days.from } else { days.to }?;
            let (year, month, _) = day.to_ymd()?;
            Competence::from_year_month(year, month).map(month_year)
        }
        None => None,
    }
}

fn date(day: Day) -> String {
    match day.to_ymd() {
        Some((year, month, day)) => format!("{day:02}/{month:02}/{year:04}"),
        None => day.to_string(),
    }
}

fn month_year(competence: Competence) -> String {
    const MONTHS: [&str; 12] =
        ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"];
    format!("{} de {}", MONTHS[usize::from(competence.month() - 1)], competence.year())
}

fn month_label(key: u64) -> String {
    let year = key / 12;
    let month = key % 12 + 1;
    Competence::from_year_month(u16::try_from(year).unwrap_or(2000), u8::try_from(month).unwrap_or(1))
        .map_or_else(|| key.to_string(), month_year)
}

fn status_name(status: Status) -> String {
    match status {
        Status::Null => "nula",
        Status::Active => "ativa",
        Status::Suspended => "suspensa",
        Status::Unfit => "inapta",
        Status::Closed => "baixada",
    }
    .into()
}

fn population_label(reference: PopulationRef) -> String {
    match reference {
        PopulationRef::IbgeCensus(year) => format!("IBGE Censo {year}"),
        PopulationRef::IbgeEstimate(year) => format!("IBGE estimativa {year}"),
    }
}

/// The population the numbers were divided by: the one named in the metric, else (for
/// population filters) the request's reference, else the provenance default. `None` when
/// the answer does not use population at all.
fn population_ref(request: &opencnpj_query::ir::Request, provenance: &Provenance) -> Option<PopulationRef> {
    match request.op.metric() {
        Some(Metric::Per10k(reference) | Metric::InhabitantsPer(reference)) => Some(reference),
        _ if request.scope.place_population.is_some() => request.scope.population_ref.or(provenance.population),
        _ => None,
    }
}

fn porte_name(porte: opencnpj_query::ir::Porte) -> String {
    match porte {
        opencnpj_query::ir::Porte::Unreported => "não informado",
        opencnpj_query::ir::Porte::Micro => "microempresa",
        opencnpj_query::ir::Porte::Small => "empresa de pequeno porte",
        opencnpj_query::ir::Porte::Other => "demais",
    }
    .into()
}

fn day_range(range: opencnpj_query::ir::DayRange) -> String {
    bounds(range.from.map(date), range.to.map(date))
}

fn bounds(from: Option<String>, to: Option<String>) -> String {
    match (from, to) {
        (Some(from), Some(to)) => format!("de {from} a {to}"),
        (Some(from), None) => format!("a partir de {from}"),
        (None, Some(to)) => format!("até {to}"),
        (None, None) => "sem limite".into(),
    }
}

/// Masks 11-digit runs (owner CPFs embedded in MEI/EI names) as `***.456.789-**`, keeping
/// the six digits RFB itself publishes for partners. Longer digit runs (CNPJs) are kept.
/// Mask an owner CPF embedded in an MEI/EI name (SR-14, D-05).
pub fn mask_cpf(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let run: String = chars[start..i].iter().collect();
            if run.len() == 11 {
                out.push_str(&format!("***.{}.{}-**", &run[3..6], &run[6..9]));
            } else {
                out.push_str(&run);
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

fn list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => format!("{a} e {b}"),
        _ => format!("{} e {}", items[..items.len() - 1].join(", "), items[items.len() - 1]),
    }
}

fn int(value: i64) -> String {
    let grouped = int_u(value.unsigned_abs());
    if value < 0 { format!("-{grouped}") } else { grouped }
}

/// pt-BR thousands grouping for unsigned values (no clamping to `i64::MAX`).
fn int_u(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push('.');
        }
        grouped.push(ch);
    }
    grouped
}

/// One decimal, pt-BR separators ("12.345,6"); non-finite values are never printed.
fn decimal1(value: f64) -> String {
    if !value.is_finite() {
        return NO_DATA.into();
    }
    let tenths = (value.abs() * 10.0).round();
    if tenths > u64::MAX as f64 {
        return NO_DATA.into();
    }
    let tenths = tenths as u64;
    let sign = if value < 0.0 && tenths != 0 { "-" } else { "" };
    format!("{sign}{},{}", int_u(tenths / 10), tenths % 10)
}

fn pct1(value: f64) -> String {
    format!("{}%", decimal1(value))
}

#[derive(Debug, Clone, serde::Deserialize)]
struct TemplateEntry {
    variants: Vec<String>,
}

pub struct Templates {
    entries: BTreeMap<String, TemplateEntry>,
}

impl Templates {
    pub fn embedded() -> Self {
        Self::parse(include_str!("../templates/pt-BR/answers.toml")).expect("embedded templates")
    }

    pub fn parse(text: &str) -> Result<Self, toml::de::Error> {
        Ok(Self { entries: toml::from_str(text)? })
    }

    pub fn render(&self, key: &str, seed: u64, values: &BTreeMap<&'static str, String>) -> Result<String, RenderError> {
        let entry = self.entries.get(key).ok_or_else(|| RenderError::UnknownTemplate(key.into()))?;
        if entry.variants.is_empty() {
            return Err(RenderError::UnknownTemplate(key.into()));
        }
        let template = &entry.variants[usize::try_from(seed % entry.variants.len() as u64).expect("variant index")];
        let mut out = String::with_capacity(template.len());
        let mut rest = template.as_str();
        while let Some(open) = rest.find('{') {
            out.push_str(&rest[..open]);
            let after = &rest[open + 1..];
            let close = after.find('}').ok_or_else(|| RenderError::MissingPlaceholder(after.into()))?;
            let name = &after[..close];
            out.push_str(values.get(name).ok_or_else(|| RenderError::MissingPlaceholder(name.into()))?);
            rest = &after[close + 1..];
        }
        out.push_str(rest);
        Ok(out)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.keys().map(String::as_str).collect()
    }

    pub fn variants(&self, key: &str) -> Option<&[String]> {
        self.entries.get(key).map(|entry| entry.variants.as_slice())
    }
}

pub fn supported_keys() -> Vec<String> {
    let mut keys = Vec::new();
    for metric in ["stock", "flow", "active_change"] {
        for turn in ["first", "followup"] {
            keys.push(format!("count.{metric}.scalar.{turn}"));
        }
    }
    for metric in ["per10k", "inhabitants_per", "share"] {
        for turn in ["first", "followup"] {
            keys.push(format!("count.{metric}.ratio.{turn}"));
        }
    }
    for op in ["compare", "rank", "breakdown"] {
        for metric in ["stock", "flow", "active_change", "per10k", "inhabitants_per", "share"] {
            for turn in ["first", "followup"] {
                keys.push(format!("{op}.{metric}.rows.{turn}"));
            }
        }
    }
    for turn in ["first", "followup"] {
        keys.push(format!("trend.flow.rows.{turn}"));
        keys.push(format!("list.stock.records.{turn}"));
    }
    for turn in ["first", "followup"] {
        keys.push(format!("empty.none.rows.{turn}"));
    }
    keys.push("lookup.none.company.first".into());
    keys.push("meta.none.text.first".into());
    keys.push("clarify.none.clarify.first".into());
    keys.push("reject.none.reject.first".into());
    keys
}

pub fn schema(_key: &str) -> Vec<&'static str> {
    vec![
        "competence:month_year",
        "place",
        "place_em",
        "place_em_cap",
        "sector:label",
        "sector:label_plural",
        "unit:plural(value)",
        "status:adj(value)",
        "unit",
        "status",
        "metric_label",
        "flow",
        "from:date",
        "to:date",
        "from:month_year",
        "to:month_year",
        "value:int",
        "value:dec1",
        "population:int",
        "population_source",
        "unit:singular",
        "status:singular",
        "have",
        "had",
        "dir_most",
        "dir_more",
        "of_activity",
        "in_activity",
        "trend_subject",
        "a.place",
        "a.value:int",
        "a.value:dec1",
        "a.share:pct1",
        "b.place",
        "b.value:int",
        "b.value:dec1",
        "b.share:pct1",
        "diff",
        "n:int",
        "top",
        "first:int",
        "last:int",
        "change_pct:pct1",
        "total:int",
        "page:int",
        "cnpj",
        "name",
        "snapshot_sha256",
        "catalogue",
        "question",
        "reason",
    ]
}

pub fn lint_templates(templates: &Templates) -> Vec<String> {
    let mut errors = Vec::new();
    let supported = supported_keys();
    for key in &supported {
        if templates.variants(key).is_none() {
            errors.push(format!("missing template {key}"));
        }
    }
    for key in templates.keys() {
        if !supported.iter().any(|supported| supported == key) {
            errors.push(format!("unreachable template {key}"));
        }
    }
    let allowed = schema("");
    for key in supported {
        let Some(variants) = templates.variants(&key) else { continue };
        if variants.is_empty() {
            errors.push(format!("empty template {key}"));
        }
        for (index, variant) in variants.iter().enumerate() {
            if let Err(error) = lint_variant(variant, &allowed) {
                errors.push(format!("{key}[{index}]: {error}"));
            }
        }
    }
    errors
}

fn lint_variant(template: &str, allowed: &[&str]) -> Result<(), String> {
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        if rest[..open].chars().any(|ch| ch.is_ascii_digit()) {
            return Err("digit outside a placeholder".into());
        }
        let after = &rest[open + 1..];
        let close = after.find('}').ok_or("unclosed placeholder")?;
        let name = &after[..close];
        if !allowed.contains(&name) {
            return Err(format!("unknown placeholder {name}"));
        }
        rest = &after[close + 1..];
    }
    if rest.chars().any(|ch| ch.is_ascii_digit()) {
        return Err("digit outside a placeholder".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use opencnpj_query::{
        ir::*,
        validate::{Capabilities, validate},
    };

    const INPUTS: &str = include_str!("../../opencnpj-query/tests/fixtures/ir-v1-requests.jsonl");

    fn day(year: u16, month: u8, day: u8) -> Day {
        Day::from_ymd(year, month, day).unwrap()
    }

    fn competence(year: u16, month: u8) -> Competence {
        Competence::from_year_month(year, month).unwrap()
    }

    fn canonical(request: Request) -> ValidRequest {
        let municipality_ufs =
            BTreeMap::from([(MunicipalityId(5067), UfId(12)), (MunicipalityId(5108), UfId(12)), (MunicipalityId(5527), UfId(12))]);
        let sector_cnaes = BTreeMap::from([
            (SectorId(1), SmallSet::new([CnaeCode::Subclass(1091102), CnaeCode::Subclass(4721101), CnaeCode::Subclass(4721102)])),
            (SectorId(2), SmallSet::new([CnaeCode::Subclass(4771701), CnaeCode::Subclass(4771702), CnaeCode::Subclass(4771703)])),
        ]);
        let context = CanonicalContext {
            first_day: Day(1),
            last_day: day(2026, 10, 11),
            municipality_ufs: &municipality_ufs,
            sector_cnaes: &sector_cnaes,
        };
        let canonical = request.canonicalize(&context).unwrap().0;
        let mut competences = Vec::new();
        let (mut year, mut month) = (2024, 11);
        for _ in 0..24 {
            competences.push(competence(year, month));
            month += 1;
            if month == 13 {
                month = 1;
                year += 1;
            }
        }
        let capabilities = Capabilities {
            current: competence(2026, 10),
            competences,
            flow_days: day(2025, 1, 1)..=day(2026, 10, 11),
            populations: vec![PopulationRef::IbgeCensus(2022), PopulationRef::IbgeEstimate(2026)],
            catalogue_versions: CatalogueVersions { municipalities: 202610, sectors: 1 },
            sector_count: 2,
            geography: true,
            list_limit: 100,
        };
        validate(canonical, &capabilities).unwrap()
    }

    fn labels() -> Labels {
        Labels {
            municipalities: BTreeMap::from([
                (5067, MunicipalityLabel { name: "Sinop".into(), uf: "MT".into() }),
                (5108, MunicipalityLabel { name: "Cuiabá".into(), uf: "MT".into() }),
                (5527, MunicipalityLabel { name: "Sorriso".into(), uf: "MT".into() }),
            ]),
            sectors: BTreeMap::from([
                (1, SectorLabel { singular: "padaria".into(), plural: "padarias".into(), feminine: true }),
                (2, SectorLabel { singular: "farmácia".into(), plural: "farmácias".into(), feminine: true }),
            ]),
            ufs: BTreeMap::from([(12, "Mato Grosso".into()), (25, "São Paulo".into()), (17, "Paraná".into())]),
            regions: BTreeMap::from([(3, "Norte".into()), (4, "Nordeste".into()), (5, "Centro-Oeste".into())]),
        }
    }

    fn provenance() -> Provenance {
        Provenance {
            competence: competence(2026, 10),
            anchor: day(2026, 10, 11),
            snapshot_sha256: "76f36078c1e5b727af8416262309d1285e39f054514218a75a46206d5786037b".into(),
            municipality_catalogue: 202610,
            sector_catalogue: 1,
            population: Some(PopulationRef::IbgeEstimate(2026)),
        }
    }

    fn key_for_dim(dim: Option<Dim>, index: u16) -> u64 {
        match dim {
            Some(Dim::Municipality) => [5067, 5108, 5527].get(usize::from(index)).copied().unwrap_or(0) as u64,
            Some(Dim::Uf) => [12, 17, 25].get(usize::from(index)).copied().unwrap_or(0) as u64,
            Some(Dim::Sector) => u64::from(index + 1),
            Some(Dim::Cnae(_)) => u64::from(b'G' + index as u8),
            Some(Dim::Region(_)) => u64::from(index + 3),
            Some(Dim::Porte) => u64::from(index % 4),
            Some(Dim::Status) => u64::from(index + 1),
            Some(Dim::HeadOffice) => u64::from(index % 2),
            Some(Dim::Mei) => u64::from(index % 2),
            Some(Dim::Year) => 2025 + u64::from(index),
            Some(Dim::Month) => 24_300 + u64::from(index),
            None => 24_300 + u64::from(index),
        }
    }

    fn member_key(member: &Member) -> u64 {
        match member {
            Member::Municipality(id) => u64::from(id.0),
            Member::Uf(id) => u64::from(id.0),
            Member::Region(id) => u64::from(id.0),
            Member::Sector(id) => u64::from(id.0),
        }
    }

    fn payload(request: &Request) -> Value {
        match &request.op {
            Op::Meta { .. } => Value::Scalar { value: 0 },
            Op::List { .. } | Op::Lookup { .. } => Value::Scalar { value: 0 },
            Op::Count { metric } => match metric {
                Metric::Per10k(_) => Value::Ratio { num: 187, den: 100_000 },
                Metric::InhabitantsPer(_) => Value::Ratio { num: 100_000, den: 187 },
                Metric::Share => Value::Ratio { num: 187, den: 1000 },
                Metric::Flow(_) => Value::Scalar { value: 42 },
                Metric::ActiveChange { .. } => Value::Scalar { value: 12 },
                Metric::Stock => Value::Scalar { value: 187 },
            },
            Op::Compare { members, metric, .. } => {
                Value::Rows(members.iter().enumerate().map(|(index, member)| row(*metric, member_key(member), index as i64)).collect())
            }
            Op::Rank { dim, metric, .. } | Op::Breakdown { dim, metric, .. } => {
                Value::Rows((0..3).map(|index| row(*metric, key_for_dim(Some(*dim), index), index as i64)).collect())
            }
            Op::Trend { grain, .. } => Value::Rows(
                (0..3)
                    .map(|index| {
                        let key = match grain {
                            Grain::Month => 24_300 + index,
                            Grain::Year => 2025 + index,
                        };
                        row(Metric::Flow(FlowMetric::Registrations), key, index as i64)
                    })
                    .collect(),
            ),
        }
    }

    fn row(metric: Metric, key: u64, index: i64) -> Row {
        let count = 187 - index * 40;
        // Same layout as the executor (`execute::ratio_rows`): Per10k = count over population,
        // InhabitantsPer = population over count.
        match metric {
            Metric::Per10k(_) => Row { key, value: count, base: 100_000 },
            Metric::InhabitantsPer(_) => Row { key, value: 100_000, base: count as u64 },
            Metric::Share => Row { key, value: count, base: 1000 },
            Metric::ActiveChange { pct: true } => Row { key, value: count, base: 100 },
            _ => Row { key, value: count, base: 0 },
        }
    }

    fn list_payload() -> (u64, u16, Vec<Vec<String>>) {
        (20, 1, vec![vec!["11111111000101".into(), "Padaria".into()]])
    }

    fn company_payload() -> Vec<(String, String)> {
        vec![("cnpj".into(), "11111111000101".into()), ("name".into(), "Padaria Ltda".into()), ("status".into(), "Ativa".into())]
    }

    fn render_case(request: &ValidRequest) -> Answer {
        let labels = labels();
        let provenance = provenance();
        let value = payload(request.as_request());
        let (total, page, rows) = list_payload();
        let records = Payload::Records { total, page, rows };
        let company = Payload::Company { fields: company_payload() };
        let meta = Payload::Meta { text: "Metodologia".into() };
        let payload = match &request.op {
            Op::List { .. } => records,
            Op::Lookup { .. } => company,
            Op::Meta { .. } => meta,
            _ => Payload::Value(&value),
        };
        render(RenderInput {
            request,
            payload,
            provenance: &provenance,
            labels: &labels,
            followup: false,
            notice: None,
            suggestions: Vec::new(),
            unit_inferred: false,
            timing_us: BTreeMap::new(),
        })
        .unwrap()
    }

    #[test]
    fn formatters_follow_pt_br() {
        assert_eq!(int(1_234_567), "1.234.567");
        assert_eq!(int(-42), "-42");
        assert_eq!(decimal1(12.34), "12,3");
        assert_eq!(pct1(12.34), "12,3%");
        assert_eq!(month_year(competence(2026, 10)), "outubro de 2026");
        assert_eq!(date(day(2026, 10, 11)), "11/10/2026");
        assert_eq!(pluralize(1, "estabelecimento", "estabelecimentos"), "estabelecimento");
        assert_eq!(pluralize(2, "estabelecimento", "estabelecimentos"), "estabelecimentos");
        assert_eq!(status_adjective(&valid_request(Unit::HeadOffices, Status::Active)).0, "ativa");
        assert_eq!(status_adjective(&valid_request(Unit::Establishments, Status::Active)).1, "ativos");
    }

    fn status_serde(status: Status) -> &'static str {
        match status {
            Status::Null => "null",
            Status::Active => "active",
            Status::Suspended => "suspended",
            Status::Unfit => "unfit",
            Status::Closed => "closed",
        }
    }

    fn valid_request(unit: Unit, status: Status) -> Request {
        let mut request = serde_json::from_value::<Request>(serde_json::json!({
            "op": {"kind": "count", "metric": "stock"},
            "unit": if unit == Unit::HeadOffices { "head_offices" } else { "establishments" },
            "scope": {"status": [status_serde(status)]},
            "time": {"competence": 321, "period": null, "anchor": 46305},
            "output": {},
            "versions": {"municipalities": 202610, "sectors": 1}
        }))
        .unwrap();
        request.unit = unit;
        request
    }

    #[test]
    fn formatters_handle_edges() {
        assert_eq!(decimal1(12_345.64), "12.345,6");
        assert_eq!(decimal1(-0.01), "0,0");
        assert_eq!(decimal1(-1234.56), "-1.234,6");
        assert_eq!(decimal1(f64::NAN), NO_DATA);
        assert_eq!(decimal1(f64::INFINITY), NO_DATA);
        assert_eq!(int_u(u64::MAX), "18.446.744.073.709.551.615");
        assert_eq!(pluralize(-1, "estabelecimento", "estabelecimentos"), "estabelecimento");
        assert_eq!(uf_em("Paraná"), "no Paraná");
        assert_eq!(uf_em("Bahia"), "na Bahia");
        assert_eq!(uf_em("Mato Grosso"), "em Mato Grosso");
    }

    #[test]
    fn cpf_embedded_in_names_is_masked() {
        assert_eq!(mask_cpf("MARIA DA SILVA 12345678901"), "MARIA DA SILVA ***.456.789-**");
        assert_eq!(mask_cpf("33000167000101"), "33000167000101");
        assert_eq!(mask_cpf("PADARIA 2025 LTDA"), "PADARIA 2025 LTDA");
    }

    #[test]
    fn missing_values_fail_instead_of_rendering_blanks() {
        let templates = Templates::parse("[\"k\"]\nvariants = [\"{place} {value:int}\"]").unwrap();
        let values = BTreeMap::from([("place", "Sinop (MT)".to_string())]);
        assert_eq!(templates.render("k", 0, &values), Err(RenderError::MissingPlaceholder("value:int".into())));
    }

    fn case(name: &str) -> ValidRequest {
        golden().into_iter().find(|(case, _)| case == name).unwrap_or_else(|| panic!("no golden case {name}")).1
    }

    fn texts(answer: &Answer) -> String {
        serde_json::to_string(&answer.blocks).unwrap()
    }

    // Expected strings are written by hand (not derived from the renderer's own helpers).
    #[test]
    fn population_source_comes_from_the_metric() {
        let answer = render_case(&case("density_census_mt"));
        let all = texts(&answer);
        assert!(all.contains("IBGE Censo 2022"), "{all}");
        assert!(!all.contains("estimativa 2025"), "{all}");
        let closures = texts(&render_case(&case("closed_stock_filter")));
        assert!(!closures.contains("População"), "{closures}");
    }

    #[test]
    fn rank_direction_is_rendered() {
        let answer = render_case(&case("rank_change_percent"));
        let Block::Text { text } = &answer.blocks[0] else { panic!() };
        assert!(text.contains("menores variações"), "{text}");
    }

    #[test]
    fn filters_are_visible() {
        let closed = render_case(&case("closed_stock_filter"));
        let all = texts(&closed);
        assert!(all.contains("Estabelecimentos baixados"), "{all}");
        assert!(all.contains("Data da situação cadastral: de 01/01/2025 a 31/12/2025"), "{all}");
        let branches = texts(&render_case(&case("so_filiais")));
        assert!(branches.contains("Somente filiais"), "{branches}");
        let excluded = render_case(&case("exclusions"));
        let Block::Text { text } = &excluded.blocks[0] else { panic!() };
        assert!(text.contains("exceto"), "{text}");
        assert!(texts(&excluded).contains("Excluídos:"));
    }

    #[test]
    fn missing_denominators_never_become_numbers() {
        let request = case("density_census_mt");
        let value = Value::Ratio { num: 187, den: 0 };
        let labels = labels();
        let provenance = provenance();
        let result = render(RenderInput {
            request: &request,
            payload: Payload::Value(&value),
            provenance: &provenance,
            labels: &labels,
            followup: false,
            notice: None,
            suggestions: Vec::new(),
            unit_inferred: false,
            timing_us: BTreeMap::new(),
        });
        assert_eq!(result, Err(RenderError::InvalidPayload("ratio without a denominator")));
        let rank = Row { key: 5067, value: 187, base: 0 };
        let per10k = Request {
            op: Op::Rank {
                dim: Dim::Municipality,
                metric: Metric::Per10k(PopulationRef::IbgeCensus(2022)),
                dir: Dir::Desc,
                k: 3,
                min_base: 0,
            },
            ..request.as_request().clone()
        };
        assert_eq!(row_value(&per10k, rank), NO_DATA);
    }

    #[test]
    fn registrations_are_not_called_openings() {
        for (name, request) in golden() {
            if !matches!(request.as_request().op.metric(), Some(Metric::Flow(FlowMetric::Registrations))) {
                continue;
            }
            let all = texts(&render_case(&request)).to_lowercase();
            assert!(!all.contains("abri") && !all.contains("abertura das portas"), "{name}: {all}");
        }
    }

    #[test]
    fn template_lints_pass() {
        let templates = Templates::embedded();
        assert!(lint_templates(&templates).is_empty(), "{:?}", lint_templates(&templates));
    }

    fn golden() -> Vec<(String, ValidRequest)> {
        INPUTS
            .lines()
            .map(|line| {
                #[derive(serde::Deserialize)]
                struct Case {
                    case: String,
                    request: Request,
                }
                let case: Case = serde_json::from_str(line).unwrap();
                (case.case, canonical(case.request))
            })
            .collect()
    }

    #[test]
    fn golden_snapshots() {
        let cases = golden();
        let mut actual = BTreeMap::new();
        for (name, request) in cases {
            actual.insert(name, serde_json::to_value(render_case(&request)).unwrap());
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/answers.json");
        if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
            std::fs::write(&path, serde_json::to_string_pretty(&actual).unwrap()).unwrap();
        }
        let expected: BTreeMap<String, serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn no_invented_numbers() {
        for (name, request) in golden() {
            let answer = render_case(&request);
            let mut allowed = BTreeSet::new();
            add_source(&mut allowed, &serde_json::to_string(&answer.query).unwrap());
            for label in labels().municipalities.values() {
                add_source(&mut allowed, &label.name);
                add_source(&mut allowed, &label.uf);
            }
            for label in labels().sectors.values() {
                add_source(&mut allowed, &label.singular);
                add_source(&mut allowed, &label.plural);
            }
            for uf in labels().ufs.values() {
                add_source(&mut allowed, uf);
            }
            if let opencnpj_query::ir::ActivityFilter::Sectors { cnaes, .. }
            | opencnpj_query::ir::ActivityFilter::Cnae { codes: cnaes, .. } = &request.as_request().scope.activity
            {
                for code in cnaes.iter() {
                    add_source(&mut allowed, &cnae_label(code));
                }
            }
            let provenance = provenance();
            add_source(&mut allowed, &provenance.competence.to_string());
            add_source(&mut allowed, &month_year(provenance.competence));
            add_source(&mut allowed, &month_year(request.as_request().time.competence));
            add_source(&mut allowed, &provenance.anchor.to_string());
            add_source(&mut allowed, &date(provenance.anchor));
            add_source(&mut allowed, &provenance.snapshot_sha256);
            add_source(&mut allowed, &provenance.municipality_catalogue.to_string());
            add_source(&mut allowed, &provenance.sector_catalogue.to_string());
            if let Some(population) = provenance.population {
                add_source(&mut allowed, &population_label(population));
            }
            if let Some(Period::Days(days)) = request.as_request().time.period
                && let (Some(from), Some(to)) = (days.from, days.to)
            {
                add_source(&mut allowed, &date(from));
                add_source(&mut allowed, &date(to));
                add_source(
                    &mut allowed,
                    &month_year(Competence::from_year_month(from.to_ymd().unwrap().0, from.to_ymd().unwrap().1).unwrap()),
                );
                add_source(&mut allowed, &month_year(Competence::from_year_month(to.to_ymd().unwrap().0, to.to_ymd().unwrap().1).unwrap()));
            }
            if let Some(Period::Competences { from, to }) = request.as_request().time.period {
                add_source(&mut allowed, &month_year(from));
                add_source(&mut allowed, &month_year(to));
            }
            match &request.as_request().op {
                Op::List { .. } => {
                    let (total, page, rows) = list_payload();
                    add_source(&mut allowed, &total.to_string());
                    add_source(&mut allowed, &page.to_string());
                    add_source(&mut allowed, &rows.len().to_string());
                    for row in rows {
                        for cell in row {
                            add_source(&mut allowed, &cell);
                        }
                    }
                }
                Op::Lookup { .. } => {
                    for (_, value) in company_payload() {
                        add_source(&mut allowed, &value);
                    }
                }
                _ => {}
            }
            let scope = &request.as_request().scope;
            for range in [scope.opened, scope.status_date].into_iter().flatten() {
                for day in [range.from, range.to].into_iter().flatten() {
                    add_source(&mut allowed, &date(day));
                }
            }
            // The IR keeps capital in cents; answers show reais.
            if let Some(capital) = request.as_request().scope.capital {
                for cents in [capital.from, capital.to].into_iter().flatten() {
                    add_source(&mut allowed, &(cents / 100).to_string());
                }
            }
            let value = payload(request.as_request());
            add_value(&mut allowed, request.as_request(), &value);
            let json = serde_json::to_value(&answer).unwrap();
            walk_strings(&json, &allowed, &name);
        }
    }

    fn add_source(allowed: &mut BTreeSet<String>, text: &str) {
        for token in numeric_tokens(text) {
            allowed.insert(token);
        }
    }

    /// The displayed value of a row, computed here from the formulas in 08-RESPONSES.md
    /// rather than with the renderer's own helpers, so the test is not circular.
    fn expected_value(request: &Request, value: i64, base: u64) -> Option<f64> {
        let (v, b) = (value as f64, base as f64);
        match request.op.metric() {
            Some(Metric::Per10k(_)) => (base > 0).then(|| v * 10_000.0 / b),
            Some(Metric::InhabitantsPer(_)) => (base > 0).then(|| v / b),
            Some(Metric::Share | Metric::ActiveChange { pct: true }) => (base > 0).then(|| v * 100.0 / b),
            _ => Some(v),
        }
    }

    fn add_number(allowed: &mut BTreeSet<String>, value: f64) {
        // One decimal, both signs, comma or point: any of these spellings is the same number.
        let text = format!("{:.1}", value.abs());
        add_source(allowed, &text);
        add_source(allowed, &format!("{}", value.abs().round() as i64));
    }

    fn add_value(allowed: &mut BTreeSet<String>, request: &Request, value: &Value) {
        match value {
            Value::Scalar { value } => {
                add_source(allowed, &value.to_string());
            }
            Value::Ratio { num, den } => {
                add_source(allowed, &num.to_string());
                add_source(allowed, &den.to_string());
                if let Some(display) = expected_value(request, i64::try_from(*num).unwrap(), *den) {
                    add_number(allowed, display);
                }
            }
            Value::Rows(rows) => {
                let mut displayed = Vec::new();
                for row in rows {
                    add_source(allowed, &row.key.to_string());
                    add_source(allowed, &row.value.to_string());
                    add_source(allowed, &row.base.to_string());
                    let shown = expected_value(request, row.value, row.base);
                    if let Some(shown) = shown {
                        add_number(allowed, shown);
                    }
                    displayed.push(shown);
                    if matches!(request.op, Op::Trend { .. }) {
                        add_source(allowed, &(row.key / 12).to_string());
                    }
                }
                add_source(allowed, &rows.len().to_string());
                if let [Some(a), Some(b), ..] = displayed[..] {
                    add_number(allowed, a - b);
                    if b != 0.0 {
                        add_number(allowed, (a - b) / b.abs() * 100.0);
                    }
                }
                if let (Some(first), Some(last)) = (rows.first(), rows.last())
                    && first.value != 0
                {
                    add_number(allowed, (last.value - first.value) as f64 * 100.0 / first.value as f64);
                }
            }
        }
    }

    /// Whole numbers as written in pt-BR ("12.345,6" → "12345.6", "1.234" → "1234"), so a
    /// grouped or decimal number is checked as one value, not as loose digit runs.
    fn numeric_tokens(text: &str) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        let mut out = Vec::new();
        let mut current = String::new();
        for (i, &ch) in chars.iter().enumerate() {
            let joins = (ch == '.' || ch == ',') && !current.is_empty() && chars.get(i + 1).is_some_and(char::is_ascii_digit);
            if ch.is_ascii_digit() || joins {
                current.push(ch);
            } else if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() {
            out.push(current);
        }
        out.into_iter()
            .map(|token| {
                if token.contains(',') {
                    token.replace('.', "").replace(',', ".")
                } else if token.split('.').skip(1).all(|group| group.len() == 3) {
                    token.replace('.', "")
                } else {
                    token
                }
            })
            .collect()
    }

    fn walk_strings(value: &serde_json::Value, allowed: &BTreeSet<String>, case: &str) {
        match value {
            serde_json::Value::String(text) => {
                for token in numeric_tokens(text) {
                    assert!(allowed.contains(&token), "{case}: invented number {token} in {text:?}");
                }
            }
            serde_json::Value::Array(values) => values.iter().for_each(|value| walk_strings(value, allowed, case)),
            serde_json::Value::Object(map) => {
                for (key, value) in map {
                    if key != "query" {
                        walk_strings(value, allowed, case);
                    }
                }
            }
            _ => {}
        }
    }
}
