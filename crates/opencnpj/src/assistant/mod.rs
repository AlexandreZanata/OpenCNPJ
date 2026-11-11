//! Conversational answers: interpretation, execution, rendered blocks and files.
//!
//! One immutable [`Assistant`] holds every asset in memory, so a question costs the
//! interpretation and the query, never a reload. The reply is deterministic: the same
//! question, conversation and data produce the same text, numbers and file.

pub mod assets;
pub mod engine;
pub mod geo;
pub mod http;

use self::{
    assets::{self as questions, Assets, DataArgs},
    engine::EngineExecutor,
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
use std::{collections::BTreeMap, io::Cursor, time::Instant};

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

pub struct Options {
    /// Never write phone or e-mail columns (`--hide-contacts`).
    pub hide_contacts: bool,
    /// Rows per exported file.
    pub export_max_rows: u32,
}

pub struct Assistant {
    assets: Assets,
    labels: Labels,
    provenance: Provenance,
    record_labels: RecordLabels,
    options: Options,
}

pub struct File {
    pub filename: String,
    pub media_type: &'static str,
    pub bytes: Vec<u8>,
    pub rows: usize,
}

pub enum Outcome {
    Value(Value),
    Records { total: u64, docs: Vec<u32> },
    Meta(String),
}

fn micros(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_micros()).unwrap_or(u64::MAX)
}

impl Assistant {
    pub fn open(data: &DataArgs, options: Options) -> Result<Self> {
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

    pub fn assets(&self) -> &Assets {
        &self.assets
    }

    pub fn has_snapshot(&self) -> bool {
        self.assets.snapshot.is_some()
    }

    /// Competence of the data the assistant answers from (`YYYY-MM`).
    pub fn competence(&self) -> String {
        self.provenance.competence.to_string()
    }

    /// Interpret, execute and render one turn of the conversation.
    pub fn ask(&self, question: &str, state: &mut Conversation) -> Json {
        self.ask_capped(question, state, self.options.export_max_rows)
    }

    /// Like [`Assistant::ask`], offering files of at most `export_rows` rows (the caller's plan).
    pub fn ask_capped(&self, question: &str, state: &mut Conversation, export_rows: u32) -> Json {
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
        let export = interpretation.export.as_ref().map(|export| self.export_offer_capped(&request, export, &outcome, export_rows));
        timing.insert("total".into(), micros(started));
        answer["suggestions"] = json!(suggestions(&request, interpretation.export.is_some()));
        answer["filters"] = json!(filter_tags(request.as_request()));
        if let Some(blocks) = answer["blocks"].as_array_mut()
            && let Some(lead) = blocks.iter_mut().find(|block| block["type"] == "text")
            && let Some(exact) = lead["text"].as_str().and_then(|text| exact_period(text, request.as_request()))
        {
            lead["text"] = json!(exact);
        }
        json!({
            "interpretation": interpretation,
            "answer": answer,
            "extras": extras,
            "export": export,
            "context": state,
            "timing_us": timing,
        })
    }

    pub fn ask_company(
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
        let mut q = match engine::parse(&pairs) {
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

    pub fn engine<'a>(&'a self, context: &'a ExecContext<'a>) -> Option<EngineExecutor<'a>> {
        self.assets.snapshot.as_ref().map(|snapshot| EngineExecutor { snapshot, context, limits: Limits::DEFAULT })
    }

    pub fn run(&self, request: &ValidRequest) -> Result<Outcome> {
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
    pub fn decode(&self, docs: &[u32]) -> Result<Vec<DocRecord>> {
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

    pub fn cells(&self, records: &[DocRecord], fields: &[ExportField]) -> Result<Vec<Vec<Cell>>> {
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

    pub fn title(&self, request: &ValidRequest) -> String {
        render::title(request, &self.labels)
    }

    pub fn render(&self, request: &ValidRequest, outcome: &Outcome, interpretation: &Interpretation) -> Result<render::Answer> {
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
                // A search for companies with a contact shows the contact it asked for.
                let contacts: &[ExportField] = match request.as_request().scope.contact {
                    ContactNeed::Any => &[],
                    ContactNeed::Phone => &[ExportField::Telefone],
                    ContactNeed::Email => &[ExportField::Email],
                    ContactNeed::Both => &[ExportField::Telefone, ExportField::Email],
                };
                for (offset, field) in contacts.iter().enumerate() {
                    fields.insert(3 + offset, *field);
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

    pub fn export_offer(&self, request: &ValidRequest, export: &Export, outcome: &Outcome) -> Json {
        self.export_offer_capped(request, export, outcome, self.options.export_max_rows)
    }

    pub fn export_offer_capped(&self, request: &ValidRequest, export: &Export, outcome: &Outcome, cap: u32) -> Json {
        let cap = u64::from(cap.min(self.options.export_max_rows));
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
            notes.push(format!("O arquivo traz as primeiras {} de {} linhas (limite de linhas por arquivo).", group(rows), group(total)));
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
    pub fn export(&self, request: Request, export: &Export, question: Option<&str>) -> Result<File> {
        self.export_capped(request, export, question, self.options.export_max_rows)
    }

    /// Like [`Assistant::export`], writing at most `cap` rows (the caller's plan).
    pub fn export_capped(&self, request: Request, export: &Export, question: Option<&str>, cap: u32) -> Result<File> {
        let request = self.assets.canonical(request)?;
        if matches!(request.op, Op::Meta { .. }) {
            bail!("esta resposta não gera arquivo");
        }
        let cap = cap.min(self.options.export_max_rows);
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

fn day_br(day: Day) -> String {
    day.to_ymd().map_or_else(String::new, |(year, month, day)| format!("{day:02}/{month:02}/{year}"))
}

fn days_br(range: DayRange) -> String {
    match (range.from, range.to) {
        (Some(from), Some(to)) if from == to => format!("em {}", day_br(from)),
        (Some(from), Some(to)) => format!("de {} a {}", day_br(from), day_br(to)),
        (Some(from), None) => format!("desde {}", day_br(from)),
        (None, Some(to)) => format!("até {}", day_br(to)),
        (None, None) => String::new(),
    }
}

/// Flow sentences name months ("Entre setembro de 2026 e outubro de 2026, …"). A period that
/// is one day, starts inside a month or lies within one month is stated exactly instead.
fn exact_period(text: &str, request: &Request) -> Option<String> {
    let Some(Period::Days(range)) = request.time.period else { return None };
    let (from, to) = (range.from?, range.to?);
    let (first, rest) = text.strip_prefix("Entre ")?.split_once(" e ")?;
    let (second, tail) = rest.split_once(',')?;
    let (_, _, from_day) = from.to_ymd()?;
    if from == to {
        Some(format!("Em {},{tail}", day_br(from)))
    } else if from_day != 1 {
        Some(format!("De {} a {},{tail}", day_br(from), day_br(to)))
    } else if first == second {
        Some(format!("Em {first},{tail}"))
    } else {
        None
    }
}

fn reais(cents: u64) -> String {
    format!("R$ {}", group(cents / 100))
}

/// The filters of a request besides its place and activity, as short tags for the answer:
/// what was applied is shown, never implied.
fn filter_tags(request: &Request) -> Vec<String> {
    let scope = &request.scope;
    let mut tags = Vec::new();
    if let Some(name) = &scope.name_prefix {
        tags.push(format!("{} começa com {}", name.label_pt_br(), name.prefix));
    }
    if let Some(status) = scope.status
        && status != request.op.default_status()
    {
        let names: Vec<&str> = status
            .iter()
            .map(|status| match status {
                Status::Null => "nulas",
                Status::Active => "ativas",
                Status::Suspended => "suspensas",
                Status::Unfit => "inaptas",
                Status::Closed => "baixadas",
            })
            .collect();
        tags.push(if status == StatusSet::ALL { "qualquer situação".to_owned() } else { names.join(", ") });
    }
    match scope.mei {
        Tri::Yes => tags.push("MEI".into()),
        Tri::No => tags.push("não MEI".into()),
        Tri::Any => {}
    }
    match scope.simples {
        Tri::Yes => tags.push("Simples Nacional".into()),
        Tri::No => tags.push("fora do Simples".into()),
        Tri::Any => {}
    }
    if scope.porte != PorteSet::ALL {
        let names: Vec<&str> = scope
            .porte
            .iter()
            .map(|porte| match porte {
                Porte::Unreported => "porte não informado",
                Porte::Micro => "microempresa",
                Porte::Small => "pequeno porte",
                Porte::Other => "demais portes",
            })
            .collect();
        tags.push(names.join(", "));
    }
    match scope.head_office {
        Tri::Yes => tags.push("só matrizes".into()),
        Tri::No => tags.push("só filiais".into()),
        Tri::Any => {}
    }
    if !scope.natureza.is_empty() {
        let codes: Vec<String> = scope
            .natureza
            .iter()
            .map(|code| match code {
                NaturezaCode::Code(code) => format!("{code:04}"),
                NaturezaCode::Group(group) => format!("{group}xxx"),
            })
            .collect();
        tags.push(format!("natureza jurídica {}", codes.join(", ")));
    }
    match scope.contact {
        ContactNeed::Phone => tags.push("com telefone".into()),
        ContactNeed::Email => tags.push("com e-mail".into()),
        ContactNeed::Both => tags.push("com telefone e e-mail".into()),
        ContactNeed::Any => {}
    }
    if let Some(capital) = scope.capital {
        tags.push(match (capital.from, capital.to) {
            (Some(from), Some(to)) => format!("capital de {} a {}", reais(from), reais(to)),
            (Some(from), None) if from % 100 == 1 => format!("capital acima de {}", reais(from)),
            (Some(from), None) => format!("capital a partir de {}", reais(from)),
            (None, Some(to)) => format!("capital até {}", reais(to)),
            (None, None) => "capital informado".into(),
        });
    }
    if let Some(opened) = scope.opened {
        tags.push(format!("abertas {}", days_br(opened)));
    }
    if let Some(changed) = scope.status_date {
        tags.push(format!("situação alterada {}", days_br(changed)));
    }
    if let Some(Period::Days(period)) = request.time.period {
        tags.push(format!("período {}", days_br(period)));
    }
    tags
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
