//! Query IR lowering and in-process engine execution.

use std::{collections::BTreeMap, path::Path};

use crate::{params, params::Caps};
use anyhow::{Context, Result, bail, ensure};
use opencnpj_core::{schema::col, text::name_starts_with};
use opencnpj_insights::{
    agg,
    catalog::municipalities::MunicipalityCatalogue,
    execute::{ExecContext, Row, SectorMap, Value, resolve_place},
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
    ensure!(request.scope.name_prefix.is_none(), "name prefixes require the in-process record matcher");
    lower_indexed(request, catalogue)
}

/// Index-backed criteria only. The executor applies remaining predicates to the
/// complete candidate set before counting, ordering, paging or exporting.
fn lower_indexed(request: &Request, catalogue: &MunicipalityCatalogue) -> Result<Vec<(String, String)>> {
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

pub fn add_flow(pairs: &mut Vec<(String, String)>, request: &Request, flow: FlowMetric) -> Result<()> {
    let Some(Period::Days(days)) = request.time.period else { bail!("flow needs a day period") };
    for field in flow_fields(flow) {
        add_days(pairs, field, days);
    }
    Ok(())
}

pub fn flow_fields(flow: FlowMetric) -> Vec<&'static str> {
    match flow {
        FlowMetric::Registrations => vec!["data_inicio_atividade"],
        FlowMetric::Closures => vec!["data_situacao_cadastral"],
        FlowMetric::NetRegistrations => vec!["data_inicio_atividade", "data_situacao_cadastral"],
        FlowMetric::MeiOptIns => vec!["data_opcao_mei"],
        FlowMetric::MeiExits => vec!["data_exclusao_mei"],
    }
}

pub fn add_days(pairs: &mut Vec<(String, String)>, field: &str, range: DayRange) {
    if let Some(from) = range.from {
        pairs.push((format!("{field}_de"), from.to_string()));
    }
    if let Some(to) = range.to {
        pairs.push((format!("{field}_ate"), to.to_string()));
    }
}

pub fn cnae_values(codes: &SmallSet<CnaeCode>) -> Vec<String> {
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

pub fn status_code(status: Status) -> &'static str {
    match status {
        Status::Null => "01",
        Status::Active => "02",
        Status::Suspended => "03",
        Status::Unfit => "04",
        Status::Closed => "08",
    }
}

pub fn tri_code(tri: Tri) -> String {
    match tri {
        Tri::Any => String::new(),
        Tri::Yes => "S".into(),
        Tri::No => "N".into(),
    }
}

pub fn money(cents: u64) -> String {
    format!("{},{:02}", cents / 100, cents % 100)
}

pub fn list_sort(order: ListOrder) -> Option<&'static str> {
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
    /// Lists use the same complete filter set as counts before sorting or pagination.
    /// Lowering alone cannot express exclusions or population predicates.
    pub fn records(&self, request: &ValidRequest) -> Result<search::SearchResult> {
        let query = parse(&lower_indexed(request, self.context.municipalities)?)?;
        self.records_window(request, query.offset, query.limit)
    }

    /// Up to `limit` records from `offset` in the list order, e.g. every row of an export.
    pub fn records_window(&self, request: &ValidRequest, offset: usize, limit: usize) -> Result<search::SearchResult> {
        ensure!(matches!(request.op, Op::List { .. } | Op::Lookup { .. }), "records needs a list or lookup");
        let mut set = self.set(request)?;
        if let Some(range) = request.scope.place_population
            && !set.is_empty()
        {
            let place = super::geo::place(self.context.municipalities, &request.scope.place, &request.scope.within_ufs)?;
            let reference = request.scope.population_ref.unwrap_or_else(|| self.context.municipalities.default_population());
            let mut allowed = RoaringBitmap::new();
            for row in self.context.municipalities.rows() {
                if let Level::Municipality(geo) = row.place_level()
                    && place.admits(row.id, &geo.uf)
                    && self.population_ok(row.id, (range.from.unwrap_or(0), range.to.unwrap_or(u32::MAX)), reference)
                    && let Some(postings) = self.snapshot.postings(Field::Municipio, format!("{:04}", row.tom).as_bytes())?
                {
                    allowed |= &set & &*postings;
                }
            }
            set = allowed;
        }
        let query = parse(&lower_indexed(request, self.context.municipalities)?)?;
        let total = set.len();
        let docs = if let Some((field, desc)) = query.sort {
            let column = self.snapshot.numeric(field).context("missing sort column")?;
            column.top_k(self.snapshot.bytes(), &set, offset.saturating_add(limit), desc).into_iter().skip(offset).collect()
        } else {
            set.iter().skip(offset).take(limit).collect()
        };
        Ok(search::SearchResult { total, docs, ..Default::default() })
    }

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

    pub fn count(&self, request: &Request) -> Result<Value> {
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

    pub fn compare(&self, request: &Request, dim: Dim, members: &Members) -> Result<Value> {
        let metric = request.op.metric().context("operation metric")?;
        let mut scoped = request.clone();
        if !matches!(metric, Metric::Share) {
            match dim {
                Dim::Municipality if matches!(scoped.scope.place, PlaceFilter::Any) => {
                    let ids: Vec<_> =
                        members.iter().filter_map(|m| if let Member::Municipality(id) = m { Some(*id) } else { None }).collect();
                    if !ids.is_empty() {
                        scoped.scope.place = PlaceFilter::Municipalities(SmallSet::new(ids));
                    }
                }
                Dim::Uf if matches!(scoped.scope.place, PlaceFilter::Any) => {
                    let ids: Vec<_> = members.iter().filter_map(|m| if let Member::Uf(id) = m { Some(*id) } else { None }).collect();
                    if !ids.is_empty() {
                        scoped.scope.place = PlaceFilter::Ufs(SmallSet::new(ids));
                    }
                }
                Dim::Sector if matches!(scoped.scope.activity, ActivityFilter::Any) => {
                    let ids: Vec<_> = members.iter().filter_map(|m| if let Member::Sector(id) = m { Some(*id) } else { None }).collect();
                    let mut cnaes = Vec::new();
                    for id in &ids {
                        if let Some(codes) = self.context.sectors.get(&id.0) {
                            cnaes.extend(codes.iter().map(|&c| CnaeCode::Subclass(c)));
                        }
                    }
                    if !ids.is_empty() && !cnaes.is_empty() {
                        scoped.scope.activity =
                            ActivityFilter::Sectors { ids: SmallSet::new(ids), cnaes: SmallSet::new(cnaes), scope: CnaeScope::Main };
                    }
                }
                _ => {}
            }
        }
        let (values, bases) = self.metric_rows(&scoped, metric, dim)?;
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

    pub fn rank(&self, request: &Request, dim: Dim, dir: Dir, k: u8) -> Result<Value> {
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

    pub fn breakdown(&self, request: &Request, dim: Dim) -> Result<Value> {
        let metric = request.op.metric().context("operation metric")?;
        let (values, bases) = self.metric_rows(request, metric, dim)?;
        Ok(Value::Rows(values.into_iter().map(|(key, value)| Row { key, value, base: bases.get(&key).copied().unwrap_or(0) }).collect()))
    }

    pub fn trend(&self, request: &Request, grain: Grain) -> Result<Value> {
        let Some(Metric::Flow(flow)) = request.op.metric() else { bail!("trend needs a flow metric") };
        let buckets = self.flow_buckets(request, flow, grain)?;
        Ok(Value::Rows(buckets.into_iter().map(|(key, value)| Row { key, value, base: 0 }).collect()))
    }

    pub fn metric_rows(&self, request: &Request, metric: Metric, dim: Dim) -> Result<(BTreeMap<u64, i64>, BTreeMap<u64, u64>)> {
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

    pub fn flow_rows(&self, request: &Request, flow: FlowMetric, dim: Dim) -> Result<(BTreeMap<u64, i64>, BTreeMap<u64, u64>)> {
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

    pub fn flow_group(&self, request: &Request, flow: FlowMetric, dim: Dim) -> Result<BTreeMap<u64, i64>> {
        let clone = request_with_flow(request, flow);
        let set = self.set(&clone)?;
        self.group_dim(&clone, dim, &set)
    }

    pub fn set(&self, request: &Request) -> Result<RoaringBitmap> {
        // The place is resolved by the oracle (`oracle::place`), not by `resolve_place` and
        // `lower`, so a place-resolution bug in the executor cannot hide behind shared code.
        let mut placeless = request.clone();
        placeless.scope.place = PlaceFilter::Any;
        placeless.scope.within_ufs = SmallSet::default();
        let mut pairs = lower_indexed(&placeless, self.context.municipalities)?;
        let place = super::geo::place(self.context.municipalities, &request.scope.place, &request.scope.within_ufs)?;
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
        let candidates = search::candidates(self.snapshot, &query, &self.limits)?;
        ensure!(!candidates.truncated, "query expansion was truncated");
        let mut set = candidates.all.unwrap_or_else(|| RoaringBitmap::from_iter(0..self.snapshot.doc_count()));
        if let Some(excluded) = self.exclusions(request)? {
            set -= excluded;
        }
        if let Some(prefix) = &request.scope.name_prefix {
            set = self.name_matches(&set, prefix)?;
        }
        Ok(set)
    }

    fn name_matches(&self, candidates: &RoaringBitmap, name: &NamePrefix) -> Result<RoaringBitmap> {
        // The token index expands prefixes at any position and may truncate them.
        // Read only the already scoped records to check the full name exactly.
        ensure!(
            candidates.len() <= 25_000,
            "a busca pelo início do nome precisa de um recorte menor. Informe a atividade ou outro filtro cadastral"
        );
        let mut matches = RoaringBitmap::new();
        let mut ids = candidates.iter();
        loop {
            let batch: Vec<_> = ids.by_ref().take(4096).collect();
            if batch.is_empty() {
                break;
            }
            for record in self.snapshot.docs(&batch)? {
                let legal = name.field != NameField::Trade
                    && record.company.empresas.first().is_some_and(|row| name_starts_with(&row[col::emp::RAZAO_SOCIAL], &name.prefix));
                let trade = name.field != NameField::Legal
                    && record
                        .estab
                        .is_some_and(|i| name_starts_with(&record.company.estabelecimentos[i][col::est::NOME_FANTASIA], &name.prefix));
                if legal || trade {
                    matches.insert(record.doc);
                }
            }
        }
        Ok(matches)
    }

    pub fn exclusions(&self, request: &Request) -> Result<Option<RoaringBitmap>> {
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

    pub fn filtered_count(&self, request: &Request, set: &RoaringBitmap) -> Result<u64> {
        let Some(range) = request.scope.place_population else { return Ok(set.len()) };
        let reference = request.scope.population_ref.unwrap_or_else(|| self.context.municipalities.default_population());
        let range = (range.from.unwrap_or(0), range.to.unwrap_or(u32::MAX));
        Ok(self
            .municipality_groups(request, set)?
            .into_iter()
            .filter(|(id, _)| self.population_ok(*id, range, reference))
            .map(|(_, count)| count)
            .sum())
    }

    pub fn group_counts(&self, set: &RoaringBitmap, field: Field) -> Result<Vec<(String, u64)>> {
        if set.len() <= 500_000
            && let Some(col) = self.snapshot.doc_values(field)
        {
            let counts = self.snapshot.doc_value_counts(col, set.iter())?;
            return Ok(col.values.iter().zip(&counts[1..]).filter(|(_, n)| **n > 0).map(|(v, n)| (v.clone(), *n)).collect());
        }
        Ok(search::group_counts_postings(self.snapshot, Some(set), field)?)
    }

    pub fn municipality_groups(&self, request: &Request, set: &RoaringBitmap) -> Result<BTreeMap<u16, u64>> {
        if set.is_empty() {
            return Ok(BTreeMap::new());
        }
        let place = super::geo::place(self.context.municipalities, &request.scope.place, &request.scope.within_ufs)?;
        let mut groups = BTreeMap::new();
        if set.len() <= 500_000 && self.snapshot.doc_values(Field::Municipio).is_some() {
            for (tom, count) in self.group_counts(set, Field::Municipio)? {
                if let Ok(tom) = tom.parse::<u16>()
                    && let Some(row) = self.context.municipalities.by_tom(tom)
                    && let Level::Municipality(geo) = row.place_level()
                    && place.admits(row.id, &geo.uf)
                    && !request.scope.exclude.municipalities.contains(&MunicipalityId(row.id))
                {
                    groups.insert(row.id, count);
                }
            }
            return Ok(groups);
        }
        if place.municipalities.is_some() || place.ufs.is_some() {
            for row in self.context.municipalities.rows() {
                if let Level::Municipality(geo) = row.place_level()
                    && place.admits(row.id, &geo.uf)
                    && !request.scope.exclude.municipalities.contains(&MunicipalityId(row.id))
                    && let Some(postings) = self.snapshot.postings(Field::Municipio, format!("{:04}", row.tom).as_bytes())?
                {
                    let count = set.intersection_len(&postings);
                    if count > 0 {
                        groups.insert(row.id, count);
                    }
                }
            }
            return Ok(groups);
        }
        for (tom, count) in self.group_counts(set, Field::Municipio)? {
            if let Ok(tom) = tom.parse::<u16>()
                && let Some(row) = self.context.municipalities.by_tom(tom)
            {
                groups.insert(row.id, count);
            }
        }
        Ok(groups)
    }

    pub fn population_ok(&self, id: u16, range: (u32, u32), reference: PopulationRef) -> bool {
        let geo = match &self.context.municipalities.get(id).map(|row| &row.place) {
            Some(opencnpj_insights::catalog::municipalities::Place::Municipality(geo)) => Some(geo),
            _ => None,
        };
        geo.and_then(|geo| super::geo::population_of(geo, reference)).is_some_and(|value| (range.0..=range.1).contains(&value))
    }

    pub fn group_dim(&self, request: &Request, dim: Dim, set: &RoaringBitmap) -> Result<BTreeMap<u64, i64>> {
        if set.is_empty() {
            return Ok(BTreeMap::new());
        }
        if request.scope.place_population.is_some() {
            return self.group_dim_population(request, dim, set);
        }
        match dim {
            Dim::Municipality => {
                let mut out = BTreeMap::new();
                for (id, count) in self.municipality_groups(request, set)? {
                    out.insert(u64::from(id), i64::try_from(count)?);
                }
                Ok(out)
            }
            Dim::Uf => self.group_simple(set, Field::Uf, |value| agg::uf_id(value).map(|uf| u64::from(uf.0))),
            Dim::Cnae(CnaeLevel::Section) => {
                // Sections are counted with the engine's own section filter (`cnae=<letter>`),
                // summing division counts that match each section's 2-digit prefix range.
                let div_counts =
                    self.group_simple(set, Field::Cnae, |value| value.parse::<u32>().ok().map(|cnae| cnae_key(CnaeLevel::Division, cnae)))?;
                let mut out = BTreeMap::new();
                for letter in b'A'..=b'U' {
                    let pat = search::filter_pattern(Field::Cnae, &char::from(letter).to_string())?;
                    let mut count = 0i64;
                    for (_, t) in &pat.prefixes {
                        if let Ok(div) = t.parse::<u64>()
                            && let Some(n) = div_counts.get(&div)
                        {
                            count += *n;
                        }
                    }
                    if count > 0 {
                        out.insert(u64::from(letter), count);
                    }
                }
                Ok(out)
            }
            Dim::Cnae(level) => self.group_simple(set, Field::Cnae, |value| value.parse::<u32>().ok().map(|cnae| cnae_key(level, cnae))),
            Dim::Sector => {
                if let ActivityFilter::Sectors { cnaes, scope: CnaeScope::Main, .. } = &request.scope.activity
                    && !cnaes.is_empty()
                {
                    let mut out = BTreeMap::new();
                    for code in cnaes.iter() {
                        if let CnaeCode::Subclass(cnae) = code
                            && let Some(postings) = self.snapshot.postings(Field::Cnae, format!("{cnae:07}").as_bytes())?
                        {
                            let count = i64::try_from(set.intersection_len(&postings))?;
                            if count > 0 {
                                for sector in self.context.sectors_of(*cnae) {
                                    *out.entry(u64::from(*sector)).or_default() += count;
                                }
                            }
                        }
                    }
                    return Ok(out);
                }
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
                let groups = self.municipality_groups(request, set)?;
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

    pub fn group_dim_population(&self, request: &Request, dim: Dim, set: &RoaringBitmap) -> Result<BTreeMap<u64, i64>> {
        let reference = request.scope.population_ref.unwrap_or_else(|| self.context.municipalities.default_population());
        let range = request.scope.place_population.map(|range| (range.from.unwrap_or(0), range.to.unwrap_or(u32::MAX))).unwrap();
        let groups = self
            .municipality_groups(request, set)?
            .into_iter()
            .filter(|(id, _)| self.population_ok(*id, range, reference))
            .collect::<Vec<_>>();
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

    pub fn group_simple(&self, set: &RoaringBitmap, field: Field, map: impl Fn(&str) -> Option<u64>) -> Result<BTreeMap<u64, i64>> {
        if set.is_empty() {
            return Ok(BTreeMap::new());
        }
        let mut out = BTreeMap::new();
        for (value, count) in self.group_counts(set, field)? {
            if let Some(key) = map(&value) {
                *out.entry(key).or_default() += i64::try_from(count)?;
            }
        }
        Ok(out)
    }

    pub fn region_of(&self, id: u16, level: RegionLevel) -> Option<u64> {
        let row = self.context.municipalities.get(id)?;
        match (&row.place_level(), level) {
            (Level::Municipality(geo), RegionLevel::Major) => super::geo::macro_region(&geo.uf).map(u64::from),
            (Level::Municipality(geo), RegionLevel::Intermediate) => Some(u64::from(geo.intermediate_region)),
            (Level::Municipality(geo), RegionLevel::Immediate) => Some(u64::from(geo.immediate_region)),
            _ => None,
        }
    }

    pub fn place_population(&self, request: &Request, reference: PopulationRef) -> u64 {
        super::geo::population(self.context.municipalities, request, None, reference).expect("oracle population")
    }

    pub fn member_population(&self, request: &Request, key: u64, dim: Dim, _metric: Metric) -> u64 {
        let reference = request.scope.population_ref.unwrap_or_else(|| match request.op.metric() {
            Some(Metric::Per10k(reference) | Metric::InhabitantsPer(reference)) => reference,
            _ => self.context.municipalities.default_population(),
        });
        super::geo::population(self.context.municipalities, request, Some((dim, key)), reference).expect("oracle population")
    }
}

pub enum Level<'a> {
    Municipality(&'a opencnpj_insights::catalog::municipalities::Geography),
    Abroad,
}

pub trait PlaceLevel {
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

pub fn cnae_key(level: CnaeLevel, cnae: u32) -> u64 {
    match level {
        CnaeLevel::Section => unreachable!("sections are counted with the engine's section filter"),
        CnaeLevel::Division => u64::from(cnae / 100_000),
        CnaeLevel::Group => u64::from(cnae / 10_000),
        CnaeLevel::Class => u64::from(cnae / 100),
        CnaeLevel::Subclass => u64::from(cnae),
    }
}

pub fn member_key(member: &Member) -> u64 {
    match member {
        Member::Municipality(id) => u64::from(id.0),
        Member::Uf(id) => u64::from(id.0),
        Member::Region(id) => u64::from(id.0),
        Member::Sector(id) => u64::from(id.0),
    }
}

pub fn compare_ratio(a: Row, b: Row) -> std::cmp::Ordering {
    let left = i128::from(a.value) * i128::from(b.base.max(1));
    let right = i128::from(b.value) * i128::from(a.base.max(1));
    left.cmp(&right)
}

pub fn request_with_flow(request: &Request, flow: FlowMetric) -> Request {
    let mut clone = request.clone();
    match &mut clone.op {
        Op::Count { metric } | Op::Compare { metric, .. } | Op::Rank { metric, .. } | Op::Breakdown { metric, .. } => {
            *metric = Metric::Flow(flow);
        }
        Op::Trend { metric, .. } => *metric = flow,
        _ => {}
    }
    // Only a metric change (the two components of a net flow) needs a new
    // implied status. An explicit same-flow status is part of the user's filter.
    if request.op.metric() != Some(Metric::Flow(flow)) {
        clone.scope.status = Some(clone.op.default_status());
    }
    clone
}

impl EngineExecutor<'_> {
    pub fn flow_count(&self, request: &Request, flow: FlowMetric) -> Result<i64> {
        match flow {
            FlowMetric::NetRegistrations => {
                let registrations = self.set(&request_with_flow(request, FlowMetric::Registrations))?.len();
                let closures = self.set(&request_with_flow(request, FlowMetric::Closures))?.len();
                Ok(i64::try_from(registrations)? - i64::try_from(closures)?)
            }
            _ => Ok(i64::try_from(self.set(&request_with_flow(request, flow))?.len())?),
        }
    }

    pub fn flow_buckets(&self, request: &Request, flow: FlowMetric, grain: Grain) -> Result<BTreeMap<u64, i64>> {
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
        // Current snapshots retain historical registration/status dates.
        // Aggregate monthly buckets start in 2021; older or unaligned periods use the engine.
        flow_days: Day(1)..=last_day,
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
