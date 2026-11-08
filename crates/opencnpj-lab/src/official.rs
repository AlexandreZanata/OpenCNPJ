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
    catalog::municipalities::{MunicipalityCatalogue, Place, fold_name},
    official::{
        model::{CellStatus, DatasetManifest, Observation},
        query::{self, Answer, CategoryFilter, Request},
        registry::{Dataset, Registry},
        store::{StoreSet, missing_dataset_message, sha256_file, write_store},
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
            println!("{error}");
            return Ok(());
        }
    };
    let Some(dir) = args.dir else {
        println!("{}", missing_dataset_message(&registry, request.variable_ids()[0]));
        return Ok(());
    };
    let stores = match StoreSet::open_dir(&dir) {
        Ok(stores) => stores,
        Err(error) => {
            println!("{}", query::unavailable("ask", format!("Nenhum ativo oficial foi importado: {error}")).text);
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
    fn rejects_non_array_payloads() {
        let registry = Registry::builtin().unwrap();
        let dataset = registry.dataset("ibge-6579").unwrap();
        assert!(parse_ibge(b"<html>not data</html>", dataset, &registry, "https://example.test").is_err());
    }
}
