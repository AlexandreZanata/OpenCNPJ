//! Text-only interpretation and auditable question-to-engine evaluation.

use crate::{eval, query};
use anyhow::{Context, Result, ensure};
pub(crate) use opencnpj::assistant::assets::{Assets, DataArgs, meta_text, remember_execution, value_json};
use opencnpj_core::schema::Table;
use opencnpj_ingest::source::sha256_file;
use opencnpj_insights::{
    execute::{self, ExecContext},
    plan::{self, Backend},
};
use opencnpj_nlu::{context::Conversation, policy::Interpretation};
use opencnpj_query::{ir::*, validate::ValidRequest};
use opencnpj_store::search::Limits;
use serde_json::{Value as Json, json};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
};

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

/// Execute a request and, when asked, check it against an independent backend.
fn execute(assets: &Assets, request: &ValidRequest, verify: bool) -> Result<Json> {
    let selected = plan::plan(request)?;
    let context = ExecContext::new(&assets.catalogue, &assets.sectors);
    if selected.backend == Backend::Static {
        let text = meta_text(request);
        return Ok(json!({"status":"ok","backend":"static","text":text,"verification":"not_numeric"}));
    }
    let engine = assets.snapshot.as_ref().map(|snapshot| query::EngineExecutor { snapshot, context: &context, limits: Limits::DEFAULT });
    if matches!(request.op, Op::List { .. } | Op::Lookup { .. }) {
        let snapshot = assets.snapshot.as_ref().context("supply --snapshot to execute lists and lookups")?;
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
            let aggregates = assets.aggregates.as_ref().context("supply --agg to execute aggregate-backed requests")?;
            let value = execute::execute(request, aggregates, &context)?;
            if !verify {
                (value, "not_requested", None)
            } else if request.time.competence != aggregates.current().manifest().competence
                || matches!(request.op.metric(), Some(Metric::ActiveChange { .. }))
            {
                match crate::oracle::marginal_value(request, aggregates, &assets.sectors)? {
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
        .map(|r| execute(&assets, r, false).unwrap_or_else(|error| json!({"status":"error","error":error.to_string()})));
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
pub(crate) fn interpretation_match(prediction: &Interpretation, gold: &eval::Gold, expected: Option<&ValidRequest>) -> bool {
    let export = gold.export.clone().map(serde_json::from_value::<opencnpj_query::export::Export>).transpose();
    let company = gold.company.clone().map(serde_json::from_value::<opencnpj_nlu::policy::CompanySearch>).transpose();
    match gold.decision.as_str() {
        "execute" | "execute_with_note" => {
            if gold.company.is_some() {
                matches!(prediction.decision, opencnpj_nlu::policy::Decision::Execute | opencnpj_nlu::policy::Decision::ExecuteWithNote)
                    && prediction.request.is_none()
                    && company.is_ok_and(|c| c == prediction.company)
            } else {
                prediction.request.as_ref().zip(expected).is_some_and(|(a, b)| a == b)
                    && export.is_ok_and(|e| e == prediction.export)
                    && company.is_ok_and(|c| c == prediction.company)
            }
        }
        "clarify" => prediction.decision == opencnpj_nlu::policy::Decision::Clarify,
        "reject" => prediction.decision == opencnpj_nlu::policy::Decision::Reject,
        _ => false,
    }
}

/// Items of one dialogue share a session id; every other item starts a conversation.
pub(crate) fn session_of(item: &eval::Item) -> String {
    item.context
        .as_ref()
        .and_then(|c| c.get("session_id"))
        .and_then(Json::as_str)
        .map(str::to_owned)
        .or_else(|| item.id.split_once('#').map(|(id, _)| id.to_owned()))
        .unwrap_or_else(|| item.id.clone())
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
        let session = session_of(item);
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
            json!({"id":item.id,"decision":serde_json::to_value(prediction.decision)?,"intent":pred_json.as_ref().and_then(|ir|ir.get("op")).and_then(|op|op.get("kind")),"ir":pred_json,"export":prediction.export,"confidence":prediction.evidence})
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
            let mut fresh = execute(&assets, r, true).unwrap_or_else(|error| json!({"status":"error","error":error.to_string()}));
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
        let executed = prediction.request.is_some() || prediction.company.is_some();
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
    use opencnpj_insights::catalog::municipalities::MunicipalityCatalogue;
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

    #[test]
    fn search_phrases_keep_the_expected_activity_and_intent() {
        use opencnpj_nlu::policy::Decision;

        #[derive(serde::Deserialize)]
        struct SectorIds {
            sector: Vec<SectorEntry>,
        }

        #[derive(serde::Deserialize)]
        struct SectorEntry {
            id: String,
        }

        let catalogue = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../opencnpj-insights/catalog");
        let data = DataArgs {
            catalog: catalogue.join("municipalities.tsv"),
            sectors: catalogue.join("sectors.toml"),
            anchor: "2026-10-11".into(),
            agg: None,
            history: None,
            snapshot: None,
        };
        let assets = Assets::open(&data).unwrap();
        let sector_ids: SectorIds = toml::from_str(&std::fs::read_to_string(&data.sectors).unwrap()).unwrap();
        let sectors: BTreeMap<_, _> =
            sector_ids.sector.iter().enumerate().map(|(id, entry)| (entry.id.as_str(), u16::try_from(id).unwrap())).collect();

        let cases = include_str!("../tests/fixtures/sector-phrases.tsv")
            .lines()
            .chain(include_str!("../tests/fixtures/redundant-phrases.tsv").lines());
        for line in cases.filter(|line| !line.starts_with('#') && !line.is_empty()) {
            let columns: Vec<_> = line.splitn(6, '\t').collect();
            let [expected, city, op, format, contact, question] = columns.as_slice() else {
                panic!("invalid search test fixture: {line}");
            };
            let result = assets.interpreter.interpret(question, &mut Conversation::default());

            if matches!(*expected, "?" | "!") {
                let decision = if *expected == "?" { Decision::Clarify } else { Decision::Reject };
                assert_eq!(result.decision, decision, "{question}: {result:?}");
                assert!(result.request.is_none() && result.company.is_none(), "{question}: {result:?}");
                continue;
            }
            let city = MunicipalityId(city.parse().unwrap());
            assert!(matches!(result.decision, Decision::Execute | Decision::ExecuteWithNote), "{question}: {result:?}");
            if let Some(expected_name) = expected.strip_prefix('@') {
                let company = result.company.unwrap_or_else(|| panic!("expected a company search: {question}"));
                assert_eq!(company.name, expected_name, "{question}");
                assert_eq!(company.municipalities, [city], "{question}");
                continue;
            }
            let request = result.request.unwrap_or_else(|| panic!("missing cadastral request: {question}"));
            let sector = sectors[expected];
            let cnaes = SmallSet::new(assets.sectors[&sector].iter().copied().map(CnaeCode::Subclass));
            assert_eq!(
                request.scope.activity,
                ActivityFilter::Sectors { ids: SmallSet::new([SectorId(sector)]), cnaes, scope: CnaeScope::Main },
                "{question}"
            );
            assert_eq!(request.scope.place, PlaceFilter::Municipalities(SmallSet::new([city])), "{question}");
            assert_eq!(request.scope.status, Some(StatusSet::only(Status::Active)), "{question}");
            let contact = match *contact {
                "phone" => ContactNeed::Phone,
                "email" => ContactNeed::Email,
                "both" => ContactNeed::Both,
                "-" => ContactNeed::Any,
                _ => panic!("invalid contact in fixture: {contact}"),
            };
            assert_eq!(request.scope.contact, contact, "{question}");
            match *op {
                "list" => assert!(matches!(request.op, Op::List { .. }), "{question}: {:?}", request.op),
                "count" => assert!(matches!(request.op, Op::Count { .. }), "{question}: {:?}", request.op),
                "by_size" => assert!(matches!(request.op, Op::Breakdown { dim: Dim::Porte, .. }), "{question}: {:?}", request.op),
                limit if limit.starts_with("list=") => {
                    let expected_limit = limit["list=".len()..].parse::<u16>().unwrap();
                    assert!(matches!(request.op, Op::List { limit, .. } if limit == expected_limit), "{question}: {:?}", request.op);
                }
                _ => panic!("unknown operation: {op}"),
            }
            if *format == "-" {
                assert!(result.export.is_none(), "{question}");
            } else {
                let export = result.export.unwrap_or_else(|| panic!("missing export: {question}"));
                assert_eq!(export.format.extension(), *format, "{question}");
                assert_eq!(export.rows, None, "{question}");
            }
        }

        let mut conversation = Conversation::default();
        let original = assets.interpreter.interpret("Me passe as farmácias de Sorriso MT", &mut conversation).request.unwrap();
        let followup =
            assets.interpreter.interpret("Dessas, só as que têm um endereço de e-mail cadastrado", &mut conversation).request.unwrap();
        assert_eq!(followup.op, original.op);
        assert_eq!(followup.scope.place, original.scope.place);
        assert_eq!(followup.scope.activity, original.scope.activity);
        assert_eq!(followup.scope.contact, ContactNeed::Email);
    }

    /// Every file request of the skills round, dialogues included, is interpreted to its
    /// gold request and gold export.
    #[test]
    fn skills_export_round_is_interpreted_exactly() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let data = DataArgs {
            catalog: root.join("crates/opencnpj-insights/catalog/municipalities.tsv"),
            sectors: root.join("crates/opencnpj-insights/catalog/sectors.toml"),
            anchor: "2026-10-11".into(),
            agg: None,
            history: None,
            snapshot: None,
        };
        let assets = Assets::open(&data).unwrap();
        let items = eval::load_items(&root.join("crates/opencnpj-lab/eval/skills-export-01/questions.jsonl")).unwrap();
        assert!(items.len() >= 1500);
        let (mut state, mut session) = (Conversation::default(), String::new());
        let mut wrong = Vec::new();
        for item in &items {
            if session_of(item) != session {
                (state, session) = (Conversation::default(), session_of(item));
            }
            let prediction = assets.interpreter.interpret(&item.text, &mut state);
            let expected = item.gold.ir.clone().map(|ir| assets.canonical(serde_json::from_value(ir).unwrap()).unwrap());
            if !interpretation_match(&prediction, &item.gold, expected.as_ref()) {
                wrong.push(&item.id);
            }
        }
        assert!(wrong.is_empty(), "{} wrong: {wrong:?}", wrong.len());
    }

    /// Every search-style, contact, period and unknown-word question of the prospecting
    /// round is interpreted to its gold request, company search or clarification.
    #[test]
    fn prospecting_round_is_interpreted_exactly() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let data = DataArgs {
            catalog: root.join("crates/opencnpj-insights/catalog/municipalities.tsv"),
            sectors: root.join("crates/opencnpj-insights/catalog/sectors.toml"),
            anchor: "2026-10-11".into(),
            agg: None,
            history: None,
            snapshot: None,
        };
        let assets = Assets::open(&data).unwrap();
        let items = eval::load_items(&root.join("crates/opencnpj-lab/eval/prospecting-01/questions.jsonl")).unwrap();
        assert!(items.len() >= 1400);
        let mut wrong = Vec::new();
        for item in &items {
            let prediction = assets.interpreter.interpret(&item.text, &mut Conversation::default());
            let expected = item.gold.ir.clone().map(|ir| assets.canonical(serde_json::from_value(ir).unwrap()).unwrap());
            if !interpretation_match(&prediction, &item.gold, expected.as_ref()) {
                wrong.push(&item.id);
            }
        }
        assert!(wrong.is_empty(), "{} wrong: {wrong:?}", wrong.len());
    }

    /// Every company-name search of the skills round, refusals and activity lists included,
    /// is interpreted to its gold company search or gold request.
    #[test]
    fn skills_company_round_is_interpreted_exactly() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let data = DataArgs {
            catalog: root.join("crates/opencnpj-insights/catalog/municipalities.tsv"),
            sectors: root.join("crates/opencnpj-insights/catalog/sectors.toml"),
            anchor: "2026-10-11".into(),
            agg: None,
            history: None,
            snapshot: None,
        };
        let assets = Assets::open(&data).unwrap();
        let items = eval::load_items(&root.join("crates/opencnpj-lab/eval/skills-company-01/questions.jsonl")).unwrap();
        assert!(items.len() >= 600);
        let (mut state, mut session) = (Conversation::default(), String::new());
        let mut wrong = Vec::new();
        for item in &items {
            if session_of(item) != session {
                (state, session) = (Conversation::default(), session_of(item));
            }
            let prediction = assets.interpreter.interpret(&item.text, &mut state);
            let expected = item.gold.ir.clone().map(|ir| assets.canonical(serde_json::from_value(ir).unwrap()).unwrap());
            if !interpretation_match(&prediction, &item.gold, expected.as_ref()) {
                wrong.push(&item.id);
            }
        }
        assert!(wrong.is_empty(), "{} wrong: {wrong:?}", wrong.len());
    }
}
