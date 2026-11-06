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
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).with_context(|| format!("{}", path.display())))
        .collect()
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
    metrics.risk = if metrics.executed == 0 { 0.0 } else { (metrics.executed - metrics.correct) as f64 / metrics.executed as f64 };
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
    metrics.risk_coverage = risk_coverage(&selective, metrics.in_domain);
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
    for (index, (_, error)) in sorted.iter().enumerate() {
        if *error {
            errors += 1;
        }
        points.push(((index + 1) as f64 / total as f64, errors as f64 / (index + 1) as f64));
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
    if previous == 0.0 { 0.0 } else { area / previous }
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
    let name = set.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_ascii_lowercase();
    if (name.contains("h-blind") || name.contains("hblind")) && !milestone {
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
            gold: Gold { decision: decision.into(), intent: Some(intent.into()), ir, act: Some("new_query".into()) },
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
        assert!((metrics.aurc - 1.0).abs() < 1e-9);
        assert!((metrics.ece - 0.5).abs() < 1e-9);
    }

    #[test]
    fn milestone_gates_h_blind() {
        let path = Path::new("/tmp/h-blind.jsonl");
        assert!(ensure_milestone(path, false).is_err());
        assert!(ensure_milestone(path, true).is_ok());
        assert!(ensure_milestone(Path::new("h-dev.jsonl"), false).is_ok());
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
    const CONTEXT: &str = include_str!("../eval/context.json");
    const CARD: &str = include_str!("../eval/DATASET-CARD.md");

    #[derive(Deserialize)]
    struct ContextFile {
        municipalities: BTreeMap<u16, String>,
        sectors: BTreeMap<u16, Vec<u32>>,
        sector_count: u16,
        catalogue_versions: CatalogueVersions,
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

    fn validation_caps(context: &ContextFile) -> Capabilities {
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
            populations: vec![PopulationRef::IbgeCensus(2022), PopulationRef::IbgeEstimate(2025)],
            catalogue_versions: context.catalogue_versions,
            sector_count: context.sector_count,
            geography: true,
            list_limit: 100,
        }
    }

    fn validate_ir(item: &Item, context: &CanonicalContext<'_>, caps: &Capabilities) {
        let ir = item.gold.ir.as_ref().unwrap_or_else(|| panic!("{} lacks the executable IR", item.id));
        let request: Request = serde_json::from_value(ir.clone()).unwrap_or_else(|error| panic!("{} IR: {error}", item.id));
        let canonical = request.canonicalize(context).unwrap_or_else(|error| panic!("{} canonical: {error:?}", item.id)).0;
        validate::validate(canonical, caps).unwrap_or_else(|issues| panic!("{} invalid: {issues:?}", item.id));
        if let Some(intent) = &item.gold.intent {
            assert_eq!(intent_of(ir).as_deref(), Some(intent.as_str()), "{} intent", item.id);
        }
    }

    #[test]
    fn frozen_sets_validate_and_cover_rules() {
        let context_file: ContextFile = serde_json::from_str(CONTEXT).expect("context");
        let municipality_ufs = context_file
            .municipalities
            .iter()
            .filter_map(|(id, uf)| agg::uf_id(uf).map(|uf| (MunicipalityId(*id), uf)))
            .collect::<BTreeMap<_, _>>();
        let sector_cnaes = context_file
            .sectors
            .iter()
            .map(|(id, cnaes)| (SectorId(*id), SmallSet::new(cnaes.iter().map(|code| CnaeCode::Subclass(*code)))))
            .collect::<BTreeMap<_, _>>();
        let context =
            CanonicalContext { first_day: Day(1), last_day: Day(46305), municipality_ufs: &municipality_ufs, sector_cnaes: &sector_cnaes };
        let caps = validation_caps(&context_file);
        let mut rules: BTreeMap<String, usize> = BTreeMap::new();
        let mut traps: BTreeMap<String, usize> = BTreeMap::new();
        let mut ids = BTreeSet::new();
        for (name, text, minimum) in [("adv", ADV, 300), ("ood", OOD, 400), ("informal", INFORMAL, 200)] {
            let items: Vec<Item> = parse(text);
            assert!(items.len() >= minimum, "{name} size {}", items.len());
            for item in &items {
                assert!(ids.insert(item.id.clone()), "duplicate {}", item.id);
                match item.gold.decision.as_str() {
                    "execute" | "execute_with_note" => validate_ir(item, &context, &caps),
                    "clarify" | "reject" => assert!(item.gold.ir.is_none(), "{} must not carry an IR", item.id),
                    other => panic!("{} decision {other}", item.id),
                }
                for rule in &item.rules {
                    *rules.entry(rule.clone()).or_default() += 1;
                }
                if let Some(trap) = item.meta.get("trap").and_then(serde_json::Value::as_str) {
                    *traps.entry(trap.into()).or_default() += 1;
                }
            }
            if name == "ood" {
                assert!(items.iter().all(|item| item.gold.decision == "reject"), "OOD must reject");
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

        for (name, text) in [
            ("adv.jsonl", ADV),
            ("ood.jsonl", OOD),
            ("h-informal.jsonl", INFORMAL),
            ("dialogues.jsonl", DIALOGUES),
            ("context.json", CONTEXT),
        ] {
            let digest = sha(text);
            assert!(CARD.contains(&digest), "{name} SHA-256 not in the dataset card");
        }
    }
}
