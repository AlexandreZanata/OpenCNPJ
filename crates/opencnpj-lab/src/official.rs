//! Discovery, import and query commands for official source-backed indicators.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail, ensure};
use clap::{Args, Subcommand};
use opencnpj_insights::{
    agg::{Aggregate, MUNICIPALITY_MISSING, Section, Table},
    catalog::municipalities::{MunicipalityCatalogue, Place, fold_name},
    official::{
        model::{CellStatus, DatasetManifest, Observation},
        query::{self, Answer, CategoryFilter, Request},
        registry::{Dataset, Registry},
        store::{StoreSet, missing_dataset_message, sha256_file, write_store},
    },
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

const AUDIT_PROBES_TOML: &str = include_str!("../../opencnpj-insights/official/audit-probes.toml");

mod ask;
mod xlsx;

#[derive(Subcommand)]
pub(crate) enum SourcesCommand {
    /// List the compiled source and dataset registry.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show registry states and locally published indicator assets.
    Status {
        /// Directory containing .ocind stores.
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Verify hashes, SQLite integrity and manifest row accounting.
    Verify {
        #[arg(long)]
        dir: PathBuf,
    },
}

#[derive(Subcommand)]
pub(crate) enum OfficialCommand {
    /// Fetch an IBGE aggregate through the official API and publish a .ocind store.
    ImportIbge(IbgeImportArgs),
    /// Import the RAIS municipal employment table from the official XLSX file.
    ImportRais(RaisImportArgs),
    /// Import the Senatran municipal fleet table from the official XLSX file.
    ImportSenatran(SenatranImportArgs),
    /// Audit CNPJ municipality marginals against the IBGE catalogue and official stores.
    AuditCnpj(AuditCnpjArgs),
    /// Revalidate every registered source and catalog downloaded samples exactly as received.
    AuditSources(AuditSourcesArgs),
    /// Execute an admitted indicator operation.
    Query(QueryArgs),
    /// Interpret a Portuguese question and execute it against published stores.
    Ask(AskArgs),
}

pub(crate) fn run_sources(command: SourcesCommand) -> Result<()> {
    match command {
        SourcesCommand::List { json } => list(json),
        SourcesCommand::Status { dir, json } => status(dir.as_deref(), json),
        SourcesCommand::Verify { dir } => verify(&dir),
    }
}

pub(crate) fn run_official(command: OfficialCommand) -> Result<()> {
    match command {
        OfficialCommand::ImportIbge(args) => import_ibge(args),
        OfficialCommand::ImportRais(args) => import_rais(args),
        OfficialCommand::ImportSenatran(args) => import_senatran(args),
        OfficialCommand::AuditCnpj(args) => audit_cnpj(args),
        OfficialCommand::AuditSources(args) => audit_sources(args),
        OfficialCommand::Query(args) => run_query(args),
        OfficialCommand::Ask(args) => run_ask(args),
    }
}

#[derive(Serialize)]
struct SourceRow {
    source_id: String,
    dataset_id: String,
    title: String,
    publisher: String,
    state: String,
    licence: String,
    licence_status: String,
    licence_url: String,
    attribution: String,
    cadence: String,
    coverage: String,
    imported: bool,
    rows: Option<u64>,
    period_from: Option<String>,
    period_to: Option<String>,
    raw_sha256: Option<String>,
}

fn rows(registry: &Registry, manifests: &[&DatasetManifest]) -> Vec<SourceRow> {
    let published = manifests.iter().map(|manifest| (manifest.dataset_id.as_str(), *manifest)).collect::<BTreeMap<_, _>>();
    registry
        .datasets()
        .map(|dataset| {
            let source = registry.source(&dataset.source_id).expect("validated registry source");
            let manifest = published.get(dataset.id.as_str());
            SourceRow {
                source_id: dataset.source_id.clone(),
                dataset_id: dataset.id.clone(),
                title: dataset.title.clone(),
                publisher: source.publisher.clone(),
                state: if manifest.is_some() { "queryable".to_string() } else { dataset.state.clone() },
                licence: source.licence.clone(),
                licence_status: source.licence_status.clone(),
                licence_url: source.licence_url.clone(),
                attribution: source.attribution.clone(),
                cadence: dataset.cadence.clone(),
                coverage: dataset.coverage.clone(),
                imported: manifest.is_some(),
                rows: manifest.map(|value| value.accepted_rows),
                period_from: manifest.and_then(|value| value.period_from.clone()),
                period_to: manifest.and_then(|value| value.period_to.clone()),
                raw_sha256: manifest.map(|value| value.raw_sha256.clone()),
            }
        })
        .collect()
}

fn list(json: bool) -> Result<()> {
    let registry = Registry::builtin()?;
    let rows = rows(&registry, &[]);
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }
    println!("source\tdataset\tstate\tlicence\tcadence\tcoverage");
    for row in rows {
        println!(
            "{}\t{}\t{}\t{} ({})\t{}\t{}",
            row.source_id, row.dataset_id, row.state, row.licence_status, row.licence_url, row.cadence, row.coverage
        );
    }
    println!("\nSources: {}", registry.sources().count());
    Ok(())
}

fn status(dir: Option<&Path>, json: bool) -> Result<()> {
    let registry = Registry::builtin()?;
    let stores = match dir {
        Some(dir) if dir.exists() => Some(StoreSet::open_dir(dir)?),
        Some(dir) => {
            eprintln!("{} does not exist; showing registry states only", dir.display());
            None
        }
        None => None,
    };
    let manifests =
        stores.as_ref().map(|stores| stores.stores().iter().map(|store| store.manifest()).collect::<Vec<_>>()).unwrap_or_default();
    let rows = rows(&registry, &manifests);
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!("source\tdataset\tstate\timported\trows\tperiod\tlicence");
        for row in &rows {
            let period = match (&row.period_from, &row.period_to) {
                (Some(from), Some(to)) => format!("{from}..{to}"),
                _ => "-".into(),
            };
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                row.source_id,
                row.dataset_id,
                row.state,
                if row.imported { "yes" } else { "no" },
                row.rows.map(|value| value.to_string()).unwrap_or_else(|| "-".into()),
                period,
                row.licence_status
            );
        }
    }
    Ok(())
}

fn verify(dir: &Path) -> Result<()> {
    let stores = StoreSet::open_dir(dir)?;
    stores.verify()?;
    for store in stores.stores() {
        let manifest = store.manifest();
        println!(
            "{}\t{}\trows={}\tperiod={:?}..{:?}\tsha256={}",
            store.path().display(),
            manifest.dataset_id,
            manifest.accepted_rows,
            manifest.period_from,
            manifest.period_to,
            sha256_file(store.path())?
        );
    }
    Ok(())
}

#[derive(Args)]
pub(crate) struct IbgeImportArgs {
    /// Registry dataset ID, for example ibge-6579.
    #[arg(long)]
    dataset: String,
    /// Source variable IDs separated by commas. Defaults to the curated variables.
    #[arg(long)]
    variable: Option<String>,
    /// Curated category IDs separated by commas. Defaults to all categories for the dataset.
    #[arg(long)]
    category: Option<String>,
    /// IBGE periods: `all`, `2024`, `2020-2023` or a comma-separated list.
    #[arg(long, default_value = "all")]
    periods: String,
    /// IBGE localities, for example `N3[51]|N6[N3[51]]`.
    #[arg(long)]
    localities: Option<String>,
    /// Destination .ocind path. Must not exist.
    #[arg(long)]
    out: PathBuf,
    /// Raw JSON evidence path. Defaults to `<out>.raw.json`; must not exist.
    #[arg(long)]
    raw_out: Option<PathBuf>,
    /// Retrieval timestamp recorded in the manifest.
    #[arg(long)]
    retrieved_at: Option<String>,
    #[arg(long, default_value_t = 3)]
    retries: u8,
    #[arg(long, default_value_t = 60)]
    timeout_seconds: u64,
}

#[derive(Args)]
pub(crate) struct RaisImportArgs {
    /// Local official XLSX file.
    #[arg(long)]
    xlsx: PathBuf,
    /// Destination .ocind path. Must not exist.
    #[arg(long)]
    out: PathBuf,
    /// Raw XLSX evidence path. Defaults to `<out>.raw.xlsx`; must not exist.
    #[arg(long)]
    raw_out: Option<PathBuf>,
    /// Reference year of the table.
    #[arg(long, default_value_t = 2024)]
    year: u16,
    /// Retrieval timestamp recorded in the manifest.
    #[arg(long)]
    retrieved_at: Option<String>,
}

#[derive(Args)]
pub(crate) struct SenatranImportArgs {
    /// Local official XLSX file.
    #[arg(long)]
    xlsx: PathBuf,
    /// Destination .ocind path. Must not exist.
    #[arg(long)]
    out: PathBuf,
    /// Raw XLSX evidence path. Defaults to `<out>.raw.xlsx`; must not exist.
    #[arg(long)]
    raw_out: Option<PathBuf>,
    /// Reference month written to the observations, as YYYYMM.
    #[arg(long, default_value = "202609")]
    period: String,
    /// State slice to import.
    #[arg(long, default_value = "MT")]
    uf: String,
    /// Retrieval timestamp recorded in the manifest.
    #[arg(long)]
    retrieved_at: Option<String>,
}

#[derive(Args)]
pub(crate) struct AuditCnpjArgs {
    /// Aggregate artifact built from the verified CNPJ snapshot.
    #[arg(long)]
    agg: PathBuf,
    /// Municipality catalogue used by the aggregate.
    #[arg(long)]
    catalog: PathBuf,
    /// Optional directory of official `.ocind` stores.
    #[arg(long)]
    stores: Option<PathBuf>,
    /// JSON audit output.
    #[arg(long)]
    out: PathBuf,
}

#[derive(Args)]
pub(crate) struct AuditSourcesArgs {
    /// Directory for raw audit downloads and the JSON report.
    #[arg(long)]
    out_dir: PathBuf,
    /// JSON audit report path.
    #[arg(long)]
    json_out: PathBuf,
    /// Only probe one source ID.
    #[arg(long)]
    only: Option<String>,
    #[arg(long, default_value_t = 60)]
    timeout_seconds: u64,
}

#[derive(Args)]
pub(crate) struct QueryArgs {
    #[arg(long)]
    dir: PathBuf,
    #[arg(long, default_value = "lookup")]
    operation: String,
    #[arg(long)]
    variable: String,
    #[arg(long)]
    denominator: Option<String>,
    #[arg(long = "geography")]
    geographies: Vec<String>,
    #[arg(long = "period")]
    periods: Vec<String>,
    #[arg(long)]
    classification: Option<String>,
    #[arg(long)]
    category: Option<String>,
    #[arg(long, default_value = "N6")]
    level: String,
    #[arg(long)]
    uf: Option<String>,
    #[arg(long, default_value_t = 10)]
    limit: usize,
    #[arg(long)]
    ascending: bool,
    #[arg(long, default_value_t = 1.0)]
    scale: f64,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
pub(crate) struct AskArgs {
    #[arg(long)]
    dir: Option<PathBuf>,
    #[arg(long)]
    text: String,
    #[arg(long, default_value = "crates/opencnpj-insights/catalog/municipalities.tsv")]
    catalog: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Deserialize)]
struct ApiVariable {
    id: String,
    variavel: String,
    unidade: String,
    #[serde(default)]
    resultados: Vec<ApiResult>,
}

#[derive(Debug, Deserialize)]
struct ApiResult {
    #[serde(default)]
    classificacoes: Vec<ApiClass>,
    #[serde(default)]
    series: Vec<ApiSeries>,
}

#[derive(Debug, Deserialize)]
struct ApiClass {
    id: String,
    nome: String,
    categoria: ApiCategory,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ApiCategory {
    Detailed { id: String, nome: String },
    Map(BTreeMap<String, String>),
}

impl ApiCategory {
    fn parts(&self) -> (String, String) {
        match self {
            Self::Detailed { id, nome } => (id.clone(), nome.clone()),
            Self::Map(values) => {
                values.iter().next().map_or_else(|| (String::new(), String::new()), |(id, nome)| (id.clone(), nome.clone()))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct ApiSeries {
    localidade: ApiPlace,
    serie: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct ApiPlace {
    id: String,
    nivel: ApiLevel,
    nome: String,
}

#[derive(Debug, Deserialize)]
struct ApiLevel {
    id: String,
}

fn import_ibge(args: IbgeImportArgs) -> Result<()> {
    ensure!(args.retries > 0, "retries must be positive");
    ensure!(args.timeout_seconds > 0, "timeout must be positive");
    let registry = Registry::builtin()?;
    let dataset = registry.dataset(&args.dataset).with_context(|| format!("unknown registry dataset {}", args.dataset))?.clone();
    let variables = selected_variables(&registry, &dataset, args.variable.as_deref())?;
    let categories = selected_categories(&registry, &dataset, args.category.as_deref())?;
    let localities = args.localities.unwrap_or_else(|| dataset.default_localities.clone().unwrap_or_else(|| "N6[N3[51]]".into()));
    let raw_path = args.raw_out.clone().unwrap_or_else(|| {
        let mut path = args.out.clone().into_os_string();
        path.push(".raw.json");
        PathBuf::from(path)
    });
    ensure!(!args.out.exists(), "destination {} already exists", args.out.display());
    ensure!(!raw_path.exists(), "raw evidence {} already exists", raw_path.display());
    if let Some(parent) = raw_path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let retrieved_at = args.retrieved_at.unwrap_or_else(now);
    let runtime = tokio::runtime::Runtime::new().context("create async runtime")?;
    let client = reqwest::Client::builder().timeout(Duration::from_secs(args.timeout_seconds)).build()?;

    let requests = if categories.is_empty() {
        vec![(None, variables.clone())]
    } else {
        categories.iter().map(|category| (Some(category.clone()), variables.clone())).collect()
    };
    let mut observations = BTreeMap::new();
    let mut raw_files = Vec::new();
    let mut total_cells = 0_u64;
    let mut duplicate_rows = 0_u64;
    let mut malformed_rows = 0_u64;
    let (mut period_from, mut period_to): (Option<String>, Option<String>) = (None, None);
    let mut units = BTreeSet::new();
    for (index, (category, variables)) in requests.iter().enumerate() {
        let (url, bytes) =
            runtime.block_on(fetch_ibge(&client, &dataset, variables, category.as_ref(), &args.periods, &localities, args.retries))?;
        let parsed = parse_ibge(&bytes, &dataset, &registry, &url)?;
        total_cells += parsed.cells;
        duplicate_rows += parsed.duplicates;
        malformed_rows += parsed.malformed;
        units.extend(parsed.units);
        for observation in parsed.observations {
            if observations.insert(observation.key(), observation).is_some() {
                duplicate_rows += 1;
            }
        }
        let segment = if index == 0 { "raw.json" } else { &format!("raw-{index}.json") };
        let path = raw_path_for(&raw_path, segment, index);
        let sha = write_raw(&path, &bytes)?;
        raw_files.push((path, sha, bytes.len()));
    }
    for observation in observations.values() {
        period_from = Some(period_from.map_or_else(|| observation.period.clone(), |current| current.min(observation.period.clone())));
        period_to = Some(period_to.map_or_else(|| observation.period.clone(), |current| current.max(observation.period.clone())));
    }
    let raw_sha256 = match raw_files.as_slice() {
        [(_, sha, _)] => sha.clone(),
        _ => {
            let mut hasher = Sha256::new();
            for (_, sha, _) in &raw_files {
                hasher.update(sha.as_bytes());
            }
            hex::encode(hasher.finalize())
        }
    };
    let raw_bytes = raw_files.iter().map(|(_, _, bytes)| *bytes as u64).sum();
    let accepted_rows = observations.len() as u64;
    ensure!(accepted_rows + duplicate_rows == total_cells, "row accounting mismatch after import");
    let source = registry.source(&dataset.source_id).context("dataset source")?;
    let unit = units.iter().cloned().collect::<Vec<_>>().join("; ");
    let manifest = DatasetManifest {
        format_version: 1,
        source_id: dataset.source_id.clone(),
        dataset_id: dataset.id.clone(),
        title: dataset.title.clone(),
        publisher: source.publisher.clone(),
        source_url: dataset.api_url.clone(),
        metadata_url: dataset.metadata_url.clone(),
        licence: source.licence.clone(),
        licence_url: source.licence_url.clone(),
        licence_status: source.licence_status.clone(),
        attribution: source.attribution.clone(),
        retrieved_at,
        publication: None,
        coverage: dataset.coverage.clone(),
        unit,
        period_from,
        period_to,
        raw_sha256,
        raw_bytes,
        row_count: total_cells,
        accepted_rows,
        rejected_rows: duplicate_rows,
        duplicate_rows,
        malformed_rows,
        state: "queryable".into(),
        limitations: dataset.limitations.clone(),
    };
    let published = write_store(&args.out, &manifest, observations.into_values())?;
    for (path, sha, bytes) in raw_files {
        println!("raw\t{}\tbytes={bytes}\tsha256={sha}", path.display());
    }
    println!(
        "published\t{}\trows={}\tduplicates={}\tmalformed={}\tbytes={}\tsha256={}",
        published.path.display(),
        published.manifest.accepted_rows,
        published.manifest.duplicate_rows,
        published.manifest.malformed_rows,
        published.bytes,
        published.sha256
    );
    Ok(())
}

fn import_rais(args: RaisImportArgs) -> Result<()> {
    ensure!(!args.out.exists(), "destination {} already exists", args.out.display());
    let raw_path = args.raw_out.clone().unwrap_or_else(|| {
        let mut path = args.out.clone().into_os_string();
        path.push(".raw.xlsx");
        PathBuf::from(path)
    });
    ensure!(!raw_path.exists(), "raw evidence {} already exists", raw_path.display());
    if let Some(parent) = raw_path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let bytes = fs::read(&args.xlsx).with_context(|| format!("read {}", args.xlsx.display()))?;
    let raw_sha256 = write_raw(&raw_path, &bytes)?;
    let workbook = xlsx::Workbook::open(&raw_path)?;
    if !workbook.sheet_names().any(|name| name == "TABELA 4") {
        bail!("RAIS workbook has no TABELA 4 sheet");
    }
    let rows = workbook.rows("TABELA 4")?;
    let header = rows.get(12).context("RAIS sheet has no header row")?;
    ensure!(
        xlsx::cell(header, 1) == "UF" && xlsx::cell(header, 2) == "Código" && xlsx::cell(header, 3) == "Município",
        "unexpected RAIS header: {header:?}"
    );
    let catalogue = MunicipalityCatalogue::load(Path::new("crates/opencnpj-insights/catalog/municipalities.tsv"))?;
    let bridge = catalogue
        .rows()
        .iter()
        .filter_map(|row| match &row.place {
            Place::Municipality(place) if place.uf == "MT" => Some((place.ibge / 10, place.ibge)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let registry = Registry::builtin()?;
    let dataset = registry.dataset("mte-rais-2024").context("RAIS dataset registry entry")?.clone();
    let source = registry.source("mte").context("MTE source registry entry")?;
    let columns = [
        ("rais_emprego_agropecuaria", 4_usize, 5_usize),
        ("rais_emprego_industria", 8, 9),
        ("rais_emprego_construcao", 12, 13),
        ("rais_emprego_comercio", 16, 17),
        ("rais_emprego_servicos", 20, 21),
        ("rais_emprego_total", 24, 25),
    ];
    let mut observations = Vec::new();
    let mut total_cells = 0_u64;
    let mut rejected_rows = 0_u64;
    let mut malformed_rows = 0_u64;
    let mut unmatched = Vec::new();
    for row in rows.iter().skip(15) {
        if xlsx::cell(row, 1).trim() != "MT" {
            continue;
        }
        let code = xlsx::cell(row, 2).trim();
        let name = xlsx::cell(row, 3).trim();
        let key = xlsx::parse_u32(code).context("RAIS municipality code")?;
        let Some(ibge) = bridge.get(&key).copied() else {
            total_cells += 12;
            rejected_rows += 12;
            unmatched.push(format!("{code} {name}"));
            continue;
        };
        for (variable_id, column_2023, column_2024) in columns {
            let variable = registry.variable(variable_id).context("RAIS curated variable")?;
            for (column, period) in [(column_2023, "2023"), (column_2024, args.year.to_string().as_str())] {
                total_cells += 1;
                let raw = xlsx::cell(row, column).trim();
                if raw.is_empty() || raw == ".." || raw == "..." {
                    observations.push(Observation {
                        dataset_id: dataset.id.clone(),
                        variable_id: variable.id.clone(),
                        variable_label: variable.label.clone(),
                        unit: variable.unit.clone(),
                        classification_id: String::new(),
                        classification_label: String::new(),
                        category_id: String::new(),
                        category_label: String::new(),
                        geography_level: "N6".into(),
                        geography_code: format!("{ibge:07}"),
                        geography_name: name.to_string(),
                        period: period.to_string(),
                        value: None,
                        status: CellStatus::NotAvailable,
                        source_url: dataset.api_url.clone(),
                    });
                    continue;
                }
                let (status, value) = rais_cell(raw);
                if status == CellStatus::Malformed {
                    malformed_rows += 1;
                }
                observations.push(Observation {
                    dataset_id: dataset.id.clone(),
                    variable_id: variable.id.clone(),
                    variable_label: variable.label.clone(),
                    unit: variable.unit.clone(),
                    classification_id: String::new(),
                    classification_label: String::new(),
                    category_id: String::new(),
                    category_label: String::new(),
                    geography_level: "N6".into(),
                    geography_code: format!("{ibge:07}"),
                    geography_name: name.to_string(),
                    period: period.to_string(),
                    value,
                    status,
                    source_url: dataset.api_url.clone(),
                });
            }
        }
    }
    let accepted_rows = observations.len() as u64;
    ensure!(accepted_rows + rejected_rows == total_cells, "RAIS row accounting mismatch");
    let mut limitations = dataset.limitations.clone();
    if !unmatched.is_empty() {
        limitations.push(format!(
            "{} MT rows had no IBGE municipality bridge and were not stored: {}.",
            unmatched.len(),
            unmatched.join(", ")
        ));
    }
    let manifest = DatasetManifest {
        format_version: 1,
        source_id: dataset.source_id.clone(),
        dataset_id: dataset.id.clone(),
        title: dataset.title.clone(),
        publisher: source.publisher.clone(),
        source_url: dataset.api_url.clone(),
        metadata_url: dataset.metadata_url.clone(),
        licence: source.licence.clone(),
        licence_url: source.licence_url.clone(),
        licence_status: source.licence_status.clone(),
        attribution: source.attribution.clone(),
        retrieved_at: args.retrieved_at.unwrap_or_else(now),
        publication: None,
        coverage: "Mato Grosso municipalities (RAIS 2024)".into(),
        unit: "Vínculos".into(),
        period_from: Some("2023".into()),
        period_to: Some(args.year.to_string()),
        raw_sha256,
        raw_bytes: bytes.len() as u64,
        row_count: total_cells,
        accepted_rows,
        rejected_rows,
        duplicate_rows: 0,
        malformed_rows,
        state: "queryable".into(),
        limitations,
    };
    let published = write_store(&args.out, &manifest, observations)?;
    println!("raw	{}	bytes={}	sha256={}", raw_path.display(), bytes.len(), published.manifest.raw_sha256);
    println!(
        "published	{}	rows={}	rejected={}	malformed={}	bytes={}	sha256={}",
        published.path.display(),
        published.manifest.accepted_rows,
        published.manifest.rejected_rows,
        published.manifest.malformed_rows,
        published.bytes,
        published.sha256
    );
    Ok(())
}

fn rais_cell(raw: &str) -> (CellStatus, Option<f64>) {
    match raw {
        "" | ".." | "..." => (CellStatus::NotAvailable, None),
        "X" | "x" => (CellStatus::Suppressed, None),
        "-" => (CellStatus::Observed, Some(0.0)),
        _ => match parse_number(raw) {
            Some(value) => (CellStatus::Observed, Some(value)),
            None => (CellStatus::Malformed, None),
        },
    }
}

fn import_senatran(args: SenatranImportArgs) -> Result<()> {
    ensure!(!args.out.exists(), "destination {} already exists", args.out.display());
    ensure!(args.period.len() == 6 && args.period.chars().all(|character| character.is_ascii_digit()), "period must be YYYYMM");
    let raw_path = args.raw_out.clone().unwrap_or_else(|| {
        let mut path = args.out.clone().into_os_string();
        path.push(".raw.xlsx");
        PathBuf::from(path)
    });
    ensure!(!raw_path.exists(), "raw evidence {} already exists", raw_path.display());
    if let Some(parent) = raw_path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let bytes = fs::read(&args.xlsx).with_context(|| format!("read {}", args.xlsx.display()))?;
    let raw_sha256 = write_raw(&raw_path, &bytes)?;
    let workbook = xlsx::Workbook::open(&raw_path)?;
    let sheet = workbook.sheet_names().next().context("Senatran workbook has no sheet")?.to_string();
    let rows = workbook.rows(&sheet)?;
    let header = rows.get(2).context("Senatran sheet has no header row")?;
    ensure!(xlsx::cell(header, 0) == "UF" && xlsx::cell(header, 1) == "MUNICIPIO", "unexpected Senatran header");
    let mut columns = Vec::new();
    for column in 2..header.len() {
        let label = xlsx::cell(header, column).trim();
        if label.is_empty() {
            continue;
        }
        let slug = slug(label);
        let source_variable = label.to_string();
        let variable = registry_variable_for_senatran(&source_variable);
        columns.push((column, source_variable, slug, variable));
    }
    ensure!(!columns.is_empty(), "Senatran sheet has no vehicle columns");
    let catalogue = MunicipalityCatalogue::load(Path::new("crates/opencnpj-insights/catalog/municipalities.tsv"))?;
    let mut by_alias: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    let mut official_name: BTreeMap<u32, String> = BTreeMap::new();
    for row in catalogue.rows() {
        let Place::Municipality(place) = &row.place else { continue };
        if place.uf != args.uf {
            continue;
        }
        official_name.insert(place.ibge, row.name.clone());
        by_alias.entry(row.folded.clone()).or_default().push(place.ibge);
        for variant in &row.variants {
            by_alias.entry(variant.clone()).or_default().push(place.ibge);
        }
    }
    let registry = Registry::builtin()?;
    let dataset = registry.dataset("senatran-frota-2026-09").context("Senatran dataset registry entry")?.clone();
    let source = registry.source("senatran").context("Senatran source registry entry")?;
    let mut observations = Vec::new();
    let mut total_cells = 0_u64;
    let mut rejected_rows = 0_u64;
    let mut malformed_rows = 0_u64;
    let mut unmatched = Vec::new();
    for row in rows.iter().skip(3) {
        if xlsx::cell(row, 0).trim() != args.uf {
            continue;
        }
        let name = xlsx::cell(row, 1).trim();
        let folded = senatran_alias(&fold_name(name));
        let aliases = by_alias.get(&folded).cloned().unwrap_or_default();
        let code = match aliases.as_slice() {
            [code] => *code,
            _ => {
                total_cells += columns.len() as u64;
                rejected_rows += columns.len() as u64;
                unmatched.push(name.to_string());
                continue;
            }
        };
        let geography_name = official_name.get(&code).cloned().unwrap_or_else(|| name.to_string());
        for (column, source_variable, slug, variable) in &columns {
            total_cells += 1;
            let raw = xlsx::cell(row, *column).trim();
            let (status, value) = parse_cell(raw);
            if status == CellStatus::Malformed {
                malformed_rows += 1;
            }
            let variable_id = variable.as_ref().map_or_else(|| format!("senatran_{slug}"), |variable| variable.id.clone());
            let variable_label = variable.as_ref().map_or_else(|| source_variable.clone(), |variable| variable.label.clone());
            let unit = variable.as_ref().map_or_else(|| "Veículos".to_string(), |variable| variable.unit.clone());
            observations.push(Observation {
                dataset_id: dataset.id.clone(),
                variable_id,
                variable_label,
                unit,
                classification_id: String::new(),
                classification_label: String::new(),
                category_id: String::new(),
                category_label: String::new(),
                geography_level: "N6".into(),
                geography_code: format!("{code:07}"),
                geography_name: geography_name.clone(),
                period: args.period.clone(),
                value,
                status,
                source_url: dataset.api_url.clone(),
            });
        }
    }
    let accepted_rows = observations.len() as u64;
    ensure!(accepted_rows + rejected_rows == total_cells, "Senatran row accounting mismatch");
    let mut limitations = dataset.limitations.clone();
    if !unmatched.is_empty() {
        limitations.push(format!(
            "{} {} rows had no unique IBGE municipality bridge and were not stored: {}.",
            unmatched.len(),
            args.uf,
            unmatched.join(", ")
        ));
    }
    let manifest = DatasetManifest {
        format_version: 1,
        source_id: dataset.source_id.clone(),
        dataset_id: dataset.id.clone(),
        title: dataset.title.clone(),
        publisher: source.publisher.clone(),
        source_url: dataset.api_url.clone(),
        metadata_url: dataset.metadata_url.clone(),
        licence: source.licence.clone(),
        licence_url: source.licence_url.clone(),
        licence_status: source.licence_status.clone(),
        attribution: source.attribution.clone(),
        retrieved_at: args.retrieved_at.unwrap_or_else(now),
        publication: None,
        coverage: format!("Mato Grosso municipalities ({})", args.period),
        unit: "Veículos".into(),
        period_from: Some(args.period.clone()),
        period_to: Some(args.period.clone()),
        raw_sha256,
        raw_bytes: bytes.len() as u64,
        row_count: total_cells,
        accepted_rows,
        rejected_rows,
        duplicate_rows: 0,
        malformed_rows,
        state: "queryable".into(),
        limitations,
    };
    let published = write_store(&args.out, &manifest, observations)?;
    println!("raw	{}	bytes={}	sha256={}", raw_path.display(), bytes.len(), published.manifest.raw_sha256);
    println!(
        "published	{}	rows={}	rejected={}	malformed={}	bytes={}	sha256={}",
        published.path.display(),
        published.manifest.accepted_rows,
        published.manifest.rejected_rows,
        published.manifest.malformed_rows,
        published.bytes,
        published.sha256
    );
    Ok(())
}

fn registry_variable_for_senatran(source_variable: &str) -> Option<open_cnpj_variable::RegisteredVariable> {
    let registry = Registry::builtin().ok()?;
    registry
        .variables()
        .find(|variable| variable.dataset_id == "senatran-frota-2026-09" && variable.source_variable == source_variable)
        .map(|variable| open_cnpj_variable::RegisteredVariable {
            id: variable.id.clone(),
            label: variable.label.clone(),
            unit: variable.unit.clone(),
        })
}

mod open_cnpj_variable {
    pub struct RegisteredVariable {
        pub id: String,
        pub label: String,
        pub unit: String,
    }
}

fn senatran_alias(name: &str) -> String {
    match name {
        "vila bela da santissima trinda" => "vila bela da santissima trindade".into(),
        other => other.to_string(),
    }
}

fn slug(value: &str) -> String {
    value
        .chars()
        .map(|character| if character.is_ascii_alphanumeric() { character.to_ascii_lowercase() } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

#[derive(Debug, Deserialize)]
struct ProbeFile {
    version: u32,
    #[serde(default)]
    probe: Vec<Probe>,
}

#[derive(Debug, Clone, Deserialize)]
struct Probe {
    source_id: String,
    category: String,
    label: String,
    url: String,
    kind: String,
    max_bytes: usize,
    #[serde(default)]
    note: String,
}

fn audit_sources(args: AuditSourcesArgs) -> Result<()> {
    ensure!(args.timeout_seconds > 0, "timeout must be positive");
    let file: ProbeFile = toml::from_str(AUDIT_PROBES_TOML).context("parse audit probes")?;
    ensure!(file.version == 1, "unsupported audit probe version {}", file.version);
    let registry = Registry::builtin()?;
    for probe in &file.probe {
        ensure!(registry.source(&probe.source_id).is_some(), "probe references unknown source {}", probe.source_id);
        ensure!(probe.url.starts_with("https://"), "audit probe must use HTTPS: {}", probe.url);
        ensure!(probe.max_bytes > 0, "audit probe needs a positive max_bytes");
    }
    let probes = file.probe.into_iter().filter(|probe| args.only.as_ref().is_none_or(|only| &probe.source_id == only)).collect::<Vec<_>>();
    ensure!(!probes.is_empty(), "no audit probes selected");
    let runtime = tokio::runtime::Runtime::new().context("create async runtime")?;
    let client = reqwest::Client::builder().timeout(Duration::from_secs(args.timeout_seconds)).build()?;
    let mut results = Vec::new();
    for probe in probes {
        results.push(runtime.block_on(run_probe(&client, &probe, &args.out_dir)));
    }
    let mut by_category: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_format: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_source: BTreeMap<String, usize> = BTreeMap::new();
    let mut errors = 0_usize;
    for result in &results {
        *by_category.entry(result["category"].as_str().unwrap_or("unknown").into()).or_default() += 1;
        *by_format.entry(result["format"].as_str().unwrap_or("unknown").into()).or_default() += 1;
        *by_source.entry(result["source_id"].as_str().unwrap_or("unknown").into()).or_default() += 1;
        if result.get("error").is_some()
            || result.get("http_status").and_then(serde_json::Value::as_u64).is_some_and(|status| !(200..300).contains(&status))
        {
            errors += 1;
        }
    }
    let report = json!({
        "generated_at": now(),
        "source_count": registry.sources().count(),
        "dataset_count": registry.datasets().count(),
        "probe_count": results.len(),
        "error_count": errors,
        "by_category": by_category,
        "by_format": by_format,
        "by_source": by_source,
        "sources": registry.sources().map(|source| json!({
            "source_id": source.id,
            "category": source.category,
            "title": source.title,
            "publisher": source.publisher,
            "state": source.state,
            "licence": source.licence,
            "licence_status": source.licence_status,
            "licence_url": source.licence_url,
        })).collect::<Vec<_>>(),
        "datasets": registry.datasets().map(|dataset| json!({
            "dataset_id": dataset.id,
            "source_id": dataset.source_id,
            "category": dataset.category,
            "title": dataset.title,
            "state": dataset.state,
            "cadence": dataset.cadence,
            "coverage": dataset.coverage,
        })).collect::<Vec<_>>(),
        "probes": results,
    });
    if let Some(parent) = args.json_out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.json_out, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "audit written to {}: {} probes, {} errors; categories {:?}; formats {:?}",
        args.json_out.display(),
        report["probe_count"].as_u64().unwrap_or(0),
        errors,
        report["by_category"],
        report["by_format"]
    );
    Ok(())
}

async fn run_probe(client: &reqwest::Client, probe: &Probe, out_dir: &Path) -> serde_json::Value {
    let response = client.get(&probe.url).send().await;
    let mut response = match response {
        Ok(response) => response,
        Err(error) => {
            return json!({
                "source_id": probe.source_id,
                "category": probe.category,
                "label": probe.label,
                "url": probe.url,
                "kind": probe.kind,
                "note": probe.note,
                "error": error.to_string(),
                "format": "unavailable",
                "bytes": 0,
                "truncated": false,
            });
        }
    };
    let status = response.status();
    let content_type =
        response.headers().get(reqwest::header::CONTENT_TYPE).and_then(|value| value.to_str().ok()).unwrap_or("").to_string();
    let content_length = response.content_length();
    let max_bytes = probe.max_bytes;
    let mut bytes = Vec::new();
    let mut truncated = false;
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if bytes.len() + chunk.len() > max_bytes {
                    bytes.extend_from_slice(&chunk[..max_bytes - bytes.len()]);
                    truncated = true;
                    break;
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(error) => {
                return json!({
                    "source_id": probe.source_id,
                    "category": probe.category,
                    "label": probe.label,
                    "url": probe.url,
                    "kind": probe.kind,
                    "note": probe.note,
                    "http_status": status.as_u16(),
                    "content_type": content_type,
                    "error": error.to_string(),
                    "format": "unavailable",
                    "bytes": bytes.len(),
                    "truncated": truncated,
                });
            }
        }
    }
    if content_length.is_none() && bytes.len() == max_bytes {
        truncated = true;
    }
    let (format, extension, schema) = classify_payload(&bytes, &content_type, truncated, &probe.url);
    let source_dir = out_dir.join("raw").join(&probe.source_id);
    if let Err(error) = fs::create_dir_all(&source_dir) {
        return json!({"source_id": probe.source_id, "error": error.to_string(), "format": format});
    }
    let path = source_dir.join(format!("{}.{extension}", sanitize_label(&probe.label)));
    let saved = if status.is_success() && !bytes.is_empty() {
        fs::write(&path, &bytes).map(|_| path.display().to_string()).map_err(|error| error.to_string())
    } else {
        Err("empty or failed response".to_string())
    };
    let bytes_sha256 = {
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        hex::encode(hasher.finalize())
    };
    let saved = match saved {
        Ok(path) => json!(path),
        Err(error) => json!({"error": error}),
    };
    json!({
        "source_id": probe.source_id,
        "category": probe.category,
        "label": probe.label,
        "url": probe.url,
        "kind": probe.kind,
        "note": probe.note,
        "http_status": status.as_u16(),
        "content_type": content_type,
        "content_length": content_length,
        "bytes": bytes.len(),
        "truncated": truncated,
        "sha256": bytes_sha256,
        "format": format,
        "schema": schema,
        "file": saved,
    })
}

fn extract_resource_links(text: &str, base_url: &str) -> Vec<serde_json::Value> {
    let base = reqwest::Url::parse(base_url).ok();
    let mut links = BTreeSet::new();
    for attribute in ["href=\"", "src=\""] {
        let mut rest = text;
        while let Some(start) = rest.find(attribute) {
            rest = &rest[start + attribute.len()..];
            let Some(end) = rest.find('"') else { break };
            let raw = rest[..end].trim();
            rest = &rest[end + 1..];
            if raw.is_empty() || raw.starts_with('#') || raw.starts_with("javascript:") || raw.starts_with("mailto:") {
                continue;
            }
            let url = if raw.starts_with("http://") || raw.starts_with("https://") {
                raw.to_string()
            } else if let Some(base) = &base {
                match base.join(raw) {
                    Ok(url) => url.to_string(),
                    Err(_) => continue,
                }
            } else {
                continue;
            };
            let lower = url.to_ascii_lowercase();
            let extension = ["csv", "zip", "xlsx", "xls", "json", "pdf", "7z", "txt", "ods"]
                .into_iter()
                .find(|extension| lower.contains(&format!(".{extension}")))
                .unwrap_or("");
            if extension.is_empty() {
                continue;
            }
            links.insert((url, extension.to_string()));
            if links.len() >= 50 {
                break;
            }
        }
    }
    links.into_iter().map(|(url, extension)| json!({"url": url, "extension": extension})).collect()
}

fn sanitize_label(value: &str) -> String {
    value
        .chars()
        .map(|character| if character.is_ascii_alphanumeric() || character == '-' || character == '_' { character } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn classify_payload(bytes: &[u8], content_type: &str, truncated: bool, base_url: &str) -> (String, String, serde_json::Value) {
    let lower_type = content_type.to_ascii_lowercase();
    if bytes.starts_with(b"PK\x03\x04") {
        let mut archive = match zip::ZipArchive::new(std::io::Cursor::new(bytes)) {
            Ok(archive) => archive,
            Err(error) => return ("zip_partial_or_invalid".into(), "zip".into(), json!({"error": error.to_string()})),
        };
        let names = (0..archive.len())
            .filter_map(|index| archive.by_index(index).ok().and_then(|file| file.name().ok().map(|name| name.into_owned())))
            .collect::<Vec<_>>();
        let xlsx = names.iter().any(|name| name == "xl/workbook.xml");
        return (
            if xlsx { "xlsx" } else { "zip" }.into(),
            if xlsx { "xlsx" } else { "zip" }.into(),
            json!({"zip_entries": names.len(), "has_workbook": xlsx, "first_entries": names.iter().take(12).collect::<Vec<_>>()}),
        );
    }
    if bytes.starts_with(b"%PDF") {
        return ("pdf".into(), "pdf".into(), json!({"note": "PDF body not parsed"}));
    }
    if bytes.starts_with(b"7z\xBC\xAF\x27\x1C") {
        return ("7z".into(), "7z".into(), json!({"note": "7z body not parsed"}));
    }
    let text = std::str::from_utf8(bytes).ok();
    let head_lossy = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]).to_string();
    let trimmed_head = head_lossy.trim_start();
    let trimmed = text.map(str::trim_start).unwrap_or(trimmed_head);
    let lower_head = trimmed_head.get(..512).unwrap_or(trimmed_head).to_ascii_lowercase();
    if lower_type.contains("text/html")
        || lower_type.contains("application/xhtml")
        || lower_head.starts_with("<!doctype")
        || lower_head.starts_with("<html")
        || lower_head.contains("<html")
    {
        let title = trimmed.find("<title>").and_then(|start| {
            let rest = &trimmed[start + 7..];
            rest.find("</title>").map(|end| rest[..end].trim().to_string())
        });
        let links = text.map(|text| extract_resource_links(text, base_url)).unwrap_or_default();
        return ("html".into(), "html".into(), json!({"title": title, "content_type": content_type, "candidate_resources": links}));
    }
    if trimmed_head.starts_with('{') || trimmed_head.starts_with('[') {
        return match serde_json::from_slice::<serde_json::Value>(bytes) {
            Ok(value) => {
                let schema = if let Some(array) = value.as_array() {
                    json!({"json_type": "array", "items": array.len(), "first_keys": array.first().and_then(|item| item.as_object()).map(|object| object.keys().take(20).cloned().collect::<Vec<_>>())})
                } else if let Some(object) = value.as_object() {
                    json!({"json_type": "object", "keys": object.keys().take(30).collect::<Vec<_>>()})
                } else {
                    json!({"json_type": "scalar"})
                };
                ("json".into(), "json".into(), schema)
            }
            Err(error) => ("json_partial_or_invalid".into(), "json".into(), json!({"error": error.to_string(), "truncated": truncated})),
        };
    }
    if trimmed_head.starts_with('<') {
        return ("xml_or_markup".into(), "xml".into(), json!({"content_type": content_type}));
    }
    if let Some(text) = text {
        let first_line = text.lines().next().unwrap_or("");
        let semicolons = first_line.matches(';').count();
        let commas = first_line.matches(',').count();
        let tabs = first_line.matches('\t').count();
        let (format, extension) = if semicolons >= 1 {
            ("csv_semicolon", "csv")
        } else if tabs >= 1 {
            ("tsv", "tsv")
        } else if commas >= 1 {
            ("csv_comma", "csv")
        } else {
            ("text", "txt")
        };
        return (
            format.into(),
            extension.into(),
            json!({"first_line": first_line.chars().take(300).collect::<String>(), "columns_guess": semicolons.max(commas).max(tabs) + 1, "content_type": content_type, "truncated": truncated}),
        );
    }
    ("binary".into(), "bin".into(), json!({"content_type": content_type, "truncated": truncated, "lower_type": lower_type}))
}

fn normalized_geo_name(name: &str) -> String {
    let mut value = name.trim();
    if let Some((left, right)) = value.rsplit_once(" - ")
        && (right == "MT" || (right.len() == 2 && right.chars().all(|character| character.is_ascii_uppercase())))
    {
        value = left.trim();
    }
    fold_name(value)
}

fn audit_cnpj(args: AuditCnpjArgs) -> Result<()> {
    let aggregate = Aggregate::open(&args.agg)?;
    let catalogue = MunicipalityCatalogue::load(&args.catalog)?;
    let registry = Registry::builtin()?;
    let mut counts: BTreeMap<u16, u64> = BTreeMap::new();
    let mut missing_municipality = 0_u64;
    let mut unmatched_ids = Vec::new();
    let mut totals = BTreeMap::new();
    for table in [Table::T1, Table::T2] {
        let mut total = 0_u64;
        for cell in aggregate.cells(Section::Municipality(table)) {
            let cell = cell?;
            total += cell.count;
            if cell.key == u64::from(MUNICIPALITY_MISSING) {
                missing_municipality += cell.count;
                continue;
            }
            let Ok(id) = u16::try_from(cell.key) else {
                unmatched_ids.push(json!({"table": format!("{table:?}"), "id": cell.key, "reason": "id exceeds u16"}));
                continue;
            };
            if catalogue.get(id).is_none() {
                unmatched_ids.push(json!({"table": format!("{table:?}"), "id": id, "reason": "id absent from catalogue"}));
                continue;
            }
            *counts.entry(id).or_default() += cell.count;
        }
        totals.insert(format!("{table:?}"), total);
    }

    let mut duplicate_names = Vec::new();
    let mut names: BTreeMap<(String, String), Vec<u16>> = BTreeMap::new();
    for row in catalogue.rows() {
        let Place::Municipality(place) = &row.place else { continue };
        names.entry((place.uf.clone(), row.folded.clone())).or_default().push(row.id);
    }
    for ((uf, name), ids) in names {
        if ids.len() > 1 {
            duplicate_names.push(json!({"uf": uf, "name": name, "ids": ids}));
        }
    }
    let zero_cnpj = catalogue
        .rows()
        .iter()
        .filter(|row| matches!(&row.place, Place::Municipality(_)) && counts.get(&row.id).copied().unwrap_or(0) == 0)
        .map(|row| json!({"id": row.id, "tom": format!("{:04}", row.tom), "name": row.name}))
        .collect::<Vec<_>>();
    let census_missing = catalogue
        .rows()
        .iter()
        .filter_map(|row| match &row.place {
            Place::Municipality(place) if place.census_2022.is_none() => Some(json!({"id": row.id, "ibge": place.ibge, "name": row.name})),
            _ => None,
        })
        .collect::<Vec<_>>();

    let mt_catalogue = catalogue
        .rows()
        .iter()
        .filter_map(|row| match &row.place {
            Place::Municipality(place) if place.uf == "MT" => Some((place.ibge, row.name.clone())),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut inconsistencies = Vec::new();
    let mut push = |kind: &str, severity: &str, entity: String, detail: String, action: String| {
        inconsistencies.push(json!({
            "kind": kind,
            "severity": severity,
            "entity": entity,
            "detail": detail,
            "suggested_action": action
        }));
    };
    if missing_municipality > 0 {
        push(
            "aggregate_missing_municipality",
            "high",
            "aggregate".into(),
            format!("{missing_municipality} cells use the missing municipality sentinel"),
            "Decide whether these records are out of scope or need a bridge; never assign them to a real municipality by name.".into(),
        );
    }
    for unmatched in &unmatched_ids {
        push(
            "cnpj_municipality_not_in_catalogue",
            "high",
            unmatched["id"].to_string(),
            unmatched.to_string(),
            "Rebuild the aggregate with the matching municipality catalogue version or bridge the identifier explicitly.".into(),
        );
    }
    for duplicate in &duplicate_names {
        push(
            "catalogue_duplicate_name_same_uf",
            "high",
            duplicate["uf"].to_string(),
            duplicate.to_string(),
            "Disambiguate by IBGE code; never resolve a name-only match silently.".into(),
        );
    }
    for row in &census_missing {
        push(
            "catalogue_census_missing",
            "info",
            row["name"].as_str().unwrap_or_default().into(),
            row.to_string(),
            "Keep the Census cell absent; do not backfill it from the population estimate.".into(),
        );
    }
    if !zero_cnpj.is_empty() {
        push(
            "catalogue_municipality_without_cnpj",
            "low",
            format!("{} municipalities", zero_cnpj.len()),
            "Municipalities exist in the IBGE catalogue but have zero active CNPJ establishments in T1.".into(),
            "Keep the zero or missing distinction; no synthetic establishment should be created.".into(),
        );
    }

    let mut source_reports = Vec::new();
    let mut normalization_tasks = Vec::new();
    if let Some(directory) = &args.stores {
        let stores = StoreSet::open_dir(directory)?;
        for store in stores.stores() {
            let manifest = store.manifest();
            let geographies = store.geographies()?;
            let mut unmatched = Vec::new();
            let mut name_mismatches = Vec::new();
            let mut without_cnpj = Vec::new();
            let mut non_municipal = Vec::new();
            for (level, code, name) in &geographies {
                if level != "N6" {
                    non_municipal.push(json!({"level": level, "code": code, "name": name}));
                    continue;
                }
                let Ok(ibge) = code.parse::<u32>() else {
                    unmatched.push(json!({"code": code, "name": name, "reason": "non-numeric IBGE code"}));
                    continue;
                };
                let Some(row) = catalogue.by_ibge(ibge) else {
                    unmatched.push(json!({"code": code, "name": name, "reason": "IBGE code absent from catalogue"}));
                    continue;
                };
                let folded = normalized_geo_name(name);
                if folded != row.folded && !row.variants.iter().any(|variant| variant == &folded) {
                    name_mismatches.push(json!({"code": code, "source_name": name, "catalogue_name": row.name}));
                }
                if counts.get(&row.id).copied().unwrap_or(0) == 0 {
                    without_cnpj.push(json!({"code": code, "name": name}));
                }
            }
            for unmatched_geo in &unmatched {
                push(
                    "official_geography_not_in_catalogue",
                    "high",
                    format!("{} {}", manifest.dataset_id, unmatched_geo["code"]),
                    unmatched_geo.to_string(),
                    "Add a verified crosswalk or exclude the row; do not guess the municipality.".into(),
                );
            }
            for mismatch in &name_mismatches {
                push(
                    "official_geography_name_mismatch",
                    "medium",
                    format!("{} {}", manifest.dataset_id, mismatch["code"]),
                    mismatch.to_string(),
                    "Keep the IBGE code as identity and record the source spelling as an alias.".into(),
                );
            }
            if !without_cnpj.is_empty() {
                push(
                    "official_municipality_without_cnpj",
                    "low",
                    manifest.dataset_id.clone(),
                    format!("{} official geographies have zero active CNPJ establishments in T1", without_cnpj.len()),
                    "Keep the zero as zero; it is not a missing official cell.".into(),
                );
            }
            let present = geographies
                .iter()
                .filter(|(level, _, _)| level == "N6")
                .filter_map(|(_, code, _)| code.parse::<u32>().ok())
                .collect::<BTreeSet<_>>();
            let missing_catalogue = mt_catalogue
                .iter()
                .filter(|(code, _)| !present.contains(code))
                .map(|(code, name)| json!({"ibge": code, "name": name}))
                .collect::<Vec<_>>();
            let variables = store.variables()?;
            let unknown_variables = variables.iter().filter(|variable| registry.variable(variable).is_none()).cloned().collect::<Vec<_>>();
            if !missing_catalogue.is_empty() && manifest.dataset_id != "ibge-6588" {
                normalization_tasks.push(json!({
                    "kind": "official_source_coverage_gap",
                    "dataset_id": manifest.dataset_id,
                    "missing_count": missing_catalogue.len(),
                    "sample": missing_catalogue.iter().take(10).collect::<Vec<_>>(),
                    "action": "Keep absent cells absent; if the municipality is in scope, record the source coverage gap and refresh when the publisher includes it."
                }));
            }
            if !non_municipal.is_empty() {
                normalization_tasks.push(json!({
                    "kind": "official_non_municipal_scope",
                    "dataset_id": manifest.dataset_id,
                    "geographies": non_municipal,
                    "action": "Do not join this geography to a municipality; keep the published scope explicit."
                }));
            }
            if !unknown_variables.is_empty() {
                normalization_tasks.push(json!({
                    "kind": "uncurated_official_variables",
                    "dataset_id": manifest.dataset_id,
                    "variables": unknown_variables,
                    "action": "Add curated IDs, labels and pt-BR synonyms, or mark these columns as generated-only."
                }));
            }
            source_reports.push(json!({
                "dataset_id": manifest.dataset_id,
                "category": registry.dataset(&manifest.dataset_id).map(|dataset| dataset.category.clone()).unwrap_or_else(|| "unregistered".into()),
                "rows": manifest.accepted_rows,
                "variables": variables,
                "unknown_variables": unknown_variables,
                "geographies": geographies.len(),
                "non_municipal_geographies": non_municipal,
                "unmatched_geographies": unmatched,
                "name_mismatches": name_mismatches,
                "without_cnpj": without_cnpj,
                "missing_catalogue_municipalities": missing_catalogue,
            }));
        }
    }

    let report = json!({
        "generated_at": now(),
        "aggregate": {
            "path": args.agg.display().to_string(),
            "competence": aggregate.manifest().competence.to_string(),
            "municipality_catalogue_sha256": aggregate.manifest().municipality_sha256,
            "totals": totals,
            "missing_municipality_cells": missing_municipality,
            "unmatched_ids": unmatched_ids,
        },
        "catalogue": {
            "path": args.catalog.display().to_string(),
            "rows": catalogue.rows().len(),
            "duplicate_names_same_uf": duplicate_names,
            "census_missing": census_missing,
            "municipalities_without_cnpj": zero_cnpj,
        },
        "sources": source_reports,
        "inconsistencies": inconsistencies,
        "normalization_tasks": normalization_tasks,
    });
    if let Some(parent) = args.out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.out, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "audit written to {}: {} inconsistencies, {} unmatched CNPJ municipality ids, {} official geography mismatches",
        args.out.display(),
        report["inconsistencies"].as_array().map(Vec::len).unwrap_or(0),
        report["aggregate"]["unmatched_ids"].as_array().map(Vec::len).unwrap_or(0),
        report["sources"]
            .as_array()
            .map(|rows| rows
                .iter()
                .map(|row| row["unmatched_geographies"].as_array().map(Vec::len).unwrap_or(0)
                    + row["name_mismatches"].as_array().map(Vec::len).unwrap_or(0))
                .sum::<usize>())
            .unwrap_or(0)
    );
    Ok(())
}

fn raw_path_for(base: &Path, _segment: &str, index: usize) -> PathBuf {
    if index == 0 {
        base.to_path_buf()
    } else {
        let mut path = base.as_os_str().to_owned();
        path.push(format!(".{index}"));
        PathBuf::from(path)
    }
}

fn selected_variables(registry: &Registry, dataset: &Dataset, requested: Option<&str>) -> Result<Vec<String>> {
    if let Some(requested) = requested {
        let variables = requested.split(',').map(str::trim).filter(|value| !value.is_empty()).map(str::to_string).collect::<Vec<_>>();
        ensure!(!variables.is_empty(), "no variables selected");
        for variable in &variables {
            ensure!(
                registry.variable_by_source(&dataset.id, variable).is_some() || variable.parse::<u32>().is_ok(),
                "variable {variable} does not belong to {}",
                dataset.id
            );
        }
        return Ok(variables);
    }
    let variables = registry
        .variables()
        .filter(|variable| variable.dataset_id == dataset.id)
        .map(|variable| variable.source_variable.clone())
        .collect::<BTreeSet<_>>();
    ensure!(!variables.is_empty(), "no curated variables for {}", dataset.id);
    Ok(variables.into_iter().collect())
}

fn selected_categories(registry: &Registry, dataset: &Dataset, requested: Option<&str>) -> Result<Vec<String>> {
    if let Some(requested) = requested {
        let categories = requested.split(',').map(str::trim).filter(|value| !value.is_empty()).map(str::to_string).collect::<Vec<_>>();
        ensure!(!categories.is_empty(), "no categories selected");
        for category in &categories {
            ensure!(
                registry.category(category).is_some_and(|value| value.dataset_id == dataset.id),
                "category {category} does not belong to {}",
                dataset.id
            );
        }
        return Ok(categories);
    }
    Ok(registry.categories().filter(|category| category.dataset_id == dataset.id).map(|category| category.id.clone()).collect())
}

async fn fetch_ibge(
    client: &reqwest::Client,
    dataset: &Dataset,
    variables: &[String],
    category: Option<&String>,
    periods: &str,
    localities: &str,
    retries: u8,
) -> Result<(String, Vec<u8>)> {
    let mut url = format!("{}/periodos/{}/variaveis/{}", dataset.api_url.trim_end_matches('/'), periods, variables.join(","));
    let classification = category.and_then(|id| {
        let registry = Registry::builtin().ok()?;
        let category = registry.category(id)?;
        Some(format!("{}[{}]", category.classification_id, category.category_id))
    });
    let mut last_error = None;
    for attempt in 1..=retries {
        let mut request = client.get(&url).query(&[("localidades", localities)]);
        if let Some(classification) = &classification {
            request = request.query(&[("classificacao", classification)]);
        }
        match request.send().await {
            Ok(response) if response.status().is_success() => {
                let bytes = response.bytes().await.context("read IBGE response")?.to_vec();
                if bytes.first() != Some(&b'[') {
                    bail!("IBGE returned a non-array payload for {url}");
                }
                return Ok((url, bytes));
            }
            Ok(response) => last_error = Some(format!("HTTP {}", response.status())),
            Err(error) => last_error = Some(error.to_string()),
        }
        if attempt < retries {
            tokio::time::sleep(Duration::from_millis(250 * u64::from(attempt))).await;
        }
        url = format!("{}/periodos/{}/variaveis/{}", dataset.api_url.trim_end_matches('/'), periods, variables.join(","));
    }
    bail!("IBGE request failed after {retries} attempts: {}", last_error.unwrap_or_else(|| "unknown error".into()))
}

struct ParsedIbge {
    observations: Vec<Observation>,
    cells: u64,
    duplicates: u64,
    malformed: u64,
    units: BTreeSet<String>,
}

fn parse_ibge(bytes: &[u8], dataset: &Dataset, registry: &Registry, url: &str) -> Result<ParsedIbge> {
    let variables: Vec<ApiVariable> = serde_json::from_slice(bytes).with_context(|| format!("parse IBGE JSON from {url}"))?;
    let mut observations = Vec::new();
    let mut seen = BTreeSet::new();
    let mut cells = 0_u64;
    let mut duplicates = 0_u64;
    let mut malformed = 0_u64;
    let mut units = BTreeSet::new();
    for variable in variables {
        let curated = registry.variable_by_source(&dataset.id, &variable.id);
        let variable_id = curated.map_or_else(|| format!("source_{}", variable.id), |value| value.id.clone());
        let variable_label = curated.map_or_else(|| variable.variavel.clone(), |value| value.label.clone());
        let unit = curated.map_or_else(|| variable.unidade.clone(), |value| value.unit.clone());
        units.insert(unit.clone());
        for result in variable.resultados {
            let (classification_id, classification_label, category_id, category_label) = result.classificacoes.first().map_or_else(
                || (String::new(), String::new(), String::new(), String::new()),
                |class| {
                    let (category_id, category_label) = class.categoria.parts();
                    (class.id.clone(), class.nome.clone(), category_id, category_label)
                },
            );
            for series in result.series {
                for (period, raw) in series.serie {
                    cells += 1;
                    let (status, value) = parse_cell(&raw);
                    if status == CellStatus::Malformed {
                        malformed += 1;
                    }
                    let observation = Observation {
                        dataset_id: dataset.id.clone(),
                        variable_id: variable_id.clone(),
                        variable_label: variable_label.clone(),
                        unit: unit.clone(),
                        classification_id: classification_id.clone(),
                        classification_label: classification_label.clone(),
                        category_id: category_id.clone(),
                        category_label: category_label.clone(),
                        geography_level: series.localidade.nivel.id.clone(),
                        geography_code: series.localidade.id.clone(),
                        geography_name: series.localidade.nome.clone(),
                        period,
                        value,
                        status,
                        source_url: url.to_string(),
                    };
                    if !seen.insert(observation.key()) {
                        duplicates += 1;
                        continue;
                    }
                    observations.push(observation);
                }
            }
        }
    }
    Ok(ParsedIbge { observations, cells, duplicates, malformed, units })
}

fn parse_cell(raw: &str) -> (CellStatus, Option<f64>) {
    let raw = raw.trim();
    match raw {
        "" | "..." | ".." => (CellStatus::NotAvailable, None),
        "X" | "x" => (CellStatus::Suppressed, None),
        "-" => (CellStatus::Observed, Some(0.0)),
        _ => match parse_number(raw) {
            Some(value) => (CellStatus::Observed, Some(value)),
            None => (CellStatus::Malformed, None),
        },
    }
}

fn parse_number(raw: &str) -> Option<f64> {
    let cleaned = raw.trim().trim_matches('"').replace(' ', "");
    if cleaned.is_empty() {
        return None;
    }
    let value = if cleaned.contains(',') { cleaned.replace('.', "").replace(',', ".") } else { cleaned };
    value.parse::<f64>().ok().filter(|value| value.is_finite())
}

fn write_raw(path: &Path, bytes: &[u8]) -> Result<String> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("create raw evidence {}", path.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize()))
}

fn now() -> String {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map(|duration| duration.as_secs()).unwrap_or(0);
    format!("unix:{seconds}")
}

fn run_query(args: QueryArgs) -> Result<()> {
    ensure!(args.limit > 0, "limit must be positive");
    let registry = Registry::builtin()?;
    let category = match (&args.classification, &args.category) {
        (Some(classification), Some(category)) => Some(CategoryFilter::new(classification, category)),
        (None, None) => None,
        _ => bail!("--classification and --category must be supplied together"),
    };
    let request = match args.operation.as_str() {
        "lookup" => Request::Lookup { variable: args.variable, geographies: args.geographies, periods: args.periods, category },
        "compare" => {
            Request::Compare { variable: args.variable, geographies: args.geographies, period: args.periods.into_iter().next(), category }
        }
        "rank" => Request::Rank {
            variable: args.variable,
            period: args.periods.into_iter().next(),
            geography_level: args.level,
            uf_code: args.uf.as_deref().and_then(query::uf_code).map(str::to_string).or(args.uf),
            category,
            limit: args.limit,
            descending: !args.ascending,
        },
        "trend" => Request::Trend {
            variable: args.variable,
            geography: args.geographies.into_iter().next().context("trend needs --geography")?,
            periods: args.periods,
            category,
        },
        "ratio" => Request::Ratio {
            numerator: args.variable,
            denominator: args.denominator.context("ratio needs --denominator")?,
            geographies: args.geographies,
            period: args.periods.into_iter().next(),
            category,
            scale: args.scale,
        },
        other => bail!("unknown operation {other}"),
    };
    let stores = StoreSet::open_dir(&args.dir)?;
    let answer = query::execute(&stores, &registry, &request)?;
    print_answer(&answer, args.json)?;
    Ok(())
}

fn run_ask(args: AskArgs) -> Result<()> {
    let catalogue = MunicipalityCatalogue::load(&args.catalog)?;
    let registry = Registry::builtin()?;
    let request = match ask::parse(&args.text, &catalogue, &registry) {
        Ok(request) => request,
        Err(error) => {
            let status = match error {
                ask::AskError::Clarify(_) => "clarification_required",
                ask::AskError::Unsupported(_) => "unsupported",
                ask::AskError::Unavailable(_) => "unavailable",
            };
            if args.json {
                println!("{}", serde_json::json!({"status": status, "text": error.to_string()}));
            } else {
                println!("{error}");
            }
            return Ok(());
        }
    };
    let Some(dir) = args.dir else {
        let message = missing_dataset_message(&registry, request.variable_ids()[0]);
        if args.json {
            println!("{}", serde_json::json!({"status": "unavailable", "text": message}));
        } else {
            println!("{message}");
        }
        return Ok(());
    };
    let stores = match StoreSet::open_dir(&dir) {
        Ok(stores) => stores,
        Err(error) => {
            let answer = query::unavailable("ask", format!("Nenhum ativo oficial foi importado: {error}"));
            if args.json {
                println!("{}", serde_json::to_string_pretty(&answer)?);
            } else {
                println!("{}", answer.text);
            }
            return Ok(());
        }
    };
    let answer = query::execute(&stores, &registry, &request)?;
    print_answer(&answer, args.json)?;
    Ok(())
}

fn print_answer(answer: &Answer, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(answer)?);
    } else {
        println!("{}", answer.text);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_brazilian_numbers_and_cell_states() {
        assert_eq!(parse_cell("1.234,56"), (CellStatus::Observed, Some(1234.56)));
        assert_eq!(parse_cell("..."), (CellStatus::NotAvailable, None));
        assert_eq!(parse_cell("X"), (CellStatus::Suppressed, None));
        assert_eq!(parse_cell("-"), (CellStatus::Observed, Some(0.0)));
        assert_eq!(parse_cell("abc"), (CellStatus::Malformed, None));
    }

    #[test]
    fn parses_ibge_fixture_with_independent_values() {
        let registry = Registry::builtin().unwrap();
        let dataset = registry.dataset("ibge-6579").unwrap();
        let json = r#"[
          {
            "id": "9324",
            "variavel": "População residente estimada",
            "unidade": "Pessoas",
            "resultados": [
              {
                "classificacoes": [],
                "series": [
                  {"localidade": {"id": "5107925", "nivel": {"id": "N6", "nome": "Município"}, "nome": "Sorriso - MT"}, "serie": {"2023": "110000", "2024": "120985"}},
                  {"localidade": {"id": "5107909", "nivel": {"id": "N6", "nome": "Município"}, "nome": "Sinop - MT"}, "serie": {"2023": "...", "2024": "196067"}}
                ]
              }
            ]
          }
        ]"#;
        let parsed = parse_ibge(json.as_bytes(), dataset, &registry, "https://example.test/ibge").unwrap();
        assert_eq!(parsed.cells, 4);
        assert_eq!(parsed.observations.len(), 4);
        assert_eq!(parsed.malformed, 0);
        let sorriso = parsed.observations.iter().find(|row| row.geography_code == "5107925" && row.period == "2024").unwrap();
        assert_eq!(sorriso.variable_id, "populacao_estimada");
        assert_eq!(sorriso.value, Some(120985.0));
        assert_eq!(sorriso.status, CellStatus::Observed);
        let sinop = parsed.observations.iter().find(|row| row.geography_code == "5107909" && row.period == "2023").unwrap();
        assert_eq!(sinop.status, CellStatus::NotAvailable);
        assert_eq!(sinop.value, None);
        assert_eq!(parsed.units.into_iter().collect::<Vec<_>>(), ["Pessoas"]);
    }

    #[test]
    fn audit_normalizes_uf_suffixes_and_extracts_resource_links() {
        assert_eq!(normalized_geo_name("Acorizal - MT"), "acorizal");
        assert_eq!(normalized_geo_name("Vila Bela da Santíssima Trindade"), "vila bela da santissima trindade");
        let links = extract_resource_links(
            r#"<a href="/files/fleet.csv">fleet</a><a href="https://example.test/meta.json">meta</a>"#,
            "https://example.test/dataset/page",
        );
        assert_eq!(links.len(), 2);
        assert!(links.iter().any(|link| link["url"] == "https://example.test/files/fleet.csv"));
        assert!(links.iter().any(|link| link["extension"] == "json"));
    }

    #[test]
    fn audit_classifies_raw_payloads() {
        let (format, _, schema) = classify_payload(b"{\"a\":1}", "application/json", false, "https://example.test");
        assert_eq!(format, "json");
        assert_eq!(schema["keys"][0], "a");
        let (format, _, _) = classify_payload(b"A;B;\n1;2\n", "text/csv", false, "https://example.test");
        assert_eq!(format, "csv_semicolon");
        let (format, _, schema) = classify_payload(b"<!doctype html><title>x</title>", "text/html", false, "https://example.test");
        assert_eq!(format, "html");
        assert_eq!(schema["title"], "x");
    }

    #[test]
    fn rejects_non_array_payloads() {
        let registry = Registry::builtin().unwrap();
        let dataset = registry.dataset("ibge-6579").unwrap();
        assert!(parse_ibge(b"<html>not data</html>", dataset, &registry, "https://example.test").is_err());
    }
}
