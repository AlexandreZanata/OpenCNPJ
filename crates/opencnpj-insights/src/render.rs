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
    let text = templates.render(&key, seed, &values)?;
    let mut blocks = vec![Block::Text { text }];
    payload_blocks(request, &input, &mut blocks)?;
    blocks.push(Block::Methodology { items: methodology(request, &input) });
    blocks.push(Block::Sources {
        items: vec![Source {
            name: "Receita Federal — Dados Abertos do CNPJ".into(),
            competence: input.provenance.competence.to_string(),
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
    let mut values = BTreeMap::new();
    let (unit_singular, unit_plural) = unit(request);
    let (status_singular, status_plural) = status_adjective(request);
    let (sector_singular, sector_plural) = sector_label(request, input.labels);
    for key in schema("") {
        values.insert(key, String::new());
    }
    values.insert("place", place_label(request, input.labels));
    let place_em = place_em(request, input.labels);
    values.insert("place_em_cap", capitalize(&place_em));
    values.insert("place_em", place_em);
    values.insert("sector:label", sector_singular);
    values.insert("sector:label_plural", sector_plural);
    values.insert("unit", unit_plural.clone());
    values.insert("unit:plural(value)", unit_plural.clone());
    values.insert("status", status_plural.clone());
    values.insert("status:adj(value)", status_plural.clone());
    values.insert("metric_label", metric_label(request));
    values.insert("flow", flow_label(request).into());
    values.insert("competence:month_year", month_year(input.provenance.competence));
    values.insert("snapshot_sha256", input.provenance.snapshot_sha256.clone());
    values.insert(
        "catalogue",
        format!("municípios {} e setores {}", input.provenance.municipality_catalogue, input.provenance.sector_catalogue),
    );
    values.insert("min_base:int", min_base(request).to_string());
    let _ = (unit_singular, status_singular);
    values
}

fn fill_payload(
    request: &opencnpj_query::ir::Request,
    input: &RenderInput<'_>,
    values: &mut BTreeMap<&'static str, String>,
) -> Result<(), RenderError> {
    let (unit_singular, unit_plural) = unit(request);
    let (status_singular, status_plural) = status_adjective(request);
    values.insert("from:date", period_date(request, true).unwrap_or_default());
    values.insert("to:date", period_date(request, false).unwrap_or_default());
    values.insert("from:month_year", period_month(request, true).unwrap_or_default());
    values.insert("to:month_year", period_month(request, false).unwrap_or_default());
    match &input.payload {
        Payload::Value(Value::Scalar { value }) => {
            values.insert("value:int", int(*value));
            values.insert("value:dec1", decimal1(*value as f64));
            values.insert("unit:plural(value)", pluralize(*value, &unit_singular, &unit_plural).into());
            values.insert("status:adj(value)", pluralize(*value, &status_singular, &status_plural).into());
        }
        Payload::Value(Value::Ratio { num, den }) => {
            let value = display_ratio(request, *num, *den);
            values.insert("value:dec1", decimal1(value));
            values.insert("value:int", int(i64::try_from(*num).unwrap_or(i64::MAX)));
            let population = match request.op.metric() {
                Some(Metric::Per10k(_)) => *den,
                Some(Metric::InhabitantsPer(_)) => *num,
                _ => 0,
            };
            values.insert("population:int", int(i64::try_from(population).unwrap_or(i64::MAX)));
            if let Some(reference) = input.provenance.population {
                values.insert("population_year", population_year(reference).to_string());
            }
            values.insert("unit:plural(value)", pluralize(2, &unit_singular, &unit_plural).into());
            values.insert("status:adj(value)", pluralize(2, &status_singular, &status_plural).into());
        }
        Payload::Value(Value::Rows(rows)) => {
            let dim = request.op.dimension();
            let labels: Vec<String> = rows.iter().map(|row| row_label(request, row.key, dim, input.labels)).collect();
            let displays: Vec<String> = rows.iter().map(|row| row_value(request, *row)).collect();
            if let Some((label, value)) = labels.first().zip(displays.first()) {
                values.insert("r1", format!("{label} ({value})"));
                values.insert("top", format!("{label} ({value})"));
            }
            if let Some((label, value)) = labels.get(1).zip(displays.get(1)) {
                values.insert("r2", format!("{label} ({value})"));
            }
            if let Some((label, value)) = labels.get(2).zip(displays.get(2)) {
                values.insert("r3", format!("{label} ({value})"));
            }
            values.insert("n:int", rows.len().to_string());
            values.insert("top", list(&labels.iter().take(3).zip(displays.iter()).map(|(l, v)| format!("{l} ({v})")).collect::<Vec<_>>()));
            if matches!(request.op, Op::Compare { .. }) && rows.len() >= 2 {
                let (a, b) = (rows[0], rows[1]);
                let a_display = row_value(request, a);
                let b_display = row_value(request, b);
                let a_metric = metric_value(request, a);
                let b_metric = metric_value(request, b);
                let diff = a_metric - b_metric;
                values.insert("a.place", labels[0].clone());
                values.insert("b.place", labels[1].clone());
                values.insert("a.value:int", a_display.clone());
                values.insert("a.value:dec1", decimal1(a_metric));
                values.insert("a.share:pct1", pct1(a_metric));
                values.insert("b.value:int", b_display.clone());
                values.insert("b.value:dec1", decimal1(b_metric));
                values.insert("b.share:pct1", pct1(b_metric));
                values.insert("diff_pct:pct1", if b_metric == 0.0 { "—".into() } else { pct1(diff / b_metric.abs() * 100.0) });
                values.insert(
                    "diff_dir",
                    if diff > 0.0 {
                        "mais"
                    } else if diff < 0.0 {
                        "menos"
                    } else {
                        "igual"
                    }
                    .into(),
                );
            }
            if matches!(request.op, Op::Trend { .. }) && !rows.is_empty() {
                let first = rows.first().map_or(0, |row| row.value);
                let last = rows.last().map_or(0, |row| row.value);
                values.insert("first:int", int(first));
                values.insert("last:int", int(last));
                values.insert("change_pct:pct1", if first == 0 { "—".into() } else { pct1((last - first) as f64 * 100.0 / first as f64) });
            }
        }
        Payload::Records { total, page, rows } => {
            values.insert("total:int", int(i64::try_from(*total).unwrap_or(i64::MAX)));
            values.insert("n:int", rows.len().to_string());
            values.insert("page:int", page.to_string());
            values.insert("unit:plural(value)", pluralize(2, &unit_singular, &unit_plural).into());
            values.insert("status:adj(value)", pluralize(2, &status_singular, &status_plural).into());
        }
        Payload::Company { fields } => {
            for (key, value) in fields {
                match key.as_str() {
                    "cnpj" => values.insert("cnpj", value.clone()),
                    "name" | "razao_social" => values.insert("name", value.clone()),
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
            blocks.push(Block::Kpi { label: kpi_label(request), value: decimal1(display_ratio(request, *num, *den)) });
        }
        Payload::Value(Value::Rows(rows)) => {
            let dim = request.op.dimension();
            let labels: Vec<String> = rows.iter().map(|row| row_label(request, row.key, dim, input.labels)).collect();
            let displays: Vec<String> = rows.iter().map(|row| row_value(request, *row)).collect();
            let table_rows: Vec<Vec<String>> = labels.iter().cloned().zip(displays.iter().cloned()).map(|(a, b)| vec![a, b]).collect();
            if matches!(request.op, Op::Compare { .. }) && rows.len() >= 2 {
                let a_metric = metric_value(request, rows[0]);
                let b_metric = metric_value(request, rows[1]);
                let diff = a_metric - b_metric;
                blocks.push(Block::Comparison {
                    members: rows
                        .iter()
                        .map(|row| ComparisonMember {
                            label: row_label(request, row.key, dim, input.labels),
                            value: row_value(request, *row),
                            base: (row.base != 0).then(|| int(i64::try_from(row.base).unwrap_or(i64::MAX))),
                        })
                        .collect(),
                    difference_abs: decimal1(diff),
                    difference_pct: (b_metric != 0.0).then(|| pct1(diff / b_metric.abs() * 100.0)),
                });
            }
            let chart = labels
                .iter()
                .zip(rows.iter())
                .map(|(label, row)| ChartPoint { label: label.clone(), value: ratio_value(request, *row) })
                .collect::<Vec<_>>();
            if matches!(request.op, Op::Trend { .. }) {
                blocks.push(Block::Chart { kind: ChartKind::Line, series: chart });
            } else if matches!(request.op, Op::Rank { .. } | Op::Breakdown { .. } | Op::Compare { .. }) {
                blocks.push(Block::Chart { kind: ChartKind::Bar, series: chart });
            }
            blocks.push(Block::Table { columns: vec![column_label(dim, request), "Valor".into()], rows: table_rows, total: None });
        }
        Payload::Records { total, page, rows } => {
            blocks.push(Block::Records { columns: vec!["CNPJ".into(), "Nome".into()], rows: rows.clone(), total: *total, page: *page });
        }
        Payload::Company { fields } => blocks.push(Block::Company { fields: fields.clone() }),
        Payload::Meta { .. } => {}
    }
    Ok(())
}

fn display_ratio(request: &opencnpj_query::ir::Request, num: u64, den: u64) -> f64 {
    if den == 0 {
        return 0.0;
    }
    match request.op.metric() {
        Some(Metric::Per10k(_)) => num as f64 * 10_000.0 / den as f64,
        Some(Metric::InhabitantsPer(_)) => num as f64 / den as f64,
        Some(Metric::Share) => num as f64 * 100.0 / den as f64,
        _ => num as f64 / den as f64,
    }
}

fn metric_value(request: &opencnpj_query::ir::Request, row: Row) -> f64 {
    match request.op.metric() {
        Some(Metric::Per10k(_) | Metric::InhabitantsPer(_) | Metric::Share) => ratio_value(request, row),
        Some(Metric::ActiveChange { pct: true }) if row.base > 0 => row.value as f64 / row.base as f64,
        _ => row.value as f64,
    }
}

fn ratio_value(request: &opencnpj_query::ir::Request, row: Row) -> f64 {
    match request.op.metric() {
        Some(Metric::Per10k(_)) if row.base > 0 => row.value as f64 * 10_000.0 / row.base as f64,
        Some(Metric::InhabitantsPer(_)) if row.base > 0 => row.value as f64 / row.base as f64,
        Some(Metric::Share) if row.base > 0 => row.value as f64 * 100.0 / row.base as f64,
        _ => row.value as f64,
    }
}

fn row_value(request: &opencnpj_query::ir::Request, row: Row) -> String {
    match request.op.metric() {
        Some(Metric::Per10k(_) | Metric::InhabitantsPer(_)) => decimal1(ratio_value(request, row)),
        Some(Metric::Share) => pct1(ratio_value(request, row)),
        Some(Metric::ActiveChange { pct: true }) if row.base > 0 => pct1(row.value as f64 * 100.0 / row.base as f64),
        _ => int(row.value),
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
        Some(Dim::Sector) => labels.sectors.get(&(key as u16)).map(|label| label.plural.clone()).unwrap_or_else(|| key.to_string()),
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
        Some(Dim::Region(_)) | Some(Dim::Year) => key.to_string(),
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
    match request.op.metric() {
        Some(Metric::Flow(_)) => flow_label(request).into(),
        Some(Metric::Per10k(_)) => "Por dez mil habitantes".into(),
        Some(Metric::InhabitantsPer(_)) => "Habitantes por estabelecimento".into(),
        Some(Metric::ActiveChange { .. }) => "Variação de ativos".into(),
        Some(Metric::Share) => "Participação".into(),
        _ => {
            if request.unit == opencnpj_query::ir::Unit::HeadOffices {
                "Matrizes ativas".into()
            } else {
                "Estabelecimentos ativos".into()
            }
        }
    }
}

fn metric_label(request: &opencnpj_query::ir::Request) -> String {
    match request.op.metric() {
        Some(Metric::Flow(_)) => flow_label(request).into(),
        Some(Metric::Per10k(_)) => "estabelecimentos ativos por dez mil habitantes".into(),
        Some(Metric::InhabitantsPer(_)) => "habitantes por estabelecimento ativo".into(),
        Some(Metric::ActiveChange { .. }) => "variação de estabelecimentos ativos".into(),
        Some(Metric::Share) => "participação".into(),
        _ => "estabelecimentos ativos".into(),
    }
}

fn flow_label(request: &opencnpj_query::ir::Request) -> &'static str {
    match request.op.metric() {
        Some(Metric::Flow(FlowMetric::Registrations)) => "registros",
        Some(Metric::Flow(FlowMetric::Closures)) => "baixas",
        Some(Metric::Flow(FlowMetric::NetRegistrations)) => "saldo de registros",
        Some(Metric::Flow(FlowMetric::MeiOptIns)) => "adesões ao MEI",
        Some(Metric::Flow(FlowMetric::MeiExits)) => "saídas do MEI",
        _ => "",
    }
}

fn place_em(request: &opencnpj_query::ir::Request, labels: &Labels) -> String {
    match &request.scope.place {
        PlaceFilter::Any => "no Brasil".into(),
        PlaceFilter::Region(_) => "na região".into(),
        PlaceFilter::Near { .. } => format!("na {}", &place_label(request, labels)[2..]),
        _ => format!("em {}", place_label(request, labels)),
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn place_label(request: &opencnpj_query::ir::Request, labels: &Labels) -> String {
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
        PlaceFilter::Region(_) => "a região".into(),
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
            (format!("CNAE {}", list(&values)), format!("CNAEs {}", list(&values)))
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

fn unit(request: &opencnpj_query::ir::Request) -> (String, String) {
    match request.unit {
        opencnpj_query::ir::Unit::Establishments => ("estabelecimento".into(), "estabelecimentos".into()),
        opencnpj_query::ir::Unit::HeadOffices => ("matriz".into(), "matrizes".into()),
    }
}

fn status_adjective(request: &opencnpj_query::ir::Request) -> (String, String) {
    let status = request.scope.status.unwrap_or_else(|| request.op.default_status()).iter().next();
    let feminine = request.unit == opencnpj_query::ir::Unit::HeadOffices;
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
    if n == 1 { singular } else { plural }
}

fn methodology(request: &opencnpj_query::ir::Request, input: &RenderInput<'_>) -> Vec<String> {
    let mut items = Vec::new();
    items.push(format!("Atividade: {}", sector_label(request, input.labels).1));
    let status = request.scope.status.unwrap_or_else(|| request.op.default_status());
    items.push(format!("Situação cadastral: {}", list(&status.iter().map(status_name).collect::<Vec<_>>())));
    items.push(format!(
        "Unidade: {}",
        match request.unit {
            opencnpj_query::ir::Unit::Establishments => "estabelecimento (CNPJ completo)",
            opencnpj_query::ir::Unit::HeadOffices => "matriz",
        }
    ));
    if let Some(reference) = input.provenance.population {
        items.push(format!("População: {}", population_label(reference)));
    }
    if let Some(Period::Days(days)) = request.time.period
        && let (Some(from), Some(to)) = (days.from, days.to)
    {
        items.push(format!("Período: {} a {}", date(from), date(to)));
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
        && let Some(reference) = input.provenance.population
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
    if let opencnpj_query::ir::ActivityFilter::Sectors { cnaes, .. } = &request.scope.activity {
        let codes = cnaes.iter().map(cnae_label).collect::<Vec<_>>();
        items.push(Caveat { id: "C-SECT", text: format!("Atividade principal: CNAE {}.", list(&codes)) });
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

fn population_year(reference: PopulationRef) -> u16 {
    match reference {
        PopulationRef::IbgeCensus(year) | PopulationRef::IbgeEstimate(year) => year,
    }
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
    let digits = value.unsigned_abs().to_string();
    let mut grouped = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push('.');
        }
        grouped.push(ch);
    }
    if value < 0 { format!("-{grouped}") } else { grouped }
}

fn decimal1(value: f64) -> String {
    format!("{value:.1}").replace('.', ",")
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
        "population_year",
        "a.place",
        "a.value:int",
        "a.value:dec1",
        "a.share:pct1",
        "b.place",
        "b.value:int",
        "b.value:dec1",
        "b.share:pct1",
        "diff_pct:pct1",
        "diff_dir",
        "r1",
        "r2",
        "r3",
        "n:int",
        "min_base:int",
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
            populations: vec![PopulationRef::IbgeCensus(2022), PopulationRef::IbgeEstimate(2025)],
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
        }
    }

    fn provenance() -> Provenance {
        Provenance {
            competence: competence(2026, 10),
            anchor: day(2026, 10, 11),
            snapshot_sha256: "76f36078c1e5b727af8416262309d1285e39f054514218a75a46206d5786037b".into(),
            municipality_catalogue: 202610,
            sector_catalogue: 1,
            population: Some(PopulationRef::IbgeEstimate(2025)),
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
        let value = 187 - index * 40;
        let base = match metric {
            Metric::Per10k(_) | Metric::InhabitantsPer(_) => 100_000,
            Metric::Share => 1000,
            Metric::ActiveChange { pct: true } => 100,
            _ => 0,
        };
        Row { key, value, base }
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

    fn add_value(allowed: &mut BTreeSet<String>, request: &Request, value: &Value) {
        match value {
            Value::Scalar { value } => {
                add_source(allowed, &value.to_string());
                add_source(allowed, &int(*value));
                add_source(allowed, &decimal1(*value as f64));
            }
            Value::Ratio { num, den } => {
                add_source(allowed, &num.to_string());
                add_source(allowed, &den.to_string());
                let display = display_ratio(request, *num, *den);
                add_source(allowed, &decimal1(display));
                add_source(allowed, &pct1(display));
                add_source(allowed, &int(i64::try_from(*num).unwrap_or(i64::MAX)));
                add_source(allowed, &int(i64::try_from(*den).unwrap_or(i64::MAX)));
            }
            Value::Rows(rows) => {
                for row in rows {
                    add_source(allowed, &row.key.to_string());
                    add_source(allowed, &int(row.value));
                    add_source(allowed, &int(i64::try_from(row.base).unwrap_or(i64::MAX)));
                    add_source(allowed, &row_value(request, *row));
                    add_source(allowed, &decimal1(ratio_value(request, *row)));
                    if let Some(dim) = request.op.dimension() {
                        add_source(allowed, &row_label(request, row.key, Some(dim), &labels()));
                    }
                    if matches!(request.op, Op::Trend { .. }) {
                        add_source(allowed, &month_label(row.key));
                    }
                }
                add_source(allowed, &rows.len().to_string());
                if rows.len() >= 2 {
                    let a = metric_value(request, rows[0]);
                    let b = metric_value(request, rows[1]);
                    let diff = a - b;
                    add_source(allowed, &decimal1(diff));
                    add_source(allowed, &pct1(diff / b.abs() * 100.0));
                }
                if let (Some(first), Some(last)) = (rows.first(), rows.last()) {
                    add_source(allowed, &int((last.value - first.value) as f64 as i64));
                    add_source(allowed, &pct1((last.value - first.value) as f64 * 100.0 / first.value as f64));
                }
            }
        }
    }

    fn numeric_tokens(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut current = String::new();
        for ch in text.chars() {
            if ch.is_ascii_digit() {
                current.push(ch);
            } else if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() {
            out.push(current);
        }
        out
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
