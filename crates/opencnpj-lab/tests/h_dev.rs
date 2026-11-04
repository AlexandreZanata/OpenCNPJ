#[path = "support/annotation.rs"]
mod annotation;

use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fs::File,
    io::{BufRead, BufReader},
};

use annotation::{Act, Decision, Flow, Intent, Item, MetricKind, Origin, Request, Rule};

const DEFAULT_PATH: &str = "/data/opencnpj-data/intelligence/eval/h-dev.jsonl";

#[test]
#[ignore = "requires the local H-dev JSONL artifact"]
fn h_dev_matches_annotation_ir() {
    let path = env::var("OPENCNPJ_H_DEV_PATH").unwrap_or_else(|_| DEFAULT_PATH.into());
    let file = File::open(&path).unwrap_or_else(|err| panic!("cannot open {path}: {err}"));
    let items = BufReader::new(file)
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let line = line.unwrap_or_else(|err| panic!("{path}:{}: {err}", index + 1));
            serde_json::from_str::<Item>(&line).unwrap_or_else(|err| panic!("{path}:{}: {err}", index + 1))
        })
        .collect::<Vec<_>>();

    let mut by_id = BTreeMap::new();
    let mut inputs = BTreeSet::new();
    let mut decisions = BTreeMap::new();
    let mut intents = BTreeMap::new();
    let mut acts = BTreeSet::new();
    let mut metrics = BTreeSet::new();
    let mut flows = BTreeSet::new();
    let mut rules = BTreeSet::new();
    let mut ai_drafts = 0;
    for item in &items {
        validate_item(item, &by_id);
        assert!(by_id.insert(item.id.as_str(), item).is_none(), "duplicate ID {}", item.id);
        let context = item.context.as_ref().map(|ctx| ctx.previous.as_str());
        assert!(inputs.insert((context, item.text.trim())), "{} repeats an input with the same context", item.id);
        *decisions.entry(&item.gold.decision).or_insert(0) += 1;
        acts.insert(&item.gold.act);
        rules.extend(&item.rules);
        ai_drafts += usize::from(matches!(item.origin, Origin::AiDraft));
        if let Some(ir) = &item.gold.ir {
            *intents.entry(ir.op.intent()).or_insert(0) += 1;
            if let Some((metric, flow)) = ir.op.metric() {
                metrics.insert(metric);
                flows.extend(flow);
            }
        }
    }

    println!("items: {} (AI drafts: {ai_drafts})", items.len());
    println!("executed by intent: {intents:?}");
    println!("decisions: {decisions:?}");
    println!("acts: {acts:?}");
    println!("metrics: {metrics:?}, flows: {flows:?}");
    println!("semantic rules: {}", rules.len());

    assert!(items.len() >= 300, "H-dev needs at least 300 items");
    for intent in Intent::ALL {
        assert!(intents.get(intent).is_some_and(|count| *count >= 15), "fewer than 15 executed {intent:?} items");
    }
    for act in Act::ALL {
        assert!(acts.contains(act), "missing dialogue act {act:?}");
    }
    for metric in MetricKind::ALL {
        assert!(metrics.contains(metric), "missing metric {metric:?}");
    }
    for flow in Flow::ALL {
        assert!(flows.contains(flow), "missing flow {flow:?}");
    }
    assert_eq!(rules.len(), Rule::COUNT, "missing semantic rule tags");
    assert_eq!(decisions.len(), 4, "H-dev must cover all policy outcomes");
}

fn validate_item(item: &Item, by_id: &BTreeMap<&str, &Item>) {
    let id = &item.id;
    assert!(!item.text.trim().is_empty() && item.text.chars().count() <= 500, "{id} invalid utterance length");
    let previous = item.context.as_ref().map(|context| {
        let previous = by_id.get(context.previous.as_str()).unwrap_or_else(|| panic!("{id} must reference an earlier item"));
        assert!(context.focus.len() <= 4 && context.result.len() <= 10, "{id} context exceeds bounded state");
        previous
    });
    let gold = &item.gold;
    match gold.act {
        Act::AnswerClarification => {
            assert!(previous.is_some_and(|item| item.gold.decision == Decision::Clarify), "{id} needs a clarification")
        }
        Act::ReplaceSlot | Act::AddFilter | Act::RemoveFilter | Act::CompareWithFocus | Act::ReferToResult | Act::Correction
            if gold.decision != Decision::Clarify =>
        {
            assert!(previous.is_some_and(|item| item.gold.ir.is_some()), "{id} needs a previous request");
        }
        _ => {}
    }
    if gold.act == Act::CompareWithFocus && gold.ir.is_some() {
        assert!(item.context.as_ref().is_some_and(|ctx| (2..=4).contains(&ctx.focus.len())), "{id} needs focused members");
    }
    if gold.act == Act::ReferToResult && gold.ir.is_some() {
        assert!(item.context.as_ref().is_some_and(|ctx| !ctx.result.is_empty()), "{id} needs previous result members");
    }

    let validate = |request: &Request| {
        request.validate(&item.capabilities).unwrap_or_else(|err| panic!("{id}: {err}"));
    };
    if gold.decision != Decision::Execute {
        assert!(gold.reason.as_ref().is_some_and(|reason| !reason.trim().is_empty()), "{id} needs a rationale");
    }
    match gold.decision {
        Decision::Execute | Decision::ExecuteWithNote => {
            let ir = gold.ir.as_ref().unwrap_or_else(|| panic!("{id} needs an executable IR"));
            assert_eq!(gold.intent.as_ref(), Some(&ir.op.intent()), "{id} intent differs from the IR operation");
            assert!(gold.options.is_empty(), "{id} cannot both execute and offer options");
            validate(ir);
        }
        Decision::Clarify => {
            assert!(gold.ir.is_none() && gold.options.len() <= 4, "{id} must ask a bounded clarification");
            gold.options.iter().for_each(validate);
        }
        Decision::Reject => assert!(gold.ir.is_none() && gold.options.is_empty(), "{id} rejection must have no executable IR"),
    }
}

#[test]
fn annotation_ir_rejects_invalid_filters_and_metrics() {
    let base = serde_json::json!({
        "op": {"kind":"count", "metric":"stock"},
        "unit":"establishments", "scope":{"status":["02"]},
        "time":{"competence":"2026-10", "anchor":"2026-10-11"},
        "output":{"page":1, "with_methodology":false}
    });
    let check = |value| serde_json::from_value::<Request>(value).is_ok_and(|ir| ir.validate(&Default::default()).is_ok());
    assert!(check(base.clone()));
    let mut ir = base.clone();
    ir["scope"]["status"] = serde_json::json!(["02", "02"]);
    assert!(!check(ir));
    let mut ir = base.clone();
    ir["op"]["metric"] = serde_json::json!({"flow":"registrations"});
    assert!(!check(ir));
    let mut ir = base.clone();
    ir["scope"]["ignore_negation"] = serde_json::json!(true);
    assert!(!check(ir));
    let mut ir = base;
    ir["op"] = serde_json::json!({"kind":"lookup", "id":"33000167000102"});
    assert!(!check(ir));
}
