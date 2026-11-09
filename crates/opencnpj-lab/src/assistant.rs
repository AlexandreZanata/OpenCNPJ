//! Conversational answers: interpretation, execution, rendered blocks and files.
//!
//! One immutable [`Assistant`] holds every asset in memory, so a question costs the
//! interpretation and the query, never a reload. The reply is deterministic: the same
//! question, conversation and data produce the same text, numbers and file.

use crate::{
    query::EngineExecutor,
    questions::{self, Assets, DataArgs},
};
use anyhow::{Context, Result, bail};
use opencnpj_core::schema::col;
use opencnpj_insights::{
    agg,
    catalog::municipalities::Place,
    execute::{self, ExecContext, Value},
    export::{self as files, Cell, RecordLabels, Sheet},
    plan::{self, Backend},
    render::{self, Block, Labels, MunicipalityLabel, Payload, Provenance, RenderInput},
};
use opencnpj_nlu::{
    context::Conversation,
    policy::{CompanySearch, Decision, Interpretation},
};
use opencnpj_query::{
    export::{Export, ExportField, default_fields},
    ir::*,
    validate::ValidRequest,
};
use opencnpj_store::{
    DocRecord,
    search::{self, Limits, Query},
};
use serde_json::{Value as Json, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Cursor,
    time::Instant,
};

const UF_NAMES: [&str; 27] = [
    "Acre",
    "Alagoas",
    "Amazonas",
    "Amapá",
    "Bahia",
    "Ceará",
    "Distrito Federal",
    "Espírito Santo",
    "Goiás",
    "Maranhão",
    "Minas Gerais",
    "Mato Grosso do Sul",
    "Mato Grosso",
    "Pará",
    "Paraíba",
    "Pernambuco",
    "Piauí",
    "Paraná",
    "Rio de Janeiro",
    "Rio Grande do Norte",
    "Rondônia",
    "Roraima",
    "Rio Grande do Sul",
    "Santa Catarina",
    "Sergipe",
    "São Paulo",
    "Tocantins",
];

/// Columns shown in the chat for a page of records; the file can hold more.
const PREVIEW_FIELDS: [ExportField; 9] = [
    ExportField::Cnpj,
    ExportField::RazaoSocial,
    ExportField::NomeFantasia,
    ExportField::Situacao,
    ExportField::DataInicio,
    ExportField::CnaeDescricao,
    ExportField::Municipio,
    ExportField::Porte,
    ExportField::CapitalSocial,
];

/// Rows are establishments: without the CNPJ, a company's branches look like repeats.
const BRANCHES_NOTE: &str = "Cada linha é um estabelecimento: empresas com filiais aparecem uma vez por estabelecimento. \
     Inclua o CNPJ para distingui-las ou peça “só as matrizes” para uma linha por empresa.";

pub(crate) struct Options {
    /// Never write phone or e-mail columns (`--hide-contacts`).
    pub hide_contacts: bool,
    /// Rows per exported file.
    pub export_max_rows: u32,
}

pub(crate) struct Assistant {
    assets: Assets,
    labels: Labels,
    provenance: Provenance,
    record_labels: RecordLabels,
    options: Options,
}

pub(crate) struct File {
    pub filename: String,
    pub media_type: &'static str,
    pub bytes: Vec<u8>,
    pub rows: usize,
}

enum Outcome {
    Value(Value),
    Records { total: u64, docs: Vec<u32> },
    Meta(String),
}

fn micros(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_micros()).unwrap_or(u64::MAX)
}

impl Assistant {
    pub(crate) fn open(data: &DataArgs, options: Options) -> Result<Self> {
        let assets = Assets::open(data)?;
        let mut labels = Labels::default();
        let mut record_labels = RecordLabels::default();
        for row in assets.catalogue.rows() {
            if let Place::Municipality(geo) = &row.place {
                labels.municipalities.insert(row.id, MunicipalityLabel { name: row.name.clone(), uf: geo.uf.clone() });
                record_labels.municipalities.insert(format!("{:04}", row.tom), row.name.clone());
            }
        }
        labels.sectors = assets.sector_labels.clone();
        labels.ufs = UF_NAMES.iter().enumerate().map(|(i, name)| (i as u8, (*name).to_owned())).collect();
        labels.regions = ["Norte", "Nordeste", "Sudeste", "Sul", "Centro-Oeste"]
            .iter()
            .enumerate()
            .map(|(i, name)| (i as u16 + 1, (*name).to_owned()))
            .collect();
        // Questions over every activity read per-table rollups; build them before serving.
        if let Some(aggregates) = &assets.aggregates {
            aggregates.current().warm()?;
        }
        let (competence, aggregate_sha) = match &assets.aggregates {
            Some(set) => (set.current().manifest().competence, Some(set.current().manifest().snapshot_sha256.clone())),
            None => {
                let (year, month, _) = assets.anchor.to_ymd().context("anchor")?;
                (Competence::from_year_month(year, month).context("competence")?, None)
            }
        };
        let provenance = Provenance {
            competence,
            anchor: assets.anchor,
            snapshot_sha256: assets.snapshot_sha256.clone().or(aggregate_sha).unwrap_or_default(),
            municipality_catalogue: 202610,
            sector_catalogue: 1,
            population: Some(assets.catalogue.default_population()),
        };
        Ok(Self { assets, labels, provenance, record_labels, options })
    }

    pub(crate) fn assets(&self) -> &Assets {
        &self.assets
    }

    /// Data contracts of one interpretation (see `contracts`): counts agree with the records
    /// behind them, every record satisfies the query, a file holds the rows it promises, the
    /// sentence states the number of the tile, and a company search returns the searched name.
    pub(crate) fn data_findings(&self, interpretation: &Interpretation) -> Vec<crate::contracts::Finding> {
        use crate::contracts::{RecordScope, finding_of as finding, record_matches};
        let mut out = Vec::new();
        let context = ExecContext::new(&self.assets.catalogue, &self.assets.sectors);
        let Some(engine) = self.engine(&context) else { return out };
        if let Some(search) = &interpretation.company {
            let reply = self.ask_company(search, interpretation, &Conversation::default(), Instant::now(), BTreeMap::new());
            let approximate = reply["answer"]["blocks"].as_array().is_some_and(|b| b.iter().any(|b| b["type"] == "notice"));
            let tokens: Vec<String> = opencnpj_nlu::normalize::words(&search.name);
            for block in reply["answer"]["blocks"].as_array().into_iter().flatten().filter(|b| b["type"] == "records") {
                for row in block["rows"].as_array().into_iter().flatten() {
                    let names =
                        opencnpj_nlu::normalize::words(&format!("{} {}", row[1].as_str().unwrap_or(""), row[2].as_str().unwrap_or("")));
                    if !approximate && !tokens.iter().all(|t| names.iter().any(|n| n.starts_with(t.as_str()))) {
                        out.push(finding("company_result_without_name", format!("{} {}", row[0], row[1])));
                    }
                }
            }
            return out;
        }
        let Some(request) = &interpretation.request else { return out };
        let outcome = match self.run(request) {
            Ok(outcome) => outcome,
            Err(error) => return vec![finding("execution_error", error.to_string())],
        };
        let codes = |ids: &[MunicipalityId]| -> BTreeSet<String> {
            ids.iter().filter_map(|id| self.assets.catalogue.get(id.0)).map(|row| format!("{:04}", row.tom)).collect()
        };
        let ufs = |ids: &[UfId]| -> BTreeSet<String> { ids.iter().filter_map(|id| agg::uf_code(*id)).map(str::to_owned).collect() };
        match (&request.op, &outcome) {
            (Op::Count { metric }, Outcome::Value(Value::Scalar { value })) => {
                // D4: the sentence states the number of the tile.
                if let Ok(answer) = self.render(request, &outcome, interpretation) {
                    let text = answer.blocks.iter().find_map(|b| if let Block::Text { text } = b { Some(text.clone()) } else { None });
                    let kpi = answer.blocks.iter().find_map(|b| if let Block::Kpi { value, .. } = b { Some(value.clone()) } else { None });
                    if let (Some(text), Some(kpi)) = (text, kpi)
                        && !text.contains(&kpi)
                    {
                        out.push(finding("sentence_without_number", format!("{kpi} not in: {text}")));
                    }
                }
                // D1: an aggregate count equals the records an equivalent list finds.
                let base = request.as_request();
                let mut list = base.clone();
                list.op = Op::List { order: ListOrder::Cnpj, limit: 1 };
                list.time.period = None;
                let convertible = match (metric, base.time.period) {
                    (Metric::Stock, _) => true,
                    (Metric::Flow(FlowMetric::Closures), Some(Period::Days(days))) => {
                        list.scope.status_date = Some(days);
                        true
                    }
                    (Metric::Flow(FlowMetric::Registrations), Some(Period::Days(days))) => {
                        list.scope.opened = Some(days);
                        true
                    }
                    _ => false,
                };
                if convertible
                    && plan::plan(request).is_ok_and(|p| p.backend == Backend::Aggregates)
                    && let Ok(valid) = self.assets.canonical(list)
                    && let Ok(result) = engine.records_window(&valid, 0, 1)
                    && i64::try_from(result.total).ok() != Some(*value)
                {
                    out.push(finding("count_differs_from_records", format!("aggregates {value}, records {}", result.total)));
                }
            }
            (Op::List { .. }, Outcome::Records { total, docs }) => {
                // D2: every record of the page satisfies the query.
                let base = request.as_request();
                let scope = RecordScope {
                    municipality_codes: match &base.scope.place {
                        PlaceFilter::Municipalities(ids) => Some(codes(ids)),
                        _ => None,
                    },
                    uf_codes: match &base.scope.place {
                        PlaceFilter::Ufs(ids) => Some(ufs(ids)),
                        _ => None,
                    },
                    excluded_municipality_codes: codes(&base.scope.exclude.municipalities),
                    excluded_uf_codes: ufs(&base.scope.exclude.ufs),
                };
                if let Ok(records) = self.decode(docs) {
                    for record in &records {
                        if let Some(est) = record.estab.and_then(|i| record.company.estabelecimentos.get(i)) {
                            out.extend(record_matches(base, &scope, est));
                        }
                    }
                }
                // D3: the file holds the rows the answer promised.
                if let Some(export) = &interpretation.export
                    && *total <= 3000
                {
                    let promised = self.export_offer(request, export, &outcome)["rows"].as_u64().unwrap_or(0);
                    match self.export(base.clone(), export, None) {
                        Ok(file) if file.rows as u64 == promised => {}
                        Ok(file) => out.push(finding("file_rows_differ", format!("promised {promised}, wrote {}", file.rows))),
                        Err(error) => out.push(finding("file_error", error.to_string())),
                    }
                }
            }
            _ => {}
        }
        out
    }

    pub(crate) fn has_snapshot(&self) -> bool {
        self.assets.snapshot.is_some()
    }

    /// Interpret, execute and render one turn of the conversation.
    pub(crate) fn ask(&self, question: &str, state: &mut Conversation) -> Json {
        let started = Instant::now();
        let interpretation = self.assets.interpreter.interpret(question, state);
        let mut timing = BTreeMap::from([("interpret".to_owned(), micros(started))]);
        if let Some(company) = &interpretation.company {
            return self.ask_company(company, &interpretation, state, started, timing);
        }
        let Some(request) = interpretation.request.clone() else {
            return json!({
                "interpretation": interpretation,
                "answer": self.stop_answer(&interpretation),
                "extras": [],
                "export": null,
                "context": state,
                "timing_us": {"interpret": timing["interpret"], "total": micros(started)},
            });
        };
        let executed = Instant::now();
        let outcome = match self.run(&request) {
            Ok(outcome) => outcome,
            Err(error) => {
                let reason = if error.to_string().contains("snapshot") {
                    "Esta consulta precisa da base completa de estabelecimentos, que não está carregada neste servidor.".to_owned()
                } else {
                    format!("Não consegui executar esta consulta: {error}.")
                };
                return json!({
                    "interpretation": interpretation,
                    "answer": {"decision": "reject", "understood": "", "blocks": [{"type": "reject", "reason": reason}], "suggestions": []},
                    "extras": [],
                    "export": null,
                    "context": state,
                    "timing_us": {"interpret": timing["interpret"], "total": micros(started)},
                });
            }
        };
        timing.insert("execute".into(), micros(executed));
        if let Outcome::Value(value) = &outcome {
            let execution = json!({"status": "ok", "value": questions::value_json(value)});
            questions::remember_execution(state, &interpretation, &execution);
        }
        let rendered = Instant::now();
        let mut answer = match self.render(&request, &outcome, &interpretation) {
            Ok(answer) => serde_json::to_value(answer).expect("answer JSON"),
            Err(error) => json!({"decision": "reject", "understood": "", "blocks": [{"type": "reject", "reason": error.to_string()}]}),
        };
        timing.insert("render".into(), micros(rendered));
        let enriched = Instant::now();
        let extras = self.extras(&request, &outcome);
        timing.insert("extras".into(), micros(enriched));
        let export = interpretation.export.as_ref().map(|export| self.export_offer(&request, export, &outcome));
        timing.insert("total".into(), micros(started));
        answer["suggestions"] = json!(suggestions(&request, interpretation.export.is_some()));
        json!({
            "interpretation": interpretation,
            "answer": answer,
            "extras": extras,
            "export": export,
            "context": state,
            "timing_us": timing,
        })
    }

    fn ask_company(
        &self,
        company: &CompanySearch,
        interpretation: &Interpretation,
        state: &Conversation,
        started: Instant,
        mut timing: BTreeMap<String, u64>,
    ) -> Json {
        let reject = |reason: String, timing: &BTreeMap<String, u64>| {
            json!({
                "interpretation": interpretation,
                "answer": {"decision": "reject", "understood": "", "blocks": [{"type": "reject", "reason": reason}], "suggestions": []},
                "extras": [],
                "export": null,
                "context": state,
                "timing_us": {"interpret": timing["interpret"], "total": micros(started)},
            })
        };
        let Some(snapshot) = &self.assets.snapshot else {
            return reject(
                "Esta consulta precisa da base completa de estabelecimentos, que não está carregada neste servidor.".to_owned(),
                &timing,
            );
        };
        let executed = Instant::now();
        let mut pairs = vec![
            ("q".to_owned(), company.name.clone()),
            ("field".to_owned(), "any".to_owned()),
            ("fuzzy".to_owned(), "false".to_owned()),
            ("limit".to_owned(), "50".to_owned()),
        ];
        let toms: Vec<String> =
            company.municipalities.iter().filter_map(|id| self.assets.catalogue.get(id.0).map(|row| format!("{:04}", row.tom))).collect();
        if !toms.is_empty() {
            pairs.push(("municipio".to_owned(), toms.join(",")));
        }
        let ufs: Vec<&str> = company.ufs.iter().filter_map(|uf| agg::uf_code(*uf)).collect();
        if !ufs.is_empty() {
            pairs.push(("uf".to_owned(), ufs.join(",")));
        }
        let mut q = match crate::query::parse(&pairs) {
            Ok(q) => q,
            Err(error) => return reject(format!("Não consegui executar esta consulta: {error}."), &timing),
        };
        let mut result = match search::search(snapshot, &q, &Limits::DEFAULT) {
            Ok(result) => result,
            Err(error) => return reject(format!("Não consegui executar esta consulta: {error}."), &timing),
        };
        let mut fuzzy_note = false;
        if result.total == 0 {
            q.fuzzy = true;
            if let Ok(retry) = search::search(snapshot, &q, &Limits::DEFAULT)
                && retry.total > 0
            {
                result = retry;
                fuzzy_note = true;
            }
        }
        timing.insert("execute".into(), micros(executed));
        let rendered = Instant::now();
        let decoded = match self.decode(&result.docs) {
            Ok(decoded) => decoded,
            Err(error) => return reject(format!("Não consegui executar esta consulta: {error}."), &timing),
        };
        let mut roots = Vec::new();
        for record in &decoded {
            if !roots.iter().any(|r: &String| r == &record.company.basico) {
                roots.push(record.company.basico.clone());
            }
        }
        let mut preview = PREVIEW_FIELDS.to_vec();
        if self.options.hide_contacts {
            preview.retain(|f| !f.is_contact());
        }
        let mut blocks = Vec::new();
        let mut suggestions = Vec::new();
        if decoded.is_empty() {
            blocks.push(json!({
                "type": "text",
                "text": format!("Não encontrei empresas com o nome “{}”. Confira a grafia ou informe o CNPJ.", company.name),
            }));
        } else if roots.len() == 1 {
            let primary = decoded
                .iter()
                .find(|r| {
                    r.estab
                        .and_then(|i| r.company.estabelecimentos.get(i))
                        .and_then(|row| row.get(col::est::MATRIZ_FILIAL))
                        .is_some_and(|v| v.trim() == "1")
                })
                .unwrap_or(&decoded[0]);
            let summary = files::record_cells(
                snapshot,
                &self.record_labels,
                primary,
                &[ExportField::Cnpj, ExportField::RazaoSocial, ExportField::Situacao, ExportField::Municipio, ExportField::Uf],
            );
            let cnpj = summary[0].display();
            let razao = summary[1].display();
            let situacao = summary[2].display().to_lowercase();
            let municipio = summary[3].display();
            let uf = summary[4].display();
            let place = match (municipio.is_empty(), uf.is_empty()) {
                (false, false) => format!(", em {municipio}/{uf}"),
                (false, true) => format!(", em {municipio}"),
                (true, false) => format!(", em {uf}"),
                (true, true) => String::new(),
            };
            let tail = if result.total > 1 { format!(" ({} estabelecimentos).", group(result.total)) } else { ".".to_owned() };
            blocks.push(json!({
                "type": "text",
                "text": format!("Encontrei {razao} ({cnpj}), situação {situacao}{place}{tail}"),
            }));
            if let Ok(fields) = self.company_fields(primary)
                && !fields.is_empty()
            {
                blocks.push(json!({"type": "company", "fields": fields}));
            }
            if decoded.len() > 1
                && let Ok(rows) = self.cells(&decoded, &preview)
            {
                blocks.push(json!({
                    "type": "records",
                    "columns": preview.iter().map(|f| f.header()).collect::<Vec<_>>(),
                    "rows": rows.iter().map(|row| row.iter().map(Cell::display).collect::<Vec<_>>()).collect::<Vec<_>>(),
                    "total": result.total,
                    "page": 1,
                }));
            }
        } else {
            let text = if result.total as usize > decoded.len() {
                format!(
                    "Encontrei {} estabelecimentos para “{}”. Mostrando {}.",
                    group(result.total),
                    company.name,
                    group(decoded.len() as u64)
                )
            } else {
                format!("Encontrei {} estabelecimentos para “{}”.", group(result.total), company.name)
            };
            blocks.push(json!({"type": "text", "text": text}));
            if let Ok(rows) = self.cells(&decoded, &preview) {
                blocks.push(json!({
                    "type": "records",
                    "columns": preview.iter().map(|f| f.header()).collect::<Vec<_>>(),
                    "rows": rows.iter().map(|row| row.iter().map(Cell::display).collect::<Vec<_>>()).collect::<Vec<_>>(),
                    "total": result.total,
                    "page": 1,
                }));
            }
            let mut seen_roots = Vec::new();
            for record in &decoded {
                if seen_roots.len() >= 5 {
                    break;
                }
                if seen_roots.iter().any(|r: &String| r == &record.company.basico) {
                    continue;
                }
                seen_roots.push(record.company.basico.clone());
                let cells = files::record_cells(snapshot, &self.record_labels, record, &[ExportField::Cnpj]);
                let cnpj = cells[0].display();
                if !cnpj.is_empty() {
                    suggestions.push(format!("Dados do CNPJ {cnpj}"));
                }
            }
        }
        blocks.push(json!({
            "type": "sources",
            "items": [{
                "name": "Receita Federal — Dados Abertos do CNPJ",
                "competence": self.provenance.competence.to_string(),
                "snapshot_sha256": self.provenance.snapshot_sha256,
            }],
        }));
        let mut notes = interpretation.notes.clone();
        if fuzzy_note {
            notes.push("Não achei esse nome exato; mostrei nomes parecidos.".to_owned());
        }
        if !notes.is_empty() {
            blocks.push(json!({"type": "notice", "text": notes.join(" ")}));
        }
        let decision = if notes.is_empty() { "execute" } else { "execute_with_note" };
        let understood = format!("busca por nome · “{}” · RFB {}", company.name, self.provenance.competence);
        timing.insert("render".into(), micros(rendered));
        timing.insert("total".into(), micros(started));
        json!({
            "interpretation": interpretation,
            "answer": {
                "decision": decision,
                "understood": understood,
                "blocks": blocks,
                "suggestions": suggestions,
            },
            "extras": [],
            "export": null,
            "context": state,
            "timing_us": timing,
        })
    }

    fn stop_answer(&self, interpretation: &Interpretation) -> Json {
        let options: Vec<Json> = interpretation
            .options
            .iter()
            .enumerate()
            .map(|(i, option)| {
                let label = self.assets.canonical(option.clone()).map_or_else(|_| format!("Opção {}", i + 1), |valid| self.title(&valid));
                json!({"label": label, "reply": (i + 1).to_string()})
            })
            .collect();
        let block = if interpretation.decision == Decision::Clarify {
            json!({"type": "clarify", "question": interpretation.reason, "options": options})
        } else {
            json!({"type": "reject", "reason": interpretation.reason})
        };
        let mut blocks = vec![block];
        if !interpretation.notes.is_empty() {
            blocks.push(json!({"type": "notice", "text": interpretation.notes.join(" ")}));
        }
        json!({"decision": interpretation.decision, "understood": "", "blocks": blocks, "suggestions": []})
    }

    fn engine<'a>(&'a self, context: &'a ExecContext<'a>) -> Option<EngineExecutor<'a>> {
        self.assets.snapshot.as_ref().map(|snapshot| EngineExecutor { snapshot, context, limits: Limits::DEFAULT })
    }

    fn run(&self, request: &ValidRequest) -> Result<Outcome> {
        let context = ExecContext::new(&self.assets.catalogue, &self.assets.sectors);
        match plan::plan(request)?.backend {
            Backend::Static => Ok(Outcome::Meta(questions::meta_text(request))),
            _ if matches!(request.op, Op::List { .. } | Op::Lookup { .. }) => {
                let engine = self.engine(&context).context("supply --snapshot to execute lists and lookups")?;
                let result = engine.records(request)?;
                Ok(Outcome::Records { total: result.total, docs: result.docs })
            }
            Backend::Aggregates => {
                let aggregates = self.assets.aggregates.as_ref().context("supply --agg to execute aggregate-backed requests")?;
                Ok(Outcome::Value(execute::execute(request, aggregates, &context)?))
            }
            Backend::Engine => {
                Ok(Outcome::Value(self.engine(&context).context("supply --snapshot to execute engine-backed requests")?.execute(request)?))
            }
        }
    }

    /// Decode records in the requested order; blocks are read once when ids are sorted.
    fn decode(&self, docs: &[u32]) -> Result<Vec<DocRecord>> {
        let snapshot = self.assets.snapshot.as_ref().context("snapshot")?;
        let mut order: Vec<(usize, u32)> = docs.iter().copied().enumerate().collect();
        order.sort_unstable_by_key(|(_, doc)| *doc);
        let mut records: Vec<Option<DocRecord>> = (0..docs.len()).map(|_| None).collect();
        for chunk in order.chunks(4096) {
            let ids: Vec<u32> = chunk.iter().map(|(_, doc)| *doc).collect();
            for ((at, doc), record) in chunk.iter().zip(snapshot.docs(&ids)?) {
                if record.doc != *doc {
                    bail!("snapshot returned document {} for {doc}", record.doc);
                }
                records[*at] = Some(record);
            }
        }
        Ok(records.into_iter().flatten().collect())
    }

    fn cells(&self, records: &[DocRecord], fields: &[ExportField]) -> Result<Vec<Vec<Cell>>> {
        let snapshot = self.assets.snapshot.as_ref().context("snapshot")?;
        Ok(records.iter().map(|record| files::record_cells(snapshot, &self.record_labels, record, fields)).collect())
    }

    fn company_fields(&self, record: &DocRecord) -> Result<Vec<(String, String)>> {
        let first = self.cells(std::slice::from_ref(record), &ExportField::ALL)?;
        let mut fields = Vec::new();
        if let Some(row) = first.first() {
            for (field, cell) in ExportField::ALL.iter().zip(row) {
                if (self.options.hide_contacts && field.is_contact()) || matches!(cell, Cell::Empty) {
                    continue;
                }
                let key = match field {
                    ExportField::Cnpj => "cnpj".to_owned(),
                    ExportField::RazaoSocial => "razao_social".to_owned(),
                    ExportField::Situacao => "situacao".to_owned(),
                    other => other.header().to_owned(),
                };
                fields.push((key, cell.display()));
            }
        }
        Ok(fields)
    }

    fn title(&self, request: &ValidRequest) -> String {
        render::title(request, &self.labels)
    }

    fn render(&self, request: &ValidRequest, outcome: &Outcome, interpretation: &Interpretation) -> Result<render::Answer> {
        let mut records = None;
        let payload = match outcome {
            Outcome::Value(value) => Payload::Value(value),
            Outcome::Meta(text) => Payload::Meta { text: text.clone() },
            Outcome::Records { total, docs } => {
                let decoded = self.decode(docs)?;
                let fields = self.preview_fields(request, interpretation.export.as_ref());
                records = Some((fields.clone(), self.cells(&decoded, &fields)?));
                if matches!(request.op, Op::Lookup { .. }) {
                    let fields = decoded.first().map(|record| self.company_fields(record)).transpose()?.unwrap_or_default();
                    if fields.is_empty() {
                        bail!("o CNPJ informado não consta na base de {}", self.provenance.competence);
                    }
                    Payload::Company { fields }
                } else {
                    let rows = decoded
                        .iter()
                        .map(|record| {
                            let cells = files::record_cells(
                                self.assets.snapshot.as_ref().expect("decoded records come from the snapshot"),
                                &self.record_labels,
                                record,
                                &[ExportField::Cnpj, ExportField::RazaoSocial],
                            );
                            cells.iter().map(Cell::display).collect()
                        })
                        .collect();
                    Payload::Records { total: *total, page: request.output.page, rows }
                }
            }
        };
        let notice = (!interpretation.notes.is_empty()).then(|| interpretation.notes.join(" "));
        let mut answer = render::render(RenderInput {
            request,
            payload,
            provenance: &self.provenance,
            labels: &self.labels,
            followup: false,
            notice,
            suggestions: Vec::new(),
            unit_inferred: false,
            timing_us: BTreeMap::new(),
        })
        .map_err(|error| anyhow::anyhow!("{error}"))?;
        // The chat shows every cadastral column of the page, not only CNPJ and name.
        if let Some((fields, rows)) = records {
            for block in &mut answer.blocks {
                if let Block::Records { columns, rows: shown, .. } = block {
                    *columns = fields.iter().map(|f| f.header().to_owned()).collect();
                    *shown = rows.iter().map(|row| row.iter().map(Cell::display).collect()).collect();
                }
            }
        }
        Ok(answer)
    }

    fn preview_fields(&self, request: &ValidRequest, export: Option<&Export>) -> Vec<ExportField> {
        let mut fields = match export {
            Some(export) if !export.fields.is_empty() => export.fields.clone(),
            _ => {
                let mut fields = PREVIEW_FIELDS.to_vec();
                if default_fields(request.as_request()).contains(&ExportField::DataSituacao) {
                    fields.insert(4, ExportField::DataSituacao);
                }
                fields
            }
        };
        if self.options.hide_contacts {
            fields.retain(|f| !f.is_contact());
        }
        fields
    }

    /// Context a manager reads next to a count: the monthly series behind it and a sample
    /// of the real establishments. Only cheap backends are used; nothing is estimated.
    fn extras(&self, request: &ValidRequest, outcome: &Outcome) -> Vec<Json> {
        let mut extras = Vec::new();
        if let Op::Lookup { id } = &request.op {
            let Some(snapshot) = &self.assets.snapshot else { return extras };
            let prefix = &id.as_str()[..id.as_str().len().min(8)];
            let q = Query { cnpj_prefixes: vec![prefix.to_owned()], limit: 50, ..Default::default() };
            if let Ok(result) = search::search(snapshot, &q, &Limits::DEFAULT)
                && result.docs.len() > 1
                && let Ok(records) = self.decode(&result.docs)
            {
                let fields = self.preview_fields(request, None);
                if let Ok(rows) = self.cells(&records, &fields) {
                    extras.push(json!({
                        "type": "records",
                        "title": "Estabelecimentos desta empresa",
                        "columns": fields.iter().map(|f| f.header()).collect::<Vec<_>>(),
                        "rows": rows.iter().map(|row| row.iter().map(Cell::display).collect::<Vec<_>>()).collect::<Vec<_>>(),
                        "total": result.total,
                    }));
                }
            }
            return extras;
        }
        let Op::Count { metric } = request.op else { return extras };
        let base = request.as_request();
        let series: Vec<(FlowMetric, &str)> = match metric {
            Metric::Stock => vec![(FlowMetric::Registrations, "Aberturas"), (FlowMetric::Closures, "Baixas")],
            Metric::Flow(flow @ (FlowMetric::Registrations | FlowMetric::Closures | FlowMetric::NetRegistrations)) => {
                vec![(flow, render_flow_name(flow))]
            }
            _ => Vec::new(),
        };
        let period = match (metric, base.time.period) {
            (Metric::Flow(_), Some(Period::Days(days))) => Some(days),
            _ => self.last_twelve_months(),
        };
        if let Some(days) = period {
            let mut lines = Vec::new();
            for (flow, name) in series {
                let mut trend = base.clone();
                trend.op = Op::Trend { metric: flow, grain: Grain::Month };
                trend.scope.status = None;
                trend.time.period = Some(Period::Days(days));
                if let Some(rows) = self.aggregate_rows(trend) {
                    lines.push(json!({"name": name, "points": rows.iter().map(|(label, value)| json!({"label": label, "value": value})).collect::<Vec<_>>()}));
                }
            }
            if !lines.is_empty() {
                let (from, to) = (days.from.and_then(Day::to_ymd), days.to.and_then(Day::to_ymd));
                let range = match (from, to) {
                    (Some((fy, fm, _)), Some((ty, tm, _))) => format!(" de {fm:02}/{fy} a {tm:02}/{ty}"),
                    _ => String::new(),
                };
                extras.push(json!({"type": "series", "title": format!("Evolução mensal{range}"), "series": lines}));
            }
        }
        if !matches!(outcome, Outcome::Value(Value::Scalar { value }) if *value > 0) || self.assets.snapshot.is_none() {
            return extras;
        }
        // The establishments behind the number, newest first.
        let mut list = base.clone();
        list.op = Op::List { order: ListOrder::NewestFirst, limit: 8 };
        list.time.period = None;
        match (metric, base.time.period) {
            (Metric::Flow(FlowMetric::Closures), Some(Period::Days(days))) => list.scope.status_date = Some(days),
            (Metric::Flow(FlowMetric::Registrations), Some(Period::Days(days))) => {
                list.scope.opened.get_or_insert(days);
            }
            (Metric::Stock, _) => {}
            _ => return extras,
        }
        let context = ExecContext::new(&self.assets.catalogue, &self.assets.sectors);
        if let Ok(valid) = self.assets.canonical(list)
            && let Some(engine) = self.engine(&context)
            && let Ok(result) = engine.records(&valid)
            && let Ok(records) = self.decode(&result.docs)
        {
            let fields = self.preview_fields(&valid, None);
            if let Ok(rows) = self.cells(&records, &fields) {
                extras.push(json!({
                    "type": "records",
                    "title": "Alguns desses estabelecimentos (mais recentes primeiro)",
                    "columns": fields.iter().map(|f| f.header()).collect::<Vec<_>>(),
                    "rows": rows.iter().map(|row| row.iter().map(Cell::display).collect::<Vec<_>>()).collect::<Vec<_>>(),
                    "total": result.total,
                }));
            }
        }
        extras
    }

    /// Twelve complete months ending before the data anchor (aggregate buckets are monthly).
    fn last_twelve_months(&self) -> Option<DayRange> {
        let (year, month, _) = self.assets.anchor.to_ymd()?;
        let last = Day(Day::from_ymd(year, month, 1)?.0 - 1);
        let (ly, lm, _) = last.to_ymd()?;
        let start = u32::from(ly) * 12 + u32::from(lm) - 1 - 11;
        Some(Range { from: Day::from_ymd(u16::try_from(start / 12).ok()?, (start % 12) as u8 + 1, 1), to: Some(last) })
    }

    fn aggregate_rows(&self, request: Request) -> Option<Vec<(String, i64)>> {
        let valid = self.assets.canonical(request).ok()?;
        if plan::plan(&valid).ok()?.backend != Backend::Aggregates {
            return None;
        }
        let context = ExecContext::new(&self.assets.catalogue, &self.assets.sectors);
        match execute::execute(&valid, self.assets.aggregates.as_ref()?, &context).ok()? {
            Value::Rows(rows) => Some(
                rows.iter()
                    .map(|row| {
                        let (year, month) = (row.key / 12, row.key % 12 + 1);
                        (format!("{month:02}/{year}"), row.value)
                    })
                    .collect(),
            ),
            _ => None,
        }
    }

    fn export_columns(&self, request: &ValidRequest, export: &Export) -> (Vec<ExportField>, bool) {
        let mut fields = export.columns(request.as_request());
        let before = fields.len();
        if self.options.hide_contacts {
            fields.retain(|f| !f.is_contact());
        }
        let hidden = fields.len() != before;
        if fields.is_empty() {
            fields.push(ExportField::Cnpj);
        }
        (fields, hidden)
    }

    fn export_offer(&self, request: &ValidRequest, export: &Export, outcome: &Outcome) -> Json {
        let cap = u64::from(self.options.export_max_rows);
        let wanted = export.rows.map_or(cap, |rows| u64::from(rows).min(cap));
        let (rows, total, columns, hidden) = match outcome {
            Outcome::Records { total, .. } => {
                let (fields, hidden) = self.export_columns(request, export);
                ((*total).min(wanted), *total, fields.iter().map(|f| f.header().to_owned()).collect::<Vec<_>>(), hidden)
            }
            Outcome::Value(value) => {
                let table = render::value_table(request, value, &self.labels);
                let n = table.rows.len() as u64;
                let with_base = table.rows.iter().any(|(_, _, base)| *base != 0);
                (n, n, table.columns[..if with_base { 3 } else { 2 }].to_vec(), false)
            }
            Outcome::Meta(_) => (0, 0, Vec::new(), false),
        };
        let mut notes = Vec::new();
        if total > rows && export.rows.is_none_or(|asked| u64::from(asked) > rows) {
            notes.push(format!("O arquivo traz as primeiras {} de {} linhas (limite do servidor).", group(rows), group(total)));
        }
        if hidden {
            notes.push("Telefone e e-mail não são disponibilizados por este servidor.".to_owned());
        }
        if matches!(outcome, Outcome::Records { .. }) && !columns.iter().any(|c| c == ExportField::Cnpj.header()) {
            notes.push(BRANCHES_NOTE.to_owned());
        }
        json!({
            "format": export.format,
            "label": export.format.label(),
            "filename": self.filename(request, export),
            "rows": rows,
            "total": total,
            "columns": columns,
            "notes": notes,
            "request": request.as_request(),
            "export": export,
        })
    }

    fn filename(&self, request: &ValidRequest, export: &Export) -> String {
        let folded = opencnpj_nlu::normalize::fold(&self.title(request));
        let slug: String = folded
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let slug: String = slug.chars().take(80).collect::<String>().trim_end_matches('-').to_owned();
        format!("opencnpj-{slug}.{}", export.format.extension())
    }

    /// Build the file for a request the client received from [`Assistant::ask`]. The request
    /// is canonicalized and validated again: a client cannot widen it.
    pub(crate) fn export(&self, request: Request, export: &Export, question: Option<&str>) -> Result<File> {
        let request = self.assets.canonical(request)?;
        if matches!(request.op, Op::Meta { .. }) {
            bail!("esta resposta não gera arquivo");
        }
        let cap = self.options.export_max_rows;
        let limit = export.rows.map_or(cap, |rows| rows.min(cap));
        let title = self.title(&request);
        let mut about = Vec::new();
        if let Some(question) = question.filter(|q| !q.trim().is_empty()) {
            about.push(("Pergunta".to_owned(), question.trim().to_owned()));
        }
        about.push(("Consulta".to_owned(), request.as_request().to_string()));
        about.push((
            "Fonte".to_owned(),
            format!("Receita Federal do Brasil — Dados Abertos do CNPJ, competência {}", self.provenance.competence),
        ));
        about.push(("Data de referência".to_owned(), files::Cell::Date(self.provenance.anchor).display()));
        if !self.provenance.snapshot_sha256.is_empty() {
            about.push(("Snapshot (SHA-256)".to_owned(), self.provenance.snapshot_sha256.clone()));
        }
        let context = ExecContext::new(&self.assets.catalogue, &self.assets.sectors);
        let sheet = if matches!(request.op, Op::List { .. } | Op::Lookup { .. }) {
            let engine = self.engine(&context).context("supply --snapshot to export records")?;
            let result = engine.records_window(&request, 0, limit as usize)?;
            let (fields, hidden) = self.export_columns(&request, export);
            let records = self.decode(&result.docs)?;
            about.push(("Linhas".to_owned(), format!("{} de {} estabelecimentos", group(records.len() as u64), group(result.total))));
            about.push((
                "Privacidade".to_owned(),
                "CPFs de titulares de MEI/EI embutidos no nome foram mascarados (LGPD, art. 6º).".to_owned(),
            ));
            if hidden {
                about.push(("Contatos".to_owned(), "Telefone e e-mail não são disponibilizados por este servidor.".to_owned()));
            }
            let rows = self.cells(&records, &fields)?;
            let distinct =
                rows.iter().map(|row| row.iter().map(Cell::display).collect::<Vec<_>>()).collect::<std::collections::BTreeSet<_>>();
            let repeated = rows.len() - distinct.len();
            if repeated > 0 {
                about.push((
                    "Linhas repetidas".to_owned(),
                    format!("{} linhas iguais nas colunas escolhidas. {BRANCHES_NOTE}", group(repeated as u64)),
                ));
            }
            Sheet { title, columns: fields.iter().map(|f| f.header().to_owned()).collect(), rows, about }
        } else {
            let value = match self.run(&request)? {
                Outcome::Value(value) => value,
                _ => bail!("esta resposta não gera tabela"),
            };
            let table = render::value_table(&request, &value, &self.labels);
            let with_base = table.rows.iter().any(|(_, _, base)| *base != 0);
            let number = |value: Option<f64>| match value {
                Some(v) if table.integer => Cell::Integer(v as i64),
                Some(v) => Cell::Decimal(v),
                None => Cell::Text("sem dado".into()),
            };
            let mut columns = vec![table.columns[0].clone(), table.columns[1].clone()];
            if with_base {
                columns.push(table.columns[2].clone());
            }
            about.push(("Linhas".to_owned(), group(table.rows.len() as u64)));
            Sheet {
                title,
                columns,
                rows: table
                    .rows
                    .iter()
                    .map(|(label, value, base)| {
                        let mut row = vec![Cell::Text(label.clone()), number(*value)];
                        if with_base {
                            row.push(Cell::Integer(i64::try_from(*base).unwrap_or(i64::MAX)));
                        }
                        row
                    })
                    .collect(),
                about,
            }
        };
        let mut out = Cursor::new(Vec::new());
        files::write(export.format, &sheet, &mut out)?;
        Ok(File {
            filename: self.filename(&request, export),
            media_type: export.format.media_type(),
            bytes: out.into_inner(),
            rows: sheet.rows.len(),
        })
    }
}

fn render_flow_name(flow: FlowMetric) -> &'static str {
    match flow {
        FlowMetric::Registrations => "Aberturas",
        FlowMetric::Closures => "Baixas",
        FlowMetric::NetRegistrations => "Saldo",
        FlowMetric::MeiOptIns => "Opções pelo MEI",
        FlowMetric::MeiExits => "Saídas do MEI",
    }
}

fn group(value: u64) -> String {
    Cell::Integer(i64::try_from(value).unwrap_or(i64::MAX)).display()
}

/// Follow-ups the interpreter resolves against this answer (verified in the tests).
fn suggestions(request: &ValidRequest, exported: bool) -> Vec<String> {
    let mut out: Vec<&str> = match request.op {
        Op::Count { metric: Metric::Stock } => vec!["Agora liste esses estabelecimentos", "E só as matrizes?", "Distribua por porte"],
        Op::Count { metric: Metric::Flow(_) } => vec!["Agora liste esses estabelecimentos", "E só as matrizes?"],
        Op::Count { .. } => vec!["E só as matrizes?"],
        Op::List { .. } => vec!["E só as matrizes?", "Mostre os mais recentes primeiro"],
        Op::Rank { .. } | Op::Compare { .. } | Op::Breakdown { .. } | Op::Trend { .. } => vec![],
        Op::Lookup { .. } | Op::Meta { .. } => vec![],
    };
    if !matches!(request.op, Op::Meta { .. }) {
        out.extend(if exported {
            ["Agora em CSV", "Agora em TXT", "Só o nome e o CNPJ"].as_slice()
        } else {
            ["Me mande isso em Excel", "Me mande isso em CSV"].as_slice()
        });
    }
    out.into_iter().map(str::to_owned).collect()
}
