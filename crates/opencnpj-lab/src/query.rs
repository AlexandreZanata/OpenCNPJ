//! Query IR lowering and in-process engine execution.

use std::{collections::BTreeMap, path::Path};

use anyhow::{Context, Result, bail, ensure};
use opencnpj::{params, params::Caps};
use opencnpj_ingest::source::sha256_file;
use opencnpj_insights::{
    agg,
    catalog::municipalities::MunicipalityCatalogue,
    execute::{AggregateSet, ExecContext, Row, SectorMap, Value, resolve_place},
};
use opencnpj_query::{
    ir::*,
    validate::{self, Capabilities, ValidRequest},
};
use opencnpj_store::{
    Snapshot,
    numeric::NumField,
    postings::Field,
    search::{self, Limits, Query},
};
use roaring::RoaringBitmap;

pub fn parse(pairs: &[(String, String)]) -> Result<Query> {
    params::parse(pairs, Caps { max_limit: 10_000, max_offset: usize::MAX }).map_err(|error| anyhow::anyhow!(error))
}

/// Lower a request to the HTTP/CLI parameter grammar.
pub fn lower(request: &Request, catalogue: &MunicipalityCatalogue) -> Result<Vec<(String, String)>> {
    let scope = &request.scope;
    let mut pairs = Vec::new();
    let head = match request.unit {
        Unit::HeadOffices => "1".to_string(),
        Unit::Establishments => match scope.head_office {
            Tri::Any => "1,2".to_string(),
            Tri::Yes => "1".to_string(),
            Tri::No => "2".to_string(),
        },
    };
    pairs.push(("matriz_filial".into(), head));
    let (municipalities, ufs) = resolve_place(catalogue, &scope.place, &scope.within_ufs)?;
    if let Some(rows) = &municipalities {
        let toms = rows.iter().filter_map(|id| catalogue.get(*id).map(|row| format!("{:04}", row.tom))).collect::<Vec<_>>();
        if toms.is_empty() {
            bail!("place resolves to no municipality");
        }
        pairs.push(("municipio".into(), toms.join(",")));
    }
    if let Some(values) = &ufs {
        let codes = values.iter().filter_map(|uf| agg::uf_code(UfId(*uf)).map(str::to_string)).collect::<Vec<_>>();
        if codes.is_empty() {
            bail!("place resolves to no UF");
        }
        pairs.push(("uf".into(), codes.join(",")));
    }
    match &scope.activity {
        ActivityFilter::Any => {}
        ActivityFilter::Sectors { cnaes, scope: CnaeScope::Main, .. } | ActivityFilter::Cnae { codes: cnaes, scope: CnaeScope::Main } => {
            let values = cnae_values(cnaes);
            if values.is_empty() {
                bail!("activity resolves to no CNAE");
            }
            pairs.push(("cnae".into(), values.join(",")));
        }
        ActivityFilter::Sectors { cnaes, scope: CnaeScope::Any, .. } | ActivityFilter::Cnae { codes: cnaes, scope: CnaeScope::Any } => {
            let values = cnae_values(cnaes);
            if values.is_empty() {
                bail!("activity resolves to no CNAE");
            }
            pairs.push(("cnae_qualquer".into(), values.join(",")));
        }
    }
    let status = scope.status.unwrap_or_else(|| request.op.default_status());
    if status != StatusSet::ALL {
        let values = status.iter().map(status_code).collect::<Vec<_>>();
        if !values.is_empty() {
            pairs.push(("situacao".into(), values.join(",")));
        }
    }
    if scope.porte != PorteSet::ALL {
        let values = scope
            .porte
            .iter()
            .map(|porte| match porte {
                Porte::Unreported => "00",
                Porte::Micro => "01",
                Porte::Small => "03",
                Porte::Other => "05",
            })
            .collect::<Vec<_>>();
        pairs.push(("porte".into(), values.join(",")));
    }
    if scope.mei != Tri::Any {
        pairs.push(("mei".into(), tri_code(scope.mei)));
    }
    if scope.simples != Tri::Any {
        pairs.push(("simples".into(), tri_code(scope.simples)));
    }
    if let Some(range) = scope.capital {
        if let Some(from) = range.from {
            pairs.push(("capital_social_de".into(), money(from)));
        }
        if let Some(to) = range.to {
            pairs.push(("capital_social_ate".into(), money(to)));
        }
    }
    if !scope.natureza.is_empty() {
        let values = scope
            .natureza
            .iter()
            .map(|code| match code {
                NaturezaCode::Group(group) => group.to_string(),
                NaturezaCode::Code(code) => format!("{code:04}"),
            })
            .collect::<Vec<_>>();
        pairs.push(("natureza".into(), values.join(",")));
    }
    if let Some(range) = scope.opened {
        add_days(&mut pairs, "data_inicio_atividade", range);
    }
    if let Some(range) = scope.status_date {
        add_days(&mut pairs, "data_situacao_cadastral", range);
    }
    match scope.contact {
        ContactNeed::Any => {}
        ContactNeed::Phone => pairs.push(("tem_telefone".into(), "true".into())),
        ContactNeed::Email => pairs.push(("tem_email".into(), "true".into())),
        ContactNeed::Both => {
            pairs.push(("tem_email".into(), "true".into()));
            pairs.push(("tem_telefone".into(), "true".into()));
        }
    }
    if let Some(Metric::Flow(flow)) = request.op.metric() {
        add_flow(&mut pairs, request, flow)?;
    }
    match &request.op {
        Op::List { limit, order } => {
            pairs.push(("limit".into(), limit.to_string()));
            let offset = usize::from(request.output.page.saturating_sub(1)) * usize::from(*limit);
            if offset > 0 {
                pairs.push(("offset".into(), offset.to_string()));
            }
            if let Some(sort) = list_sort(*order) {
                pairs.push(("ordenar".into(), sort.into()));
            }
        }
        Op::Lookup { id } => {
            pairs.push(("cnpj".into(), id.as_str().into()));
            pairs.push(("limit".into(), "1".into()));
        }
        Op::Meta { .. } => {}
        _ => pairs.push(("limit".into(), "1".into())),
    }
    Ok(pairs)
}

fn add_flow(pairs: &mut Vec<(String, String)>, request: &Request, flow: FlowMetric) -> Result<()> {
    let Some(Period::Days(days)) = request.time.period else { bail!("flow needs a day period") };
    for field in flow_fields(flow) {
        add_days(pairs, field, days);
    }
    Ok(())
}

fn flow_fields(flow: FlowMetric) -> Vec<&'static str> {
    match flow {
        FlowMetric::Registrations => vec!["data_inicio_atividade"],
        FlowMetric::Closures => vec!["data_situacao_cadastral"],
        FlowMetric::NetRegistrations => vec!["data_inicio_atividade", "data_situacao_cadastral"],
        FlowMetric::MeiOptIns => vec!["data_opcao_mei"],
        FlowMetric::MeiExits => vec!["data_exclusao_mei"],
    }
}

fn add_days(pairs: &mut Vec<(String, String)>, field: &str, range: DayRange) {
    if let Some(from) = range.from {
        pairs.push((format!("{field}_de"), from.to_string()));
    }
    if let Some(to) = range.to {
        pairs.push((format!("{field}_ate"), to.to_string()));
    }
}

fn cnae_values(codes: &SmallSet<CnaeCode>) -> Vec<String> {
    codes
        .iter()
        .map(|code| match code {
            CnaeCode::Section(letter) => char::from(*letter).to_string(),
            CnaeCode::Division(code) => format!("{code:02}"),
            CnaeCode::Group(code) => format!("{code:03}"),
            CnaeCode::Class(code) => format!("{code:05}"),
            CnaeCode::Subclass(code) => format!("{code:07}"),
        })
        .collect()
}

fn status_code(status: Status) -> &'static str {
    match status {
        Status::Null => "01",
        Status::Active => "02",
        Status::Suspended => "03",
        Status::Unfit => "04",
        Status::Closed => "08",
    }
}

fn tri_code(tri: Tri) -> String {
    match tri {
        Tri::Any => String::new(),
        Tri::Yes => "S".into(),
        Tri::No => "N".into(),
    }
}

fn money(cents: u64) -> String {
    format!("{},{:02}", cents / 100, cents % 100)
}

fn list_sort(order: ListOrder) -> Option<&'static str> {
    match order {
        ListOrder::Cnpj => None,
        ListOrder::NewestFirst => Some("data_inicio_atividade:desc"),
        ListOrder::OldestFirst => Some("data_inicio_atividade:asc"),
        ListOrder::CapitalDesc => Some("capital_social:desc"),
    }
}

/// In-process engine executor used as the differential oracle.
pub struct EngineExecutor<'a> {
    pub snapshot: &'a Snapshot,
    pub context: &'a ExecContext<'a>,
    pub limits: Limits,
}

impl EngineExecutor<'_> {
    pub fn execute(&self, request: &ValidRequest) -> Result<Value> {
        let request = request.as_request();
        match &request.op {
            Op::Count { .. } => self.count(request),
            Op::Compare { dim, members, .. } => {
                let dim = *dim;
                self.compare(request, dim, members)
            }
            Op::Rank { dim, dir, k, .. } => {
                let (dim, dir, k) = (*dim, *dir, *k);
                self.rank(request, dim, dir, k)
            }
            Op::Breakdown { dim, .. } => {
                let dim = *dim;
                self.breakdown(request, dim)
            }
            Op::Trend { grain, .. } => {
                let grain = *grain;
                self.trend(request, grain)
            }
            _ => bail!("engine executor does not serve this operation"),
        }
    }

    fn count(&self, request: &Request) -> Result<Value> {
        let metric = request.op.metric().context("operation metric")?;
        match metric {
            Metric::Stock | Metric::Per10k(_) | Metric::InhabitantsPer(_) => {
                let set = self.set(request)?;
                let count = self.filtered_count(request, &set)?;
                Ok(match metric {
                    Metric::Per10k(reference) => Value::Ratio { num: count, den: self.place_population(request, reference) },
                    Metric::InhabitantsPer(reference) => Value::Ratio { num: self.place_population(request, reference), den: count },
                    _ => Value::Scalar { value: i64::try_from(count)? },
                })
            }
            Metric::Flow(flow) => Ok(Value::Scalar { value: self.flow_count(request, flow)? }),
            Metric::Share => bail!("share needs a breakdown, ranking or comparison"),
            Metric::ActiveChange { .. } => bail!("engine has no historical snapshots"),
        }
    }

    fn compare(&self, request: &Request, dim: Dim, members: &Members) -> Result<Value> {
        let metric = request.op.metric().context("operation metric")?;
        let (values, bases) = self.metric_rows(request, metric, dim)?;
        let mut rows = Vec::with_capacity(members.len());
        for member in members.iter() {
            let key = member_key(member);
            let value = values.get(&key).copied().unwrap_or(0);
            let base = match bases.get(&key) {
                Some(base) => *base,
                None if matches!(metric, Metric::Per10k(_) | Metric::InhabitantsPer(_)) => {
                    self.member_population(request, key, dim, metric)
                }
                None => 0,
            };
            rows.push(Row { key, value, base });
        }
        Ok(Value::Rows(rows))
    }

    fn rank(&self, request: &Request, dim: Dim, dir: Dir, k: u8) -> Result<Value> {
        let metric = request.op.metric().context("operation metric")?;
        let (values, bases) = self.metric_rows(request, metric, dim)?;
        let mut rows: Vec<Row> =
            values.into_iter().map(|(key, value)| Row { key, value, base: bases.get(&key).copied().unwrap_or(0) }).collect();
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

    fn breakdown(&self, request: &Request, dim: Dim) -> Result<Value> {
        let metric = request.op.metric().context("operation metric")?;
        let (values, bases) = self.metric_rows(request, metric, dim)?;
        Ok(Value::Rows(values.into_iter().map(|(key, value)| Row { key, value, base: bases.get(&key).copied().unwrap_or(0) }).collect()))
    }

    fn trend(&self, request: &Request, grain: Grain) -> Result<Value> {
        let Some(Metric::Flow(flow)) = request.op.metric() else { bail!("trend needs a flow metric") };
        let buckets = self.flow_buckets(request, flow, grain)?;
        Ok(Value::Rows(buckets.into_iter().map(|(key, value)| Row { key, value, base: 0 }).collect()))
    }

    fn metric_rows(&self, request: &Request, metric: Metric, dim: Dim) -> Result<(BTreeMap<u64, i64>, BTreeMap<u64, u64>)> {
        match metric {
            Metric::Stock | Metric::Share | Metric::Per10k(_) | Metric::InhabitantsPer(_) => {
                let set = self.set(request)?;
                let mut values = self.group_dim(request, dim, &set)?;
                let mut bases = BTreeMap::new();
                match metric {
                    Metric::Per10k(reference) => {
                        for &key in values.keys() {
                            bases.insert(key, self.member_population(request, key, dim, Metric::Per10k(reference)));
                        }
                    }
                    Metric::InhabitantsPer(reference) => {
                        for key in values.keys().copied().collect::<Vec<_>>() {
                            let count = u64::try_from(values[&key].max(0))?;
                            let population = self.member_population(request, key, dim, Metric::InhabitantsPer(reference));
                            bases.insert(key, count);
                            values.insert(key, i64::try_from(population)?);
                        }
                    }
                    Metric::Share => {
                        let total = self.filtered_count(request, &set)?;
                        for &key in values.keys() {
                            bases.insert(key, total);
                        }
                    }
                    _ => {}
                }
                Ok((values, bases))
            }
            Metric::Flow(flow) => {
                let (values, bases) = self.flow_rows(request, flow, dim)?;
                Ok((values, bases))
            }
            Metric::ActiveChange { .. } => bail!("engine has no historical snapshots"),
        }
    }

    fn flow_rows(&self, request: &Request, flow: FlowMetric, dim: Dim) -> Result<(BTreeMap<u64, i64>, BTreeMap<u64, u64>)> {
        if matches!(dim, Dim::Year | Dim::Month) {
            bail!("year and month dimensions use trends");
        }
        match flow {
            FlowMetric::NetRegistrations => {
                let registrations = self.flow_group(request, FlowMetric::Registrations, dim)?;
                let closures = self.flow_group(request, FlowMetric::Closures, dim)?;
                let mut values = registrations;
                for (key, value) in closures {
                    *values.entry(key).or_default() -= value;
                }
                Ok((values, BTreeMap::new()))
            }
            _ => Ok((self.flow_group(request, flow, dim)?, BTreeMap::new())),
        }
    }

    fn flow_group(&self, request: &Request, flow: FlowMetric, dim: Dim) -> Result<BTreeMap<u64, i64>> {
        let clone = request_with_flow(request, flow);
        let set = self.set(&clone)?;
        self.group_dim(&clone, dim, &set)
    }

    fn set(&self, request: &Request) -> Result<RoaringBitmap> {
        // The place is resolved by the oracle (`oracle::place`), not by `resolve_place` and
        // `lower`, so a place-resolution bug in the executor cannot hide behind shared code.
        let mut placeless = request.clone();
        placeless.scope.place = PlaceFilter::Any;
        placeless.scope.within_ufs = SmallSet::default();
        let mut pairs = lower(&placeless, self.context.municipalities)?;
        let place = crate::oracle::place(self.context.municipalities, &request.scope.place, &request.scope.within_ufs)?;
        if place.is_empty() {
            return Ok(RoaringBitmap::new());
        }
        if let Some(ids) = &place.municipalities {
            let toms = ids.iter().filter_map(|id| self.context.municipalities.get(*id).map(|row| format!("{:04}", row.tom)));
            pairs.push(("municipio".into(), toms.collect::<Vec<_>>().join(",")));
        }
        if let Some(ufs) = &place.ufs {
            pairs.push(("uf".into(), ufs.iter().cloned().collect::<Vec<_>>().join(",")));
        }
        let query = parse(&pairs)?;
        let mut set = search::candidates(self.snapshot, &query, &self.limits)?
            .all
            .unwrap_or_else(|| RoaringBitmap::from_iter(0..self.snapshot.doc_count()));
        if let Some(excluded) = self.exclusions(request)? {
            set -= excluded;
        }
        Ok(set)
    }

    fn exclusions(&self, request: &Request) -> Result<Option<RoaringBitmap>> {
        let scope = &request.scope;
        let mut total = RoaringBitmap::new();
        let mut any = false;
        let mut add = |key: &str, values: Vec<String>| -> Result<()> {
            if values.is_empty() {
                return Ok(());
            }
            let pairs = vec![(key.to_string(), values.join(",")), ("matriz_filial".into(), "1,2".into())];
            let query = parse(&pairs)?;
            if let Some(set) = search::candidates(self.snapshot, &query, &self.limits)?.all {
                total |= set;
            }
            any = true;
            Ok(())
        };
        let toms = scope
            .exclude
            .municipalities
            .iter()
            .filter_map(|id| self.context.municipalities.get(id.0).map(|row| format!("{:04}", row.tom)))
            .collect();
        add("municipio", toms)?;
        let ufs = scope.exclude.ufs.iter().filter_map(|uf| agg::uf_code(UfId(uf.0)).map(str::to_string)).collect();
        add("uf", ufs)?;
        let mut cnaes = scope
            .exclude
            .sector_cnaes
            .iter()
            .filter_map(|code| match code {
                CnaeCode::Subclass(code) => Some(format!("{code:07}")),
                _ => None,
            })
            .collect::<Vec<_>>();
        if cnaes.is_empty() {
            for sector in scope.exclude.sectors.iter() {
                if let Some(values) = self.context.sectors.get(&sector.0) {
                    cnaes.extend(values.iter().map(|code| format!("{code:07}")));
                }
            }
        }
        add("cnae", cnaes)?;
        Ok(any.then_some(total))
    }

    fn filtered_count(&self, request: &Request, set: &RoaringBitmap) -> Result<u64> {
        let Some(range) = request.scope.place_population else { return Ok(set.len()) };
        let reference = request.scope.population_ref.unwrap_or_else(|| self.context.municipalities.default_population());
        let range = (range.from.unwrap_or(0), range.to.unwrap_or(u32::MAX));
        Ok(self
            .municipality_groups(set)?
            .into_iter()
            .filter(|(id, _)| self.population_ok(*id, range, reference))
            .map(|(_, count)| count)
            .sum())
    }

    fn municipality_groups(&self, set: &RoaringBitmap) -> Result<BTreeMap<u16, u64>> {
        let mut groups = BTreeMap::new();
        for (tom, count) in search::group_counts_postings(self.snapshot, Some(set), Field::Municipio)? {
            if let Ok(tom) = tom.parse::<u16>()
                && let Some(row) = self.context.municipalities.by_tom(tom)
            {
                groups.insert(row.id, count);
            }
        }
        Ok(groups)
    }

    fn population_ok(&self, id: u16, range: (u32, u32), reference: PopulationRef) -> bool {
        let geo = match &self.context.municipalities.get(id).map(|row| &row.place) {
            Some(opencnpj_insights::catalog::municipalities::Place::Municipality(geo)) => Some(geo),
            _ => None,
        };
        geo.and_then(|geo| crate::oracle::population_of(geo, reference)).is_some_and(|value| (range.0..=range.1).contains(&value))
    }

    fn group_dim(&self, request: &Request, dim: Dim, set: &RoaringBitmap) -> Result<BTreeMap<u64, i64>> {
        if request.scope.place_population.is_some() {
            return self.group_dim_population(request, dim, set);
        }
        match dim {
            Dim::Municipality => {
                let mut out = BTreeMap::new();
                for (id, count) in self.municipality_groups(set)? {
                    out.insert(u64::from(id), i64::try_from(count)?);
                }
                Ok(out)
            }
            Dim::Uf => self.group_simple(set, Field::Uf, |value| agg::uf_id(value).map(|uf| u64::from(uf.0))),
            Dim::Cnae(CnaeLevel::Section) => {
                // Sections are counted with the engine's own section filter (`cnae=<letter>`),
                // not with a copy of the section table.
                let mut out = BTreeMap::new();
                for letter in b'A'..=b'U' {
                    let query = parse(&[("cnae".into(), char::from(letter).to_string()), ("matriz_filial".into(), "1,2".into())])?;
                    if let Some(docs) = search::candidates(self.snapshot, &query, &self.limits)?.all {
                        let count = docs.intersection_len(set);
                        if count > 0 {
                            out.insert(u64::from(letter), i64::try_from(count)?);
                        }
                    }
                }
                Ok(out)
            }
            Dim::Cnae(level) => self.group_simple(set, Field::Cnae, |value| value.parse::<u32>().ok().map(|cnae| cnae_key(level, cnae))),
            Dim::Sector => {
                let groups = self.group_simple(set, Field::Cnae, |value| value.parse::<u32>().ok().map(u64::from))?;
                let mut out = BTreeMap::new();
                for (cnae, count) in groups {
                    for sector in self.context.sectors_of(cnae as u32) {
                        *out.entry(u64::from(*sector)).or_default() += count;
                    }
                }
                Ok(out)
            }
            Dim::Porte => self.group_simple(set, Field::Porte, |value| match value {
                "00" => Some(0),
                "01" => Some(1),
                "03" => Some(2),
                "05" => Some(3),
                _ => None,
            }),
            Dim::Status => self.group_simple(set, Field::Situacao, |value| match value {
                "01" => Some(1),
                "02" => Some(2),
                "03" => Some(3),
                "04" => Some(4),
                "08" => Some(5),
                _ => None,
            }),
            Dim::HeadOffice => self.group_simple(set, Field::MatrizFilial, |value| match value {
                "1" => Some(0),
                "2" => Some(1),
                _ => None,
            }),
            Dim::Mei => self.group_simple(set, Field::Mei, |value| match value {
                "N" => Some(0),
                "S" => Some(1),
                _ => None,
            }),
            Dim::Region(level) => {
                let groups = self.municipality_groups(set)?;
                let mut out = BTreeMap::new();
                for (id, count) in groups {
                    if let Some(region) = self.region_of(id, level) {
                        *out.entry(region).or_default() += i64::try_from(count)?;
                    }
                }
                Ok(out)
            }
            Dim::Year | Dim::Month => bail!("year and month dimensions need a flow trend"),
        }
    }

    fn group_dim_population(&self, request: &Request, dim: Dim, set: &RoaringBitmap) -> Result<BTreeMap<u64, i64>> {
        let reference = request.scope.population_ref.unwrap_or_else(|| self.context.municipalities.default_population());
        let range = request.scope.place_population.map(|range| (range.from.unwrap_or(0), range.to.unwrap_or(u32::MAX))).unwrap();
        let groups =
            self.municipality_groups(set)?.into_iter().filter(|(id, _)| self.population_ok(*id, range, reference)).collect::<Vec<_>>();
        match dim {
            Dim::Municipality => {
                let mut out = BTreeMap::new();
                for (id, count) in groups {
                    out.insert(u64::from(id), i64::try_from(count)?);
                }
                Ok(out)
            }
            Dim::Uf => {
                let mut out = BTreeMap::new();
                for (id, count) in groups {
                    if let Some(opencnpj_insights::catalog::municipalities::Place::Municipality(geo)) =
                        self.context.municipalities.get(id).map(|row| &row.place)
                        && let Some(uf) = agg::uf_id(&geo.uf)
                    {
                        *out.entry(u64::from(uf.0)).or_default() += i64::try_from(count)?;
                    }
                }
                Ok(out)
            }
            _ => bail!("population filter with this dimension needs the aggregates"),
        }
    }

    fn group_simple(&self, set: &RoaringBitmap, field: Field, map: impl Fn(&str) -> Option<u64>) -> Result<BTreeMap<u64, i64>> {
        let mut out = BTreeMap::new();
        for (value, count) in search::group_counts_postings(self.snapshot, Some(set), field)? {
            if let Some(key) = map(&value) {
                *out.entry(key).or_default() += i64::try_from(count)?;
            }
        }
        Ok(out)
    }

    fn region_of(&self, id: u16, level: RegionLevel) -> Option<u64> {
        let row = self.context.municipalities.get(id)?;
        match (&row.place_level(), level) {
            (Level::Municipality(geo), RegionLevel::Major) => crate::oracle::macro_region(&geo.uf).map(u64::from),
            (Level::Municipality(geo), RegionLevel::Intermediate) => Some(u64::from(geo.intermediate_region)),
            (Level::Municipality(geo), RegionLevel::Immediate) => Some(u64::from(geo.immediate_region)),
            _ => None,
        }
    }

    fn place_population(&self, request: &Request, reference: PopulationRef) -> u64 {
        crate::oracle::population(self.context.municipalities, request, None, reference).expect("oracle population")
    }

    fn member_population(&self, request: &Request, key: u64, dim: Dim, _metric: Metric) -> u64 {
        let reference = request.scope.population_ref.unwrap_or_else(|| match request.op.metric() {
            Some(Metric::Per10k(reference) | Metric::InhabitantsPer(reference)) => reference,
            _ => self.context.municipalities.default_population(),
        });
        crate::oracle::population(self.context.municipalities, request, Some((dim, key)), reference).expect("oracle population")
    }
}

enum Level<'a> {
    Municipality(&'a opencnpj_insights::catalog::municipalities::Geography),
    Abroad,
}

trait PlaceLevel {
    fn place_level(&self) -> Level<'_>;
}

impl PlaceLevel for opencnpj_insights::catalog::municipalities::Municipality {
    fn place_level(&self) -> Level<'_> {
        match &self.place {
            opencnpj_insights::catalog::municipalities::Place::Municipality(geo) => Level::Municipality(geo),
            opencnpj_insights::catalog::municipalities::Place::Abroad => Level::Abroad,
        }
    }
}

fn cnae_key(level: CnaeLevel, cnae: u32) -> u64 {
    match level {
        CnaeLevel::Section => unreachable!("sections are counted with the engine's section filter"),
        CnaeLevel::Division => u64::from(cnae / 100_000),
        CnaeLevel::Group => u64::from(cnae / 10_000),
        CnaeLevel::Class => u64::from(cnae / 100),
        CnaeLevel::Subclass => u64::from(cnae),
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

fn compare_ratio(a: Row, b: Row) -> std::cmp::Ordering {
    let left = i128::from(a.value) * i128::from(b.base.max(1));
    let right = i128::from(b.value) * i128::from(a.base.max(1));
    left.cmp(&right)
}

fn request_with_flow(request: &Request, flow: FlowMetric) -> Request {
    let mut clone = request.clone();
    match &mut clone.op {
        Op::Count { metric } | Op::Compare { metric, .. } | Op::Rank { metric, .. } | Op::Breakdown { metric, .. } => {
            *metric = Metric::Flow(flow);
        }
        Op::Trend { metric, .. } => *metric = flow,
        _ => {}
    }
    clone.scope.status = Some(clone.op.default_status());
    clone
}

impl EngineExecutor<'_> {
    fn flow_count(&self, request: &Request, flow: FlowMetric) -> Result<i64> {
        match flow {
            FlowMetric::NetRegistrations => {
                let registrations = self.set(&request_with_flow(request, FlowMetric::Registrations))?.len();
                let closures = self.set(&request_with_flow(request, FlowMetric::Closures))?.len();
                Ok(i64::try_from(registrations)? - i64::try_from(closures)?)
            }
            _ => Ok(i64::try_from(self.set(&request_with_flow(request, flow))?.len())?),
        }
    }

    fn flow_buckets(&self, request: &Request, flow: FlowMetric, grain: Grain) -> Result<BTreeMap<u64, i64>> {
        match flow {
            FlowMetric::NetRegistrations => {
                let registrations = self.flow_buckets(request, FlowMetric::Registrations, grain)?;
                let closures = self.flow_buckets(request, FlowMetric::Closures, grain)?;
                let mut values = registrations;
                for (key, value) in closures {
                    *values.entry(key).or_default() -= value;
                }
                Ok(values)
            }
            _ => {
                let clone = request_with_flow(request, flow);
                let set = self.set(&clone)?;
                let field = match flow {
                    FlowMetric::Registrations => NumField::InicioAtividade,
                    FlowMetric::Closures => NumField::DataSituacao,
                    FlowMetric::MeiOptIns => NumField::OpcaoMei,
                    FlowMetric::MeiExits => NumField::ExclusaoMei,
                    FlowMetric::NetRegistrations => unreachable!(),
                };
                let column = self.snapshot.numeric(field).context("snapshot needs the flow date column")?;
                let mut values = BTreeMap::new();
                for doc in set {
                    let value = column.get(self.snapshot.bytes(), doc);
                    if let Some((year, month, _)) = Day(u32::try_from(value).unwrap_or(0)).to_ymd() {
                        let key = match grain {
                            Grain::Month => u64::from(year) * 12 + u64::from(month) - 1,
                            Grain::Year => u64::from(year),
                        };
                        *values.entry(key).or_default() += 1;
                    }
                }
                Ok(values)
            }
        }
    }
}

/// Load the curated sector TOML into dense sector IDs in file order.
/// Sector id (position in the catalogue) → **core** CNAE subclasses. The extended set is
/// opt-in ("incluir fabricação", 09-DATA-ASSETS.md §3); the former line scanner also read
/// `cnae_extended`, so lab sectors (and both sides of `diff-exec`) were wider than the
/// catalogue's definition.
pub fn load_sectors(path: &Path) -> Result<SectorMap> {
    #[derive(serde::Deserialize)]
    struct File {
        sector: Vec<Entry>,
    }
    #[derive(serde::Deserialize)]
    struct Entry {
        cnae_core: Vec<String>,
    }
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let file: File = toml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    let mut map = SectorMap::new();
    for (index, entry) in file.sector.into_iter().enumerate() {
        let codes =
            entry.cnae_core.iter().map(|code| code.parse::<u32>().with_context(|| format!("CNAE {code}"))).collect::<Result<_>>()?;
        map.insert(u16::try_from(index)?, codes);
    }
    Ok(map)
}

pub fn canonicalization(
    catalogue: &MunicipalityCatalogue,
    sectors: &SectorMap,
    _last_day: Day,
) -> (BTreeMap<MunicipalityId, UfId>, BTreeMap<SectorId, SmallSet<CnaeCode>>) {
    let municipality_ufs = catalogue
        .rows()
        .iter()
        .filter_map(|row| match &row.place {
            opencnpj_insights::catalog::municipalities::Place::Municipality(geo) => {
                agg::uf_id(&geo.uf).map(|uf| (MunicipalityId(row.id), uf))
            }
            _ => None,
        })
        .collect();
    let sector_cnaes =
        sectors.iter().map(|(&id, cnaes)| (SectorId(id), SmallSet::new(cnaes.iter().map(|code| CnaeCode::Subclass(*code))))).collect();
    (municipality_ufs, sector_cnaes)
}

pub fn validate_capabilities(catalogue: &MunicipalityCatalogue, sectors: &SectorMap, last_day: Day) -> Capabilities {
    // Offer only the population references the catalogue actually holds.
    let mut populations = vec![PopulationRef::IbgeCensus(2022)];
    let mut years: Vec<u16> = catalogue
        .rows()
        .iter()
        .filter_map(|row| match &row.place {
            opencnpj_insights::catalog::municipalities::Place::Municipality(geo) => Some(geo.estimate_year),
            opencnpj_insights::catalog::municipalities::Place::Abroad => None,
        })
        .collect();
    years.sort_unstable();
    years.dedup();
    populations.extend(years.into_iter().map(PopulationRef::IbgeEstimate));
    let mut competences = Vec::new();
    let (mut year, mut month) = (2024, 11);
    for _ in 0..24 {
        competences.push(Competence::from_year_month(year, month).expect("competence"));
        month += 1;
        if month == 13 {
            month = 1;
            year += 1;
        }
    }
    Capabilities {
        current: Competence::from_year_month(2026, 10).expect("current competence"),
        competences,
        flow_days: Day::from_ymd(2025, 1, 1).expect("flow start")..=last_day,
        populations,
        catalogue_versions: CatalogueVersions { municipalities: 202610, sectors: 1 },
        sector_count: u16::try_from(sectors.len()).expect("sector count"),
        geography: true,
        list_limit: 100,
    }
}

pub fn validate_request(request: Request, catalogue: &MunicipalityCatalogue, sectors: &SectorMap, last_day: Day) -> Result<ValidRequest> {
    let (municipality_ufs, sector_cnaes) = canonicalization(catalogue, sectors, last_day);
    let context = CanonicalContext { first_day: Day(1), last_day, municipality_ufs: &municipality_ufs, sector_cnaes: &sector_cnaes };
    let canonical = request.canonicalize(&context)?.0;
    let capabilities = validate_capabilities(catalogue, sectors, last_day);
    validate::validate(canonical, &capabilities)
        .map_err(|issues| anyhow::anyhow!("{}", issues.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")))
}

pub fn load_golden(path: &Path) -> Result<Vec<(String, Request)>> {
    #[derive(serde::Deserialize)]
    struct Case {
        case: String,
        request: Request,
    }
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    text.lines()
        .map(|line| {
            let case: Case = serde_json::from_str(line)?;
            Ok((case.case, case.request))
        })
        .collect()
}

#[derive(clap::Args)]
pub struct DiffArgs {
    #[arg(long)]
    pub agg: std::path::PathBuf,
    #[arg(long)]
    pub history: Option<std::path::PathBuf>,
    #[arg(long)]
    pub snapshot: std::path::PathBuf,
    #[arg(long, default_value = "/data/opencnpj-data/intelligence/catalog-municipalities/2026-10/municipalities.tsv")]
    pub catalog: std::path::PathBuf,
    #[arg(long, default_value = "crates/opencnpj-insights/catalog/sectors.toml")]
    pub sectors: std::path::PathBuf,
    #[arg(long, default_value = "crates/opencnpj-query/tests/fixtures/ir-v1-requests.jsonl")]
    pub cases: std::path::PathBuf,
    #[arg(long)]
    pub case: Option<String>,
}

pub fn diff(args: DiffArgs) -> Result<()> {
    let catalogue = MunicipalityCatalogue::load(&args.catalog)?;
    let sectors = load_sectors(&args.sectors)?;
    let aggregates = opencnpj_insights::execute::AggregateSet::open(&args.agg, args.history.as_deref())?;
    ensure!(
        sha256_file(&args.catalog)? == aggregates.current().manifest().municipality_sha256,
        "aggregate uses another municipality catalogue"
    );
    let context = ExecContext::new(&catalogue, &sectors);
    let snapshot = Snapshot::open(&args.snapshot)?;
    snapshot.set_postings_cache_bytes(512 << 20);
    let engine = EngineExecutor { snapshot: &snapshot, context: &context, limits: Limits::DEFAULT };
    let last_day = Day::from_ymd(2026, 10, 11).expect("anchor");
    let current = Competence::from_year_month(2026, 10).expect("current");
    let mut compared = 0;
    let mut mismatches = 0;
    let mut aggregate_only = 0;
    let mut engine_only = 0;
    let mut engine_errors = 0;
    let mut invalid = 0;
    for (case, request) in load_golden(&args.cases)? {
        if args.case.as_ref().is_some_and(|filter| !case.contains(filter.as_str())) {
            continue;
        }
        let valid = match validate_request(request, &catalogue, &sectors, last_day) {
            Ok(valid) => valid,
            Err(error) => {
                println!("INVALID\t{case}\t{error}");
                invalid += 1;
                continue;
            }
        };
        let request = valid.as_request();
        let plan = match opencnpj_insights::plan::plan(&valid) {
            Ok(plan) => plan,
            Err(error) => {
                println!("UNSUPPORTED\t{case}\t{error}");
                continue;
            }
        };
        match plan.backend {
            opencnpj_insights::plan::Backend::Static => println!("static\t{case}"),
            opencnpj_insights::plan::Backend::Engine => match engine.execute(&valid) {
                Ok(value) => {
                    println!("engine-only\t{case}\t{value:?}");
                    engine_only += 1;
                }
                Err(error) => {
                    println!("engine-n/a\t{case}\t{error}");
                    engine_errors += 1;
                }
            },
            opencnpj_insights::plan::Backend::Aggregates => {
                let aggregate = opencnpj_insights::execute::execute(&valid, &aggregates, &context);
                let aggregate = match aggregate {
                    Ok(value) => value,
                    Err(error) => {
                        println!("aggregate-error\t{case}\t{error}");
                        mismatches += 1;
                        continue;
                    }
                };
                let historical = request.time.competence != current;
                let active_change = matches!(request.op.metric(), Some(Metric::ActiveChange { .. }));
                if historical || active_change {
                    // The engine has no past snapshots: check these against the artifacts'
                    // raw marginal sections instead, when the request shape allows it.
                    match crate::oracle::marginal_value(request, &aggregates, &sectors)? {
                        Some(expected) if expected == aggregate => {
                            println!("marginal-match\t{case}");
                            compared += 1;
                        }
                        Some(expected) => {
                            println!("MISMATCH\t{case}\taggregate={aggregate:?}\tmarginals={expected:?}");
                            compared += 1;
                            mismatches += 1;
                        }
                        None => {
                            println!("aggregate-only\t{case}\t{aggregate:?}");
                            aggregate_only += 1;
                        }
                    }
                    continue;
                }
                match engine.execute(&valid) {
                    Ok(engine_value) => {
                        if aggregate == engine_value {
                            println!("match\t{case}");
                        } else {
                            println!("MISMATCH\t{case}\taggregate={aggregate:?}\tengine={engine_value:?}");
                            mismatches += 1;
                        }
                        compared += 1;
                    }
                    Err(error) => {
                        println!("engine-n/a\t{case}\t{error}");
                        engine_errors += 1;
                    }
                }
            }
        }
    }
    println!(
        "summary\tcompared={compared}\tmismatches={mismatches}\taggregate_only={aggregate_only}\tengine_only={engine_only}\tengine_n/a={engine_errors}\tinvalid={invalid}"
    );
    ensure!(invalid == 0, "{invalid} invalid cases");
    ensure!(mismatches == 0, "{mismatches} mismatches");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::io::BufWriter;

    use super::*;
    use opencnpj_insights::{
        agg::{self, Manifest},
        catalog::municipalities::{Geography, Municipality, Place},
    };
    use serde_json::json;

    fn fitting_catalogue(snapshot: &opencnpj_store::Snapshot) -> MunicipalityCatalogue {
        let mut toms = BTreeSet::new();
        for (tom, _) in snapshot.field_values(Field::Municipio).unwrap() {
            toms.insert(tom.parse::<u16>().unwrap());
        }
        let rows = toms
            .into_iter()
            .enumerate()
            .map(|(id, tom)| {
                let name = format!("Fixture {tom}");
                Municipality {
                    id: id as u16,
                    tom,
                    folded: opencnpj_insights::catalog::municipalities::fold_name(&name),
                    name,
                    variants: vec![],
                    place: Place::Municipality(Geography {
                        ibge: 1_000_001 + id as u32,
                        uf: "SP".into(),
                        census_2022: Some(1),
                        population_estimate: 1,
                        estimate_year: 2025,
                        latitude: 0.0,
                        longitude: 0.0,
                        neighbours: vec![],
                        immediate_region: 1,
                        intermediate_region: 1,
                        capital: false,
                    }),
                }
            })
            .collect();
        MunicipalityCatalogue::from_rows(rows).unwrap()
    }

    fn request(op: serde_json::Value, scope: serde_json::Value) -> Request {
        serde_json::from_value(json!({
            "op": op,
            "unit": "establishments",
            "scope": scope,
            "time": {"competence": 321, "period": null, "anchor": 46305},
            "output": {},
            "versions": {"municipalities": 202610, "sectors": 1}
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn http_client_uses_key_timeout_and_concurrency() {
        use axum::{Json, Router, extract::Query, http::HeaderMap, routing::get};
        let app = Router::new().route(
            "/api/v1/search",
            get(|headers: HeaderMap, Query(params): Query<Vec<(String, String)>>| async move {
                assert_eq!(headers.get("authorization").and_then(|value| value.to_str().ok()), Some("Bearer test-key"));
                assert!(params.iter().any(|(key, value)| key == "limit" && value == "1"));
                assert!(params.iter().any(|(key, value)| key == "matriz_filial" && value == "1,2"));
                Json(json!({"total": 42}))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let catalogue = MunicipalityCatalogue::from_rows(vec![Municipality {
            id: 0,
            tom: 7107,
            name: "Test".into(),
            folded: opencnpj_insights::catalog::municipalities::fold_name("Test"),
            variants: vec![],
            place: Place::Municipality(Geography {
                ibge: 1_000_001,
                uf: "SP".into(),
                census_2022: Some(1),
                population_estimate: 1,
                estimate_year: 2025,
                latitude: 0.0,
                longitude: 0.0,
                neighbours: vec![],
                immediate_region: 1,
                intermediate_region: 1,
                capital: false,
            }),
        }])
        .unwrap();
        let client =
            HttpEngineClient::new(format!("http://{address}"), Some("test-key".into()), std::time::Duration::from_secs(5), 2).unwrap();
        let request = request(json!({"kind": "count", "metric": "stock"}), json!({}));
        assert_eq!(client.count(&request, &catalogue).await.unwrap(), 42);
        assert_eq!(client.count_many(&[request.clone(), request], &catalogue).await.unwrap(), vec![42, 42]);
    }

    #[test]
    fn fixture_lowering_and_differential() {
        let directory = tempfile::tempdir().unwrap();
        let (snapshot_path, _) = opencnpj_ingest::testkit::build_fixture(directory.path(), "balanced");
        let snapshot = opencnpj_store::Snapshot::open(&snapshot_path).unwrap();
        snapshot.set_postings_cache_bytes(64 << 20);
        let catalogue = fitting_catalogue(&snapshot);
        let sectors = SectorMap::from([(0, BTreeSet::from([1091102])), (1, BTreeSet::from([4712100]))]);
        let result = agg::builder::build(&snapshot, &catalogue, &agg::Table::ALL).unwrap();
        let manifest = Manifest {
            competence: Competence(321),
            snapshot_sha256: "0".repeat(64),
            municipality_sha256: "0".repeat(64),
            tables: agg::Table::ALL.to_vec(),
            establishments: result.establishments,
            missing_registration_date: result.missing_registration_date,
            missing_closure_date: result.missing_closure_date,
        };
        let aggregate_path = directory.path().join("fixture.ocagg");
        agg::write(BufWriter::new(std::fs::File::create(&aggregate_path).unwrap()), &manifest, &result.sections).unwrap();
        let aggregates = AggregateSet::open(&aggregate_path, None).unwrap();
        let context = ExecContext::new(&catalogue, &sectors);
        let engine = EngineExecutor { snapshot: &snapshot, context: &context, limits: Limits::DEFAULT };
        let last_day = Day::from_ymd(2026, 10, 11).unwrap();
        let sector_scope = json!({"place": {"municipalities": [3]}, "activity": {"sectors": {"ids": [0], "cnaes": [], "scope": "main"}}, "status": ["active"]});
        let cases = [
            ("count", request(json!({"kind": "count", "metric": "stock"}), sector_scope.clone())),
            (
                "compare",
                request(
                    json!({"kind": "compare", "dim": "municipality", "members": [{"municipality": 3}, {"municipality": 1}], "metric": "stock"}),
                    json!({"activity": {"sectors": {"ids": [1], "cnaes": [], "scope": "main"}}, "status": ["active"]}),
                ),
            ),
            (
                "breakdown",
                request(
                    json!({"kind": "breakdown", "dim": "uf", "metric": "stock"}),
                    json!({"activity": {"sectors": {"ids": [1], "cnaes": [], "scope": "main"}}, "status": ["active"]}),
                ),
            ),
            (
                "rank",
                request(
                    json!({"kind": "rank", "dim": "municipality", "metric": "stock", "dir": "desc", "k": 10, "min_base": 0}),
                    json!({"activity": {"sectors": {"ids": [1], "cnaes": [], "scope": "main"}}, "status": ["active"]}),
                ),
            ),
            ("closed", request(json!({"kind": "count", "metric": "stock"}), json!({"status": ["closed"]}))),
            ("density", request(json!({"kind": "count", "metric": {"per10k": {"ibge_estimate": 2025}}}), sector_scope.clone())),
            ("head_office", request(json!({"kind": "count", "metric": "stock"}), json!({"head_office": "yes", "status": ["active"]}))),
        ];
        for (name, request) in cases {
            let valid = validate_request(request, &catalogue, &sectors, last_day).unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!(opencnpj_insights::plan::plan(&valid).unwrap().backend, opencnpj_insights::plan::Backend::Aggregates, "{name}");
            let aggregate = opencnpj_insights::execute::execute(&valid, &aggregates, &context).unwrap();
            let engine_value = engine.execute(&valid).unwrap();
            assert_eq!(aggregate, engine_value, "{name}");
        }

        let valid =
            validate_request(request(json!({"kind": "count", "metric": "stock"}), sector_scope), &catalogue, &sectors, last_day).unwrap();
        let query = parse(&lower(valid.as_request(), &catalogue).unwrap()).unwrap();
        assert!(query.filters.contains(&(Field::Situacao, vec!["02".into()])));
        assert!(query.filters.contains(&(Field::Municipio, vec!["7107".into()])));
        assert!(query.filters.contains(&(Field::Cnae, vec!["1091102".into()])));
        assert!(query.filters.contains(&(Field::MatrizFilial, vec!["1".into(), "2".into()])));

        let mut list = request(
            json!({"kind": "list", "order": "capital_desc", "limit": 10}),
            json!({"place": {"ufs": [25]}, "capital": {"from": 5_000_000_000_u64}, "contact": "phone"}),
        );
        list.output.page = 2;
        let query = parse(&lower(&list, &catalogue).unwrap()).unwrap();
        assert_eq!(query.limit, 10);
        assert_eq!(query.offset, 10);
        assert_eq!(query.sort, Some((NumField::CapitalSocial, true)));
        assert!(query.filters.contains(&(Field::Contato, vec!["telefone".into()])));
        let capital = query.ranges.iter().find(|range| range.field == NumField::CapitalSocial).unwrap();
        assert_eq!(capital.range.unwrap().lo, 5_000_000_001);
    }

    #[test]
    fn lowering_an_unknown_municipality_is_an_error() {
        let catalogue = MunicipalityCatalogue::from_rows(vec![Municipality {
            id: 0,
            tom: 7107,
            name: "Test".into(),
            folded: opencnpj_insights::catalog::municipalities::fold_name("Test"),
            variants: vec![],
            place: Place::Municipality(Geography {
                ibge: 1_000_001,
                uf: "SP".into(),
                census_2022: Some(1),
                population_estimate: 1,
                estimate_year: 2025,
                latitude: 0.0,
                longitude: 0.0,
                neighbours: vec![],
                immediate_region: 1,
                intermediate_region: 1,
                capital: false,
            }),
        }])
        .unwrap();
        let bad = request(json!({"kind": "count", "metric": "stock"}), json!({"place": {"municipalities": [9999]}}));
        assert!(lower(&bad, &catalogue).is_err());
    }
}

/// HTTP client for the engine API (service key, timeout, bounded concurrency).
pub struct HttpEngineClient {
    client: reqwest::Client,
    base: String,
    key: Option<String>,
    permits: tokio::sync::Semaphore,
    concurrency: usize,
}

impl HttpEngineClient {
    pub fn new(base: impl Into<String>, key: Option<String>, timeout: std::time::Duration, concurrency: usize) -> Result<Self> {
        ensure!(concurrency > 0, "concurrency must be positive");
        let client = reqwest::Client::builder().timeout(timeout).build().context("build HTTP client")?;
        Ok(Self {
            client,
            base: base.into().trim_end_matches('/').to_string(),
            key,
            permits: tokio::sync::Semaphore::new(concurrency),
            concurrency,
        })
    }

    pub async fn count(&self, request: &Request, catalogue: &MunicipalityCatalogue) -> Result<u64> {
        let pairs = lower(request, catalogue)?;
        let mut call = self.client.get(format!("{}/api/v1/search", self.base)).query(&pairs);
        if let Some(key) = &self.key {
            call = call.bearer_auth(key);
        }
        let response = call.send().await.context("engine request failed")?;
        ensure!(response.status().is_success(), "engine returned {}", response.status());
        let body: serde_json::Value =
            serde_json::from_str(&response.text().await.context("engine response body")?).context("engine response is not JSON")?;
        body.get("total").and_then(serde_json::Value::as_u64).context("engine response has no total")
    }

    pub async fn count_many(&self, requests: &[Request], catalogue: &MunicipalityCatalogue) -> Result<Vec<u64>> {
        use futures_util::{StreamExt, stream};
        let results = stream::iter(requests)
            .map(|request| async move {
                let _permit = self.permits.acquire().await.context("client closed")?;
                self.count(request, catalogue).await
            })
            .buffered(self.concurrency)
            .collect::<Vec<_>>()
            .await;
        results.into_iter().collect()
    }
}

pub fn bench(args: DiffArgs) -> Result<()> {
    let catalogue = MunicipalityCatalogue::load(&args.catalog)?;
    let sectors = load_sectors(&args.sectors)?;
    let aggregates = AggregateSet::open(&args.agg, args.history.as_deref())?;
    ensure!(
        sha256_file(&args.catalog)? == aggregates.current().manifest().municipality_sha256,
        "aggregate uses another municipality catalogue"
    );
    let context = ExecContext::new(&catalogue, &sectors);
    let snapshot = Snapshot::open(&args.snapshot)?;
    snapshot.set_postings_cache_bytes(512 << 20);
    let engine = EngineExecutor { snapshot: &snapshot, context: &context, limits: Limits::DEFAULT };
    let last_day = Day::from_ymd(2026, 10, 11).expect("anchor");
    let current = Competence::from_year_month(2026, 10).expect("current");
    println!("case\tbackend\taggregate_us\tengine_us");
    let (mut aggregate_total, mut engine_total) = (0_u128, 0_u128);
    for (case, request) in load_golden(&args.cases)? {
        if args.case.as_ref().is_some_and(|filter| !case.contains(filter.as_str())) {
            continue;
        }
        let valid = validate_request(request, &catalogue, &sectors, last_day)?;
        let plan = opencnpj_insights::plan::plan(&valid)?;
        let request = valid.as_request();
        let mut aggregate_ns = 0;
        let mut engine_ns = 0;
        match plan.backend {
            opencnpj_insights::plan::Backend::Aggregates => {
                let start = std::time::Instant::now();
                opencnpj_insights::execute::execute(&valid, &aggregates, &context)?;
                aggregate_ns = start.elapsed().as_nanos();
                let historical = request.time.competence != current;
                let active_change = matches!(request.op.metric(), Some(Metric::ActiveChange { .. }));
                if !historical && !active_change {
                    let start = std::time::Instant::now();
                    engine.execute(&valid)?;
                    engine_ns = start.elapsed().as_nanos();
                }
            }
            opencnpj_insights::plan::Backend::Engine => {
                let start = std::time::Instant::now();
                if engine.execute(&valid).is_ok() {
                    engine_ns = start.elapsed().as_nanos();
                }
            }
            opencnpj_insights::plan::Backend::Static => {}
        }
        aggregate_total += aggregate_ns;
        engine_total += engine_ns;
        println!("{case}\t{:?}\t{}\t{}", plan.backend, aggregate_ns / 1000, engine_ns / 1000);
    }
    println!("total\t\t{}\t{}", aggregate_total / 1000, engine_total / 1000);
    Ok(())
}

#[derive(clap::Args)]
pub struct RemoteArgs {
    #[arg(long)]
    pub base: String,
    #[arg(long)]
    pub key: Option<String>,
    #[arg(long, default_value_t = 8)]
    pub concurrency: usize,
    #[arg(long, default_value_t = 10)]
    pub timeout_seconds: u64,
    #[arg(long, default_value = "/data/opencnpj-data/intelligence/catalog-municipalities/2026-10/municipalities.tsv")]
    pub catalog: std::path::PathBuf,
    #[arg(long, default_value = "crates/opencnpj-insights/catalog/sectors.toml")]
    pub sectors: std::path::PathBuf,
    #[arg(long, default_value = "crates/opencnpj-query/tests/fixtures/ir-v1-requests.jsonl")]
    pub cases: std::path::PathBuf,
    #[arg(long)]
    pub case: Option<String>,
}

pub fn remote_count(args: RemoteArgs) -> Result<()> {
    let catalogue = MunicipalityCatalogue::load(&args.catalog)?;
    let sectors = load_sectors(&args.sectors)?;
    let last_day = Day::from_ymd(2026, 10, 11).expect("anchor");
    let client = HttpEngineClient::new(args.base, args.key, std::time::Duration::from_secs(args.timeout_seconds), args.concurrency)?;
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    let mut cases = Vec::new();
    for (case, request) in load_golden(&args.cases)? {
        if args.case.as_ref().is_some_and(|filter| !case.contains(filter.as_str())) {
            continue;
        }
        let valid = validate_request(request, &catalogue, &sectors, last_day)?;
        cases.push((case, valid.into_request()));
    }
    let totals = runtime.block_on(client.count_many(&cases.iter().map(|(_, request)| request.clone()).collect::<Vec<_>>(), &catalogue))?;
    for ((case, _), total) in cases.iter().zip(totals) {
        println!("{case}\t{total}");
    }
    Ok(())
}
