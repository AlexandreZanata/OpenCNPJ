use std::collections::BTreeMap;

use opencnpj_query::{
    ir::*,
    validate::{Capabilities, Issue, IssueCode, IssueKind, validate},
};

const INPUTS: &str = include_str!("fixtures/ir-v1-requests.jsonl");

#[derive(serde::Deserialize)]
struct Case {
    case: String,
    request: Request,
}

fn day(year: u16, month: u8, day: u8) -> Day {
    Day::from_ymd(year, month, day).unwrap()
}

fn competence(year: u16, month: u8) -> Competence {
    Competence::from_year_month(year, month).unwrap()
}

fn context() -> (BTreeMap<MunicipalityId, UfId>, BTreeMap<SectorId, SmallSet<CnaeCode>>) {
    (
        BTreeMap::from([(MunicipalityId(5067), UfId(12)), (MunicipalityId(5108), UfId(12)), (MunicipalityId(5527), UfId(12))]),
        BTreeMap::from([
            (SectorId(1), SmallSet::new([CnaeCode::Subclass(1091102), CnaeCode::Subclass(4721101), CnaeCode::Subclass(4721102)])),
            (SectorId(2), SmallSet::new([CnaeCode::Subclass(4771701), CnaeCode::Subclass(4771702), CnaeCode::Subclass(4771703)])),
        ]),
    )
}

fn canonical(request: Request) -> Request {
    let (municipality_ufs, sector_cnaes) = context();
    let context = CanonicalContext {
        first_day: Day(1),
        last_day: day(2026, 10, 11),
        municipality_ufs: &municipality_ufs,
        sector_cnaes: &sector_cnaes,
    };
    request.canonicalize(&context).unwrap().0
}

fn fixture(name: &str) -> Request {
    let case = INPUTS
        .lines()
        .map(|line| serde_json::from_str::<Case>(line).unwrap())
        .find(|case| case.case == name)
        .unwrap_or_else(|| panic!("missing fixture {name}"));
    canonical(case.request)
}

fn capabilities() -> Capabilities {
    Capabilities {
        current: competence(2026, 10),
        competences: [
            competence(2024, 11),
            competence(2025, 12),
            competence(2026, 4),
            competence(2026, 7),
            competence(2026, 9),
            competence(2026, 10),
        ]
        .to_vec(),
        flow_days: day(2025, 1, 1)..=day(2026, 10, 11),
        populations: vec![PopulationRef::IbgeCensus(2022), PopulationRef::IbgeEstimate(2026)],
        catalogue_versions: CatalogueVersions { municipalities: 202610, sectors: 1 },
        sector_count: 2,
        geography: true,
        list_limit: 100,
    }
}

fn set_metric(request: &mut Request, metric: Metric) {
    match &mut request.op {
        Op::Count { metric: current }
        | Op::Compare { metric: current, .. }
        | Op::Rank { metric: current, .. }
        | Op::Breakdown { metric: current, .. } => {
            *current = metric;
        }
        _ => panic!("operation has no metric"),
    }
}

fn expect_issue(request: Request, capabilities: &Capabilities, kind: IssueKind, code: IssueCode) -> Vec<Issue> {
    let issues = validate(request, capabilities).expect_err("request should be rejected");
    assert!(issues.iter().any(|issue| issue.kind == kind && issue.code == code), "expected {kind:?}/{code:?}, got {issues:?}");
    issues
}

#[test]
fn golden_requests_validate() {
    let capabilities = capabilities();
    for line in INPUTS.lines() {
        let case: Case = serde_json::from_str(line).unwrap();
        validate(canonical(case.request), &capabilities).unwrap_or_else(|issues| panic!("{}: {issues:?}", case.case));
    }
}

#[test]
fn population_metric_needs_place_dimension() {
    let capabilities = capabilities();
    let mut request = fixture("padarias_sorriso");
    set_metric(&mut request, Metric::Per10k(PopulationRef::IbgeEstimate(2026)));
    request.scope.place = PlaceFilter::Any;
    let issues = expect_issue(request, &capabilities, IssueKind::Missing, IssueCode::PopulationNeedsGeography);
    assert!(issues.iter().find(|issue| issue.code == IssueCode::PopulationNeedsGeography).unwrap().options.is_empty());

    let mut request = fixture("padarias_sorriso");
    set_metric(&mut request, Metric::Per10k(PopulationRef::IbgeEstimate(2026)));
    validate(request, &capabilities).unwrap();
}

#[test]
fn population_reference_must_be_available() {
    let capabilities = capabilities();
    let mut request = fixture("padarias_sorriso");
    set_metric(&mut request, Metric::Per10k(PopulationRef::IbgeEstimate(2025)));
    let issues = expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::PopulationUnavailable);
    let option = &issues.iter().find(|issue| issue.code == IssueCode::PopulationUnavailable).unwrap().options[0];
    let Metric::Per10k(reference) = option.op.metric().unwrap() else { panic!("expected density metric") };
    assert!(capabilities.populations.contains(&reference));

    let mut request = fixture("padarias_sorriso");
    set_metric(&mut request, Metric::Per10k(PopulationRef::IbgeCensus(2022)));
    validate(request, &capabilities).unwrap();
}

#[test]
fn population_filter_needs_reference() {
    let capabilities = capabilities();
    let mut request = fixture("padarias_sorriso");
    request.scope.place_population = Some(Range { from: Some(30_000), to: Some(50_000) });
    request.scope.population_ref = None;
    let issues = expect_issue(request, &capabilities, IssueKind::Missing, IssueCode::MissingPopulationRef);
    let option = &issues.iter().find(|issue| issue.code == IssueCode::MissingPopulationRef).unwrap().options[0];
    assert!(option.scope.population_ref.is_some());

    let mut request = fixture("padarias_sorriso");
    request.scope.place_population = Some(Range { from: Some(30_000), to: Some(50_000) });
    request.scope.population_ref = Some(PopulationRef::IbgeEstimate(2025));
    expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::PopulationUnavailable);

    let mut request = fixture("padarias_sorriso");
    request.scope.place_population = Some(Range { from: Some(30_000), to: Some(50_000) });
    request.scope.population_ref = Some(PopulationRef::IbgeEstimate(2026));
    validate(request, &capabilities).unwrap();
}

#[test]
fn active_change_requires_snapshots() {
    let capabilities = capabilities();
    validate(fixture("rank_sector_change_mt"), &capabilities).unwrap();

    let mut request = fixture("rank_sector_change_mt");
    request.time.period = Some(Period::Competences { from: competence(2026, 1), to: competence(2026, 9) });
    let issues = expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::ActiveChangeUnavailable);
    let option = &issues.iter().find(|issue| issue.code == IssueCode::ActiveChangeUnavailable).unwrap().options[0];
    assert!(matches!(option.op.metric(), Some(Metric::Flow(FlowMetric::Registrations))));
}

#[test]
fn active_change_missing_period_offers_default() {
    let capabilities = capabilities();
    let mut request = fixture("rank_sector_change_mt");
    request.time.period = None;
    let issues = expect_issue(request, &capabilities, IssueKind::Missing, IssueCode::MissingPeriod);
    let option = &issues.iter().find(|issue| issue.code == IssueCode::MissingPeriod).unwrap().options[0];
    assert!(matches!(option.time.period, Some(Period::Competences { from, to }) if from < to));
}

#[test]
fn stock_at_past_date_needs_snapshot() {
    let capabilities = capabilities();
    let mut request = fixture("padarias_sorriso");
    request.time.competence = competence(2026, 4);
    validate(request, &capabilities).unwrap();

    let mut request = fixture("padarias_sorriso");
    request.time.competence = competence(2025, 2);
    let issues = expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::SnapshotUnavailable);
    let option = &issues.iter().find(|issue| issue.code == IssueCode::SnapshotUnavailable).unwrap().options[0];
    assert_eq!(option.time.competence, capabilities.current);
}

#[test]
fn trend_needs_period_and_two_grains() {
    let capabilities = capabilities();
    validate(fixture("monthly_closures"), &capabilities).unwrap();

    let mut request = fixture("monthly_closures");
    request.time.period = None;
    let issues = expect_issue(request, &capabilities, IssueKind::Missing, IssueCode::MissingPeriod);
    assert!(issues.iter().find(|issue| issue.code == IssueCode::MissingPeriod).unwrap().options[0].time.period.is_some());

    let mut request = fixture("monthly_closures");
    request.time.period = Some(Period::Days(Range { from: Some(day(2026, 10, 1)), to: Some(day(2026, 10, 11)) }));
    let issues = expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::TrendNeedsTwoGrains);
    assert!(issues.iter().find(|issue| issue.code == IssueCode::TrendNeedsTwoGrains).unwrap().options[0].time.period.is_some());

    let mut request = fixture("monthly_closures");
    request.time.period = Some(Period::Competences { from: competence(2026, 4), to: competence(2026, 10) });
    expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::WrongPeriodKind);
}

#[test]
fn compare_requires_members_of_the_dimension() {
    let capabilities = capabilities();
    validate(fixture("compare_sorriso_com_sinop"), &capabilities).unwrap();

    let mut request = fixture("compare_sorriso_com_sinop");
    if let Op::Compare { members, .. } = &mut request.op {
        *members = SmallSet::new([Member::Municipality(MunicipalityId(5527))]);
    }
    expect_issue(request, &capabilities, IssueKind::Missing, IssueCode::CompareMembers);

    let mut request = fixture("compare_sorriso_com_sinop");
    if let Op::Compare { members, .. } = &mut request.op {
        *members = SmallSet::new((1..=5).map(|id| Member::Municipality(MunicipalityId(id))));
    }
    let issues = expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::CompareMembers);
    let option = &issues.iter().find(|issue| issue.code == IssueCode::CompareMembers).unwrap().options[0];
    let Op::Compare { members, .. } = &option.op else { panic!("expected comparison option") };
    assert_eq!(members.len(), 4);

    let mut request = fixture("compare_sorriso_com_sinop");
    if let Op::Compare { dim, .. } = &mut request.op {
        *dim = Dim::Uf;
    }
    expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::CompareMembers);

    let mut request = fixture("compare_sorriso_com_sinop");
    if let Op::Compare { dim, .. } = &mut request.op {
        *dim = Dim::Status;
    }
    expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::CompareMembers);
}

#[test]
fn sector_ranking_needs_the_catalogue() {
    let mut capabilities = capabilities();
    capabilities.sector_count = 1;
    expect_issue(fixture("rank_sector_change_mt"), &capabilities, IssueKind::Impossible, IssueCode::SectorCatalogueTooSmall);
}

#[test]
fn closures_require_closed_status() {
    let capabilities = capabilities();
    validate(fixture("monthly_closures"), &capabilities).unwrap();

    let mut request = fixture("monthly_closures");
    request.scope.status = Some(StatusSet::only(Status::Active));
    let issues = expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::ClosedFlowStatus);
    let option = &issues.iter().find(|issue| issue.code == IssueCode::ClosedFlowStatus).unwrap().options[0];
    assert_eq!(option.scope.status, Some(StatusSet::only(Status::Closed)));

    let mut request = fixture("padarias_sorriso");
    request.scope.status = Some(StatusSet::new([]));
    let issues = expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::EmptyStatus);
    let option = &issues.iter().find(|issue| issue.code == IssueCode::EmptyStatus).unwrap().options[0];
    assert_eq!(option.scope.status, Some(StatusSet::only(Status::Active)));
}

#[test]
fn mei_contradicts_small_and_other_porte() {
    let capabilities = capabilities();
    let mut request = fixture("padarias_sorriso");
    request.scope.mei = Tri::Yes;
    request.scope.porte = PorteSet::new([Porte::Small, Porte::Other]);
    let issues = expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::MeiPorteContradiction);
    let option = &issues.iter().find(|issue| issue.code == IssueCode::MeiPorteContradiction).unwrap().options[0];
    assert_eq!(option.scope.porte, PorteSet::ALL);

    let mut request = fixture("padarias_sorriso");
    request.scope.mei = Tri::Yes;
    request.scope.porte = PorteSet::new([Porte::Micro, Porte::Small]);
    validate(request, &capabilities).unwrap();
}

#[test]
fn capital_filter_is_allowed_with_establishments() {
    let capabilities = capabilities();
    validate(fixture("largest_capital_with_contact"), &capabilities).unwrap();

    let mut request = fixture("padarias_sorriso");
    request.scope.capital = Some(Range { from: Some(1_000_000), to: None });
    validate(request, &capabilities).unwrap();
}

#[test]
fn list_limits_and_contact() {
    let capabilities = capabilities();
    validate(fixture("largest_capital_with_contact"), &capabilities).unwrap();

    let mut request = fixture("largest_capital_with_contact");
    if let Op::List { limit, .. } = &mut request.op {
        *limit = 101;
    }
    let issues = expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::ListLimit);
    let option = &issues.iter().find(|issue| issue.code == IssueCode::ListLimit).unwrap().options[0];
    assert!(matches!(&option.op, Op::List { limit: 100, .. }));

    let mut request = fixture("padarias_sorriso");
    request.scope.contact = ContactNeed::Phone;
    let issues = expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::ContactOnlyForList);
    let option = &issues.iter().find(|issue| issue.code == IssueCode::ContactOnlyForList).unwrap().options[0];
    assert_eq!(option.scope.contact, ContactNeed::Any);

    let mut request = fixture("padarias_sorriso");
    request.output.page = 0;
    let issues = expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::InvalidPage);
    let option = &issues.iter().find(|issue| issue.code == IssueCode::InvalidPage).unwrap().options[0];
    assert_eq!(option.output.page, 1);
}

#[test]
fn near_filter_needs_geography() {
    let mut disabled = capabilities();
    disabled.geography = false;
    let issues = expect_issue(fixture("near_sinop_km"), &disabled, IssueKind::Impossible, IssueCode::NearNeedsGeography);
    let option = &issues.iter().find(|issue| issue.code == IssueCode::NearNeedsGeography).unwrap().options[0];
    assert!(matches!(&option.scope.place, PlaceFilter::Municipalities(ids) if ids.len() == 1));

    let mut enabled = capabilities();
    enabled.geography = true;
    validate(fixture("near_sinop_km"), &enabled).unwrap();
}

#[test]
fn lookup_ids_are_type_checked() {
    let capabilities = capabilities();
    validate(fixture("lookup_numeric"), &capabilities).unwrap();
    assert!(CnpjRef::parse("33.000.167/0001-02").is_err());
    assert!(serde_json::from_str::<CnpjRef>(r#""33000167000102""#).is_err());
}

#[test]
fn catalogue_versions_must_match() {
    let capabilities = capabilities();
    validate(fixture("padarias_sorriso"), &capabilities).unwrap();

    let mut request = fixture("padarias_sorriso");
    request.versions.sectors = 2;
    expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::CatalogueVersion);
}

#[test]
fn head_office_unit_contradicts_branches_only() {
    let capabilities = capabilities();
    let mut request = fixture("padarias_sorriso");
    request.unit = Unit::HeadOffices;
    request.scope.head_office = Tri::No;
    let issues = expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::HeadOfficeUnit);
    let option = &issues.iter().find(|issue| issue.code == IssueCode::HeadOfficeUnit).unwrap().options[0];
    assert_eq!(option.scope.head_office, Tri::Any);

    let mut request = fixture("padarias_sorriso");
    request.unit = Unit::HeadOffices;
    request.scope.head_office = Tri::Yes;
    validate(request, &capabilities).unwrap();
}

#[test]
fn empty_filters_are_rejected() {
    let capabilities = capabilities();
    let mut request = fixture("padarias_sorriso");
    request.scope.place = PlaceFilter::Municipalities(SmallSet::default());
    expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::EmptyFilter);

    let mut request = fixture("padarias_sorriso");
    request.scope.porte = PorteSet::new([]);
    expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::EmptyFilter);

    validate(fixture("padarias_sorriso"), &capabilities).unwrap();
}

#[test]
fn period_kind_must_match_metric() {
    let capabilities = capabilities();
    let mut request = fixture("padarias_sorriso");
    request.time.period = Some(Period::Days(Range { from: Some(day(2026, 1, 1)), to: Some(day(2026, 6, 1)) }));
    let issues = expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::WrongPeriodKind);
    let option = &issues.iter().find(|issue| issue.code == IssueCode::WrongPeriodKind).unwrap().options[0];
    assert_eq!(option.time.period, None);

    let mut request = fixture("empresas_abertas_2025_mt");
    request.time.period = Some(Period::Competences { from: competence(2026, 4), to: competence(2026, 10) });
    expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::WrongPeriodKind);
}

#[test]
fn flow_period_must_be_covered() {
    let capabilities = capabilities();
    validate(fixture("empresas_abertas_2025_mt"), &capabilities).unwrap();

    let mut request = fixture("empresas_abertas_2025_mt");
    request.time.period = Some(Period::Days(Range { from: Some(day(2024, 1, 1)), to: Some(day(2024, 12, 31)) }));
    expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::PeriodOutsideData);
}

#[test]
fn rank_limit_must_be_positive() {
    let capabilities = capabilities();
    let mut request = fixture("rank_cnae_section");
    if let Op::Rank { k, .. } = &mut request.op {
        *k = 0;
    }
    let issues = expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::InvalidRankLimit);
    let option = &issues.iter().find(|issue| issue.code == IssueCode::InvalidRankLimit).unwrap().options[0];
    assert!(matches!(&option.op, Op::Rank { k: 10, .. }));

    let mut request = fixture("rank_cnae_section");
    if let Op::Rank { k, .. } = &mut request.op {
        *k = 5;
    }
    validate(request, &capabilities).unwrap();
}

#[test]
fn percent_change_needs_minimum_base() {
    let capabilities = capabilities();
    validate(fixture("rank_change_percent"), &capabilities).unwrap();

    let mut request = fixture("rank_change_percent");
    if let Op::Rank { min_base, .. } = &mut request.op {
        *min_base = 10;
    }
    let issues = expect_issue(request, &capabilities, IssueKind::Missing, IssueCode::ActiveChangeMinBase);
    let option = &issues.iter().find(|issue| issue.code == IssueCode::ActiveChangeMinBase).unwrap().options[0];
    assert!(matches!(&option.op, Op::Rank { min_base: 30, .. }));
}

#[test]
fn issues_serialize_options_as_requests() {
    let capabilities = capabilities();
    let mut request = fixture("padarias_sorriso");
    request.scope.mei = Tri::Yes;
    request.scope.porte = PorteSet::new([Porte::Small]);
    let issues = expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::MeiPorteContradiction);
    let issue = issues.iter().find(|issue| issue.code == IssueCode::MeiPorteContradiction).unwrap();
    let json = serde_json::to_value(issue).unwrap();
    assert_eq!(json["kind"], "impossible");
    assert_eq!(json["code"], "mei_porte_contradiction");
    assert!(json["options"][0]["op"].is_object());
}

#[test]
fn invalid_ranges_are_rejected() {
    let capabilities = capabilities();
    let mut request = fixture("padarias_sorriso");
    request.scope.opened = Some(Range { from: Some(day(2026, 6, 1)), to: Some(day(2026, 1, 1)) });
    let issues = expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::InvalidRange);
    let option = &issues.iter().find(|issue| issue.code == IssueCode::InvalidRange).unwrap().options[0];
    assert_eq!(option.scope.opened, None);

    let mut request = fixture("rank_sector_change_mt");
    request.time.period = Some(Period::Competences { from: competence(2026, 10), to: competence(2026, 4) });
    expect_issue(request, &capabilities, IssueKind::Impossible, IssueCode::InvalidRange);
}
