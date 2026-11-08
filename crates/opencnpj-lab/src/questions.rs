//! Text-only interpretation and auditable question-to-engine evaluation.

use crate::{eval, query};
use anyhow::{Context, Result, ensure};
use opencnpj_core::schema::Table;
use opencnpj_ingest::source::sha256_file;
use opencnpj_insights::{
    agg,
    catalog::municipalities::{MunicipalityCatalogue, Place},
    execute::{self, AggregateSet, ExecContext, Value},
    plan::{self, Backend},
};
use opencnpj_nlu::{Interpreter, context::Conversation, lang::LanguagePack, policy::Interpretation};
use opencnpj_query::{ir::*, validate::ValidRequest};
use opencnpj_store::{Snapshot, search::Limits};
use serde::Deserialize;
use serde_json::{Value as Json, json};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
};

#[derive(clap::Args)]
pub struct DataArgs {
    #[arg(long, default_value = "crates/opencnpj-insights/catalog/municipalities.tsv")]
    pub catalog: PathBuf,
    #[arg(long, default_value = "crates/opencnpj-insights/catalog/sectors.toml")]
    pub sectors: PathBuf,
    #[arg(long, default_value = "2026-10-11")]
    pub anchor: String,
    #[arg(long)]
    pub agg: Option<PathBuf>,
    #[arg(long)]
    pub history: Option<PathBuf>,
    #[arg(long)]
    pub snapshot: Option<PathBuf>,
}

#[derive(clap::Args)]
pub struct AskArgs {
    #[command(flatten)]
    pub data: DataArgs,
    #[arg(long)]
    pub text: String,
    /// Optional local state JSON. The caller must authenticate transport outside this CLI.
    #[arg(long)]
    pub state: Option<PathBuf>,
}

#[derive(clap::Args)]
pub struct EvalArgs {
    /// Score text interpretation without executing either predicted or reference IR.
    #[arg(long)]
    pub interpret_only: bool,
    #[command(flatten)]
    pub data: DataArgs,
    #[arg(long)]
    pub set: PathBuf,
    #[arg(long)]
    pub out: PathBuf,
    #[arg(long)]
    pub milestone: bool,
}

struct Assets {
    catalogue: MunicipalityCatalogue,
    sectors: execute::SectorMap,
    interpreter: Interpreter,
    aggregates: Option<AggregateSet>,
    snapshot: Option<Snapshot>,
    anchor: Day,
    snapshot_sha256: Option<String>,
}

#[derive(Deserialize)]
struct SectorFile {
    sector: Vec<Sector>,
}
#[derive(Deserialize)]
struct Sector {
    aliases: Vec<String>,
    display: Display,
    cnae_core: Vec<String>,
}
#[derive(Deserialize)]
struct Display {
    sing: String,
    plural: String,
}

impl Assets {
    fn open(args: &DataArgs) -> Result<Self> {
        let parts = args.anchor.split('-').map(str::parse::<u16>).collect::<std::result::Result<Vec<_>, _>>()?;
        ensure!(parts.len() == 3, "anchor must be YYYY-MM-DD");
        let anchor = Day::from_ymd(parts[0], u8::try_from(parts[1])?, u8::try_from(parts[2])?).context("invalid data anchor")?;
        let catalogue = MunicipalityCatalogue::load(&args.catalog)?;
        let sectors = query::load_sectors(&args.sectors)?;
        let mut pack = LanguagePack::new();
        for row in catalogue.rows() {
            if let Place::Municipality(geo) = &row.place {
                pack.municipality(MunicipalityId(row.id), agg::uf_id(&geo.uf).context("UF")?, &row.name, &row.variants);
            }
        }
        let file: SectorFile = toml::from_str(&fs::read_to_string(&args.sectors)?)?;
        for (i, s) in file.sector.into_iter().enumerate() {
            let mut aliases = s.aliases;
            aliases.extend([s.display.sing, s.display.plural]);
            pack.sector(
                SectorId(u16::try_from(i)?),
                &aliases,
                SmallSet::new(
                    s.cnae_core.iter().map(|c| c.parse::<u32>().map(CnaeCode::Subclass)).collect::<std::result::Result<Vec<_>, _>>()?,
                ),
            );
        }
        let aggregates = args.agg.as_ref().map(|p| AggregateSet::open(p, args.history.as_deref())).transpose()?;
        if let Some(agg) = &aggregates {
            ensure!(sha256_file(&args.catalog)? == agg.current().manifest().municipality_sha256, "aggregate/catalogue identity mismatch");
        }
        let snapshot = args.snapshot.as_ref().map(|path| Snapshot::open(path)).transpose()?;
        let snapshot_sha256 = if let Some(snapshot) = &snapshot {
            snapshot.set_postings_cache_bytes(512 << 20);
            let sha = sha256_file(snapshot.path())?;
            if let Some(agg) = &aggregates {
                ensure!(sha == agg.current().manifest().snapshot_sha256, "snapshot/aggregate identity mismatch");
            }
            Some(sha)
        } else {
            None
        };
        let mut caps = query::validate_capabilities(&catalogue, &sectors, anchor);
        caps.current = Competence::from_year_month(parts[0], parts[1] as u8).context("competence")?;
        caps.competences = if let Some(aggregates) = &aggregates {
            ensure!(aggregates.current().manifest().competence == caps.current, "anchor and aggregate competence differ");
            (0..=caps.current.0).map(Competence).filter(|c| aggregates.get(*c).is_some()).collect()
        } else {
            vec![caps.current]
        };
        let interpreter = Interpreter::new(pack, caps, anchor);
        Ok(Self { catalogue, sectors, interpreter, aggregates, snapshot, anchor, snapshot_sha256 })
    }

    fn canonical(&self, request: Request) -> Result<ValidRequest> {
        query::validate_request(request, &self.catalogue, &self.sectors, self.anchor)
    }

    fn execute(&self, request: &ValidRequest, verify: bool) -> Result<Json> {
        let selected = plan::plan(request)?;
        let context = ExecContext::new(&self.catalogue, &self.sectors);
        if selected.backend == Backend::Static {
            let text=match request.op {
                Op::Meta{topic:MetaTopic::DataDate}=>format!("Competência {}; referência {}.",request.time.competence,request.time.anchor),
                Op::Meta{topic:MetaTopic::Sources}=>"Receita Federal: dados cadastrais; IBGE: população e geografia. Veja os manifestos dos catálogos e agregados.".into(),
                Op::Meta{topic:MetaTopic::Cnaes}=>format!("CNAEs da consulta: {:?}",request.scope.activity),
                Op::Meta{topic:MetaTopic::Methodology}=>"A unidade padrão é o estabelecimento. Estoque ativo usa situação 02. Registros usam início de atividade; baixas usam a última data da situação 08. Capital social não é faturamento.".into(),
                _=>"Posso contar e listar estabelecimentos, comparar locais e setores, mostrar distribuições, registros, baixas, estoque histórico e densidade por habitante.".into(),
            };
            return Ok(json!({"status":"ok","backend":"static","text":text,"verification":"not_numeric"}));
        }
        let engine = self.snapshot.as_ref().map(|snapshot| query::EngineExecutor { snapshot, context: &context, limits: Limits::DEFAULT });
        if matches!(request.op, Op::List { .. } | Op::Lookup { .. }) {
            let snapshot = self.snapshot.as_ref().context("supply --snapshot to execute lists and lookups")?;
            let result = engine.as_ref().context("engine")?.records(request)?;
            ensure!(!result.expansion_truncated, "query expansion was truncated");
            // Only public identifiers and cadastral business fields are copied to the artefact.
            let columns = Table::Estabelecimentos.row_columns();
            let rows = snapshot
                .docs(&result.docs)?
                .into_iter()
                .map(|record| {
                    let mut row = json!({"cnpj_root":record.company.basico});
                    if let Some(est) = record.estab.and_then(|i| record.company.estabelecimentos.get(i)) {
                        for field in [
                            "cnpj_ordem",
                            "cnpj_dv",
                            "identificador_matriz_filial",
                            "situacao_cadastral",
                            "data_inicio_atividade",
                            "cnae_fiscal_principal",
                            "uf",
                            "municipio",
                        ] {
                            if let Some(at) = columns.iter().position(|c| c.name == field) {
                                row[field] = json!(est.get(at));
                            }
                        }
                    }
                    row
                })
                .collect::<Vec<_>>();
            return Ok(
                json!({"status":"ok","backend":"engine","adapter":"filtered_records","total":result.total,"rows":rows,"returned":rows.len(),"page":request.output.page,"verification":"single_backend","contacts":"presence_filter_only"}),
            );
        }
        let (value, verification, reference) = match selected.backend {
            Backend::Aggregates => {
                let aggregates = self.aggregates.as_ref().context("supply --agg to execute aggregate-backed requests")?;
                let value = execute::execute(request, aggregates, &context)?;
                if !verify {
                    (value, "not_requested", None)
                } else if request.time.competence != aggregates.current().manifest().competence
                    || matches!(request.op.metric(), Some(Metric::ActiveChange { .. }))
                {
                    match crate::oracle::marginal_value(request, aggregates, &self.sectors)? {
                        Some(other) => {
                            let status = if value == other { "marginal_match" } else { "mismatch" };
                            (value, status, Some(other))
                        }
                        None => (value, "no_independent_reference", None),
                    }
                } else if let Some(engine) = &engine {
                    match engine.execute(request) {
                        Ok(other) => {
                            let status = if value == other { "engine_match" } else { "mismatch" };
                            (value, status, Some(other))
                        }
                        Err(error) => {
                            return Ok(
                                json!({"status":"reference_error","backend":"aggregates","value":value_json(&value),"error":error.to_string()}),
                            );
                        }
                    }
                } else {
                    (value, "no_independent_reference", None)
                }
            }
            Backend::Engine => {
                (engine.context("supply --snapshot to execute engine-backed requests")?.execute(request)?, "single_backend", None)
            }
            Backend::Static => unreachable!(),
        };
        Ok(
            json!({"status":if verification=="mismatch"{"mismatch"}else{"ok"},"backend":format!("{:?}",selected.backend).to_lowercase(),"value":value_json(&value),"verification":verification,"reference":reference.as_ref().map(value_json)}),
        )
    }
}

fn value_json(value: &Value) -> Json {
    match value {
        Value::Scalar { value } => json!({"scalar":value}),
        Value::Ratio { num, den } => json!({"numerator":num,"denominator":den}),
        Value::Rows(rows) => json!({"rows":rows.iter().map(|r|json!({"key":r.key,"value":r.value,"base":r.base})).collect::<Vec<_>>()}),
    }
}

fn remember_execution(state: &mut Conversation, interpretation: &Interpretation, execution: &Json) {
    if execution["status"] != "ok" {
        return;
    }
    let Some(request) = &interpretation.request else {
        return;
    };
    let Some(rows) = execution.pointer("/value/rows").and_then(Json::as_array) else {
        return;
    };
    let members = rows
        .iter()
        .filter_map(|row| {
            let key = row.get("key")?.as_u64()?;
            match request.op.dimension()? {
                Dim::Municipality => u16::try_from(key).ok().map(|id| Member::Municipality(MunicipalityId(id))),
                Dim::Uf => u8::try_from(key).ok().map(|id| Member::Uf(UfId(id))),
                Dim::Sector => u16::try_from(key).ok().map(|id| Member::Sector(SectorId(id))),
                _ => None,
            }
        })
        .collect();
    state.remember_result(members);
}

pub fn ask(args: AskArgs) -> Result<()> {
    let assets = Assets::open(&args.data)?;
    let mut state = match &args.state {
        Some(path) if path.exists() => serde_json::from_str(&fs::read_to_string(path)?)?,
        _ => Conversation::default(),
    };
    let interpreted = assets.interpreter.interpret(&args.text, &mut state);
    let execution = interpreted
        .request
        .as_ref()
        .map(|r| assets.execute(r, false).unwrap_or_else(|error| json!({"status":"error","error":error.to_string()})));
    if let Some(result) = &execution {
        remember_execution(&mut state, &interpreted, result);
    }
    println!("{}", serde_json::to_string_pretty(&json!({"interpretation":interpreted,"execution":execution,"context":state}))?);
    if let Some(path) = &args.state {
        fs::write(path, serde_json::to_string_pretty(&state)?)?;
    }
    Ok(())
}

fn increment(counts: &mut BTreeMap<String, u64>, key: &str) {
    *counts.entry(key.into()).or_default() += 1;
}
fn interpretation_match(prediction: &Interpretation, gold: &eval::Gold, expected: Option<&ValidRequest>) -> bool {
    match gold.decision.as_str() {
        "execute" | "execute_with_note" => prediction.request.as_ref().zip(expected).is_some_and(|(a, b)| a == b),
        "clarify" => prediction.decision == opencnpj_nlu::policy::Decision::Clarify,
        "reject" => prediction.decision == opencnpj_nlu::policy::Decision::Reject,
        _ => false,
    }
}

pub fn evaluate(args: EvalArgs) -> Result<()> {
    eval::ensure_milestone(&args.set, args.milestone)?;
    let assets = Assets::open(&args.data)?;
    let items = eval::load_items(&args.set)?;
    let mut ids = std::collections::BTreeSet::new();
    for item in &items {
        ensure!(ids.insert(&item.id), "duplicate item {}", item.id);
    }
    fs::create_dir_all(&args.out)?;
    let mut output = BufWriter::new(File::create(args.out.join("results.jsonl"))?);
    let mut predictions = BufWriter::new(File::create(args.out.join("predictions.jsonl"))?);
    let mut counts = BTreeMap::new();
    let mut slices: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    let mut exec_cache: BTreeMap<String, Json> = BTreeMap::new();
    let mut state = Conversation::default();
    let mut previous_session = String::new();
    for (index, item) in items.iter().enumerate() {
        let session = item
            .context
            .as_ref()
            .and_then(|c| c.get("session_id"))
            .and_then(Json::as_str)
            .map(str::to_owned)
            .or_else(|| item.id.split_once('#').map(|(id, _)| id.to_owned()))
            .unwrap_or_else(|| item.id.clone());
        if session != previous_session {
            state = Conversation::default();
            previous_session = session;
        }
        let prediction = assets.interpreter.interpret(&item.text, &mut state);
        let expected = item
            .gold
            .ir
            .clone()
            .map(|v| serde_json::from_value::<Request>(v).map_err(anyhow::Error::from).and_then(|r| assets.canonical(r)))
            .transpose();
        let (expected, error) = match expected {
            Ok(r) => (r, None),
            Err(e) => (None, Some(e.to_string())),
        };
        let correct = interpretation_match(&prediction, &item.gold, expected.as_ref());
        let pred_json = prediction.request.as_ref().map(serde_json::to_value).transpose()?;
        writeln!(
            predictions,
            "{}",
            json!({"id":item.id,"decision":serde_json::to_value(prediction.decision)?,"intent":pred_json.as_ref().and_then(|ir|ir.get("op")).and_then(|op|op.get("kind")),"ir":pred_json,"confidence":prediction.evidence})
        )?;
        let mut execute = |r: &ValidRequest| {
            let key = serde_json::to_string(r).expect("ValidRequest serializes");
            if let Some(cached) = exec_cache.get(&key) {
                let mut reused = cached.clone();
                if let Some(obj) = reused.as_object_mut() {
                    obj.insert("execution_provenance".into(), json!("reused_identical_ir_and_data"));
                }
                increment(&mut counts, "execution_reused_identical_ir");
                return reused;
            }
            let mut fresh = assets.execute(r, true).unwrap_or_else(|error| json!({"status":"error","error":error.to_string()}));
            if let Some(obj) = fresh.as_object_mut() {
                obj.insert("execution_provenance".into(), json!("fresh"));
            }
            increment(&mut counts, "execution_fresh");
            exec_cache.insert(key, fresh.clone());
            fresh
        };
        let oracle = if !args.interpret_only && (assets.aggregates.is_some() || assets.snapshot.is_some()) {
            expected.as_ref().map(&mut execute)
        } else {
            None
        };
        // A wrong predicted IR is retained, not executed as if it answered the question.
        let actual = if correct {
            oracle.clone()
        } else if !args.interpret_only && (assets.aggregates.is_some() || assets.snapshot.is_some()) {
            prediction.request.as_ref().map(&mut execute)
        } else {
            None
        };
        if let Some(result) = &actual {
            remember_execution(&mut state, &prediction, result);
        }
        let category = item.meta.get("category").and_then(Json::as_str).unwrap_or("existing_suite");
        let slice = slices.entry(category.into()).or_default();
        increment(slice, "items");
        increment(&mut counts, "items");
        let executed = prediction.request.is_some();
        if executed && !matches!(item.gold.decision.as_str(), "execute" | "execute_with_note") {
            increment(&mut counts, "out_of_policy_execution");
        }
        if let Some(act) = &item.gold.act {
            increment(&mut counts, "act_items");
            if *act == prediction.act {
                increment(&mut counts, "act_correct");
            }
        }
        let decision_matches = match item.gold.decision.as_str() {
            "execute" | "execute_with_note" => executed,
            "clarify" => prediction.decision == opencnpj_nlu::policy::Decision::Clarify,
            "reject" => prediction.decision == opencnpj_nlu::policy::Decision::Reject,
            _ => false,
        };
        if decision_matches {
            increment(&mut counts, "decision_correct");
        }
        if correct && actual.as_ref().is_some_and(|r| r["status"] == "ok") {
            increment(&mut counts, "correct_executed_answer");
        }
        if let Some(result) = &oracle {
            if result.pointer("/value/scalar").and_then(Json::as_i64) == Some(0) {
                increment(&mut counts, "gold_zero_scalar");
            }
            if result.get("total").and_then(Json::as_u64) == Some(0) {
                increment(&mut counts, "gold_empty_records");
            }
        }
        if correct {
            increment(&mut counts, "interpretation_correct");
            increment(slice, "interpretation_correct");
        } else {
            increment(&mut counts, "interpretation_wrong");
        }
        if let Some(error) = &error {
            increment(&mut counts, "invalid_gold");
            eprintln!("INVALID GOLD {}: {error}", item.id);
        }
        if let Some(result) = &oracle {
            let status = result.get("status").and_then(Json::as_str).unwrap_or("unknown");
            increment(&mut counts, &format!("gold_backend_{status}"));
            increment(slice, &format!("gold_backend_{status}"));
            if let Some(v) = result.get("verification").and_then(Json::as_str) {
                increment(&mut counts, &format!("verification_{v}"));
            }
        }
        writeln!(
            output,
            "{}",
            json!({"id":item.id,"text":item.text,"category":category,"expected_decision":item.gold.decision,"interpretation_correct":correct,"prediction":prediction,"gold_error":error,"gold_canonical":expected,"gold_execution":oracle,"predicted_execution":actual})
        )?;
        if (index + 1) % 50 == 0 {
            output.flush()?;
            println!("processed {}/{} correct {}", index + 1, items.len(), counts.get("interpretation_correct").unwrap_or(&0));
        }
    }
    output.flush()?;
    predictions.flush()?;
    let summary = json!({"set":args.set,"set_sha256":sha256_file(&args.set)?,"data_anchor":args.data.anchor,"snapshot_sha256":assets.snapshot_sha256,"counts":counts,"categories":slices,"method":"Actual text-only deterministic interpretation; gold IR used only for canonical scoring and separate backend reference, never as a prediction or conversation seed.","evidence_score":"Rule evidence is not a calibrated probability.","human_validation":"Not performed; human validation is deferred to product refinement."});
    fs::write(args.out.join("summary.json"), serde_json::to_string_pretty(&summary)?)?;
    println!("{}", serde_json::to_string_pretty(&summary["counts"])?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commercial_round_has_unique_questions_and_valid_frozen_labels() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let set = root.join("crates/opencnpj-lab/eval/commercial-round-01/questions.jsonl");
        let items = eval::load_items(&set).unwrap();
        assert_eq!(items.len(), 1000);
        let catalogue = MunicipalityCatalogue::load(&root.join("crates/opencnpj-insights/catalog/municipalities.tsv")).unwrap();
        let sectors = query::load_sectors(&root.join("crates/opencnpj-insights/catalog/sectors.toml")).unwrap();
        let mut ids = std::collections::BTreeSet::new();
        let mut texts = std::collections::BTreeSet::new();
        let mut categories = BTreeMap::new();
        let mut decisions = BTreeMap::new();
        for item in items {
            assert!(ids.insert(item.id.clone()));
            assert!(texts.insert(opencnpj_nlu::normalize::fold(&item.text)));
            increment(&mut categories, item.meta["category"].as_str().unwrap());
            increment(&mut decisions, &item.gold.decision);
            assert_eq!(item.meta["human_validated"], false);
            if let Some(ir) = item.gold.ir {
                assert_eq!(item.gold.decision, "execute");
                let request: Request = serde_json::from_value(ir).unwrap();
                query::validate_request(request, &catalogue, &sectors, Day::from_ymd(2026, 10, 11).unwrap())
                    .unwrap_or_else(|e| panic!("{}: {e}", item.id));
            } else {
                assert!(matches!(item.gold.decision.as_str(), "clarify" | "reject"));
            }
        }
        assert_eq!(categories.len(), 20);
        assert!(categories.values().all(|n| *n == 50));
        assert_eq!(decisions, BTreeMap::from([("execute".into(), 895), ("clarify".into(), 50), ("reject".into(), 55)]));
    }
}
