//! HTTP contract of the public API (`opencnpj::api::router`), driven in-process over the
//! synthetic fixture of `opencnpj_ingest::testkit` (see `common`).
//!
//! Fixture documents (310): one per establishment (307) plus one per company without
//! establishments (33333333, 44444444, 55555555). Notable rows:
//! * 00000000 BANCO DO BRASIL SA: 0001 DF/9701 and 0002 SP/7107, CNAE 6422100, porte 05.
//! * 11111111 PADARIA SÃO JOÃO LTDA: SP/7107, CNAE 1091102, Simples S, partner `***123456**`.
//! * 12ABC345/01DE-35: alphanumeric CNPJ, RJ/6001, porte 03, capital 0,00.
//! * 22222222: 0001 MG situação 08, 0002 without município/CNAE; PJ partner
//!   33000167000101; MEI S since 2021-01-01.
//! * 33333333 (no establishment), 44444444 "55555555 JOANA PEREIRA" and 55555555
//!   "MARIA DA SILVA 12345678901" (natureza 2135, no establishment).
//! * 40000000..40000299 MERCADO NUMERO i LTDA / MERCADINHO i: SP (even i) or PR, 7107,
//!   CNAE 4712100, capital 10,00.
//! * 99999999/0001 ORFAO SEM EMPRESA: BA, no empresa row.
//!
//! Every establishment: situação 02 (except 22222222/0001), motivo 00, opened and situação
//! dated 2020-01-01, CEP 01310100, DDD 11, phone and e-mail, secondary CNAE 4711302,4712100.

mod common;

use std::collections::BTreeSet;

use axum::Router;
use axum::http::StatusCode;
use common::*;
use serde_json::Value;

// ---------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------

const CACHE: &str = "public, max-age=3600";

fn keys(v: &Value) -> BTreeSet<String> {
    v.as_object().unwrap_or_else(|| panic!("not an object: {v}")).keys().cloned().collect()
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|s| s.to_string()).collect()
}

/// Search result identity: the 14-character CNPJ, or the root for companies without
/// establishments.
fn hit_id(r: &Value) -> String {
    r.get("cnpj").or_else(|| r.get("cnpj_basico")).and_then(Value::as_str).expect("hit id").to_string()
}

/// `GET /api/v1/search?<qs>`, asserting 200.
async fn search(app: &Router, qs: &str) -> Value {
    let (s, _, v) = get_json(app, &format!("/api/v1/search?{qs}")).await;
    assert_eq!(s, StatusCode::OK, "search?{qs}: {v}");
    v
}

async fn total(app: &Router, qs: &str) -> u64 {
    search(app, qs).await["total"].as_u64().expect("total")
}

/// Every hit of a query (single page of up to 10,000).
async fn all_ids(app: &Router, qs: &str) -> Vec<String> {
    let v = search(app, &format!("{qs}&limit=10000")).await;
    let ids: Vec<String> = v["results"].as_array().unwrap().iter().map(hit_id).collect();
    assert_eq!(ids.len() as u64, v["total"].as_u64().unwrap(), "{qs}: one page holds every hit");
    ids
}

async fn id_set(app: &Router, qs: &str) -> BTreeSet<String> {
    all_ids(app, qs).await.into_iter().collect()
}

/// Asserts an error response with the legacy shape; returns the message.
async fn expect_error(app: &Router, uri: &str, status: StatusCode, code: &str) -> String {
    let r = send(app, get_request(uri)).await;
    assert_eq!(r.status, status, "{uri}: {}", String::from_utf8_lossy(&r.body));
    assert_eq!(r.header("content-type"), Some("application/json"), "{uri}");
    assert_eq!(r.header("cache-control"), None, "{uri}: errors are not cacheable");
    let v = r.json();
    assert_eq!(keys(&v), set(&["error", "message", "code"]), "{uri}: {v}");
    assert_eq!(v["error"], code, "{uri}: {v}");
    assert_eq!(v["code"].as_u64(), Some(u64::from(status.as_u16())), "{uri}");
    let m = v["message"].as_str().expect("message").to_string();
    assert!(!m.is_empty());
    m
}

async fn bad_request(app: &Router, uri: &str) -> String {
    expect_error(app, uri, StatusCode::BAD_REQUEST, "invalid_request").await
}

/// `1.000,50` → 1000.5 (RFB capital strings).
fn reais(s: &str) -> f64 {
    s.replace('.', "").replace(',', ".").parse().unwrap_or_else(|_| panic!("capital {s}"))
}

const SNAPSHOT_REF: &str = r#"{"competence":"2026-10","source_snapshot":"D61010"}"#;

fn snapshot_ref() -> Value {
    serde_json::from_str(SNAPSHOT_REF).unwrap()
}

/// Columns of an establishment row (RFB layout, without `cnpj_basico`).
const ESTAB_COLUMNS: [&str; 29] = [
    "cnpj_ordem",
    "cnpj_dv",
    "identificador_matriz_filial",
    "nome_fantasia",
    "situacao_cadastral",
    "data_situacao_cadastral",
    "motivo_situacao_cadastral",
    "nome_cidade_exterior",
    "pais",
    "data_inicio_atividade",
    "cnae_fiscal_principal",
    "cnae_fiscal_secundaria",
    "tipo_logradouro",
    "logradouro",
    "numero",
    "complemento",
    "bairro",
    "cep",
    "uf",
    "municipio",
    "ddd_1",
    "telefone_1",
    "ddd_2",
    "telefone_2",
    "ddd_fax",
    "fax",
    "correio_eletronico",
    "situacao_especial",
    "data_situacao_especial",
];
const CONTACT_COLUMNS: [&str; 7] = ["ddd_1", "telefone_1", "ddd_2", "telefone_2", "ddd_fax", "fax", "correio_eletronico"];
const EMPRESA_COLUMNS: [&str; 6] =
    ["razao_social", "natureza_juridica", "qualificacao_responsavel", "capital_social", "porte_empresa", "ente_federativo_responsavel"];
const SOCIO_COLUMNS: [&str; 10] = [
    "identificador_socio",
    "nome_socio_razao_social",
    "cpf_cnpj_socio",
    "qualificacao_socio",
    "data_entrada_sociedade",
    "pais",
    "representante_legal",
    "nome_do_representante",
    "qualificacao_representante_legal",
    "faixa_etaria",
];
const SIMPLES_COLUMNS: [&str; 6] =
    ["opcao_pelo_simples", "data_opcao_simples", "data_exclusao_simples", "opcao_mei", "data_opcao_mei", "data_exclusao_mei"];

fn with(base: &[&str], extra: &[&str]) -> BTreeSet<String> {
    base.iter().chain(extra).map(|s| s.to_string()).collect()
}

const SEARCH_KEYS: [&str; 7] = ["total", "results", "expansion_truncated", "fuzzy_used", "next_cursor", "snapshot", "took_ms"];

// ---------------------------------------------------------------------------------------
// Service routes
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn root_lists_endpoints_and_every_listed_endpoint_is_routed() {
    let app = app();
    let (s, h, v) = get_json(&app, "/").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(h["content-type"], "application/json");
    assert_eq!(keys(&v), set(&["service", "version", "endpoints"]));
    assert_eq!(v["service"], "opencnpj");
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    let endpoints: Vec<&str> = v["endpoints"].as_array().unwrap().iter().map(|e| e.as_str().unwrap()).collect();
    assert_eq!(endpoints.len(), 14);
    for e in endpoints {
        let uri = e
            .replace("{cnpj}", "00000000000191")
            .replace("{cnpj_basico}", "00000000")
            .replace("{cpf}", "12345678901")
            .replace("{file}", "balanced.ocnpj");
        let (s, _, _) = get(&app, &uri).await;
        // `/search` and `/export` answer 400 without criteria; nothing advertised is a 404.
        assert!(s == StatusCode::OK || s == StatusCode::BAD_REQUEST, "{uri}: {s}");
    }
}

#[tokio::test]
async fn health_and_readiness() {
    let app = app();
    for p in ["/healthz", "/health"] {
        let r = send(&app, get_request(p)).await;
        assert_eq!(r.status, StatusCode::OK);
        assert_eq!(r.text(), "ok");
        assert_eq!(r.header("content-type"), Some("text/plain; charset=utf-8"));
    }
    let (s, h, v) = get_json(&app, "/readyz").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(h.get("cache-control"), None);
    assert_eq!(v, serde_json::json!({ "status": "ready", "competence": "2026-10", "documents": 310 }));
}

#[tokio::test]
async fn metrics_are_prometheus_text_per_matched_route() {
    let app = app();
    search(&app, "q=banco").await;
    search(&app, "uf=SP").await;
    bad_request(&app, "/api/v1/search?foo=1").await;
    get(&app, "/api/v1/cnpj/00000000000191").await;
    let r = send(&app, get_request("/metrics")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.header("content-type"), Some("text/plain; version=0.0.4"));
    let t = r.text();
    for line in [
        "opencnpj_requests_total{route=\"/api/v1/search\"} 3",
        "opencnpj_responses_total{route=\"/api/v1/search\",class=\"2xx\"} 2",
        "opencnpj_responses_total{route=\"/api/v1/search\",class=\"4xx\"} 1",
        "opencnpj_requests_total{route=\"/api/v1/cnpj/{cnpj}\"} 1",
        "opencnpj_snapshot_documents 310",
        "opencnpj_timeouts_total 0",
        "opencnpj_overload_rejections_total 0",
    ] {
        assert!(t.lines().any(|l| l == line), "missing `{line}` in\n{t}");
    }
    let bytes = std::fs::metadata(snapshot_path()).unwrap().len();
    assert!(t.contains(&format!("opencnpj_snapshot_bytes {bytes}\n")));
    for m in ["opencnpj_postings_cache_bytes ", "opencnpj_process_rss_anon_bytes ", "opencnpj_request_seconds_bucket{"] {
        assert!(t.contains(m), "missing {m}");
    }
}

#[tokio::test]
async fn unknown_route_is_404() {
    let app = app();
    assert_eq!(get(&app, "/api/v1/nope").await.0, StatusCode::NOT_FOUND);
    assert_eq!(get(&app, "/api/v2/search?q=banco").await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn admin_router_merges_the_public_api() {
    let public = app();
    let admin = admin_app_with(&[]);
    let a = search(&public, "q=mercado&uf=SP&limit=5").await;
    let (s, _, b) = get_json(&admin, "/api/v1/search?q=mercado&uf=SP&limit=5").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(a["results"], b["results"]);
    assert_eq!(a["total"], b["total"]);
    assert_eq!(get(&admin, "/admin/api/overview").await.0, StatusCode::OK);
    // The admin suite is not exposed on the public router.
    assert_eq!(get(&public, "/admin/api/overview").await.0, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------------------
// Lookups
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn cnpj_lookup_shape_and_values() {
    let app = app();
    let r = send(&app, get_request("/api/v1/cnpj/00000000000191")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.header("content-type"), Some("application/json"));
    assert_eq!(r.header("cache-control"), Some(CACHE));
    let v = r.json();
    assert_eq!(
        keys(&v),
        set(&[
            "cnpj",
            "cnpj_formatado",
            "cnpj_basico",
            "estabelecimento",
            "empresa",
            "socios",
            "simples",
            "total_estabelecimentos",
            "snapshot"
        ])
    );
    assert_eq!(v["cnpj"], "00000000000191");
    assert_eq!(v["cnpj_formatado"], "00.000.000/0001-91");
    assert_eq!(v["cnpj_basico"], "00000000");
    assert_eq!(v["total_estabelecimentos"], 2);
    assert_eq!(v["snapshot"], snapshot_ref());
    assert_eq!(v["socios"], serde_json::json!([]));
    assert_eq!(v["simples"], Value::Null);
    let e = &v["estabelecimento"];
    assert_eq!(
        keys(e),
        with(
            &ESTAB_COLUMNS,
            &[
                "identificador_matriz_filial_descricao",
                "situacao_cadastral_descricao",
                "motivo_situacao_cadastral_descricao",
                "cnae_fiscal_principal_descricao"
            ]
        ),
        "9701 has no município label"
    );
    assert_eq!(e["cnpj_ordem"], "0001");
    assert_eq!(e["cnpj_dv"], "91");
    assert_eq!(e["nome_fantasia"], "DIRECAO GERAL");
    assert_eq!(e["uf"], "DF");
    assert_eq!(e["municipio"], "9701");
    assert_eq!(e["identificador_matriz_filial_descricao"], "MATRIZ");
    assert_eq!(e["situacao_cadastral"], "02");
    assert_eq!(e["situacao_cadastral_descricao"], "ATIVA");
    assert_eq!(e["motivo_situacao_cadastral_descricao"], "SEM MOTIVO");
    assert_eq!(e["cnae_fiscal_principal"], "6422100");
    assert_eq!(e["cnae_fiscal_principal_descricao"], "Bancos múltiplos, com carteira comercial");
    assert_eq!(e["cnae_fiscal_secundaria"], "4711302,4712100");
    assert_eq!(e["data_inicio_atividade"], "20200101");
    assert_eq!(e["cep"], "01310100");
    assert_eq!(e["correio_eletronico"], "contato@example.com");
    assert_eq!(e["telefone_1"], "33334444");
    let emp = &v["empresa"];
    assert_eq!(keys(emp), with(&EMPRESA_COLUMNS, &["porte_empresa_descricao"]));
    assert_eq!(emp["razao_social"], "BANCO DO BRASIL SA");
    assert_eq!(emp["capital_social"], "120000000000,00");
    assert_eq!(emp["porte_empresa_descricao"], "DEMAIS");
}

#[tokio::test]
async fn cnpj_lookup_partners_simples_and_labels() {
    let app = app();
    let (_, _, v) = get_json(&app, "/api/v1/cnpj/11111111000191").await;
    let emp = &v["empresa"];
    assert_eq!(
        keys(emp),
        with(&EMPRESA_COLUMNS, &["natureza_juridica_descricao", "porte_empresa_descricao", "qualificacao_responsavel_descricao"])
    );
    assert_eq!(emp["razao_social"], "PADARIA SÃO JOÃO LTDA");
    assert_eq!(emp["capital_social"], "1.000,00", "exact source string");
    assert_eq!(emp["natureza_juridica_descricao"], "Sociedade Empresária Limitada");
    assert_eq!(emp["qualificacao_responsavel_descricao"], "Sócio-Administrador");
    assert_eq!(v["estabelecimento"]["municipio_descricao"], "SAO PAULO");
    assert_eq!(v["estabelecimento"]["nome_fantasia"], "PADARIA DO JOÃO");
    let socios = v["socios"].as_array().unwrap();
    assert_eq!(socios.len(), 1);
    let s = &socios[0];
    assert_eq!(keys(s), with(&SOCIO_COLUMNS, &["identificador_socio_descricao", "qualificacao_socio_descricao", "faixa_etaria_descricao"]));
    assert_eq!(s["nome_socio_razao_social"], "JOÃO DA SILVA");
    assert_eq!(s["cpf_cnpj_socio"], "***123456**");
    assert_eq!(s["identificador_socio_descricao"], "PESSOA FISICA");
    assert_eq!(s["qualificacao_socio_descricao"], "Sócio-Administrador");
    assert_eq!(s["faixa_etaria_descricao"], "41 A 50 ANOS");
    assert_eq!(s["representante_legal"], "***000000**");
    let sim = &v["simples"];
    assert_eq!(keys(sim), set(&SIMPLES_COLUMNS));
    assert_eq!(sim["opcao_pelo_simples"], "S");
    assert_eq!(sim["data_opcao_simples"], "20200101");
    assert_eq!(sim["opcao_mei"], "N");

    let (_, _, v) = get_json(&app, "/api/v1/cnpj/22222222000272").await;
    assert_eq!(v["empresa"]["razao_social"], "COMERCIO DE \"PEÇAS\" AUTO LTDA");
    assert_eq!(v["total_estabelecimentos"], 2);
    let e = &v["estabelecimento"];
    assert_eq!((e["municipio"].as_str(), e["cnae_fiscal_principal"].as_str()), (Some(""), Some("")));
    assert!(e.get("municipio_descricao").is_none() && e.get("cnae_fiscal_principal_descricao").is_none());
    assert_eq!(e["identificador_matriz_filial_descricao"], "FILIAL");
    let s = &v["socios"][0];
    assert_eq!(s["cpf_cnpj_socio"], "33000167000101");
    assert_eq!(s["identificador_socio_descricao"], "PESSOA JURIDICA");
    assert_eq!(s["faixa_etaria_descricao"], "NAO SE APLICA");
    assert_eq!(v["simples"]["opcao_mei"], "S");
    assert_eq!(v["simples"]["data_opcao_mei"], "20210101");
    assert_eq!(v["simples"]["opcao_pelo_simples"], "N");

    let (_, _, v) = get_json(&app, "/api/v1/cnpj/22222222000191").await;
    assert_eq!(v["estabelecimento"]["situacao_cadastral_descricao"], "BAIXADA");
}

#[tokio::test]
async fn cnpj_lookup_accepts_punctuation_case_and_alphanumeric() {
    let app = app();
    let (_, _, plain) = get_json(&app, "/api/v1/cnpj/00000000000191").await;
    for id in ["00.000.000%2F0001-91", "00.000.000%2F0001%2091", "00%20000%20000%200001%2091"] {
        let (s, _, v) = get_json(&app, &format!("/api/v1/cnpj/{id}")).await;
        assert_eq!(s, StatusCode::OK, "{id}");
        assert_eq!(v, plain, "{id}");
    }
    for id in ["12ABC34501DE35", "12abc34501de35", "12.ABC.345%2F01DE-35"] {
        let (s, _, v) = get_json(&app, &format!("/api/v1/cnpj/{id}")).await;
        assert_eq!(s, StatusCode::OK, "{id}");
        assert_eq!(v["cnpj"], "12ABC34501DE35");
        assert_eq!(v["cnpj_formatado"], "12.ABC.345/01DE-35");
        assert_eq!(v["cnpj_basico"], "12ABC345");
        assert_eq!(v["estabelecimento"]["cnpj_ordem"], "01DE");
        assert_eq!(v["estabelecimento"]["cnpj_dv"], "35");
        assert_eq!(v["empresa"]["razao_social"], "EMPRESA ALFANUMÉRICA LTDA");
    }
}

#[tokio::test]
async fn cnpj_root_lookup_and_establishment_paging() {
    let app = app();
    let (s, h, v) = get_json(&app, "/api/v1/cnpj/00000000").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(h["cache-control"], CACHE);
    assert_eq!(
        keys(&v),
        set(&[
            "cnpj_basico",
            "empresa",
            "estabelecimentos",
            "total_estabelecimentos",
            "estabelecimentos_offset",
            "socios",
            "simples",
            "snapshot"
        ])
    );
    assert_eq!(v["total_estabelecimentos"], 2);
    assert_eq!(v["estabelecimentos_offset"], 0);
    let cnpjs: Vec<&str> = v["estabelecimentos"].as_array().unwrap().iter().map(|e| e["cnpj"].as_str().unwrap()).collect();
    assert_eq!(cnpjs, ["00000000000191", "00000000000272"]);
    assert!(keys(&v["estabelecimentos"][0]).contains("cnpj"));

    let page = |o: usize, l: usize| format!("/api/v1/cnpj/00000000?estab_offset={o}&estab_limit={l}");
    let (_, _, v) = get_json(&app, &page(0, 1)).await;
    assert_eq!(v["estabelecimentos"].as_array().unwrap().len(), 1);
    assert_eq!(v["estabelecimentos"][0]["cnpj_ordem"], "0001");
    assert_eq!(v["total_estabelecimentos"], 2);
    let (_, _, v) = get_json(&app, &page(1, 1)).await;
    assert_eq!(v["estabelecimentos"][0]["cnpj_ordem"], "0002");
    assert_eq!(v["estabelecimentos_offset"], 1);
    let (_, _, v) = get_json(&app, &page(5, 10)).await;
    assert_eq!(v["estabelecimentos"], serde_json::json!([]));
    assert_eq!(v["total_estabelecimentos"], 2);
    assert_eq!(v["estabelecimentos_offset"], 5);

    bad_request(&app, "/api/v1/cnpj/00000000?estab_limit=0").await;
    bad_request(&app, "/api/v1/cnpj/00000000?estab_limit=10001").await;
    bad_request(&app, "/api/v1/cnpj/00000000?estab_limit=x").await;
    bad_request(&app, "/api/v1/cnpj/00000000?estab_offset=-1").await;

    // Company without establishments, and establishment without a company.
    let (_, _, v) = get_json(&app, "/api/v1/cnpj/33333333").await;
    assert_eq!(v["estabelecimentos"], serde_json::json!([]));
    assert_eq!(v["total_estabelecimentos"], 0);
    assert_eq!(v["empresa"]["razao_social"], "SEM ESTABELECIMENTO SA");
    assert_eq!(v["empresa"]["capital_social"], "1,5");
    let (_, _, v) = get_json(&app, "/api/v1/cnpj/99999999").await;
    assert_eq!(v["empresa"], Value::Null);
    assert_eq!(v["estabelecimentos"][0]["nome_fantasia"], "ORFAO SEM EMPRESA");
    let (_, _, v) = get_json(&app, "/api/v1/cnpj/99999999000191").await;
    assert_eq!(v["empresa"], Value::Null);
    assert_eq!(v["estabelecimento"]["uf"], "BA");
    // The alphanumeric root, any case.
    let (_, _, v) = get_json(&app, "/api/v1/cnpj/12abc345").await;
    assert_eq!(v["cnpj_basico"], "12ABC345");
    assert_eq!(v["estabelecimentos"][0]["cnpj"], "12ABC34501DE35");
}

#[tokio::test]
async fn lookup_aliases_share_the_contract() {
    let app = app();
    for id in ["00000000000272", "00000000", "11111111", "12ABC34501DE35"] {
        let (_, _, a) = get_json(&app, &format!("/api/v1/cnpj/{id}")).await;
        for alias in ["estabelecimentos", "empresas"] {
            let (s, h, b) = get_json(&app, &format!("/api/v1/{alias}/{id}")).await;
            assert_eq!(s, StatusCode::OK, "{alias}/{id}");
            assert_eq!(h["cache-control"], CACHE);
            assert_eq!(a, b, "{alias}/{id}");
        }
    }
    let (_, _, v) = get_json(&app, "/api/v1/empresas/00000000?estab_offset=1&estab_limit=1").await;
    assert_eq!(v["estabelecimentos"][0]["cnpj"], "00000000000272");
    expect_error(&app, "/api/v1/estabelecimentos/11222333000181", StatusCode::NOT_FOUND, "cnpj_not_found").await;
    expect_error(&app, "/api/v1/empresas/zz", StatusCode::BAD_REQUEST, "invalid_document").await;
}

#[tokio::test]
async fn cnpj_lookup_errors() {
    let app = app();
    let nf = |id: &str| format!("/api/v1/cnpj/{id}");
    // Valid but absent (a third Banco do Brasil branch has valid check digits).
    let dv = opencnpj_core::cnpj::check_digits(b"000000000003");
    let absent_branch = format!("000000000003{}", std::str::from_utf8(&dv).unwrap());
    for id in ["11222333000181", "55555556", absent_branch.as_str()] {
        let m = expect_error(&app, &nf(id), StatusCode::NOT_FOUND, "cnpj_not_found").await;
        assert_eq!(m, "cnpj not found");
    }
    // Malformed: wrong length, wrong check digits, garbage, too long.
    for id in ["abc", "1234567", "000000000001", "00000000000192", "ABCDEFG!", "0000000000019", "000000000001911"] {
        expect_error(&app, &nf(id), StatusCode::BAD_REQUEST, "invalid_document").await;
    }
    let m = expect_error(&app, &nf("00000000000192"), StatusCode::BAD_REQUEST, "invalid_document").await;
    assert!(m.contains("check digits"), "{m}");
    let long = "1".repeat(33);
    let m = expect_error(&app, &nf(&long), StatusCode::BAD_REQUEST, "invalid_document").await;
    assert_eq!(m, "invalid cnpj");
    // 32 characters is still parsed (punctuation is stripped) instead of being rejected early.
    let p32 = format!("{}00000000000191", " ".repeat(18)).replace(' ', "%20");
    assert_eq!(get(&app, &nf(&p32)).await.0, StatusCode::OK);
}

#[tokio::test]
async fn cpf_lookup_owner_exact_match() {
    let app = app();
    for cpf in ["12345678901", "123.456.789-01"] {
        let (s, h, v) = get_json(&app, &format!("/api/v1/cpf/{cpf}")).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(h["cache-control"], CACHE);
        assert_eq!(keys(&v), set(&["cpf", "titular", "socios", "nota", "snapshot"]));
        assert_eq!(v["cpf"], "12345678901");
        assert_eq!(v["snapshot"], snapshot_ref());
        let t = &v["titular"];
        assert_eq!(keys(t), set(&["match", "total", "results"]));
        assert_eq!(t["match"], "exact");
        assert_eq!(t["total"], 1);
        let r = &t["results"][0];
        assert_eq!(keys(r), set(&["cnpj_basico", "razao_social", "natureza_juridica", "porte_empresa", "capital_social"]));
        assert_eq!(r["cnpj_basico"], "55555555");
        assert_eq!(r["razao_social"], "MARIA DA SILVA 12345678901");
        let so = &v["socios"];
        assert_eq!(keys(so), set(&["match", "mascara", "total_documentos", "results"]));
        assert_eq!(so["match"], "possible");
        assert_eq!(so["mascara"], "***456789**");
        assert_eq!(so["total_documentos"], 0);
        assert_eq!(so["results"], serde_json::json!([]));
    }
}

#[tokio::test]
async fn cpf_lookup_masked_partner_possible_match() {
    let app = app();
    let (s, _, v) = get_json(&app, "/api/v1/cpf/000.123.456-00").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["cpf"], "00012345600");
    assert_eq!(v["titular"]["total"], 0);
    assert_eq!(v["titular"]["results"], serde_json::json!([]));
    let so = &v["socios"];
    assert_eq!(so["mascara"], "***123456**");
    assert_eq!(so["total_documentos"], 1);
    let r = &so["results"][0];
    assert_eq!(r["cnpj"], "11111111000191");
    assert_eq!(r["razao_social"], "PADARIA SÃO JOÃO LTDA");
    let rows = r["socios_correspondentes"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["nome_socio_razao_social"], "JOÃO DA SILVA");
    assert_eq!(rows[0]["cpf_cnpj_socio"], "***123456**");
    // Any CPF with the same middle digits is a possible match.
    let (_, _, w) = get_json(&app, "/api/v1/cpf/99912345699").await;
    assert_eq!(w["socios"]["results"][0]["cnpj"], "11111111000191");
    // The all-zero representative mask is never indexed.
    let (_, _, z) = get_json(&app, "/api/v1/cpf/00000000000").await;
    assert_eq!(z["socios"]["total_documentos"], 0);
}

#[tokio::test]
async fn cpf_lookup_limit_and_errors() {
    let app = app();
    // `limit` is clamped to 1..=max_limit rather than rejected.
    for l in ["0", "1", "100000"] {
        let (s, _, v) = get_json(&app, &format!("/api/v1/cpf/12345678901?limit={l}")).await;
        assert_eq!(s, StatusCode::OK, "limit={l}");
        assert_eq!(v["titular"]["results"].as_array().unwrap().len(), 1);
    }
    bad_request(&app, "/api/v1/cpf/12345678901?limit=x").await;
    for cpf in ["123", "1234567890", "123456789012", "abcdefghijk"] {
        let m = expect_error(&app, &format!("/api/v1/cpf/{cpf}"), StatusCode::BAD_REQUEST, "invalid_document").await;
        assert_eq!(m, "cpf must have 11 digits");
    }
}

// ---------------------------------------------------------------------------------------
// Search routes and response shape
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn search_response_shape() {
    let app = app();
    let r = send(&app, get_request("/api/v1/search?q=banco")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.header("content-type"), Some("application/json"));
    assert_eq!(r.header("cache-control"), Some(CACHE));
    let v = r.json();
    assert_eq!(keys(&v), set(&SEARCH_KEYS));
    assert_eq!(v["total"], 2);
    assert_eq!(v["expansion_truncated"], false);
    assert_eq!(v["fuzzy_used"], false);
    assert_eq!(v["next_cursor"], Value::Null);
    assert_eq!(v["snapshot"], snapshot_ref());
    let took = &v["took_ms"];
    assert_eq!(keys(took), set(&["search", "total"]));
    let (ts, tt) = (took["search"].as_f64().unwrap(), took["total"].as_f64().unwrap());
    assert!(ts >= 0.0 && tt >= ts, "{took}");

    // Ranking: same bucket, both active → CNPJ order.
    let res = v["results"].as_array().unwrap();
    assert_eq!(res.iter().map(hit_id).collect::<Vec<_>>(), ["00000000000191", "00000000000272"]);
    let base = [
        "cnpj_basico",
        "razao_social",
        "cnpj",
        "nome_fantasia",
        "matriz_filial",
        "situacao_cadastral",
        "situacao_cadastral_descricao",
        "uf",
        "municipio",
        "cnae_fiscal_principal",
        "data_inicio_atividade",
        "cep",
        "natureza_juridica",
        "porte_empresa",
        "capital_social",
    ];
    assert_eq!(keys(&res[0]), with(&base, &["cnae_fiscal_principal_descricao"]));
    assert_eq!(keys(&res[1]), with(&base, &["cnae_fiscal_principal_descricao", "municipio_descricao"]));
    let a = &res[1];
    assert_eq!(a["razao_social"], "BANCO DO BRASIL SA");
    assert_eq!(a["nome_fantasia"], "AGENCIA CENTRO");
    assert_eq!(a["matriz_filial"], "2");
    assert_eq!(a["situacao_cadastral_descricao"], "ATIVA");
    assert_eq!(a["municipio_descricao"], "SAO PAULO");
    assert_eq!(a["capital_social"], "120000000000,00");

    // With Simples: opcao_simples / opcao_mei.
    let v = search(&app, "q=padaria").await;
    assert_eq!(keys(&v["results"][0]), with(&base, &["municipio_descricao", "opcao_simples", "opcao_mei"]));
    assert_eq!(v["results"][0]["opcao_simples"], "S");
    assert_eq!(v["results"][0]["opcao_mei"], "N");
    // Company without establishments: company fields only.
    let v = search(&app, "q=joana").await;
    assert_eq!(v["total"], 1);
    assert_eq!(keys(&v["results"][0]), set(&["cnpj_basico", "razao_social", "natureza_juridica", "porte_empresa", "capital_social"]));
    assert_eq!(v["results"][0]["cnpj_basico"], "44444444");
    // Establishment without a company: establishment fields and an empty razão social.
    let v = search(&app, "q=orfao").await;
    let r = &v["results"][0];
    assert_eq!(
        keys(r),
        set(&[
            "cnpj_basico",
            "razao_social",
            "cnpj",
            "nome_fantasia",
            "matriz_filial",
            "situacao_cadastral",
            "situacao_cadastral_descricao",
            "uf",
            "municipio",
            "cnae_fiscal_principal",
            "data_inicio_atividade",
            "cep"
        ])
    );
    assert_eq!(r["razao_social"], "");
    assert_eq!(r["cnpj"], "99999999000191");
}

#[tokio::test]
async fn search_route_aliases() {
    let app = app();
    let qs = "q=agencia&limit=5";
    let a = search(&app, qs).await;
    assert_eq!(a["total"], 1);
    assert_eq!(a["results"][0]["cnpj"], "00000000000272");
    for route in ["/api/v1/busca", "/api/v1/estabelecimentos/search"] {
        let (s, h, b) = get_json(&app, &format!("{route}?{qs}")).await;
        assert_eq!(s, StatusCode::OK, "{route}");
        assert_eq!(h["cache-control"], CACHE);
        assert_eq!(keys(&b), set(&SEARCH_KEYS));
        assert_eq!(a["results"], b["results"], "{route}");
        assert_eq!(a["total"], b["total"], "{route}");
    }
    for route in ["/api/v1/busca", "/api/v1/estabelecimentos/search", "/api/v1/empresas/search"] {
        bad_request(&app, route).await;
        bad_request(&app, &format!("{route}?nope=1")).await;
    }
}

#[tokio::test]
async fn legacy_empresas_search_defaults_to_razao_social() {
    let app = app();
    let get_total = |uri: &'static str| {
        let app = app.clone();
        async move {
            let (s, _, v) = get_json(&app, uri).await;
            assert_eq!(s, StatusCode::OK, "{uri}");
            assert_eq!(keys(&v), set(&SEARCH_KEYS));
            v["total"].as_u64().unwrap()
        }
    };
    // "direcao" only exists as a nome fantasia.
    assert_eq!(get_total("/api/v1/empresas/search?q=direcao&fuzzy=false").await, 0);
    assert_eq!(get_total("/api/v1/empresas/search?nome=direcao&fuzzy=false").await, 0);
    assert_eq!(total(&app, "q=direcao&fuzzy=false").await, 1);
    // An explicit `field` wins over the legacy default.
    assert_eq!(get_total("/api/v1/empresas/search?q=direcao&field=any&fuzzy=false").await, 1);
    assert_eq!(get_total("/api/v1/empresas/search?q=direcao&field=nome_fantasia&fuzzy=false").await, 1);
    assert_eq!(get_total("/api/v1/empresas/search?razao_social=banco").await, 2);
    // Filters work as on /search.
    assert_eq!(get_total("/api/v1/empresas/search?q=mercado&uf=PR").await, 150);
}

#[tokio::test]
async fn legacy_empresas_search_honours_nome_fantasia_parameter() {
    let app = app();
    // `nome_fantasia=` selects the fantasia field on /search; the legacy default
    // (razão social) silently overrides it on /empresas/search, returning 0.
    let (_, _, v) = get_json(&app, "/api/v1/empresas/search?nome_fantasia=agencia&fuzzy=false").await;
    assert_eq!(v["total"], 1);
}

// ---------------------------------------------------------------------------------------
// Text parameters
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn text_aliases_and_field_selection() {
    let app = app();
    // q / nome search both fields; razao_social / nome_fantasia restrict to one.
    assert_eq!(total(&app, "q=direcao&fuzzy=false").await, 1);
    assert_eq!(total(&app, "nome=direcao&fuzzy=false").await, 1);
    assert_eq!(total(&app, "razao_social=direcao&fuzzy=false").await, 0);
    assert_eq!(total(&app, "nome_fantasia=direcao&fuzzy=false").await, 1);
    assert_eq!(total(&app, "razao_social=banco&fuzzy=false").await, 2);
    assert_eq!(total(&app, "nome_fantasia=banco&fuzzy=false").await, 0);
    for (field, mercadinho, mercado) in [
        ("any", 300, 300),
        ("todos", 300, 300),
        ("razao_social", 0, 300),
        ("razao", 0, 300),
        ("nome_fantasia", 300, 0),
        ("fantasia", 300, 0),
    ] {
        assert_eq!(total(&app, &format!("q=mercadinho&field={field}&fuzzy=false")).await, mercadinho, "field={field}");
        assert_eq!(total(&app, &format!("q=mercado%20&campo={field}&fuzzy=false")).await, mercado, "campo={field}");
    }
    let m = bad_request(&app, "/api/v1/search?q=mercado&field=cnpj").await;
    assert_eq!(m, "field must be any, razao_social or nome_fantasia");
    bad_request(&app, "/api/v1/search?q=mercado&campo=").await;
}

#[tokio::test]
async fn text_matching_rules() {
    let app = app();
    // Accents and case folded; stopwords ignored; every token must match.
    assert_eq!(total(&app, "q=PADARIA%20sao%20JO%C3%83O").await, 1);
    assert_eq!(total(&app, "q=padaria%20da%20joao").await, 1);
    assert_eq!(total(&app, "q=padaria%20banco&fuzzy=false").await, 0);
    // The last token is a prefix unless the query ends with a space.
    assert_eq!(total(&app, "q=banco%20bras").await, 2);
    assert_eq!(total(&app, "q=banco%20bras%20&fuzzy=false").await, 0);
    assert_eq!(total(&app, "q=mercad").await, 300);
    assert_eq!(total(&app, "q=mercado%2017%20").await, 1);
    assert_eq!(total(&app, "q=mercadinho+17+").await, 1);
    assert_eq!(search(&app, "q=mercadinho+17+").await["results"][0]["cnpj"], "40000017000163");
    assert_eq!(total(&app, "q=mercado%2017").await, 11, "17 and 170..179");
    // Only stopwords: no match, not an error.
    let v = search(&app, "q=da%20de").await;
    assert_eq!((v["total"].as_u64(), v["results"].as_array().unwrap().len()), (Some(0), 0));
    // A CNPJ or root typed in the box is a CNPJ prefix search; 11 digits is an owner CPF.
    assert_eq!(all_ids(&app, "q=00.000.000").await, ["00000000000191", "00000000000272"]);
    assert_eq!(all_ids(&app, "q=00000000000191").await, ["00000000000191"]);
    assert_eq!(all_ids(&app, "q=55555555").await, ["55555555"]);
    assert_eq!(all_ids(&app, "q=12345678901").await, ["55555555"]);
    assert_eq!(all_ids(&app, "q=maria%20silva%2012345678901%20").await, ["55555555"]);
    assert_eq!(all_ids(&app, "q=joana%2055555555").await, ["44444444"]);
}

#[tokio::test]
async fn too_many_tokens_sets_expansion_truncated() {
    let app = app();
    // Only the first 8 tokens are used; the flag tells the client.
    let v = search(&app, "q=mercado%20numero%2017%20ltda%20mercado%20numero%2017%20ltda%20zzzz&fuzzy=false").await;
    assert_eq!(v["expansion_truncated"], true);
    assert_eq!(all_ids(&app, "q=mercado%20numero%2017%20ltda&fuzzy=false").await, ["40000017000163"]);
    assert_eq!(v["results"][0]["cnpj"], "40000017000163");
    assert_eq!(search(&app, "q=mercado%20numero%2017%20ltda").await["expansion_truncated"], false);
}

#[tokio::test]
async fn typo_tolerance_explicit_and_automatic() {
    let app = app();
    // substitution, deletion, insertion, adjacent transposition of "padaria".
    for typo in ["padarua", "padria", "padarria", "pdaaria"] {
        let exact = search(&app, &format!("q={typo}%20&fuzzy=false")).await;
        assert_eq!((exact["total"].as_u64(), exact["fuzzy_used"].as_bool()), (Some(0), Some(false)), "{typo} exact");
        for fz in ["true", "1", "sim", "TRUE"] {
            let v = search(&app, &format!("q={typo}%20&fuzzy={fz}")).await;
            assert_eq!(all_ids(&app, &format!("q={typo}%20&fuzzy={fz}")).await, ["11111111000191"], "{typo} fuzzy={fz}");
            // Explicit fuzzy is not an automatic retry.
            assert_eq!(v["fuzzy_used"], false, "{typo} fuzzy={fz}");
        }
        for auto in [format!("q={typo}%20"), format!("q={typo}%20&fuzzy=auto")] {
            let v = search(&app, &auto).await;
            assert_eq!(v["total"], 1, "{auto}");
            assert_eq!(v["fuzzy_used"], true, "{auto}");
            assert_eq!(v["results"][0]["cnpj"], "11111111000191");
        }
        for off in ["false", "0", "nao", "não"] {
            assert_eq!(total(&app, &format!("q={typo}%20&fuzzy={}", off.replace('ã', "%C3%A3"))).await, 0, "{typo} {off}");
        }
    }
    // Two tokens with a typo; tokens shorter than 4 characters are never fuzzy.
    assert_eq!(all_ids(&app, "q=merkado%2017%20&fuzzy=true").await, ["40000017000163"]);
    assert_eq!(total(&app, "q=bamco%20&fuzzy=true").await, 2);
    assert_eq!(total(&app, "q=sx%20&fuzzy=true").await, 0);
    // Enough exact hits: no retry.
    let v = search(&app, "q=padaria%20").await;
    assert_eq!((v["total"].as_u64(), v["fuzzy_used"].as_bool()), (Some(1), Some(false)));
    let m = bad_request(&app, "/api/v1/search?q=padaria&fuzzy=maybe").await;
    assert_eq!(m, "fuzzy must be auto, true or false");
}

// ---------------------------------------------------------------------------------------
// Paging and sorting
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn limit_offset_defaults_and_errors() {
    let app = app();
    let v = search(&app, "q=mercado").await;
    assert_eq!(v["total"], 300);
    assert_eq!(v["results"].as_array().unwrap().len(), 20, "default limit");
    let v = search(&app, "q=mercado&limit=7&offset=10").await;
    let ids: Vec<String> = v["results"].as_array().unwrap().iter().map(hit_id).collect();
    let all = all_ids(&app, "q=mercado").await;
    assert_eq!(ids, all[10..17]);
    assert_eq!(search(&app, "q=mercado&limit=10000").await["results"].as_array().unwrap().len(), 300);
    let v = search(&app, "q=mercado&offset=1000").await;
    assert_eq!((v["total"].as_u64(), v["results"].as_array().unwrap().len()), (Some(300), 0));
    let v = search(&app, "uf=SP&offset=1000").await;
    assert_eq!((v["total"].as_u64(), v["results"].as_array().unwrap().len()), (Some(152), 0));
    assert_eq!(v["next_cursor"], Value::Null);
    for bad in ["limit=0", "limit=10001", "limit=-1", "limit=x", "offset=x", "offset=-1", "cursor=x", "cursor=-1"] {
        bad_request(&app, &format!("/api/v1/search?q=mercado&{bad}")).await;
    }
    assert_eq!(bad_request(&app, "/api/v1/search?q=mercado&limit=0").await, "limit must be between 1 and 10000");
}

#[tokio::test]
async fn cursor_paging_for_filter_only_queries() {
    let app = app();
    let all = all_ids(&app, "uf=SP").await;
    assert_eq!(all.len(), 152);
    // 152 = 76 + 76: the second page is full, so it still has a cursor; the third is empty.
    let p1 = search(&app, "uf=SP&limit=76").await;
    let c1 = p1["next_cursor"].as_str().expect("cursor").to_string();
    assert!(c1.parse::<u32>().is_ok());
    let p2 = search(&app, &format!("uf=SP&limit=76&cursor={c1}")).await;
    assert_eq!(p2["total"], 152, "total ignores the cursor");
    let c2 = p2["next_cursor"].as_str().expect("cursor").to_string();
    let p3 = search(&app, &format!("uf=SP&limit=76&cursor={c2}")).await;
    assert_eq!(p3["results"], serde_json::json!([]));
    assert_eq!(p3["next_cursor"], Value::Null);
    let mut paged: Vec<String> = Vec::new();
    for p in [&p1, &p2] {
        paged.extend(p["results"].as_array().unwrap().iter().map(hit_id));
    }
    assert_eq!(paged, all);
    // Short last page: no cursor.
    let p1 = search(&app, "uf=SP&limit=100").await;
    let p2 = search(&app, &format!("uf=SP&limit=100&cursor={}", p1["next_cursor"].as_str().unwrap())).await;
    assert_eq!(p2["results"].as_array().unwrap().len(), 52);
    assert_eq!(p2["next_cursor"], Value::Null);
    // Cursors are document ids: `cnpj=4` starts at the first MERCADO document (id 7).
    assert_eq!(search(&app, "cnpj=4&limit=1").await["next_cursor"], "7");
    // No cursor for text queries or sorted queries, even with a full page.
    assert_eq!(search(&app, "q=mercado&limit=5").await["next_cursor"], Value::Null);
    assert_eq!(search(&app, "uf=SP&limit=5&ordenar=capital_social:desc").await["next_cursor"], Value::Null);
}

#[tokio::test]
async fn sort_by_numeric_columns() {
    let app = app();
    let caps = |v: &Value| -> Vec<Option<f64>> {
        v["results"].as_array().unwrap().iter().map(|r| r.get("capital_social").and_then(Value::as_str).map(reais)).collect()
    };
    // Situação 02 (306 documents, all with capital except the orphan establishment).
    for (dir, desc) in [("desc", true), ("decrescente", true), ("asc", false), ("", false)] {
        let ord = if dir.is_empty() { "capital_social".to_string() } else { format!("capital_social:{dir}") };
        for key in ["ordenar", "sort"] {
            let v = search(&app, &format!("situacao=02&{key}={ord}&limit=10000")).await;
            assert_eq!(v["total"], 306);
            assert_eq!(v["next_cursor"], Value::Null);
            let c = caps(&v);
            assert_eq!(c.len(), 306);
            // Documents without a value come last, in both directions.
            assert_eq!(c.last().unwrap(), &None, "{key}={ord}");
            assert_eq!(v["results"][305]["cnpj"], "99999999000191");
            let present: Vec<f64> = c.iter().flatten().copied().collect();
            assert_eq!(present.len(), 305);
            assert!(present.windows(2).all(|w| if desc { w[0] >= w[1] } else { w[0] <= w[1] }), "{key}={ord}");
            if desc {
                assert_eq!(hit_id(&v["results"][0]), "00000000000191");
            } else {
                assert_eq!(hit_id(&v["results"][0]), "12ABC34501DE35", "capital 0,00");
            }
        }
    }
    // Ties keep document (CNPJ) order; offset applies after sorting.
    let all = all_ids(&app, "situacao=02&ordenar=capital_social:desc").await;
    let page = search(&app, "situacao=02&ordenar=capital_social:desc&limit=5&offset=3").await;
    assert_eq!(page["results"].as_array().unwrap().iter().map(hit_id).collect::<Vec<_>>(), all[3..8]);
    assert_eq!(all[2], "11111111000191");
    assert_eq!(all[3], "22222222000272");
    // Sorting a text query.
    let v = search(&app, "q=banco%20&ordenar=data_inicio_atividade:desc").await;
    assert_eq!(v["total"], 2);
    // Every numeric column is sortable.
    for f in opencnpj_store::numeric::NumField::ALL {
        for d in ["asc", "desc"] {
            let v = search(&app, &format!("uf=SP&ordenar={}:{d}&limit=3", f.name())).await;
            assert_eq!(v["results"].as_array().unwrap().len(), 3, "{}", f.name());
        }
    }
    for bad in ["uf", "razao_social", "capital", "", "data_inicio_atividade_de"] {
        let m = bad_request(&app, &format!("/api/v1/search?uf=SP&ordenar={bad}:desc")).await;
        assert_eq!(m, format!("cannot sort by {bad}"));
    }
}

#[tokio::test]
async fn sort_rejects_unknown_direction() {
    let app = app();
    bad_request(&app, "/api/v1/search?uf=SP&ordenar=capital_social:sideways").await;
}

// ---------------------------------------------------------------------------------------
// Filters
// ---------------------------------------------------------------------------------------

/// One example per `Field::FILTERS` name with its exact count on the fixture.
const FILTER_EXAMPLES: [(&str, &str, u64); 18] = [
    ("cpf", "12345678901", 1),
    ("cpf_titular", "12345678901", 1),
    ("socio_documento", "33000167000101", 2),
    ("motivo", "00", 307),
    ("cep", "01310100", 307),
    ("ddd", "11", 307),
    ("contato", "telefone", 307),
    ("cnae_qualquer", "6422100", 2),
    ("uf", "RJ", 1),
    ("municipio", "6001", 1),
    ("cnae", "6201501", 1),
    ("cnae_secundaria", "4711302", 307),
    ("situacao", "08", 1),
    ("porte", "03", 1),
    ("natureza", "2054", 1),
    ("simples", "S", 1),
    ("mei", "S", 2),
    ("matriz_filial", "2", 3),
];

#[tokio::test]
async fn every_filter_field_is_accepted_with_exact_counts() {
    let app = app();
    let names: BTreeSet<&str> = opencnpj_store::postings::Field::FILTERS.iter().map(|f| f.name()).collect();
    let covered: BTreeSet<&str> = FILTER_EXAMPLES.iter().map(|e| e.0).collect();
    assert_eq!(names, covered, "every filter field has an example");
    for (name, value, n) in FILTER_EXAMPLES {
        assert_eq!(total(&app, &format!("{name}={value}")).await, n, "{name}={value}");
    }
    // /campos lists the same filters.
    let (_, _, c) = get_json(&app, "/api/v1/campos").await;
    let listed: BTreeSet<&str> = c["filtros"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(listed, names);
}

#[tokio::test]
async fn filter_aliases() {
    let app = app();
    for (qs, n) in [
        ("cnae_principal=6422100", 2),
        ("cnae_fiscal_principal=6422100", 2),
        ("cnae_fiscal_secundaria=4712100", 307),
        ("cnae_any=1091102", 1),
        ("cnae_todos=4530703", 1),
        ("situacao_cadastral=02", 306),
        ("porte_empresa=05", 3),
        ("natureza_juridica=2135", 2),
        ("opcao_simples=N", 2),
        ("opcao_pelo_simples=S", 1),
        ("opcao_mei=N", 1),
        ("matriz=1", 304),
        ("identificador_matriz_filial=2", 3),
        ("motivo_situacao_cadastral=00", 307),
        ("socio_cnpj=33.000.167%2F0001-01", 2),
        ("socio_cpf=000.123.456-00", 1),
        ("socio=123456", 1),
        ("socio_documento=***123456**", 1),
    ] {
        assert_eq!(total(&app, qs).await, n, "{qs}");
    }
    // Alias and canonical name select the same documents.
    assert_eq!(id_set(&app, "cnae_principal=47").await, id_set(&app, "cnae=47").await);
    assert_eq!(id_set(&app, "socio_cpf=00012345600").await, id_set(&app, "socio_documento=00012345600").await);
}

#[tokio::test]
async fn value_normalization() {
    let app = app();
    for (qs, n) in [
        ("uf=sp", 152),
        ("uf=%20SP%20", 152),
        ("simples=s", 1),
        ("mei=s", 2),
        ("contato=EMAIL", 307),
        ("contato=email", 307),
        ("ddd=011", 307),
        ("cep=01310-100", 307),
        ("cnae=6422-1%2F00", 2),
        ("cpf=123.456.789-01", 1),
        ("cpf_titular=123.456.789-01", 1),
    ] {
        assert_eq!(total(&app, qs).await, n, "{qs}");
    }
}

#[tokio::test]
async fn cnae_hierarchy_prefixes_and_sections() {
    let app = app();
    for (v, n) in [
        ("47", 301), // divisão: 4712100 x300 + 4711302 (orphan)
        ("47*", 301),
        ("471", 301), // grupo
        ("4712", 300),
        ("47121", 300), // classe
        ("4712*", 300),
        ("4712100", 300), // subclasse
        ("4712100*", 300),
        ("4711302", 1),
        ("G", 302), // 45–47: + 4530703
        ("g", 302),
        ("K", 2), // 64–66: banks
        ("C", 1), // 10–33: 1091102
        ("J", 1), // 58–63: 6201501
        ("A", 0),
        ("99", 0),
    ] {
        assert_eq!(total(&app, &format!("cnae={v}")).await, n, "cnae={v}");
    }
    assert_eq!(total(&app, "cnae_secundaria=47").await, 307);
    assert_eq!(total(&app, "cnae_secundaria=G").await, 307);
    assert_eq!(total(&app, "cnae_secundaria=64").await, 0);
    assert_eq!(total(&app, "cnae_qualquer=64").await, 2);
    assert_eq!(total(&app, "cnae_qualquer=G").await, 307);
    assert_eq!(total(&app, "cnae_qualquer=1091102").await, 1);
    assert_eq!(bad_request(&app, "/api/v1/search?cnae=Z").await, "unknown CNAE section Z");
    assert_eq!(bad_request(&app, "/api/v1/search?cnae=12345678").await, "bad CNAE 12345678");
    bad_request(&app, "/api/v1/search?cnae=*").await;
    bad_request(&app, "/api/v1/search?cnae_qualquer=GG").await;
    bad_request(&app, "/api/v1/search?cnae_secundaria=Z").await;
}

#[tokio::test]
async fn code_prefixes_cep_natureza_and_star() {
    let app = app();
    for (qs, n) in [
        ("cep=01310", 307),
        ("cep=0131*", 307),
        ("cep=01310100*", 307),
        ("cep=99", 0),
        ("natureza=21", 2),
        ("natureza=20", 307), // 2038 x2, 2062 x304, 2054 x1
        ("natureza=2062", 304),
        ("natureza=206*", 304),
        ("uf=S*", 152),
        ("municipio=71*", 302),
        ("municipio=71", 0), // only CEP/natureza/CNAE treat short codes as prefixes
        ("situacao=0*", 307),
        ("porte=0*", 309),
    ] {
        assert_eq!(total(&app, qs).await, n, "{qs}");
    }
    assert_eq!(bad_request(&app, "/api/v1/search?cep=123456789").await, "bad cep 123456789");
    bad_request(&app, "/api/v1/search?natureza=20621").await;
    bad_request(&app, "/api/v1/search?cep=abc").await;
}

#[tokio::test]
async fn identity_filters_cpf_and_partners() {
    let app = app();
    // cpf: owner (exact) or masked partner (possible).
    assert_eq!(all_ids(&app, "cpf=12345678901").await, ["55555555"]);
    assert_eq!(all_ids(&app, "cpf=00012345600").await, ["11111111000191"]);
    assert_eq!(id_set(&app, "cpf=12345678901,00012345600").await, set(&["55555555", "11111111000191"]));
    // cpf_titular: owner only.
    assert_eq!(all_ids(&app, "cpf_titular=12345678901").await, ["55555555"]);
    assert_eq!(total(&app, "cpf_titular=00012345600").await, 0);
    // Partner documents: CNPJ (exact), CPF (6 middle digits), or the 6 digits alone.
    assert_eq!(all_ids(&app, "socio_cnpj=33000167000101").await, ["22222222000191", "22222222000272"]);
    assert_eq!(all_ids(&app, "socio_cpf=99912345699").await, ["11111111000191"]);
    assert_eq!(total(&app, "socio_documento=000000").await, 0, "zero mask is not indexed");
    assert_eq!(bad_request(&app, "/api/v1/search?cpf=123").await, "cpf must have 11 digits");
    bad_request(&app, "/api/v1/search?cpf_titular=1234567890").await;
    for v in ["123", "1234567", "330001670001011"] {
        let m = bad_request(&app, &format!("/api/v1/search?socio_cpf={v}")).await;
        assert!(m.starts_with("socio document must be"), "{m}");
    }
}

#[tokio::test]
async fn cnpj_prefix_filter() {
    let app = app();
    for (qs, n) in [
        ("cnpj=4", 301), // 40000000..40000299 + 44444444
        ("cnpj=4000001", 10),
        ("cnpj=40000017", 1),
        ("cnpj=00000000", 2),
        ("cnpj=00000000000", 2),
        ("cnpj=000000000002", 1),
        ("cnpj=00000000000191", 1),
        ("cnpj=00.000.000%2F0002", 1),
        ("cnpj=00.000.000%2F0002-72", 1),
        ("cnpj=12abc", 1),
        ("cnpj=12ABC34501DE", 1),
        ("cnpj=12ABC34501DF", 0),
        ("cnpj=3", 1),
        ("cnpj=8", 0),
        ("cnpj_basico=11111111", 1),
        ("cnpj_prefixo=22222222", 2),
        // Comma-separated prefixes are OR-ed; so are repeated `cnpj` parameters.
        ("cnpj=00000000,11111111", 3),
        ("cnpj=0&cnpj=1", 4),
        ("cnpj=0,", 2),
        // With other criteria: AND.
        ("cnpj=4&uf=PR", 150),
        ("cnpj=4&q=mercado%2017", 11),
    ] {
        assert_eq!(total(&app, qs).await, n, "{qs}");
    }
    assert_eq!(all_ids(&app, "cnpj=12abc").await, ["12ABC34501DE35"]);
    // Prefixes past 8 characters select establishments of one company.
    assert_eq!(all_ids(&app, "cnpj=000000000002").await, ["00000000000272"]);
    let m = bad_request(&app, "/api/v1/search?cnpj=123456789012345").await;
    assert_eq!(m, "cnpj prefix must have 1 to 14 characters");
    bad_request(&app, "/api/v1/search?cnpj=..%2F--").await;
}

#[tokio::test]
async fn comma_values_are_or_and_repeated_parameters_are_and() {
    let app = app();
    let sp = id_set(&app, "uf=SP").await;
    let pr = id_set(&app, "uf=PR").await;
    let or = id_set(&app, "uf=SP,PR").await;
    assert_eq!(or.len(), 302);
    assert_eq!(or, sp.union(&pr).cloned().collect());
    assert_eq!(id_set(&app, "uf=SP,%20PR,").await, or, "blank items are skipped");
    assert_eq!(total(&app, "uf=SP&uf=PR").await, 0);
    assert_eq!(id_set(&app, "uf=SP,PR&uf=PR").await, pr);
    // Different parameters: intersection.
    let c64 = id_set(&app, "cnae=64").await;
    let both = id_set(&app, "uf=SP&cnae=64").await;
    assert_eq!(both, sp.intersection(&c64).cloned().collect());
    assert_eq!(both, set(&["00000000000272"]));
    assert_eq!(total(&app, "uf=SP&situacao=02&cnae=47&municipio=7107").await, 150);
    assert_eq!(total(&app, "cnae=64,1091102").await, 3);
    assert_eq!(total(&app, "matriz_filial=1&matriz_filial=2").await, 0);
}

#[tokio::test]
async fn composite_filters() {
    let app = app();
    for t in ["true", "1", "sim", "s", "yes", "TRUE"] {
        assert_eq!(all_ids(&app, &format!("mei_ativo={t}")).await, ["22222222000191", "22222222000272"], "mei_ativo={t}");
        assert_eq!(all_ids(&app, &format!("simples_ativo={t}")).await, ["11111111000191"], "simples_ativo={t}");
        assert_eq!(total(&app, &format!("tem_email={t}")).await, 307);
        assert_eq!(total(&app, &format!("tem_telefone={t}")).await, 307);
    }
    assert_eq!(id_set(&app, "tem_email=true").await, id_set(&app, "contato=email").await);
    assert_eq!(id_set(&app, "tem_telefone=true").await, id_set(&app, "contato=telefone").await);
    // `false` adds no condition: alone it leaves no criteria.
    for k in ["mei_ativo", "simples_ativo", "tem_email", "tem_telefone"] {
        bad_request(&app, &format!("/api/v1/search?{k}=false")).await;
        assert_eq!(total(&app, &format!("uf=SP&{k}=no")).await, 152, "{k}=no");
    }
    assert_eq!(total(&app, "mei_ativo=true&uf=MG").await, 2);
    assert_eq!(total(&app, "mei_ativo=true&simples_ativo=true").await, 0);
}

#[tokio::test]
async fn text_with_filters() {
    let app = app();
    assert_eq!(total(&app, "q=mercado&uf=PR").await, 150);
    assert_eq!(total(&app, "q=mercado&uf=PR&cnpj=40000001").await, 1);
    assert_eq!(total(&app, "q=banco&uf=SP").await, 1);
    assert_eq!(total(&app, "q=banco&uf=RJ&fuzzy=false").await, 0);
    assert_eq!(total(&app, "q=padaria&simples_ativo=true").await, 1);
    assert_eq!(total(&app, "q=mercado&data_inicio_atividade_de=2021").await, 0);
}

// ---------------------------------------------------------------------------------------
// Numeric ranges
// ---------------------------------------------------------------------------------------

/// Per numeric column: (from, to, matches in range, documents without a value).
const RANGE_EXAMPLES: [(&str, &str, &str, u64, u64); 7] = [
    ("data_inicio_atividade", "2020-01-01", "2020-01-01", 307, 3),
    ("data_situacao_cadastral", "20200101", "20200101", 307, 3),
    ("data_opcao_simples", "2020", "2020", 1, 309),
    ("data_exclusao_simples", "1900", "9999", 0, 310),
    ("data_opcao_mei", "2021-01-01", "2021-12-31", 2, 308),
    ("data_exclusao_mei", "1900-01-01", "9999-12-31", 0, 310),
    ("capital_social", "500,00", "1.000,00", 3, 1),
];

#[tokio::test]
async fn every_numeric_column_supports_ranges_and_vazio() {
    let app = app();
    let names: BTreeSet<&str> = opencnpj_store::numeric::NumField::ALL.iter().map(|f| f.name()).collect();
    assert_eq!(names, RANGE_EXAMPLES.iter().map(|e| e.0).collect::<BTreeSet<_>>());
    for (col, lo, hi, n, empty) in RANGE_EXAMPLES {
        let enc = |s: &str| s.replace(',', "%2C");
        let (lo, hi) = (enc(lo), enc(hi));
        assert_eq!(total(&app, &format!("{col}_de={lo}&{col}_ate={hi}")).await, n, "{col} de/ate");
        assert_eq!(total(&app, &format!("{col}_min={lo}&{col}_max={hi}")).await, n, "{col} min/max");
        assert_eq!(total(&app, &format!("{col}_vazio=true")).await, empty, "{col}_vazio");
        // Values and "no value" partition the documents.
        let any = total(&app, &format!("{col}_de={}", if col == "capital_social" { "0" } else { "1900" })).await;
        assert_eq!(any + empty, 310, "{col}");
        bad_request(&app, &format!("/api/v1/search?{col}_de=abc")).await;
        bad_request(&app, &format!("/api/v1/search?{col}_ate=")).await;
        // `_vazio=false` adds nothing.
        bad_request(&app, &format!("/api/v1/search?{col}_vazio=false")).await;
    }
    let (_, _, c) = get_json(&app, "/api/v1/campos").await;
    let iv = c["intervalos"].as_array().unwrap();
    assert_eq!(iv.iter().map(|i| i["campo"].as_str().unwrap()).collect::<BTreeSet<_>>(), names);
    assert!(iv.iter().all(|i| i["disponivel"] == true));
}

#[tokio::test]
async fn date_bounds_formats() {
    let app = app();
    for (qs, n) in [
        ("data_inicio_atividade_de=2020-01-01", 307),
        ("data_inicio_atividade_de=20200101", 307),
        ("data_inicio_atividade_de=2020", 307),
        ("data_inicio_atividade_de=2020-01-02", 0),
        ("data_inicio_atividade_de=2021", 0),
        ("data_inicio_atividade_ate=2019-12-31", 0),
        ("data_inicio_atividade_ate=20200101", 307),
        ("data_inicio_atividade_ate=2020", 307),
        ("data_inicio_atividade_ate=2019", 0),
        // A bare year is Jan 1 as a lower bound and Dec 31 as an upper bound.
        ("data_inicio_atividade_de=2020&data_inicio_atividade_ate=2020", 307),
        // Repeated bounds: the tightest wins.
        ("data_inicio_atividade_de=2019&data_inicio_atividade_de=2021", 0),
        ("data_inicio_atividade_ate=2025&data_inicio_atividade_ate=2019", 0),
        // Empty interval.
        ("data_inicio_atividade_de=2021&data_inicio_atividade_ate=2019", 0),
        ("data_opcao_mei_de=2021&data_opcao_mei_ate=2021", 2),
        ("data_opcao_mei_de=2020&data_opcao_mei_ate=2020", 0),
    ] {
        assert_eq!(total(&app, qs).await, n, "{qs}");
    }
    for bad in ["2020-13-01", "2020-02-30", "2019-02-29", "1899", "1899-12-31", "01-01-2020", "202001", "20200", "abcd", "2020-1-1"] {
        let m = bad_request(&app, &format!("/api/v1/search?data_inicio_atividade_de={bad}")).await;
        assert!(m.starts_with("bad date") || m.starts_with("bad year"), "{bad}: {m}");
    }
    assert_eq!(total(&app, "data_inicio_atividade_de=2020-02-29&data_inicio_atividade_ate=2024-02-29").await, 0, "leap days are valid");
}

#[tokio::test]
async fn capital_bounds_in_reais() {
    let app = app();
    for (qs, n) in [
        ("capital_social_min=1000%2C00", 3), // BB x2 + padaria
        ("capital_social_min=1.000%2C00", 3),
        ("capital_social_min=1000.00", 3),
        ("capital_social_min=1000", 3),
        ("capital_social_de=1.000%2C50", 2),
        ("capital_social_de=1.000%2C01", 2),
        ("capital_social_max=0", 3), // alfanumérica + two MEI companies
        ("capital_social_ate=0%2C00", 3),
        ("capital_social_ate=1%2C5", 4), // + "1,5"
        ("capital_social_ate=1%2C49", 3),
        ("capital_social_ate=10", 304),
        ("capital_social_de=500&capital_social_ate=500", 2),
        ("capital_social_de=120000000000", 2),
        ("capital_social_de=120000000000%2C01", 0),
        ("capital_social_vazio=true", 1), // the orphan establishment
    ] {
        assert_eq!(total(&app, qs).await, n, "{qs}");
    }
    assert_eq!(all_ids(&app, "capital_social_vazio=true").await, ["99999999000191"]);
    for bad in ["abc", "-1", "1x", "12345678901234567"] {
        let m = bad_request(&app, &format!("/api/v1/search?capital_social_min={bad}")).await;
        assert!(m.starts_with("bad amount"), "{bad}: {m}");
    }
}

// ---------------------------------------------------------------------------------------
// Parameter validation
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn parameter_validation_errors() {
    let app = app();
    assert_eq!(bad_request(&app, "/api/v1/search?q=banco&foo=1").await, "unknown parameter foo");
    assert_eq!(bad_request(&app, "/api/v1/search?q=banco&razao_social_de=1").await, "unknown parameter razao_social_de");
    assert_eq!(
        bad_request(&app, "/api/v1/search?q=banco&ano_data_inicio_atividade=2020").await,
        "unknown parameter ano_data_inicio_atividade"
    );
    // Query-only text fields are not filters.
    bad_request(&app, "/api/v1/search?razao_len=3").await;
    for empty in ["uf=", "uf=,", "uf=%20,%20", "cnae=", "municipio=,,"] {
        let m = bad_request(&app, &format!("/api/v1/search?{empty}")).await;
        assert!(m.starts_with("empty value for "), "{empty}: {m}");
    }
    let no_criteria = "provide q (name, CNPJ or CPF) and/or at least one filter; see /api/v1/campos";
    for qs in ["", "q=", "q=%20%20", "cnpj=", "limit=5", "format=csv", "fuzzy=true&field=any", "ordenar=capital_social:desc"] {
        assert_eq!(bad_request(&app, &format!("/api/v1/search?{qs}")).await, no_criteria, "{qs}");
    }
    // Values up to 2000 bytes are accepted; longer ones are rejected.
    let ok = "A".repeat(2000);
    assert_eq!(total(&app, &format!("uf={ok}")).await, 0);
    let long = "A".repeat(2001);
    assert_eq!(bad_request(&app, &format!("/api/v1/search?uf={long}")).await, "parameter uf too long");
    assert_eq!(bad_request(&app, &format!("/api/v1/search?q={long}")).await, "parameter q too long");
    // Parameters of other endpoints are ignored by /search.
    assert_eq!(total(&app, "q=banco&group_by=uf&agrupar=uf&format=csv&max=1&estab_offset=3&estab_limit=1").await, 2);
}

// ---------------------------------------------------------------------------------------
// Server caps and privacy flags
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn max_limit_flag() {
    let app = app_with(&["--max-limit", "50"]);
    assert_eq!(search(&app, "q=mercado&limit=50").await["results"].as_array().unwrap().len(), 50);
    assert_eq!(bad_request(&app, "/api/v1/search?q=mercado&limit=51").await, "limit must be between 1 and 50");
    bad_request(&app, "/api/v1/search?q=mercado&limit=0").await;
    bad_request(&app, "/api/v1/stats?uf=SP&limit=51").await;
    assert_eq!(bad_request(&app, "/api/v1/cnpj/00000000?estab_limit=51").await, "estab_limit must be between 1 and 50");
    assert_eq!(get(&app, "/api/v1/cnpj/00000000?estab_limit=50").await.0, StatusCode::OK);
    // The CPF lookup clamps instead.
    assert_eq!(get(&app, "/api/v1/cpf/12345678901?limit=51").await.0, StatusCode::OK);
    let (_, _, c) = get_json(&app, "/api/v1/campos").await;
    assert_eq!(c["paginacao"]["limit"], 50);
    // Export is not bound by --max-limit.
    let (s, _, body) = get(&app, "/api/v1/export?uf=SP&limit=1").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body.split(|b| *b == b'\n').filter(|l| !l.is_empty()).count(), 152);
}

#[tokio::test]
async fn max_offset_flag() {
    let app = app_with(&["--max-offset", "100"]);
    assert_eq!(search(&app, "q=mercado&offset=100&limit=10").await["results"].as_array().unwrap().len(), 10);
    assert_eq!(bad_request(&app, "/api/v1/search?q=mercado&offset=101").await, "offset above 100");
    bad_request(&app, "/api/v1/search?uf=SP&offset=1000").await;
    // Cursors are the way past the cap.
    let p = search(&app, "uf=SP&limit=100").await;
    let next = search(&app, &format!("uf=SP&limit=100&cursor={}", p["next_cursor"].as_str().unwrap())).await;
    assert_eq!(next["results"].as_array().unwrap().len(), 52);
    // Default: no offset cap.
    assert_eq!(search(&app_with(&[]), "q=mercado&offset=1000000").await["total"], 300);
}

#[tokio::test]
async fn disable_export_flag() {
    let app = app_with(&["--disable-export"]);
    for uri in ["/api/v1/export?uf=SP", "/api/v1/export?uf=SP&format=csv", "/api/v1/export"] {
        let m = expect_error(&app, uri, StatusCode::FORBIDDEN, "export_disabled").await;
        assert_eq!(m, "export is disabled on this server");
    }
    assert_eq!(total(&app, "uf=SP").await, 152, "search still works");
}

#[tokio::test]
async fn export_max_rows_flag() {
    let app = app_with(&["--export-max-rows", "5"]);
    let lines = |b: &[u8]| b.split(|c| *c == b'\n').filter(|l| !l.is_empty()).count();
    for (qs, n) in [("uf=SP", 5), ("uf=SP&max=3", 3), ("uf=SP&max=10", 5), ("uf=SP&max=0", 0), ("uf=DF", 1)] {
        let (s, _, b) = get(&app, &format!("/api/v1/export?{qs}")).await;
        assert_eq!(s, StatusCode::OK, "{qs}");
        assert_eq!(lines(&b), n, "{qs}");
    }
    let (_, _, b) = get(&app, "/api/v1/export?uf=SP&format=csv").await;
    assert_eq!(lines(&b), 6, "header + 5 rows");
}

#[tokio::test]
async fn hide_socios_flag() {
    let app = app_with(&["--hide-socios"]);
    let (_, _, v) = get_json(&app, "/api/v1/cnpj/11111111000191").await;
    assert_eq!(v["socios"], Value::Null);
    assert!(v["empresa"].is_object() && v["estabelecimento"].is_object() && v["simples"].is_object());
    let (_, _, v) = get_json(&app, "/api/v1/cnpj/22222222").await;
    assert_eq!(v["socios"], Value::Null);
    assert_eq!(v["estabelecimentos"].as_array().unwrap().len(), 2);
    // Default: partners are present.
    let (_, _, v) = get_json(&app_with(&[]), "/api/v1/cnpj/22222222").await;
    assert_eq!(v["socios"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn hide_socios_applies_to_cpf_lookup() {
    let app = app_with(&["--hide-socios"]);
    let (_, _, v) = get_json(&app, "/api/v1/cpf/00012345600").await;
    assert!(!v.to_string().contains("JOÃO DA SILVA"), "{v}");
}

#[tokio::test]
async fn hide_contacts_flag() {
    let app = app_with(&["--hide-contacts"]);
    let (_, _, v) = get_json(&app, "/api/v1/cnpj/00000000000191").await;
    let k = keys(&v["estabelecimento"]);
    for c in CONTACT_COLUMNS {
        assert!(!k.contains(c), "{c} must be hidden");
    }
    let visible: Vec<&str> = ESTAB_COLUMNS.iter().copied().filter(|c| !CONTACT_COLUMNS.contains(c)).collect();
    assert_eq!(
        k,
        with(
            &visible,
            &[
                "identificador_matriz_filial_descricao",
                "situacao_cadastral_descricao",
                "motivo_situacao_cadastral_descricao",
                "cnae_fiscal_principal_descricao"
            ]
        )
    );
    let (_, _, v) = get_json(&app, "/api/v1/cnpj/00000000").await;
    for e in v["estabelecimentos"].as_array().unwrap() {
        assert!(e.get("correio_eletronico").is_none() && e.get("telefone_1").is_none() && e.get("ddd_1").is_none());
    }
    // Partners stay (contacts only), and filtering by contact presence still works.
    assert_eq!(v["socios"], serde_json::json!([]));
    assert_eq!(total(&app, "tem_email=true").await, 307);
}

#[tokio::test]
async fn hide_contacts_applies_to_export() {
    let app = app_with(&["--hide-contacts"]);
    let (_, _, nd) = get(&app, "/api/v1/export?uf=DF").await;
    let nd = String::from_utf8(nd.to_vec()).unwrap();
    assert!(!nd.contains("contato@example.com") && !nd.contains("33334444"), "{nd}");
    let (_, _, csv) = get(&app, "/api/v1/export?uf=DF&format=csv").await;
    let csv = String::from_utf8(csv.to_vec()).unwrap();
    assert!(!csv.contains("contato@example.com") && !csv.contains("correio_eletronico"), "{csv}");
}

// ---------------------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------------------

async fn groups(app: &Router, qs: &str) -> Vec<(String, Value, u64)> {
    let (s, h, v) = get_json(app, &format!("/api/v1/stats?{qs}")).await;
    assert_eq!(s, StatusCode::OK, "stats?{qs}: {v}");
    assert_eq!(h["cache-control"], CACHE);
    assert_eq!(keys(&v), set(&["group_by", "groups", "snapshot"]));
    assert_eq!(v["snapshot"], snapshot_ref());
    v["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| {
            assert_eq!(keys(g), set(&["value", "label", "count"]), "{g}");
            (g["value"].as_str().unwrap().to_string(), g["label"].clone(), g["count"].as_u64().unwrap())
        })
        .collect()
}

fn g(value: &str, label: Option<&str>, count: u64) -> (String, Value, u64) {
    (value.to_string(), label.map_or(Value::Null, |l| Value::String(l.into())), count)
}

#[tokio::test]
async fn stats_group_by_every_groupable_field() {
    let app = app();
    let (_, _, v) = get_json(&app, "/api/v1/stats").await;
    assert_eq!(v["group_by"], "uf", "default group");
    let uf = vec![g("SP", None, 152), g("PR", None, 150), g("MG", None, 2), g("BA", None, 1), g("DF", None, 1), g("RJ", None, 1)];
    assert_eq!(groups(&app, "").await, uf);
    assert_eq!(groups(&app, "group_by=uf").await, uf);
    assert_eq!(groups(&app, "agrupar=uf").await, uf);
    // Low-cardinality fields: no criteria needed.
    for (field, expected) in [
        ("situacao", vec![g("02", Some("ATIVA"), 306), g("08", Some("BAIXADA"), 1)]),
        ("porte", vec![g("01", Some("MICRO EMPRESA"), 305), g("05", Some("DEMAIS"), 3), g("03", Some("EMPRESA DE PEQUENO PORTE"), 1)]),
        ("matriz_filial", vec![g("1", Some("MATRIZ"), 304), g("2", Some("FILIAL"), 3)]),
        ("motivo", vec![g("00", Some("SEM MOTIVO"), 307)]),
        ("simples", vec![g("N", None, 2), g("S", None, 1)]),
        ("mei", vec![g("S", None, 2), g("N", None, 1)]),
        ("cep", vec![g("01310100", None, 307)]),
        ("ddd", vec![g("11", None, 307)]),
        ("contato", vec![g("email", None, 307), g("telefone", None, 307)]),
        ("cpf_titular", vec![g("12345678901", None, 1)]),
        ("socio_documento", vec![g("33000167000101", None, 2), g("*123456", None, 1)]),
    ] {
        assert_eq!(groups(&app, &format!("group_by={field}")).await, expected, "group_by={field}");
        let (_, _, v) = get_json(&app, &format!("/api/v1/stats?group_by={field}")).await;
        assert_eq!(v["group_by"], field);
    }
    // High-cardinality fields need q or a filter.
    for field in ["municipio", "cnae", "cnae_secundaria", "natureza"] {
        let m = bad_request(&app, &format!("/api/v1/stats?group_by={field}")).await;
        assert_eq!(m, format!("group_by={field} requires q or a filter"));
    }
    assert_eq!(groups(&app, "group_by=municipio&uf=SP").await, vec![g("7107", Some("SAO PAULO"), 152)]);
    assert_eq!(
        groups(&app, "group_by=municipio&situacao=02").await,
        vec![g("7107", Some("SAO PAULO"), 302), g("3849", None, 1), g("6001", Some("RIO DE JANEIRO"), 1), g("9701", None, 1)]
    );
    assert_eq!(
        groups(&app, "group_by=cnae&uf=SP").await,
        vec![g("4712100", None, 150), g("1091102", None, 1), g("6422100", Some("Bancos múltiplos, com carteira comercial"), 1)]
    );
    assert_eq!(groups(&app, "group_by=cnae_secundaria&uf=SP").await, vec![g("4711302", None, 152), g("4712100", None, 152)]);
    assert_eq!(
        groups(&app, "group_by=natureza&situacao=02").await,
        vec![g("2062", Some("Sociedade Empresária Limitada"), 303), g("2038", None, 2)]
    );
    assert_eq!(groups(&app, "group_by=natureza&natureza=21").await, vec![g("2135", None, 2)]);
    // The groups of every filter field (except query-only ones) are accepted.
    for f in opencnpj_store::postings::Field::FILTERS {
        let n = f.name();
        let (s, _, _) = get(&app, &format!("/api/v1/stats?group_by={n}&uf=SP")).await;
        let query_only = n == "cpf" || n == "cnae_qualquer";
        assert_eq!(s, if query_only { StatusCode::BAD_REQUEST } else { StatusCode::OK }, "group_by={n}");
    }
}

#[tokio::test]
async fn stats_with_filters_and_text() {
    let app = app();
    assert_eq!(groups(&app, "q=mercado").await, vec![g("PR", None, 150), g("SP", None, 150)]);
    assert_eq!(groups(&app, "group_by=situacao&uf=MG").await, vec![g("02", Some("ATIVA"), 1), g("08", Some("BAIXADA"), 1)]);
    assert_eq!(groups(&app, "group_by=uf&cnae=K").await, vec![g("DF", None, 1), g("SP", None, 1)]);
    assert_eq!(groups(&app, "group_by=uf&uf=AM").await, vec![]);
    assert_eq!(groups(&app, "group_by=porte&natureza=2135").await, vec![g("01", Some("MICRO EMPRESA"), 2)]);
    assert_eq!(groups(&app, "group_by=municipio&q=mercado").await, vec![g("7107", Some("SAO PAULO"), 300)]);
    // Group counts add up to the search total when each document has exactly one value.
    for qs in ["uf=SP", "q=mercado", "cnae=G", "situacao=02"] {
        let t = total(&app, qs).await;
        let sum: u64 = groups(&app, &format!("group_by=situacao&{qs}")).await.iter().map(|x| x.2).sum();
        assert_eq!(sum, t, "{qs}");
    }
    // Paging parameters are validated but do not change the counts.
    assert_eq!(groups(&app, "group_by=uf&q=mercado&limit=1&offset=5").await, vec![g("PR", None, 150), g("SP", None, 150)]);
}

#[tokio::test]
async fn stats_yearly_histograms() {
    let app = app();
    let years = |qs: &'static str| {
        let app = app.clone();
        async move {
            let (s, _, v) = get_json(&app, &format!("/api/v1/stats?{qs}")).await;
            assert_eq!(s, StatusCode::OK, "{qs}: {v}");
            assert_eq!(keys(&v), set(&["group_by", "groups", "snapshot"]));
            v["groups"]
                .as_array()
                .unwrap()
                .iter()
                .map(|g| {
                    assert_eq!(keys(g), set(&["value", "count"]));
                    (g["value"].as_str().unwrap().to_string(), g["count"].as_u64().unwrap())
                })
                .collect::<Vec<_>>()
        }
    };
    let y = |v: &str, n: u64| (v.to_string(), n);
    assert_eq!(years("group_by=ano_data_inicio_atividade").await, [y("2020", 307)]);
    assert_eq!(years("group_by=ano_data_situacao_cadastral").await, [y("2020", 307)]);
    assert_eq!(years("group_by=ano_data_opcao_simples").await, [y("2020", 1)]);
    assert_eq!(years("group_by=ano_data_exclusao_simples").await, []);
    assert_eq!(years("group_by=ano_data_opcao_mei").await, [y("2021", 2)]);
    assert_eq!(years("group_by=ano_data_exclusao_mei").await, []);
    assert_eq!(years("group_by=ano_data_inicio_atividade&uf=SP").await, [y("2020", 152)]);
    assert_eq!(years("agrupar=ano_data_inicio_atividade&q=banco").await, [y("2020", 2)]);
    assert_eq!(years("group_by=ano_data_inicio_atividade&uf=AM").await, []);
    let (_, _, v) = get_json(&app, "/api/v1/stats?group_by=ano_data_opcao_mei").await;
    assert_eq!(v["group_by"], "ano_data_opcao_mei");
    for bad in
        ["ano_capital_social", "ano_bogus", "ano_", "bogus", "cpf", "cnae_qualquer", "razao_social", "nome_fantasia", "razao_len", ""]
    {
        let m = bad_request(&app, &format!("/api/v1/stats?group_by={bad}")).await;
        assert_eq!(m, "bad group_by; use a filter field or ano_<date column>", "{bad}");
    }
    bad_request(&app, "/api/v1/stats?group_by=uf&foo=1").await;
    bad_request(&app, "/api/v1/stats?group_by=uf&cnae=Z").await;
    bad_request(&app, "/api/v1/stats?group_by=ano_data_inicio_atividade&data_inicio_atividade_de=2020-13-01").await;
}

// ---------------------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------------------

fn ndjson(b: &[u8]) -> Vec<Value> {
    std::str::from_utf8(b).unwrap().lines().map(|l| serde_json::from_str(l).expect("ndjson line")).collect()
}

#[tokio::test]
async fn export_ndjson() {
    let app = app();
    let r = send(&app, get_request("/api/v1/export?uf=DF")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.header("content-type"), Some("application/x-ndjson"));
    assert_eq!(r.header("cache-control"), None);
    let rows = ndjson(&r.body);
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(
        keys(row),
        with(
            &ESTAB_COLUMNS,
            &[
                "cnpj",
                "razao_social",
                "identificador_matriz_filial_descricao",
                "situacao_cadastral_descricao",
                "motivo_situacao_cadastral_descricao",
                "cnae_fiscal_principal_descricao"
            ]
        )
    );
    assert_eq!(row["cnpj"], "00000000000191");
    assert_eq!(row["razao_social"], "BANCO DO BRASIL SA");
    assert_eq!(row["nome_fantasia"], "DIRECAO GERAL");
    // Same as the explicit format, and same rows as the lookup.
    let (_, _, b) = get(&app, "/api/v1/export?uf=DF&format=ndjson").await;
    assert_eq!(b, r.body);
    let (_, _, l) = get_json(&app, "/api/v1/cnpj/00000000000191").await;
    let mut lookup = l["estabelecimento"].clone();
    lookup["cnpj"] = "00000000000191".into();
    lookup["razao_social"] = "BANCO DO BRASIL SA".into();
    assert_eq!(&lookup, row);
}

#[tokio::test]
async fn export_csv() {
    let app = app();
    let r = send(&app, get_request("/api/v1/export?cnpj=22222222&format=csv")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.header("content-type"), Some("text/csv; charset=utf-8"));
    let text = r.text();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "header + 2 establishments");
    let header: Vec<&str> = lines[0].split(';').collect();
    let mut expected = vec!["cnpj", "razao_social"];
    expected.extend(ESTAB_COLUMNS);
    assert_eq!(header, expected);
    // Every field quoted, quotes doubled, RFB values verbatim.
    assert!(lines[1].starts_with(r#""22222222000191";"COMERCIO DE ""PEÇAS"" AUTO LTDA";"0001";"91";"1";"";"08";"#), "{}", lines[1]);
    assert!(lines[2].starts_with(r#""22222222000272";"COMERCIO DE ""PEÇAS"" AUTO LTDA";"0002";"72";"2";"#), "{}", lines[2]);
    assert_eq!(lines[1].matches("\";\"").count(), expected.len() - 1);
}

#[tokio::test]
async fn export_rows_match_search_totals() {
    let app = app();
    for qs in
        ["uf=SP", "cnae=G", "q=mercado", "situacao=02", "cnpj=00000000", "q=banco&uf=SP", "mei_ativo=true", "data_inicio_atividade_de=2020"]
    {
        let ids = id_set(&app, qs).await;
        let (s, _, b) = get(&app, &format!("/api/v1/export?{qs}")).await;
        assert_eq!(s, StatusCode::OK, "{qs}");
        let rows = ndjson(&b);
        assert_eq!(rows.len(), ids.len(), "{qs}");
        let exported: BTreeSet<String> = rows.iter().map(|r| r["cnpj"].as_str().unwrap().to_string()).collect();
        assert_eq!(exported, ids, "{qs}");
        // CNPJ order.
        let order: Vec<&str> = rows.iter().map(|r| r["cnpj"].as_str().unwrap()).collect();
        let mut sorted = order.clone();
        sorted.sort();
        assert_eq!(order, sorted, "{qs}");
        let (_, _, c) = get(&app, &format!("/api/v1/export?{qs}&format=csv")).await;
        assert_eq!(std::str::from_utf8(&c).unwrap().lines().count(), ids.len() + 1, "{qs} csv");
    }
    // `max` truncates; `limit` is ignored by export.
    for (qs, n) in [("uf=SP&max=7", 7), ("uf=SP&max=0", 0), ("uf=SP&max=1000", 152), ("uf=SP&limit=1", 152)] {
        let (_, _, b) = get(&app, &format!("/api/v1/export?{qs}")).await;
        assert_eq!(ndjson(&b).len(), n, "{qs}");
    }
    let (_, _, b) = get(&app, "/api/v1/export?uf=SP&max=7&format=csv").await;
    assert_eq!(std::str::from_utf8(&b).unwrap().lines().count(), 8);
    // No matches: empty ndjson, header-only CSV.
    let (s, _, b) = get(&app, "/api/v1/export?uf=AM").await;
    assert_eq!((s, b.len()), (StatusCode::OK, 0));
    let (_, _, b) = get(&app, "/api/v1/export?uf=AM&format=csv").await;
    assert_eq!(std::str::from_utf8(&b).unwrap().lines().count(), 1);
}

#[tokio::test]
async fn export_parameter_errors() {
    let app = app();
    assert_eq!(bad_request(&app, "/api/v1/export?uf=SP&format=xml").await, "format must be ndjson or csv");
    assert_eq!(bad_request(&app, "/api/v1/export?uf=SP&max=abc").await, "bad max");
    bad_request(&app, "/api/v1/export?uf=SP&max=-1").await;
    bad_request(&app, "/api/v1/export").await;
    bad_request(&app, "/api/v1/export?format=csv").await;
    bad_request(&app, "/api/v1/export?uf=SP&foo=1").await;
    bad_request(&app, "/api/v1/export?data_inicio_atividade_de=2020-13-01").await;
}

#[tokio::test]
async fn export_rejects_invalid_filter_values_before_streaming() {
    let app = app();
    for uri in ["/api/v1/export?cnae=Z", "/api/v1/export?cnpj=123456789012345", "/api/v1/export?cpf=123"] {
        let (s, _, body) = try_send(&app, get_request(uri)).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{uri}: {body:?}");
    }
}

/// Export rows are establishment rows: a company without establishments matches a search
/// (as a company-only hit) but has no row to export (documented in docs/API.md).
#[tokio::test]
async fn export_rows_are_establishments() {
    let app = app();
    assert_eq!(total(&app, "natureza=2135").await, 2);
    let (s, _, b) = get(&app, "/api/v1/export?natureza=2135").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(ndjson(&b).len(), 0);
}

// ---------------------------------------------------------------------------------------
// Metadata routes
// ---------------------------------------------------------------------------------------

fn file_sha256(p: &std::path::Path) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(std::fs::read(p).unwrap()))
}

#[tokio::test]
async fn snapshot_info() {
    let app = app();
    let (s, h, v) = get_json(&app, "/api/v1/snapshot").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(h.get("cache-control"), None, "changes on reload");
    assert_eq!(
        keys(&v),
        set(&[
            "file_bytes",
            "format_version",
            "competence",
            "source_snapshot",
            "profile",
            "block_codec",
            "block_target_bytes",
            "zstd_dictionary_bytes",
            "counts",
            "sections",
            "blocks",
            "terms",
            "sources",
            "sha256",
            "trust"
        ]),
        "no server path is exposed"
    );
    assert_eq!(v["competence"], "2026-10");
    assert_eq!(v["source_snapshot"], "D61010");
    assert_eq!(v["profile"], "balanced");
    assert_eq!(v["format_version"], serde_json::json!([1, 2]));
    assert_eq!(v["file_bytes"].as_u64(), Some(std::fs::metadata(snapshot_path()).unwrap().len()));
    assert_eq!(v["sha256"].as_str(), Some(file_sha256(snapshot_path()).as_str()));
    let c = &v["counts"];
    assert_eq!(c["documents"], 310);
    assert_eq!(c["companies"], 308);
    assert_eq!(c["companies_without_empresa"], 1);
    assert_eq!(c["rejected_lines"], 1);
    assert_eq!(c["rows"], serde_json::json!({ "empresas": 307, "estabelecimentos": 307, "simples": 2, "socios": 2 }));
    let sources: Vec<&str> = v["sources"].as_array().unwrap().iter().map(|s| s["path"].as_str().unwrap()).collect();
    for z in ["Empresas0.zip", "Estabelecimentos0.zip", "Socios0.zip", "Simples.zip", "Cnaes.zip", "Municipios.zip"] {
        assert!(sources.contains(&z), "{z} in {sources:?}");
    }
    assert!(v["sources"].as_array().unwrap().iter().all(|s| s["sha256"].as_str().is_some_and(|h| h.len() == 64)));
    for sec in ["data", "postings", "terms", "manifest", "numeric", "docvalues", "rejects"] {
        assert!(v["sections"][sec].as_u64().is_some_and(|n| n > 0), "section {sec}");
    }
}

#[tokio::test]
async fn distribution_lists_files() {
    let app = app();
    let (s, h, v) = get_json(&app, "/api/v1/distribution").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(h.get("cache-control"), None);
    assert_eq!(keys(&v), set(&["files", "competence", "sources", "how_to_verify"]));
    assert_eq!(v["competence"], "2026-10");
    let files = v["files"].as_array().unwrap();
    assert_eq!(files.len(), 2, "{files:?}");
    let snap = &files[0];
    assert_eq!(keys(snap), set(&["name", "bytes", "sha256", "url", "active"]));
    assert_eq!(snap["name"], "balanced.ocnpj");
    assert_eq!(snap["url"], "/downloads/balanced.ocnpj");
    assert_eq!(snap["active"], true);
    assert_eq!(snap["bytes"].as_u64(), Some(std::fs::metadata(snapshot_path()).unwrap().len()));
    assert_eq!(snap["sha256"].as_str(), Some(file_sha256(snapshot_path()).as_str()));
    let side = &files[1];
    assert_eq!(side["name"], "balanced.ocnpj.sha256");
    assert_eq!(side["active"], false);
    assert_eq!(side["sha256"], Value::Null);
    let (_, _, info) = get_json(&app, "/api/v1/snapshot").await;
    assert_eq!(v["sources"], info["sources"]);
}

#[tokio::test]
async fn downloads_full_and_ranges() {
    let app = app();
    let file = std::fs::read(snapshot_path()).unwrap();
    let len = file.len();
    let url = "/downloads/balanced.ocnpj";
    let r = send(&app, get_request(url)).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.header("content-type"), Some("application/octet-stream"));
    assert_eq!(r.header("accept-ranges"), Some("bytes"));
    assert_eq!(r.header("content-length"), Some(len.to_string().as_str()));
    assert_eq!(r.header("content-disposition"), Some("attachment; filename=\"balanced.ocnpj\""));
    assert_eq!(r.header("content-range"), None);
    assert_eq!(&r.body[..], &file[..]);

    for (spec, a, b) in [
        ("bytes=0-9", 0, 9),
        ("bytes=10-", 10, len - 1),
        ("bytes=-16", len - 16, len - 1),
        ("bytes=100-199", 100, 199),
        ("bytes=0-99999999", 0, len - 1),
        ("bytes=-99999999", 0, len - 1),
    ] {
        let r = get_with_header(&app, url, "range", spec).await;
        assert_eq!(r.status, StatusCode::PARTIAL_CONTENT, "{spec}");
        assert_eq!(r.header("content-range"), Some(format!("bytes {a}-{b}/{len}").as_str()), "{spec}");
        assert_eq!(r.header("content-length"), Some((b - a + 1).to_string().as_str()), "{spec}");
        assert_eq!(&r.body[..], &file[a..=b], "{spec}");
    }
    // Resuming at the last byte.
    let last = get_with_header(&app, url, "range", &format!("bytes={}-", len - 1)).await;
    assert_eq!(last.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(&last.body[..], &file[len - 1..]);
    // Unsatisfiable.
    for spec in [format!("bytes={len}-"), format!("bytes={}-", len + 10), "bytes=5-2".to_string(), "bytes=-0".to_string()] {
        let r = get_with_header(&app, url, "range", &spec).await;
        assert_eq!(r.status, StatusCode::RANGE_NOT_SATISFIABLE, "{spec}");
        let v = r.json();
        assert_eq!(v["error"], "bad_range");
        assert_eq!(v["code"], 416);
        assert_eq!(v["message"], format!("file has {len} bytes"));
    }
    // Malformed ranges are 400; other units are ignored (full body).
    for spec in ["bytes=abc", "bytes=-", "bytes=a-b"] {
        let r = get_with_header(&app, url, "range", spec).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{spec}");
        assert_eq!(r.json()["error"], "invalid_request");
    }
    let r = get_with_header(&app, url, "range", "items=0-5").await;
    assert_eq!((r.status, r.body.len()), (StatusCode::OK, len));

    // The checksum sidecar is downloadable and matches the file.
    let r = send(&app, get_request("/downloads/balanced.ocnpj.sha256")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.text(), format!("{}  balanced.ocnpj\n", file_sha256(snapshot_path())));
    // Only distributable files.
    for name in ["nope.ocnpj", "Empresas0.zip", "2026-10", "..%2Fbalanced.ocnpj", "balanced.ocnpj.bak"] {
        let m = expect_error(&app, &format!("/downloads/{name}"), StatusCode::NOT_FOUND, "not_found").await;
        assert_eq!(m, "no such file");
    }
}

#[tokio::test]
async fn reference_tables() {
    let app = app();
    let (s, _, v) = get_json(&app, "/api/v1/dominios").await;
    assert_eq!(s, StatusCode::OK);
    let tables: Vec<(String, u64, String)> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            assert_eq!(keys(t), set(&["tabela", "linhas", "url"]));
            (t["tabela"].as_str().unwrap().into(), t["linhas"].as_u64().unwrap(), t["url"].as_str().unwrap().into())
        })
        .collect();
    let expected: Vec<(String, u64, String)> =
        [("cnaes", 1), ("motivos", 1), ("municipios", 2), ("naturezas", 1), ("paises", 1), ("qualificacoes", 1)]
            .iter()
            .map(|(t, n)| (t.to_string(), *n, format!("/api/v1/dominios/{t}")))
            .collect();
    assert_eq!(tables, expected);
    for (t, n, url) in &tables {
        let (s, h, rows) = get_json(&app, url).await;
        assert_eq!(s, StatusCode::OK, "{t}");
        assert_eq!(h["cache-control"], CACHE);
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len() as u64, *n, "{t}");
        assert!(rows.iter().all(|r| keys(r) == set(&["codigo", "descricao"])));
    }
    let (_, _, m) = get_json(&app, "/api/v1/dominios/municipios").await;
    assert_eq!(m, serde_json::json!([{ "codigo": "7107", "descricao": "SAO PAULO" }, { "codigo": "6001", "descricao": "RIO DE JANEIRO" }]));
    let (_, _, c) = get_json(&app, "/api/v1/dominios/cnaes").await;
    assert_eq!(c, serde_json::json!([{ "codigo": "6422100", "descricao": "Bancos múltiplos, com carteira comercial" }]));
    let m = expect_error(&app, "/api/v1/dominios/nope", StatusCode::NOT_FOUND, "not_found").await;
    assert!(m.starts_with("unknown table; available:") && m.contains("municipios"), "{m}");
}

#[tokio::test]
async fn campos_self_description() {
    let app = app();
    let (s, _, v) = get_json(&app, "/api/v1/campos").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(keys(&v), set(&["texto", "filtros", "filtros_notas", "intervalos", "ordenar", "paginacao", "agrupar"]));
    assert_eq!(v["texto"]["field"], serde_json::json!(["any", "razao_social", "nome_fantasia"]));
    let ordenar: Vec<&str> = v["ordenar"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
    assert_eq!(ordenar.len(), 14);
    for o in &ordenar {
        // Every advertised sort option works.
        search(&app, &format!("uf=SP&limit=1&ordenar={o}")).await;
    }
    for i in v["intervalos"].as_array().unwrap() {
        let params: Vec<&str> = i["parametros"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
        assert_eq!(params.len(), 3);
        // Every advertised range parameter parses.
        let value = if i["campo"] == "capital_social" { "1" } else { "2020" };
        search(&app, &format!("{}={value}&{}={value}&{}=true", params[0], params[1], params[2])).await;
    }
    assert_eq!(v["paginacao"]["limit"], 10000);
}

// ---------------------------------------------------------------------------------------
// Consistency properties
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn total_equals_rows_reached_by_paging() {
    let app = app();
    for qs in [
        "q=mercado",
        "q=mercad",
        "uf=SP",
        "cnae=G",
        "situacao=02",
        "data_exclusao_simples_vazio=true",
        "natureza=20&ordenar=capital_social:desc",
        "q=banco",
        "cpf=00012345600",
    ] {
        let t = total(&app, qs).await as usize;
        let mut seen: Vec<String> = Vec::new();
        let mut offset = 0;
        loop {
            let v = search(&app, &format!("{qs}&limit=37&offset={offset}")).await;
            let page = v["results"].as_array().unwrap();
            assert_eq!(v["total"].as_u64(), Some(t as u64), "{qs} offset={offset}");
            if page.is_empty() {
                break;
            }
            assert!(page.len() <= 37);
            seen.extend(page.iter().map(hit_id));
            offset += 37;
        }
        assert_eq!(seen.len(), t, "{qs}");
        assert_eq!(seen.iter().collect::<BTreeSet<_>>().len(), t, "{qs}: no duplicates");
        assert_eq!(seen, all_ids(&app, qs).await, "{qs}: pages concatenate to the full list");
    }
    // Cursor paging over every document.
    let mut seen = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let qs = match &cursor {
            Some(c) => format!("data_exclusao_mei_vazio=true&limit=50&cursor={c}"),
            None => "data_exclusao_mei_vazio=true&limit=50".to_string(),
        };
        let v = search(&app, &qs).await;
        seen.extend(v["results"].as_array().unwrap().iter().map(hit_id));
        match v["next_cursor"].as_str() {
            Some(c) => cursor = Some(c.to_string()),
            None => break,
        }
    }
    assert_eq!(seen.len(), 310);
    assert_eq!(seen.iter().collect::<BTreeSet<_>>().len(), 310);
}

#[tokio::test]
async fn filtered_results_are_subsets() {
    let app = app();
    for (base, extra) in [
        ("uf=SP", "cnae=47"),
        ("uf=SP", "situacao=08"),
        ("q=mercado", "uf=PR"),
        ("q=mercado", "cnpj=400001"),
        ("q=banco", "matriz_filial=2"),
        ("cnae=G", "situacao=08"),
        ("natureza=20", "porte=05"),
        ("situacao=02", "capital_social_min=500"),
        ("situacao=02", "tem_email=true"),
        ("cnae_qualquer=4712100", "cnae=47"),
        ("data_inicio_atividade_de=2020", "mei_ativo=true"),
    ] {
        let all = id_set(&app, base).await;
        let narrowed = id_set(&app, &format!("{base}&{extra}")).await;
        let other = id_set(&app, extra).await;
        assert!(narrowed.is_subset(&all), "{base} & {extra}");
        assert_eq!(narrowed, all.intersection(&other).cloned().collect(), "{base} & {extra} is the intersection");
    }
}

#[tokio::test]
async fn every_search_hit_resolves_by_lookup() {
    let app = app();
    let v = search(&app, "data_exclusao_simples_vazio=true&limit=10000").await;
    let hits = v["results"].as_array().unwrap();
    assert_eq!(hits.len(), 310);
    for h in hits {
        let razao = h["razao_social"].as_str().unwrap();
        match h.get("cnpj").and_then(Value::as_str) {
            Some(cnpj) => {
                let (s, _, l) = get_json(&app, &format!("/api/v1/cnpj/{cnpj}")).await;
                assert_eq!(s, StatusCode::OK, "{cnpj}");
                assert_eq!(l["cnpj"], cnpj);
                assert_eq!(l["cnpj_basico"], h["cnpj_basico"]);
                assert_eq!(l["estabelecimento"]["nome_fantasia"], h["nome_fantasia"]);
                assert_eq!(l["estabelecimento"]["uf"], h["uf"]);
                assert_eq!(l["estabelecimento"]["situacao_cadastral"], h["situacao_cadastral"]);
                assert_eq!(l["estabelecimento"]["cnae_fiscal_principal"], h["cnae_fiscal_principal"]);
                let lr = l["empresa"].get("razao_social").and_then(Value::as_str).unwrap_or("");
                assert_eq!(lr, razao, "{cnpj}");
                // The formatted CNPJ resolves too.
                let f = l["cnpj_formatado"].as_str().unwrap().replace('/', "%2F");
                assert_eq!(get(&app, &format!("/api/v1/cnpj/{f}")).await.0, StatusCode::OK, "{f}");
            }
            None => {
                let b = h["cnpj_basico"].as_str().unwrap();
                let (s, _, l) = get_json(&app, &format!("/api/v1/empresas/{b}")).await;
                assert_eq!(s, StatusCode::OK, "{b}");
                assert_eq!(l["total_estabelecimentos"], 0);
                assert_eq!(l["empresa"]["razao_social"], razao);
            }
        }
    }
}

#[tokio::test]
async fn search_is_deterministic_and_cacheable() {
    let app = app();
    for qs in ["q=mercado&limit=50", "uf=SP&limit=50", "cnae=G&ordenar=capital_social:desc&limit=50", "q=pdaaria%20"] {
        let a = search(&app, qs).await;
        let b = search(&app, qs).await;
        assert_eq!(a["results"], b["results"], "{qs}");
        assert_eq!(a["total"], b["total"], "{qs}");
        assert_eq!(a["fuzzy_used"], b["fuzzy_used"], "{qs}");
        assert_eq!(a["next_cursor"], b["next_cursor"], "{qs}");
    }
}
