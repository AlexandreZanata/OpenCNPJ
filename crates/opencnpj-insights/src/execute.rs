//! Aggregate execution for validated requests.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use anyhow::{Context as _, Result, bail};
use opencnpj_query::{
    ir::{
        ActivityFilter, CnaeCode, CnaeLevel, Competence, Dim, Dir, FlowMetric, Grain, Member, Metric, NearRule, Op, Period, PlaceFilter,
        PopulationRef, Porte, PorteSet, RegionLevel, Status, StatusSet, Tri, Unit,
    },
    validate::ValidRequest,
};

use crate::{
    agg::{self, ActiveDetail, Aggregate, Key, MUNICIPALITY_MISSING, Section, Table, UF_MISSING, flow_bounds, unpack_stock_detail},
    catalog::municipalities::{Geography, Municipality, MunicipalityCatalogue, Place},
    plan::{Backend, Plan, plan},
};

pub type SectorMap = BTreeMap<u16, BTreeSet<u32>>;

/// Aggregates served to the executor: the current artifact and the historical stock.
pub struct AggregateSet {
    current: Aggregate,
    history: BTreeMap<Competence, Aggregate>,
}

impl AggregateSet {
    pub fn open(current: &Path, history: Option<&Path>) -> Result<Self> {
        let current = Aggregate::open(current)?;
        let mut historical = BTreeMap::new();
        if let Some(directory) = history {
            for entry in std::fs::read_dir(directory).with_context(|| format!("read {}", directory.display()))? {
                let path = entry?.path();
                if path.extension().and_then(|value| value.to_str()) != Some("ocagg") {
                    continue;
                }
                let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else { continue };
                let Some((year, month)) = stem.split_once('-') else { continue };
                let (Ok(year), Ok(month)) = (year.parse(), month.parse()) else { continue };
                let Some(competence) = Competence::from_year_month(year, month) else { continue };
                if competence == current.manifest().competence {
                    continue;
                }
                let aggregate = Aggregate::open(&path)?;
                // The file name picks the competence; the manifest must agree, or a renamed
                // file would answer for the wrong month.
                if aggregate.manifest().competence != competence {
                    bail!("{} declares competence {} in its manifest", path.display(), aggregate.manifest().competence);
                }
                historical.insert(competence, aggregate);
            }
        }
        Ok(Self { current, history: historical })
    }

    pub fn current(&self) -> &Aggregate {
        &self.current
    }

    pub fn get(&self, competence: Competence) -> Option<&Aggregate> {
        if competence == self.current.manifest().competence { Some(&self.current) } else { self.history.get(&competence) }
    }
}

/// Catalogue data used by aggregate execution.
pub struct ExecContext<'a> {
    pub municipalities: &'a MunicipalityCatalogue,
    pub sectors: &'a SectorMap,
    cnae_sectors: BTreeMap<u32, Vec<u16>>,
}

impl<'a> ExecContext<'a> {
    pub fn new(municipalities: &'a MunicipalityCatalogue, sectors: &'a SectorMap) -> Self {
        let mut cnae_sectors: BTreeMap<u32, Vec<u16>> = BTreeMap::new();
        for (&sector, cnaes) in sectors {
            for cnae in cnaes {
                cnae_sectors.entry(*cnae).or_default().push(sector);
            }
        }
        Self { municipalities, sectors, cnae_sectors }
    }

    pub fn sectors_of(&self, cnae: u32) -> &[u16] {
        self.cnae_sectors.get(&cnae).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// One output row. `value` is the metric numerator and `base` its denominator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub key: u64,
    pub value: i64,
    pub base: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Scalar { value: i64 },
    Ratio { num: u64, den: u64 },
    Rows(Vec<Row>),
}

/// Execute an aggregate-backed request.
pub fn execute(request: &ValidRequest, aggregates: &AggregateSet, context: &ExecContext<'_>) -> Result<Value> {
    match plan(request)? {
        Plan { backend: Backend::Aggregates, .. } => {}
        Plan { backend, .. } => bail!("request needs the {backend:?} backend"),
    }
    let filters = Filters::new(request, context)?;
    let metric = request.op.metric().context("operation metric")?;
    match &request.op {
        Op::Count { .. } => scalar(request, &filters, metric, aggregates, context),
        Op::Compare { dim, members, .. } => compare(request, &filters, metric, *dim, members, aggregates, context),
        Op::Rank { dim, dir, k, min_base, .. } => {
            rank(request, &filters, metric, RankSpec { dim: *dim, dir: *dir, k: *k, min_base: *min_base }, aggregates, context)
        }
        Op::Breakdown { dim, .. } => breakdown(request, &filters, metric, *dim, aggregates, context),
        Op::Trend { grain, .. } => trend(request, &filters, *grain, aggregates, context),
        Op::List { .. } | Op::Lookup { .. } | Op::Meta { .. } => bail!("operation is not aggregate-backed"),
    }
}

#[derive(Debug, Clone)]
enum CnaePattern {
    Exact { value: u32, divisor: u32 },
    Section(u8),
}

impl CnaePattern {
    fn matches(&self, cnae: u32) -> bool {
        match self {
            Self::Exact { value, divisor } => cnae / divisor == *value,
            Self::Section(letter) => section_of(cnae) == *letter,
        }
    }
}

#[derive(Debug, Clone)]
struct Filters {
    municipalities: Option<BTreeSet<u16>>,
    ufs: Option<BTreeSet<u8>>,
    cnaes: Option<Vec<CnaePattern>>,
    exclude_municipalities: BTreeSet<u16>,
    exclude_ufs: BTreeSet<u8>,
    exclude_cnaes: BTreeSet<u32>,
    status: StatusSet,
    active_only: bool,
    porte: PorteSet,
    mei: Tri,
    simples: Tri,
    head_office: Tri,
    population: Option<(u32, u32)>,
    population_ref: Option<PopulationRef>,
}

pub type PlaceSets = (Option<BTreeSet<u16>>, Option<BTreeSet<u8>>);

/// Resolve a place filter to dense municipality and UF sets.
pub fn resolve_place(
    catalogue: &MunicipalityCatalogue,
    place: &PlaceFilter,
    within_ufs: &opencnpj_query::ir::SmallSet<opencnpj_query::ir::UfId>,
) -> Result<PlaceSets> {
    let mut municipalities: Option<BTreeSet<u16>> = None;
    let mut ufs: Option<BTreeSet<u8>> = None;
    match place {
        PlaceFilter::Any => {}
        PlaceFilter::Municipalities(ids) => municipalities = Some(ids.iter().map(|id| id.0).collect()),
        PlaceFilter::Ufs(ids) => ufs = Some(ids.iter().map(|id| id.0).collect()),
        PlaceFilter::Region(id) if (1..=5).contains(&id.0) => {
            ufs = Some(
                catalogue
                    .rows()
                    .iter()
                    .filter(|row| major_uf(row).and_then(major_region) == Some(id.0 as u8))
                    .filter_map(major_uf)
                    .collect(),
            );
        }
        PlaceFilter::Region(id) => {
            municipalities = Some(
                catalogue
                    .rows()
                    .iter()
                    .filter(|row| municipality(row).is_some_and(|geo| geo.intermediate_region == id.0))
                    .map(|row| row.id)
                    .collect(),
            );
        }
        PlaceFilter::Near { center, rule } => municipalities = Some(near(catalogue, *center, *rule)),
    }
    if !within_ufs.is_empty() {
        let allowed: BTreeSet<u8> = within_ufs.iter().map(|uf| uf.0).collect();
        match (&mut municipalities, &mut ufs) {
            (Some(rows), _) => rows.retain(|id| catalogue.get(*id).and_then(major_uf).is_some_and(|uf| allowed.contains(&uf))),
            (None, Some(values)) => values.retain(|uf| allowed.contains(uf)),
            (None, None) => ufs = Some(allowed),
        }
    }
    Ok((municipalities, ufs))
}

impl Filters {
    fn new(request: &ValidRequest, context: &ExecContext<'_>) -> Result<Self> {
        let scope = &request.scope;
        let (municipalities, ufs) = resolve_place(context.municipalities, &scope.place, &scope.within_ufs)?;
        let cnaes = match &scope.activity {
            ActivityFilter::Any => None,
            ActivityFilter::Sectors { cnaes, .. } => Some(cnaes.iter().filter_map(cnae_pattern).collect()),
            ActivityFilter::Cnae { codes, .. } => Some(codes.iter().filter_map(cnae_pattern).collect()),
        };
        let status = scope.status.unwrap_or_else(|| request.op.default_status());
        let population_ref = scope.place_population.map(|_| scope.population_ref.unwrap_or_else(|| population_reference(request, context)));
        Ok(Self {
            municipalities,
            ufs,
            cnaes,
            exclude_municipalities: scope.exclude.municipalities.iter().map(|id| id.0).collect(),
            exclude_ufs: scope.exclude.ufs.iter().map(|uf| uf.0).collect(),
            exclude_cnaes: scope
                .exclude
                .sector_cnaes
                .iter()
                .filter_map(|code| match code {
                    CnaeCode::Subclass(code) => Some(*code),
                    _ => None,
                })
                .collect(),
            status,
            active_only: status == StatusSet::only(Status::Active),
            porte: scope.porte,
            mei: scope.mei,
            simples: scope.simples,
            head_office: if request.unit == Unit::HeadOffices { Tri::Yes } else { scope.head_office },
            population: scope.place_population.map(|range| (range.from.unwrap_or(0), range.to.unwrap_or(u32::MAX))),
            population_ref,
        })
    }

    fn matches(&self, key: Key, context: &ExecContext<'_>) -> bool {
        if let Some(rows) = &self.municipalities
            && !rows.contains(&key.municipality.0)
        {
            return false;
        }
        if let Some(values) = &self.ufs
            && !values.contains(&key.uf.0)
        {
            return false;
        }
        if let Some(patterns) = &self.cnaes
            && !patterns.iter().any(|pattern| pattern.matches(key.cnae))
        {
            return false;
        }
        if self.exclude_municipalities.contains(&key.municipality.0)
            || self.exclude_ufs.contains(&key.uf.0)
            || self.exclude_cnaes.contains(&key.cnae)
        {
            return false;
        }
        if let Some((low, high)) = self.population {
            let Some(row) = context.municipalities.get(key.municipality.0) else { return false };
            let Some(population) = population_of(row, self.population_ref.unwrap_or_else(|| context.municipalities.default_population()))
            else {
                return false;
            };
            if !(u64::from(low)..=u64::from(high)).contains(&u64::from(population)) {
                return false;
            }
        }
        true
    }
}

fn cnae_pattern(code: &CnaeCode) -> Option<CnaePattern> {
    Some(match code {
        CnaeCode::Section(letter) => CnaePattern::Section(*letter),
        CnaeCode::Division(code) => CnaePattern::Exact { value: u32::from(*code), divisor: 100_000 },
        CnaeCode::Group(code) => CnaePattern::Exact { value: u32::from(*code), divisor: 10_000 },
        CnaeCode::Class(code) => CnaePattern::Exact { value: *code, divisor: 100 },
        CnaeCode::Subclass(code) => CnaePattern::Exact { value: *code, divisor: 1 },
    })
}

fn section_of(cnae: u32) -> u8 {
    match cnae / 100_000 {
        1..=3 => b'A',
        5..=9 => b'B',
        10..=33 => b'C',
        35 => b'D',
        36..=39 => b'E',
        41..=43 => b'F',
        45..=47 => b'G',
        49..=53 => b'H',
        55..=56 => b'I',
        58..=63 => b'J',
        64..=66 => b'K',
        68 => b'L',
        69..=75 => b'M',
        77..=82 => b'N',
        84 => b'O',
        85 => b'P',
        86..=88 => b'Q',
        90..=93 => b'R',
        94..=96 => b'S',
        97 => b'T',
        99 => b'U',
        // Divisions that do not exist in CNAE 2.x.
        _ => b'?',
    }
}

fn municipality(row: &Municipality) -> Option<&Geography> {
    match &row.place {
        Place::Municipality(geo) => Some(geo),
        Place::Abroad => None,
    }
}

pub fn major_uf(row: &Municipality) -> Option<u8> {
    agg::uf_id(&municipality(row)?.uf).map(|uf| uf.0)
}

pub fn major_region(uf: u8) -> Option<u8> {
    Some(match agg::uf_code(opencnpj_query::ir::UfId(uf))? {
        "AC" | "AP" | "AM" | "PA" | "RO" | "RR" | "TO" => 1,
        "AL" | "BA" | "CE" | "MA" | "PB" | "PE" | "PI" | "RN" | "SE" => 2,
        "ES" | "MG" | "RJ" | "SP" => 3,
        "PR" | "RS" | "SC" => 4,
        "DF" | "GO" | "MT" | "MS" => 5,
        _ => return None,
    })
}

fn near(catalogue: &MunicipalityCatalogue, center: opencnpj_query::ir::MunicipalityId, rule: NearRule) -> BTreeSet<u16> {
    let mut selected = BTreeSet::from([center.0]);
    match rule {
        NearRule::Neighbours(depth) => {
            let mut frontier = vec![center.0];
            for _ in 0..depth {
                let mut next = Vec::new();
                for id in frontier {
                    let Some(geo) = catalogue.get(id).and_then(municipality) else { continue };
                    for &neighbour in &geo.neighbours {
                        if selected.insert(neighbour) {
                            next.push(neighbour);
                        }
                    }
                }
                frontier = next;
            }
        }
        NearRule::WithinKm(km) => {
            let Some(center_row) = catalogue.get(center.0).and_then(municipality) else { return selected };
            for row in catalogue.rows() {
                let Some(geo) = municipality(row) else { continue };
                if haversine(center_row, geo) <= f64::from(km) {
                    selected.insert(row.id);
                }
            }
        }
    }
    selected
}

fn haversine(a: &Geography, b: &Geography) -> f64 {
    const EARTH_KM: f64 = 6_371.0;
    let (lat1, lat2) = (a.latitude.to_radians(), b.latitude.to_radians());
    let dlat = (b.latitude - a.latitude).to_radians();
    let dlon = (b.longitude - a.longitude).to_radians();
    let h = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * EARTH_KM * h.sqrt().asin()
}

/// Population of the exact reference asked for. A year the catalogue does not hold has no
/// population (answers used to say "estimativa 2025" while dividing by the 2026 estimate).
pub fn population_of(row: &Municipality, reference: PopulationRef) -> Option<u32> {
    match (&row.place, reference) {
        (Place::Municipality(geo), PopulationRef::IbgeCensus(2022)) => geo.census_2022,
        (Place::Municipality(geo), PopulationRef::IbgeEstimate(year)) if year == geo.estimate_year => Some(geo.population_estimate),
        _ => None,
    }
}

pub fn place_population_for(
    catalogue: &MunicipalityCatalogue,
    municipalities: Option<&BTreeSet<u16>>,
    ufs: Option<&BTreeSet<u8>>,
    range: Option<(u32, u32)>,
    reference: PopulationRef,
) -> u64 {
    catalogue
        .rows()
        .iter()
        .filter(|row| {
            if let Some(values) = ufs
                && !major_uf(row).is_some_and(|uf| values.contains(&uf))
            {
                return false;
            }
            if let Some(values) = municipalities
                && !values.contains(&row.id)
            {
                return false;
            }
            if let Some((low, high)) = range {
                let Some(value) = population_of(row, reference) else { return false };
                if !(u64::from(low)..=u64::from(high)).contains(&u64::from(value)) {
                    return false;
                }
            }
            true
        })
        .filter_map(|row| population_of(row, reference))
        .map(u64::from)
        .sum()
}

fn place_population(filters: &Filters, context: &ExecContext<'_>, reference: PopulationRef) -> u64 {
    place_population_for(context.municipalities, filters.municipalities.as_ref(), filters.ufs.as_ref(), filters.population, reference)
}

fn status_of(code: u8) -> Option<Status> {
    Some(match code {
        1 => Status::Null,
        2 => Status::Active,
        3 => Status::Suspended,
        4 => Status::Unfit,
        5 => Status::Closed,
        _ => return None,
    })
}

fn tri_matches(tri: Tri, code: u8, yes: u8, no: u8) -> bool {
    match tri {
        Tri::Any => true,
        Tri::Yes => code == yes,
        Tri::No => code == no,
    }
}

fn head_code(tri: Tri) -> Option<u8> {
    match tri {
        Tri::Any => None,
        Tri::Yes => Some(1),
        Tri::No => Some(2),
    }
}

fn porte_of(code: u8) -> Porte {
    match code {
        1 => Porte::Unreported,
        2 => Porte::Micro,
        3 => Porte::Small,
        _ => Porte::Other,
    }
}

fn stock_table(filters: &Filters) -> Table {
    if filters.active_only { Table::T1 } else { Table::T2 }
}

fn stock_matches(table: Table, key: Key, filters: &Filters, context: &ExecContext<'_>) -> bool {
    if !filters.matches(key, context) {
        return false;
    }
    match table {
        Table::T1 => {
            let detail = ActiveDetail::unpack(key.detail);
            let head = head_code(filters.head_office);
            head.is_none_or(|head| detail.head_office == head)
                && (filters.porte == PorteSet::ALL || filters.porte.contains(porte_of(detail.porte)))
                && tri_matches(filters.mei, detail.mei, 2, 1)
                && tri_matches(filters.simples, detail.simples, 2, 1)
        }
        Table::T2 => {
            let (head, status) = unpack_stock_detail(key.detail);
            head_code(filters.head_office).is_none_or(|value| head == value)
                && status_of(status).is_some_and(|status| filters.status.contains(status))
        }
        _ => false,
    }
}

fn for_each_cell(aggregate: &Aggregate, table: Table, filters: &Filters, mut visit: impl FnMut(Key, u64) -> Result<()>) -> Result<()> {
    let exact = filters
        .cnaes
        .as_ref()
        .filter(|patterns| patterns.len() <= 8 && patterns.iter().all(|pattern| matches!(pattern, CnaePattern::Exact { divisor: 1, .. })));
    if let Some(patterns) = exact {
        for pattern in patterns {
            let CnaePattern::Exact { value, .. } = pattern else { unreachable!() };
            for cell in aggregate.cells_for_cnae(table, *value)? {
                let cell = cell?;
                visit(Key::unpack(cell.key), cell.count)?;
            }
        }
    } else {
        for cell in aggregate.cells(Section::Cells(table)) {
            let cell = cell?;
            visit(Key::unpack(cell.key), cell.count)?;
        }
    }
    Ok(())
}

fn stock_count(aggregate: &Aggregate, filters: &Filters, context: &ExecContext<'_>) -> Result<u64> {
    let table = stock_table(filters);
    let mut total = 0;
    for_each_cell(aggregate, table, filters, |key, count| {
        if stock_matches(table, key, filters, context) {
            total += count;
        }
        Ok(())
    })?;
    Ok(total)
}

fn flow_count(
    aggregate: &Aggregate,
    filters: &Filters,
    period: opencnpj_query::ir::DayRange,
    flow: FlowMetric,
    context: &ExecContext<'_>,
) -> Result<i64> {
    let mut total = 0;
    for (table, sign) in flow_tables(flow) {
        for_each_cell(aggregate, table, filters, |key, count| {
            if filters.matches(key, context) && bucket_in(key.detail, period) {
                total += sign * i64::try_from(count)?;
            }
            Ok(())
        })?;
    }
    Ok(total)
}

fn flow_tables(flow: FlowMetric) -> Vec<(Table, i64)> {
    match flow {
        FlowMetric::Registrations => vec![(Table::T3, 1)],
        FlowMetric::Closures => vec![(Table::T4, 1)],
        FlowMetric::NetRegistrations => vec![(Table::T3, 1), (Table::T4, -1)],
        FlowMetric::MeiOptIns | FlowMetric::MeiExits => Vec::new(),
    }
}

fn bucket_in(bucket: u32, period: opencnpj_query::ir::DayRange) -> bool {
    let (Some(from), Some(to)) = (period.from, period.to) else { return false };
    flow_bounds(bucket).is_some_and(|(start, end)| start >= from && end <= to)
}

/// The stock table of the competence the request asks about. Stock answers for a past
/// competence ("ativas em dezembro de 2025") must come from that month's aggregates, never
/// from the current ones; flows always come from the current artifact (09-DATA-ASSETS.md §4).
fn stock_aggregate<'a>(request: &ValidRequest, aggregates: &'a AggregateSet) -> Result<&'a Aggregate> {
    let competence = request.time.competence;
    aggregates.get(competence).with_context(|| format!("no aggregates for competence {competence}"))
}

fn metric_count(
    request: &ValidRequest,
    filters: &Filters,
    metric: Metric,
    aggregates: &AggregateSet,
    context: &ExecContext<'_>,
) -> Result<i64> {
    match metric {
        Metric::Stock | Metric::Share => Ok(i64::try_from(stock_count(stock_aggregate(request, aggregates)?, filters, context)?)?),
        Metric::Flow(flow) => {
            let Some(Period::Days(period)) = request.time.period else { bail!("flow needs a day period") };
            flow_count(aggregates.current(), filters, period, flow, context)
        }
        Metric::ActiveChange { .. } => {
            let Some(Period::Competences { from, to }) = request.time.period else { bail!("active change needs two competences") };
            let before = stock_count(aggregates.get(from).context("missing first aggregate")?, filters, context)?;
            let after = stock_count(aggregates.get(to).context("missing second aggregate")?, filters, context)?;
            Ok(i64::try_from(after)? - i64::try_from(before)?)
        }
        Metric::Per10k(_) | Metric::InhabitantsPer(_) => bail!("ratio metric needs a place dimension"),
    }
}

fn scalar(
    request: &ValidRequest,
    filters: &Filters,
    metric: Metric,
    aggregates: &AggregateSet,
    context: &ExecContext<'_>,
) -> Result<Value> {
    match metric {
        Metric::Stock | Metric::Flow(_) | Metric::ActiveChange { .. } => {
            Ok(Value::Scalar { value: metric_count(request, filters, metric, aggregates, context)? })
        }
        Metric::Per10k(reference) => {
            let count = metric_count(request, filters, Metric::Stock, aggregates, context)?;
            Ok(Value::Ratio { num: u64::try_from(count)?, den: place_population(filters, context, reference) })
        }
        Metric::InhabitantsPer(reference) => {
            let count = metric_count(request, filters, Metric::Stock, aggregates, context)?;
            Ok(Value::Ratio { num: place_population(filters, context, reference), den: u64::try_from(count)? })
        }
        Metric::Share => bail!("share needs a breakdown, ranking or comparison"),
    }
}

/// Group keys of one cell. A dimension the table does not carry is an error, never a guess:
/// the flow tables' detail field is a month bucket, not a status or head-office code.
fn group_keys(dim: Dim, table: Table, key: Key, context: &ExecContext<'_>) -> Result<Vec<u64>> {
    let stock = matches!(table, Table::T1 | Table::T2);
    match dim {
        Dim::Status | Dim::HeadOffice if !stock => bail!("{table:?} has no {dim:?} dimension"),
        Dim::Porte | Dim::Mei if table != Table::T1 => bail!("{table:?} has no {dim:?} dimension"),
        Dim::Year | Dim::Month => bail!("{dim:?} groups come from the flow buckets"),
        _ => {}
    }
    Ok(match dim {
        Dim::Municipality => (key.municipality.0 != MUNICIPALITY_MISSING).then_some(u64::from(key.municipality.0)).into_iter().collect(),
        Dim::Uf => (key.uf.0 != UF_MISSING).then_some(u64::from(key.uf.0)).into_iter().collect(),
        Dim::Region(level) => context
            .municipalities
            .get(key.municipality.0)
            .and_then(municipality)
            .and_then(|geo| match level {
                RegionLevel::Major => major_uf(context.municipalities.get(key.municipality.0)?).and_then(major_region).map(u64::from),
                RegionLevel::Intermediate => Some(u64::from(geo.intermediate_region)),
                RegionLevel::Immediate => Some(u64::from(geo.immediate_region)),
            })
            .into_iter()
            .collect(),
        Dim::Sector => {
            context.cnae_sectors.get(&key.cnae).map(|sectors| sectors.iter().map(|id| u64::from(*id)).collect()).unwrap_or_default()
        }
        Dim::Cnae(level) => vec![match level {
            CnaeLevel::Section => u64::from(section_of(key.cnae)),
            CnaeLevel::Division => u64::from(key.cnae / 100_000),
            CnaeLevel::Group => u64::from(key.cnae / 10_000),
            CnaeLevel::Class => u64::from(key.cnae / 100),
            CnaeLevel::Subclass => u64::from(key.cnae),
        }],
        Dim::Porte if table == Table::T1 => {
            let code = ActiveDetail::unpack(key.detail).porte;
            (1..=4).contains(&code).then_some(u64::from(code - 1)).into_iter().collect()
        }
        Dim::Status => {
            let code = if table == Table::T1 { 2 } else { unpack_stock_detail(key.detail).1 };
            (1..=5).contains(&code).then_some(u64::from(code)).into_iter().collect()
        }
        Dim::HeadOffice => {
            let code = if table == Table::T1 { ActiveDetail::unpack(key.detail).head_office } else { unpack_stock_detail(key.detail).0 };
            (1..=2).contains(&code).then_some(u64::from(code - 1)).into_iter().collect()
        }
        Dim::Mei if table == Table::T1 => {
            let code = ActiveDetail::unpack(key.detail).mei;
            (1..=2).contains(&code).then_some(u64::from(code - 1)).into_iter().collect()
        }
        Dim::Year | Dim::Month | Dim::Porte | Dim::Mei => unreachable!("rejected above"),
    })
}

fn group_stock(aggregate: &Aggregate, filters: &Filters, dim: Dim, context: &ExecContext<'_>) -> Result<BTreeMap<u64, i64>> {
    let table = stock_table(filters);
    let mut map = BTreeMap::new();
    for_each_cell(aggregate, table, filters, |key, count| {
        if stock_matches(table, key, filters, context) {
            for group in group_keys(dim, table, key, context)? {
                *map.entry(group).or_default() += i64::try_from(count)?;
            }
        }
        Ok(())
    })?;
    Ok(map)
}

fn group_flow(
    aggregate: &Aggregate,
    filters: &Filters,
    period: opencnpj_query::ir::DayRange,
    flow: FlowMetric,
    dim: Dim,
    grain: Option<Grain>,
    context: &ExecContext<'_>,
) -> Result<BTreeMap<u64, i64>> {
    let mut map = BTreeMap::new();
    for (table, sign) in flow_tables(flow) {
        for_each_cell(aggregate, table, filters, |key, count| {
            if !filters.matches(key, context) || !bucket_in(key.detail, period) {
                return Ok(());
            }
            let groups = match grain {
                Some(grain) => flow_group_key(grain, key.detail).into_iter().collect(),
                None => group_keys(dim, table, key, context)?,
            };
            for group in groups {
                *map.entry(group).or_default() += sign * i64::try_from(count)?;
            }
            Ok(())
        })?;
    }
    Ok(map)
}

fn flow_group_key(grain: Grain, detail: u32) -> Option<u64> {
    let (start, _) = flow_bounds(detail)?;
    let (year, month, _) = start.to_ymd()?;
    Some(match grain {
        Grain::Month => u64::from(year) * 12 + u64::from(month) - 1,
        Grain::Year => u64::from(year),
    })
}

fn group_active_change(
    request: &ValidRequest,
    filters: &Filters,
    dim: Dim,
    aggregates: &AggregateSet,
    context: &ExecContext<'_>,
) -> Result<(BTreeMap<u64, i64>, BTreeMap<u64, u64>)> {
    let Some(Period::Competences { from, to }) = request.time.period else { bail!("active change needs two competences") };
    let before = group_stock(aggregates.get(from).context("missing first aggregate")?, filters, dim, context)?;
    let after = group_stock(aggregates.get(to).context("missing second aggregate")?, filters, dim, context)?;
    let mut values = BTreeMap::new();
    let mut bases = BTreeMap::new();
    for key in before.keys().chain(after.keys()).copied().collect::<BTreeSet<_>>() {
        let start = before.get(&key).copied().unwrap_or(0);
        let end = after.get(&key).copied().unwrap_or(0);
        values.insert(key, end - start);
        bases.insert(key, u64::try_from(start.max(0))?);
    }
    Ok((values, bases))
}

fn metric_rows(
    request: &ValidRequest,
    filters: &Filters,
    metric: Metric,
    dim: Dim,
    aggregates: &AggregateSet,
    context: &ExecContext<'_>,
) -> Result<(BTreeMap<u64, i64>, BTreeMap<u64, u64>)> {
    match metric {
        Metric::Stock | Metric::Share | Metric::Per10k(_) | Metric::InhabitantsPer(_) => {
            let counts = group_stock(stock_aggregate(request, aggregates)?, filters, dim, context)?;
            let (values, bases) = ratio_rows(request, filters, metric, dim, counts, aggregates, context)?;
            Ok((values, bases))
        }
        Metric::Flow(flow) => {
            let Some(Period::Days(period)) = request.time.period else { bail!("flow needs a day period") };
            Ok((group_flow(aggregates.current(), filters, period, flow, dim, None, context)?, BTreeMap::new()))
        }
        Metric::ActiveChange { .. } => group_active_change(request, filters, dim, aggregates, context),
    }
}

fn ratio_rows(
    request: &ValidRequest,
    filters: &Filters,
    metric: Metric,
    dim: Dim,
    counts: BTreeMap<u64, i64>,
    aggregates: &AggregateSet,
    context: &ExecContext<'_>,
) -> Result<(BTreeMap<u64, i64>, BTreeMap<u64, u64>)> {
    let mut values = counts;
    let mut bases = BTreeMap::new();
    match metric {
        Metric::Per10k(reference) => {
            for &key in values.keys() {
                bases.insert(key, member_population(key, dim, filters, context, reference));
            }
        }
        Metric::InhabitantsPer(reference) => {
            for key in values.keys().copied().collect::<Vec<_>>() {
                let count = u64::try_from(values[&key].max(0))?;
                let population = member_population(key, dim, filters, context, reference);
                bases.insert(key, count);
                values.insert(key, i64::try_from(population)?);
            }
        }
        Metric::Share => {
            let total = metric_count(request, filters, Metric::Stock, aggregates, context)?;
            let total = u64::try_from(total.max(0))?;
            for &key in values.keys() {
                bases.insert(key, total);
            }
        }
        _ => {}
    }
    Ok((values, bases))
}

pub fn member_population_for(
    catalogue: &MunicipalityCatalogue,
    key: u64,
    dim: Dim,
    municipalities: Option<&BTreeSet<u16>>,
    ufs: Option<&BTreeSet<u8>>,
    reference: PopulationRef,
) -> u64 {
    match dim {
        Dim::Municipality => catalogue.get(key as u16).and_then(|row| population_of(row, reference)).map(u64::from).unwrap_or(0),
        Dim::Uf => catalogue
            .rows()
            .iter()
            .filter(|row| major_uf(row) == Some(key as u8))
            .filter_map(|row| population_of(row, reference))
            .map(u64::from)
            .sum(),
        Dim::Region(level) => catalogue
            .rows()
            .iter()
            .filter(|row| {
                municipality(row).is_some_and(|geo| match level {
                    RegionLevel::Major => major_uf(row).and_then(major_region).map(u64::from) == Some(key),
                    RegionLevel::Intermediate => u64::from(geo.intermediate_region) == key,
                    RegionLevel::Immediate => u64::from(geo.immediate_region) == key,
                })
            })
            .filter_map(|row| population_of(row, reference))
            .map(u64::from)
            .sum(),
        _ => place_population_for(catalogue, municipalities, ufs, None, reference),
    }
}

fn member_population(key: u64, dim: Dim, filters: &Filters, context: &ExecContext<'_>, reference: PopulationRef) -> u64 {
    member_population_for(context.municipalities, key, dim, filters.municipalities.as_ref(), filters.ufs.as_ref(), reference)
}

fn population_reference(request: &ValidRequest, context: &ExecContext<'_>) -> PopulationRef {
    match request.op.metric() {
        Some(Metric::Per10k(reference) | Metric::InhabitantsPer(reference)) => reference,
        _ => request.scope.population_ref.unwrap_or_else(|| context.municipalities.default_population()),
    }
}

fn member_key(member: &Member) -> u64 {
    match member {
        Member::Municipality(id) => u64::from(id.0),
        Member::Uf(id) => u64::from(id.0),
        Member::Region(id) => u64::from(id.0),
        Member::Sector(id) => u64::from(id.0),
    }
}

fn compare(
    request: &ValidRequest,
    filters: &Filters,
    metric: Metric,
    dim: Dim,
    members: &opencnpj_query::ir::Members,
    aggregates: &AggregateSet,
    context: &ExecContext<'_>,
) -> Result<Value> {
    let (values, bases) = metric_rows(request, filters, metric, dim, aggregates, context)?;
    let mut rows = Vec::with_capacity(members.len());
    for member in members.iter() {
        let key = member_key(member);
        let value = values.get(&key).copied().unwrap_or(0);
        let base = match bases.get(&key) {
            Some(base) => *base,
            None if matches!(metric, Metric::Per10k(_) | Metric::InhabitantsPer(_)) => {
                member_population(key, dim, filters, context, population_reference(request, context))
            }
            None => 0,
        };
        rows.push(Row { key, value, base });
    }
    Ok(Value::Rows(rows))
}

fn breakdown(
    request: &ValidRequest,
    filters: &Filters,
    metric: Metric,
    dim: Dim,
    aggregates: &AggregateSet,
    context: &ExecContext<'_>,
) -> Result<Value> {
    let (values, bases) = metric_rows(request, filters, metric, dim, aggregates, context)?;
    let rows = values.into_iter().map(|(key, value)| Row { key, value, base: bases.get(&key).copied().unwrap_or(0) }).collect();
    Ok(Value::Rows(rows))
}

#[derive(Debug, Clone, Copy)]
struct RankSpec {
    dim: Dim,
    dir: Dir,
    k: u8,
    min_base: u32,
}

fn rank(
    request: &ValidRequest,
    filters: &Filters,
    metric: Metric,
    spec: RankSpec,
    aggregates: &AggregateSet,
    context: &ExecContext<'_>,
) -> Result<Value> {
    let RankSpec { dim, dir, k, min_base } = spec;
    let (values, bases) = metric_rows(request, filters, metric, dim, aggregates, context)?;
    let mut rows: Vec<Row> =
        values.into_iter().map(|(key, value)| Row { key, value, base: bases.get(&key).copied().unwrap_or(0) }).collect();
    match metric {
        Metric::ActiveChange { pct: true } => rows.retain(|row| row.base >= u64::from(min_base)),
        // Members without population (EX, abroad, missing IBGE rows) have no density: ranking
        // them with a substituted denominator put "abroad" first with an infinite value.
        Metric::Per10k(_) => rows.retain(|row| row.base > 0),
        Metric::InhabitantsPer(_) => rows.retain(|row| row.base > 0 && row.value > 0),
        _ => {}
    }
    let ratio = matches!(metric, Metric::Per10k(_) | Metric::InhabitantsPer(_) | Metric::Share | Metric::ActiveChange { pct: true });
    rows.sort_by(|a, b| {
        let primary = if ratio { compare_ratio(*a, *b) } else { a.value.cmp(&b.value) };
        let primary = match dir {
            Dir::Asc => primary,
            Dir::Desc => primary.reverse(),
        };
        primary.then_with(|| a.key.cmp(&b.key))
    });
    rows.truncate(usize::from(k));
    Ok(Value::Rows(rows))
}

fn compare_ratio(a: Row, b: Row) -> std::cmp::Ordering {
    let left = i128::from(a.value) * i128::from(b.base.max(1));
    let right = i128::from(b.value) * i128::from(a.base.max(1));
    left.cmp(&right)
}

fn trend(request: &ValidRequest, filters: &Filters, grain: Grain, aggregates: &AggregateSet, context: &ExecContext<'_>) -> Result<Value> {
    let Some(Metric::Flow(flow)) = request.op.metric() else { bail!("trend needs a flow metric") };
    let Some(Period::Days(period)) = request.time.period else { bail!("trend needs a day period") };
    let values = group_flow(aggregates.current(), filters, period, flow, Dim::Month, Some(grain), context)?;
    Ok(Value::Rows(values.into_iter().map(|(key, value)| Row { key, value, base: 0 }).collect()))
}

#[cfg(test)]
mod tests {
    use super::section_of;

    /// CNAE 2.x sections as the engine defines them (`opencnpj-store/src/search.rs`,
    /// `cnae_section`), written letter → divisions so the check is independent of `section_of`.
    const SECTIONS: [(u8, u32, u32); 21] = [
        (b'A', 1, 3),
        (b'B', 5, 9),
        (b'C', 10, 33),
        (b'D', 35, 35),
        (b'E', 36, 39),
        (b'F', 41, 43),
        (b'G', 45, 47),
        (b'H', 49, 53),
        (b'I', 55, 56),
        (b'J', 58, 63),
        (b'K', 64, 66),
        (b'L', 68, 68),
        (b'M', 69, 75),
        (b'N', 77, 82),
        (b'O', 84, 84),
        (b'P', 85, 85),
        (b'Q', 86, 88),
        (b'R', 90, 93),
        (b'S', 94, 96),
        (b'T', 97, 97),
        (b'U', 99, 99),
    ];

    #[test]
    fn sections_match_the_engine() {
        for division in 1..=99u32 {
            let expected = SECTIONS.iter().find(|(_, a, b)| (*a..=*b).contains(&division)).map_or(b'?', |(letter, _, _)| *letter);
            assert_eq!(section_of(division * 100_000 + 1), expected, "division {division}");
        }
        // Restaurants (5611-2/01) are lodging and food (I), not information (J).
        assert_eq!(section_of(5_611_201), b'I');
    }
}
