//! Data contracts of assistant answers, checked against the records behind them.

use crate::contracts::{Finding, RecordScope, finding_of as finding, record_matches};
use opencnpj::assistant::Outcome;
pub(crate) use opencnpj::assistant::{Assistant, Options};
use opencnpj_insights::{
    agg,
    execute::{ExecContext, Value},
    plan::{self, Backend},
    render::Block,
};
use opencnpj_nlu::{context::Conversation, policy::Interpretation};
use opencnpj_query::ir::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

/// Data contracts of one interpretation (see `contracts`): counts agree with the records
/// behind them, every record satisfies the query, a file holds the rows it promises, the
/// sentence states the number of the tile, and a company search returns the searched name.
pub(crate) fn data_findings(assistant: &Assistant, interpretation: &Interpretation) -> Vec<Finding> {
    let mut out = Vec::new();
    let context = ExecContext::new(&assistant.assets().catalogue, &assistant.assets().sectors);
    let Some(engine) = assistant.engine(&context) else { return out };
    if let Some(search) = &interpretation.company {
        let reply = assistant.ask_company(search, interpretation, &Conversation::default(), Instant::now(), BTreeMap::new());
        let approximate = reply["answer"]["blocks"].as_array().is_some_and(|b| b.iter().any(|b| b["type"] == "notice"));
        let tokens: Vec<String> = opencnpj_nlu::normalize::words(&search.name);
        for block in reply["answer"]["blocks"].as_array().into_iter().flatten().filter(|b| b["type"] == "records") {
            for row in block["rows"].as_array().into_iter().flatten() {
                let names = opencnpj_nlu::normalize::words(&format!("{} {}", row[1].as_str().unwrap_or(""), row[2].as_str().unwrap_or("")));
                if !approximate && !tokens.iter().all(|t| names.iter().any(|n| n.starts_with(t.as_str()))) {
                    out.push(finding("company_result_without_name", format!("{} {}", row[0], row[1])));
                }
            }
        }
        return out;
    }
    let Some(request) = &interpretation.request else { return out };
    let outcome = match assistant.run(request) {
        Ok(outcome) => outcome,
        Err(error) => return vec![finding("execution_error", error.to_string())],
    };
    let codes = |ids: &[MunicipalityId]| -> BTreeSet<String> {
        ids.iter().filter_map(|id| assistant.assets().catalogue.get(id.0)).map(|row| format!("{:04}", row.tom)).collect()
    };
    let ufs = |ids: &[UfId]| -> BTreeSet<String> { ids.iter().filter_map(|id| agg::uf_code(*id)).map(str::to_owned).collect() };
    match (&request.op, &outcome) {
        (Op::Count { metric }, Outcome::Value(Value::Scalar { value })) => {
            // D4: the sentence states the number of the tile.
            if let Ok(answer) = assistant.render(request, &outcome, interpretation) {
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
                && let Ok(valid) = assistant.assets().canonical(list)
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
            if let Ok(records) = assistant.decode(docs) {
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
                let promised = assistant.export_offer(request, export, &outcome)["rows"].as_u64().unwrap_or(0);
                match assistant.export(base.clone(), export, None) {
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
