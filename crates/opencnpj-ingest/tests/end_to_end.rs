//! Builds a snapshot from synthetic RFB-format ZIPs and checks lossless reconstruction,
//! lookups, search, filters and corruption handling.

use std::collections::BTreeMap;
use std::path::Path;

use opencnpj_core::Table;
use opencnpj_ingest::build::{self, BuildConfig, Profile};
use opencnpj_ingest::testkit::fixture;
use opencnpj_store::Snapshot;
use opencnpj_store::format::{self, Footer, Header, SectionKind};
use opencnpj_store::postings::Field;
use opencnpj_store::search::{self, Limits, Query};
use roaring::RoaringBitmap;

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
    // MEI-style names: CPF-like 11-digit tokens are stored but not indexed.
    assert_eq!(snap.company("55555555").unwrap().unwrap().empresas[0][0], "MARIA DA SILVA 12345678901");
    assert_eq!(search::search(&snap, &q("maria silva 12345678901 "), &Limits::DEFAULT).unwrap().total, 1);
    assert!(snap.postings(Field::Razao, b"12345678901").unwrap().is_none());
    assert_eq!(search::search(&snap, &q("55555555 "), &Limits::DEFAULT).unwrap().total, 1);
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
    // Company values are propagated: both establishments of the MEI company match.
    let r = search::search(&snap, &f(vec![(Field::Mei, vec!["S".into()])]), &Limits::DEFAULT).unwrap();
    assert_eq!(r.total, 2);
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

/// Rewrites a snapshot as format 1.0 would have written it: no `docvalues` section.
fn downgrade_to_1_0(path: &Path, out: &Path) {
    let b = std::fs::read(path).unwrap();
    let footer = Footer::decode(&b[b.len() - format::FOOTER_LEN..]).unwrap();
    let table = &b[footer.table_offset as usize..b.len() - format::FOOTER_LEN];
    let mut sections = format::decode_sections(table, footer.table_offset, format::FORMAT_MINOR).unwrap();
    assert!(sections.iter().any(|s| s.kind == SectionKind::DocValues));
    sections.retain(|s| s.kind != SectionKind::DocValues);
    let mut header = Header::decode(&b).unwrap();
    header.minor = 0;
    let table = format::encode_sections(&sections);
    let mut o = b[..footer.table_offset as usize].to_vec();
    o[..format::HEADER_LEN].copy_from_slice(&header.encode());
    o.extend_from_slice(&table);
    let file_len = (o.len() + format::FOOTER_LEN) as u64;
    let table_xxh3 = xxhash_rust::xxh3::xxh3_64(&table);
    o.extend_from_slice(&Footer { table_offset: footer.table_offset, table_len: table.len() as u64, table_xxh3, file_len }.encode());
    std::fs::write(out, o).unwrap();
}

fn sorted(mut v: Vec<(String, u64)>) -> Vec<(String, u64)> {
    v.sort();
    v
}

#[test]
fn doc_values_match_postings() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("2026-10")).unwrap();
    fixture(&dir.path().join("2026-10"));
    let (snap, report) = build_profile(dir.path(), "balanced");
    assert_eq!(snap.header.minor, format::FORMAT_MINOR);
    assert!(report.section_bytes.contains_key("docvalues"));
    snap.verify(|_, _| {}).unwrap();
    for f in [Field::Municipio, Field::Cnae] {
        assert!(snap.doc_values(f).is_some(), "{} column", f.name());
    }
    assert!(snap.doc_values(Field::CnaeSecundaria).is_none());

    let old_path = dir.path().join("v1_0.ocnpj");
    downgrade_to_1_0(snap.path(), &old_path);
    let old = Snapshot::open(&old_path).unwrap();
    assert_eq!(old.header.minor, 0);
    assert!(old.doc_values(Field::Municipio).is_none());
    old.verify(|_, _| {}).unwrap();

    let f = |filters: Vec<(Field, Vec<String>)>| Query { filters, limit: 10, ..Default::default() };
    let queries = [
        f(vec![(Field::Uf, vec!["SP".into()])]),
        f(vec![(Field::Uf, vec!["MG".into(), "PR".into()])]),
        f(vec![(Field::Situacao, vec!["02".into()])]),
        f(vec![(Field::Porte, vec!["05".into()])]),
        Query { text: Some("mercad".into()), limit: 10, ..Default::default() },
    ];
    let all = RoaringBitmap::from_iter(0..snap.doc_count());
    for field in [Field::Municipio, Field::Cnae] {
        // Every document, including those without the field.
        let col = snap.doc_values(field).unwrap();
        let counts = snap.doc_value_counts(col, all.iter()).unwrap();
        let by_dv: Vec<(String, u64)> = col.values.iter().cloned().zip(counts[1..].iter().copied()).collect();
        assert_eq!(sorted(by_dv), sorted(search::group_counts_postings(&snap, Some(&all), field).unwrap()));
        assert!(counts[0] >= 4, "documents without {}: {}", field.name(), counts[0]);
        for q in &queries {
            let set = search::candidates(&snap, q, &Limits::DEFAULT).unwrap().all.unwrap();
            let fast = search::group_counts(&snap, q, field, &Limits::DEFAULT).unwrap();
            let reference = search::group_counts_postings(&snap, Some(&set), field).unwrap();
            assert!(!fast.is_empty());
            assert_eq!(sorted(fast.clone()), sorted(reference), "{} {:?}", field.name(), q.filters);
            // Older snapshots answer through postings, with the same order.
            assert_eq!(search::group_counts(&old, q, field, &Limits::DEFAULT).unwrap(), fast);
        }
    }
    let sp = search::group_counts(&snap, &queries[0], Field::Municipio, &Limits::DEFAULT).unwrap();
    assert_eq!(sp, [("7107".to_string(), 152)]);
    assert!(search::group_counts(&snap, &Query::default(), Field::Municipio, &Limits::DEFAULT).is_err());
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

/// Format 1.2 features: CPF and partner documents, CNPJ prefixes, CNAE hierarchy, CEP/DDD/
/// contact filters, numeric ranges, sorting, histograms and typo tolerance.
#[test]
fn identity_ranges_and_hierarchies() {
    use opencnpj_store::numeric::{NumField, Range, date_value};
    use opencnpj_store::search::RangeFilter;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("2026-10")).unwrap();
    fixture(&dir.path().join("2026-10"));
    let (snap, _) = build_profile(dir.path(), "balanced");
    snap.verify(|_, _| {}).unwrap();
    let n = |q: Query| search::search(&snap, &Query { limit: 1000, ..q }, &Limits::DEFAULT).unwrap().total;
    let f = |field: Field, v: &str| Query { filters: vec![(field, vec![v.to_string()])], ..Default::default() };

    // CPF: owner (in the MEI name), masked partner, partner CNPJ; the zero mask is ignored.
    assert_eq!(n(f(Field::Cpf, "123.456.789-01")), 1);
    assert_eq!(n(f(Field::CpfQualquer, "12345678901")), 1);
    assert_eq!(n(f(Field::CpfQualquer, "00012345600")), 1, "masked partner ***123456**");
    assert_eq!(n(f(Field::SocioDoc, "33.000.167/0001-01")), 2, "both establishments of 22222222");
    assert_eq!(n(f(Field::SocioDoc, "000000")), 0);
    assert_eq!(n(Query { text: Some("12345678901".into()), ..Default::default() }), 1, "CPF typed in the search box");

    // CNPJ prefixes: contiguous ranges, alphanumeric, and branch-level prefixes.
    assert_eq!(n(Query { cnpj_prefixes: vec!["4000001".into()], ..Default::default() }), 10);
    assert_eq!(n(Query { cnpj_prefixes: vec!["12abc".into()], ..Default::default() }), 1);
    assert_eq!(n(Query { cnpj_prefixes: vec!["00.000.000/0002".into()], ..Default::default() }), 1);
    assert_eq!(n(Query { text: Some("00.000.000".into()), ..Default::default() }), 2, "CNPJ root typed in the search box");

    // CNAE: section letter, division prefix, secondary, either.
    assert_eq!(n(f(Field::Cnae, "G")), 302);
    assert_eq!(n(f(Field::Cnae, "64")), 2);
    assert_eq!(n(f(Field::Cnae, "6422-1/00")), 2);
    assert_eq!(n(f(Field::CnaeSecundaria, "4711302")), 307);
    assert_eq!(n(f(Field::CnaeQualquer, "6422100")), 2);
    assert!(search::filter_pattern(Field::Cnae, "Z").is_err());

    // Address/contact codes.
    assert_eq!(n(f(Field::Cep, "01310")), 307);
    assert_eq!(n(f(Field::Cep, "01310100")), 307);
    assert_eq!(n(f(Field::Ddd, "011")), 307);
    assert_eq!(n(f(Field::Contato, "email")), 307);
    assert_eq!(n(f(Field::Natureza, "21")), 2, "natureza prefix: 2135 x2");

    // Numeric ranges, "no value", composites.
    let year = |y: &str| Range { lo: date_value(&format!("{y}0101")).unwrap(), hi: date_value(&format!("{y}1231")).unwrap() };
    let r = |field, range| Query { ranges: vec![RangeFilter { field, range }], ..Default::default() };
    assert_eq!(n(r(NumField::InicioAtividade, Some(year("2020")))), 307);
    assert_eq!(n(r(NumField::InicioAtividade, Some(year("2019")))), 0);
    assert_eq!(n(r(NumField::InicioAtividade, None)), 3, "companies without establishments");
    assert_eq!(n(r(NumField::CapitalSocial, Some(Range { lo: 100_001, hi: u64::MAX }))), 3);
    let mei_ativo = Query { ranges: vec![RangeFilter { field: NumField::ExclusaoMei, range: None }], ..f(Field::Mei, "S") };
    assert_eq!(n(mei_ativo), 2);
    assert_eq!(n(r(NumField::OpcaoMei, Some(year("2021")))), 2);

    // Sorting by a numeric column and yearly histograms.
    let top =
        search::search(&snap, &Query { sort: Some((NumField::CapitalSocial, true)), limit: 1, ..f(Field::Uf, "DF") }, &Limits::DEFAULT)
            .unwrap();
    assert_eq!(snap.docs(&top.docs).unwrap()[0].company.basico, "00000000");
    let col = snap.numeric(NumField::InicioAtividade).unwrap();
    assert_eq!(col.histogram(snap.bytes(), None, opencnpj_store::numeric::date_year), vec![(2020, 307)]);

    // Typos: adjacent transposition and automatic fuzzy retry.
    let auto = |t: &str| {
        search::search(&snap, &Query { text: Some(t.into()), fuzzy_auto: true, limit: 5, ..Default::default() }, &Limits::DEFAULT).unwrap()
    };
    let r = auto("pdaaria joao ");
    assert!(r.fuzzy_used && r.total == 1);
    assert!(!auto("padaria joao ").fuzzy_used);
}
