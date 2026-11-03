//! Builds a snapshot from synthetic RFB-format ZIPs and checks lossless reconstruction,
//! lookups, search, filters and corruption handling.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use opencnpj_core::Table;
use opencnpj_ingest::build::{self, BuildConfig, Profile};
use opencnpj_store::Snapshot;
use opencnpj_store::postings::Field;
use opencnpj_store::search::{self, Limits, Query};

fn zip(dir: &Path, name: &str, member: &str, lines: &[Vec<u8>]) {
    let f = std::fs::File::create(dir.join(name)).unwrap();
    let mut z = zip::ZipWriter::new(f);
    z.start_file(member, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)).unwrap();
    for l in lines {
        z.write_all(l).unwrap();
        z.write_all(b"\n").unwrap();
    }
    z.finish().unwrap();
}

fn rec(fields: &[&str]) -> Vec<u8> {
    // Latin-1 output, as published by RFB.
    let s = fields.iter().map(|f| format!("\"{f}\"")).collect::<Vec<_>>().join(";");
    s.chars().map(|c| u8::try_from(u32::from(c)).expect("latin-1 test data")).collect()
}

fn estab(basico: &str, ordem: &str, fantasia: &str, uf: &str, mun: &str, cnae: &str, sit: &str) -> Vec<u8> {
    let base: [u8; 12] = format!("{basico}{ordem}").as_bytes().try_into().unwrap();
    let dv = opencnpj_core::cnpj::check_digits(&base);
    let dv = std::str::from_utf8(&dv).unwrap().to_string();
    let matriz = if ordem == "0001" { "1" } else { "2" };
    rec(&[
        basico,
        ordem,
        &dv,
        matriz,
        fantasia,
        sit,
        "20200101",
        "00",
        "",
        "",
        "20200101",
        cnae,
        "4711302,4712100",
        "RUA",
        "DAS FLORES",
        "10",
        "",
        "CENTRO",
        "01310100",
        uf,
        mun,
        "11",
        "33334444",
        "",
        "",
        "",
        "",
        "contato@example.com",
        "",
        "",
    ])
}

fn fixture(dir: &Path) -> usize {
    let mut empresas = vec![
        rec(&["00000000", "BANCO DO BRASIL SA", "2038", "10", "120000000000,00", "05", ""]),
        rec(&["11111111", "PADARIA SÃO JOÃO LTDA", "2062", "49", "1.000,00", "01", ""]),
        rec(&["12ABC345", "EMPRESA ALFANUMÉRICA LTDA", "2062", "49", "0,00", "03", ""]),
        rec(&["22222222", "COMERCIO DE \"PEÇAS\" AUTO LTDA", "2062", "49", "500,00", "01", ""]),
        rec(&["33333333", "SEM ESTABELECIMENTO SA", "2054", "10", "1,5", "05", ""]),
        rec(&["44444444", "55555555 JOANA PEREIRA", "2135", "50", "0,00", "01", ""]),
        rec(&["55555555", "55555555 MARIA DA SILVA", "2135", "50", "0,00", "01", ""]),
    ];
    for i in 0..300 {
        empresas.push(rec(&[&format!("4{i:07}"), &format!("MERCADO NUMERO {i} LTDA"), "2062", "49", "10,00", "01", ""]));
    }
    let mut estabs = vec![
        estab("00000000", "0001", "DIRECAO GERAL", "DF", "9701", "6422100", "02"),
        estab("00000000", "0002", "AGENCIA CENTRO", "SP", "7107", "6422100", "02"),
        estab("11111111", "0001", "PADARIA DO JOÃO", "SP", "7107", "1091102", "02"),
        estab("12ABC345", "01DE", "", "RJ", "6001", "6201501", "02"),
        estab("22222222", "0001", "", "MG", "4123", "4530703", "08"),
        estab("99999999", "0001", "ORFAO SEM EMPRESA", "BA", "3849", "4711302", "02"),
    ];
    for i in 0..300 {
        estabs.push(estab(
            &format!("4{i:07}"),
            "0001",
            &format!("MERCADINHO {i}"),
            if i % 2 == 0 { "SP" } else { "PR" },
            "7107",
            "4712100",
            "02",
        ));
    }
    // A line that cannot be parsed must be kept as a reject, not dropped.
    estabs.insert(3, b"\"broken line without layout".to_vec());
    let socios = vec![rec(&["11111111", "2", "JOÃO DA SILVA", "***123456**", "49", "20200101", "", "***000000**", "", "00", "5"])];
    let simples = vec![
        rec(&["11111111", "S", "20200101", "00000000", "N", "00000000", "00000000"]),
        rec(&["22222222", "N", "", "", "S", "20210101", "00000000"]),
    ];
    zip(dir, "Empresas0.zip", "K3241.K03200Y0.D61010.EMPRECSV", &empresas);
    zip(dir, "Estabelecimentos0.zip", "K3241.K03200Y0.D61010.ESTABELE", &estabs);
    zip(dir, "Socios0.zip", "K3241.K03200Y0.D61010.SOCIOCSV", &socios);
    zip(dir, "Simples.zip", "F.K03200$W.SIMPLES.CSV.D61010", &simples);
    for (name, rows) in [
        ("Cnaes", vec![rec(&["6422100", "Bancos múltiplos, com carteira comercial"])]),
        ("Motivos", vec![rec(&["00", "SEM MOTIVO"])]),
        ("Municipios", vec![rec(&["7107", "SAO PAULO"]), rec(&["6001", "RIO DE JANEIRO"])]),
        ("Naturezas", vec![rec(&["2062", "Sociedade Empresária Limitada"])]),
        ("Paises", vec![rec(&["105", "BRASIL"])]),
        ("Qualificacoes", vec![rec(&["49", "Sócio-Administrador"])]),
    ] {
        zip(dir, &format!("{name}.zip"), &format!("F.K03200$Z.D61010.{}", name.to_uppercase()), &rows);
    }
    empresas.len() + estabs.len() - 1 + socios.len() + simples.len()
}

fn build_profile(dir: &Path, profile: &str) -> (Snapshot, build::BuildReport) {
    let out = dir.join(format!("{profile}.ocnpj"));
    let cfg = BuildConfig {
        input: dir.join("2026-10"),
        output: out.clone(),
        tmp: dir.join(format!("work-{profile}")),
        competence: "2026-10".into(),
        profile: Profile::preset(profile).unwrap(),
        workers: 4,
        readers: 2,
        keep_partition: false,
        sample_every: None,
        plain_columns: false,
    };
    let report = build::build(&cfg, &|_| {}).unwrap();
    (Snapshot::open(&out).unwrap(), report)
}

fn all_rows(s: &Snapshot) -> BTreeMap<(Table, String), Vec<Vec<String>>> {
    let mut m: BTreeMap<(Table, String), Vec<Vec<String>>> = BTreeMap::new();
    for i in 0..s.blocks().len() {
        for c in s.block_companies(i).unwrap() {
            for t in Table::ALL {
                for r in c.rows(t) {
                    m.entry((t, c.basico.clone())).or_default().push(r.clone());
                }
            }
        }
    }
    m
}

#[test]
fn build_query_and_integrity() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("2026-10")).unwrap();
    let rows = fixture(&dir.path().join("2026-10"));
    let (snap, report) = build_profile(dir.path(), "balanced");

    // Counts: every source row is stored; the broken line is kept as a reject.
    assert_eq!(report.counts.rows.values().sum::<u64>() as usize, rows);
    let rejects = snap.rejects().unwrap();
    assert_eq!(rejects.len(), 1);
    assert_eq!(rejects[0].line, 4);
    assert_eq!(rejects[0].raw_latin1, b"\"broken line without layout");
    assert_eq!(report.counts.companies_without_empresa, 1);
    assert!(snap.company("55555555").unwrap().unwrap().estabelecimentos.is_empty());
    snap.verify(|_, _| {}).unwrap();

    // Exact values, including Latin-1 text, unescaped quotes and non-canonical money.
    let c = snap.company("11111111").unwrap().unwrap();
    assert_eq!(c.empresas[0][0], "PADARIA SÃO JOÃO LTDA");
    assert_eq!(c.empresas[0][3], "1.000,00");
    assert_eq!(c.socios[0][1], "JOÃO DA SILVA");
    let c = snap.company("22222222").unwrap().unwrap();
    assert_eq!(c.empresas[0][0], "COMERCIO DE \"PEÇAS\" AUTO LTDA");
    assert_eq!(c.simples[0][1], "");
    assert_eq!(snap.company("33333333").unwrap().unwrap().empresas[0][3], "1,5");
    let alnum = snap.company("12ABC345").unwrap().unwrap();
    assert_eq!(alnum.estabelecimentos[0][0], "01DE");
    assert_eq!(alnum.estabelecimentos[0][1], "35");
    assert!(snap.company("99999999").unwrap().unwrap().empresas.is_empty());
    assert!(snap.company("55555556").unwrap().is_none());

    // Text search: accents folded, prefix on the last token, ranking by shortest name.
    let q = |text: &str| Query { text: Some(text.into()), limit: 10, ..Default::default() };
    let r = search::search(&snap, &q("padaria sao joao"), &Limits::DEFAULT).unwrap();
    assert_eq!(r.total, 1);
    let r = search::search(&snap, &q("banco bras"), &Limits::DEFAULT).unwrap();
    assert_eq!(r.total, 2);
    let docs = snap.docs(&r.docs).unwrap();
    assert_eq!(docs[0].company.estabelecimentos[docs[0].estab.unwrap()][0], "0001");
    // MEI-style names: the own-root token is resolved through the block directory.
    assert_eq!(search::search(&snap, &q("55555555 maria "), &Limits::DEFAULT).unwrap().total, 1);
    assert_eq!(search::search(&snap, &q("55555555 "), &Limits::DEFAULT).unwrap().total, 2);
    assert_eq!(search::search(&snap, &q("55555555"), &Limits::DEFAULT).unwrap().total, 2);
    let r = search::search(&snap, &q("mercado 17 "), &Limits::DEFAULT).unwrap();
    assert_eq!(r.total, 1);
    let mut fz = q("merkado 17 ");
    fz.fuzzy = true;
    assert_eq!(search::search(&snap, &fz, &Limits::DEFAULT).unwrap().total, 1);

    // Filters and keyset pagination.
    let f = |filters: Vec<(Field, Vec<String>)>| Query { filters, limit: 100, ..Default::default() };
    let sp = search::search(&snap, &f(vec![(Field::Uf, vec!["sp".into()])]), &Limits::DEFAULT).unwrap();
    assert_eq!(sp.total, 152);
    let mut page = f(vec![(Field::Uf, vec!["SP".into()])]);
    page.after = sp.next_after;
    let p2 = search::search(&snap, &page, &Limits::DEFAULT).unwrap();
    assert_eq!(p2.docs.len(), 52);
    assert!(p2.docs[0] > *sp.docs.last().unwrap());
    let r = search::search(
        &snap,
        &f(vec![(Field::CnaeSecundaria, vec!["4712100".into()]), (Field::Situacao, vec!["08".into()])]),
        &Limits::DEFAULT,
    )
    .unwrap();
    assert_eq!(r.total, 1);
    let r = search::search(&snap, &f(vec![(Field::Mei, vec!["S".into()])]), &Limits::DEFAULT).unwrap();
    assert_eq!(r.total, 1);
    let groups = search::group_counts(&snap, &Query::default(), Field::Uf, &Limits::DEFAULT).unwrap();
    assert_eq!(groups[0], ("SP".to_string(), 152));
    assert!(search::search(&snap, &Query { limit: 10, ..Default::default() }, &Limits::DEFAULT).is_err());

    // Every profile stores the same semantic content.
    let base = all_rows(&snap);
    for p in ["compact", "fast"] {
        let (other, _) = build_profile(dir.path(), p);
        assert_eq!(all_rows(&other), base, "profile {p} differs");
    }
    assert_eq!(snap.label("municipios", "7107"), Some("SAO PAULO"));
}

#[test]
fn corruption_is_detected() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("2026-10")).unwrap();
    fixture(&dir.path().join("2026-10"));
    let (snap, _) = build_profile(dir.path(), "fast");
    let path = snap.path().to_path_buf();
    let first = snap.blocks()[0];
    drop(snap);
    let good = std::fs::read(&path).unwrap();

    // Flipped byte inside a data block: open succeeds (lazy), verify and reads fail.
    let mut bad = good.clone();
    bad[first.offset as usize + 3] ^= 0x40;
    std::fs::write(&path, &bad).unwrap();
    let s = Snapshot::open(&path).unwrap();
    assert!(s.verify(|_, _| {}).is_err());
    assert!(s.company("00000000").is_err());
    drop(s);

    // Truncation and garbage are rejected at open.
    for cut in [10, good.len() / 2, good.len() - 1] {
        std::fs::write(&path, &good[..cut]).unwrap();
        assert!(Snapshot::open(&path).is_err(), "truncated at {cut}");
    }
    std::fs::write(&path, b"not a snapshot at all, definitely not").unwrap();
    assert!(Snapshot::open(&path).is_err());
}

/// Random single-byte corruptions anywhere in the file: every operation must return
/// `Ok` or `Err`, never panic; corruption inside checksummed regions must be reported.
#[test]
fn random_corruption_never_panics() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("2026-10")).unwrap();
    fixture(&dir.path().join("2026-10"));
    let (snap, _) = build_profile(dir.path(), "fast");
    let path = snap.path().to_path_buf();
    drop(snap);
    let good = std::fs::read(&path).unwrap();
    let mut seed = 0x5EED_u64;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        seed >> 33
    };
    let mut detected = 0;
    for _ in 0..400 {
        let mut bad = good.clone();
        let pos = next() as usize % bad.len();
        bad[pos] ^= 1 << (next() % 8);
        if next() % 8 == 0 {
            bad.truncate(next() as usize % bad.len());
        }
        std::fs::write(&path, &bad).unwrap();
        let Ok(s) = Snapshot::open(&path) else {
            detected += 1;
            continue;
        };
        let verified = s.verify(|_, _| {});
        if verified.is_err() {
            detected += 1;
        }
        let _ = s.company("11111111");
        let _ = s.establishment("00000000", "0002", "05");
        let q = Query { text: Some("mercado 1".into()), limit: 5, ..Default::default() };
        if let Ok(r) = search::search(&s, &q, &Limits::DEFAULT) {
            let _ = s.docs(&r.docs);
        }
        let _ = s.rejects();
    }
    // Every flip lands in a checksummed region except the postings bytes (validated
    // structurally on decode), so nearly all must be detected by open or verify.
    assert!(detected >= 300, "only {detected} of 400 corruptions detected");
}
