//! Golden-set differentials and benchmarks over the engine executor of the assistant.

use std::path::Path;

use anyhow::{Context, Result, ensure};
pub use opencnpj::assistant::engine::*;
use opencnpj_ingest::source::sha256_file;
use opencnpj_insights::{
    catalog::municipalities::MunicipalityCatalogue,
    execute::{AggregateSet, ExecContext},
};
use opencnpj_query::ir::*;
use opencnpj_store::{Snapshot, search::Limits};

pub fn load_golden(path: &Path) -> Result<Vec<(String, Request)>> {
    #[derive(serde::Deserialize)]
    struct Case {
        case: String,
        request: Request,
    }
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    text.lines()
        .map(|line| {
            let case: Case = serde_json::from_str(line)?;
            Ok((case.case, case.request))
        })
        .collect()
}

#[derive(clap::Args)]
pub struct DiffArgs {
    #[arg(long)]
    pub agg: std::path::PathBuf,
    #[arg(long)]
    pub history: Option<std::path::PathBuf>,
    #[arg(long)]
    pub snapshot: std::path::PathBuf,
    #[arg(long, default_value = "/data/opencnpj-data/intelligence/catalog-municipalities/2026-10/municipalities.tsv")]
    pub catalog: std::path::PathBuf,
    #[arg(long, default_value = "crates/opencnpj-insights/catalog/sectors.toml")]
    pub sectors: std::path::PathBuf,
    #[arg(long, default_value = "crates/opencnpj-query/tests/fixtures/ir-v1-requests.jsonl")]
    pub cases: std::path::PathBuf,
    #[arg(long)]
    pub case: Option<String>,
}

pub fn diff(args: DiffArgs) -> Result<()> {
    let catalogue = MunicipalityCatalogue::load(&args.catalog)?;
    let sectors = load_sectors(&args.sectors)?;
    let aggregates = opencnpj_insights::execute::AggregateSet::open(&args.agg, args.history.as_deref())?;
    ensure!(
        sha256_file(&args.catalog)? == aggregates.current().manifest().municipality_sha256,
        "aggregate uses another municipality catalogue"
    );
    let context = ExecContext::new(&catalogue, &sectors);
    let snapshot = Snapshot::open(&args.snapshot)?;
    snapshot.set_postings_cache_bytes(512 << 20);
    let engine = EngineExecutor { snapshot: &snapshot, context: &context, limits: Limits::DEFAULT };
    let last_day = Day::from_ymd(2026, 10, 11).expect("anchor");
    let current = Competence::from_year_month(2026, 10).expect("current");
    let mut compared = 0;
    let mut mismatches = 0;
    let mut aggregate_only = 0;
    let mut engine_only = 0;
    let mut engine_errors = 0;
    let mut invalid = 0;
    for (case, request) in load_golden(&args.cases)? {
        if args.case.as_ref().is_some_and(|filter| !case.contains(filter.as_str())) {
            continue;
        }
        let valid = match validate_request(request, &catalogue, &sectors, last_day) {
            Ok(valid) => valid,
            Err(error) => {
                println!("INVALID\t{case}\t{error}");
                invalid += 1;
                continue;
            }
        };
        let request = valid.as_request();
        let plan = match opencnpj_insights::plan::plan(&valid) {
            Ok(plan) => plan,
            Err(error) => {
                println!("UNSUPPORTED\t{case}\t{error}");
                continue;
            }
        };
        match plan.backend {
            opencnpj_insights::plan::Backend::Static => println!("static\t{case}"),
            opencnpj_insights::plan::Backend::Engine => match engine.execute(&valid) {
                Ok(value) => {
                    println!("engine-only\t{case}\t{value:?}");
                    engine_only += 1;
                }
                Err(error) => {
                    println!("engine-n/a\t{case}\t{error}");
                    engine_errors += 1;
                }
            },
            opencnpj_insights::plan::Backend::Aggregates => {
                let aggregate = opencnpj_insights::execute::execute(&valid, &aggregates, &context);
                let aggregate = match aggregate {
                    Ok(value) => value,
                    Err(error) => {
                        println!("aggregate-error\t{case}\t{error}");
                        mismatches += 1;
                        continue;
                    }
                };
                let historical = request.time.competence != current;
                let active_change = matches!(request.op.metric(), Some(Metric::ActiveChange { .. }));
                if historical || active_change {
                    // The engine has no past snapshots: check these against the artifacts'
                    // raw marginal sections instead, when the request shape allows it.
                    match crate::oracle::marginal_value(request, &aggregates, &sectors)? {
                        Some(expected) if expected == aggregate => {
                            println!("marginal-match\t{case}");
                            compared += 1;
                        }
                        Some(expected) => {
                            println!("MISMATCH\t{case}\taggregate={aggregate:?}\tmarginals={expected:?}");
                            compared += 1;
                            mismatches += 1;
                        }
                        None => {
                            println!("aggregate-only\t{case}\t{aggregate:?}");
                            aggregate_only += 1;
                        }
                    }
                    continue;
                }
                match engine.execute(&valid) {
                    Ok(engine_value) => {
                        if aggregate == engine_value {
                            println!("match\t{case}");
                        } else {
                            println!("MISMATCH\t{case}\taggregate={aggregate:?}\tengine={engine_value:?}");
                            mismatches += 1;
                        }
                        compared += 1;
                    }
                    Err(error) => {
                        println!("engine-n/a\t{case}\t{error}");
                        engine_errors += 1;
                    }
                }
            }
        }
    }
    println!(
        "summary\tcompared={compared}\tmismatches={mismatches}\taggregate_only={aggregate_only}\tengine_only={engine_only}\tengine_n/a={engine_errors}\tinvalid={invalid}"
    );
    ensure!(invalid == 0, "{invalid} invalid cases");
    ensure!(mismatches == 0, "{mismatches} mismatches");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::io::BufWriter;

    use super::*;
    use opencnpj_insights::{
        agg::{self, Manifest},
        catalog::municipalities::{Geography, Municipality, Place},
        execute::SectorMap,
    };
    use opencnpj_store::{numeric::NumField, postings::Field};
    use serde_json::json;

    fn fitting_catalogue(snapshot: &opencnpj_store::Snapshot) -> MunicipalityCatalogue {
        let mut toms = BTreeSet::new();
        for (tom, _) in snapshot.field_values(Field::Municipio).unwrap() {
            toms.insert(tom.parse::<u16>().unwrap());
        }
        let rows = toms
            .into_iter()
            .enumerate()
            .map(|(id, tom)| {
                let name = format!("Fixture {tom}");
                Municipality {
                    id: id as u16,
                    tom,
                    folded: opencnpj_insights::catalog::municipalities::fold_name(&name),
                    name,
                    variants: vec![],
                    place: Place::Municipality(Geography {
                        ibge: 1_000_001 + id as u32,
                        uf: "SP".into(),
                        census_2022: Some(1),
                        population_estimate: 1,
                        estimate_year: 2025,
                        latitude: 0.0,
                        longitude: 0.0,
                        neighbours: vec![],
                        immediate_region: 1,
                        intermediate_region: 1,
                        capital: false,
                    }),
                }
            })
            .collect();
        MunicipalityCatalogue::from_rows(rows).unwrap()
    }

    fn request(op: serde_json::Value, scope: serde_json::Value) -> Request {
        serde_json::from_value(json!({
            "op": op,
            "unit": "establishments",
            "scope": scope,
            "time": {"competence": 321, "period": null, "anchor": 46305},
            "output": {},
            "versions": {"municipalities": 202610, "sectors": 1}
        }))
        .unwrap()
    }

    #[test]
    fn records_preserve_exclusions_and_population_before_pagination() {
        let directory = tempfile::tempdir().unwrap();
        let (path, _) = opencnpj_ingest::testkit::build_fixture(directory.path(), "balanced");
        let snapshot = Snapshot::open(&path).unwrap();
        let catalogue = fitting_catalogue(&snapshot);
        let sectors = SectorMap::new();
        let context = ExecContext::new(&catalogue, &sectors);
        let engine = EngineExecutor { snapshot: &snapshot, context: &context, limits: Limits::DEFAULT };
        let plain =
            request(json!({"kind":"list","order":"cnpj","limit":100}), json!({"status":["null","active","suspended","unfit","closed"]}));
        let valid = validate_request(plain.clone(), &catalogue, &sectors, Day(46305)).unwrap();
        let original = engine.records(&valid).unwrap();
        assert!(original.total > 0);
        let municipality_column =
            opencnpj_core::schema::Table::Estabelecimentos.row_columns().iter().position(|c| c.name == "municipio").unwrap();
        let unknown = snapshot
            .docs(&original.docs)
            .unwrap()
            .into_iter()
            .filter(|r| {
                r.estab
                    .and_then(|i| r.company.estabelecimentos.get(i))
                    .and_then(|row| row.get(municipality_column))
                    .and_then(|code| code.parse::<u16>().ok())
                    .and_then(|tom| catalogue.by_tom(tom))
                    .is_none()
            })
            .count();
        let mut excluded = plain.clone();
        excluded.scope.exclude.municipalities = SmallSet::new(catalogue.rows().iter().map(|r| MunicipalityId(r.id)));
        let valid = validate_request(excluded, &catalogue, &sectors, Day(46305)).unwrap();
        let result = engine.records(&valid).unwrap();
        // Excluding every known municipality must preserve source rows with no valid municipality.
        assert_eq!(result.total, unknown as u64);
        assert_eq!(result.docs.len(), unknown);
        let mut filtered = plain;
        filtered.scope.place_population = Some(Range { from: Some(2), to: None });
        filtered.scope.population_ref = Some(PopulationRef::IbgeEstimate(2025));
        filtered.scope.place = PlaceFilter::Ufs(SmallSet::new([UfId(25)]));
        let valid = validate_request(filtered, &catalogue, &sectors, Day(46305)).unwrap();
        let result = engine.records(&valid).unwrap();
        assert_eq!(result.total, 0);
        assert!(result.docs.is_empty());
    }

    #[test]
    fn registration_flow_preserves_an_explicit_current_status_filter() {
        let directory = tempfile::tempdir().unwrap();
        let (path, _) = opencnpj_ingest::testkit::build_fixture(directory.path(), "balanced");
        let snapshot = Snapshot::open(&path).unwrap();
        let catalogue = fitting_catalogue(&snapshot);
        let sectors = SectorMap::new();
        let context = ExecContext::new(&catalogue, &sectors);
        let engine = EngineExecutor { snapshot: &snapshot, context: &context, limits: Limits::DEFAULT };
        let mut flow = request(json!({"kind":"count","metric":{"flow":"registrations"}}), json!({"status":["active"]}));
        let range = Range { from: Some(Day(1)), to: Some(Day(46305)) };
        flow.time.period = Some(Period::Days(range));
        let valid = validate_request(flow.clone(), &catalogue, &sectors, Day(46305)).unwrap();
        let actual = engine.execute(&valid).unwrap();
        let stock = request(json!({"kind":"count","metric":"stock"}), json!({"status":["active"],"opened":{"from":1,"to":46305}}));
        let stock = validate_request(stock, &catalogue, &sectors, Day(46305)).unwrap();
        assert_eq!(actual, engine.execute(&stock).unwrap());
        flow.scope.status = Some(StatusSet::ALL);
        let all = validate_request(flow, &catalogue, &sectors, Day(46305)).unwrap();
        assert_ne!(actual, engine.execute(&all).unwrap(), "fixture must distinguish active registrations from every status");
    }

    #[tokio::test]
    async fn http_client_uses_key_timeout_and_concurrency() {
        use axum::{Json, Router, extract::Query, http::HeaderMap, routing::get};
        let app = Router::new().route(
            "/api/v1/search",
            get(|headers: HeaderMap, Query(params): Query<Vec<(String, String)>>| async move {
                assert_eq!(headers.get("authorization").and_then(|value| value.to_str().ok()), Some("Bearer test-key"));
                assert!(params.iter().any(|(key, value)| key == "limit" && value == "1"));
                assert!(params.iter().any(|(key, value)| key == "matriz_filial" && value == "1,2"));
                Json(json!({"total": 42}))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let catalogue = MunicipalityCatalogue::from_rows(vec![Municipality {
            id: 0,
            tom: 7107,
            name: "Test".into(),
            folded: opencnpj_insights::catalog::municipalities::fold_name("Test"),
            variants: vec![],
            place: Place::Municipality(Geography {
                ibge: 1_000_001,
                uf: "SP".into(),
                census_2022: Some(1),
                population_estimate: 1,
                estimate_year: 2025,
                latitude: 0.0,
                longitude: 0.0,
                neighbours: vec![],
                immediate_region: 1,
                intermediate_region: 1,
                capital: false,
            }),
        }])
        .unwrap();
        let client =
            HttpEngineClient::new(format!("http://{address}"), Some("test-key".into()), std::time::Duration::from_secs(5), 2).unwrap();
        let request = request(json!({"kind": "count", "metric": "stock"}), json!({}));
        assert_eq!(client.count(&request, &catalogue).await.unwrap(), 42);
        assert_eq!(client.count_many(&[request.clone(), request], &catalogue).await.unwrap(), vec![42, 42]);
    }

    #[test]
    fn fixture_lowering_and_differential() {
        let directory = tempfile::tempdir().unwrap();
        let (snapshot_path, _) = opencnpj_ingest::testkit::build_fixture(directory.path(), "balanced");
        let snapshot = opencnpj_store::Snapshot::open(&snapshot_path).unwrap();
        snapshot.set_postings_cache_bytes(64 << 20);
        let catalogue = fitting_catalogue(&snapshot);
        let sectors = SectorMap::from([(0, BTreeSet::from([1091102])), (1, BTreeSet::from([4712100]))]);
        let result = agg::builder::build(&snapshot, &catalogue, &agg::Table::ALL).unwrap();
        let manifest = Manifest {
            competence: Competence(321),
            snapshot_sha256: "0".repeat(64),
            municipality_sha256: "0".repeat(64),
            tables: agg::Table::ALL.to_vec(),
            establishments: result.establishments,
            missing_registration_date: result.missing_registration_date,
            missing_closure_date: result.missing_closure_date,
        };
        let aggregate_path = directory.path().join("fixture.ocagg");
        agg::write(BufWriter::new(std::fs::File::create(&aggregate_path).unwrap()), &manifest, &result.sections).unwrap();
        let aggregates = AggregateSet::open(&aggregate_path, None).unwrap();
        let context = ExecContext::new(&catalogue, &sectors);
        let engine = EngineExecutor { snapshot: &snapshot, context: &context, limits: Limits::DEFAULT };
        let last_day = Day::from_ymd(2026, 10, 11).unwrap();
        let sector_scope = json!({"place": {"municipalities": [3]}, "activity": {"sectors": {"ids": [0], "cnaes": [], "scope": "main"}}, "status": ["active"]});
        let cases = [
            ("count", request(json!({"kind": "count", "metric": "stock"}), sector_scope.clone())),
            (
                "compare",
                request(
                    json!({"kind": "compare", "dim": "municipality", "members": [{"municipality": 3}, {"municipality": 1}], "metric": "stock"}),
                    json!({"activity": {"sectors": {"ids": [1], "cnaes": [], "scope": "main"}}, "status": ["active"]}),
                ),
            ),
            (
                "breakdown",
                request(
                    json!({"kind": "breakdown", "dim": "uf", "metric": "stock"}),
                    json!({"activity": {"sectors": {"ids": [1], "cnaes": [], "scope": "main"}}, "status": ["active"]}),
                ),
            ),
            (
                "rank",
                request(
                    json!({"kind": "rank", "dim": "municipality", "metric": "stock", "dir": "desc", "k": 10, "min_base": 0}),
                    json!({"activity": {"sectors": {"ids": [1], "cnaes": [], "scope": "main"}}, "status": ["active"]}),
                ),
            ),
            ("closed", request(json!({"kind": "count", "metric": "stock"}), json!({"status": ["closed"]}))),
            ("density", request(json!({"kind": "count", "metric": {"per10k": {"ibge_estimate": 2025}}}), sector_scope.clone())),
            ("head_office", request(json!({"kind": "count", "metric": "stock"}), json!({"head_office": "yes", "status": ["active"]}))),
        ];
        for (name, request) in cases {
            let valid = validate_request(request, &catalogue, &sectors, last_day).unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!(opencnpj_insights::plan::plan(&valid).unwrap().backend, opencnpj_insights::plan::Backend::Aggregates, "{name}");
            let aggregate = opencnpj_insights::execute::execute(&valid, &aggregates, &context).unwrap();
            let engine_value = engine.execute(&valid).unwrap();
            assert_eq!(aggregate, engine_value, "{name}");
        }

        let valid =
            validate_request(request(json!({"kind": "count", "metric": "stock"}), sector_scope), &catalogue, &sectors, last_day).unwrap();
        let query = parse(&lower(valid.as_request(), &catalogue).unwrap()).unwrap();
        assert!(query.filters.contains(&(Field::Situacao, vec!["02".into()])));
        assert!(query.filters.contains(&(Field::Municipio, vec!["7107".into()])));
        assert!(query.filters.contains(&(Field::Cnae, vec!["1091102".into()])));
        assert!(query.filters.contains(&(Field::MatrizFilial, vec!["1".into(), "2".into()])));

        let mut list = request(
            json!({"kind": "list", "order": "capital_desc", "limit": 10}),
            json!({"place": {"ufs": [25]}, "capital": {"from": 5_000_000_000_u64}, "contact": "phone"}),
        );
        list.output.page = 2;
        let query = parse(&lower(&list, &catalogue).unwrap()).unwrap();
        assert_eq!(query.limit, 10);
        assert_eq!(query.offset, 10);
        assert_eq!(query.sort, Some((NumField::CapitalSocial, true)));
        assert!(query.filters.contains(&(Field::Contato, vec!["telefone".into()])));
        let capital = query.ranges.iter().find(|range| range.field == NumField::CapitalSocial).unwrap();
        assert_eq!(capital.range.unwrap().lo, 5_000_000_001);
    }

    #[test]
    fn lowering_an_unknown_municipality_is_an_error() {
        let catalogue = MunicipalityCatalogue::from_rows(vec![Municipality {
            id: 0,
            tom: 7107,
            name: "Test".into(),
            folded: opencnpj_insights::catalog::municipalities::fold_name("Test"),
            variants: vec![],
            place: Place::Municipality(Geography {
                ibge: 1_000_001,
                uf: "SP".into(),
                census_2022: Some(1),
                population_estimate: 1,
                estimate_year: 2025,
                latitude: 0.0,
                longitude: 0.0,
                neighbours: vec![],
                immediate_region: 1,
                intermediate_region: 1,
                capital: false,
            }),
        }])
        .unwrap();
        let bad = request(json!({"kind": "count", "metric": "stock"}), json!({"place": {"municipalities": [9999]}}));
        assert!(lower(&bad, &catalogue).is_err());
    }
}

/// HTTP client for the engine API (service key, timeout, bounded concurrency).
pub struct HttpEngineClient {
    client: reqwest::Client,
    base: String,
    key: Option<String>,
    permits: tokio::sync::Semaphore,
    concurrency: usize,
}

impl HttpEngineClient {
    pub fn new(base: impl Into<String>, key: Option<String>, timeout: std::time::Duration, concurrency: usize) -> Result<Self> {
        ensure!(concurrency > 0, "concurrency must be positive");
        let client = reqwest::Client::builder().timeout(timeout).build().context("build HTTP client")?;
        Ok(Self {
            client,
            base: base.into().trim_end_matches('/').to_string(),
            key,
            permits: tokio::sync::Semaphore::new(concurrency),
            concurrency,
        })
    }

    pub async fn count(&self, request: &Request, catalogue: &MunicipalityCatalogue) -> Result<u64> {
        let pairs = lower(request, catalogue)?;
        let mut call = self.client.get(format!("{}/api/v1/search", self.base)).query(&pairs);
        if let Some(key) = &self.key {
            call = call.bearer_auth(key);
        }
        let response = call.send().await.context("engine request failed")?;
        ensure!(response.status().is_success(), "engine returned {}", response.status());
        let body: serde_json::Value =
            serde_json::from_str(&response.text().await.context("engine response body")?).context("engine response is not JSON")?;
        body.get("total").and_then(serde_json::Value::as_u64).context("engine response has no total")
    }

    pub async fn count_many(&self, requests: &[Request], catalogue: &MunicipalityCatalogue) -> Result<Vec<u64>> {
        use futures_util::{StreamExt, stream};
        let results = stream::iter(requests)
            .map(|request| async move {
                let _permit = self.permits.acquire().await.context("client closed")?;
                self.count(request, catalogue).await
            })
            .buffered(self.concurrency)
            .collect::<Vec<_>>()
            .await;
        results.into_iter().collect()
    }
}

pub fn bench(args: DiffArgs) -> Result<()> {
    let catalogue = MunicipalityCatalogue::load(&args.catalog)?;
    let sectors = load_sectors(&args.sectors)?;
    let aggregates = AggregateSet::open(&args.agg, args.history.as_deref())?;
    ensure!(
        sha256_file(&args.catalog)? == aggregates.current().manifest().municipality_sha256,
        "aggregate uses another municipality catalogue"
    );
    let context = ExecContext::new(&catalogue, &sectors);
    let snapshot = Snapshot::open(&args.snapshot)?;
    snapshot.set_postings_cache_bytes(512 << 20);
    let engine = EngineExecutor { snapshot: &snapshot, context: &context, limits: Limits::DEFAULT };
    let last_day = Day::from_ymd(2026, 10, 11).expect("anchor");
    let current = Competence::from_year_month(2026, 10).expect("current");
    println!("case\tbackend\taggregate_us\tengine_us");
    let (mut aggregate_total, mut engine_total) = (0_u128, 0_u128);
    for (case, request) in load_golden(&args.cases)? {
        if args.case.as_ref().is_some_and(|filter| !case.contains(filter.as_str())) {
            continue;
        }
        let valid = validate_request(request, &catalogue, &sectors, last_day)?;
        let plan = opencnpj_insights::plan::plan(&valid)?;
        let request = valid.as_request();
        let mut aggregate_ns = 0;
        let mut engine_ns = 0;
        match plan.backend {
            opencnpj_insights::plan::Backend::Aggregates => {
                let start = std::time::Instant::now();
                opencnpj_insights::execute::execute(&valid, &aggregates, &context)?;
                aggregate_ns = start.elapsed().as_nanos();
                let historical = request.time.competence != current;
                let active_change = matches!(request.op.metric(), Some(Metric::ActiveChange { .. }));
                if !historical && !active_change {
                    let start = std::time::Instant::now();
                    engine.execute(&valid)?;
                    engine_ns = start.elapsed().as_nanos();
                }
            }
            opencnpj_insights::plan::Backend::Engine => {
                let start = std::time::Instant::now();
                if engine.execute(&valid).is_ok() {
                    engine_ns = start.elapsed().as_nanos();
                }
            }
            opencnpj_insights::plan::Backend::Static => {}
        }
        aggregate_total += aggregate_ns;
        engine_total += engine_ns;
        println!("{case}\t{:?}\t{}\t{}", plan.backend, aggregate_ns / 1000, engine_ns / 1000);
    }
    println!("total\t\t{}\t{}", aggregate_total / 1000, engine_total / 1000);
    Ok(())
}

#[derive(clap::Args)]
pub struct RemoteArgs {
    #[arg(long)]
    pub base: String,
    #[arg(long)]
    pub key: Option<String>,
    #[arg(long, default_value_t = 8)]
    pub concurrency: usize,
    #[arg(long, default_value_t = 10)]
    pub timeout_seconds: u64,
    #[arg(long, default_value = "/data/opencnpj-data/intelligence/catalog-municipalities/2026-10/municipalities.tsv")]
    pub catalog: std::path::PathBuf,
    #[arg(long, default_value = "crates/opencnpj-insights/catalog/sectors.toml")]
    pub sectors: std::path::PathBuf,
    #[arg(long, default_value = "crates/opencnpj-query/tests/fixtures/ir-v1-requests.jsonl")]
    pub cases: std::path::PathBuf,
    #[arg(long)]
    pub case: Option<String>,
}

pub fn remote_count(args: RemoteArgs) -> Result<()> {
    let catalogue = MunicipalityCatalogue::load(&args.catalog)?;
    let sectors = load_sectors(&args.sectors)?;
    let last_day = Day::from_ymd(2026, 10, 11).expect("anchor");
    let client = HttpEngineClient::new(args.base, args.key, std::time::Duration::from_secs(args.timeout_seconds), args.concurrency)?;
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    let mut cases = Vec::new();
    for (case, request) in load_golden(&args.cases)? {
        if args.case.as_ref().is_some_and(|filter| !case.contains(filter.as_str())) {
            continue;
        }
        let valid = validate_request(request, &catalogue, &sectors, last_day)?;
        cases.push((case, valid.into_request()));
    }
    let totals = runtime.block_on(client.count_many(&cases.iter().map(|(_, request)| request.clone()).collect::<Vec<_>>(), &catalogue))?;
    for ((case, _), total) in cases.iter().zip(totals) {
        println!("{case}\t{total}");
    }
    Ok(())
}
