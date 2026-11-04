//! Synthetic RFB release used by the integration tests of this workspace (not part of the
//! stable API). Covers matriz/filial, an alphanumeric CNPJ, MEI owners with a CPF in the
//! name, masked partner CPFs, a PJ partner, Simples/MEI options and a broken line.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::build::{self, BuildConfig, Profile};

fn zip(dir: &Path, name: &str, member: &str, lines: &[Vec<u8>]) {
    let f = std::fs::File::create(dir.join(name)).expect("fixture");
    let mut z = zip::ZipWriter::new(f);
    z.start_file(member, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)).expect("fixture");
    for l in lines {
        z.write_all(l).expect("fixture");
        z.write_all(b"\n").expect("fixture");
    }
    z.finish().expect("fixture");
}

pub fn rec(fields: &[&str]) -> Vec<u8> {
    // Latin-1 output, as published by RFB.
    let s = fields.iter().map(|f| format!("\"{f}\"")).collect::<Vec<_>>().join(";");
    s.chars().map(|c| u8::try_from(u32::from(c)).expect("latin-1 test data")).collect()
}

fn estab(basico: &str, ordem: &str, fantasia: &str, uf: &str, mun: &str, cnae: &str, sit: &str) -> Vec<u8> {
    let base: [u8; 12] = format!("{basico}{ordem}").as_bytes().try_into().expect("fixture");
    let dv = opencnpj_core::cnpj::check_digits(&base);
    let dv = std::str::from_utf8(&dv).expect("fixture").to_string();
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

/// Writes the synthetic RFB release (all tables) into `dir` and returns the number of
/// source rows that must round-trip.
pub fn fixture(dir: &Path) -> usize {
    let mut empresas = vec![
        rec(&["00000000", "BANCO DO BRASIL SA", "2038", "10", "120000000000,00", "05", ""]),
        rec(&["11111111", "PADARIA SÃO JOÃO LTDA", "2062", "49", "1.000,00", "01", ""]),
        rec(&["12ABC345", "EMPRESA ALFANUMÉRICA LTDA", "2062", "49", "0,00", "03", ""]),
        rec(&["22222222", "COMERCIO DE \"PEÇAS\" AUTO LTDA", "2062", "49", "500,00", "01", ""]),
        rec(&["33333333", "SEM ESTABELECIMENTO SA", "2054", "10", "1,5", "05", ""]),
        rec(&["44444444", "55555555 JOANA PEREIRA", "2135", "50", "0,00", "01", ""]),
        rec(&["55555555", "MARIA DA SILVA 12345678901", "2135", "50", "0,00", "01", ""]),
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
        // No município and no CNAE: no doc value (id 0).
        estab("22222222", "0002", "", "MG", "", "", "02"),
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
    let socios = vec![
        rec(&["11111111", "2", "JOÃO DA SILVA", "***123456**", "49", "20200101", "", "***000000**", "", "00", "5"]),
        rec(&["22222222", "1", "PETROLEO BRASILEIRO S A PETROBRAS", "33000167000101", "22", "20210101", "", "***000000**", "", "00", "0"]),
    ];
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

/// Builds the fixture under `dir/2026-10` into `dir/<profile>.ocnpj`.
pub fn build_fixture(dir: &Path, profile: &str) -> (PathBuf, build::BuildReport) {
    let input = dir.join("2026-10");
    std::fs::create_dir_all(&input).expect("fixture dir");
    fixture(&input);
    let out = dir.join(format!("{profile}.ocnpj"));
    let cfg = BuildConfig {
        input,
        output: out.clone(),
        tmp: dir.join(format!("work-{profile}")),
        competence: "2026-10".into(),
        profile: Profile::preset(profile).expect("profile"),
        workers: 4,
        readers: 2,
        keep_partition: false,
        sample_every: None,
        plain_columns: false,
    };
    let report = build::build(&cfg, &|_| {}).expect("fixture build");
    (out, report)
}
