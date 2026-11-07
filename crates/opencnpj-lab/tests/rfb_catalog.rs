use std::{collections::BTreeMap, env, fs, path::Path, process::Command};

use axum::{Router, body::Body, http::Request};
use opencnpj::api::{self, ServeArgs};
use serde::{Deserialize, de::DeserializeOwned};
use tower::ServiceExt;

type Counts = BTreeMap<String, u64>;

struct Usage {
    count: u64,
    by_uf: Counts,
}

#[tokio::test]
async fn fixture_rfb_catalog_matches_stats() {
    let temp = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("fixture tempdir");
    let (snapshot, _) = opencnpj_ingest::testkit::build_fixture(temp.path(), "balanced");
    let out = temp.path().join("rfb");
    let cli = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_opencnpj-intel"));
        command.args(["catalog", "rfb", "--snapshot"]).arg(&snapshot).arg("--out").arg(&out);
        command.output().expect("launch catalog rfb")
    };
    let result = cli();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    assert_eq!(read(&out, "rfb_municipios.tsv"), "code\tlabel\n6001\tRIO DE JANEIRO\n7107\tSAO PAULO\n");
    assert_eq!(read(&out, "rfb_cnaes.tsv"), "code\tlabel\n6422100\tBancos múltiplos, com carteira comercial\n");
    assert_eq!(read(&out, "rfb_naturezas.tsv"), "code\tlabel\n2062\tSociedade Empresária Limitada\n");
    let usage = read_usage(&out);
    assert_eq!(usage["municipio"]["7107"].by_uf, BTreeMap::from([("PR".into(), 150), ("SP".into(), 152)]));
    assert!(usage["municipio"].contains_key("9701"), "retain used codes that have no reference label");
    assert!(usage["cnae_secundaria"].len() > 1, "fixture must exercise multivalued counts");
    assert_stats(&snapshot, &usage).await;

    let saved = read(&out, "used_codes.tsv");
    assert!(!cli().status.success(), "preserve a published destination on retry");
    assert_eq!(read(&out, "used_codes.tsv"), saved);
}

#[tokio::test]
#[ignore = "requires the national snapshot and its local RFB extraction"]
async fn national_rfb_catalog_matches_stats() {
    let snapshot = env::var_os("OPENCNPJ_RFB_SNAPSHOT").expect("set OPENCNPJ_RFB_SNAPSHOT");
    let out = env::var_os("OPENCNPJ_RFB_OUT").expect("set OPENCNPJ_RFB_OUT");
    let out = Path::new(&out);
    for (table, expected) in [("municipios", Some(5572)), ("cnaes", Some(1359)), ("naturezas", None)] {
        let tsv = read(out, &format!("rfb_{table}.tsv"));
        let mut lines = tsv.lines();
        assert_eq!(lines.next(), Some("code\tlabel"));
        let count = lines.count();
        println!("reference table {table}: {count} rows");
        if let Some(expected) = expected {
            assert_eq!(count, expected, "reference table {table}");
        }
    }
    assert_stats(Path::new(&snapshot), &read_usage(out)).await;
}

fn read(dir: &Path, name: &str) -> String {
    fs::read_to_string(dir.join(name)).unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn read_usage(dir: &Path) -> BTreeMap<String, BTreeMap<String, Usage>> {
    let data = read(dir, "used_codes.tsv");
    let mut lines = data.lines();
    assert_eq!(lines.next(), Some("code\tfield\tdocument_count\tuf_distribution"));
    let mut fields = BTreeMap::<String, BTreeMap<String, Usage>>::new();
    for line in lines {
        let parts: Vec<_> = line.split('\t').collect();
        let [code, field, count, distribution] = parts[..] else { panic!("invalid usage row {line:?}") };
        let count = count.parse().expect("integer document count");
        let by_uf: Counts = serde_json::from_str(distribution).expect("UF distribution JSON");
        assert_eq!(by_uf.values().sum::<u64>(), count, "{field} {code} UF distribution total");
        assert!(fields.entry(field.into()).or_default().insert(code.into(), Usage { count, by_uf }).is_none(), "duplicate {field} {code}");
    }
    assert_eq!(fields.keys().map(String::as_str).collect::<Vec<_>>(), ["cnae", "cnae_secundaria", "municipio"]);
    fields
}

#[derive(Deserialize)]
struct Stats {
    group_by: String,
    groups: Vec<Group>,
}

#[derive(Deserialize)]
struct Group {
    value: String,
    count: u64,
}

async fn stats(app: &Router, field: &str, filter: &str) -> Counts {
    let uri = format!("/api/v1/stats?group_by={field}{filter}");
    let response: Stats = get(app, &uri).await;
    assert_eq!(response.group_by, field, "{uri}");
    let mut counts = Counts::new();
    for group in response.groups {
        assert!(counts.insert(group.value, group.count).is_none(), "{uri} contains duplicate groups");
    }
    counts
}

async fn get<T: DeserializeOwned>(app: &Router, uri: &str) -> T {
    let response = app.clone().oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.expect("router response");
    let (parts, body) = response.into_parts();
    let body = axum::body::to_bytes(body, usize::MAX).await.expect("response body");
    assert!(parts.status.is_success(), "{uri}: {} {}", parts.status, String::from_utf8_lossy(&body));
    serde_json::from_slice(&body).unwrap_or_else(|error| panic!("{uri}: {error}"))
}

#[derive(Deserialize)]
struct SearchTotal {
    total: u64,
}

async fn assert_stats(snapshot: &Path, fields: &BTreeMap<String, BTreeMap<String, Usage>>) {
    let flags = ["--snapshot", snapshot.to_str().expect("UTF-8 snapshot path"), "--admin-bind", "", "--auth", "off"];
    let state = api::state(ServeArgs::from_flags(&flags).expect("serve flags")).expect("API state");
    let doc_count = u64::from(state.snapshot().doc_count());
    let app = api::router(state);
    // The high-cardinality route requires a filter. All possible first CNPJ
    // characters select the entire snapshot, including roots without establishments.
    let prefixes = ('0'..='9').chain('A'..='Z').map(|ch| ch.to_string()).collect::<Vec<_>>().join(",");
    let filter = format!("&cnpj={prefixes}");
    let SearchTotal { total } = get(&app, &format!("/api/v1/search?limit=1{filter}")).await;
    assert_eq!(total, doc_count, "CNPJ prefix filter must cover the entire snapshot");
    let ufs = stats(&app, "uf", "").await;
    println!("API /search confirms the CNPJ prefixes cover {total} documents, UF groups: {}", ufs.len());
    for (field, codes) in fields {
        let expected = stats(&app, field, &filter).await;
        let actual = codes.iter().map(|(code, usage)| (code.clone(), usage.count)).collect();
        assert_equal_groups(&expected, &actual, &format!("{field}{filter}"));
        assert!(codes.values().all(|usage| usage.by_uf.keys().all(|uf| uf.is_empty() || ufs.contains_key(uf))));
        for uf in ufs.keys() {
            let filter = format!("&uf={uf}");
            let expected = stats(&app, field, &filter).await;
            let actual = codes.iter().filter_map(|(code, usage)| usage.by_uf.get(uf).map(|&count| (code.clone(), count))).collect();
            assert_equal_groups(&expected, &actual, &format!("{field}{filter}"));
        }
        println!(
            "{field}: {} codes, {} document-code pairs, {} missing-UF pairs, all code counts and UF groups match /api/v1/stats",
            expected.len(),
            expected.values().sum::<u64>(),
            codes.values().filter_map(|usage| usage.by_uf.get("")).sum::<u64>(),
        );
    }
}

fn assert_equal_groups(expected: &Counts, actual: &Counts, label: &str) {
    for code in expected.keys().chain(actual.keys()) {
        assert_eq!(expected.get(code), actual.get(code), "{label}: code {code} differs (API vs. export)");
    }
}
