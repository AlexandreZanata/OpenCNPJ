//! Evaluation and benchmark harness for the frozen intelligence sets.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use opencnpj_query::ir::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const CURRENT: Competence = Competence(321);
const ANCHOR: Day = Day(46305);

#[derive(Debug, Clone, Deserialize)]
pub struct Item {
    pub id: String,
    #[serde(default)]
    pub text: String,
    pub gold: Gold,
    #[serde(default)]
    pub meta: Value,
    #[serde(default)]
    pub rules: Vec<String>,
    #[serde(default)]
    pub context: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Gold {
    pub decision: String,
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub ir: Option<Value>,
    #[serde(default)]
    pub act: Option<String>,
    /// File delivery requested with the question (`opencnpj_query::export::Export`).
    /// Null or absent means the question asks for no file.
    #[serde(default)]
    pub export: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Prediction {
    pub id: String,
    pub decision: String,
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub ir: Option<Value>,
    #[serde(default)]
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct IntentMetrics {
    pub gold: u64,
    pub predicted: u64,
    pub correct: u64,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Metrics {
    pub items: u64,
    pub in_domain: u64,
    pub executed: u64,
    pub correct: u64,
    pub intent_accuracy: f64,
    pub macro_f1: f64,
    pub entity_precision: f64,
    pub entity_recall: f64,
    pub entity_f1: f64,
    pub exact_match: f64,
    pub coverage: f64,
    pub risk: f64,
    pub clarification_recall: f64,
    pub unnecessary_clarification: f64,
    pub ood_rejection: f64,
    pub false_rejection: f64,
    pub negation_items: u64,
    pub negation_errors: u64,
    pub context_items: u64,
    pub context_accuracy: f64,
    pub filter_leaks: u64,
    pub risk_coverage: Vec<(f64, f64)>,
    pub aurc: f64,
    pub ece: f64,
    pub per_intent: BTreeMap<String, IntentMetrics>,
}

pub fn load_items(path: &Path) -> Result<Vec<Item>> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut out = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let value: Value = serde_json::from_str(line).with_context(|| format!("{}", path.display()))?;
        if let Some(turns) = value.get("turns") {
            // A dialogue line expands to one item per turn; later turns see the
            // previous turn's place as their focus.
            let id = value.get("id").and_then(Value::as_str).unwrap_or_default();
            let turns = turns.as_array().cloned().unwrap_or_default();
            let mut previous_place: Option<Value> = None;
            for (index, turn) in turns.iter().enumerate() {
                let text = turn.get("text").and_then(Value::as_str).unwrap_or_default().to_string();
                let gold: Gold = serde_json::from_value(turn.get("gold").cloned().unwrap_or(Value::Null))
                    .with_context(|| format!("{}", path.display()))?;
                let context = previous_place.clone().map(|focus| json!({"focus": focus}));
                previous_place = turn
                    .get("gold")
                    .and_then(|gold| gold.get("ir"))
                    .and_then(|ir| ir.get("scope"))
                    .and_then(|scope| scope.get("place"))
                    .cloned();
                out.push(Item { id: format!("{id}#{}", index + 1), text, gold, meta: Value::Null, rules: Vec::new(), context });
            }
        } else {
            out.push(serde_json::from_value(value).with_context(|| format!("{}", path.display()))?);
        }
    }
    Ok(out)
}

pub fn load_predictions(path: &Path) -> Result<Vec<Prediction>> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).with_context(|| format!("{}", path.display())))
        .collect()
}

/// Trivial baseline: always count establishments with the default active status and no entities.
pub fn baseline(items: &[Item]) -> Vec<Prediction> {
    let request = Request {
        op: Op::Count { metric: Metric::Stock },
        unit: Unit::Establishments,
        scope: Filters { status: Some(StatusSet::only(Status::Active)), ..Filters::default() },
        time: TimeSpec { competence: CURRENT, period: None, anchor: ANCHOR },
        output: Output::default(),
        versions: CatalogueVersions { municipalities: 202610, sectors: 1 },
    };
    let ir = serde_json::to_value(request).expect("baseline IR");
    items
        .iter()
        .map(|item| Prediction {
            id: item.id.clone(),
            decision: "execute".into(),
            intent: Some("count".into()),
            ir: Some(ir.clone()),
            confidence: 0.5,
        })
        .collect()
}

pub fn evaluate(items: &[Item], predictions: &[Prediction]) -> Metrics {
    let by_id: BTreeMap<&str, &Prediction> = predictions.iter().map(|prediction| (prediction.id.as_str(), prediction)).collect();
    let mut metrics = Metrics { items: items.len() as u64, ..Metrics::default() };
    let mut intents: BTreeMap<String, IntentMetrics> = BTreeMap::new();
    let mut gold_entities = 0_u64;
    let mut predicted_entities = 0_u64;
    let mut entity_tp = 0_u64;
    let mut selective: Vec<(f64, bool)> = Vec::new();
    let mut clarified_gold = 0_u64;
    let mut rejected_gold = 0_u64;
    // Items whose gold decision is clarify or reject but that were executed anyway: always
    // wrong answers, and part of the accepted error rate (12-BENCHMARK-PROTOCOL.md §3).
    let mut out_of_policy = 0_u64;
    for item in items {
        let prediction = by_id.get(item.id.as_str());
        let decision = prediction.map_or("missing", |prediction| prediction.decision.as_str());
        match item.gold.decision.as_str() {
            "execute" | "execute_with_note" => {
                metrics.in_domain += 1;
                let gold_intent = item.gold.intent.clone().or_else(|| item.gold.ir.as_ref().and_then(intent_of));
                let predicted_intent = prediction
                    .and_then(|prediction| prediction.intent.clone())
                    .or_else(|| prediction.and_then(|prediction| prediction.ir.as_ref()).and_then(intent_of));
                if let Some(intent) = &gold_intent {
                    let entry = intents.entry(intent.clone()).or_default();
                    entry.gold += 1;
                    if predicted_intent.as_deref() == Some(intent) {
                        entry.correct += 1;
                    }
                }
                if let Some(intent) = &predicted_intent {
                    intents.entry(intent.clone()).or_default().predicted += 1;
                }
                let executed = matches!(decision, "execute" | "execute_with_note");
                if executed {
                    metrics.executed += 1;
                }
                if decision == "clarify" {
                    metrics.unnecessary_clarification += 1.0;
                }
                if decision == "reject" {
                    metrics.false_rejection += 1.0;
                }
                let correct =
                    executed && item.gold.ir.is_some() && prediction.and_then(|prediction| prediction.ir.as_ref()) == item.gold.ir.as_ref();
                if correct {
                    metrics.correct += 1;
                }
                if executed && let Some(prediction) = prediction {
                    selective.push((prediction.confidence, !correct));
                }
                let gold_set = item.gold.ir.as_ref().map(entities).unwrap_or_default();
                let predicted_set = prediction.and_then(|prediction| prediction.ir.as_ref()).map(entities).unwrap_or_default();
                gold_entities += gold_set.len() as u64;
                predicted_entities += predicted_set.len() as u64;
                entity_tp += gold_set.intersection(&predicted_set).count() as u64;
                let gold_negation = item.gold.ir.as_ref().map(negations).unwrap_or_default();
                let predicted_negation = prediction.and_then(|prediction| prediction.ir.as_ref()).map(negations).unwrap_or_default();
                if !gold_negation.is_empty() {
                    metrics.negation_items += 1;
                    if gold_negation != predicted_negation {
                        metrics.negation_errors += 1;
                    }
                }
                if let Some(context) = &item.context {
                    metrics.context_items += 1;
                    if correct {
                        metrics.context_accuracy += 1.0;
                    }
                    if filter_leak(context, item.gold.ir.as_ref(), prediction.and_then(|prediction| prediction.ir.as_ref())) {
                        metrics.filter_leaks += 1;
                    }
                }
            }
            "clarify" | "reject" if matches!(decision, "execute" | "execute_with_note") => {
                out_of_policy += 1;
                if item.gold.decision == "clarify" {
                    clarified_gold += 1;
                } else {
                    rejected_gold += 1;
                }
                if let Some(prediction) = prediction {
                    selective.push((prediction.confidence, true));
                }
            }
            "clarify" => {
                clarified_gold += 1;
                if decision == "clarify" {
                    metrics.clarification_recall += 1.0;
                }
            }
            "reject" => {
                rejected_gold += 1;
                if decision == "reject" {
                    metrics.ood_rejection += 1.0;
                }
            }
            _ => {}
        }
    }
    let in_domain = metrics.in_domain.max(1) as f64;
    metrics.intent_accuracy = metrics.correct as f64 / in_domain;
    metrics.exact_match = metrics.correct as f64 / in_domain;
    metrics.coverage = metrics.executed as f64 / in_domain;
    let executed_all = metrics.executed + out_of_policy;
    metrics.risk = if executed_all == 0 { 0.0 } else { (metrics.executed - metrics.correct + out_of_policy) as f64 / executed_all as f64 };
    metrics.unnecessary_clarification /= in_domain;
    metrics.false_rejection /= in_domain;
    metrics.clarification_recall = if clarified_gold == 0 { 0.0 } else { metrics.clarification_recall / clarified_gold as f64 };
    metrics.ood_rejection = if rejected_gold == 0 { 0.0 } else { metrics.ood_rejection / rejected_gold as f64 };
    metrics.context_accuracy = if metrics.context_items == 0 { 0.0 } else { metrics.context_accuracy / metrics.context_items as f64 };
    metrics.entity_precision = if predicted_entities == 0 { 0.0 } else { entity_tp as f64 / predicted_entities as f64 };
    metrics.entity_recall = if gold_entities == 0 { 0.0 } else { entity_tp as f64 / gold_entities as f64 };
    metrics.entity_f1 = if metrics.entity_precision + metrics.entity_recall == 0.0 {
        0.0
    } else {
        2.0 * metrics.entity_precision * metrics.entity_recall / (metrics.entity_precision + metrics.entity_recall)
    };
    let mut macro_sum = 0.0;
    let mut macro_count = 0;
    for intent in intents.values_mut() {
        intent.precision = if intent.predicted == 0 { 0.0 } else { intent.correct as f64 / intent.predicted as f64 };
        intent.recall = if intent.gold == 0 { 0.0 } else { intent.correct as f64 / intent.gold as f64 };
        intent.f1 = if intent.precision + intent.recall == 0.0 {
            0.0
        } else {
            2.0 * intent.precision * intent.recall / (intent.precision + intent.recall)
        };
        macro_sum += intent.f1;
        macro_count += 1;
    }
    metrics.macro_f1 = if macro_count == 0 { 0.0 } else { macro_sum / macro_count as f64 };
    // Out-of-policy executions are points on the curve, so they belong in its denominator:
    // with only the in-domain count, coverage could exceed 1 (AURC 5/3 on the baseline).
    metrics.risk_coverage = risk_coverage(&selective, metrics.in_domain + out_of_policy);
    metrics.aurc = aurc(&metrics.risk_coverage);
    metrics.ece = ece(&selective);
    metrics.intent_accuracy = if metrics.in_domain == 0 {
        0.0
    } else {
        intents.values().map(|intent| intent.correct).sum::<u64>() as f64 / metrics.in_domain as f64
    };
    metrics.per_intent = intents;
    metrics
}

fn intent_of(ir: &Value) -> Option<String> {
    ir.get("op")?.get("kind")?.as_str().map(str::to_string)
}

fn entities(ir: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if let Some(scope) = ir.get("scope") {
        collect_leaves(scope.get("place"), "place", &mut out);
        collect_leaves(scope.get("activity"), "activity", &mut out);
    }
    if let Some(time) = ir.get("time") {
        collect_leaves(time.get("period"), "period", &mut out);
    }
    if let Some(op) = ir.get("op") {
        collect_leaves(op.get("members"), "member", &mut out);
        if let Some(id) = op.get("id").and_then(Value::as_str) {
            out.insert(format!("lookup:{id}"));
        }
    }
    out
}

fn collect_leaves(value: Option<&Value>, prefix: &str, out: &mut BTreeSet<String>) {
    let Some(value) = value else { return };
    match value {
        Value::Null => {}
        Value::Bool(flag) => {
            out.insert(format!("{prefix}:{flag}"));
        }
        Value::Number(number) => {
            out.insert(format!("{prefix}:{number}"));
        }
        Value::String(text) => {
            if text != "any" && !text.is_empty() {
                out.insert(format!("{prefix}:{text}"));
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_leaves(Some(value), prefix, out);
            }
        }
        Value::Object(map) => {
            for value in map.values() {
                collect_leaves(Some(value), prefix, out);
            }
        }
    }
}

fn negations(ir: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Some(scope) = ir.get("scope") else { return out };
    if let Some(exclude) = scope.get("exclude") {
        collect_leaves(Some(exclude), "exclude", &mut out);
    }
    match scope.get("mei") {
        Some(Value::Bool(false)) => {
            out.insert("mei:no".into());
        }
        Some(Value::String(value)) if value == "no" => {
            out.insert("mei:no".into());
        }
        _ => {}
    }
    match scope.get("head_office") {
        Some(Value::Bool(false)) => {
            out.insert("head_office:no".into());
        }
        Some(Value::String(value)) if value == "no" => {
            out.insert("head_office:no".into());
        }
        _ => {}
    }
    out
}

fn filter_leak(context: &Value, gold: Option<&Value>, predicted: Option<&Value>) -> bool {
    let Some(focus) = context.get("focus") else { return false };
    let focus = entities(&json!({"scope": {"place": focus}}));
    let predicted = predicted.map(entities).unwrap_or_default();
    let gold = gold.map(entities).unwrap_or_default();
    predicted.intersection(&focus).any(|entity| !gold.contains(entity))
}

fn risk_coverage(selective: &[(f64, bool)], total: u64) -> Vec<(f64, f64)> {
    if selective.is_empty() || total == 0 {
        return Vec::new();
    }
    let mut sorted = selective.to_vec();
    sorted.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut errors = 0_usize;
    let mut points = Vec::new();
    for (index, (confidence, error)) in sorted.iter().enumerate() {
        if *error {
            errors += 1;
        }
        // Equal confidences form a single point: only emit at the end of each run.
        let last_in_run = index + 1 == sorted.len() || sorted[index + 1].0 != *confidence;
        if last_in_run {
            points.push(((index + 1) as f64 / total as f64, errors as f64 / (index + 1) as f64));
        }
    }
    points
}

fn aurc(points: &[(f64, f64)]) -> f64 {
    if points.is_empty() {
        return 0.0;
    }
    let mut previous = 0.0;
    let mut area = 0.0;
    for &(coverage, risk) in points {
        area += risk * (coverage - previous);
        previous = coverage;
    }
    area
}

fn ece(selective: &[(f64, bool)]) -> f64 {
    if selective.is_empty() {
        return 0.0;
    }
    let mut bins = [(0_u64, 0.0_f64, 0.0_f64); 10];
    for &(confidence, error) in selective {
        let index = ((confidence.clamp(0.0, 1.0) * 10.0).floor() as usize).min(9);
        bins[index].0 += 1;
        bins[index].1 += confidence;
        bins[index].2 += if error { 0.0 } else { 1.0 };
    }
    let total = selective.len() as f64;
    bins.iter()
        .filter(|bin| bin.0 > 0)
        .map(|bin| {
            let count = bin.0 as f64;
            let confidence = bin.1 / count;
            let accuracy = bin.2 / count;
            (count / total) * (accuracy - confidence).abs()
        })
        .sum()
}

pub fn slice_of(item: &Item) -> Vec<String> {
    let mut slices = Vec::new();
    if let Some(register) = item.meta.get("register").and_then(Value::as_str) {
        slices.push(register.into());
    }
    if let Some(family) = item.meta.get("family").and_then(Value::as_str) {
        if family.contains("compare") {
            slices.push("compound".into());
        }
        if family.contains("dialogue") {
            slices.push("follow_up".into());
        }
    }
    if item.rules.iter().any(|rule| rule == "SR-13") {
        slices.push("negation".into());
    }
    if item.context.is_some() || item.gold.act.as_deref().is_some_and(|act| act != "new_query") {
        slices.push("follow_up".into());
    }
    if item.text.contains('(') {
        slices.push("homonym".into());
    }
    if let Some(noise) = item.meta.get("noise").and_then(Value::as_array)
        && noise.iter().any(|value| matches!(value.as_str(), Some("typo" | "abbrev" | "fragment" | "no_accent")))
    {
        slices.push("informal".into());
    }
    slices.sort();
    slices.dedup();
    slices
}

pub fn evaluate_slices(items: &[Item], predictions: &[Prediction]) -> BTreeMap<String, Metrics> {
    let mut groups: BTreeMap<String, Vec<&Item>> = BTreeMap::new();
    for item in items {
        for slice in slice_of(item) {
            groups.entry(slice).or_default().push(item);
        }
    }
    groups
        .into_iter()
        .map(|(slice, items)| {
            let ids: BTreeSet<&str> = items.iter().map(|item| item.id.as_str()).collect();
            let predictions = predictions.iter().filter(|prediction| ids.contains(prediction.id.as_str())).cloned().collect::<Vec<_>>();
            (slice, evaluate(&items.into_iter().cloned().collect::<Vec<_>>(), &predictions))
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct Environment {
    pub git_commit: String,
    pub rustc: String,
    pub cpu: String,
    pub governor: String,
    pub cores: usize,
    pub load_average: String,
    pub rss_kb: u64,
    pub hwm_kb: u64,
}

pub fn environment() -> Environment {
    let command = |arguments: &[&str]| {
        std::process::Command::new(arguments[0])
            .args(&arguments[1..])
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|value| value.trim().to_string())
    };
    let cpu = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|text| text.lines().find_map(|line| line.strip_prefix("model name\t: ").map(str::to_string)))
        .unwrap_or_else(|| "unknown".into());
    let governor = std::fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
        .map_or_else(|_| "unknown".into(), |value| value.trim().into());
    let load_average = std::fs::read_to_string("/proc/loadavg").map_or_else(|_| "unknown".into(), |value| value.trim().into());
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let read_status = |name: &str| {
        status
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .and_then(|value| value.split_whitespace().next())
            .and_then(|value| value.parse().ok())
            .unwrap_or(0)
    };
    Environment {
        git_commit: command(&["git", "rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into()),
        rustc: command(&["rustc", "-V"]).unwrap_or_else(|| "unknown".into()),
        cpu,
        governor,
        cores: std::thread::available_parallelism().map_or(0, |value| value.get()),
        load_average,
        rss_kb: read_status("VmRSS:"),
        hwm_kb: read_status("VmHWM:"),
    }
}

fn results_markdown(metrics: &Metrics, slices: &BTreeMap<String, Metrics>) -> String {
    let mut out = String::from("# Evaluation\n\n## Overall\n\n| Metric | Value |\n|---|---|\n");
    for (name, value) in [
        ("items", metrics.items as f64),
        ("in_domain", metrics.in_domain as f64),
        ("executed", metrics.executed as f64),
        ("intent_accuracy", metrics.intent_accuracy),
        ("macro_f1", metrics.macro_f1),
        ("entity_precision", metrics.entity_precision),
        ("entity_recall", metrics.entity_recall),
        ("entity_f1", metrics.entity_f1),
        ("exact_match", metrics.exact_match),
        ("coverage", metrics.coverage),
        ("risk", metrics.risk),
        ("clarification_recall", metrics.clarification_recall),
        ("unnecessary_clarification", metrics.unnecessary_clarification),
        ("ood_rejection", metrics.ood_rejection),
        ("false_rejection", metrics.false_rejection),
        ("negation_items", metrics.negation_items as f64),
        ("negation_errors", metrics.negation_errors as f64),
        ("context_items", metrics.context_items as f64),
        ("context_accuracy", metrics.context_accuracy),
        ("filter_leaks", metrics.filter_leaks as f64),
        ("aurc", metrics.aurc),
        ("ece", metrics.ece),
    ] {
        out.push_str(&format!("| {name} | {value:.4} |\n"));
    }
    out.push_str("\n## Per intent\n\n| Intent | Gold | Predicted | Correct | P | R | F1 |\n|---|---|---|---|---|---|---|\n");
    for (intent, row) in &metrics.per_intent {
        out.push_str(&format!(
            "| {intent} | {} | {} | {} | {:.3} | {:.3} | {:.3} |\n",
            row.gold, row.predicted, row.correct, row.precision, row.recall, row.f1
        ));
    }
    out.push_str("\n## Slices\n\n| Slice | Items | Risk | Coverage | Exact |\n|---|---|---|---|---|\n");
    for (slice, row) in slices {
        out.push_str(&format!("| {slice} | {} | {:.4} | {:.4} | {:.4} |\n", row.items, row.risk, row.coverage, row.exact_match));
    }
    out
}

pub fn write_report(
    out: &Path,
    metrics: &Metrics,
    slices: &BTreeMap<String, Metrics>,
    predictions: &[Prediction],
    env: &Environment,
) -> Result<()> {
    fs::create_dir_all(out)?;
    let payload = json!({"metrics": metrics, "slices": slices, "predictions": predictions, "env": env});
    fs::write(out.join("results.json"), serde_json::to_string_pretty(&payload)?)?;
    fs::write(out.join("results.md"), results_markdown(metrics, slices))?;
    fs::write(out.join("env.json"), serde_json::to_string_pretty(env)?)?;
    Ok(())
}

pub fn ensure_milestone(set: &Path, milestone: bool) -> Result<()> {
    // Every path component counts: WP-0.3 stores the set as a directory (`eval/h-blind/`).
    let blind = set.components().any(|part| {
        let part = part.as_os_str().to_string_lossy().to_ascii_lowercase();
        part.contains("h-blind") || part.contains("hblind") || part.contains("h_blind")
    });
    if blind && !milestone {
        bail!("H-blind runs only with --milestone");
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct EvalArgs {
    #[arg(long)]
    pub set: PathBuf,
    #[arg(long)]
    pub predictions: Option<PathBuf>,
    #[arg(long, default_value = "count")]
    pub baseline: String,
    #[arg(long)]
    pub out: PathBuf,
    #[arg(long)]
    pub milestone: bool,
}

pub fn run_eval(args: EvalArgs) -> Result<()> {
    ensure_milestone(&args.set, args.milestone)?;
    let items = load_items(&args.set)?;
    let predictions = match &args.predictions {
        Some(path) => load_predictions(path)?,
        None => {
            ensure!(args.baseline == "count", "only the count baseline is implemented");
            baseline(&items)
        }
    };
    let metrics = evaluate(&items, &predictions);
    let slices = evaluate_slices(&items, &predictions);
    let env = environment();
    write_report(&args.out, &metrics, &slices, &predictions, &env)?;
    println!(
        "{}: items {} in-domain {} executed {} exact {:.3} risk {:.3} coverage {:.3} aurc {:.3} ece {:.3}",
        args.set.display(),
        metrics.items,
        metrics.in_domain,
        metrics.executed,
        metrics.exact_match,
        metrics.risk,
        metrics.coverage,
        metrics.aurc,
        metrics.ece
    );
    Ok(())
}

#[derive(clap::Args)]
pub struct BenchNluArgs {
    #[arg(long)]
    pub set: PathBuf,
    #[arg(long, default_value_t = 20_000)]
    pub iterations: usize,
    #[arg(long, default_value_t = 42)]
    pub seed: u64,
    #[arg(long)]
    pub out: PathBuf,
    #[arg(long, default_value = "count")]
    pub baseline: String,
}

pub fn run_bench_nlu(args: BenchNluArgs) -> Result<()> {
    let items = load_items(&args.set)?;
    ensure!(!items.is_empty(), "empty set");
    ensure!(args.baseline == "count", "only the count baseline is implemented");
    let predictions = baseline(&items);
    let mut rng = SplitMix(args.seed);
    let mut latencies = Vec::with_capacity(args.iterations);
    for index in 0..args.iterations {
        let item = &items[rng.below(items.len())];
        let start = std::time::Instant::now();
        let prediction = predictions.iter().find(|prediction| prediction.id == item.id).expect("baseline prediction");
        std::hint::black_box(prediction.ir.as_ref());
        let elapsed = start.elapsed();
        if index >= 10 {
            latencies.push(elapsed.as_nanos() as u64);
        }
    }
    latencies.sort_unstable();
    let percentile = |p: f64| latencies[((latencies.len() as f64 - 1.0) * p).round() as usize];
    let result = json!({
        "set": args.set,
        "iterations": args.iterations,
        "variant": "baseline-count",
        "p50_ns": percentile(0.50),
        "p95_ns": percentile(0.95),
        "p99_ns": percentile(0.99),
        "max_ns": latencies.last().copied().unwrap_or(0),
        "env": environment(),
    });
    fs::create_dir_all(&args.out)?;
    fs::write(args.out.join("bench-nlu.json"), serde_json::to_string_pretty(&result)?)?;
    println!("bench-nlu: p50 {} ns, p95 {} ns, p99 {} ns", percentile(0.50), percentile(0.95), percentile(0.99));
    Ok(())
}

struct SplitMix(u64);

impl SplitMix {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^= value >> 31;
        if n == 0 { 0 } else { usize::try_from(value % n as u64).unwrap_or(0) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, decision: &str, intent: &str, ir: Option<Value>) -> Item {
        Item {
            id: id.into(),
            text: String::new(),
            gold: Gold { decision: decision.into(), intent: Some(intent.into()), ir, act: Some("new_query".into()), export: None },
            meta: Value::Null,
            rules: Vec::new(),
            context: None,
        }
    }

    fn ir(place: &str, sector: &str) -> Value {
        json!({"op": {"kind": "count", "metric": "stock"}, "scope": {"place": {"municipalities": [place]}, "activity": {"sectors": {"ids": [sector]}}}})
    }

    fn prediction(id: &str, intent: &str, ir: Option<Value>, confidence: f64) -> Prediction {
        Prediction { id: id.into(), decision: "execute".into(), intent: Some(intent.into()), ir, confidence }
    }

    #[test]
    fn executing_clarify_or_reject_items_counts_as_error() {
        // Always-execute on out-of-domain items used to report risk 0 (executed 0).
        let items = vec![
            item("1", "reject", "count", None),
            item("2", "clarify", "count", None),
            item("3", "execute", "count", Some(ir("A", "S"))),
        ];
        let predictions = vec![
            prediction("1", "count", Some(ir("A", "S")), 0.9),
            prediction("2", "count", Some(ir("A", "S")), 0.9),
            prediction("3", "count", Some(ir("A", "S")), 0.9),
        ];
        let metrics = evaluate(&items, &predictions);
        assert!((metrics.risk - 2.0 / 3.0).abs() < 1e-9, "{}", metrics.risk);
        assert_eq!(metrics.ood_rejection, 0.0);
        assert_eq!(metrics.clarification_recall, 0.0);
    }

    #[test]
    fn equal_confidences_form_a_single_point() {
        let points = risk_coverage(&[(0.9, false), (0.9, true), (0.5, false)], 3);
        assert_eq!(points, vec![(2.0 / 3.0, 0.5), (1.0, 1.0 / 3.0)]);
    }

    #[test]
    fn dialogue_lines_expand_to_one_item_per_turn() {
        use std::io::Write as _;
        let line = json!({
            "id": "x",
            "turns": [
                {"text": "first", "gold": {"decision": "execute", "intent": "count", "ir": {"scope": {"place": {"municipalities": [1]}}}, "act": "new_query"}},
                {"text": "second", "gold": {"decision": "execute", "intent": "count", "ir": null, "act": "new_query"}},
            ],
        });
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "{}", serde_json::to_string(&line).unwrap()).unwrap();
        let items = load_items(file.path()).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "x#1");
        assert_eq!(items[1].id, "x#2");
        assert!(items[0].context.is_none());
        assert_eq!(items[1].context, Some(json!({"focus": {"municipalities": [1]}})));
    }

    #[test]
    fn metrics_match_hand_computed_example() {
        let items = vec![
            item("1", "execute", "count", Some(ir("A", "S"))),
            item("2", "execute", "count", Some(ir("B", "S"))),
            item("3", "execute", "list", Some(ir("C", "S"))),
            item("4", "clarify", "count", None),
            item("5", "reject", "count", None),
        ];
        let predictions = vec![
            prediction("1", "count", Some(ir("A", "S")), 0.9),
            prediction("2", "count", Some(json!({"op": {"kind": "count", "metric": "stock"}})), 0.8),
            prediction("3", "list", Some(ir("C", "S")), 0.7),
            Prediction { id: "4".into(), decision: "clarify".into(), intent: None, ir: None, confidence: 0.0 },
            Prediction { id: "5".into(), decision: "reject".into(), intent: None, ir: None, confidence: 0.0 },
        ];
        let metrics = evaluate(&items, &predictions);
        assert_eq!(metrics.in_domain, 3);
        assert_eq!(metrics.executed, 3);
        assert_eq!(metrics.correct, 2);
        assert!((metrics.exact_match - 2.0 / 3.0).abs() < 1e-9);
        assert!((metrics.risk - 1.0 / 3.0).abs() < 1e-9);
        assert!((metrics.coverage - 1.0).abs() < 1e-9);
        assert!((metrics.intent_accuracy - 1.0).abs() < 1e-9);
        assert!((metrics.macro_f1 - 1.0).abs() < 1e-9);
        assert!((metrics.entity_precision - 1.0).abs() < 1e-9);
        assert!((metrics.entity_recall - 4.0 / 6.0).abs() < 1e-9);
        assert!((metrics.entity_f1 - 0.8).abs() < 1e-9);
        assert!((metrics.clarification_recall - 1.0).abs() < 1e-9);
        assert_eq!(metrics.unnecessary_clarification, 0.0);
        assert!((metrics.ood_rejection - 1.0).abs() < 1e-9);
        assert_eq!(metrics.false_rejection, 0.0);
        assert_eq!(metrics.risk_coverage, vec![(1.0 / 3.0, 0.0), (2.0 / 3.0, 0.5), (1.0, 1.0 / 3.0)]);
        assert!((metrics.aurc - 5.0 / 18.0).abs() < 1e-9);
        assert!((metrics.ece - 0.4).abs() < 1e-9);
    }

    #[test]
    fn baseline_produces_expected_numbers() {
        let items = vec![
            item("1", "execute", "count", Some(ir("A", "S"))),
            item("2", "execute", "list", Some(ir("B", "S"))),
            item("3", "execute", "count", None),
            item("4", "clarify", "count", None),
            item("5", "reject", "count", None),
        ];
        let predictions = baseline(&items);
        let metrics = evaluate(&items, &predictions);
        assert_eq!(metrics.executed, 3);
        assert_eq!(metrics.correct, 0);
        assert!((metrics.exact_match - 0.0).abs() < 1e-9);
        assert!((metrics.risk - 1.0).abs() < 1e-9);
        assert!((metrics.coverage - 1.0).abs() < 1e-9);
        assert!((metrics.intent_accuracy - 2.0 / 3.0).abs() < 1e-9);
        assert!((metrics.entity_precision - 0.0).abs() < 1e-9);
        assert!((metrics.entity_recall - 0.0).abs() < 1e-9);
        assert_eq!(metrics.clarification_recall, 0.0);
        assert_eq!(metrics.ood_rejection, 0.0);
        // All five predictions share confidence 0.5 and all are wrong (two of them execute
        // clarify/reject items): one point at full coverage with risk 1.
        assert_eq!(metrics.risk_coverage, vec![(1.0, 1.0)]);
        assert!((metrics.aurc - 1.0).abs() < 1e-9);
        assert!((metrics.ece - 0.5).abs() < 1e-9);
    }

    #[test]
    fn milestone_gates_h_blind() {
        let path = Path::new("/tmp/h-blind.jsonl");
        assert!(ensure_milestone(path, false).is_err());
        assert!(ensure_milestone(path, true).is_ok());
        assert!(ensure_milestone(Path::new("h-dev.jsonl"), false).is_ok());
        assert!(ensure_milestone(Path::new("/data/eval/h-blind/items.jsonl"), false).is_err());
    }

    #[test]
    fn negation_and_context_are_counted() {
        let mut negated = item("1", "execute", "count", Some(json!({"op":{"kind":"count","metric":"stock"},"scope":{"mei":false}})));
        negated.rules = vec!["SR-13".into()];
        let predictions = vec![prediction("1", "count", Some(json!({"op":{"kind":"count","metric":"stock"},"scope":{"mei":"any"}})), 0.5)];
        let metrics = evaluate(&[negated], &predictions);
        assert_eq!(metrics.negation_items, 1);
        assert_eq!(metrics.negation_errors, 1);
    }
}

#[cfg(test)]
mod sets {
    use std::collections::{BTreeMap, BTreeSet};

    use opencnpj_insights::agg;
    use opencnpj_query::validate::{self, Capabilities};
    use serde::Deserialize;
    use sha2::{Digest, Sha256};

    use super::*;

    const ADV: &str = include_str!("../eval/adv.jsonl");
    const OOD: &str = include_str!("../eval/ood.jsonl");
    const INFORMAL: &str = include_str!("../eval/h-informal.jsonl");
    const DIALOGUES: &str = include_str!("../eval/dialogues.jsonl");
    const CARD: &str = include_str!("../eval/DATASET-CARD.md");
    /// Frozen files whose SHA-256 the dataset card records, in card order.
    const FROZEN: [(&str, &str); 4] =
        [("adv.jsonl", ADV), ("ood.jsonl", OOD), ("h-informal.jsonl", INFORMAL), ("dialogues.jsonl", DIALOGUES)];

    /// The committed catalogues the sets are labelled against (no generated copy of them).
    struct Catalogue {
        municipality_ufs: BTreeMap<MunicipalityId, UfId>,
        sector_cnaes: BTreeMap<SectorId, SmallSet<CnaeCode>>,
        sector_count: u16,
    }

    fn catalogue() -> Catalogue {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../opencnpj-insights/catalog");
        let municipalities =
            opencnpj_insights::catalog::municipalities::MunicipalityCatalogue::load(&root.join("municipalities.tsv")).expect("catalogue");
        let municipality_ufs = municipalities
            .rows()
            .iter()
            .filter_map(|row| match &row.place {
                opencnpj_insights::catalog::municipalities::Place::Municipality(geo) => {
                    agg::uf_id(&geo.uf).map(|uf| (MunicipalityId(row.id), uf))
                }
                opencnpj_insights::catalog::municipalities::Place::Abroad => None,
            })
            .collect();
        let sectors = crate::query::load_sectors(&root.join("sectors.toml")).expect("sectors");
        let sector_count = u16::try_from(sectors.len()).expect("sector count");
        let sector_cnaes =
            sectors.into_iter().map(|(id, codes)| (SectorId(id), SmallSet::new(codes.into_iter().map(CnaeCode::Subclass)))).collect();
        Catalogue { municipality_ufs, sector_cnaes, sector_count }
    }

    #[derive(Deserialize)]
    struct Dialogue {
        id: String,
        turns: Vec<DialogueTurn>,
    }

    #[derive(Deserialize)]
    struct DialogueTurn {
        gold: Gold,
    }

    fn parse<T: serde::de::DeserializeOwned>(text: &str) -> Vec<T> {
        text.lines().filter(|line| !line.trim().is_empty()).map(|line| serde_json::from_str(line).expect("set line")).collect()
    }

    fn sha(text: &str) -> String {
        let digest = Sha256::digest(text.as_bytes());
        digest.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn validation_caps(catalogue: &Catalogue) -> Capabilities {
        let mut competences = Vec::new();
        let (mut year, mut month) = (2024, 11);
        for _ in 0..24 {
            competences.push(Competence::from_year_month(year, month).unwrap());
            month += 1;
            if month == 13 {
                month = 1;
                year += 1;
            }
        }
        Capabilities {
            current: Competence(321),
            competences,
            flow_days: Day(1)..=Day(46305),
            populations: vec![PopulationRef::IbgeCensus(2022), PopulationRef::IbgeEstimate(2026)],
            catalogue_versions: CatalogueVersions { municipalities: 202610, sectors: 1 },
            sector_count: catalogue.sector_count,
            geography: true,
            list_limit: 100,
        }
    }

    fn validate_ir(item: &Item, context: &CanonicalContext<'_>, caps: &Capabilities) {
        let ir = item.gold.ir.as_ref().unwrap_or_else(|| panic!("{} lacks the executable IR", item.id));
        let request: Request = serde_json::from_value(ir.clone()).unwrap_or_else(|error| panic!("{} IR: {error}", item.id));
        let canonical = request.canonicalize(context).unwrap_or_else(|error| panic!("{} canonical: {error:?}", item.id)).0;
        // A gold that spells out a sector's CNAEs must use exactly the catalogue's core set
        // (the padaria gold once carried an extended code and inverted a minimal pair).
        if let (ActivityFilter::Sectors { cnaes: gold, .. }, ActivityFilter::Sectors { cnaes: catalogue, .. }) =
            (&request.scope.activity, &canonical.scope.activity)
            && !gold.is_empty()
        {
            assert_eq!(gold, catalogue, "{} sector CNAEs differ from the catalogue core", item.id);
        }
        // "Abriram em 2025" counts registrations of any current status (SR-02); 68 golds once
        // filtered them to active ones.
        if matches!(request.op.metric(), Some(Metric::Flow(FlowMetric::Registrations | FlowMetric::NetRegistrations))) {
            assert!(request.scope.status.is_none_or(|status| status == StatusSet::ALL), "{} registrations count every status", item.id);
        }
        validate::validate(canonical, caps).unwrap_or_else(|issues| panic!("{} invalid: {issues:?}", item.id));
        if let Some(intent) = &item.gold.intent {
            assert_eq!(intent_of(ir).as_deref(), Some(intent.as_str()), "{} intent", item.id);
        }
    }

    #[test]
    fn frozen_sets_validate_and_cover_rules() {
        let catalogue = catalogue();
        let context = CanonicalContext {
            first_day: Day(1),
            last_day: Day(46305),
            municipality_ufs: &catalogue.municipality_ufs,
            sector_cnaes: &catalogue.sector_cnaes,
        };
        let caps = validation_caps(&catalogue);
        let mut ids = BTreeSet::new();
        for (name, text) in [("adv", ADV), ("ood", OOD), ("informal", INFORMAL)] {
            let items: Vec<Item> = parse(text);
            for item in &items {
                assert!(ids.insert(item.id.clone()), "duplicate {}", item.id);
                match item.gold.decision.as_str() {
                    "execute" | "execute_with_note" => validate_ir(item, &context, &caps),
                    "clarify" | "reject" => assert!(item.gold.ir.is_none(), "{} must not carry an IR", item.id),
                    other => panic!("{} decision {other}", item.id),
                }
            }
            if name == "ood" {
                assert!(items.iter().all(|item| item.gold.decision == "reject"), "OOD must reject");
            }
        }

        let dialogues: Vec<Dialogue> = parse(DIALOGUES);
        for dialogue in &dialogues {
            assert!(dialogue.turns.len() >= 2, "{} turns", dialogue.id);
            for turn in &dialogue.turns {
                if turn.gold.decision == "execute" || turn.gold.decision == "execute_with_note" {
                    let item = Item {
                        id: dialogue.id.clone(),
                        text: String::new(),
                        gold: turn.gold.clone(),
                        meta: serde_json::Value::Null,
                        rules: Vec::new(),
                        context: None,
                    };
                    validate_ir(&item, &context, &caps);
                }
            }
        }

        // The sets are edited by hand now (no generator script): their card rows are
        // refreshed with `UPDATE_CARD=1 cargo test -p opencnpj-lab frozen_sets`.
        let rows: Vec<(&str, String)> = FROZEN
            .iter()
            .map(|(name, text)| {
                (*name, format!("| {name} | {} | {} |", text.lines().filter(|line| !line.trim().is_empty()).count(), sha(text)))
            })
            .collect();
        if std::env::var_os("UPDATE_CARD").is_some() {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("eval/DATASET-CARD.md");
            let card = std::fs::read_to_string(&path).expect("dataset card");
            let updated: Vec<String> = card
                .lines()
                .map(|line| {
                    rows.iter()
                        .find(|(name, _)| line.starts_with(&format!("| {name} |")))
                        .map_or_else(|| line.to_string(), |(_, row)| row.clone())
                })
                .collect();
            std::fs::write(&path, updated.join("\n") + "\n").expect("write dataset card");
        }
        for (name, row) in &rows {
            assert!(CARD.lines().any(|line| line == row), "{name}: dataset card row is out of date (run with UPDATE_CARD=1)");
        }
    }

    #[test]
    #[ignore = "WP-3.3 sizes and coverage met on 2026-11-07, but the sets are AI-drafted; run with --include-ignored until people rewrite them"]
    fn wp_3_3_acceptance() {
        let mut rules: BTreeMap<String, usize> = BTreeMap::new();
        let mut traps: BTreeMap<String, usize> = BTreeMap::new();
        for (name, text, minimum) in [("adv", ADV, 300), ("ood", OOD, 400), ("informal", INFORMAL, 200)] {
            let items: Vec<Item> = parse(text);
            assert!(items.len() >= minimum, "{name} size {}", items.len());
            for item in &items {
                for rule in &item.rules {
                    *rules.entry(rule.clone()).or_default() += 1;
                }
                if let Some(trap) = item.meta.get("trap").and_then(serde_json::Value::as_str) {
                    *traps.entry(trap.into()).or_default() += 1;
                }
            }
        }
        for rule in (1..=14).map(|index| format!("SR-{index:02}")) {
            assert!(rules.get(&rule).copied().unwrap_or(0) >= 5, "rule {rule} coverage");
        }
        for trap in ["abertas", "menos", "grandes", "fecharam", "crescendo", "mais"] {
            assert!(traps.get(trap).copied().unwrap_or(0) >= 5, "trap {trap} coverage");
        }
        let dialogues: Vec<Dialogue> = parse(DIALOGUES);
        assert!(dialogues.len() >= 60, "dialogues {}", dialogues.len());
    }
}
