use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    path::Path,
    process::Command,
};

use opencnpj_core::schema::col;
use opencnpj_insights::{
    agg::{ActiveDetail, Aggregate, Cell, Key, MUNICIPALITY_MISSING, Section, Table, UF_MISSING, flow_bucket, stock_detail, uf_id},
    catalog::municipalities::{Geography, Municipality, MunicipalityCatalogue, Place, fold_name},
};
use opencnpj_query::ir::{MunicipalityId, UfId};
use opencnpj_store::Snapshot;

#[test]
fn aggregates_match_decoded_rows_and_engine() {
    let directory = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let (snapshot_path, _) = opencnpj_ingest::testkit::build_fixture(directory.path(), "balanced");
    let snapshot = Snapshot::open(&snapshot_path).unwrap();
    let mut toms = BTreeSet::new();
    for (tom, _) in snapshot.field_values(opencnpj_store::postings::Field::Municipio).unwrap() {
        toms.insert(tom.parse::<u16>().unwrap());
    }
    let catalogue = MunicipalityCatalogue::from_rows(
        toms.into_iter()
            .enumerate()
            .map(|(id, tom)| {
                let name = format!("Fixture {tom}");
                Municipality {
                    id: id as u16,
                    tom,
                    folded: fold_name(&name),
                    name,
                    variants: vec![],
                    place: Place::Municipality(Geography {
                        ibge: 1_000_001 + id as u32,
                        uf: "SP".into(),
                        census_2022: Some(1),
                        population_estimate: 1,
                        estimate_year: 2026,
                        latitude: 0.0,
                        longitude: 0.0,
                        neighbours: vec![],
                        immediate_region: 1,
                        intermediate_region: 1,
                        capital: false,
                    }),
                }
            })
            .collect(),
    )
    .unwrap();
    let catalogue_path = directory.path().join("municipalities.tsv");
    catalogue.write(File::create(&catalogue_path).unwrap()).unwrap();
    let aggregate_path = directory.path().join("fixture.ocagg");
    run(&["agg", "build"], &snapshot_path, &catalogue_path, "--out", &aggregate_path);
    run(&["diff-agg", "--sample", "10000", "--seed", "42"], &snapshot_path, &catalogue_path, "--agg", &aggregate_path);

    let aggregate = Aggregate::open(&aggregate_path).unwrap();
    let mut expected: BTreeMap<Section, BTreeMap<u64, u64>> = BTreeMap::new();
    let mut add = |section: Section, key: u64| *expected.entry(section).or_default().entry(key).or_default() += 1;
    let ids = (0..snapshot.doc_count()).collect::<Vec<_>>();
    for record in snapshot.docs(&ids).unwrap() {
        let Some(estab) = record.estab else { continue };
        let company = record.company;
        let row = &company.estabelecimentos[estab];
        let tom = row[col::est::MUNICIPIO].parse::<u16>().ok();
        let municipality = MunicipalityId(tom.map(|tom| catalogue.by_tom(tom).unwrap().id).unwrap_or(MUNICIPALITY_MISSING));
        let uf = uf_id(&row[col::est::UF]).unwrap_or(UfId(UF_MISSING));
        let main = row[col::est::CNAE_PRINCIPAL].parse().unwrap_or(0);
        let head = row[col::est::MATRIZ_FILIAL].parse().unwrap();
        let status = match row[col::est::SITUACAO].as_str() {
            "02" => 2,
            "08" => 5,
            other => panic!("unexpected fixture status {other}"),
        };
        let key = |cnae, detail| Key { cnae, municipality, uf, detail }.pack().unwrap();
        let mut cells = vec![(Table::T2, key(main, stock_detail(head, status)))];
        let event = |date: &str| flow_bucket(date[..4].parse().unwrap(), date[4..6].parse().unwrap());
        cells.push((Table::T3, key(main, event(&row[col::est::DATA_INICIO]))));
        if status == 5 {
            cells.push((Table::T4, key(main, event(&row[col::est::DATA_SITUACAO]))));
        }
        if status == 2 {
            let porte = company.empresas.first().map_or(0, |row| match row[col::emp::PORTE].as_str() {
                "00" => 1,
                "01" => 2,
                "03" => 3,
                "05" => 4,
                _ => 0,
            });
            let flags = |column| {
                company.simples.first().map_or(0, |row| {
                    let flag: &String = &row[column];
                    match flag.as_str() {
                        "N" => 1,
                        "S" => 2,
                        _ => 0,
                    }
                })
            };
            let detail =
                ActiveDetail { head_office: head, porte, mei: flags(col::sim::OPCAO_MEI), simples: flags(col::sim::OPCAO_SIMPLES) }.pack();
            cells.push((Table::T1, key(main, detail)));
            let secondary = row[col::est::CNAE_SECUNDARIA].split(',').filter_map(|code| code.parse::<u32>().ok()).collect::<BTreeSet<_>>();
            cells.extend(secondary.into_iter().map(|code| (Table::T5, key(code, 0))));
        }
        for (table, key) in cells {
            add(Section::Cells(table), key);
            if table != Table::T5 {
                add(Section::Cnae(table), u64::from(main));
                add(Section::Uf(table), u64::from(uf.0));
                add(Section::Municipality(table), u64::from(municipality.0));
                add(Section::UfCnae(table), (u64::from(uf.0) << 24) | u64::from(main));
            }
        }
    }

    for table in Table::ALL {
        assert!(expected.contains_key(&Section::Cells(table)), "fixture must exercise {table:?}");
    }
    for (section, cells) in expected {
        let actual = aggregate.cells(section).collect::<anyhow::Result<Vec<_>>>().unwrap();
        let expected = cells.into_iter().map(|(key, count)| Cell { key, count }).collect::<Vec<_>>();
        assert_eq!(actual, expected, "{section:?}");
        if let Section::Cells(table) = section {
            let codes = actual.iter().map(|cell| Key::unpack(cell.key).cnae).collect::<BTreeSet<_>>();
            for code in codes {
                let indexed = aggregate.cells_for_cnae(table, code).unwrap().collect::<anyhow::Result<Vec<_>>>().unwrap();
                let expected = actual.iter().copied().filter(|cell| Key::unpack(cell.key).cnae == code).collect::<Vec<_>>();
                assert_eq!(indexed, expected, "{table:?}, CNAE {code}");
            }
        }
    }
}

fn run(arguments: &[&str], snapshot: &Path, catalogue: &Path, out_flag: &str, out: &Path) {
    let result = Command::new(env!("CARGO_BIN_EXE_opencnpj-intel"))
        .args(arguments)
        .arg("--snapshot")
        .arg(snapshot)
        .arg("--catalog")
        .arg(catalogue)
        .arg(out_flag)
        .arg(out)
        .output()
        .unwrap();
    println!("{}", String::from_utf8_lossy(&result.stdout));
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
}
