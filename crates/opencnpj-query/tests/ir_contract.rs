use std::collections::BTreeMap;

use opencnpj_query::{
    codec::{IR_VERSION, cache_key, decode, encode},
    ir::*,
};
use proptest::prelude::*;
use serde::{Deserialize, Serialize};

const INPUTS: &str = include_str!("fixtures/ir-v1-requests.jsonl");
const EXPECTED: &str = include_str!("fixtures/ir-v1-golden.jsonl");

#[derive(Deserialize)]
struct Case {
    case: String,
    request: Request,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Golden {
    case: String,
    hex: String,
    text: String,
}

fn case_context() -> (BTreeMap<MunicipalityId, UfId>, BTreeMap<SectorId, SmallSet<CnaeCode>>) {
    (
        BTreeMap::from([(MunicipalityId(5067), UfId(12)), (MunicipalityId(5108), UfId(12)), (MunicipalityId(5527), UfId(12))]),
        BTreeMap::from([
            (SectorId(1), SmallSet::new([CnaeCode::Subclass(1091102), CnaeCode::Subclass(4721101), CnaeCode::Subclass(4721102)])),
            (SectorId(2), SmallSet::new([CnaeCode::Subclass(4771701), CnaeCode::Subclass(4771702), CnaeCode::Subclass(4771703)])),
        ]),
    )
}

fn goldens() -> Vec<Golden> {
    let (municipality_ufs, sector_cnaes) = case_context();
    let context = CanonicalContext {
        first_day: Day::from_ymd(1901, 1, 1).unwrap(),
        last_day: Day::from_ymd(2026, 10, 11).unwrap(),
        municipality_ufs: &municipality_ufs,
        sector_cnaes: &sector_cnaes,
    };
    INPUTS
        .lines()
        .map(|line| {
            let case: Case = serde_json::from_str(line).unwrap();
            let (canonical, _) = case.request.canonicalize(&context).unwrap();
            let bytes = encode(&canonical);
            assert_eq!(decode(&bytes).unwrap(), canonical, "{}", case.case);
            assert_eq!(canonical.canonicalize(&context).unwrap(), (canonical.clone(), Vec::new()));
            Golden { case: case.case, hex: hex::encode(bytes), text: canonical.to_string() }
        })
        .collect()
}

#[test]
fn golden_requests() {
    let actual = goldens();
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ir-v1-golden.jsonl");
        let lines: Vec<String> = actual.iter().map(|row| serde_json::to_string(row).unwrap()).collect();
        std::fs::write(path, lines.join("\n") + "\n").unwrap();
    }
    let expected = EXPECTED.lines().map(|line| serde_json::from_str::<Golden>(line).unwrap()).collect::<Vec<_>>();
    assert!(expected.len() >= 30);
    assert_eq!(actual, expected);
    let by_case = actual.iter().map(|row| (row.case.as_str(), row.hex.as_str())).collect::<BTreeMap<_, _>>();
    for (a, b) in [
        ("padarias_sorriso", "qtas_padarias_sorrisso_mt"),
        ("compare_sorriso_com_sinop", "sinop_vs_sorriso"),
        ("farmacias_ativas_cuiaba", "drogarias_em_funcionamento_cuiaba_mt"),
        ("empresas_abertas_2025_mt", "cnpjs_registrados_2025_mato_grosso"),
    ] {
        assert_eq!(by_case[a], by_case[b], "{a} and {b}");
    }
    for (a, b) in [
        ("empresas_abertas_2025_mt", "estavam_ativas_2025_mt"),
        ("padarias_sorriso", "fabricacao_de_paes_sorriso"),
        ("sorriso", "sorriso_e_regiao"),
        ("sem_filiais", "so_filiais"),
        ("compare_bakeries_pharmacies", "union_bakeries_pharmacies"),
    ] {
        assert_ne!(by_case[a], by_case[b], "{a} and {b}");
    }
    let lengths = actual.iter().map(|row| row.hex.len() / 2).collect::<Vec<_>>();
    println!("IR v{IR_VERSION}: {} cases, {}..={} bytes", lengths.len(), lengths.iter().min().unwrap(), lengths.iter().max().unwrap());
    for (row, len) in actual.iter().zip(lengths) {
        println!("{}\t{len}", row.case);
    }
}

#[test]
#[ignore = "prints deliberate golden fixture updates"]
fn print_golden_rows() {
    for row in goldens() {
        println!("{}", serde_json::to_string(&row).unwrap());
    }
}

#[test]
fn name_prefix_roundtrips_without_changing_legacy_requests() {
    let mut request = serde_json::from_str::<Case>(INPUTS.lines().next().unwrap()).unwrap().request;
    let legacy = encode(&request);
    assert_eq!(legacy[0], 1);
    assert_eq!(decode(&legacy).unwrap(), request);
    let original = cache_key(&request, b"first");

    request.scope.name_prefix = Some(NamePrefix { field: NameField::Trade, prefix: "ZA".into() });
    let bytes = encode(&request);
    assert_eq!(bytes[0], IR_VERSION);
    assert_eq!(decode(&bytes).unwrap(), request);
    assert!(decode(&bytes[..bytes.len() - 1]).is_err());
    assert_eq!(serde_json::from_slice::<Request>(&serde_json::to_vec(&request).unwrap()).unwrap(), request);
    assert_ne!(cache_key(&request, b"first"), original);
}

#[test]
fn cache_keys_use_aggregate_and_catalogue_versions() {
    let case: Case = serde_json::from_str(INPUTS.lines().nth(2).unwrap()).unwrap();
    let (municipality_ufs, sector_cnaes) = case_context();
    let context =
        CanonicalContext { first_day: Day(1), last_day: Day(60000), municipality_ufs: &municipality_ufs, sector_cnaes: &sector_cnaes };
    let (mut request, order) = case.request.canonicalize(&context).unwrap();
    assert_eq!(order, case.request.output.display_order);
    let original = cache_key(&request, b"first");
    request.output.display_order = order;
    assert_eq!(cache_key(&request, b"first"), original);
    assert_ne!(cache_key(&request, b"second"), original);
    request.versions.sectors += 1;
    assert_ne!(cache_key(&request, b"first"), original);
}

#[test]
fn clipping_and_conflicting_scope() {
    let case: Case = serde_json::from_str(INPUTS.lines().next().unwrap()).unwrap();
    let (municipality_ufs, sector_cnaes) = case_context();
    let context =
        CanonicalContext { first_day: Day(100), last_day: Day(900), municipality_ufs: &municipality_ufs, sector_cnaes: &sector_cnaes };
    let mut request = case.request;
    request.scope.opened = Some(Range { from: Some(Day(1)), to: Some(Day(1000)) });
    let (canonical, _) = request.canonicalize(&context).unwrap();
    assert_eq!(canonical.scope.opened, Some(Range { from: Some(Day(100)), to: Some(Day(900)) }));
    request.scope.within_ufs = SmallSet::new([UfId(25)]);
    assert_eq!(request.canonicalize(&context), Err(CanonicalError::ConflictingPlace));
    request.scope.within_ufs = SmallSet::default();
    request.scope.opened = Some(Range { from: Some(Day(901)), to: None });
    assert_eq!(request.canonicalize(&context), Err(CanonicalError::DateRangeOutsideData));
}

#[test]
fn exclusions_are_canonicalized_against_the_place() {
    let (municipality_ufs, sector_cnaes) = case_context();
    let context = CanonicalContext {
        first_day: Day::from_ymd(1901, 1, 1).unwrap(),
        last_day: Day::from_ymd(2026, 10, 11).unwrap(),
        municipality_ufs: &municipality_ufs,
        sector_cnaes: &sector_cnaes,
    };
    let case: Case =
        serde_json::from_str(INPUTS.lines().find(|line| serde_json::from_str::<Case>(line).unwrap().case == "exclusions").unwrap())
            .unwrap();
    let (canonical, _) = case.request.canonicalize(&context).unwrap();
    assert_eq!(canonical.scope.place, PlaceFilter::Ufs(SmallSet::new([UfId(12)])));
    assert!(canonical.scope.exclude.ufs.is_empty());
    assert_eq!(canonical.scope.exclude.municipalities, SmallSet::new([MunicipalityId(5108)]));
    let mut conflicting = case.request;
    conflicting.scope.place = PlaceFilter::Ufs(SmallSet::new([UfId(25)]));
    conflicting.scope.exclude.ufs = SmallSet::new([UfId(25)]);
    assert_eq!(conflicting.canonicalize(&context), Err(CanonicalError::ConflictingPlace));
}

#[test]
fn malformed_binary_and_invalid_cnpj_are_rejected() {
    let request = serde_json::from_str::<Case>(INPUTS.lines().next().unwrap()).unwrap().request;
    let bytes = encode(&request);
    for i in 0..bytes.len() {
        assert!(decode(&bytes[..i]).is_err(), "accepted truncation at byte {i}");
    }
    let mut wrong_version = bytes.clone();
    wrong_version[0] += 1;
    assert!(decode(&wrong_version).is_err());
    let mut nonminimal = bytes.clone();
    let final_byte = nonminimal.pop().unwrap();
    nonminimal.extend([final_byte | 0x80, 0]);
    assert!(decode(&nonminimal).is_err());
    let mut overflow = bytes.clone();
    overflow.pop();
    overflow.push(final_byte | 0x80);
    overflow.extend([0x80; 8]);
    overflow.push(2);
    assert!(decode(&overflow).is_err());
    let mut trailing = bytes;
    trailing.push(0);
    assert!(decode(&trailing).is_err());
    assert!(CnpjRef::parse("33.000.167/0001-02").is_err());
    assert!(serde_json::from_str::<CnpjRef>(r#""33000167000102""#).is_err());
}

#[test]
fn day_and_competence_share_the_documented_epoch() {
    assert_eq!(Day::from_ymd(1900, 1, 1), Some(Day(1)));
    assert_eq!(Day::from_ymd(1900, 2, 29), None);
    assert_eq!(Day::from_ymd(2000, 2, 29).unwrap().to_string(), "2000-02-29");
    assert_eq!(Day::from_ymd(9999, 12, 31).unwrap().to_string(), "9999-12-31");
    assert_eq!(Competence::from_year_month(2026, 10), Some(Competence(321)));
    assert_eq!(Competence::from_year_month(2026, 0), None);
}

fn requests() -> impl Strategy<Value = Request> {
    (any::<u8>(), any::<u8>(), any::<u32>(), any::<u64>(), any::<u16>(), prop::collection::vec(any::<u16>(), 0..24), any::<bool>())
        .prop_map(|(tag, flags, number, cents, id, ids, display)| {
            let members = SmallSet::new(ids.iter().copied().map(|id| Member::Municipality(MunicipalityId(id))));
            let period = match flags % 3 {
                0 => None,
                1 => Some(Period::Days(Range { from: Some(Day(1)), to: Some(Day(number)) })),
                _ => Some(Period::Competences { from: Competence(id), to: Competence(u16::MAX) }),
            };
            let metric = match flags % 6 {
                0 => Metric::Stock,
                1 => Metric::Flow(FlowMetric::Closures),
                2 => Metric::ActiveChange { pct: display },
                3 => Metric::Per10k(PopulationRef::IbgeCensus(2022)),
                4 => Metric::InhabitantsPer(PopulationRef::IbgeEstimate(2026)),
                _ => Metric::Share,
            };
            let op = match tag % 8 {
                0 => Op::Count { metric },
                1 => Op::List { order: ListOrder::NewestFirst, limit: id },
                2 => Op::Lookup { id: CnpjRef::parse(if display { "12.ABC.345/01DE-35" } else { "33.000.167" }).unwrap() },
                3 => Op::Compare { dim: Dim::Municipality, members, metric },
                4 => Op::Rank { dim: Dim::Region(RegionLevel::Immediate), metric, dir: Dir::Asc, k: flags, min_base: number },
                5 => Op::Breakdown { dim: Dim::Status, metric },
                6 => Op::Trend { metric: FlowMetric::MeiExits, grain: Grain::Year },
                _ => Op::Meta { topic: MetaTopic::Sources },
            };
            let scope = Filters {
                place: if display {
                    PlaceFilter::Municipalities(SmallSet::new(ids.iter().copied().map(MunicipalityId)))
                } else {
                    PlaceFilter::Any
                },
                status: display.then(|| StatusSet::new(StatusSet::ALL.iter().filter(|status| flags & (1 << *status as u8) != 0))),
                activity: ActivityFilter::Cnae {
                    codes: SmallSet::new(ids.iter().copied().map(|id| CnaeCode::Subclass(u32::from(id)))),
                    scope: CnaeScope::Any,
                },
                capital: Some(Range { from: None, to: Some(cents) }),
                ..Filters::default()
            };
            let display_order =
                if display { ids.into_iter().map(|id| Member::Municipality(MunicipalityId(id))).collect() } else { Vec::new() };
            Request {
                op,
                unit: Unit::HeadOffices,
                scope,
                time: TimeSpec { competence: Competence(id), period, anchor: Day(number) },
                output: Output { page: id, with_methodology: display, display_order },
                versions: CatalogueVersions { municipalities: number, sectors: u32::from(id) },
            }
        })
}

proptest! {
    #[test]
    fn binary_and_json_roundtrip(request in requests()) {
        prop_assert_eq!(decode(&encode(&request)).unwrap(), request.clone());
        prop_assert_eq!(serde_json::from_slice::<Request>(&serde_json::to_vec(&request).unwrap()).unwrap(), request);
    }

    #[test]
    fn canonicalization_is_idempotent(request in requests()) {
        let context = CanonicalContext { first_day: Day(1), last_day: Day(u32::MAX), municipality_ufs: &BTreeMap::new(), sector_cnaes: &BTreeMap::new() };
        if let Ok((canonical, _)) = request.canonicalize(&context) {
            prop_assert_eq!(canonical.canonicalize(&context).unwrap(), (canonical, Vec::new()));
        }
    }

    #[test]
    fn set_permutations_encode_identically(mut request in requests(), ids in prop::collection::vec(any::<u16>(), 0..100)) {
        request.scope.place = PlaceFilter::Municipalities(SmallSet::new(ids.iter().copied().map(MunicipalityId)));
        let first = encode(&request);
        request.scope.place = PlaceFilter::Municipalities(SmallSet::new(ids.iter().rev().chain(&ids).copied().map(MunicipalityId)));
        prop_assert_eq!(first, encode(&request));
    }
}
