use std::path::Path;

use super::{
    model::{CellStatus, DatasetManifest, Observation},
    query::{Answer, CategoryFilter, Request, execute},
    registry::Registry,
    store::{StoreSet, write_store},
};

fn manifest(dataset_id: &str, source_id: &str, title: &str, accepted: u64) -> DatasetManifest {
    DatasetManifest {
        format_version: 1,
        source_id: source_id.into(),
        dataset_id: dataset_id.into(),
        title: title.into(),
        publisher: "IBGE".into(),
        source_url: "https://servicodados.ibge.gov.br/api/v3/agregados/6579".into(),
        metadata_url: "https://servicodados.ibge.gov.br/api/v3/agregados/6579/metadados".into(),
        licence: "IBGE public statistics".into(),
        licence_url: "https://www.ibge.gov.br/acesso-informacao/termos-de-uso.html".into(),
        licence_status: "review_required".into(),
        attribution: "Fonte: IBGE".into(),
        retrieved_at: "2026-11-08T00:00:00Z".into(),
        publication: None,
        coverage: "Mato Grosso".into(),
        unit: "Pessoas".into(),
        period_from: Some("2023".into()),
        period_to: Some("2024".into()),
        raw_sha256: "0".repeat(64),
        raw_bytes: 1,
        row_count: accepted,
        accepted_rows: accepted,
        rejected_rows: 0,
        duplicate_rows: 0,
        malformed_rows: 0,
        state: "queryable".into(),
        limitations: vec!["Uma estatística municipal não é um fato sobre um CNPJ.".into()],
    }
}

#[allow(clippy::too_many_arguments)]
fn observation(
    dataset: &str,
    variable: &str,
    label: &str,
    unit: &str,
    class: &str,
    category: &str,
    category_label: &str,
    code: &str,
    name: &str,
    period: &str,
    value: Option<f64>,
    status: CellStatus,
) -> Observation {
    Observation {
        dataset_id: dataset.into(),
        variable_id: variable.into(),
        variable_label: label.into(),
        unit: unit.into(),
        classification_id: class.into(),
        classification_label: if class.is_empty() { String::new() } else { "Tipo".into() },
        category_id: category.into(),
        category_label: category_label.into(),
        geography_level: "N6".into(),
        geography_code: code.into(),
        geography_name: name.into(),
        period: period.into(),
        value,
        status,
        source_url: "https://servicodados.ibge.gov.br/api/v3/agregados/6579".into(),
    }
}

fn population_store(directory: &Path) {
    let rows = vec![
        observation(
            "ibge-6579",
            "populacao_estimada",
            "População residente estimada",
            "Pessoas",
            "",
            "",
            "",
            "5107925",
            "Sorriso",
            "2023",
            Some(110_000.0),
            CellStatus::Observed,
        ),
        observation(
            "ibge-6579",
            "populacao_estimada",
            "População residente estimada",
            "Pessoas",
            "",
            "",
            "",
            "5107925",
            "Sorriso",
            "2024",
            Some(120_985.0),
            CellStatus::Observed,
        ),
        observation(
            "ibge-6579",
            "populacao_estimada",
            "População residente estimada",
            "Pessoas",
            "",
            "",
            "",
            "5107909",
            "Sinop",
            "2023",
            Some(190_000.0),
            CellStatus::Observed,
        ),
        observation(
            "ibge-6579",
            "populacao_estimada",
            "População residente estimada",
            "Pessoas",
            "",
            "",
            "",
            "5107909",
            "Sinop",
            "2024",
            Some(196_067.0),
            CellStatus::Observed,
        ),
    ];
    let manifest = manifest("ibge-6579", "ibge", "População residente estimada", rows.len() as u64);
    write_store(directory.join("ibge-6579.ocind"), &manifest, rows).unwrap();
}

fn pib_store(directory: &Path) {
    let rows = vec![
        observation(
            "ibge-5938",
            "pib_municipal",
            "Produto Interno Bruto",
            "Mil Reais",
            "",
            "",
            "",
            "5107925",
            "Sorriso",
            "2023",
            Some(13_748_757.0),
            CellStatus::Observed,
        ),
        observation(
            "ibge-5938",
            "pib_municipal",
            "Produto Interno Bruto",
            "Mil Reais",
            "",
            "",
            "",
            "5107909",
            "Sinop",
            "2023",
            Some(9_000_000.0),
            CellStatus::Observed,
        ),
    ];
    let manifest = manifest("ibge-5938", "ibge", "PIB dos municípios", rows.len() as u64);
    write_store(directory.join("ibge-5938.ocind"), &manifest, rows).unwrap();
}

fn ppm_store(directory: &Path) {
    let rows = vec![
        observation(
            "ibge-3939",
            "ppm_efetivo",
            "Efetivo dos rebanhos",
            "Cabeças",
            "79",
            "2670",
            "Bovino",
            "5107925",
            "Sorriso",
            "2024",
            Some(600_000.0),
            CellStatus::Observed,
        ),
        observation(
            "ibge-3939",
            "ppm_efetivo",
            "Efetivo dos rebanhos",
            "Cabeças",
            "79",
            "2670",
            "Bovino",
            "5107909",
            "Sinop",
            "2024",
            None,
            CellStatus::Suppressed,
        ),
    ];
    let manifest = manifest("ibge-3939", "ibge", "PPM", rows.len() as u64);
    write_store(directory.join("ibge-3939.ocind"), &manifest, rows).unwrap();
}

fn setup() -> (tempfile::TempDir, StoreSet, Registry) {
    let directory = tempfile::tempdir().unwrap();
    population_store(directory.path());
    pib_store(directory.path());
    ppm_store(directory.path());
    let stores = StoreSet::open_dir(directory.path()).unwrap();
    (directory, stores, Registry::builtin().unwrap())
}

fn assert_observed(answer: &Answer, code: &str, value: f64) {
    let row = answer.rows.iter().find(|row| row.geography_code == code).unwrap();
    assert_eq!(row.value, Some(value));
    assert_eq!(row.status, "observed");
}

#[test]
fn store_verifies_and_refuses_overwrite() {
    let directory = tempfile::tempdir().unwrap();
    population_store(directory.path());
    let path = directory.path().join("ibge-6579.ocind");
    let original = std::fs::read(&path).unwrap();
    let stores = StoreSet::open_dir(directory.path()).unwrap();
    stores.verify().unwrap();
    let rows =
        vec![observation("ibge-6579", "populacao_estimada", "x", "Pessoas", "", "", "", "1", "x", "2024", Some(1.0), CellStatus::Observed)];
    let manifest = manifest("ibge-6579", "ibge", "x", 1);
    assert!(write_store(&path, &manifest, rows).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn lookup_returns_source_period_unit_and_limitations() {
    let (_directory, stores, registry) = setup();
    let request = Request::Lookup {
        variable: "populacao_estimada".into(),
        geographies: vec!["5107925".into()],
        periods: vec!["2024".into()],
        category: None,
    };
    let answer = execute(&stores, &registry, &request).unwrap();
    assert_eq!(answer.status, "ok");
    assert_eq!(answer.periods, ["2024"]);
    assert_eq!(answer.unit, "Pessoas");
    assert_eq!(answer.sources.len(), 1);
    assert_eq!(answer.sources[0].dataset_id, "ibge-6579");
    assert_observed(&answer, "5107925", 120_985.0);
    assert!(answer.limitations.iter().any(|value| value.contains("CNPJ")));
    assert!(answer.text.contains("Fonte:"));
}

#[test]
fn compare_and_rank_preserve_units_and_missing_state() {
    let (_directory, stores, registry) = setup();
    let compare = execute(
        &stores,
        &registry,
        &Request::Compare {
            variable: "populacao_estimada".into(),
            geographies: vec!["5107925".into(), "5107909".into()],
            period: Some("2024".into()),
            category: None,
        },
    )
    .unwrap();
    assert_eq!(compare.rows.len(), 2);
    assert_observed(&compare, "5107909", 196_067.0);

    let rank = execute(
        &stores,
        &registry,
        &Request::Rank {
            variable: "populacao_estimada".into(),
            period: Some("2024".into()),
            geography_level: "N6".into(),
            uf_code: Some("51".into()),
            category: None,
            limit: 10,
            descending: true,
        },
    )
    .unwrap();
    assert_eq!(rank.rows[0].geography_code, "5107909");
    assert_eq!(rank.rows.len(), 2);
}

#[test]
fn trend_and_ratio_use_aligned_periods() {
    let (_directory, stores, registry) = setup();
    let trend = execute(
        &stores,
        &registry,
        &Request::Trend { variable: "populacao_estimada".into(), geography: "5107925".into(), periods: vec![], category: None },
    )
    .unwrap();
    assert_eq!(trend.periods, ["2023", "2024"]);
    assert_eq!(trend.rows.len(), 2);

    let ratio = execute(
        &stores,
        &registry,
        &Request::Ratio {
            numerator: "pib_municipal".into(),
            denominator: "populacao_estimada".into(),
            geographies: vec!["5107925".into()],
            period: Some("2023".into()),
            category: None,
            scale: 1.0,
        },
    )
    .unwrap();
    assert_eq!(ratio.rows.len(), 1);
    assert_observed(&ratio, "5107925", 13_748_757.0 / 110_000.0);
    assert!(ratio.limitations.iter().any(|value| value.contains("poder de compra")));
}

#[test]
fn category_filter_and_suppressed_cell() {
    let (_directory, stores, registry) = setup();
    let answer = execute(
        &stores,
        &registry,
        &Request::Lookup {
            variable: "ppm_efetivo".into(),
            geographies: vec!["5107925".into(), "5107909".into()],
            periods: vec!["2024".into()],
            category: Some(CategoryFilter::new("79", "2670")),
        },
    )
    .unwrap();
    assert_eq!(answer.status, "partial");
    assert_observed(&answer, "5107925", 600_000.0);
    let sinop = answer.rows.iter().find(|row| row.geography_code == "5107909").unwrap();
    assert_eq!(sinop.status, "suppressed");
    assert_eq!(sinop.value, None);
}

#[test]
fn duplicate_cells_across_stores_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    population_store(directory.path());
    let rows = vec![observation(
        "ibge-6579",
        "populacao_estimada",
        "População residente estimada",
        "Pessoas",
        "",
        "",
        "",
        "5107925",
        "Sorriso",
        "2024",
        Some(1.0),
        CellStatus::Observed,
    )];
    let manifest = manifest("ibge-6579", "ibge", "copy", 1);
    write_store(directory.path().join("copy.ocind"), &manifest, rows).unwrap();
    let stores = StoreSet::open_dir(directory.path()).unwrap();
    assert!(stores.unique_observations("populacao_estimada", None).is_err());
}
