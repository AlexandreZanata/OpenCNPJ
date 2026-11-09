//! Contracts: invariants every answer must keep, checked on evaluation sets and real data.
//!
//! A contract finding is a concrete inconsistency (a named place missing from the query, a
//! record outside the requested activity), never a style preference. Findings carry a stable
//! code so evaluation runs can be compared over time.

use opencnpj_nlu::{
    policy::{Decision, Interpretation},
    spotter::Entity,
};
use opencnpj_query::ir::*;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(crate) struct Finding {
    pub code: &'static str,
    pub detail: String,
}

pub(crate) fn finding_of(code: &'static str, detail: impl Into<String>) -> Finding {
    finding(code, detail)
}

fn finding(code: &'static str, detail: impl Into<String>) -> Finding {
    Finding { code, detail: detail.into() }
}

/// Places and sectors a request mentions anywhere: filters, exclusions and members.
#[derive(Default)]
struct Mentions {
    municipalities: BTreeSet<MunicipalityId>,
    ufs: BTreeSet<UfId>,
    regions: BTreeSet<RegionId>,
    sectors: BTreeSet<SectorId>,
}

fn mentions(request: &Request, municipality_ufs: &BTreeMap<MunicipalityId, UfId>) -> Mentions {
    let mut m = Mentions::default();
    let scope = &request.scope;
    match &scope.place {
        PlaceFilter::Municipalities(ids) => m.municipalities.extend(ids.iter().copied()),
        PlaceFilter::Ufs(ids) => m.ufs.extend(ids.iter().copied()),
        PlaceFilter::Region(id) => {
            m.regions.insert(*id);
        }
        PlaceFilter::Near { center, .. } => {
            m.municipalities.insert(*center);
        }
        PlaceFilter::Any => {}
    }
    m.ufs.extend(scope.within_ufs.iter().copied());
    m.municipalities.extend(scope.exclude.municipalities.iter().copied());
    m.ufs.extend(scope.exclude.ufs.iter().copied());
    m.sectors.extend(scope.exclude.sectors.iter().copied());
    if let ActivityFilter::Sectors { ids, .. } = &scope.activity {
        m.sectors.extend(ids.iter().copied());
    }
    if let Op::Compare { members, .. } = &request.op {
        for member in members.iter() {
            match member {
                Member::Municipality(id) => {
                    m.municipalities.insert(*id);
                }
                Member::Uf(id) => {
                    m.ufs.insert(*id);
                }
                Member::Region(id) => {
                    m.regions.insert(*id);
                }
                Member::Sector(id) => {
                    m.sectors.insert(*id);
                }
            }
        }
    }
    // A city's state is named by the city ("Sorriso/MT").
    let city_ufs: Vec<UfId> = m.municipalities.iter().filter_map(|id| municipality_ufs.get(id)).copied().collect();
    m.ufs.extend(city_ufs);
    m
}

/// Municipality names that are also ordinary words the grammar reads as words ("N passos de
/// vizinhança", "registro", "contagem").
const COMMON_WORD_PLACES: [&str; 5] = ["passos", "registro", "contagem", "campanha", "combinado"];

/// Every place and sector named in an executed question appears in its request. A name may
/// legitimately be absent when it is excluded ("exceto os de MA" folds into the place), when
/// it is a lowercase two-letter word that is not a state there ("informe se", "para"), when it
/// is a common word that is also a city, or when the answer is a CNPJ lookup.
pub(crate) fn entity_coverage(
    text: &str,
    entities: &[(usize, usize, Vec<Entity>)],
    request: &Request,
    municipality_ufs: &BTreeMap<MunicipalityId, UfId>,
) -> Vec<Finding> {
    if matches!(request.op, Op::Lookup { .. } | Op::Meta { .. }) {
        return Vec::new();
    }
    let words = opencnpj_nlu::normalize::words(text);
    let original: BTreeSet<&str> = text.split(|c: char| !c.is_alphanumeric()).collect();
    let m = mentions(request, municipality_ufs);
    // Anything named after an exclusion marker may be folded into the place by
    // canonicalization ("em RS e DF, exceto … e os de DF"), wherever else it is named.
    let first_exclusion = words
        .iter()
        .position(|w| matches!(w.as_str(), "exceto" | "excluindo" | "tirando" | "menos" | "sem" | "fora"))
        .unwrap_or(usize::MAX);
    let excluded_entities: BTreeSet<String> = entities
        .iter()
        .filter(|(start, _, _)| *start > first_exclusion)
        .flat_map(|(_, _, readings)| readings.iter().map(|e| format!("{e:?}")))
        .collect();
    let mut out = Vec::new();
    for (start, end, readings) in entities {
        let covered = readings.iter().any(|entity| match entity {
            Entity::Municipality { id, .. } => m.municipalities.contains(id),
            Entity::Uf(id) => m.ufs.contains(id),
            Entity::Region(id) => m.regions.contains(id),
            Entity::Sector(id) => m.sectors.contains(id),
        });
        if covered {
            continue;
        }
        let excluded = readings.iter().any(|e| excluded_entities.contains(&format!("{e:?}")));
        let single = end - start == 1;
        let token = &words[*start];
        let previous = start.checked_sub(1).map(|at| words[at].as_str());
        let plain_word = single
            && readings.iter().all(|e| matches!(e, Entity::Uf(_)))
            && token.len() == 2
            && !original.contains(token.to_uppercase().as_str())
            && !matches!(previous, Some("em" | "no" | "na" | "do" | "da" | "de" | "estado"));
        let preposition = single && token == "para" && !matches!(previous, Some("no" | "do" | "estado"));
        let common = single && COMMON_WORD_PLACES.contains(&token.as_str());
        if excluded || plain_word || preposition || common {
            continue;
        }
        let kind = match readings.first() {
            Some(Entity::Sector(_)) => "dropped_sector",
            Some(Entity::Region(_)) => "dropped_region",
            Some(Entity::Uf(_)) => "dropped_uf",
            _ => "dropped_municipality",
        };
        out.push(finding(kind, format!("{} {readings:?}", words[*start..*end].join(" "))));
    }
    out
}

/// Structural invariants of one interpretation.
pub(crate) fn interpretation_shape(interpretation: &Interpretation) -> Vec<Finding> {
    let mut out = Vec::new();
    let executed = matches!(interpretation.decision, Decision::Execute | Decision::ExecuteWithNote);
    if executed && interpretation.request.is_none() && interpretation.company.is_none() {
        out.push(finding("executed_without_query", interpretation.act.clone()));
    }
    if !executed && (interpretation.request.is_some() || interpretation.company.is_some()) {
        out.push(finding("refused_with_query", interpretation.reason.clone()));
    }
    if interpretation.request.is_some() && interpretation.company.is_some() {
        out.push(finding("query_and_company_search", String::new()));
    }
    if let Some(request) = &interpretation.request
        && interpretation.export.is_some()
        && matches!(request.op, Op::Meta { .. })
    {
        out.push(finding("file_for_meta_answer", String::new()));
    }
    if let Some(company) = &interpretation.company
        && company.name.trim().is_empty()
    {
        out.push(finding("empty_company_name", String::new()));
    }
    if !executed && interpretation.reason.trim().is_empty() {
        out.push(finding("refusal_without_reason", String::new()));
    }
    out
}

#[derive(clap::Args)]
pub struct ContractArgs {
    #[command(flatten)]
    pub data: crate::questions::DataArgs,
    /// Evaluation sets (JSONL) whose questions are interpreted and checked.
    #[arg(long, required = true)]
    pub set: Vec<std::path::PathBuf>,
    /// Directory for `findings.jsonl` and `summary.json`.
    #[arg(long)]
    pub out: std::path::PathBuf,
    /// With --snapshot: run the data contracts on every N-th new question (0 = never).
    #[arg(long, default_value_t = 0)]
    pub data_every: usize,
}

/// Interpret every question of the sets and record each contract finding with the item's
/// gold verdict: findings on items scored correct calibrate a contract, the others are defects.
pub fn run(args: ContractArgs) -> anyhow::Result<()> {
    use crate::{eval, questions};
    use opencnpj_nlu::context::Conversation;
    use serde_json::json;
    use std::io::Write;

    let assistant =
        crate::assistant::Assistant::open(&args.data, crate::assistant::Options { hide_contacts: false, export_max_rows: 100_000 })?;
    let assets = assistant.assets();
    let data = args.data_every > 0 && assistant.has_snapshot();
    let mut new_questions = 0usize;
    let (municipality_ufs, _) = crate::query::canonicalization(&assets.catalogue, &assets.sectors, assets.anchor);
    std::fs::create_dir_all(&args.out)?;
    let mut findings = std::io::BufWriter::new(std::fs::File::create(args.out.join("findings.jsonl"))?);
    let mut summary: BTreeMap<String, BTreeMap<&'static str, BTreeMap<&'static str, u64>>> = BTreeMap::new();
    for set in &args.set {
        let name = set.to_string_lossy().replace("crates/opencnpj-lab/eval/", "").replace("/questions.jsonl", "").replace(".jsonl", "");
        let (mut state, mut session) = (Conversation::default(), String::new());
        for item in eval::load_items(set)? {
            if questions::session_of(&item) != session {
                (state, session) = (Conversation::default(), questions::session_of(&item));
            }
            let prediction = assets.interpreter.interpret(&item.text, &mut state);
            let expected =
                item.gold.ir.clone().and_then(|ir| serde_json::from_value::<Request>(ir).ok()).and_then(|r| assets.canonical(r).ok());
            let correct = questions::interpretation_match(&prediction, &item.gold, expected.as_ref());
            let mut found = interpretation_shape(&prediction);
            if let Some(request) = &prediction.request {
                found.extend(entity_coverage(
                    &item.text,
                    &assets.interpreter.entities(&item.text),
                    request.as_request(),
                    &municipality_ufs,
                ));
            }
            if data && prediction.act == "new_query" || data && prediction.act == "company_search" {
                new_questions += 1;
                if new_questions.is_multiple_of(args.data_every) {
                    found.extend(assistant.data_findings(&prediction));
                    *summary.entry(name.clone()).or_default().entry("data_checked").or_default().entry("all").or_default() += 1;
                }
            }
            let entry = summary.entry(name.clone()).or_default();
            *entry.entry("items").or_default().entry("all").or_default() += 1;
            for f in found {
                *entry.entry(f.code).or_default().entry(if correct { "on_correct" } else { "on_wrong" }).or_default() += 1;
                writeln!(
                    findings,
                    "{}",
                    json!({"set": name, "id": item.id, "text": item.text, "correct": correct, "expected": item.gold.decision, "code": f.code, "detail": f.detail, "decision": prediction.decision, "act": prediction.act})
                )?;
            }
        }
    }
    findings.flush()?;
    std::fs::write(args.out.join("summary.json"), serde_json::to_string_pretty(&summary)?)?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

/// What a record must satisfy to belong to a list answer, resolved to RFB codes.
pub(crate) struct RecordScope {
    pub municipality_codes: Option<BTreeSet<String>>,
    pub uf_codes: Option<BTreeSet<String>>,
    pub excluded_municipality_codes: BTreeSet<String>,
    pub excluded_uf_codes: BTreeSet<String>,
}

fn status_of(code: &str) -> Option<Status> {
    Some(match code.trim_start_matches('0') {
        "1" => Status::Null,
        "2" => Status::Active,
        "3" => Status::Suspended,
        "4" => Status::Unfit,
        "8" => Status::Closed,
        _ => return None,
    })
}

fn day_of(value: &str) -> Option<Day> {
    (value.len() == 8).then(|| Day::from_ymd(value[..4].parse().ok()?, value[4..6].parse().ok()?, value[6..].parse().ok()?)).flatten()
}

fn cnae_matches(code: &CnaeCode, cnae: u32) -> bool {
    match *code {
        CnaeCode::Subclass(c) => cnae == c,
        CnaeCode::Class(c) => cnae / 100 == c,
        CnaeCode::Group(c) => cnae / 10_000 == u32::from(c),
        CnaeCode::Division(c) => cnae / 100_000 == u32::from(c),
        // Sections span several divisions; the engine resolves them, records are not rechecked.
        CnaeCode::Section(_) => true,
    }
}

fn within(range: &DayRange, day: Option<Day>) -> bool {
    day.is_some_and(|d| range.from.is_none_or(|f| d >= f) && range.to.is_none_or(|t| d <= t))
}

/// Every filter of `request` that a record carries, checked against the record (D2).
pub(crate) fn record_matches(request: &Request, scope: &RecordScope, est: &[String]) -> Vec<Finding> {
    use opencnpj_core::schema::col;
    let mut out = Vec::new();
    let field = |i: usize| est.get(i).map_or("", |s| s.trim());
    let cnpj = format!("{}{}", field(col::est::ORDEM), field(col::est::DV));
    let municipio = field(col::est::MUNICIPIO);
    let uf = field(col::est::UF);
    if scope.municipality_codes.as_ref().is_some_and(|codes| !codes.contains(municipio)) {
        out.push(finding("record_outside_place", format!("{cnpj} municipio {municipio}")));
    }
    if scope.uf_codes.as_ref().is_some_and(|codes| !codes.contains(uf)) {
        out.push(finding("record_outside_place", format!("{cnpj} uf {uf}")));
    }
    if scope.excluded_municipality_codes.contains(municipio) || scope.excluded_uf_codes.contains(uf) {
        out.push(finding("record_in_exclusion", format!("{cnpj} {municipio}/{uf}")));
    }
    let cnae: u32 = field(col::est::CNAE_PRINCIPAL).parse().unwrap_or(0);
    let codes = match &request.scope.activity {
        ActivityFilter::Sectors { cnaes, scope: CnaeScope::Main, .. } | ActivityFilter::Cnae { codes: cnaes, scope: CnaeScope::Main } => {
            Some(cnaes)
        }
        _ => None,
    };
    if let Some(codes) = codes
        && !codes.iter().any(|code| cnae_matches(code, cnae))
    {
        out.push(finding("record_outside_activity", format!("{cnpj} cnae {cnae}")));
    }
    if request.scope.exclude.sector_cnaes.iter().any(|code| cnae_matches(code, cnae)) {
        out.push(finding("record_in_excluded_activity", format!("{cnpj} cnae {cnae}")));
    }
    let status = request.scope.status.unwrap_or_else(|| request.op.default_status());
    if !status_of(field(col::est::SITUACAO)).is_some_and(|s| status.contains(s)) {
        out.push(finding("record_outside_status", format!("{cnpj} situacao {}", field(col::est::SITUACAO))));
    }
    let head = field(col::est::MATRIZ_FILIAL);
    let head_ok = match (request.unit, request.scope.head_office) {
        (Unit::HeadOffices, _) | (_, Tri::Yes) => head == "1",
        (_, Tri::No) => head == "2",
        (_, Tri::Any) => true,
    };
    if !head_ok {
        out.push(finding("record_outside_head_office", format!("{cnpj} matriz_filial {head}")));
    }
    if let Some(range) = &request.scope.opened
        && !within(range, day_of(field(col::est::DATA_INICIO)))
    {
        out.push(finding("record_outside_opened", format!("{cnpj} inicio {}", field(col::est::DATA_INICIO))));
    }
    if let Some(range) = &request.scope.status_date
        && !within(range, day_of(field(col::est::DATA_SITUACAO)))
    {
        out.push(finding("record_outside_status_date", format!("{cnpj} data_situacao {}", field(col::est::DATA_SITUACAO))));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::questions::{Assets, DataArgs};
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn assets() -> Assets {
        let root = root();
        Assets::open(&DataArgs {
            catalog: root.join("crates/opencnpj-insights/catalog/municipalities.tsv"),
            sectors: root.join("crates/opencnpj-insights/catalog/sectors.toml"),
            anchor: "2026-10-11".into(),
            agg: None,
            history: None,
            snapshot: None,
        })
        .unwrap()
    }

    /// An establishment row in the RFB layout with the fields the contracts read.
    fn est(municipio: &str, uf: &str, cnae: &str, situacao: &str, matriz: &str, inicio: &str) -> Vec<String> {
        use opencnpj_core::schema::col;
        let mut row = vec![String::new(); 30];
        for (at, value) in [
            (col::est::ORDEM, "0001"),
            (col::est::DV, "00"),
            (col::est::MUNICIPIO, municipio),
            (col::est::UF, uf),
            (col::est::CNAE_PRINCIPAL, cnae),
            (col::est::SITUACAO, situacao),
            (col::est::MATRIZ_FILIAL, matriz),
            (col::est::DATA_INICIO, inicio),
        ] {
            row[at] = value.into();
        }
        row
    }

    #[test]
    fn records_outside_the_query_are_reported() {
        let request: Request = serde_json::from_value(serde_json::json!({
            "op": {"kind": "list", "order": "cnpj", "limit": 20}, "unit": "establishments",
            "scope": {"place": {"municipalities": [5527]}, "activity": {"cnae": {"codes": [{"division": 1}], "scope": "main"}},
                      "status": ["active"], "head_office": "yes", "opened": {"from": 45658, "to": 46022}},
            "time": {"competence": 321, "period": null, "anchor": 46305}, "output": {"page": 1},
            "versions": {"municipalities": 202610, "sectors": 1}
        }))
        .unwrap();
        let scope = RecordScope {
            municipality_codes: Some(BTreeSet::from(["9907".to_owned()])),
            uf_codes: None,
            excluded_municipality_codes: BTreeSet::new(),
            excluded_uf_codes: BTreeSet::new(),
        };
        // 2025-03-01 is inside the opened range; the matching record has no finding.
        assert!(record_matches(&request, &scope, &est("9907", "MT", "0115600", "02", "1", "20250301")).is_empty());
        let codes = |row: Vec<String>| record_matches(&request, &scope, &row).into_iter().map(|f| f.code).collect::<Vec<_>>();
        assert_eq!(codes(est("9067", "MT", "0115600", "02", "1", "20250301")), ["record_outside_place"]);
        assert_eq!(codes(est("9907", "MT", "6422100", "02", "1", "20250301")), ["record_outside_activity"]);
        assert_eq!(codes(est("9907", "MT", "0115600", "08", "1", "20250301")), ["record_outside_status"]);
        assert_eq!(codes(est("9907", "MT", "0115600", "02", "2", "20250301")), ["record_outside_head_office"]);
        assert_eq!(codes(est("9907", "MT", "0115600", "02", "1", "20190301")), ["record_outside_opened"]);
    }

    #[test]
    fn a_named_place_missing_from_the_query_is_reported() {
        let assets = assets();
        let (municipality_ufs, _) = crate::query::canonicalization(&assets.catalogue, &assets.sectors, assets.anchor);
        let national: Request = serde_json::from_value(serde_json::json!({
            "op": {"kind": "count", "metric": "stock"}, "unit": "establishments", "scope": {"status": ["active"]},
            "time": {"competence": 321, "period": null, "anchor": 46305}, "output": {"page": 1},
            "versions": {"municipalities": 202610, "sectors": 1}
        }))
        .unwrap();
        let text = "quantas padarias ativas existem em Sorriso/MT";
        let found = entity_coverage(text, &assets.interpreter.entities(text), &national, &municipality_ufs);
        let codes: BTreeSet<_> = found.iter().map(|f| f.code).collect();
        assert!(codes.contains("dropped_municipality") && codes.contains("dropped_sector"), "{found:?}");
        // The preposition "para" and an excluded state are not drops.
        let text = "liste empresas para revenda, exceto as de MT";
        assert!(entity_coverage(text, &assets.interpreter.entities(text), &national, &municipality_ufs).is_empty());
    }

    #[test]
    fn sector_ids_only_grow_at_the_end() {
        #[derive(serde::Deserialize)]
        struct File {
            sector: Vec<Entry>,
        }
        #[derive(serde::Deserialize)]
        struct Entry {
            id: String,
            cnae_core: Vec<String>,
        }
        let root = root();
        let file: File =
            toml::from_str(&std::fs::read_to_string(root.join("crates/opencnpj-insights/catalog/sectors.toml")).unwrap()).unwrap();
        let pinned = std::fs::read_to_string(root.join("crates/opencnpj-lab/eval/contracts/sector-ids.txt")).unwrap();
        let pinned: Vec<&str> = pinned.lines().filter(|l| !l.starts_with('#') && !l.is_empty()).collect();
        let current: Vec<&str> = file.sector.iter().map(|s| s.id.as_str()).collect();
        assert!(current.len() >= pinned.len() && current[..pinned.len()] == pinned[..], "a sector was removed, renamed or moved");
        // Every CNAE of a sector is a real subclass of the CNAE catalogue, or a documented
        // legacy subclass that RFB records still carry.
        let cnae = std::fs::read_to_string(root.join("crates/opencnpj-insights/catalog/cnae.tsv")).unwrap();
        let subclasses: BTreeSet<&str> = cnae
            .lines()
            .filter_map(|l| l.split('\t').collect::<Vec<_>>().get(..2).map(|c| (c[0], c[1])))
            .filter(|(_, level)| matches!(*level, "subclass" | "legacy"))
            .map(|(code, _)| code)
            .collect();
        for sector in &file.sector {
            for code in &sector.cnae_core {
                assert!(subclasses.contains(code.as_str()), "{} lists unknown CNAE {code}", sector.id);
            }
        }
    }

    #[test]
    fn delivery_words_never_name_a_place() {
        // A request, glue or tag word that is also a place would be cut out of the question.
        let assets = assets();
        let vocabulary: toml::Value =
            toml::from_str(&std::fs::read_to_string(root().join("crates/opencnpj-nlu/lang/pt-BR/delivery.toml")).unwrap()).unwrap();
        let mut offenders = Vec::new();
        for (name, items) in vocabulary["lists"].as_table().unwrap() {
            for item in items.as_array().unwrap().iter().filter_map(|i| i.as_str()).filter(|i| !i.starts_with('@')) {
                let places = assets.interpreter.entities(item).into_iter().any(|(start, end, readings)| {
                    end - start == opencnpj_nlu::normalize::words(item).len()
                        && start == 0
                        && readings.iter().any(|e| !matches!(e, Entity::Sector(_)))
                });
                if places {
                    offenders.push(format!("{name}: {item}"));
                }
            }
        }
        // Reviewed: these regional words are also city names; they are cut only right next to
        // a file request, never from the data question.
        let reviewed = ["regional_nordeste: massa", "regional_sul: capaz", "regional_sudeste: suave"];
        offenders.retain(|o| !reviewed.contains(&o.as_str()));
        assert!(offenders.is_empty(), "{offenders:?}");
    }

    /// Interpretation contracts on every set small enough for CI; the full rounds run in the
    /// evaluation tasks (docs/intelligence/EVALUATION-TASKS.md).
    #[test]
    fn evaluation_sets_keep_the_interpretation_contracts() {
        use crate::{eval, questions};
        use opencnpj_nlu::context::Conversation;
        let assets = assets();
        let (municipality_ufs, _) = crate::query::canonicalization(&assets.catalogue, &assets.sectors, assets.anchor);
        let mut findings = Vec::new();
        for set in [
            "commercial-round-01/questions.jsonl",
            "adv.jsonl",
            "dialogues.jsonl",
            "ood.jsonl",
            "h-informal.jsonl",
            "skills-export-01/questions.jsonl",
            "skills-company-01/questions.jsonl",
            "probes-01/questions.jsonl",
        ] {
            let (mut state, mut session) = (Conversation::default(), String::new());
            for item in eval::load_items(&root().join("crates/opencnpj-lab/eval").join(set)).unwrap() {
                if questions::session_of(&item) != session {
                    (state, session) = (Conversation::default(), questions::session_of(&item));
                }
                let prediction = assets.interpreter.interpret(&item.text, &mut state);
                let mut found = interpretation_shape(&prediction);
                if let Some(request) = &prediction.request {
                    found.extend(entity_coverage(
                        &item.text,
                        &assets.interpreter.entities(&item.text),
                        request.as_request(),
                        &municipality_ufs,
                    ));
                }
                findings.extend(found.into_iter().map(|f| format!("{} {}: {} ({})", item.id, f.code, f.detail, item.text)));
            }
        }
        assert!(findings.is_empty(), "{} findings:\n{}", findings.len(), findings.join("\n"));
    }

    /// Every probe (a question that once got a silently wrong answer) keeps its gold answer.
    #[test]
    fn probes_keep_their_gold_answers() {
        use crate::{eval, questions};
        use opencnpj_nlu::context::Conversation;
        let assets = assets();
        let mut wrong = Vec::new();
        for item in eval::load_items(&root().join("crates/opencnpj-lab/eval/probes-01/questions.jsonl")).unwrap() {
            let prediction = assets.interpreter.interpret(&item.text, &mut Conversation::default());
            let expected = item.gold.ir.clone().map(|ir| assets.canonical(serde_json::from_value(ir).unwrap()).unwrap());
            if !questions::interpretation_match(&prediction, &item.gold, expected.as_ref()) {
                wrong.push(format!("{} {}", item.id, item.text));
            }
        }
        assert!(wrong.is_empty(), "{wrong:#?}");
    }
}
