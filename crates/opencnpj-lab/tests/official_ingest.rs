use std::{
    fs::File,
    io::Write,
    path::Path,
    process::{Command, Output},
};

use opencnpj_insights::official::records::RecordStore;

fn ingest(dir: &Path, adapter: &str, dataset: &str, member: &str, csv: &[u8], extra_args: &[&str]) -> Output {
    let input = dir.join("input.zip");
    let mut zip = zip::ZipWriter::new(File::create(&input).unwrap());
    zip.start_file(member, zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(csv).unwrap();
    zip.finish().unwrap();

    Command::new(env!("CARGO_BIN_EXE_opencnpj-intel"))
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .args(["official", "ingest", "--adapter", adapter, "--dataset", dataset, "--input"])
        .arg(input)
        .arg("--out")
        .arg(dir.join("records.ocrec"))
        .args(extra_args)
        .output()
        .unwrap()
}

const ANEEL_MEMBER: &str = "empreendimento-geracao-distribuida.csv";

#[test]
fn aneel_uses_holder_type_and_keeps_unlinked_business_records() {
    let dir = tempfile::tempdir().unwrap();
    let output = ingest(
        dir.path(),
        "aneel",
        "aneel-gd",
        ANEEL_MEMBER,
        b"\xef\xbb\xbf\"SigUF\";SigTipoConsumidor;NumCPFCNPJ;CodMunicipioIbge;CodEmpreendimento;Notes\r\n\
          MT;PF;11294402000162;5103403;pf;\r\n\
          MT;;11294402000162;5103403;unknown;\r\n\
          MT;PJ;12.abc.345/01de-35;5103403;valid;\"a;b\r\nc\"\"d\"\r\n\
          MT;PJ;***123456**;5103403;masked;\"holder \" \"\"\r\n\
          MT;PJ;11294402000163;invalid;bad-city;\r\n\
          ;;;;;\r\n",
        &[],
    );
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let store = RecordStore::open(dir.path().join("records.ocrec")).unwrap();
    store.verify().unwrap();
    let manifest = store.manifest();
    assert_eq!((manifest.row_count, manifest.accepted_rows, manifest.out_of_scope_rows, manifest.rejected_rows), (6, 2, 3, 1));
    assert_eq!(manifest.schema["columns"][0], "SigUF");
    assert!(manifest.limitations.iter().any(|line| line.starts_with("1 in-scope rows")));
    assert!(store.records_for_cnpj("11294402000162").unwrap().is_empty());

    let records = store.records_for_cnpj("12ABC34501DE35").unwrap();
    assert_eq!(records[0].cnpj_root, "12ABC345");
    assert_eq!(records[0].data["Notes"], "a;b\r\nc\"d");
    let unlinked = store.records().unwrap().into_iter().find(|record| record.record_id == "aneel:masked").unwrap();
    assert!(unlinked.cnpj.is_empty());
    assert!(unlinked.cnpj_root.is_empty());
    assert_eq!(unlinked.data["NumCPFCNPJ"], "***123456**");
    assert_eq!(unlinked.data["Notes"], "holder  \"\"");
}

#[test]
fn aneel_rejects_malformed_full_inputs_and_respects_preview_limit() {
    let csv = b"SigUF;SigTipoConsumidor;NumCPFCNPJ;CodMunicipioIbge;CodEmpreendimento\n\
                MT;PJ;11294402000162;5103403;valid\n\
                MT;PJ;11294402000162;5103403;\"unterminated";
    let dir = tempfile::tempdir().unwrap();
    let output = ingest(dir.path(), "aneel", "aneel-gd", ANEEL_MEMBER, csv, &[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unterminated quoted field"));
    assert!(!dir.path().join("records.ocrec").exists());

    let output = ingest(dir.path(), "aneel", "aneel-gd", ANEEL_MEMBER, csv, &["--limit", "1"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let store = RecordStore::open(dir.path().join("records.ocrec")).unwrap();
    store.verify().unwrap();
    assert_eq!(store.manifest().row_count, 1);
}

#[test]
fn pgfn_keeps_legal_entity_rows_with_unusable_identifiers() {
    let dir = tempfile::tempdir().unwrap();
    let output = ingest(
        dir.path(),
        "pgfn",
        "pgfn-fgts-2026Q3",
        "fgts.csv",
        "CPF_CNPJ;TIPO_PESSOA;TIPO_DEVEDOR;UF_DEVEDOR;NUMERO_INSCRICAO\n\
         12ABC34501DE35;Pessoa jurídica;Principal;MT;1\n\
         01367173400014;Pessoa jurídica;Principal;MT;2\n\
         ;Pessoa jurídica;Principal;MT;3\n\
         00000000000000;Pessoa física;Principal;MT;4\n"
            .as_bytes(),
        &[],
    );
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let store = RecordStore::open(dir.path().join("records.ocrec")).unwrap();
    store.verify().unwrap();
    let manifest = store.manifest();
    assert_eq!((manifest.row_count, manifest.accepted_rows, manifest.out_of_scope_rows), (4, 3, 1));
    assert!(manifest.limitations.iter().any(|line| line.starts_with("2 in-scope rows")));
    assert_eq!(store.records_for_cnpj("12ABC34501DE35").unwrap()[0].cnpj_root, "12ABC345");
    let records = store.records().unwrap();
    assert!(records.iter().filter(|record| record.cnpj.is_empty()).all(|record| record.cnpj_root.is_empty()));
    assert!(records.iter().any(|record| record.cnpj.is_empty() && record.data["CPF_CNPJ"] == "01367173400014"));
}

#[test]
fn cnes_does_not_map_non_df_rows_to_brasilia() {
    let dir = tempfile::tempdir().unwrap();
    let output = ingest(
        dir.path(),
        "cnes",
        "cnes-estabelecimentos",
        "cnes_estabelecimentos.csv",
        b"CO_CNES;CO_UNIDADE;CO_UF;CO_IBGE;NU_CNPJ\n1;1;53;530040;\n2;2;51;530040;\n",
        &["--scope", "all"],
    );
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let store = RecordStore::open(dir.path().join("records.ocrec")).unwrap();
    store.verify().unwrap();
    assert_eq!((store.manifest().row_count, store.manifest().accepted_rows, store.manifest().rejected_rows), (2, 1, 1));
    let records = store.records_for_geography("N6", "5300108").unwrap();
    assert_eq!(records[0].data["CO_IBGE"], "530040");
    assert_eq!(records[0].record_id, "cnes:1:1");
}
