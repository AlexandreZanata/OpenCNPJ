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
        records::{RECORD_FORMAT_VERSION, Record, RecordManifest, sha256_file, write_records},
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
}

struct AdapterOutput {
    records: Vec<Record>,
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
            let downloaded = download(&url, &raw_path, args.timeout_seconds)?;
            (raw_path.clone(), downloaded.0, downloaded.1, url)
        }
    };
    let catalogue = MunicipalityCatalogue::load(Path::new("crates/opencnpj-insights/catalog/municipalities.tsv"))?;
    let mut output = match args.adapter.as_str() {
        "antt" => parse_antt(&input, &args.scope, args.period.as_deref(), args.limit)?,
        "cnes" => parse_cnes(&input, &args.scope, &catalogue, args.period.as_deref(), args.limit)?,
        "inep" => parse_inep(&input, &args.scope, &catalogue, args.period.as_deref(), args.limit)?,
        "pgfn" => parse_pgfn(&input, &args.scope, args.period.as_deref(), args.limit)?,
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
    let published = write_records(&args.out, &manifest, output.records)?;
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

fn download(url: &str, path: &Path, timeout_seconds: u64) -> Result<(String, u64)> {
    let runtime = tokio::runtime::Runtime::new().context("create async runtime")?;
    runtime.block_on(async {
        let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(timeout_seconds)).build()?;
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

fn parse_antt(input: &Path, scope: &str, period: Option<&str>, limit: Option<usize>) -> Result<AdapterOutput> {
    let bytes = fs::read(input).with_context(|| format!("read {}", input.display()))?;
    let document: Value = serde_json::from_slice(&bytes).with_context(|| "parse ANTT JSON")?;
    let vehicles = document.get("veiculos").and_then(Value::as_array).context("ANTT JSON has no veiculos array")?;
    let mut records = Vec::new();
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
        records.push(Record {
            record_id,
            geography_level: "UF".into(),
            geography_code: uf_code(uf).unwrap_or("").into(),
            geography_name: uf.into(),
            cnpj: String::new(),
            cnpj_root: String::new(),
            period: period.unwrap_or("current").into(),
            data: vehicle.clone(),
        });
    }
    Ok(AdapterOutput {
        records,
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
) -> Result<AdapterOutput> {
    let (entry, bytes) = zip_entry(input, |name| name == "cnes_estabelecimentos.csv")?;
    let (header, expected) = csv_header(&bytes)?;
    let bridge = ibge_bridge(catalogue, scope);
    let mut records = Vec::new();
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
        let uf = row.get("CO_UF").map(String::as_str).unwrap_or("");
        if !scope_matches(uf, scope) {
            out_of_scope += 1;
            return Ok(());
        }
        let Some(code6) = row.get("CO_IBGE").and_then(|value| value.parse::<u32>().ok()) else {
            rejected += 1;
            return Ok(());
        };
        let Some(code7) = bridge.get(&code6).copied() else {
            rejected += 1;
            if rejected_samples.len() < 20 {
                rejected_samples.push(code6.to_string());
            }
            return Ok(());
        };
        let cnpj = normalize_cnpj(row.get("NU_CNPJ").or_else(|| row.get("NU_CNPJ_MANTENEDORA")).map(String::as_str).unwrap_or(""));
        let (cnpj, root) = split_cnpj(&cnpj);
        let record_id = format!(
            "cnes:{}:{}",
            row.get("CO_CNES").map(String::as_str).unwrap_or(""),
            row.get("CO_UNIDADE").map(String::as_str).unwrap_or("")
        );
        records.push(Record {
            record_id,
            geography_level: "N6".into(),
            geography_code: format!("{code7:07}"),
            geography_name: catalogue.by_ibge(code7).map(|row| row.name.clone()).unwrap_or_default(),
            cnpj,
            cnpj_root: root,
            period: period.unwrap_or("current").into(),
            data: json_row(row),
        });
        Ok(())
    })?;
    Ok(AdapterOutput {
        records,
        total_rows: total,
        out_of_scope_rows: out_of_scope,
        rejected_rows: rejected,
        rejected_samples,
        schema: json!({"entry": entry, "columns": header}),
        period: period.unwrap_or("current").into(),
        limitations: vec![
            "CNES IDs are not CNPJs; CNPJ fields are links where present.".into(),
            "Repeated-digit CNPJ sentinels are treated as missing.".into(),
        ],
    })
}

fn parse_inep(
    input: &Path,
    scope: &str,
    catalogue: &MunicipalityCatalogue,
    period: Option<&str>,
    limit: Option<usize>,
) -> Result<AdapterOutput> {
    let (entry, bytes) = zip_entry(input, |name| name.ends_with("microdados_ed_basica_2024.csv"))?;
    let (header, expected) = csv_header(&bytes)?;
    let mut records = Vec::new();
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
        records.push(Record {
            record_id,
            geography_level: "N6".into(),
            geography_code: format!("{code:07}"),
            geography_name: catalogue.by_ibge(code).map(|row| row.name.clone()).unwrap_or_default(),
            cnpj,
            cnpj_root: root,
            period: period.unwrap_or_else(|| row.get("NU_ANO_CENSO").map(String::as_str).unwrap_or("")).into(),
            data: json_row(row),
        });
        Ok(())
    })?;
    Ok(AdapterOutput {
        records,
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

fn parse_pgfn(input: &Path, scope: &str, period: Option<&str>, limit: Option<usize>) -> Result<AdapterOutput> {
    let mut zip = zip::ZipArchive::new(File::open(input).with_context(|| format!("open {}", input.display()))?)?;
    let names = (0..zip.len())
        .filter_map(|index| zip.by_index(index).ok().and_then(|file| file.name().ok().map(|name| name.into_owned())))
        .filter(|name| name.ends_with(".csv"))
        .collect::<Vec<_>>();
    ensure!(!names.is_empty(), "PGFN ZIP has no CSV entries");
    let mut records = Vec::new();
    let mut total = 0_u64;
    let mut out_of_scope = 0_u64;
    let mut rejected = 0_u64;
    let rejected_samples = Vec::new();
    let mut header: Vec<String> = Vec::new();
    let mut expected = 0;
    'entries: for name in names {
        let mut bytes = Vec::new();
        zip.by_name(&name)?.read_to_end(&mut bytes)?;
        if header.is_empty() {
            (header, expected) = csv_header(&bytes)?;
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
            let cnpj = normalize_cnpj(row.get("CPF_CNPJ").map(String::as_str).unwrap_or(""));
            if cnpj.len() != 14 {
                out_of_scope += 1;
                return Ok(());
            }
            let (cnpj, root) = split_cnpj(&cnpj);
            let record_id = format!(
                "pgfn:{}:{}:{}",
                row.get("TIPO_DEVEDOR").map(String::as_str).unwrap_or(""),
                row.get("NUMERO_INSCRICAO").map(String::as_str).unwrap_or(""),
                cnpj
            );
            records.push(Record {
                record_id,
                geography_level: "UF".into(),
                geography_code: uf_code(uf).unwrap_or("").into(),
                geography_name: uf.into(),
                cnpj,
                cnpj_root: root,
                period: period.unwrap_or("2026Q3").into(),
                data: json_row(row),
            });
            Ok(())
        })?;
        if stop {
            break 'entries;
        }
    }
    Ok(AdapterOutput {
        records,
        total_rows: total,
        out_of_scope_rows: out_of_scope,
        rejected_rows: rejected,
        rejected_samples,
        schema: json!({"columns": header}),
        period: period.unwrap_or("2026Q3").into(),
        limitations: vec![
            "PGFN federal/FGTS debt is not all tax debt, insolvency or a credit score.".into(),
            "Natural-person records are excluded from this business-prospecting slice.".into(),
            "Repeated-digit CNPJ sentinels are treated as missing.".into(),
        ],
    })
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

fn csv_header(bytes: &[u8]) -> Result<(Vec<String>, usize)> {
    let mut header = None;
    for_each_csv_record(bytes, b';', |line, fields| {
        if line == 1 {
            header = Some(fields);
        }
        Ok(())
    })?;
    let header = header.context("empty CSV header")?;
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
        let (header, width) = csv_header(data).unwrap();
        assert_eq!(width, 3);
        assert_eq!(header, ["CO_UF", "CO_IBGE", "NU_CNPJ"]);
        let fields = vec!["51".into(), "510792".into(), "12345678000199".into()];
        let row = row_map(&header, &fields);
        assert_eq!(row.get("CO_UF").unwrap(), "51");
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
