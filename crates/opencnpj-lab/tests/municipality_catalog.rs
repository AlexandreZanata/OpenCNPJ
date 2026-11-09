use std::{env, fs, path::Path};

use opencnpj_insights::catalog::municipalities::{Geography, MunicipalityCatalogue, Place};
use opencnpj_store::{Snapshot, postings::Field};

const OUTPUTS: [&str; 4] = ["municipalities.tsv", "overrides.tsv", "rfb-name-differences.tsv", "manifest.json"];

#[test]
#[ignore = "requires two locally built national catalogues and the national snapshot"]
fn national_municipality_catalog_is_complete_and_reproducible() {
    let first = env::var_os("OPENCNPJ_MUNICIPAL_CATALOG").expect("set OPENCNPJ_MUNICIPAL_CATALOG");
    let second = env::var_os("OPENCNPJ_MUNICIPAL_REBUILD").expect("set OPENCNPJ_MUNICIPAL_REBUILD");
    let snapshot = env::var_os("OPENCNPJ_MUNICIPAL_SNAPSHOT").expect("set OPENCNPJ_MUNICIPAL_SNAPSHOT");
    let first = Path::new(&first);
    let second = Path::new(&second);
    assert_ne!(first.canonicalize().unwrap(), second.canonicalize().unwrap(), "compare separate builds");
    for file in OUTPUTS {
        assert!(fs::read(first.join(file)).unwrap() == fs::read(second.join(file)).unwrap(), "rebuild differs for {file}");
    }

    let snapshot = Snapshot::open(Path::new(&snapshot)).unwrap();
    let catalog = MunicipalityCatalogue::load(&first.join("municipalities.tsv")).unwrap();
    let used = snapshot.prefix_terms(Field::Municipio, b"", usize::MAX);
    for (code, _) in &used {
        let code = std::str::from_utf8(code).unwrap();
        assert!(catalog.by_tom(code.parse().unwrap()).is_some(), "used TOM {code} missing from catalogue");
    }
    for (tom, ibge, name, uf) in [
        (7107, 3550308, "São Paulo", "SP"),
        (9907, 5107925, "Sorriso", "MT"),
        (8985, 5107909, "Sinop", "MT"),
        (1182, 5101837, "Boa Esperança do Norte", "MT"),
    ] {
        let row = catalog.by_tom(tom).unwrap();
        assert_eq!(catalog.by_ibge(ibge), Some(row));
        assert_eq!(row.name, name);
        let Place::Municipality(geo) = &row.place else { panic!("expected municipality for {tom}") };
        assert_eq!(geo.uf, uf);
        println!("spot check: {tom:04} -> {ibge} {name}/{uf}");
    }
    assert!(matches!(catalog.by_tom(9707).unwrap().place, Place::Abroad));
    let Place::Municipality(boa) = &catalog.by_tom(1182).unwrap().place else { panic!("expected municipality for 1182") };
    assert_eq!(boa.census_2022, None);
    assert_eq!((boa.population_estimate, boa.estimate_year), (5983, 2026));
    println!("TOM 1182: census {:?}, IBGE estimate {} for {}", boa.census_2022, boa.population_estimate, boa.estimate_year);

    let geographies: Vec<&Geography> = catalog
        .rows()
        .iter()
        .filter_map(|row| match &row.place {
            Place::Municipality(geo) => Some(geo),
            Place::Abroad => None,
        })
        .collect();
    assert_eq!(geographies.iter().filter(|geo| geo.census_2022.is_none()).count(), 1);
    assert_eq!(geographies.iter().filter(|geo| geo.capital).count(), 27);
    let links: usize = geographies.iter().map(|geo| geo.neighbours.len()).sum();
    assert_eq!(links, 32952, "all source links between municipalities must survive conversion");
    println!(
        "catalogue: {} rows, {} used TOM codes, {} geographic rows, {links} symmetric directed neighbour links",
        catalog.rows().len(),
        used.len(),
        geographies.len()
    );
    println!("all output files are byte-identical after rebuilding");
}
