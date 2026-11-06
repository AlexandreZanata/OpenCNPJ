//! Backend selection for validated requests.

use opencnpj_query::{
    ir::{ActivityFilter, Dim, FlowMetric, Metric, Op, Period, PlaceFilter, Status, Tri, Unit},
    validate::ValidRequest,
};

/// Backend selected for a validated request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// Monthly aggregate artifacts.
    Aggregates,
    /// Snapshot engine (`opencnpj_store::search` or the HTTP API).
    Engine,
    /// Catalogue and methodology answers.
    Static,
}

/// Static capability table decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    pub backend: Backend,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanError {
    Unsupported(&'static str),
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(reason) => write!(f, "no backend can serve {reason}"),
        }
    }
}

impl std::error::Error for PlanError {}

/// Select a backend for a canonical, validated request.
pub fn plan(request: &ValidRequest) -> Result<Plan, PlanError> {
    let plan = |backend, reason| Ok(Plan { backend, reason });
    match &request.op {
        Op::Meta { .. } => plan(Backend::Static, "catalogue or methodology answer"),
        Op::List { .. } => plan(Backend::Engine, "lists come from the engine"),
        Op::Lookup { .. } => plan(Backend::Engine, "CNPJ lookups come from the engine"),
        Op::Trend { .. } => {
            let metric = request.op.metric().expect("trend metric");
            if aggregate_metric(&metric) && aggregate_scope(request) && aligned(request) {
                plan(Backend::Aggregates, "flow buckets cover the aligned trend period")
            } else {
                plan(Backend::Engine, "trend needs the engine")
            }
        }
        Op::Count { .. } | Op::Compare { .. } | Op::Rank { .. } | Op::Breakdown { .. } => {
            let metric = request.op.metric().expect("operation metric");
            if let Metric::ActiveChange { .. } = metric {
                return active_change(request, &metric);
            }
            if aggregate_metric(&metric) && aggregate_scope(request) && aggregate_filters(request) {
                plan(Backend::Aggregates, "counts, rows and flows fit the aggregate tables")
            } else {
                plan(Backend::Engine, "a filter, dimension or period needs the engine")
            }
        }
    }
}

fn aggregate_metric(metric: &Metric) -> bool {
    match metric {
        Metric::Stock | Metric::Share | Metric::Per10k(_) | Metric::InhabitantsPer(_) | Metric::ActiveChange { .. } => true,
        Metric::Flow(FlowMetric::MeiOptIns | FlowMetric::MeiExits) => false,
        Metric::Flow(_) => true,
    }
}

fn aggregate_scope(request: &ValidRequest) -> bool {
    let scope = &request.scope;
    if scope.capital.is_some()
        || !scope.natureza.is_empty()
        || scope.opened.is_some()
        || scope.status_date.is_some()
        || scope.contact != opencnpj_query::ir::ContactNeed::Any
    {
        return false;
    }
    if matches!(
        &scope.activity,
        ActivityFilter::Sectors { scope: opencnpj_query::ir::CnaeScope::Any, .. }
            | ActivityFilter::Cnae { scope: opencnpj_query::ir::CnaeScope::Any, .. }
    ) {
        return false;
    }
    if let PlaceFilter::Region(id) = &scope.place
        && !(1..=5).contains(&id.0)
    {
        return false;
    }
    if let Some(period) = request.time.period
        && let Period::Days(days) = period
        && !aligned_days(days)
    {
        return false;
    }
    true
}

fn aggregate_filters(request: &ValidRequest) -> bool {
    if matches!(request.op.dimension(), Some(Dim::Year | Dim::Month)) && !matches!(request.op, Op::Trend { .. }) {
        return false;
    }
    let status = request.scope.status.unwrap_or_else(|| request.op.default_status());
    let active_only = status == opencnpj_query::ir::StatusSet::only(Status::Active);
    let detail =
        request.scope.porte != opencnpj_query::ir::PorteSet::ALL || request.scope.mei != Tri::Any || request.scope.simples != Tri::Any;
    if !active_only && detail {
        return false;
    }
    if request.unit == Unit::HeadOffices && request.scope.head_office == Tri::No {
        return false;
    }
    if let Some(Metric::Flow(flow)) = request.op.metric() {
        if request.unit == Unit::HeadOffices || request.scope.head_office != Tri::Any || detail {
            return false;
        }
        match flow {
            FlowMetric::Closures if status != opencnpj_query::ir::StatusSet::only(Status::Closed) => return false,
            FlowMetric::Registrations | FlowMetric::NetRegistrations if status != opencnpj_query::ir::StatusSet::ALL => return false,
            FlowMetric::MeiOptIns | FlowMetric::MeiExits => return false,
            _ => {}
        }
    }
    true
}

fn active_change(request: &ValidRequest, metric: &Metric) -> Result<Plan, PlanError> {
    let _ = metric;
    let status = request.scope.status.unwrap_or_else(|| request.op.default_status());
    let active_only = status == opencnpj_query::ir::StatusSet::only(Status::Active);
    let detail =
        request.scope.porte != opencnpj_query::ir::PorteSet::ALL || request.scope.mei != Tri::Any || request.scope.simples != Tri::Any;
    if !active_only && detail {
        return Err(PlanError::Unsupported("active change with porte, MEI or Simples outside the active stock table"));
    }
    if !aggregate_scope(request) || !aggregate_filters(request) {
        return Err(PlanError::Unsupported("active change needs two stock snapshots"));
    }
    Ok(Plan { backend: Backend::Aggregates, reason: "two historical stock snapshots" })
}

fn aligned(request: &ValidRequest) -> bool {
    match request.time.period {
        Some(Period::Days(days)) => aligned_days(days),
        Some(Period::Competences { .. }) => false,
        None => false,
    }
}

fn aligned_days(days: opencnpj_query::ir::DayRange) -> bool {
    let (Some(from), Some(to)) = (days.from, days.to) else { return false };
    let Some((from_year, from_month, from_day)) = from.to_ymd() else { return false };
    let Some((to_year, to_month, _)) = to.to_ymd() else { return false };
    if from_year < 2021 || from_day != 1 {
        return false;
    }
    let last = match to_month {
        12 => opencnpj_query::ir::Day::from_ymd(to_year + 1, 1, 1),
        _ => opencnpj_query::ir::Day::from_ymd(to_year, to_month + 1, 1),
    };
    last.is_some_and(|next| next.0 - 1 == to.0) && (from_year, from_month) <= (to_year, to_month)
}
