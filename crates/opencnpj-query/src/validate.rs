//! Validation of a request against the data and caller capabilities.

use std::{
    fmt,
    ops::{Deref, RangeInclusive},
};

use serde::Serialize;

use crate::ir::*;

/// Data and caller limits a request is validated against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    /// Served competence, used by engine lists and lookups and by flow tables.
    pub current: Competence,
    /// Competences that have a stock table, ascending and including `current`.
    pub competences: Vec<Competence>,
    /// Day range covered by the flow tables.
    pub flow_days: RangeInclusive<Day>,
    /// Population tables available.
    pub populations: Vec<PopulationRef>,
    /// Catalogue versions served to the interpreter.
    pub catalogue_versions: CatalogueVersions,
    /// Number of curated sectors in the served catalogue.
    pub sector_count: u16,
    /// Neighbour and radius assets are loaded.
    pub geography: bool,
    /// Maximum list length allowed by the caller plan.
    pub list_limit: u16,
}

/// Severity of a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueKind {
    /// A clarification can supply the missing information.
    Missing,
    /// Several readings are possible and one must be chosen.
    Ambiguous,
    /// The request cannot be executed with the available data or plan.
    Impossible,
}

/// Stable issue code used by policy, logs and clarification templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueCode {
    CatalogueVersion,
    EmptyFilter,
    EmptyStatus,
    MissingPopulationRef,
    PopulationNeedsGeography,
    PopulationUnavailable,
    ActiveChangeUnavailable,
    ActiveChangeMinBase,
    SnapshotUnavailable,
    MissingPeriod,
    WrongPeriodKind,
    PeriodOutsideData,
    TrendNeedsTwoGrains,
    CompareMembers,
    SectorCatalogueTooSmall,
    ClosedFlowStatus,
    MeiPorteContradiction,
    ContactOnlyForList,
    ListLimit,
    InvalidPage,
    InvalidRange,
    InvalidRankLimit,
    NearNeedsGeography,
    HeadOfficeUnit,
}

impl IssueCode {
    /// Short English description used in logs and diagnostics.
    pub fn message(self) -> &'static str {
        match self {
            Self::CatalogueVersion => "request was resolved with a different catalogue version",
            Self::EmptyFilter => "filter selects no values",
            Self::EmptyStatus => "status filter selects no status",
            Self::MissingPopulationRef => "population filter needs a population reference",
            Self::PopulationNeedsGeography => "population metric or filter needs a place dimension or filter",
            Self::PopulationUnavailable => "population reference is not available",
            Self::ActiveChangeUnavailable => "active change needs two available snapshots covering the period",
            Self::ActiveChangeMinBase => "percentage change needs a minimum base of 30",
            Self::SnapshotUnavailable => "the requested snapshot is not available",
            Self::MissingPeriod => "metric needs a period",
            Self::WrongPeriodKind => "period kind does not match the metric",
            Self::PeriodOutsideData => "period is outside the flow coverage",
            Self::TrendNeedsTwoGrains => "trend needs a period of at least two grains",
            Self::CompareMembers => "comparison needs two to four members of the stated dimension",
            Self::SectorCatalogueTooSmall => "sector ranking or comparison needs at least two catalogue sectors",
            Self::ClosedFlowStatus => "closure flows require status closed",
            Self::MeiPorteContradiction => "MEI contradicts the selected porte",
            Self::ContactOnlyForList => "contact filters require a list or a count of establishments",
            Self::ListLimit => "list limit exceeds the caller plan",
            Self::InvalidPage => "page is not valid for this operation",
            Self::InvalidRange => "range is empty",
            Self::InvalidRankLimit => "ranking limit must be positive",
            Self::NearNeedsGeography => "near filter needs geography assets",
            Self::HeadOfficeUnit => "head-office unit contradicts a branches-only filter",
        }
    }
}

/// A validation issue with optional complete IR alternatives for a clarification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Issue {
    pub kind: IssueKind,
    pub code: IssueCode,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<ValidRequest>,
}

impl Issue {
    fn new(kind: IssueKind, code: IssueCode) -> Self {
        Self { kind, code, options: Vec::new() }
    }

    fn missing(code: IssueCode) -> Self {
        Self::new(IssueKind::Missing, code)
    }

    fn impossible(code: IssueCode) -> Self {
        Self::new(IssueKind::Impossible, code)
    }

    fn with(mut self, option: Option<ValidRequest>) -> Self {
        self.options.extend(option);
        self
    }
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {} ({} option(s))", self.kind, self.code.message(), self.options.len())
    }
}

/// A request accepted by [`validate`].
///
/// The tuple field is private, so external code cannot fabricate one:
///
/// ```compile_fail
/// use opencnpj_query::{ir::Request, validate::ValidRequest};
/// fn request() -> Request {
///     unimplemented!()
/// }
/// let _ = ValidRequest(request());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ValidRequest(Request);

impl Deref for ValidRequest {
    type Target = Request;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ValidRequest {
    pub fn as_request(&self) -> &Request {
        &self.0
    }

    pub fn into_request(self) -> Request {
        self.0
    }
}

/// Validate `request` against `capabilities`, returning every issue found.
pub fn validate(request: Request, capabilities: &Capabilities) -> Result<ValidRequest, Vec<Issue>> {
    let mut issues = Vec::new();
    if request.versions != capabilities.catalogue_versions {
        push_issue(&mut issues, Issue::impossible(IssueCode::CatalogueVersion));
    }
    check_scope(&request, capabilities, &mut issues);
    check_op(&request, capabilities, &mut issues);
    if issues.is_empty() { Ok(ValidRequest(request)) } else { Err(issues) }
}

fn push_issue(issues: &mut Vec<Issue>, mut issue: Issue) {
    if let Some(existing) = issues.iter_mut().find(|existing| existing.kind == issue.kind && existing.code == issue.code) {
        existing.options.append(&mut issue.options);
    } else {
        issues.push(issue);
    }
}

fn option(request: &Request, capabilities: &Capabilities, fix: impl FnOnce(&mut Request)) -> Option<ValidRequest> {
    let mut candidate = request.clone();
    fix(&mut candidate);
    validate(candidate, capabilities).ok()
}

fn set_metric(request: &mut Request, metric: Metric) -> bool {
    match &mut request.op {
        Op::Count { metric: current }
        | Op::Compare { metric: current, .. }
        | Op::Rank { metric: current, .. }
        | Op::Breakdown { metric: current, .. } => {
            *current = metric;
            true
        }
        _ => false,
    }
}

fn with_population(metric: Metric, population: PopulationRef) -> Option<Metric> {
    match metric {
        Metric::Per10k(_) => Some(Metric::Per10k(population)),
        Metric::InhabitantsPer(_) => Some(Metric::InhabitantsPer(population)),
        _ => None,
    }
}

fn check_scope(request: &Request, capabilities: &Capabilities, issues: &mut Vec<Issue>) {
    let scope = &request.scope;
    match &scope.place {
        PlaceFilter::Municipalities(ids) if ids.is_empty() => push_issue(issues, Issue::impossible(IssueCode::EmptyFilter)),
        PlaceFilter::Ufs(ids) if ids.is_empty() => push_issue(issues, Issue::impossible(IssueCode::EmptyFilter)),
        PlaceFilter::Near { center, rule } => {
            let positive = match rule {
                NearRule::Neighbours(depth) => *depth > 0,
                NearRule::WithinKm(km) => *km > 0,
            };
            if !positive {
                push_issue(issues, Issue::impossible(IssueCode::InvalidRange));
            }
            if !capabilities.geography {
                let option = option(request, capabilities, |candidate| {
                    candidate.scope.place = PlaceFilter::Municipalities(SmallSet::new([*center]));
                    candidate.scope.within_ufs = SmallSet::default();
                });
                push_issue(issues, Issue::impossible(IssueCode::NearNeedsGeography).with(option));
            }
        }
        _ => {}
    }
    match &scope.activity {
        ActivityFilter::Sectors { ids, cnaes, .. } if ids.is_empty() || cnaes.is_empty() => {
            push_issue(issues, Issue::impossible(IssueCode::EmptyFilter));
        }
        ActivityFilter::Cnae { codes, .. } if codes.is_empty() => push_issue(issues, Issue::impossible(IssueCode::EmptyFilter)),
        _ => {}
    }
    if scope.status.is_some_and(|status| status.is_empty()) {
        let default = request.op.default_status();
        let option = option(request, capabilities, |candidate| candidate.scope.status = Some(default));
        push_issue(issues, Issue::impossible(IssueCode::EmptyStatus).with(option));
    }
    if scope.porte.is_empty() {
        push_issue(issues, Issue::impossible(IssueCode::EmptyFilter));
    }
    if scope.mei == Tri::Yes && !scope.porte.is_empty() && !scope.porte.contains(Porte::Micro) {
        let option = option(request, capabilities, |candidate| candidate.scope.porte = PorteSet::ALL);
        push_issue(issues, Issue::impossible(IssueCode::MeiPorteContradiction).with(option));
    }
    if scope.contact != ContactNeed::Any && !matches!(&request.op, Op::List { .. } | Op::Count { metric: Metric::Stock }) {
        let option = option(request, capabilities, |candidate| candidate.scope.contact = ContactNeed::Any);
        push_issue(issues, Issue::impossible(IssueCode::ContactOnlyForList).with(option));
    }
    if request.unit == Unit::HeadOffices && scope.head_office == Tri::No {
        let option = option(request, capabilities, |candidate| candidate.scope.head_office = Tri::Any);
        push_issue(issues, Issue::impossible(IssueCode::HeadOfficeUnit).with(option));
    }
    if scope.opened.is_some_and(|range| !ordered(&range)) {
        let option = option(request, capabilities, |candidate| candidate.scope.opened = None);
        push_issue(issues, Issue::impossible(IssueCode::InvalidRange).with(option));
    }
    if scope.status_date.is_some_and(|range| !ordered(&range)) {
        let option = option(request, capabilities, |candidate| candidate.scope.status_date = None);
        push_issue(issues, Issue::impossible(IssueCode::InvalidRange).with(option));
    }
    if scope.capital.is_some_and(|range| !ordered(&range)) {
        let option = option(request, capabilities, |candidate| candidate.scope.capital = None);
        push_issue(issues, Issue::impossible(IssueCode::InvalidRange).with(option));
    }
    if scope.place_population.is_some_and(|range| !ordered(&range)) {
        let option = option(request, capabilities, |candidate| candidate.scope.place_population = None);
        push_issue(issues, Issue::impossible(IssueCode::InvalidRange).with(option));
    }
    if scope.place_population.is_some() {
        if !population_dimension(request) {
            push_issue(issues, Issue::missing(IssueCode::PopulationNeedsGeography));
        }
        match scope.population_ref {
            None => {
                let option = capabilities
                    .populations
                    .first()
                    .and_then(|available| option(request, capabilities, |candidate| candidate.scope.population_ref = Some(*available)));
                push_issue(issues, Issue::missing(IssueCode::MissingPopulationRef).with(option));
            }
            Some(reference) if !capabilities.populations.contains(&reference) => {
                let option = capabilities
                    .populations
                    .first()
                    .and_then(|available| option(request, capabilities, |candidate| candidate.scope.population_ref = Some(*available)));
                push_issue(issues, Issue::impossible(IssueCode::PopulationUnavailable).with(option));
            }
            Some(_) => {}
        }
    }
}

fn check_op(request: &Request, capabilities: &Capabilities, issues: &mut Vec<Issue>) {
    match &request.op {
        Op::List { limit, .. } => {
            check_current(request, capabilities, issues);
            if request.time.period.is_some() {
                let option = option(request, capabilities, |candidate| candidate.time.period = None);
                push_issue(issues, Issue::impossible(IssueCode::WrongPeriodKind).with(option));
            }
            if *limit == 0 || *limit > capabilities.list_limit {
                let clamped = (*limit).clamp(1, capabilities.list_limit.max(1));
                let option = option(request, capabilities, |candidate| {
                    if let Op::List { limit, .. } = &mut candidate.op {
                        *limit = clamped;
                    }
                });
                push_issue(issues, Issue::impossible(IssueCode::ListLimit).with(option));
            }
        }
        Op::Lookup { .. } => check_current(request, capabilities, issues),
        Op::Meta { .. } => {}
        Op::Trend { metric, grain } => {
            check_current(request, capabilities, issues);
            check_flow_status(*metric, request, capabilities, issues);
            check_flow_period(request, capabilities, issues);
            check_trend_grains(*grain, request, capabilities, issues);
        }
        Op::Count { metric } | Op::Breakdown { metric, .. } => check_metric(metric, request, capabilities, issues),
        Op::Compare { dim, members, metric } => {
            check_compare(*dim, members, request, capabilities, issues);
            check_metric(metric, request, capabilities, issues);
        }
        Op::Rank { dim, metric, k, min_base, .. } => {
            check_rank(*dim, *k, request, capabilities, issues);
            check_metric(metric, request, capabilities, issues);
            if let Metric::ActiveChange { pct: true } = metric
                && *min_base < 30
            {
                let option = option(request, capabilities, |candidate| {
                    if let Op::Rank { min_base, .. } = &mut candidate.op {
                        *min_base = 30;
                    }
                });
                push_issue(issues, Issue::missing(IssueCode::ActiveChangeMinBase).with(option));
            }
        }
    }
    let page_ok = if matches!(&request.op, Op::List { .. }) { request.output.page > 0 } else { request.output.page == 1 };
    if !page_ok {
        let option = option(request, capabilities, |candidate| candidate.output.page = 1);
        push_issue(issues, Issue::impossible(IssueCode::InvalidPage).with(option));
    }
}

fn check_metric(metric: &Metric, request: &Request, capabilities: &Capabilities, issues: &mut Vec<Issue>) {
    match metric {
        Metric::Stock | Metric::Share => check_stock(request, capabilities, issues, true),
        Metric::Per10k(reference) | Metric::InhabitantsPer(reference) => {
            if !population_dimension(request) {
                push_issue(issues, Issue::missing(IssueCode::PopulationNeedsGeography));
            }
            if !capabilities.populations.contains(reference) {
                let option = request.op.metric().and_then(|metric| {
                    let available = *capabilities.populations.first()?;
                    let replacement = with_population(metric, available)?;
                    option(request, capabilities, |candidate| {
                        set_metric(candidate, replacement);
                    })
                });
                push_issue(issues, Issue::impossible(IssueCode::PopulationUnavailable).with(option));
            }
            check_stock(request, capabilities, issues, true);
        }
        Metric::ActiveChange { pct } => check_active_change(*pct, request, capabilities, issues),
        Metric::Flow(flow) => {
            check_current(request, capabilities, issues);
            check_flow_status(*flow, request, capabilities, issues);
            check_flow_period(request, capabilities, issues);
        }
    }
}

fn check_stock(request: &Request, capabilities: &Capabilities, issues: &mut Vec<Issue>, allow_past: bool) {
    if request.time.period.is_some() {
        let option = option(request, capabilities, |candidate| candidate.time.period = None);
        push_issue(issues, Issue::impossible(IssueCode::WrongPeriodKind).with(option));
    }
    if allow_past {
        if !capabilities.competences.contains(&request.time.competence) {
            let option = capabilities
                .competences
                .last()
                .and_then(|latest| option(request, capabilities, |candidate| candidate.time.competence = *latest));
            push_issue(issues, Issue::impossible(IssueCode::SnapshotUnavailable).with(option));
        }
    } else {
        check_current(request, capabilities, issues);
    }
}

fn check_current(request: &Request, capabilities: &Capabilities, issues: &mut Vec<Issue>) {
    if request.time.competence != capabilities.current {
        let option = option(request, capabilities, |candidate| candidate.time.competence = capabilities.current);
        push_issue(issues, Issue::impossible(IssueCode::SnapshotUnavailable).with(option));
    }
}

fn check_active_change(pct: bool, request: &Request, capabilities: &Capabilities, issues: &mut Vec<Issue>) {
    match request.time.period {
        None => {
            let option = default_change(capabilities)
                .and_then(|(from, to)| {
                    option(request, capabilities, |candidate| candidate.time.period = Some(Period::Competences { from, to }))
                })
                .or_else(|| default_flow_days(request, capabilities).and_then(|days| registrations_option(request, capabilities, days)));
            push_issue(issues, Issue::missing(IssueCode::MissingPeriod).with(option));
        }
        Some(Period::Competences { from, to }) => {
            if from >= to {
                push_issue(issues, Issue::impossible(IssueCode::InvalidRange));
            } else if !capabilities.competences.contains(&from) || !capabilities.competences.contains(&to) {
                let option = registrations_option(request, capabilities, Range { from: Some(first_day(from)), to: Some(last_day(to)) });
                push_issue(issues, Issue::impossible(IssueCode::ActiveChangeUnavailable).with(option));
            }
        }
        Some(Period::Days(days)) => {
            let option = registrations_option(request, capabilities, days);
            push_issue(issues, Issue::impossible(IssueCode::WrongPeriodKind).with(option));
        }
    }
    if pct && matches!(&request.op, Op::Rank { min_base, .. } if *min_base < 30) {
        let option = option(request, capabilities, |candidate| {
            if let Op::Rank { min_base, .. } = &mut candidate.op {
                *min_base = 30;
            }
        });
        push_issue(issues, Issue::missing(IssueCode::ActiveChangeMinBase).with(option));
    }
}

fn check_flow_status(flow: FlowMetric, request: &Request, capabilities: &Capabilities, issues: &mut Vec<Issue>) {
    if flow == FlowMetric::Closures
        && let Some(status) = request.scope.status
        && status != StatusSet::only(Status::Closed)
    {
        let option = option(request, capabilities, |candidate| candidate.scope.status = Some(StatusSet::only(Status::Closed)));
        push_issue(issues, Issue::impossible(IssueCode::ClosedFlowStatus).with(option));
    }
}

fn check_flow_period(request: &Request, capabilities: &Capabilities, issues: &mut Vec<Issue>) {
    match request.time.period {
        None => {
            let option = default_flow_days(request, capabilities)
                .and_then(|days| option(request, capabilities, |candidate| candidate.time.period = Some(Period::Days(days))));
            push_issue(issues, Issue::missing(IssueCode::MissingPeriod).with(option));
        }
        Some(Period::Days(range)) => {
            if !ordered(&range) {
                push_issue(issues, Issue::impossible(IssueCode::InvalidRange));
            } else if !covered(&range, &capabilities.flow_days) {
                let option = clamp_days(range, &capabilities.flow_days)
                    .and_then(|days| option(request, capabilities, |candidate| candidate.time.period = Some(Period::Days(days))));
                push_issue(issues, Issue::impossible(IssueCode::PeriodOutsideData).with(option));
            }
        }
        Some(Period::Competences { from, to }) => {
            let option = (from < to)
                .then(|| Range { from: Some(first_day(from)), to: Some(last_day(to)) })
                .and_then(|days| option(request, capabilities, |candidate| candidate.time.period = Some(Period::Days(days))));
            push_issue(issues, Issue::impossible(IssueCode::WrongPeriodKind).with(option));
        }
    }
}

fn check_trend_grains(grain: Grain, request: &Request, capabilities: &Capabilities, issues: &mut Vec<Issue>) {
    if let Some(Period::Days(range)) = request.time.period {
        let from = range.from.unwrap_or(*capabilities.flow_days.start());
        let to = range.to.unwrap_or(*capabilities.flow_days.end());
        if !two_grains(from, to, grain) {
            let option = default_flow_days(request, capabilities).and_then(|days| {
                let from = days.from?;
                let to = days.to?;
                two_grains(from, to, grain)
                    .then(|| option(request, capabilities, |candidate| candidate.time.period = Some(Period::Days(days))))
                    .flatten()
            });
            push_issue(issues, Issue::impossible(IssueCode::TrendNeedsTwoGrains).with(option));
        }
    }
}

fn check_compare(dim: Dim, members: &Members, request: &Request, capabilities: &Capabilities, issues: &mut Vec<Issue>) {
    let same_dimension = members.iter().all(|member| {
        matches!(
            (dim, member),
            (Dim::Municipality, Member::Municipality(_))
                | (Dim::Uf, Member::Uf(_))
                | (Dim::Region(_), Member::Region(_))
                | (Dim::Sector, Member::Sector(_))
        )
    });
    if !same_dimension {
        push_issue(issues, Issue::impossible(IssueCode::CompareMembers));
    }
    match members.len() {
        0 | 1 => push_issue(issues, Issue::missing(IssueCode::CompareMembers)),
        2..=4 => {}
        _ => {
            let option = option(request, capabilities, |candidate| {
                if let Op::Compare { members, .. } = &mut candidate.op {
                    *members = SmallSet::new(members.iter().take(4).copied());
                }
            });
            push_issue(issues, Issue::impossible(IssueCode::CompareMembers).with(option));
        }
    }
    if dim == Dim::Sector && capabilities.sector_count < 2 {
        push_issue(issues, Issue::impossible(IssueCode::SectorCatalogueTooSmall));
    }
}

fn check_rank(dim: Dim, k: u8, request: &Request, capabilities: &Capabilities, issues: &mut Vec<Issue>) {
    if k == 0 {
        let option = option(request, capabilities, |candidate| {
            if let Op::Rank { k, .. } = &mut candidate.op {
                *k = 10;
            }
        });
        push_issue(issues, Issue::impossible(IssueCode::InvalidRankLimit).with(option));
    }
    if dim == Dim::Sector && capabilities.sector_count < 2 {
        push_issue(issues, Issue::impossible(IssueCode::SectorCatalogueTooSmall));
    }
}

fn population_dimension(request: &Request) -> bool {
    matches!(request.op.dimension(), Some(Dim::Municipality | Dim::Uf | Dim::Region(_)))
        || matches!(
            request.scope.place,
            PlaceFilter::Municipalities(_) | PlaceFilter::Ufs(_) | PlaceFilter::Region(_) | PlaceFilter::Near { .. }
        )
}

fn registrations_option(request: &Request, capabilities: &Capabilities, days: DayRange) -> Option<ValidRequest> {
    let mut candidate = request.clone();
    if !set_metric(&mut candidate, Metric::Flow(FlowMetric::Registrations)) {
        return None;
    }
    candidate.scope.status = Some(StatusSet::ALL);
    candidate.time.period = Some(Period::Days(days));
    validate(candidate, capabilities).ok()
}

fn default_change(capabilities: &Capabilities) -> Option<(Competence, Competence)> {
    let len = capabilities.competences.len();
    let from = *capabilities.competences.get(len.checked_sub(2)?)?;
    let to = *capabilities.competences.last()?;
    (from < to).then_some((from, to))
}

fn default_flow_days(request: &Request, capabilities: &Capabilities) -> Option<DayRange> {
    let end = if request.time.anchor >= *capabilities.flow_days.start() && request.time.anchor <= *capabilities.flow_days.end() {
        request.time.anchor
    } else {
        *capabilities.flow_days.end()
    };
    let start = Day(end.0.saturating_sub(365).max(capabilities.flow_days.start().0));
    (start <= end).then_some(Range { from: Some(start), to: Some(end) })
}

fn covered(range: &DayRange, flow: &RangeInclusive<Day>) -> bool {
    range.from.is_none_or(|from| from >= *flow.start()) && range.to.is_none_or(|to| to <= *flow.end())
}

fn clamp_days(range: DayRange, flow: &RangeInclusive<Day>) -> Option<DayRange> {
    let from = range.from.unwrap_or(*flow.start()).max(*flow.start());
    let to = range.to.unwrap_or(*flow.end()).min(*flow.end());
    (from <= to).then_some(Range { from: Some(from), to: Some(to) })
}

fn two_grains(from: Day, to: Day, grain: Grain) -> bool {
    match (from.to_ymd(), to.to_ymd()) {
        (Some((from_year, from_month, _)), Some((to_year, to_month, _))) => match grain {
            Grain::Month => (from_year, from_month) != (to_year, to_month),
            Grain::Year => from_year != to_year,
        },
        _ => false,
    }
}

fn ordered<T: Ord>(range: &Range<T>) -> bool {
    match (&range.from, &range.to) {
        (Some(from), Some(to)) => from <= to,
        _ => true,
    }
}

fn first_day(competence: Competence) -> Day {
    Day::from_ymd(competence.year(), competence.month(), 1).expect("valid competence")
}

fn last_day(competence: Competence) -> Day {
    let (year, month) = (competence.year(), competence.month());
    let next = if month == 12 { Day::from_ymd(year + 1, 1, 1) } else { Day::from_ymd(year, month + 1, 1) };
    Day(next.expect("valid next month").0 - 1)
}
