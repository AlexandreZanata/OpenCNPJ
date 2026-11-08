//! Full-resource adapters for record-level official sources.
//!
//! The adapters read the source exactly as published, map common identity fields
//! (geography, CNPJ, period) and keep every other column as JSON in an immutable
//! `.ocrec` store. The default scope is MT for the pilot; `--scope all` keeps all
//! rows. Every source row is accounted for as accepted, out of scope, rejected or
//! duplicate.

use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use clap::Args;
use opencnpj_core::text::latin1_to_string;
use opencnpj_insights::{
    catalog::municipalities::{MunicipalityCatalogue, Place},
    official::{
        query::uf_code,
        records::{RECORD_FORMAT_VERSION, Record, RecordManifest, RecordWriter, sha256_file},
        registry::Registry,
    },
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use xxhash_rust::xxh3::xxh3_64;

#[derive(Args)]
pub(crate) struct IngestArgs {
    /// Adapter name: antt, cnes, inep or pgfn.
    #[arg(long)]
    adapter: String,
    /// Registry dataset ID. Defaults for antt/cnes/inep; required for pgfn.
    #[arg(long)]
    dataset: Option<String>,
    /// Local raw file. When omitted, the registry URL is downloaded to --raw-out.
    #[arg(long)]
    input: Option<PathBuf>,
    /// Override the registry URL for the download.
    #[arg(long)]
    url: Option<String>,
    /// Destination .ocrec path. Must not exist.
    #[arg(long)]
    out: PathBuf,
    /// Raw evidence path for a download. Must not exist.
    #[arg(long)]
    raw_out: Option<PathBuf>,
    /// Scope: MT (default) or all.
    #[arg(long, default_value = "MT")]
    scope: String,
    /// Reference period recorded in the manifest.
    #[arg(long)]
    period: Option<String>,
    /// Stop after this many source rows. For verification only; recorded as a limitation.
    #[arg(long)]
    limit: Option<usize>,
    #[arg(long, default_value_t = 60)]
    timeout_seconds: u64,
    /// Accept an incomplete TLS chain for an official open-data host (INEB/COMEX).
    #[arg(long)]
    allow_invalid_cert: bool,
}

struct AdapterOutput {
    total_rows: u64,
    out_of_scope_rows: u64,
    rejected_rows: u64,
    rejected_samples: Vec<String>,
    schema: Value,
    period: String,
    limitations: Vec<String>,
}

pub(crate) fn run(args: IngestArgs) -> Result<()> {
    ensure!(args.timeout_seconds > 0, "timeout must be positive");
    ensure!(!args.out.exists(), "destination {} already exists", args.out.display());
    let registry = Registry::builtin()?;
    let dataset_id = args.dataset.clone().unwrap_or_else(|| match args.adapter.as_str() {
        "antt" => "antt-rntrc-veiculos".into(),
        "cnes" => "cnes-estabelecimentos".into(),
        "inep" => "inep-censo-escolar-2024".into(),
        _ => String::new(),
    });
    ensure!(!dataset_id.is_empty(), "--dataset is required for adapter {}", args.adapter);
    let dataset = registry.dataset(&dataset_id).with_context(|| format!("unknown registry dataset {dataset_id}"))?.clone();
    let source = registry.source(&dataset.source_id).with_context(|| format!("unknown source {}", dataset.source_id))?;
    let raw_path = args.raw_out.clone().unwrap_or_else(|| {
        let mut path = args.out.clone().into_os_string();
        path.push(".raw");
        PathBuf::from(path)
    });
    let (input, raw_sha256, raw_bytes, source_url) = match &args.input {
        Some(input) => {
            ensure!(input.exists(), "input {} does not exist", input.display());
            (input.clone(), sha256_file(input)?, fs::metadata(input)?.len(), dataset.api_url.clone())
        }
        None => {
            ensure!(!raw_path.exists(), "raw evidence {} already exists", raw_path.display());
            let url = args.url.clone().unwrap_or_else(|| dataset.api_url.clone());
            ensure!(url.starts_with("https://") || url.starts_with("http://"), "source URL must be HTTP(S)");
            if let Some(parent) = raw_path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
                fs::create_dir_all(parent)?;
            }
            let downloaded = download(&url, &raw_path, args.timeout_seconds, args.allow_invalid_cert)?;
            (raw_path.clone(), downloaded.0, downloaded.1, url)
        }
    };
    let catalogue = MunicipalityCatalogue::load(Path::new("crates/opencnpj-insights/catalog/municipalities.tsv"))?;
    let draft_period = args.period.clone().unwrap_or_else(|| match args.adapter.as_str() {
        "antt" => "current".into(),
        "cnes" => "current".into(),
        "inep" => "2024".into(),
        "pgfn" => "2026Q3".into(),
        "bcb" => "current".into(),
        "aneel" => "current".into(),
        "anp" => "current".into(),
        "comex" => "2025".into(),
        "mt" => "2024".into(),
        _ => String::new(),
    });
    let draft = RecordManifest {
        format_version: RECORD_FORMAT_VERSION,
        source_id: dataset.source_id.clone(),
        dataset_id: dataset.id.clone(),
        title: dataset.title.clone(),
        publisher: source.publisher.clone(),
        source_url: source_url.clone(),
        metadata_url: dataset.metadata_url.clone(),
        licence: source.licence.clone(),
        licence_url: source.licence_url.clone(),
        licence_status: source.licence_status.clone(),
        attribution: source.attribution.clone(),
        retrieved_at: crate::official::now(),
        period: draft_period,
        coverage: format!("{}; scope {}", dataset.coverage, args.scope),
        raw_sha256: raw_sha256.clone(),
        raw_bytes,
        row_count: 0,
        accepted_rows: 0,
        out_of_scope_rows: 0,
        rejected_rows: 0,
        duplicate_rows: 0,
        state: "queryable".into(),
        schema: Value::Null,
        limitations: dataset.limitations.clone(),
    };
    let mut writer = RecordWriter::create(&args.out, &dataset.id, &draft)?;
    let mut output = match args.adapter.as_str() {
        "antt" => parse_antt(&input, &args.scope, args.period.as_deref(), args.limit, &mut writer)?,
        "cnes" => parse_cnes(&input, &args.scope, &catalogue, args.period.as_deref(), args.limit, &mut writer)?,
        "inep" => parse_inep(&input, &args.scope, &catalogue, args.period.as_deref(), args.limit, &mut writer)?,
        "pgfn" => parse_pgfn(&input, &args.scope, args.period.as_deref(), args.limit, &mut writer)?,
        "bcb" => parse_bcb(&input, args.period.as_deref(), args.limit, &mut writer)?,
        "aneel" => parse_aneel(&input, &args.scope, &catalogue, args.period.as_deref(), args.limit, &mut writer)?,
        "anp" => parse_anp(&input, &args.scope, &catalogue, args.period.as_deref(), args.limit, &mut writer)?,
        "comex" => parse_comex(&input, &args.scope, &catalogue, args.period.as_deref(), args.limit, &mut writer)?,
        "mt" => parse_mt(&input, args.period.as_deref(), args.limit, &mut writer)?,
        other => bail!("unknown adapter {other}"),
    };
    if args.limit.is_some() {
        output.limitations.push("The ingestion was limited with --limit for verification; it is not a complete publication.".into());
    }
    let mut limitations = dataset.limitations.clone();
    limitations.extend(output.limitations);
    if !output.rejected_samples.is_empty() {
        limitations.push(format!("Rejected row sample: {}.", output.rejected_samples.join(", ")));
    }
    limitations.push(format!("Ingestion scope: {}.", args.scope));
    let manifest = RecordManifest {
        format_version: RECORD_FORMAT_VERSION,
        source_id: dataset.source_id.clone(),
        dataset_id: dataset.id.clone(),
        title: dataset.title.clone(),
        publisher: source.publisher.clone(),
        source_url,
        metadata_url: dataset.metadata_url.clone(),
        licence: source.licence.clone(),
        licence_url: source.licence_url.clone(),
        licence_status: source.licence_status.clone(),
        attribution: source.attribution.clone(),
        retrieved_at: crate::official::now(),
        period: output.period,
        coverage: format!("{}; scope {}", dataset.coverage, args.scope),
        raw_sha256,
        raw_bytes,
        row_count: output.total_rows,
        accepted_rows: 0,
        out_of_scope_rows: output.out_of_scope_rows,
        rejected_rows: output.rejected_rows,
        duplicate_rows: 0,
        state: "queryable".into(),
        schema: output.schema,
        limitations,
    };
    let published = writer.finish(&manifest)?;
    println!("raw\t{}\tbytes={}\tsha256={}", input.display(), raw_bytes, published.manifest.raw_sha256);
    println!(
        "published\t{}\trows={}\ttotal={}\tout_of_scope={}\trejected={}\tduplicates={}\tbytes={}\tsha256={}",
        published.path.display(),
        published.manifest.accepted_rows,
        published.manifest.row_count,
        published.manifest.out_of_scope_rows,
        published.manifest.rejected_rows,
        published.manifest.duplicate_rows,
        published.bytes,
        published.sha256
    );
    Ok(())
}

fn download(url: &str, path: &Path, timeout_seconds: u64, allow_invalid_cert: bool) -> Result<(String, u64)> {
    let runtime = tokio::runtime::Runtime::new().context("create async runtime")?;
    runtime.block_on(async {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_seconds))
            .danger_accept_invalid_certs(allow_invalid_cert)
            .build()?;
        let mut response = client.get(url).send().await?.error_for_status()?;
        let mut file = File::options().write(true).create_new(true).open(path)?;
        let mut hasher = Sha256::new();
        let mut bytes = 0_u64;
        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk)?;
            hasher.update(&chunk);
            bytes += chunk.len() as u64;
        }
        file.sync_all()?;
        Ok((hex::encode(hasher.finalize()), bytes))
    })
}

fn parse_antt(input: &Path, scope: &str, period: Option<&str>, limit: Option<usize>, writer: &mut RecordWriter) -> Result<AdapterOutput> {
    let bytes = fs::read(input).with_context(|| format!("read {}", input.display()))?;
    let document: Value = serde_json::from_slice(&bytes).with_context(|| "parse ANTT JSON")?;
    let vehicles = document.get("veiculos").and_then(Value::as_array).context("ANTT JSON has no veiculos array")?;
    let mut out_of_scope = 0_u64;
    let mut total = 0_u64;
    let mut schema = Value::Null;
    for vehicle in vehicles {
        if limit.is_some_and(|limit| total as usize >= limit) {
            break;
        }
        total += 1;
        let uf = vehicle.get("UFdoVeiculo").and_then(Value::as_str).unwrap_or("");
        if !scope_matches(uf, scope) {
            out_of_scope += 1;
            continue;
        }
        if schema.is_null() {
            schema = json!({"kind": "object", "keys": vehicle.as_object().map(|object| object.keys().cloned().collect::<Vec<_>>()).unwrap_or_default()});
        }
        let key = serde_json::to_vec(vehicle)?;
        let record_id = format!("antt:{:016x}", xxh3_64(&key));
        writer.push(Record {
            record_id,
            geography_level: "UF".into(),
            geography_code: uf_code(uf).unwrap_or("").into(),
            geography_name: uf.into(),
            cnpj: String::new(),
            cnpj_root: String::new(),
            period: period.unwrap_or("current").into(),
            data: vehicle.clone(),
        })?;
    }
    Ok(AdapterOutput {
        total_rows: total,
        out_of_scope_rows: out_of_scope,
        rejected_rows: 0,
        rejected_samples: Vec::new(),
        schema,
        period: period.unwrap_or("current").into(),
        limitations: vec!["ANTT rows are aggregate vehicle records, not identified CNPJ fleets.".into()],
    })
}

fn parse_cnes(
    input: &Path,
    scope: &str,
    catalogue: &MunicipalityCatalogue,
    period: Option<&str>,
    limit: Option<usize>,
    writer: &mut RecordWriter,
) -> Result<AdapterOutput> {
    let (entry, bytes) = zip_entry(input, |name| name == "cnes_estabelecimentos.csv")?;
    let (header, expected) = csv_header(&bytes, b';')?;
    let bridge = ibge_bridge(catalogue, scope);
    let mut total = 0_u64;
    let mut out_of_scope = 0_u64;
    let mut rejected = 0_u64;
    let mut rejected_samples = Vec::new();
    let mut df_region_rows = 0_u64;
    let mut stop = false;
    for_each_csv_record(&bytes, b';', |line, fields| {
        if stop || line == 1 {
            return Ok(());
        }
        if limit.is_some_and(|limit| total as usize >= limit) {
            stop = true;
            return Ok(());
        }
        total += 1;
        if fields.len() != expected {
            rejected += 1;
            return Ok(());
        }
        let row = row_map(&header, &fields);
        let uf = row.get("CO_UF").map(String::as_str).unwrap_or("");
        if !scope_matches(uf, scope) {
            out_of_scope += 1;
            return Ok(());
        }
        let Some(code6) = row.get("CO_IBGE").and_then(|value| value.parse::<u32>().ok()) else {
            rejected += 1;
            return Ok(());
        };
        let (code7, df_region) = match bridge.get(&code6).copied() {
            Some(code) => (code, false),
            None if (530_000..=539_999).contains(&code6) => (5_300_108, true),
            None => {
                rejected += 1;
                if rejected_samples.len() < 20 {
                    rejected_samples.push(code6.to_string());
                }
                return Ok(());
            }
        };
        if df_region {
            df_region_rows += 1;
        }
        let cnpj = normalize_cnpj(row.get("NU_CNPJ").or_else(|| row.get("NU_CNPJ_MANTENEDORA")).map(String::as_str).unwrap_or(""));
        let (cnpj, root) = split_cnpj(&cnpj);
        let record_id = format!(
            "cnes:{}:{}",
            row.get("CO_CNES").map(String::as_str).unwrap_or(""),
            row.get("CO_UNIDADE").map(String::as_str).unwrap_or("")
        );
        writer.push(Record {
            record_id,
            geography_level: "N6".into(),
            geography_code: format!("{code7:07}"),
            geography_name: catalogue.by_ibge(code7).map(|row| row.name.clone()).unwrap_or_default(),
            cnpj,
            cnpj_root: root,
            period: period.unwrap_or("current").into(),
            data: json_row(row),
        })?;
        Ok(())
    })?;
    Ok(AdapterOutput {
        total_rows: total,
        out_of_scope_rows: out_of_scope,
        rejected_rows: rejected,
        rejected_samples,
        schema: json!({"entry": entry, "columns": header}),
        period: period.unwrap_or("current").into(),
        limitations: {
            let mut limitations = vec![
                "CNES IDs are not CNPJs; CNPJ fields are links where present.".into(),
                "Repeated-digit CNPJ sentinels are treated as missing.".into(),
            ];
            if df_region_rows > 0 {
                limitations.push(format!(
                    "{df_region_rows} DF administrative-region codes were collapsed to Brasília (5300108) for IBGE municipality compatibility."
                ));
            }
            limitations
        },
    })
}

fn parse_inep(
    input: &Path,
    scope: &str,
    catalogue: &MunicipalityCatalogue,
    period: Option<&str>,
    limit: Option<usize>,
    writer: &mut RecordWriter,
) -> Result<AdapterOutput> {
    let (entry, bytes) = zip_entry(input, |name| name.ends_with("microdados_ed_basica_2024.csv"))?;
    let (header, expected) = csv_header(&bytes, b';')?;
    let mut total = 0_u64;
    let mut out_of_scope = 0_u64;
    let mut rejected = 0_u64;
    let mut rejected_samples = Vec::new();
    let mut stop = false;
    for_each_csv_record(&bytes, b';', |line, fields| {
        if stop || line == 1 {
            return Ok(());
        }
        if limit.is_some_and(|limit| total as usize >= limit) {
            stop = true;
            return Ok(());
        }
        total += 1;
        if fields.len() != expected {
            rejected += 1;
            return Ok(());
        }
        let row = row_map(&header, &fields);
        let uf = row.get("SG_UF").map(String::as_str).unwrap_or("");
        if !scope_matches(uf, scope) {
            out_of_scope += 1;
            return Ok(());
        }
        let Some(code) =
            row.get("CO_MUNICIPIO").and_then(|value| value.parse::<u32>().ok()).filter(|code| catalogue.by_ibge(*code).is_some())
        else {
            rejected += 1;
            if rejected_samples.len() < 20 {
                rejected_samples.push(row.get("CO_MUNICIPIO").cloned().unwrap_or_default());
            }
            return Ok(());
        };
        let cnpj =
            normalize_cnpj(row.get("NU_CNPJ_ESCOLA_PRIVADA").or_else(|| row.get("NU_CNPJ_MANTENEDORA")).map(String::as_str).unwrap_or(""));
        let (cnpj, root) = split_cnpj(&cnpj);
        let record_id = format!(
            "inep:{}:{}",
            row.get("NU_ANO_CENSO").map(String::as_str).unwrap_or(""),
            row.get("CO_ENTIDADE").map(String::as_str).unwrap_or("")
        );
        writer.push(Record {
            record_id,
            geography_level: "N6".into(),
            geography_code: format!("{code:07}"),
            geography_name: catalogue.by_ibge(code).map(|row| row.name.clone()).unwrap_or_default(),
            cnpj,
            cnpj_root: root,
            period: period.unwrap_or_else(|| row.get("NU_ANO_CENSO").map(String::as_str).unwrap_or("")).into(),
            data: json_row(row),
        })?;
        Ok(())
    })?;
    Ok(AdapterOutput {
        total_rows: total,
        out_of_scope_rows: out_of_scope,
        rejected_rows: rejected,
        rejected_samples,
        schema: json!({"entry": entry, "columns": header}),
        period: period.unwrap_or("2024").into(),
        limitations: vec![
            "INEP school codes are not CNPJs; student-level fields are out of scope.".into(),
            "Repeated-digit CNPJ sentinels are treated as missing.".into(),
        ],
    })
}

fn parse_pgfn(input: &Path, scope: &str, period: Option<&str>, limit: Option<usize>, writer: &mut RecordWriter) -> Result<AdapterOutput> {
    let mut zip = zip::ZipArchive::new(File::open(input).with_context(|| format!("open {}", input.display()))?)?;
    let names = (0..zip.len())
        .filter_map(|index| zip.by_index(index).ok().and_then(|file| file.name().ok().map(|name| name.into_owned())))
        .filter(|name| name.ends_with(".csv"))
        .collect::<Vec<_>>();
    ensure!(!names.is_empty(), "PGFN ZIP has no CSV entries");
    let mut total = 0_u64;
    let mut out_of_scope = 0_u64;
    let mut rejected = 0_u64;
    let rejected_samples = Vec::new();
    let mut invalid_cnpj_rows = 0_u64;
    let mut header: Vec<String> = Vec::new();
    let mut expected = 0;
    'entries: for name in names {
        let mut bytes = Vec::new();
        zip.by_name(&name)?.read_to_end(&mut bytes)?;
        if header.is_empty() {
            (header, expected) = csv_header(&bytes, b';')?;
        }
        let mut stop = false;
        for_each_csv_record(&bytes, b';', |line, fields| {
            if stop || line == 1 {
                return Ok(());
            }
            if limit.is_some_and(|limit| total as usize >= limit) {
                stop = true;
                return Ok(());
            }
            total += 1;
            if fields.len() != expected {
                rejected += 1;
                return Ok(());
            }
            let row = row_map(&header, &fields);
            let uf = row.get("UF_DEVEDOR").map(String::as_str).unwrap_or("");
            if !scope_matches(uf, scope) {
                out_of_scope += 1;
                return Ok(());
            }
            let person_type = row.get("TIPO_PESSOA").map(|value| fold(value)).unwrap_or_default();
            if !person_type.contains("juridica") {
                out_of_scope += 1;
                return Ok(());
            }
            let digits = normalize_cnpj(row.get("CPF_CNPJ").map(String::as_str).unwrap_or(""));
            if digits.len() != 14 {
                out_of_scope += 1;
                return Ok(());
            }
            let (cnpj, root) = if valid_cnpj(&digits) {
                (digits.clone(), digits[..8].to_string())
            } else {
                invalid_cnpj_rows += 1;
                (String::new(), String::new())
            };
            let record_id = format!(
                "pgfn:{}:{}:{}",
                row.get("TIPO_DEVEDOR").map(String::as_str).unwrap_or(""),
                row.get("NUMERO_INSCRICAO").map(String::as_str).unwrap_or(""),
                cnpj
            );
            writer.push(Record {
                record_id,
                geography_level: "UF".into(),
                geography_code: uf_code(uf).unwrap_or("").into(),
                geography_name: uf.into(),
                cnpj,
                cnpj_root: root,
                period: period.unwrap_or("2026Q3").into(),
                data: json_row(row),
            })?;
            Ok(())
        })?;
        if stop {
            break 'entries;
        }
    }
    Ok(AdapterOutput {
        total_rows: total,
        out_of_scope_rows: out_of_scope,
        rejected_rows: rejected,
        rejected_samples,
        schema: json!({"columns": header}),
        period: period.unwrap_or("2026Q3").into(),
        limitations: {
            let mut limitations = vec![
                "PGFN federal/FGTS debt is not all tax debt, insolvency or a credit score.".into(),
                "Natural-person records are excluded from this business-prospecting slice.".into(),
                "Repeated-digit CNPJ sentinels are treated as missing.".into(),
            ];
            if invalid_cnpj_rows > 0 {
                limitations.push(format!("{invalid_cnpj_rows} rows had invalid CNPJ check digits and were stored without CNPJ identity."));
            }
            limitations
        },
    })
}

fn parse_bcb(input: &Path, period: Option<&str>, limit: Option<usize>, writer: &mut RecordWriter) -> Result<AdapterOutput> {
    let bytes = fs::read(input).with_context(|| format!("read {}", input.display()))?;
    let rows: Vec<Value> = serde_json::from_slice(&bytes).with_context(|| "parse BCB SGS JSON")?;
    let mut total = 0_u64;
    let mut rejected = 0_u64;
    let mut rejected_samples = Vec::new();
    let mut schema = Value::Null;
    for row in rows {
        if limit.is_some_and(|limit| total as usize >= limit) {
            break;
        }
        total += 1;
        if schema.is_null() {
            schema = json!({"kind": "object", "keys": row.as_object().map(|object| object.keys().cloned().collect::<Vec<_>>()).unwrap_or_default()});
        }
        let Some(date) = row.get("data").and_then(Value::as_str) else {
            rejected += 1;
            if rejected_samples.len() < 20 {
                rejected_samples.push("missing data".into());
            }
            continue;
        };
        let Some(period_value) = iso_date(date) else {
            rejected += 1;
            if rejected_samples.len() < 20 {
                rejected_samples.push(date.to_string());
            }
            continue;
        };
        let record_id = format!("bcb:{period_value}");
        writer.push(Record {
            record_id,
            geography_level: "BR".into(),
            geography_code: "BR".into(),
            geography_name: "Brasil".into(),
            cnpj: String::new(),
            cnpj_root: String::new(),
            period: period.map_or(period_value, str::to_string),
            data: row,
        })?;
    }
    Ok(AdapterOutput {
        total_rows: total,
        out_of_scope_rows: 0,
        rejected_rows: rejected,
        rejected_samples,
        schema,
        period: period.unwrap_or("current").into(),
        limitations: vec!["BCB SGS series are national and are not filtered by municipality scope.".into()],
    })
}

fn parse_aneel(
    input: &Path,
    scope: &str,
    catalogue: &MunicipalityCatalogue,
    period: Option<&str>,
    limit: Option<usize>,
    writer: &mut RecordWriter,
) -> Result<AdapterOutput> {
    let mut zip = zip::ZipArchive::new(File::open(input).with_context(|| format!("open {}", input.display()))?)?;
    let entry = (0..zip.len())
        .filter_map(|index| zip.by_index(index).ok().and_then(|file| file.name().ok().map(|name| name.into_owned())))
        .find(|name| name.ends_with("empreendimento-geracao-distribuida.csv"))
        .with_context(|| format!("no ANEEL CSV entry in {}", input.display()))?;
    let mut reader = zip.by_name(&entry)?;
    let mut header = Vec::new();
    let mut expected = 0;
    let mut total = 0_u64;
    let mut out_of_scope = 0_u64;
    let mut rejected = 0_u64;
    let mut rejected_samples = Vec::new();
    let mut invalid_cnpj_rows = 0_u64;
    let mut stop = false;
    for_each_csv_reader(&mut reader, b';', |line, fields| {
        if stop {
            return Ok(());
        }
        if line == 1 {
            header = fields;
            expected = header.len();
            return Ok(());
        }
        if limit.is_some_and(|limit| total as usize >= limit) {
            stop = true;
            return Ok(());
        }
        total += 1;
        if fields.len() != expected {
            rejected += 1;
            return Ok(());
        }
        let row = row_map(&header, &fields);
        let uf = row.get("SigUF").map(String::as_str).unwrap_or("");
        if !scope_matches(uf, scope) {
            out_of_scope += 1;
            return Ok(());
        }
        let raw_cnpj = row.get("NumCPFCNPJ").map(String::as_str).unwrap_or("");
        let digits = normalize_cnpj(raw_cnpj);
        if digits.len() != 14 {
            out_of_scope += 1;
            return Ok(());
        }
        let (cnpj, root) = if valid_cnpj(&digits) {
            (digits.clone(), digits[..8].to_string())
        } else {
            invalid_cnpj_rows += 1;
            (String::new(), String::new())
        };
        let Some(code) =
            row.get("CodMunicipioIbge").and_then(|value| value.parse::<u32>().ok()).filter(|code| catalogue.by_ibge(*code).is_some())
        else {
            rejected += 1;
            if rejected_samples.len() < 20 {
                rejected_samples.push(row.get("CodMunicipioIbge").cloned().unwrap_or_default());
            }
            return Ok(());
        };
        let record_id = format!("aneel:{}", row.get("CodEmpreendimento").map(String::as_str).unwrap_or(""));
        writer.push(Record {
            record_id,
            geography_level: "N6".into(),
            geography_code: format!("{code:07}"),
            geography_name: catalogue.by_ibge(code).map(|row| row.name.clone()).unwrap_or_default(),
            cnpj,
            cnpj_root: root,
            period: period.map(str::to_string).or_else(|| row.get("AnmPeriodoReferencia").cloned()).unwrap_or_else(|| "current".into()),
            data: json_row(row),
        })?;
        Ok(())
    })?;
    let mut limitations = vec![
        "ODbL adapted-database obligations require assessment before commercial enablement.".into(),
        "A generator register does not identify every installer or prove consumption.".into(),
        "Natural-person CPF records are excluded.".into(),
    ];
    if invalid_cnpj_rows > 0 {
        limitations.push(format!("{invalid_cnpj_rows} rows had invalid CNPJ check digits and were stored without CNPJ identity."));
    }
    Ok(AdapterOutput {
        total_rows: total,
        out_of_scope_rows: out_of_scope,
        rejected_rows: rejected,
        rejected_samples,
        schema: json!({"entry": entry, "columns": header}),
        period: period.unwrap_or("current").into(),
        limitations,
    })
}

fn parse_anp(
    input: &Path,
    scope: &str,
    catalogue: &MunicipalityCatalogue,
    period: Option<&str>,
    limit: Option<usize>,
    writer: &mut RecordWriter,
) -> Result<AdapterOutput> {
    let bytes = fs::read(input).with_context(|| format!("read {}", input.display()))?;
    let (header, expected) = csv_header(&bytes, b';')?;
    let municipalities = municipality_map(catalogue, scope);
    let mut total = 0_u64;
    let mut out_of_scope = 0_u64;
    let mut rejected = 0_u64;
    let mut rejected_samples = Vec::new();
    let mut stop = false;
    for_each_csv_record(&bytes, b';', |line, fields| {
        if stop || line == 1 {
            return Ok(());
        }
        if limit.is_some_and(|limit| total as usize >= limit) {
            stop = true;
            return Ok(());
        }
        total += 1;
        if fields.len() != expected {
            rejected += 1;
            return Ok(());
        }
        let row = row_map(&header, &fields);
        let uf = row.get("Estado - Sigla").map(String::as_str).unwrap_or("");
        if !scope_matches(uf, scope) {
            out_of_scope += 1;
            return Ok(());
        }
        let cnpj = normalize_cnpj(row.get("CNPJ da Revenda").map(String::as_str).unwrap_or(""));
        if cnpj.len() != 14 {
            out_of_scope += 1;
            return Ok(());
        }
        let name = row.get("Municipio").map(|value| fold(value)).unwrap_or_default();
        let Some(Some(code)) = municipalities.get(&name) else {
            rejected += 1;
            if rejected_samples.len() < 20 {
                rejected_samples.push(row.get("Municipio").cloned().unwrap_or_default());
            }
            return Ok(());
        };
        let (cnpj, root) = split_cnpj(&cnpj);
        let date = row.get("Data da Coleta").and_then(|value| iso_date(value)).unwrap_or_else(|| period.unwrap_or("current").into());
        let key = format!(
            "{}|{}|{}|{}|{}",
            cnpj,
            row.get("Produto").map(String::as_str).unwrap_or(""),
            date,
            row.get("Revenda").map(String::as_str).unwrap_or(""),
            row.get("Valor de Venda").map(String::as_str).unwrap_or("")
        );
        writer.push(Record {
            record_id: format!("anp:{:016x}", xxh3_64(key.as_bytes())),
            geography_level: "N6".into(),
            geography_code: format!("{:07}", code),
            geography_name: catalogue.by_ibge(*code).map(|row| row.name.clone()).unwrap_or_default(),
            cnpj,
            cnpj_root: root,
            period: date,
            data: json_row(row),
        })?;
        Ok(())
    })?;
    Ok(AdapterOutput {
        total_rows: total,
        out_of_scope_rows: out_of_scope,
        rejected_rows: rejected,
        rejected_samples,
        schema: json!({"columns": header}),
        period: period.unwrap_or("current").into(),
        limitations: vec![
            "The survey is a sample, not every station or municipality.".into(),
            "Collection dates and prices are not a live feed.".into(),
            "Natural-person CPF records are excluded.".into(),
        ],
    })
}

fn parse_comex(
    input: &Path,
    scope: &str,
    catalogue: &MunicipalityCatalogue,
    period: Option<&str>,
    limit: Option<usize>,
    writer: &mut RecordWriter,
) -> Result<AdapterOutput> {
    let bytes = fs::read(input).with_context(|| format!("read {}", input.display()))?;
    let (header, expected) = csv_header(&bytes, b';')?;
    let mut total = 0_u64;
    let mut out_of_scope = 0_u64;
    let mut rejected = 0_u64;
    let mut rejected_samples = Vec::new();
    let mut stop = false;
    for_each_csv_record(&bytes, b';', |line, fields| {
        if stop || line == 1 {
            return Ok(());
        }
        if limit.is_some_and(|limit| total as usize >= limit) {
            stop = true;
            return Ok(());
        }
        total += 1;
        if fields.len() != expected {
            rejected += 1;
            return Ok(());
        }
        let row = row_map(&header, &fields);
        let uf = row.get("SG_UF_MUN").map(String::as_str).unwrap_or("");
        if !scope_matches(uf, scope) {
            out_of_scope += 1;
            return Ok(());
        }
        let Some(code) = row.get("CO_MUN").and_then(|value| value.parse::<u32>().ok()).filter(|code| catalogue.by_ibge(*code).is_some())
        else {
            rejected += 1;
            if rejected_samples.len() < 20 {
                rejected_samples.push(row.get("CO_MUN").cloned().unwrap_or_default());
            }
            return Ok(());
        };
        let year = row.get("CO_ANO").cloned().unwrap_or_default();
        let month = row.get("CO_MES").cloned().unwrap_or_default();
        let record_period = period.map(str::to_string).unwrap_or_else(|| format!("{year}{month:0>2}"));
        let key = format!(
            "{}|{}|{}|{}|{}|{}|{}",
            year,
            month,
            row.get("SH4").map(String::as_str).unwrap_or(""),
            row.get("CO_PAIS").map(String::as_str).unwrap_or(""),
            row.get("CO_MUN").map(String::as_str).unwrap_or(""),
            row.get("KG_LIQUIDO").map(String::as_str).unwrap_or(""),
            row.get("VL_FOB").map(String::as_str).unwrap_or("")
        );
        writer.push(Record {
            record_id: format!("comex:{:016x}", xxh3_64(key.as_bytes())),
            geography_level: "N6".into(),
            geography_code: format!("{code:07}"),
            geography_name: catalogue.by_ibge(code).map(|row| row.name.clone()).unwrap_or_default(),
            cnpj: String::new(),
            cnpj_root: String::new(),
            period: record_period,
            data: json_row(row),
        })?;
        Ok(())
    })?;
    Ok(AdapterOutput {
        total_rows: total,
        out_of_scope_rows: out_of_scope,
        rejected_rows: rejected,
        rejected_samples,
        schema: json!({"columns": header}),
        period: period.unwrap_or("2025").into(),
        limitations: vec![
            "Municipal detail uses the declarant's fiscal domicile and SH4.".into(),
            "Values are FOB and net kilograms; they are not company revenue.".into(),
        ],
    })
}

fn parse_mt(input: &Path, period: Option<&str>, limit: Option<usize>, writer: &mut RecordWriter) -> Result<AdapterOutput> {
    let bytes = fs::read(input).with_context(|| format!("read {}", input.display()))?;
    let (header, expected) = csv_header(&bytes, b',')?;
    let mut total = 0_u64;
    let mut out_of_scope = 0_u64;
    let mut rejected = 0_u64;
    let rejected_samples = Vec::new();
    let mut stop = false;
    for_each_csv_record(&bytes, b',', |line, fields| {
        if stop || line == 1 {
            return Ok(());
        }
        if limit.is_some_and(|limit| total as usize >= limit) {
            stop = true;
            return Ok(());
        }
        total += 1;
        if fields.len() != expected {
            rejected += 1;
            return Ok(());
        }
        let row = row_map(&header, &fields);
        let cnpj = normalize_cnpj(row.get("cpf/cnpj convenente").map(String::as_str).unwrap_or(""));
        if cnpj.len() != 14 {
            out_of_scope += 1;
            return Ok(());
        }
        let (cnpj, root) = split_cnpj(&cnpj);
        let record_id = format!("mt-convenios:{}:{}", row.get("número").map(String::as_str).unwrap_or(""), cnpj);
        writer.push(Record {
            record_id,
            geography_level: "UF".into(),
            geography_code: "51".into(),
            geography_name: "Mato Grosso".into(),
            cnpj,
            cnpj_root: root,
            period: period.unwrap_or("2024").into(),
            data: json_row(row),
        })?;
        Ok(())
    })?;
    Ok(AdapterOutput {
        total_rows: total,
        out_of_scope_rows: out_of_scope,
        rejected_rows: rejected,
        rejected_samples,
        schema: json!({"columns": header}),
        period: period.unwrap_or("2024").into(),
        limitations: vec![
            "A convenio is not a contract, payment or private B2B demand.".into(),
            "Natural-person CPF records are excluded; only 14-digit CNPJs are stored.".into(),
        ],
    })
}

fn municipality_map(catalogue: &MunicipalityCatalogue, scope: &str) -> BTreeMap<String, Option<u32>> {
    let mut map = BTreeMap::new();
    for row in catalogue.rows() {
        let Place::Municipality(place) = &row.place else { continue };
        if !scope.eq_ignore_ascii_case("all") && !place.uf.eq_ignore_ascii_case(scope) {
            continue;
        }
        map.entry(fold(&row.name)).and_modify(|value| *value = None).or_insert(Some(place.ibge));
    }
    map
}

fn iso_date(value: &str) -> Option<String> {
    let value = value.trim();
    let (day, rest) = value.split_once('/')?;
    let (month, year) = rest.split_once('/')?;
    if day.len() != 2 || month.len() != 2 || year.len() != 4 {
        return None;
    }
    if !day.chars().all(|character| character.is_ascii_digit())
        || !month.chars().all(|character| character.is_ascii_digit())
        || !year.chars().all(|character| character.is_ascii_digit())
    {
        return None;
    }
    Some(format!("{year}-{month}-{day}"))
}

fn zip_entry(input: &Path, wanted: impl Fn(&str) -> bool) -> Result<(String, Vec<u8>)> {
    let mut zip = zip::ZipArchive::new(File::open(input).with_context(|| format!("open {}", input.display()))?)?;
    let names = (0..zip.len())
        .filter_map(|index| zip.by_index(index).ok().and_then(|file| file.name().ok().map(|name| name.into_owned())))
        .collect::<Vec<_>>();
    let name = names.into_iter().find(|name| wanted(name)).with_context(|| format!("no matching entry in {}", input.display()))?;
    let mut bytes = Vec::new();
    zip.by_name(&name)?.read_to_end(&mut bytes)?;
    Ok((name, bytes))
}

struct CsvReader<R: Read> {
    inner: R,
    buffer: [u8; 64 * 1024],
    position: usize,
    available: usize,
    delimiter: u8,
    fields: Vec<String>,
    field: Vec<u8>,
    in_quotes: bool,
    quote_pending: bool,
    line: u64,
    started: bool,
}

impl<R: Read> CsvReader<R> {
    fn new(inner: R, delimiter: u8) -> Self {
        Self {
            inner,
            buffer: [0; 64 * 1024],
            position: 0,
            available: 0,
            delimiter,
            fields: Vec::new(),
            field: Vec::new(),
            in_quotes: false,
            quote_pending: false,
            line: 1,
            started: false,
        }
    }

    fn next_byte(&mut self) -> Result<Option<u8>> {
        if self.position == self.available {
            self.available = self.inner.read(&mut self.buffer)?;
            self.position = 0;
            if self.available == 0 {
                return Ok(None);
            }
        }
        let byte = self.buffer[self.position];
        self.position += 1;
        Ok(Some(byte))
    }

    fn next_record(&mut self) -> Result<Option<(u64, Vec<String>)>> {
        loop {
            let Some(byte) = self.next_byte()? else {
                if self.started || !self.field.is_empty() || !self.fields.is_empty() {
                    self.fields.push(decode(&self.field));
                    self.field.clear();
                    let fields = std::mem::take(&mut self.fields);
                    let line = self.line;
                    self.line += 1;
                    self.started = false;
                    return Ok(Some((line, fields)));
                }
                return Ok(None);
            };
            if self.in_quotes {
                if self.quote_pending {
                    if byte == b'"' {
                        self.field.push(b'"');
                        self.quote_pending = false;
                        continue;
                    }
                    self.in_quotes = false;
                    self.quote_pending = false;
                } else if byte == b'"' {
                    self.quote_pending = true;
                    continue;
                } else {
                    self.field.push(byte);
                    continue;
                }
            }
            match byte {
                b'"' if self.field.is_empty() => {
                    self.in_quotes = true;
                    self.started = true;
                }
                b if b == self.delimiter => {
                    self.fields.push(decode(&self.field));
                    self.field.clear();
                    self.started = true;
                }
                b'\n' => {
                    self.fields.push(decode(&self.field));
                    self.field.clear();
                    let line = self.line;
                    self.line += 1;
                    let fields = std::mem::take(&mut self.fields);
                    self.started = false;
                    if fields.iter().any(|field| !field.is_empty()) {
                        return Ok(Some((line, fields)));
                    }
                }
                b'\r' => {}
                _ => {
                    self.field.push(byte);
                    self.started = true;
                }
            }
        }
    }
}

fn for_each_csv_reader<R: Read>(reader: R, delimiter: u8, mut callback: impl FnMut(u64, Vec<String>) -> Result<()>) -> Result<()> {
    let mut reader = CsvReader::new(reader, delimiter);
    while let Some((line, fields)) = reader.next_record()? {
        callback(line, fields)?;
    }
    Ok(())
}

fn csv_header(bytes: &[u8], delimiter: u8) -> Result<(Vec<String>, usize)> {
    let mut header = None;
    for_each_csv_record(bytes, delimiter, |line, fields| {
        if line == 1 {
            header = Some(fields);
        }
        Ok(())
    })?;
    let mut header = header.context("empty CSV header")?;
    if let Some(first) = header.first_mut() {
        *first = first.trim_start_matches('\u{feff}').to_string();
    }
    ensure!(!header.is_empty(), "empty CSV header");
    let width = header.len();
    Ok((header, width))
}

fn row_map(header: &[String], fields: &[String]) -> BTreeMap<String, String> {
    header
        .iter()
        .enumerate()
        .map(|(index, name)| (name.clone(), fields.get(index).cloned().unwrap_or_default().trim_end_matches('\r').to_string()))
        .collect()
}

fn for_each_csv_record(bytes: &[u8], delimiter: u8, mut callback: impl FnMut(u64, Vec<String>) -> Result<()>) -> Result<()> {
    let mut fields = Vec::new();
    let mut field = Vec::new();
    let mut in_quotes = false;
    let mut line = 1_u64;
    let mut started = false;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_quotes {
            if byte == b'"' {
                if bytes.get(index + 1) == Some(&b'"') {
                    field.push(b'"');
                    index += 2;
                    continue;
                }
                in_quotes = false;
                index += 1;
                continue;
            }
            field.push(byte);
            index += 1;
            continue;
        }
        match byte {
            b'"' if field.is_empty() => {
                in_quotes = true;
                started = true;
                index += 1;
            }
            b if b == delimiter => {
                fields.push(decode(&field));
                field.clear();
                started = true;
                index += 1;
            }
            b'\n' => {
                fields.push(decode(&field));
                field.clear();
                if started || !fields.iter().all(String::is_empty) {
                    callback(line, std::mem::take(&mut fields))?;
                }
                fields.clear();
                started = false;
                line += 1;
                index += 1;
            }
            b'\r' => index += 1,
            _ => {
                field.push(byte);
                started = true;
                index += 1;
            }
        }
    }
    if started || !field.is_empty() {
        fields.push(decode(&field));
        callback(line, fields)?;
    }
    Ok(())
}

fn decode(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(value) => value.to_string(),
        Err(_) => {
            let mut value = String::new();
            latin1_to_string(bytes, &mut value);
            value
        }
    }
}

fn fold(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .map(|character| match character {
            'à'..='å' => 'a',
            'ç' => 'c',
            'è'..='ë' => 'e',
            'ì'..='ï' => 'i',
            'ñ' => 'n',
            'ò'..='ö' | 'ø' => 'o',
            'ù'..='ü' => 'u',
            _ => character,
        })
        .collect()
}

fn cnpj_digits(value: &str) -> String {
    value.chars().filter(char::is_ascii_digit).collect()
}

/// Repeated-digit CNPJ values (for example `99999999999999`) are source
/// sentinels, not identities. They are stored as missing and never joined.
fn normalize_cnpj(value: &str) -> String {
    let digits = cnpj_digits(value);
    if digits.len() == 14 && digits.bytes().all(|byte| byte == digits.as_bytes()[0]) { String::new() } else { digits }
}

fn valid_cnpj(cnpj: &str) -> bool {
    if cnpj.len() != 14 || !cnpj.chars().all(|character| character.is_ascii_digit()) {
        return false;
    }
    let digits = cnpj.bytes().map(|byte| u32::from(byte - b'0')).collect::<Vec<_>>();
    let weights1 = [5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
    let sum1 = weights1.iter().zip(&digits[..12]).map(|(weight, digit)| weight * digit).sum::<u32>();
    let dv1 = {
        let remainder = sum1 % 11;
        if remainder < 2 { 0 } else { 11 - remainder }
    };
    if dv1 != digits[12] {
        return false;
    }
    let weights2 = [6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
    let sum2 = weights2.iter().zip(&digits[..13]).map(|(weight, digit)| weight * digit).sum::<u32>();
    let dv2 = {
        let remainder = sum2 % 11;
        if remainder < 2 { 0 } else { 11 - remainder }
    };
    dv2 == digits[13]
}

fn split_cnpj(value: &str) -> (String, String) {
    if value.len() == 14 { (value.to_string(), value[..8].to_string()) } else { (String::new(), String::new()) }
}

fn scope_matches(value: &str, scope: &str) -> bool {
    if scope.eq_ignore_ascii_case("all") {
        return true;
    }
    value.eq_ignore_ascii_case(scope) || uf_code(scope).is_some_and(|code| value == code)
}

fn ibge_bridge(catalogue: &MunicipalityCatalogue, scope: &str) -> BTreeMap<u32, u32> {
    catalogue
        .rows()
        .iter()
        .filter_map(|row| match &row.place {
            Place::Municipality(place) if scope.eq_ignore_ascii_case("all") || place.uf.eq_ignore_ascii_case(scope) => {
                Some((place.ibge / 10, place.ibge))
            }
            _ => None,
        })
        .collect()
}

fn json_row(map: BTreeMap<String, String>) -> Value {
    Value::Object(map.into_iter().map(|(key, value)| (key, Value::String(value))).collect::<Map<_, _>>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_handles_empty_fields_quotes_and_escaped_quotes() {
        let data = b"a;b;c;d\r\n1;;\"x;y\";\"he said \"\"hi\"\"\"\r\n2;3;4;5\r\n";
        let mut rows = Vec::new();
        for_each_csv_record(data, b';', |line, fields| {
            rows.push((line, fields));
            Ok(())
        })
        .unwrap();
        assert_eq!(rows[0].1, ["a", "b", "c", "d"]);
        assert_eq!(rows[1].1, ["1", "", "x;y", "he said \"hi\""]);
        assert_eq!(rows[2].1, ["2", "3", "4", "5"]);
    }

    #[test]
    fn csv_header_and_row_map_preserve_columns() {
        let data = b"CO_UF;CO_IBGE;NU_CNPJ\r\n51;510792;12345678000199\r\n";
        let (header, width) = csv_header(data, b';').unwrap();
        assert_eq!(width, 3);
        assert_eq!(header, ["CO_UF", "CO_IBGE", "NU_CNPJ"]);
        let fields = vec!["51".into(), "510792".into(), "12345678000199".into()];
        let row = row_map(&header, &fields);
        assert_eq!(row.get("CO_UF").unwrap(), "51");
    }

    #[test]
    fn csv_reader_streams_records_across_chunks() {
        let mut data = Vec::new();
        data.extend_from_slice(b"a;b;c\n1;;\"x;y\"\n");
        data.extend_from_slice(&vec![b'z'; 70_000]);
        let mut rows = Vec::new();
        for_each_csv_reader(std::io::Cursor::new(data), b';', |line, fields| {
            rows.push((line, fields));
            Ok(())
        })
        .unwrap();
        assert_eq!(rows[0].1, ["a", "b", "c"]);
        assert_eq!(rows[1].1, ["1", "", "x;y"]);
        assert_eq!(rows[2].1[0].len(), 70_000);
    }

    #[test]
    fn cnpj_check_digits() {
        assert!(valid_cnpj("11294402000162"));
        assert!(!valid_cnpj("01367173400014"));
        assert!(!valid_cnpj("99999999999999"));
    }

    #[test]
    fn iso_dates_and_bom_headers() {
        assert_eq!(iso_date("07/10/2026"), Some("2026-10-07".into()));
        assert_eq!(iso_date("7/10/2026"), None);
        assert_eq!(iso_date("2026-10-07"), None);
        let (header, width) = csv_header(b"\xef\xbb\xbfCO_UF;CO_MUN\nMT;5107925\n", b';').unwrap();
        assert_eq!(width, 2);
        assert_eq!(header[0], "CO_UF");
    }

    #[test]
    fn scope_and_cnpj_helpers() {
        assert!(scope_matches("MT", "MT"));
        assert!(scope_matches("51", "MT"));
        assert!(scope_matches("SP", "all"));
        assert!(!scope_matches("SP", "MT"));
        assert_eq!(cnpj_digits("12.345.678/0001-99"), "12345678000199");
        assert_eq!(split_cnpj("12345678000199"), ("12345678000199".into(), "12345678".into()));
        assert_eq!(split_cnpj("12345678901"), (String::new(), String::new()));
    }
}
